//! Layer 4 end-to-end: DictionaryBackend with UserVocab integration (spec §10.4)。

#![cfg(feature = "dict-persist")]

use kotoha_core::dict::{
    DictionaryBackend, EngineCandidate, MorphologicalEngine, UserVocab, VocabEntry,
    VocabularyLookup,
};
use kotoha_core::kanji::{ConvertOptions, KanjiBackend, KanjiError};
use kotoha_storage::user_vocab::mock::MockUserVocabStore;
use kotoha_storage::user_vocab::store::{UserVocabRecord, UserVocabStore};

struct StubEngine {
    canned: Vec<EngineCandidate>,
}

impl MorphologicalEngine for StubEngine {
    fn tokenize(&self, reading: &str) -> Result<Vec<EngineCandidate>, KanjiError> {
        if reading.is_empty() {
            return Ok(Vec::new());
        }
        Ok(self.canned.clone())
    }
    fn engine_id(&self) -> &str {
        "stub-engine"
    }
}

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
fn user_vocab_entry_appears_in_convert_result() {
    let mock = Box::new(MockUserVocabStore::new());
    seed(&mock, "日野岡", "ひのおか", 0.9);
    let uv: Box<dyn VocabularyLookup> = Box::new(UserVocab::new(mock));
    let engine = Box::new(StubEngine { canned: Vec::new() });
    let backend = DictionaryBackend::from_parts(engine, vec![uv]);
    let opts = ConvertOptions::default();
    let result = backend.convert("ひのおか", &opts).expect("ok");
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].surface, "日野岡");
}

#[test]
fn user_vocab_score_tie_with_custom_vocab_keeps_custom_first() {
    struct CustomStub;
    impl VocabularyLookup for CustomStub {
        fn lookup(&self, _r: &str) -> Vec<VocabEntry> {
            vec![VocabEntry {
                surface: "ひの岡(custom)".to_string(),
                reading: "ひのおか".to_string(),
                pos: "名詞".to_string(),
                score: 1.0,
            }]
        }
        fn vocab_id(&self) -> &str {
            "custom-stub"
        }
    }
    let mock = Box::new(MockUserVocabStore::new());
    seed(&mock, "ひの岡(user)", "ひのおか", 1.0);
    let custom: Box<dyn VocabularyLookup> = Box::new(CustomStub);
    let uv: Box<dyn VocabularyLookup> = Box::new(UserVocab::new(mock));
    let engine = Box::new(StubEngine { canned: Vec::new() });
    let backend = DictionaryBackend::from_parts(engine, vec![custom, uv]);
    let opts = ConvertOptions::default();
    let result = backend.convert("ひのおか", &opts).expect("ok");
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].surface, "ひの岡(custom)");
}

#[test]
fn user_vocab_with_higher_score_overrides_custom() {
    struct CustomStub;
    impl VocabularyLookup for CustomStub {
        fn lookup(&self, _r: &str) -> Vec<VocabEntry> {
            vec![VocabEntry {
                surface: "ひの岡(custom)".to_string(),
                reading: "ひのおか".to_string(),
                pos: "名詞".to_string(),
                score: 1.0,
            }]
        }
        fn vocab_id(&self) -> &str {
            "custom-stub"
        }
    }
    let mock = Box::new(MockUserVocabStore::new());
    seed(&mock, "ひの岡(user-high)", "ひのおか", 5.0);
    let custom: Box<dyn VocabularyLookup> = Box::new(CustomStub);
    let uv: Box<dyn VocabularyLookup> = Box::new(UserVocab::new(mock));
    let engine = Box::new(StubEngine { canned: Vec::new() });
    let backend = DictionaryBackend::from_parts(engine, vec![custom, uv]);
    let opts = ConvertOptions::default();
    let result = backend.convert("ひのおか", &opts).expect("ok");
    assert_eq!(result[0].surface, "ひの岡(user-high)");
}

#[test]
fn dict_persist_disabled_path_skips_user_vocab() {
    let mock = Box::new(MockUserVocabStore::new());
    let uv: Box<dyn VocabularyLookup> = Box::new(UserVocab::new(mock));
    let engine = Box::new(StubEngine { canned: Vec::new() });
    let backend = DictionaryBackend::from_parts(engine, vec![uv]);
    let opts = ConvertOptions::default();
    let result = backend.convert("ひのおか", &opts).expect("ok");
    assert!(result.is_empty());
}
