//! Layer 2 integration tests for UserVocab (P2-B、spec §10.2)。

#![cfg(feature = "dict-persist")]

use kotoha_core::dict::{UserVocab, VocabularyLookup};
use kotoha_storage::user_vocab::mock::MockUserVocabStore;
use kotoha_storage::user_vocab::store::{UserVocabRecord, UserVocabWriter};

fn seed(mock: &MockUserVocabStore, surface: &str, reading: &str, score: f32) {
    mock.insert(UserVocabRecord {
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
fn user_vocab_lookup_via_mock_store() {
    let mock = Box::new(MockUserVocabStore::new());
    seed(&mock, "日野岡", "ひのおか", 1.0);
    let uv = UserVocab::new(mock);
    let result = uv.lookup("ひのおか");
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].surface, "日野岡");
}

#[test]
fn user_vocab_lookup_empty_when_store_empty() {
    let mock = Box::new(MockUserVocabStore::new());
    let uv = UserVocab::new(mock);
    assert!(uv.lookup("ひのおか").is_empty());
}

#[test]
fn user_vocab_lookup_score_descending() {
    let mock = Box::new(MockUserVocabStore::new());
    seed(&mock, "a", "あ", 0.3);
    seed(&mock, "b", "あ", 0.9);
    seed(&mock, "c", "あ", 0.6);
    let uv = UserVocab::new(mock);
    let result = uv.lookup("あ");
    assert_eq!(result.len(), 3);
    assert_eq!(result[0].surface, "b");
    assert_eq!(result[1].surface, "c");
    assert_eq!(result[2].surface, "a");
}

#[test]
fn user_vocab_lookup_invalid_reading_does_not_panic() {
    let mock = Box::new(MockUserVocabStore::new());
    let uv = UserVocab::new(mock);
    assert!(uv.lookup("ABC").is_empty());
    assert!(uv.lookup("カタカナ").is_empty());
}

#[test]
fn user_vocab_id_is_user_vocab() {
    let mock = Box::new(MockUserVocabStore::new());
    let uv = UserVocab::new(mock);
    assert_eq!(uv.vocab_id(), "user-vocab");
}

#[test]
fn user_vocab_returns_vocab_entries_with_pos() {
    let mock = Box::new(MockUserVocabStore::new());
    seed(&mock, "日野岡", "ひのおか", 1.0);
    let uv = UserVocab::new(mock);
    let result = uv.lookup("ひのおか");
    assert_eq!(result[0].pos, "名詞");
}
