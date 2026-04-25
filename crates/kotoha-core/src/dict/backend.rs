//! `DictionaryBackend` struct (P2-A、`KanjiBackend` 実装)。
//!
//! Spec: `docs/superpowers/specs/2026-04-25-p2-a-dictionary-layer-design.md` §4.3.

use crate::dict::custom_vocab::CustomVocab;
use crate::dict::engine::MorphologicalEngine;
use crate::dict::sudachi_adapter::SudachiAdapter;
use crate::dict::vocab::VocabularyLookup;
use crate::dict::{resolve_dict_path, DictionaryConfig};
use crate::kanji::backend::{score_sort_dedupe, validate_input};
use crate::kanji::{Candidate, ConvertOptions, KanjiBackend, KanjiError};

/// Dictionary-based kanji conversion backend.
///
/// `KanjiBackend` の P2-A 実装。`MorphologicalEngine` を 1 本、
/// `VocabularyLookup` を 0..N 本注入し、`convert` で両者の結果を merge した後、
/// 既存の `score_sort_dedupe` で sort + dedupe + truncate する。
///
/// # Invariants
///
/// - `engine` は `load` または `from_parts` で 1 本注入される
/// - `vocab_sources` は 0 本以上の `VocabularyLookup` 実装を保持する
/// - `model_id` は `"dictionary({engine_id})"` 形式で固定する
pub struct DictionaryBackend {
    engine: Box<dyn MorphologicalEngine>,
    vocab_sources: Vec<Box<dyn VocabularyLookup>>,
    model_id: String,
}

impl std::fmt::Debug for DictionaryBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DictionaryBackend")
            .field("engine_id", &self.engine.engine_id())
            .field("vocab_count", &self.vocab_sources.len())
            .field("model_id", &self.model_id)
            .finish()
    }
}

impl DictionaryBackend {
    /// Loads a `DictionaryBackend` from a `DictionaryConfig`.
    ///
    /// `system_dict_path` resolution order:
    ///
    /// 1. `config.system_dict_path` if `Some`
    /// 2. `KOTOHA_SYSTEM_DICT_PATH` env var if set
    /// 3. otherwise return [`KanjiError::Backend`] with an actionable hint
    ///
    /// # Errors
    ///
    /// - [`KanjiError::Backend`] — neither explicit path nor env var supplies
    ///   a SudachiDict path; the error reason names both knobs the caller can
    ///   set
    /// - [`KanjiError::ModelNotFound`] — the resolved system dict file does
    ///   not exist on disk
    /// - [`KanjiError::ModelLoadFailed`] — sudachi.rs fails to load the dict
    /// - [`KanjiError::Backend`] — custom vocab TSV cannot be read or parsed
    /// - [`KanjiError::Backend`] — `dict-persist` feature 有効時、user vocab DB
    ///   が open できない(spec §8.2)
    pub fn load(config: &DictionaryConfig) -> Result<Self, KanjiError> {
        let env_value = std::env::var("KOTOHA_SYSTEM_DICT_PATH").ok();
        let Some(dict_path) = resolve_dict_path(config.system_dict_path.as_deref(), env_value)
        else {
            return Err(KanjiError::Backend {
                reason: "no SudachiDict path: set KOTOHA_SYSTEM_DICT_PATH environment variable, \
                    or pass DictionaryConfig::system_dict_path"
                    .to_string(),
            });
        };
        let engine = Box::new(SudachiAdapter::load(&dict_path)?);
        let mut vocab_sources: Vec<Box<dyn VocabularyLookup>> = Vec::new();
        if let Some(vp) = &config.custom_vocab_path {
            vocab_sources.push(Box::new(CustomVocab::load(vp)?));
        }
        #[cfg(feature = "dict-persist")]
        {
            if let Some(db_path) = &config.user_vocab_db_path {
                let db = kotoha_storage::database::Database::open(db_path).map_err(|e| {
                    KanjiError::Backend {
                        reason: format!(
                            "failed to open user_vocab DB at {}: {e}",
                            db_path.display()
                        ),
                    }
                })?;
                let store = db.user_vocab_store();
                vocab_sources.push(Box::new(crate::dict::user_vocab::UserVocab::new(store)));
            }
        }
        let model_id = format!("dictionary({})", engine.engine_id());
        Ok(Self {
            engine,
            vocab_sources,
            model_id,
        })
    }

    /// Constructs a `DictionaryBackend` from already-built parts.
    ///
    /// Test-only injection point: tests pass `StubEngine` / `StubVocab` (or
    /// the in-tree `MockEngine` / `MockVocab`) without going through disk I/O.
    /// `#[doc(hidden)]` keeps the constructor out of the rendered rustdoc but
    /// leaves it `pub` for cross-module test composition.
    #[doc(hidden)]
    pub fn from_parts(
        engine: Box<dyn MorphologicalEngine>,
        vocab_sources: Vec<Box<dyn VocabularyLookup>>,
    ) -> Self {
        let model_id = format!("dictionary({})", engine.engine_id());
        Self {
            engine,
            vocab_sources,
            model_id,
        }
    }
}

impl KanjiBackend for DictionaryBackend {
    fn model_id(&self) -> &str {
        &self.model_id
    }

    fn convert(&self, input: &str, options: &ConvertOptions) -> Result<Vec<Candidate>, KanjiError> {
        validate_input(input)?;
        if input.is_empty() || options.top_k == 0 {
            return Ok(Vec::new());
        }
        let mut raw: Vec<Candidate> = Vec::new();
        for ec in self.engine.tokenize(input)? {
            raw.push(Candidate::new(ec.surface, ec.score));
        }
        for vocab in &self.vocab_sources {
            for ve in vocab.lookup(input) {
                raw.push(Candidate::new(ve.surface, ve.score));
            }
        }
        Ok(score_sort_dedupe(raw, options.top_k))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dict::engine::{EngineCandidate, MorphologicalEngine};
    use crate::dict::vocab::{VocabEntry, VocabularyLookup};
    use crate::kanji::{ConvertOptions, KanjiBackend, KanjiError};

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

    struct StubVocab {
        canned: Vec<VocabEntry>,
    }

    impl VocabularyLookup for StubVocab {
        fn lookup(&self, reading: &str) -> Vec<VocabEntry> {
            if reading.is_empty() {
                Vec::new()
            } else {
                self.canned.clone()
            }
        }
        fn vocab_id(&self) -> &str {
            "stub-vocab"
        }
    }

    fn backend_with(
        engine_cands: Vec<EngineCandidate>,
        vocab_cands: Vec<VocabEntry>,
    ) -> DictionaryBackend {
        DictionaryBackend::from_parts(
            Box::new(StubEngine {
                canned: engine_cands,
            }),
            vec![Box::new(StubVocab {
                canned: vocab_cands,
            })],
        )
    }

    #[test]
    fn dict_backend_rejects_non_hiragana_input() {
        let b = backend_with(Vec::new(), Vec::new());
        let err = b
            .convert("abc", &ConvertOptions::default())
            .expect_err("latin reject");
        assert!(matches!(err, KanjiError::InvalidInput { .. }));
    }

    #[test]
    fn dict_backend_empty_input_returns_empty_vec() {
        let b = backend_with(Vec::new(), Vec::new());
        let out = b.convert("", &ConvertOptions::default()).expect("empty ok");
        assert!(out.is_empty());
    }

    #[test]
    fn dict_backend_merges_engine_and_vocab_results() {
        let b = backend_with(
            vec![EngineCandidate {
                surface: "漢字".into(),
                reading: "かんじ".into(),
                score: 0.5,
            }],
            vec![VocabEntry {
                surface: "感じ".into(),
                reading: "かんじ".into(),
                pos: "動詞".into(),
                score: 0.4,
            }],
        );
        let out = b
            .convert("かんじ", &ConvertOptions::default())
            .expect("merge ok");
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].surface, "漢字");
        assert_eq!(out[1].surface, "感じ");
    }

    #[test]
    fn dict_backend_dedupes_duplicate_surfaces_keeping_highest_score() {
        let b = backend_with(
            vec![EngineCandidate {
                surface: "漢字".into(),
                reading: "かんじ".into(),
                score: 0.3,
            }],
            vec![VocabEntry {
                surface: "漢字".into(),
                reading: "かんじ".into(),
                pos: "名詞".into(),
                score: 0.9,
            }],
        );
        let out = b
            .convert("かんじ", &ConvertOptions::default())
            .expect("dedupe ok");
        assert_eq!(out.len(), 1);
        assert!((out[0].score - 0.9).abs() < 1e-6);
    }

    #[test]
    fn dict_backend_truncates_to_top_k() {
        let engine_cands = (0..10)
            .map(|i| EngineCandidate {
                surface: format!("s{i}"),
                reading: "かんじ".into(),
                score: i as f32 / 10.0,
            })
            .collect();
        let b = backend_with(engine_cands, Vec::new());
        let opts = ConvertOptions {
            top_k: 3,
            temperature: 0.0,
            seed: Some(0),
        };
        let out = b.convert("かんじ", &opts).expect("truncate ok");
        assert_eq!(out.len(), 3);
    }

    #[test]
    fn dict_backend_top_k_zero_returns_empty() {
        let b = backend_with(
            vec![EngineCandidate {
                surface: "漢字".into(),
                reading: "かんじ".into(),
                score: 0.9,
            }],
            Vec::new(),
        );
        let opts = ConvertOptions {
            top_k: 0,
            temperature: 0.0,
            seed: Some(0),
        };
        let out = b.convert("かんじ", &opts).expect("empty ok");
        assert!(out.is_empty());
    }

    #[test]
    fn dict_backend_model_id_contains_engine_id() {
        let b = backend_with(Vec::new(), Vec::new());
        let id = b.model_id();
        assert!(
            id.contains("dictionary"),
            "model_id should start with 'dictionary': {id}"
        );
        assert!(
            id.contains("stub-engine"),
            "model_id should embed engine_id: {id}"
        );
    }
}
