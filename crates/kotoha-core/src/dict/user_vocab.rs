//! `UserVocab`: `VocabularyLookup` の SQLite 永続化版実装(spec §6.7、P2-B)。

use kotoha_storage::user_vocab::store::UserVocabStore;

use crate::dict::vocab::{VocabEntry, VocabularyLookup};

/// User-managed vocabulary、`Box<dyn UserVocabStore>` に依存(DIP、spec §4.2)。
pub struct UserVocab {
    store: Box<dyn UserVocabStore>,
    vocab_id: String,
}

impl UserVocab {
    /// `Box<dyn UserVocabStore>` を inject して構築する。
    pub fn new(store: Box<dyn UserVocabStore>) -> Self {
        Self {
            store,
            vocab_id: "user-vocab".to_string(),
        }
    }
}

impl VocabularyLookup for UserVocab {
    fn lookup(&self, reading: &str) -> Vec<VocabEntry> {
        // SQLite backend エラー / validation エラー時は empty Vec を返し panic しない
        // (spec §6.7、`unwrap_or_default()` 採用根拠)
        self.store
            .find_by_reading(reading, 32)
            .unwrap_or_default()
            .into_iter()
            .map(|r| VocabEntry {
                surface: r.surface,
                reading: r.reading,
                pos: r.pos,
                score: r.score,
            })
            .collect()
    }

    fn vocab_id(&self) -> &str {
        &self.vocab_id
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kotoha_storage::user_vocab::mock::MockUserVocabStore;
    use kotoha_storage::user_vocab::store::UserVocabRecord;

    fn seed_mock(store: &MockUserVocabStore, surface: &str, reading: &str, score: f32) {
        store
            .insert(UserVocabRecord {
                id: None,
                surface: surface.to_string(),
                reading: reading.to_string(),
                pos: "名詞".to_string(),
                score,
                created_at: 0,
                updated_at: 0,
            })
            .unwrap();
    }

    #[test]
    fn user_vocab_lookup_returns_seeded_entry() {
        let mock = Box::new(MockUserVocabStore::new());
        seed_mock(&mock, "日野岡", "ひのおか", 1.0);
        let uv = UserVocab::new(mock);
        let result = uv.lookup("ひのおか");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].surface, "日野岡");
    }

    #[test]
    fn user_vocab_lookup_empty_for_unknown() {
        let mock = Box::new(MockUserVocabStore::new());
        let uv = UserVocab::new(mock);
        let result = uv.lookup("みず");
        assert!(result.is_empty());
    }

    #[test]
    fn user_vocab_lookup_invalid_reading_returns_empty_not_panic() {
        let mock = Box::new(MockUserVocabStore::new());
        let uv = UserVocab::new(mock);
        // 非 hiragana を渡すと validate_reading で error → unwrap_or_default で empty
        let result = uv.lookup("カタカナ");
        assert!(result.is_empty());
    }

    #[test]
    fn user_vocab_lookup_orders_by_score() {
        let mock = Box::new(MockUserVocabStore::new());
        seed_mock(&mock, "a", "あ", 0.3);
        seed_mock(&mock, "b", "あ", 0.9);
        let uv = UserVocab::new(mock);
        let result = uv.lookup("あ");
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].surface, "b");
        assert_eq!(result[1].surface, "a");
    }

    #[test]
    fn user_vocab_id_is_stable() {
        let mock = Box::new(MockUserVocabStore::new());
        let uv = UserVocab::new(mock);
        assert_eq!(uv.vocab_id(), "user-vocab");
    }
}
