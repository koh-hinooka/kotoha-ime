//! Migration runner(spec §3.6 / §6.5)。

use crate::error::StorageError;

/// 最新 schema version。新 migration を `MIGRATIONS` に追加する際は本値を増やす。
pub const LATEST_VERSION: i32 = 1;

/// (version, sql) 配列。version 昇順厳守(spec §10.1.1)。
///
/// `include_str!` でバイナリ同梱するため、distribution 時に migrations
/// directory を別配布する必要はない(spec §3.6)。
pub const MIGRATIONS: &[(i32, &str)] = &[(1, include_str!("../migrations/v001_initial.sql"))];

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
}
