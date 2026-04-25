//! `VocabularyLookup` trait と `VocabEntry` 値型の定義。
//!
//! Spec: `docs/superpowers/specs/2026-04-25-p2-a-dictionary-layer-design.md` §3.6, §4.2.2.

/// User dictionary / Custom vocabulary の lookup 抽象境界。
///
/// `DictionaryBackend` は `Vec<Box<dyn VocabularyLookup>>` を field に保持する。
/// P2-A は `CustomVocab` のみが本 trait を実装する。P2-B で `UserVocab` が
/// 同 trait を実装する extension path を確保する(spec §3.6 / §4.2.2)。
///
/// # Preconditions
///
/// - `reading` は hiragana 文字列(`KanjiBackend` 契約と同じ)
///
/// # Postconditions
///
/// - 同一 reading に対し 0 件以上の `VocabEntry` を返す
/// - 返値順序は score 降順(score 同値時の順序は実装依存)
pub trait VocabularyLookup {
    /// Returns all vocab entries whose reading matches `reading`.
    fn lookup(&self, reading: &str) -> Vec<VocabEntry>;

    /// Returns a stable, human-readable identifier for the vocab source.
    fn vocab_id(&self) -> &str;
}

/// A single vocabulary entry returned by [`VocabularyLookup::lookup`].
#[derive(Debug, Clone)]
pub struct VocabEntry {
    /// Surface form.
    pub surface: String,
    /// Reading (hiragana).
    pub reading: String,
    /// Part of speech (SudachiDict と同 schema、`"名詞"` / `"固有名詞"` 等)。
    pub pos: String,
    /// Score; larger is better.
    pub score: f32,
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockVocab {
        canned: Vec<VocabEntry>,
    }

    impl VocabularyLookup for MockVocab {
        fn lookup(&self, reading: &str) -> Vec<VocabEntry> {
            if reading.is_empty() {
                return Vec::new();
            }
            self.canned.clone()
        }

        fn vocab_id(&self) -> &str {
            "mock-vocab"
        }
    }

    #[test]
    fn vocab_entry_holds_surface_reading_pos_score() {
        let ve = VocabEntry {
            surface: "漢字".to_string(),
            reading: "かんじ".to_string(),
            pos: "名詞".to_string(),
            score: 0.85,
        };
        assert_eq!(ve.surface, "漢字");
        assert_eq!(ve.reading, "かんじ");
        assert_eq!(ve.pos, "名詞");
        assert!((ve.score - 0.85).abs() < 1e-6);
    }

    #[test]
    fn mock_vocab_returns_canned_entries() {
        let vocab = MockVocab {
            canned: vec![VocabEntry {
                surface: "漢字".to_string(),
                reading: "かんじ".to_string(),
                pos: "名詞".to_string(),
                score: 0.85,
            }],
        };
        let result = vocab.lookup("かんじ");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].surface, "漢字");
    }

    #[test]
    fn mock_vocab_empty_input_returns_empty() {
        let vocab = MockVocab { canned: Vec::new() };
        assert!(vocab.lookup("").is_empty());
    }

    #[test]
    fn mock_vocab_vocab_id_is_stable() {
        let vocab = MockVocab { canned: Vec::new() };
        assert_eq!(vocab.vocab_id(), "mock-vocab");
    }
}
