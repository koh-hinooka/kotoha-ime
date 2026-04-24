//! `MorphologicalEngine` trait と `EngineCandidate` 値型の定義。
//!
//! Spec: `docs/superpowers/specs/2026-04-25-p2-a-dictionary-layer-design.md` §3.5, §4.2.1.

use crate::kanji::KanjiError;

/// 形態素解析 engine の抽象境界。
///
/// `DictionaryBackend` は `Box<dyn MorphologicalEngine>` を field に保持する。
/// P2-A は `SudachiAdapter` のみが本 trait を実装する。Phase 5 以降で vibrato /
/// lindera 等への置換時に `DictionaryBackend` の変更を最小化する目的で先出し
/// する(spec §3.5 / §4.2.1)。
///
/// # Preconditions
///
/// - `reading` は `KanjiBackend` 契約 §5.6 と同じ hiragana 文字列
///   (U+3040..=U+309F + U+30FC)
/// - `reading.chars().count() <= 128`
///
/// # Postconditions
///
/// - 返値は `reading` を tokenize した候補列
/// - 同一 reading に対し複数 surface があり得る(homophone)
/// - `score` は engine 実装が付与(`SudachiAdapter` は cost を score に変換)
///
/// # Errors
///
/// - [`KanjiError::Backend`] when tokenization fails inside the engine
#[allow(dead_code)] // Consumed by `DictionaryBackend` in Task 8 (P2-A).
pub trait MorphologicalEngine {
    /// Tokenizes `reading` and returns candidate morphemes.
    fn tokenize(&self, reading: &str) -> Result<Vec<EngineCandidate>, KanjiError>;

    /// Returns a stable, human-readable identifier for the engine instance.
    ///
    /// Used by `DictionaryBackend::model_id()` to format the composite model id
    /// (e.g. `"dictionary(sudachi-0.6.11)"`).
    fn engine_id(&self) -> &str;
}

/// A single morpheme candidate returned by [`MorphologicalEngine::tokenize`].
#[allow(dead_code)] // Consumed by `DictionaryBackend` in Task 8 (P2-A).
#[derive(Debug, Clone)]
pub struct EngineCandidate {
    /// Surface form (kanji / hiragana / katakana mix).
    pub surface: String,
    /// Reading (hiragana) of `surface`.
    pub reading: String,
    /// Score; larger is better. `SudachiAdapter` converts Sudachi cost into
    /// a descending-order score.
    pub score: f32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kanji::KanjiError;

    /// tests 内専用の MockEngine。trait contract を external に lock-in する。
    struct MockEngine {
        canned: Vec<EngineCandidate>,
    }

    impl MorphologicalEngine for MockEngine {
        fn tokenize(&self, reading: &str) -> Result<Vec<EngineCandidate>, KanjiError> {
            if reading.is_empty() {
                return Ok(Vec::new());
            }
            Ok(self.canned.clone())
        }

        fn engine_id(&self) -> &str {
            "mock-engine"
        }
    }

    #[test]
    fn engine_candidate_new_holds_surface_reading_score() {
        let ec = EngineCandidate {
            surface: "日本語".to_string(),
            reading: "にほんご".to_string(),
            score: 0.9,
        };
        assert_eq!(ec.surface, "日本語");
        assert_eq!(ec.reading, "にほんご");
        assert!((ec.score - 0.9).abs() < 1e-6);
    }

    #[test]
    fn mock_engine_returns_canned_candidates() {
        let engine = MockEngine {
            canned: vec![EngineCandidate {
                surface: "日本語".to_string(),
                reading: "にほんご".to_string(),
                score: 0.9,
            }],
        };
        let result = engine.tokenize("にほんご").expect("tokenize succeeds");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].surface, "日本語");
    }

    #[test]
    fn mock_engine_empty_input_returns_empty() {
        let engine = MockEngine { canned: Vec::new() };
        let result = engine.tokenize("").expect("empty input must be accepted");
        assert!(result.is_empty());
    }

    #[test]
    fn mock_engine_engine_id_is_stable() {
        let engine = MockEngine { canned: Vec::new() };
        assert_eq!(engine.engine_id(), "mock-engine");
    }
}
