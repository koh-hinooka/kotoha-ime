//! SqliteLearningCacheStore: P2-C で本実装(spec §3.1 / §4.2-§4.5 / §6.3)。

use std::sync::Arc;

use crate::database::Database;
use crate::error::StorageError;
use crate::learning_cache::{LearningCacheReader, LearningCacheRecord, LearningCacheWriter};

pub struct SqliteLearningCacheStore {
    #[allow(dead_code)] // B3 / B5 / B10 で本実装と同時に使用開始
    pub(crate) db: Arc<Database>,
}

impl SqliteLearningCacheStore {
    pub fn new(db: Arc<Database>) -> Self {
        Self { db }
    }
}

/// `SqliteLearningCacheStore` の B1 段階の動作:
///
/// - `lookup`: 常に空 `Vec` を返す(B3 で本実装)
/// - `record_choice`: 常に Ok(())(B5 で本実装)
/// - `evict_lru`: 常に Ok(0)(B10 で本実装)
///
/// table `learning_cache` schema は v001 migration で同梱済(spec §5.2)。
impl LearningCacheReader for SqliteLearningCacheStore {
    fn lookup(
        &self,
        _kana_input: &str,
        _limit: usize,
    ) -> Result<Vec<LearningCacheRecord>, StorageError> {
        // B3 で本実装する。
        Ok(Vec::new())
    }
}

impl LearningCacheWriter for SqliteLearningCacheStore {
    fn record_choice(&self, _kana_input: &str, _chosen_kanji: &str) -> Result<(), StorageError> {
        // B5 で本実装する。
        Ok(())
    }

    fn evict_lru(&self, _max_entries: usize) -> Result<usize, StorageError> {
        // B10 で本実装する。
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

    // --- lookup tests (B2 red phase) ---

    fn fresh_store_b() -> SqliteLearningCacheStore {
        let db = Database::open_in_memory().expect("memory open");
        SqliteLearningCacheStore::new(db)
    }

    /// `lookup` は存在しない kana_input に対して空 Vec を返す。
    /// (B2) TDD red: stub は Ok(Vec::new()) を返すので本 test は PASS するが、
    /// B3 実装後も引き続き PASS することを確認する。
    #[test]
    fn lookup_returns_empty_for_unknown_kana() {
        let store = fresh_store_b();
        let result = store.lookup("みず", 10).expect("ok");
        assert!(result.is_empty());
    }

    /// `lookup` は record_choice で記録した entry を返す。
    /// (B2) TDD red: stub は常に空 Vec を返すので本 test は FAIL する。
    #[test]
    fn lookup_returns_recorded_entry() {
        let store = fresh_store_b();
        store.record_choice("あい", "愛").expect("record ok");
        let result = store.lookup("あい", 10).expect("ok");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].chosen_kanji, "愛");
        assert_eq!(result[0].frequency, 1);
    }

    /// `lookup` は frequency DESC 順で複数 entry を返す。
    /// (B2) TDD red: stub は常に空 Vec を返すので本 test は FAIL する。
    #[test]
    fn lookup_orders_by_frequency_desc() {
        let store = fresh_store_b();
        // "愛" を 2 回、"哀" を 1 回記録する。
        store.record_choice("あい", "愛").expect("record ok");
        store.record_choice("あい", "愛").expect("record ok");
        store.record_choice("あい", "哀").expect("record ok");
        let result = store.lookup("あい", 10).expect("ok");
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].chosen_kanji, "愛"); // frequency=2
        assert_eq!(result[1].chosen_kanji, "哀"); // frequency=1
    }

    /// `lookup` は limit を超えた件数を返さない。
    /// (B2) TDD red: stub は常に空 Vec を返すので本 test は FAIL する。
    #[test]
    fn lookup_respects_limit() {
        let store = fresh_store_b();
        store.record_choice("あい", "愛").expect("ok");
        store.record_choice("あい", "哀").expect("ok");
        store.record_choice("あい", "藍").expect("ok");
        let result = store.lookup("あい", 2).expect("ok");
        assert!(result.len() <= 2);
    }
}
