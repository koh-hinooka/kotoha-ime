//! MockUserVocabStore(spec §6.6、in-memory、test 用)。

use std::sync::Mutex;

use crate::error::StorageError;
use crate::user_vocab::store::{UserVocabRecord, UserVocabStore};
use crate::validation::{validate_pos, validate_reading, validate_score, validate_surface};

pub struct MockUserVocabStore {
    records: Mutex<Vec<UserVocabRecord>>,
    next_id: Mutex<i64>,
}

impl Default for MockUserVocabStore {
    fn default() -> Self {
        Self::new()
    }
}

impl MockUserVocabStore {
    pub fn new() -> Self {
        Self {
            records: Mutex::new(Vec::new()),
            next_id: Mutex::new(1),
        }
    }
}

impl UserVocabStore for MockUserVocabStore {
    fn find_by_reading(
        &self,
        reading: &str,
        limit: usize,
    ) -> Result<Vec<UserVocabRecord>, StorageError> {
        validate_reading(reading)?;
        let records = self.records.lock().unwrap();
        let mut filtered: Vec<UserVocabRecord> = records
            .iter()
            .filter(|r| r.reading == reading)
            .cloned()
            .collect();
        filtered.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        filtered.truncate(limit);
        Ok(filtered)
    }

    fn list_all(&self, limit: usize, offset: usize) -> Result<Vec<UserVocabRecord>, StorageError> {
        let records = self.records.lock().unwrap();
        Ok(records.iter().skip(offset).take(limit).cloned().collect())
    }

    fn find_by_prefix(
        &self,
        reading_prefix: &str,
        limit: usize,
    ) -> Result<Vec<UserVocabRecord>, StorageError> {
        if !reading_prefix.is_empty() {
            validate_reading(reading_prefix)?;
        }
        let records = self.records.lock().unwrap();
        let mut filtered: Vec<UserVocabRecord> = records
            .iter()
            .filter(|r| r.reading.starts_with(reading_prefix))
            .cloned()
            .collect();
        filtered.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        filtered.truncate(limit);
        Ok(filtered)
    }

    fn insert(&self, mut record: UserVocabRecord) -> Result<i64, StorageError> {
        validate_surface(&record.surface)?;
        validate_reading(&record.reading)?;
        validate_pos(&record.pos)?;
        validate_score(record.score)?;

        let mut records = self.records.lock().unwrap();
        if records
            .iter()
            .any(|r| r.surface == record.surface && r.reading == record.reading)
        {
            return Err(StorageError::DuplicateEntry {
                surface: record.surface,
                reading: record.reading,
            });
        }
        let mut next_id = self.next_id.lock().unwrap();
        let id = *next_id;
        *next_id += 1;
        record.id = Some(id);
        records.push(record);
        Ok(id)
    }

    fn delete_by_id(&self, id: i64) -> Result<(), StorageError> {
        let mut records = self.records.lock().unwrap();
        let before = records.len();
        records.retain(|r| r.id != Some(id));
        if records.len() == before {
            return Err(StorageError::NotFound);
        }
        Ok(())
    }

    fn delete_by_surface_reading(&self, surface: &str, reading: &str) -> Result<(), StorageError> {
        validate_surface(surface)?;
        validate_reading(reading)?;
        let mut records = self.records.lock().unwrap();
        let before = records.len();
        records.retain(|r| !(r.surface == surface && r.reading == reading));
        if records.len() == before {
            return Err(StorageError::NotFound);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rec(surface: &str, reading: &str, score: f32) -> UserVocabRecord {
        UserVocabRecord {
            id: None,
            surface: surface.to_string(),
            reading: reading.to_string(),
            pos: "名詞".to_string(),
            score,
            created_at: 0,
            updated_at: 0,
        }
    }

    #[test]
    fn mock_insert_then_find() {
        let store = MockUserVocabStore::new();
        let id = store.insert(rec("日野岡", "ひのおか", 1.0)).expect("ok");
        assert!(id >= 1);
        let result = store.find_by_reading("ひのおか", 10).expect("ok");
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn mock_duplicate_rejected() {
        let store = MockUserVocabStore::new();
        store.insert(rec("a", "あ", 0.0)).unwrap();
        let err = store.insert(rec("a", "あ", 0.0)).unwrap_err();
        assert!(matches!(err, StorageError::DuplicateEntry { .. }));
    }

    #[test]
    fn mock_find_orders_by_score_desc() {
        let store = MockUserVocabStore::new();
        store.insert(rec("a", "あ", 0.5)).unwrap();
        store.insert(rec("b", "あ", 1.5)).unwrap();
        let result = store.find_by_reading("あ", 10).unwrap();
        assert_eq!(result[0].surface, "b");
        assert_eq!(result[1].surface, "a");
    }

    #[test]
    fn mock_delete_by_id_works() {
        let store = MockUserVocabStore::new();
        let id = store.insert(rec("a", "あ", 0.0)).unwrap();
        store.delete_by_id(id).unwrap();
        assert!(store.find_by_reading("あ", 10).unwrap().is_empty());
    }

    #[test]
    fn mock_delete_by_id_not_found() {
        let store = MockUserVocabStore::new();
        let err = store.delete_by_id(999).unwrap_err();
        assert!(matches!(err, StorageError::NotFound));
    }

    #[test]
    fn mock_delete_by_surface_reading_works() {
        let store = MockUserVocabStore::new();
        store.insert(rec("a", "あ", 0.0)).unwrap();
        store.delete_by_surface_reading("a", "あ").unwrap();
        assert!(store.find_by_reading("あ", 10).unwrap().is_empty());
    }

    #[test]
    fn mock_list_all_pagination() {
        let store = MockUserVocabStore::new();
        let readings = ["あ", "い", "う", "え", "お"];
        let surfaces = ["s0", "s1", "s2", "s3", "s4"];
        for i in 0..5 {
            store.insert(rec(surfaces[i], readings[i], 0.0)).unwrap();
        }
        let result = store.list_all(2, 1).unwrap();
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn mock_find_by_prefix_works() {
        let store = MockUserVocabStore::new();
        store.insert(rec("日野岡", "ひのおか", 1.0)).unwrap();
        store.insert(rec("日野", "ひの", 0.5)).unwrap();
        store.insert(rec("別人", "べつじん", 0.5)).unwrap();
        let result = store.find_by_prefix("ひの", 100).unwrap();
        assert_eq!(result.len(), 2);
    }
}
