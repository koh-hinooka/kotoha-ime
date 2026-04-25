//! SqliteUserVocabStore: UserVocabStore の SQLite 実装(spec §6.1 / §6.4)。

use std::sync::Arc;

use crate::database::Database;
use crate::error::StorageError;
use crate::user_vocab::store::{UserVocabRecord, UserVocabStore};
use crate::validation::{validate_pos, validate_reading, validate_score, validate_surface};

/// User vocab 行数上限(sec-M5、spec §F6)。
pub const USER_VOCAB_MAX_ROWS: usize = 50_000;

/// テスト時の上限上書き(0 = unset → `USER_VOCAB_MAX_ROWS` を使用)。
///
/// `cargo test` ビルドでのみ意味を持ち、production binary には含まれない。
/// 50,000 行 を実際に挿入するテストは時間 / メモリの観点で非現実的なため、
/// 単体テストはこの override を介して小さな上限値で QuotaExceeded を検証する。
#[cfg(test)]
pub(crate) static USER_VOCAB_MAX_ROWS_TEST_OVERRIDE: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);

/// override の serialization 用 mutex(複数 test が同時 override しないよう直列化)。
#[cfg(test)]
pub(crate) static QUOTA_OVERRIDE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// 行数上限の effective value を返す(`#[cfg(test)]` 時のみ override を考慮)。
#[inline]
pub(crate) fn effective_max_rows() -> usize {
    #[cfg(test)]
    {
        let v = USER_VOCAB_MAX_ROWS_TEST_OVERRIDE.load(std::sync::atomic::Ordering::SeqCst);
        if v != 0 {
            return v;
        }
    }
    USER_VOCAB_MAX_ROWS
}

/// テスト中だけ `USER_VOCAB_MAX_ROWS` を `cap` に上書きする RAII guard。
///
/// drop 時に自動で 0(無効)に戻すので、test 同士の干渉を防げる。
/// override は process 全体の static なため、複数 test が同時に
/// override を活性化すると競合する。よって `QUOTA_OVERRIDE_LOCK` を
/// 取得し直列化する。
#[cfg(test)]
pub(crate) struct QuotaOverrideGuard {
    _lock: std::sync::MutexGuard<'static, ()>,
}

#[cfg(test)]
impl QuotaOverrideGuard {
    pub(crate) fn new(cap: usize) -> Self {
        // poison していても続行(直前 test の panic でも次 test を回したい)。
        let lock = QUOTA_OVERRIDE_LOCK
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        USER_VOCAB_MAX_ROWS_TEST_OVERRIDE.store(cap, std::sync::atomic::Ordering::SeqCst);
        Self { _lock: lock }
    }
}

#[cfg(test)]
impl Drop for QuotaOverrideGuard {
    fn drop(&mut self) {
        USER_VOCAB_MAX_ROWS_TEST_OVERRIDE.store(0, std::sync::atomic::Ordering::SeqCst);
    }
}

pub struct SqliteUserVocabStore {
    pub(crate) db: Arc<Database>,
}

impl SqliteUserVocabStore {
    pub fn new(db: Arc<Database>) -> Self {
        Self { db }
    }
}

impl UserVocabStore for SqliteUserVocabStore {
    fn find_by_reading(
        &self,
        reading: &str,
        limit: usize,
    ) -> Result<Vec<UserVocabRecord>, StorageError> {
        validate_reading(reading)?;
        let conn = self.db.lock_conn();
        let mut stmt = conn.prepare(
            "SELECT id, surface, reading, pos, score, created_at, updated_at \
             FROM user_vocab \
             WHERE reading = ?1 \
             ORDER BY score DESC \
             LIMIT ?2",
        )?;
        let rows = stmt.query_map(rusqlite::params![reading, limit as i64], |row| {
            Ok(UserVocabRecord {
                id: Some(row.get(0)?),
                surface: row.get(1)?,
                reading: row.get(2)?,
                pos: row.get(3)?,
                score: row.get::<_, f64>(4)? as f32,
                created_at: row.get(5)?,
                updated_at: row.get(6)?,
            })
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    fn list_all(&self, limit: usize, offset: usize) -> Result<Vec<UserVocabRecord>, StorageError> {
        let conn = self.db.lock_conn();
        let mut stmt = conn.prepare(
            "SELECT id, surface, reading, pos, score, created_at, updated_at \
             FROM user_vocab \
             ORDER BY id ASC \
             LIMIT ?1 OFFSET ?2",
        )?;
        let rows = stmt.query_map(rusqlite::params![limit as i64, offset as i64], |row| {
            Ok(UserVocabRecord {
                id: Some(row.get(0)?),
                surface: row.get(1)?,
                reading: row.get(2)?,
                pos: row.get(3)?,
                score: row.get::<_, f64>(4)? as f32,
                created_at: row.get(5)?,
                updated_at: row.get(6)?,
            })
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    fn insert(&self, record: UserVocabRecord) -> Result<i64, StorageError> {
        validate_surface(&record.surface)?;
        validate_reading(&record.reading)?;
        validate_pos(&record.pos)?;
        validate_score(record.score)?;

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        let created_at = if record.created_at == 0 {
            now
        } else {
            record.created_at
        };
        let updated_at = if record.updated_at == 0 {
            now
        } else {
            record.updated_at
        };

        let conn = self.db.lock_conn();
        // 行数上限チェック(sec-M5、spec §F6)。check + INSERT を同一 lock 下で
        // 実行することで atomic な check-and-insert を保証する。
        let max_rows = effective_max_rows();
        let count: i64 = conn.query_row("SELECT count(*) FROM user_vocab", [], |r| r.get(0))?;
        if count as usize >= max_rows {
            return Err(StorageError::QuotaExceeded {
                table: "user_vocab".to_string(),
                max: max_rows,
            });
        }
        let result = conn.execute(
            "INSERT INTO user_vocab (surface, reading, pos, score, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![
                record.surface,
                record.reading,
                record.pos,
                record.score as f64,
                created_at,
                updated_at,
            ],
        );
        match result {
            Ok(_) => Ok(conn.last_insert_rowid()),
            Err(rusqlite::Error::SqliteFailure(e, _))
                if e.code == rusqlite::ErrorCode::ConstraintViolation =>
            {
                Err(StorageError::DuplicateEntry {
                    surface: record.surface,
                    reading: record.reading,
                })
            }
            Err(e) => Err(StorageError::Sqlite(e)),
        }
    }

    fn delete_by_id(&self, id: i64) -> Result<(), StorageError> {
        let conn = self.db.lock_conn();
        let affected = conn.execute(
            "DELETE FROM user_vocab WHERE id = ?1",
            rusqlite::params![id],
        )?;
        if affected == 0 {
            return Err(StorageError::NotFound);
        }
        Ok(())
    }

    fn delete_by_surface_reading(&self, surface: &str, reading: &str) -> Result<(), StorageError> {
        validate_surface(surface)?;
        validate_reading(reading)?;
        let conn = self.db.lock_conn();
        let affected = conn.execute(
            "DELETE FROM user_vocab WHERE surface = ?1 AND reading = ?2",
            rusqlite::params![surface, reading],
        )?;
        if affected == 0 {
            return Err(StorageError::NotFound);
        }
        Ok(())
    }

    fn find_by_prefix(
        &self,
        reading_prefix: &str,
        limit: usize,
    ) -> Result<Vec<UserVocabRecord>, StorageError> {
        // prefix は hiragana / 空文字どちらも許容(空 prefix = 全件)
        if !reading_prefix.is_empty() {
            validate_reading(reading_prefix)?;
        }
        let conn = self.db.lock_conn();
        let pattern = format!("{}%", reading_prefix);
        let mut stmt = conn.prepare(
            "SELECT id, surface, reading, pos, score, created_at, updated_at \
             FROM user_vocab \
             WHERE reading LIKE ?1 \
             ORDER BY score DESC, id ASC \
             LIMIT ?2",
        )?;
        let rows = stmt.query_map(rusqlite::params![pattern, limit as i64], |row| {
            Ok(UserVocabRecord {
                id: Some(row.get(0)?),
                surface: row.get(1)?,
                reading: row.get(2)?,
                pos: row.get(3)?,
                score: row.get::<_, f64>(4)? as f32,
                created_at: row.get(5)?,
                updated_at: row.get(6)?,
            })
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh_store() -> SqliteUserVocabStore {
        let db = Database::open_in_memory().expect("memory open");
        SqliteUserVocabStore::new(db)
    }

    fn seed_row(store: &SqliteUserVocabStore, surface: &str, reading: &str, score: f32) {
        let conn = store.db.lock_conn();
        conn.execute(
            "INSERT INTO user_vocab (surface, reading, pos, score, created_at, updated_at) \
             VALUES (?1, ?2, '名詞', ?3, 0, 0)",
            rusqlite::params![surface, reading, score as f64],
        )
        .unwrap();
    }

    #[test]
    fn find_by_reading_returns_empty_for_unknown() {
        let store = fresh_store();
        let result = store.find_by_reading("みず", 10).expect("ok");
        assert!(result.is_empty());
    }

    #[test]
    fn find_by_reading_returns_single_match() {
        let store = fresh_store();
        seed_row(&store, "日野岡", "ひのおか", 1.0);
        let result = store.find_by_reading("ひのおか", 10).expect("ok");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].surface, "日野岡");
    }

    #[test]
    fn find_by_reading_orders_by_score_desc() {
        let store = fresh_store();
        seed_row(&store, "日野岡", "ひのおか", 0.5);
        seed_row(&store, "ひの岡", "ひのおか", 1.5);
        let result = store.find_by_reading("ひのおか", 10).expect("ok");
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].surface, "ひの岡"); // higher score
        assert_eq!(result[1].surface, "日野岡");
    }

    #[test]
    fn find_by_reading_respects_limit() {
        let store = fresh_store();
        for i in 0..5 {
            seed_row(&store, &format!("s{}", i), "ひのおか", i as f32);
        }
        let result = store.find_by_reading("ひのおか", 3).expect("ok");
        assert_eq!(result.len(), 3);
    }

    #[test]
    fn find_by_reading_rejects_non_hiragana() {
        let store = fresh_store();
        let err = store.find_by_reading("カタカナ", 10).unwrap_err();
        assert!(matches!(err, StorageError::InvalidField { .. }));
    }

    #[test]
    fn insert_assigns_auto_increment_id() {
        let store = fresh_store();
        let r = UserVocabRecord {
            id: None,
            surface: "日野岡".to_string(),
            reading: "ひのおか".to_string(),
            pos: "名詞-固有名詞-人名".to_string(),
            score: 1.0,
            created_at: 0,
            updated_at: 0,
        };
        let id = store.insert(r).expect("insert ok");
        assert!(id >= 1);
    }

    #[test]
    fn insert_persists_then_can_find() {
        let store = fresh_store();
        let r = UserVocabRecord {
            id: None,
            surface: "琴葉".to_string(),
            reading: "ことば".to_string(),
            pos: "名詞-固有名詞-人名".to_string(),
            score: 1.0,
            created_at: 0,
            updated_at: 0,
        };
        store.insert(r).expect("insert ok");
        let result = store.find_by_reading("ことば", 10).expect("find ok");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].surface, "琴葉");
    }

    #[test]
    fn insert_duplicate_returns_duplicate_entry_error() {
        let store = fresh_store();
        let r1 = UserVocabRecord {
            id: None,
            surface: "日野岡".to_string(),
            reading: "ひのおか".to_string(),
            pos: "名詞".to_string(),
            score: 1.0,
            created_at: 0,
            updated_at: 0,
        };
        store.insert(r1.clone()).expect("first insert ok");
        let err = store.insert(r1).unwrap_err();
        match err {
            StorageError::DuplicateEntry { surface, reading } => {
                assert_eq!(surface, "日野岡");
                assert_eq!(reading, "ひのおか");
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn insert_rejects_invalid_reading() {
        let store = fresh_store();
        let r = UserVocabRecord {
            id: None,
            surface: "X".to_string(),
            reading: "abc".to_string(), // ASCII reject
            pos: "名詞".to_string(),
            score: 1.0,
            created_at: 0,
            updated_at: 0,
        };
        let err = store.insert(r).unwrap_err();
        assert!(matches!(err, StorageError::InvalidField { .. }));
    }

    #[test]
    fn insert_rejects_negative_score() {
        let store = fresh_store();
        let r = UserVocabRecord {
            id: None,
            surface: "X".to_string(),
            reading: "あ".to_string(),
            pos: "名詞".to_string(),
            score: -1.0,
            created_at: 0,
            updated_at: 0,
        };
        let err = store.insert(r).unwrap_err();
        assert!(matches!(err, StorageError::InvalidField { .. }));
    }

    #[test]
    fn list_all_returns_all_rows_when_no_filter() {
        let store = fresh_store();
        for i in 0..5 {
            seed_row(&store, &format!("s{}", i), &format!("あ{}", i), i as f32);
        }
        let result = store.list_all(100, 0).expect("ok");
        assert_eq!(result.len(), 5);
    }

    #[test]
    fn list_all_respects_limit_and_offset() {
        let store = fresh_store();
        for i in 0..10 {
            seed_row(&store, &format!("s{}", i), &format!("あ{}", i), i as f32);
        }
        let result = store.list_all(3, 2).expect("ok");
        assert_eq!(result.len(), 3);
    }

    #[test]
    fn list_all_returns_empty_when_offset_past_end() {
        let store = fresh_store();
        seed_row(&store, "x", "あ", 0.0);
        let result = store.list_all(10, 100).expect("ok");
        assert!(result.is_empty());
    }

    #[test]
    fn find_by_prefix_matches_partial() {
        let store = fresh_store();
        seed_row(&store, "日野岡", "ひのおか", 1.0);
        seed_row(&store, "日野", "ひの", 0.5);
        seed_row(&store, "別人", "べつじん", 0.5);
        let result = store.find_by_prefix("ひの", 100).expect("ok");
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn delete_by_id_removes_existing_row() {
        let store = fresh_store();
        let r = UserVocabRecord {
            id: None,
            surface: "x".to_string(),
            reading: "あ".to_string(),
            pos: "名詞".to_string(),
            score: 0.0,
            created_at: 0,
            updated_at: 0,
        };
        let id = store.insert(r).expect("insert");
        store.delete_by_id(id).expect("delete ok");
        let result = store.find_by_reading("あ", 10).expect("find ok");
        assert!(result.is_empty());
    }

    #[test]
    fn delete_by_id_returns_not_found_when_absent() {
        let store = fresh_store();
        let err = store.delete_by_id(99999).unwrap_err();
        assert!(matches!(err, StorageError::NotFound));
    }

    #[test]
    fn delete_by_surface_reading_removes_row() {
        let store = fresh_store();
        let r = UserVocabRecord {
            id: None,
            surface: "日野岡".to_string(),
            reading: "ひのおか".to_string(),
            pos: "名詞".to_string(),
            score: 0.0,
            created_at: 0,
            updated_at: 0,
        };
        store.insert(r).expect("insert");
        store
            .delete_by_surface_reading("日野岡", "ひのおか")
            .expect("delete ok");
        let result = store.find_by_reading("ひのおか", 10).expect("find ok");
        assert!(result.is_empty());
    }

    #[test]
    fn delete_by_surface_reading_returns_not_found_when_absent() {
        let store = fresh_store();
        let err = store.delete_by_surface_reading("ない", "ない").unwrap_err();
        assert!(matches!(err, StorageError::NotFound));
    }

    #[test]
    fn delete_by_surface_reading_validates_reading() {
        let store = fresh_store();
        let err = store.delete_by_surface_reading("x", "abc").unwrap_err();
        assert!(matches!(err, StorageError::InvalidField { .. }));
    }

    /// sec-M5 review T-C1: 行数上限到達時に `QuotaExceeded` を返すことを検証する。
    /// production 値 50,000 は test では非現実的なので `QuotaOverrideGuard` で
    /// 上限を 5 に縮める。
    #[test]
    fn sqlite_insert_rejects_at_quota_cap() {
        let _guard = QuotaOverrideGuard::new(5);
        let store = fresh_store();
        // 上限ぴったり 5 件まで insert は成功する。
        for i in 0..5 {
            let reading = char::from_u32(0x3042 + i)
                .expect("valid hiragana code point")
                .to_string();
            let r = UserVocabRecord {
                id: None,
                surface: format!("filler-{i}"),
                reading,
                pos: "名詞".to_string(),
                score: 0.0,
                created_at: 0,
                updated_at: 0,
            };
            store.insert(r).expect("under-cap insert ok");
        }
        // 6 件目は QuotaExceeded で reject される。
        let over = UserVocabRecord {
            id: None,
            surface: "over-cap".to_string(),
            reading: "き".to_string(),
            pos: "名詞".to_string(),
            score: 0.0,
            created_at: 0,
            updated_at: 0,
        };
        let err = store.insert(over).unwrap_err();
        assert!(matches!(
            err,
            StorageError::QuotaExceeded { ref table, max } if table == "user_vocab" && max == 5
        ));
    }
}
