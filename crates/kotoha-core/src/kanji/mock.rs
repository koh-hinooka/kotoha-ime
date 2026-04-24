//! Deterministic mock backend for tests.
//!
//! Enabled by the `mock-backend` Cargo feature so that downstream integration
//! tests (which live in a separate crate via `tests/`) can import it. A plain
//! `#[cfg(test)]` would not expose the type across crates.
//!
//! # Fixture decision (spec §13 Q4 follow-up)
//!
//! The fixture is hard-coded (not externalized to TSV). Spec §13 Q4 accepts
//! this for small fixture counts. Four known inputs (`"にほんご"`,
//! `"かんじ"`, `"あした"`, `"にほん"`) produce candidates sufficient to
//! exercise every contract property (score-descending sort, surface dedupe,
//! top_k truncation, empty result, unknown-input empty result) in Layer 2
//! integration tests. The `"にほん"` entry emits three raw candidates with
//! one duplicate surface ("日本" at 0.9 and 0.3), so the dedupe path of
//! `score_sort_dedupe` is actually exercised end-to-end.
//!
//! Any input not listed above returns an empty `Vec` rather than an error,
//! matching the trait contract (spec §5.3, §5.7 bullet 4 "empty allowed").

use crate::kanji::backend::{score_sort_dedupe, validate_input};
use crate::kanji::{Candidate, ConvertOptions, KanjiBackend, KanjiError};

/// Deterministic mock backend. Enabled by `feature = "mock-backend"`.
///
/// Construct with [`MockBackend::new`] or through
/// [`crate::kanji::load_backend`] applied to [`crate::kanji::BackendConfig::Mock`].
#[derive(Debug, Default)]
pub struct MockBackend {
    // Zero-sized placeholder; fields may be added in Phase 2+.
    _private: (),
}

impl MockBackend {
    /// Constructs a new [`MockBackend`].
    ///
    /// # Postconditions
    ///
    /// - The returned backend's `model_id()` equals `"mock"`.
    pub fn new() -> Self {
        Self { _private: () }
    }
}

impl KanjiBackend for MockBackend {
    fn model_id(&self) -> &str {
        "mock"
    }

    fn convert(&self, input: &str, options: &ConvertOptions) -> Result<Vec<Candidate>, KanjiError> {
        validate_input(input)?;
        let fixture: Vec<Candidate> = match input {
            "にほんご" => vec![Candidate::new("日本語", 0.9), Candidate::new("二本後", 0.3)],
            "かんじ" => vec![Candidate::new("漢字", 0.85), Candidate::new("感じ", 0.45)],
            "あした" => vec![Candidate::new("明日", 0.92), Candidate::new("足した", 0.25)],
            // Dedupe fixture: two candidates share the same surface "日本" but
            // with different scores; after score_sort_dedupe, only the higher
            // (0.9) is retained. This is the one input that exercises the
            // surface-dedupe property end-to-end in Layer 2.
            "にほん" => vec![
                Candidate::new("日本", 0.9),
                Candidate::new("日本", 0.3),
                Candidate::new("二本", 0.5),
            ],
            _ => Vec::new(),
        };
        Ok(score_sort_dedupe(fixture, options.top_k))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mock_model_id_is_mock() {
        let b = MockBackend::new();
        assert_eq!(b.model_id(), "mock");
    }

    #[test]
    fn mock_known_input_returns_expected() {
        let b = MockBackend::new();
        let opts = ConvertOptions::default();
        let out = b.convert("にほんご", &opts).expect("convert must succeed");
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].surface, "日本語");
        assert!((out[0].score - 0.9).abs() < 1e-6);
        assert_eq!(out[1].surface, "二本後");
    }

    #[test]
    fn mock_unknown_input_returns_empty() {
        let b = MockBackend::new();
        let opts = ConvertOptions::default();
        let out = b
            .convert("あいうえお", &opts)
            .expect("convert must succeed");
        assert!(out.is_empty());
    }

    #[test]
    fn mock_respects_top_k_1() {
        let b = MockBackend::new();
        let opts = ConvertOptions {
            top_k: 1,
            temperature: 0.0,
            seed: Some(0),
        };
        let out = b.convert("にほんご", &opts).expect("convert must succeed");
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].surface, "日本語");
    }

    #[test]
    fn mock_invalid_input_returns_invalid_input_error() {
        let b = MockBackend::new();
        let opts = ConvertOptions::default();
        let err = b
            .convert("abc", &opts)
            .expect_err("latin input must be rejected");
        assert!(matches!(err, KanjiError::InvalidInput { .. }));
    }

    #[test]
    fn mock_dedupe_fixture_collapses_duplicate_surfaces() {
        let backend = MockBackend::new();
        let options = ConvertOptions {
            top_k: 5,
            ..ConvertOptions::default()
        };
        let result = backend.convert("にほん", &options).expect("valid input");

        assert_eq!(result.len(), 2, "3 raw candidates dedupe to 2");
        assert_eq!(result[0].surface, "日本");
        assert!((result[0].score - 0.9).abs() < 1e-6);
        assert_eq!(result[1].surface, "二本");
        assert!((result[1].score - 0.5).abs() < 1e-6);
    }
}
