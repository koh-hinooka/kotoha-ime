//! `LearningCacheStore` trait skeleton(spec §6.3、本実装は P2-C)。

pub mod sqlite;

pub use sqlite::SqliteLearningCacheStore;

use crate::error::StorageError;

/// Learning cache store の抽象境界(P2-C で本格使用)。
pub trait LearningCacheStore: Send + Sync {
    fn lookup(
        &self,
        kana_input: &str,
        limit: usize,
    ) -> Result<Vec<LearningCacheRecord>, StorageError>;
    fn record_choice(&self, kana_input: &str, chosen_kanji: &str) -> Result<(), StorageError>;
    fn evict_lru(&self, max_entries: usize) -> Result<usize, StorageError>;
}

#[derive(Debug, Clone, PartialEq)]
pub struct LearningCacheRecord {
    pub id: i64,
    pub kana_input: String,
    pub chosen_kanji: String,
    pub frequency: u32,
    pub last_used_at: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn learning_cache_record_holds_all_fields() {
        let r = LearningCacheRecord {
            id: 1,
            kana_input: "あい".to_string(),
            chosen_kanji: "愛".to_string(),
            frequency: 5,
            last_used_at: 1745529600,
        };
        assert_eq!(r.id, 1);
        assert_eq!(r.frequency, 5);
    }
}
