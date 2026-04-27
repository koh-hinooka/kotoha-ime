//! `Database` 構造体: Mutex<Connection> + Arc 共有 ownership(spec §6.4)。

use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use rusqlite::Connection;

use crate::error::StorageError;
use crate::migrations::apply_migrations;

/// SQLite Database wrapper(spec §6.4)。`Mutex<Connection>` を内部保持し、
/// `Arc<Database>` で複数 Store(`UserVocabReader/Writer` /
/// `LearningCacheReader/Writer`)が同一 Connection を共有する
/// (spec §6.4.1 共有 ownership)。
pub struct Database {
    conn: Mutex<Connection>,
}

impl Database {
    /// `path` に SQLite DB を open / create し、未適用 migration を apply する。
    ///
    /// # Postconditions
    ///
    /// - 戻り値は `Arc<Database>`(spec §6.4 lifetime parameter 不在)
    /// - `path` の親 dir が無い場合は再帰的に作成
    /// - PRAGMA journal_mode=WAL / synchronous=NORMAL / foreign_keys=ON / temp_store=MEMORY を設定
    /// - `PRAGMA user_version` を読取り、`MIGRATIONS` の未適用分を apply
    ///
    /// # Errors
    ///
    /// - [`StorageError::Io`] when parent dir cannot be created
    /// - [`StorageError::Sqlite`] when open / PRAGMA / migration fails
    pub fn open(path: &Path) -> Result<Arc<Self>, StorageError> {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() && !parent.exists() {
                std::fs::create_dir_all(parent)?;
            }
        }
        let conn = Connection::open(path)?;
        Self::configure_pragma(&conn)?;
        apply_migrations(&conn)?;
        // 業務上 5s が SQLite default よりも user-friendly(spec §3 / Phase 3 IBus
        // engine の lock 競合に備えた busy_timeout)。
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        Ok(Arc::new(Self {
            conn: Mutex::new(conn),
        }))
    }

    /// `:memory:` SQLite を open する(test 用、spec §10.1)。
    pub fn open_in_memory() -> Result<Arc<Self>, StorageError> {
        let conn = Connection::open_in_memory()?;
        Self::configure_pragma(&conn)?;
        apply_migrations(&conn)?;
        Ok(Arc::new(Self {
            conn: Mutex::new(conn),
        }))
    }

    /// PRAGMA を設定する(spec §5.3)。
    fn configure_pragma(conn: &Connection) -> Result<(), StorageError> {
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             PRAGMA foreign_keys = ON;
             PRAGMA temp_store = MEMORY;",
        )?;
        Ok(())
    }

    /// 内部 `Mutex<Connection>` を lock する。Store 実装側で使用。
    ///
    /// `pub(crate)` に絞って、外部 crate からは Connection 直アクセスではなく
    /// `UserVocabReader/Writer` / `LearningCacheReader/Writer` 抽象境界を
    /// 通すことを強制する(review A-H2)。
    ///
    /// # Postconditions
    ///
    /// - 戻り値は使用可能な `MutexGuard<Connection>` で、本関数自体は panic を伝播しない
    /// - 他 thread の panic で `Mutex` が poisoned 状態になっていた場合も、
    ///   `PoisonError::into_inner` で guard を回収して continuation する
    ///   (Phase 3 IBus engine の thread cascade death 防止 / sec-F4)
    /// - 各 SQL 文は SQLite の auto-commit により atomic に確定するため、recovery 時点で
    ///   partial-statement state が観測されることはない(本 crate は現時点で明示
    ///   transaction を使用していない)
    pub(crate) fn lock_conn(&self) -> MutexGuard<'_, Connection> {
        self.conn.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// `Arc<Database>` を `Box<dyn UserVocabReader>` として公開する(arch-M-2 ISP split)。
    ///
    /// # Postconditions
    ///
    /// - 戻り値の trait object は内部で `SqliteUserVocabStore` を `Box` で保持し、
    ///   その `SqliteUserVocabStore` は `Arc<Database>` を `Arc::clone` で共有する
    /// - 同一 `Arc<Database>` から生成した reader / writer は同一 `Mutex<Connection>` を共有する
    pub fn user_vocab_reader(
        self: &Arc<Self>,
    ) -> Box<dyn crate::user_vocab::store::UserVocabReader> {
        Box::new(crate::user_vocab::sqlite::SqliteUserVocabStore::new(
            Arc::clone(self),
        ))
    }

    /// `Arc<Database>` を `Box<dyn UserVocabWriter>` として公開する(arch-M-2 ISP split)。
    ///
    /// # Postconditions
    ///
    /// - 戻り値の trait object は内部で `SqliteUserVocabStore` を `Box` で保持し、
    ///   その `SqliteUserVocabStore` は `Arc<Database>` を `Arc::clone` で共有する
    /// - 同一 `Arc<Database>` から生成した reader / writer は同一 `Mutex<Connection>` を共有する
    pub fn user_vocab_writer(
        self: &Arc<Self>,
    ) -> Box<dyn crate::user_vocab::store::UserVocabWriter> {
        Box::new(crate::user_vocab::sqlite::SqliteUserVocabStore::new(
            Arc::clone(self),
        ))
    }

    /// `Arc<Database>` を `Box<dyn LearningCacheReader>` として公開する(spec §3.1 / §4.6)。
    ///
    /// # Postconditions
    ///
    /// - 戻り値の trait object は内部で `SqliteLearningCacheStore` を `Box` で保持し、
    ///   その `SqliteLearningCacheStore` は `Arc<Database>` を `Arc::clone` で共有する
    /// - 同一 `Arc<Database>` から生成した reader / writer は同一 `Mutex<Connection>` を共有する
    pub fn learning_cache_reader(
        self: &Arc<Self>,
    ) -> Box<dyn crate::learning_cache::LearningCacheReader> {
        Box::new(crate::learning_cache::SqliteLearningCacheStore::new(
            Arc::clone(self),
        ))
    }

    /// `Arc<Database>` を `Box<dyn LearningCacheWriter>` として公開する(spec §3.1 / §4.6)。
    ///
    /// # Postconditions
    ///
    /// - 戻り値の trait object は内部で `SqliteLearningCacheStore` を `Box` で保持し、
    ///   その `SqliteLearningCacheStore` は `Arc<Database>` を `Arc::clone` で共有する
    /// - 同一 `Arc<Database>` から生成した reader / writer は同一 `Mutex<Connection>` を共有する
    pub fn learning_cache_writer(
        self: &Arc<Self>,
    ) -> Box<dyn crate::learning_cache::LearningCacheWriter> {
        Box::new(crate::learning_cache::SqliteLearningCacheStore::new(
            Arc::clone(self),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn open_in_memory_returns_arc() {
        let db = Database::open_in_memory().expect("memory open");
        let _: Arc<Database> = db; // type assertion
    }

    #[test]
    fn open_in_memory_applies_v001_migration() {
        let db = Database::open_in_memory().expect("memory open");
        let conn = db.lock_conn();
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
    fn open_in_memory_journal_mode_is_memory() {
        let db = Database::open_in_memory().expect("memory open");
        let conn = db.lock_conn();
        let mode: String = conn
            .query_row("PRAGMA journal_mode", [], |r| r.get(0))
            .unwrap();
        // `:memory:` connection の journal_mode は常に "memory" になる(SQLite spec)。
        // WAL の挙動は file-backed の `open_with_path_sets_wal_journal_mode` 側でカバーする。
        assert_eq!(mode, "memory");
    }

    #[test]
    fn open_with_path_creates_db_file() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = tmp.path().join("test.db");
        let _db = Database::open(&path).expect("open succeeds");
        assert!(path.exists(), "db file must be created");
    }

    #[test]
    fn open_with_path_applies_migration() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = tmp.path().join("test.db");
        let db = Database::open(&path).expect("open succeeds");
        let conn = db.lock_conn();
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
    fn open_with_path_sets_wal_journal_mode() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = tmp.path().join("test.db");
        let db = Database::open(&path).expect("open succeeds");
        let conn = db.lock_conn();
        let mode: String = conn
            .query_row("PRAGMA journal_mode", [], |r| r.get(0))
            .unwrap();
        assert_eq!(mode, "wal");
    }

    #[test]
    fn open_with_path_sets_foreign_keys_on() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = tmp.path().join("test.db");
        let db = Database::open(&path).expect("open succeeds");
        let conn = db.lock_conn();
        let on: i32 = conn
            .query_row("PRAGMA foreign_keys", [], |r| r.get(0))
            .unwrap();
        assert_eq!(on, 1);
    }

    #[test]
    fn open_with_path_creates_parent_directory() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let nested = tmp.path().join("a/b/c");
        let path = nested.join("test.db");
        let _db = Database::open(&path).expect("open succeeds even when parent missing");
        assert!(nested.exists());
    }

    #[test]
    fn idempotent_reopen_does_not_reapply_migration() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = tmp.path().join("test.db");
        let _db1 = Database::open(&path).expect("first open");
        let db2 = Database::open(&path).expect("second open");
        let conn = db2.lock_conn();
        let v: i32 = conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(v, crate::migrations::LATEST_VERSION);
    }

    #[test]
    fn user_vocab_store_factory_returns_owned_box() {
        let db = Database::open_in_memory().expect("memory open");
        let _store: Box<dyn crate::user_vocab::store::UserVocabReader> = db.user_vocab_reader();
    }

    #[test]
    fn user_vocab_store_factory_shares_arc() {
        let db = Database::open_in_memory().expect("memory open");
        let writer = db.user_vocab_writer();
        let reader = db.user_vocab_reader();
        let r = crate::user_vocab::store::UserVocabRecord {
            id: None,
            surface: "x".to_string(),
            reading: "あ".to_string(),
            pos: "名詞".to_string(),
            score: 0.0,
            created_at: 0,
            updated_at: 0,
        };
        writer.insert(r).expect("insert ok");
        let result = reader.find_by_reading("あ", 10).expect("find ok");
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn open_with_path_sets_busy_timeout() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = tmp.path().join("test.db");
        let db = Database::open(&path).expect("ok");
        let conn = db.lock_conn();
        let ms: i64 = conn
            .query_row("PRAGMA busy_timeout", [], |r| r.get(0))
            .unwrap();
        assert!(ms >= 5000, "busy_timeout must be >= 5000ms, got {ms}");
    }

    #[test]
    fn learning_cache_reader_factory_returns_owned_box() {
        let db = Database::open_in_memory().expect("memory open");
        let _reader: Box<dyn crate::learning_cache::LearningCacheReader> =
            db.learning_cache_reader();
    }

    #[test]
    fn learning_cache_writer_factory_returns_owned_box() {
        let db = Database::open_in_memory().expect("memory open");
        let _writer: Box<dyn crate::learning_cache::LearningCacheWriter> =
            db.learning_cache_writer();
    }

    #[test]
    fn learning_cache_factories_share_same_connection() {
        // writer で record_choice → reader で lookup が同一 DB に到達することを確認する
        // (両 factory が同じ `Arc<Database>` の同一 `Mutex<Connection>` を共有している証拠)。
        // 並列実行される他 test の `CapOverrideGuard` が record_choice 内の自動 eviction を
        // 小さな cap で発火させないように `CAP_OVERRIDE_LOCK` を取得する。
        let _lock = crate::learning_cache::sqlite::CapOverrideGuard::lock_only();
        let db = Database::open_in_memory().expect("memory open");
        let writer = db.learning_cache_writer();
        let reader = db.learning_cache_reader();
        writer.record_choice("あい", "愛").expect("record ok");
        let result = reader.lookup("あい", 10).expect("lookup ok");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].chosen_kanji, "愛");
    }

    #[test]
    fn parallel_inserts_via_arc_share_does_not_deadlock() {
        use std::thread;

        let db = Database::open_in_memory().expect("memory open");
        let writer = Arc::new(db.user_vocab_writer());
        let handles: Vec<_> = (0..8u32)
            .map(|i| {
                let writer = Arc::clone(&writer);
                thread::spawn(move || {
                    // hiragana を index から計算する(ASCII 数字混入を避ける)。
                    // U+3042 = 'あ' 起点で、あ/い/う/え/お/か/き/く を割り当てる。
                    let reading = char::from_u32(0x3042 + i)
                        .expect("valid hiragana code point")
                        .to_string();
                    let r = crate::user_vocab::store::UserVocabRecord {
                        id: None,
                        surface: format!("s{i}"),
                        reading,
                        pos: "名詞".to_string(),
                        score: 0.0,
                        created_at: 0,
                        updated_at: 0,
                    };
                    writer.insert(r).expect("insert ok");
                })
            })
            .collect();
        for h in handles {
            h.join().unwrap();
        }
        let reader = db.user_vocab_reader();
        let result = reader.list_all(100, 0).unwrap();
        assert_eq!(result.len(), 8);
    }

    /// `lock_conn` が poisoned `Mutex` から recovery することを実測検証する。
    ///
    /// # シナリオ
    ///
    /// Phase 3 IBus engine は input dispatch / auto-eviction / UI thread を併走させる
    /// multi-threaded 構成である。いずれかの thread が `Mutex<Connection>` を保持中に
    /// panic すると、`Mutex` は poisoned となり、後続の `Mutex::lock()` は `Err(PoisonError)`
    /// を返す。修正前の `.expect("Database mutex poisoned")` は その `Err` を panic に
    /// 変換していたため、1 件の panic が他 thread にも cascade death を誘発していた。
    ///
    /// # 実測検証
    ///
    /// 本 test は旧実装(`.expect`)上では「2 回目の `lock_conn()` が panic」によって
    /// FAIL する性質を持ち、修正後の `unwrap_or_else(PoisonError::into_inner)` でのみ
    /// PASS する。`PRAGMA user_version` の round-trip が、recovery 後の Connection が
    /// 引き続き使用可能であることの smoke check として機能する。
    #[test]
    fn lock_conn_recovers_from_poisoned_mutex() {
        let db = Database::open_in_memory().expect("memory open");

        let db_clone = Arc::clone(&db);
        let handle = std::thread::spawn(move || {
            let guard = db_clone.lock_conn();
            // SQL を 1 回 round-trip させ、guard が optimizer に elide されないようにする。
            let _: i32 = guard
                .query_row("PRAGMA user_version", [], |r| r.get(0))
                .expect("query within poisoning thread must succeed");
            // guard を保持したまま panic することで、unwind 時に Mutex が poisoned になる。
            panic!("intentional panic to poison the mutex");
        });
        let join_result = handle.join();
        assert!(
            join_result.is_err(),
            "spawned thread must have panicked to poison the mutex"
        );

        // 修正後の `lock_conn` は poisoned `Mutex` から recovery し、panic を伝播しない。
        // 旧実装(`.expect`)では本行で 2 段目の panic が発生し、test は FAIL する。
        let conn = db.lock_conn();
        let version: i32 = conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .expect("recovered connection must remain usable");
        assert_eq!(version, crate::migrations::LATEST_VERSION);
    }
}
