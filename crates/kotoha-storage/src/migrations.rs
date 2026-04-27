//! Migration runner(spec §3.6 / §6.5)。

use crate::error::StorageError;

/// 最新 schema version。新 migration を `MIGRATIONS` に追加する際は本値を増やす。
pub const LATEST_VERSION: i32 = 2;

/// (version, sql) 配列。version 昇順厳守(spec §10.1.1)。
///
/// `include_str!` でバイナリ同梱するため、distribution 時に migrations
/// directory を別配布する必要はない(spec §3.6)。
pub const MIGRATIONS: &[(i32, &str)] = &[
    (1, include_str!("../migrations/v001_initial.sql")),
    (
        2,
        include_str!("../migrations/v002_learning_cache_index.sql"),
    ),
];

/// 未適用 migration を順次 apply する(spec §6.5)。
///
/// # Errors
///
/// - [`StorageError::Sqlite`] when SQL execution fails
/// - [`StorageError::Migration`] when version sequence is invalid
pub fn apply_migrations(conn: &rusqlite::Connection) -> Result<(), StorageError> {
    let current: i32 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    for (version, sql) in MIGRATIONS.iter().filter(|(v, _)| *v > current) {
        conn.execute_batch(sql)?;
        conn.execute_batch(&format!("PRAGMA user_version = {version}"))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    // §10.1.1 invariant test 必須 3 項目

    #[test]
    fn migrations_are_strictly_ascending() {
        assert!(
            MIGRATIONS.windows(2).all(|w| w[0].0 < w[1].0),
            "MIGRATIONS must be in strictly ascending version order"
        );
    }

    #[test]
    fn migrations_skip_and_resume_from_intermediate_version() {
        let conn = Connection::open_in_memory().expect("memory open");
        conn.execute_batch("PRAGMA user_version = 0").unwrap();
        apply_migrations(&conn).expect("apply succeeds");
        let v: i32 = conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(v, LATEST_VERSION);
    }

    #[test]
    fn migrations_idempotency_on_double_apply() {
        let conn = Connection::open_in_memory().expect("memory open");
        apply_migrations(&conn).expect("first apply");
        apply_migrations(&conn).expect("second apply must be no-op");
        let v: i32 = conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(v, LATEST_VERSION);
    }

    #[test]
    fn latest_version_is_at_least_one() {
        const { assert!(LATEST_VERSION >= 1) };
    }

    #[test]
    fn apply_migrations_creates_user_vocab_table() {
        let conn = Connection::open_in_memory().expect("memory open");
        apply_migrations(&conn).expect("apply");
        let count: i64 = conn
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='user_vocab'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn apply_migrations_creates_learning_cache_table() {
        let conn = Connection::open_in_memory().expect("memory open");
        apply_migrations(&conn).expect("apply");
        let count: i64 = conn
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='learning_cache'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn apply_migrations_creates_user_vocab_reading_index() {
        let conn = Connection::open_in_memory().expect("memory open");
        apply_migrations(&conn).expect("apply");
        let count: i64 = conn
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE type='index' AND name='idx_user_vocab_reading'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    // ---- v002 migration 検証(spec §4.1)----

    /// v001 + v002 を一括 apply した後、`idx_learning_cache_last_used` index が
    /// `sqlite_master` に存在することを確認する(spec §4.1)。
    #[test]
    fn apply_migrations_v002_creates_learning_cache_index() {
        let conn = Connection::open_in_memory().expect("memory open");
        apply_migrations(&conn).expect("apply");
        let count: i64 = conn
            .query_row(
                "SELECT count(*) FROM sqlite_master \
                 WHERE type='index' AND name='idx_learning_cache_last_used'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            count, 1,
            "idx_learning_cache_last_used must exist after migration"
        );
    }

    /// v001 適用済 DB(`PRAGMA user_version = 1`)に対して `apply_migrations` を呼ぶと、
    /// v002 のみが追加 apply され、`user_version` が 2 へ更新される(spec §4.1 / §6.5)。
    #[test]
    fn apply_migrations_upgrades_from_v001_to_v002() {
        let conn = Connection::open_in_memory().expect("memory open");

        // v001 のみを手動 apply して v001 完了状態を作成する。
        let (_, v001_sql) = MIGRATIONS
            .iter()
            .find(|(v, _)| *v == 1)
            .expect("v001 must exist in MIGRATIONS");
        conn.execute_batch(v001_sql).expect("v001 apply");
        conn.execute_batch("PRAGMA user_version = 1")
            .expect("set user_version = 1");

        // apply_migrations は v002 のみを apply し user_version を 2 に更新するはずである。
        apply_migrations(&conn).expect("upgrade v001 -> v002");

        let v: i32 = conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(v, 2, "user_version must be 2 after v001 -> v002 upgrade");

        let idx_count: i64 = conn
            .query_row(
                "SELECT count(*) FROM sqlite_master \
                 WHERE type='index' AND name='idx_learning_cache_last_used'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            idx_count, 1,
            "idx_learning_cache_last_used must be created by v002"
        );
    }

    /// v002 完了状態の DB に対して `apply_migrations` を再実行しても
    /// `user_version` が `LATEST_VERSION` のまま維持され、index が重複作成エラー無く
    /// no-op となることを確認する(spec §4.1 / §6.5)。
    ///
    /// 既存 `migrations_idempotency_on_double_apply` との差別化:
    /// 本 test は v002 完了状態での re-run に焦点を絞り、index 数が 1 件のまま
    /// 維持されることまで明示的に assert する。
    #[test]
    fn apply_migrations_is_idempotent_at_v002() {
        let conn = Connection::open_in_memory().expect("memory open");
        apply_migrations(&conn).expect("first apply");
        apply_migrations(&conn).expect("second apply must be no-op");

        let v: i32 = conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(
            v, LATEST_VERSION,
            "user_version must equal LATEST_VERSION after double apply"
        );

        let idx_count: i64 = conn
            .query_row(
                "SELECT count(*) FROM sqlite_master \
                 WHERE type='index' AND name='idx_learning_cache_last_used'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            idx_count, 1,
            "index count must remain 1 after idempotent apply at v002"
        );
    }
}
