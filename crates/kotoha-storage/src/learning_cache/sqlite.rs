//! SqliteLearningCacheStore: P2-B では unimplemented stub、P2-C で実装(spec §6.3)。

use std::sync::Arc;

use crate::database::Database;
use crate::error::StorageError;
use crate::learning_cache::{LearningCacheRecord, LearningCacheStore};

pub struct SqliteLearningCacheStore {
    #[allow(dead_code)] // P2-C で使用開始
    pub(crate) db: Arc<Database>,
}

impl SqliteLearningCacheStore {
    pub fn new(db: Arc<Database>) -> Self {
        Self { db }
    }
}

/// `SqliteLearningCacheStore` の P2-B 動作:
///
/// - `lookup`: 常に空 `Vec` を返す(LearningCache の lookup 実装は P2-C)
/// - `record_choice`: 常に Ok(())(record 実装は P2-C)
/// - `evict_lru`: 常に Ok(0)(eviction 実装は P2-C)
///
/// table `learning_cache` schema は v001 migration で同梱済(spec §5.2)。
impl LearningCacheStore for SqliteLearningCacheStore {
    fn lookup(
        &self,
        _kana_input: &str,
        _limit: usize,
    ) -> Result<Vec<LearningCacheRecord>, StorageError> {
        // P2-C で実装
        Ok(Vec::new())
    }

    fn record_choice(&self, _kana_input: &str, _chosen_kanji: &str) -> Result<(), StorageError> {
        // P2-C で実装
        Ok(())
    }

    fn evict_lru(&self, _max_entries: usize) -> Result<usize, StorageError> {
        // P2-C で実装
        Ok(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn p2b_stub_lookup_returns_empty() {
        let db = Database::open_in_memory().expect("memory open");
        let store = SqliteLearningCacheStore::new(db);
        let result = store.lookup("あい", 10).expect("ok");
        assert!(result.is_empty());
    }

    #[test]
    fn p2b_stub_record_choice_noop() {
        let db = Database::open_in_memory().expect("memory open");
        let store = SqliteLearningCacheStore::new(db);
        store.record_choice("あい", "愛").expect("noop ok");
    }

    #[test]
    fn p2b_stub_evict_lru_returns_zero() {
        let db = Database::open_in_memory().expect("memory open");
        let store = SqliteLearningCacheStore::new(db);
        let evicted = store.evict_lru(100).expect("noop ok");
        assert_eq!(evicted, 0);
    }
}
