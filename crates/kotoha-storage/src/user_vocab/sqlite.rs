//! SqliteUserVocabStore: UserVocabStore の SQLite 実装(spec §6.1 / §6.4)。

use std::sync::Arc;

use crate::database::Database;
use crate::error::StorageError;
use crate::user_vocab::store::{UserVocabRecord, UserVocabStore};
use crate::validation::{validate_pos, validate_reading, validate_score, validate_surface};

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

    fn delete_by_id(&self, _id: i64) -> Result<(), StorageError> {
        unimplemented!("Task B5")
    }

    fn delete_by_surface_reading(
        &self,
        _surface: &str,
        _reading: &str,
    ) -> Result<(), StorageError> {
        unimplemented!("Task B6")
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
}
