//! `Database` 構造体: Mutex<Connection> + Arc 共有 ownership(spec §6.4)。

use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard};

use rusqlite::Connection;

use crate::error::StorageError;
use crate::migrations::apply_migrations;

/// SQLite Database wrapper(spec §6.4)。`Mutex<Connection>` を内部保持し、
/// `Arc<Database>` で複数 Store(`UserVocabStore` / `LearningCacheStore`)が
/// 同一 Connection を共有する(spec §6.4.1 共有 ownership)。
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
    /// # Panics
    ///
    /// poison された場合 panic する(other thread が panic 中に lock を保持していた場合)。
    pub fn lock_conn(&self) -> MutexGuard<'_, Connection> {
        self.conn.lock().expect("Database mutex poisoned")
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
    fn open_in_memory_sets_pragma_journal_mode_wal() {
        let db = Database::open_in_memory().expect("memory open");
        let conn = db.lock_conn();
        let mode: String = conn
            .query_row("PRAGMA journal_mode", [], |r| r.get(0))
            .unwrap();
        // memory or wal のどちらかが返る(:memory: の場合 memory)
        assert!(mode == "memory" || mode == "wal");
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
}
