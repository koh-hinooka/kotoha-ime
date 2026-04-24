//! Backend abstraction and shared helpers for the kanji subsystem.
//!
//! Spec: `docs/superpowers/specs/2026-04-24-kotoha-phase-1-design.md` §5.3, §5.6, §5.7.
//!
//! The [`KanjiBackend`] trait defines the central abstraction for pluggable
//! conversion backends (Mock / Zenz / future). The `pub(crate)` helpers
//! [`validate_input`] and [`score_sort_dedupe`] live here so that every
//! backend implementation enforces the same input contract and output
//! guarantees without reimplementing the logic.

use std::path::PathBuf;

use crate::kanji::{Candidate, ConvertOptions, KanjiError};

/// Backend construction parameters.
///
/// Spec: `docs/superpowers/specs/2026-04-24-kotoha-phase-1-design.md` §5.4.
///
/// Marked `#[non_exhaustive]` per ADR 0006 so Phase 2+ can add new backend
/// kinds (system dictionary / remote HTTP / learning-cache hybrid) without
/// breaking external match sites.
#[non_exhaustive]
#[derive(Debug, Clone)]
pub enum BackendConfig {
    /// Deterministic mock backend (for tests). Constructible only when the
    /// `mock-backend` feature flag is enabled at build time.
    Mock,

    /// Zenz GGUF model backend (via llama-cpp-2). Constructible only when
    /// the `zenz` feature flag is enabled at build time.
    Zenz {
        /// Absolute path to the GGUF file on disk.
        model_path: PathBuf,
    },
}

/// Abstraction for a kana-to-kanji conversion backend.
///
/// Implementations are not required to be `Send + Sync` in Phase 1
/// (the CLI is single-threaded). Phase 3 may revisit this when wiring
/// the backend into an IBus engine running on a separate thread.
///
/// # Contract (spec §5.3)
///
/// Implementations MUST:
///
/// - Reject non-hiragana input (hiragana block `U+3040..=U+309F` plus the
///   prolonged-sound mark `ー U+30FC` is the only accepted alphabet) by
///   returning [`KanjiError::InvalidInput`]. Use the shared
///   [`validate_input`] helper to ensure a uniform error shape.
/// - Reject input longer than 128 characters (counted by
///   `str::chars().count()`) with [`KanjiError::InvalidInput`].
/// - Return candidates sorted in descending `score` order.
/// - Return candidates with unique `surface` (a surface that the model
///   produces with multiple scores collapses to the one with the highest
///   score).
/// - Return at most `options.top_k` candidates.
/// - Treat `options.top_k == 0` as a request for an empty `Vec`, not an
///   error.
///
/// Use [`score_sort_dedupe`] to enforce the last four properties in a single
/// call on the raw model output.
///
/// # Errors
///
/// - [`KanjiError::InvalidInput`] when input violates §5.6.
/// - [`KanjiError::Backend`] when inference fails inside the backend.
/// - [`KanjiError::ModelLoadFailed`] / [`KanjiError::ModelNotFound`] —
///   typically surfaced only from the constructor, not from `convert`.
pub trait KanjiBackend {
    /// Returns a human-readable identifier for the active model.
    ///
    /// Examples: `"mock"`, `"zenz-v2.5-medium"`. Used for logging and
    /// CLI diagnostics.
    fn model_id(&self) -> &str;

    /// Converts a hiragana string into top-`options.top_k` kanji candidates.
    ///
    /// # Contract
    ///
    /// See the trait-level `# Contract` section.
    ///
    /// # Errors
    ///
    /// See the trait-level `# Errors` section.
    fn convert(&self, input: &str, options: &ConvertOptions) -> Result<Vec<Candidate>, KanjiError>;
}

/// Validates that `input` satisfies the backend input contract (spec §5.6).
///
/// # Accepted characters
///
/// - Hiragana block: `U+3040..=U+309F`
/// - Prolonged sound mark: `U+30FC` (ー)
///
/// Empty input is accepted (the backend returns an empty `Vec`).
///
/// # Errors
///
/// - [`KanjiError::InvalidInput`] if `input` contains any character outside
///   the accepted ranges or if `input.chars().count() > 128`.
// Consumed by `MockBackend` in P1-1-6 and by `ZenzBackend` in Phase B.
#[allow(dead_code)]
pub(crate) fn validate_input(input: &str) -> Result<(), KanjiError> {
    let count = input.chars().count();
    if count > 128 {
        return Err(KanjiError::InvalidInput {
            reason: format!("input length {count} exceeds the 128-character limit"),
        });
    }
    for ch in input.chars() {
        let ok = matches!(ch, '\u{3040}'..='\u{309F}' | '\u{30FC}');
        if !ok {
            return Err(KanjiError::InvalidInput {
                reason: format!(
                    "character {ch:?} is outside the hiragana block (U+3040..=U+309F) and is not the prolonged sound mark (U+30FC)"
                ),
            });
        }
    }
    Ok(())
}

/// Enforces the output ordering and deduplication contract from spec §5.7.
///
/// Steps:
///
/// 1. Sort `candidates` in descending `score` order (stable, NaN treated as equal).
/// 2. Deduplicate by `surface`, keeping the first occurrence (which is the
///    highest-score entry thanks to step 1).
/// 3. Truncate to `top_k` entries. If `top_k == 0`, the returned `Vec` is empty.
///
/// # Postconditions
///
/// - `result.len() <= top_k`.
/// - For every adjacent pair `(result[i], result[i+1])`, `result[i].score >= result[i+1].score`.
/// - No two entries in `result` share the same `surface`.
// Consumed by `MockBackend` in P1-1-6 and by `ZenzBackend` in Phase B.
#[allow(dead_code)]
pub(crate) fn score_sort_dedupe(mut candidates: Vec<Candidate>, top_k: usize) -> Vec<Candidate> {
    if top_k == 0 {
        return Vec::new();
    }
    candidates.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let mut seen: Vec<String> = Vec::with_capacity(candidates.len());
    let mut out: Vec<Candidate> = Vec::with_capacity(top_k.min(candidates.len()));
    for c in candidates {
        if seen.iter().any(|s| s == &c.surface) {
            continue;
        }
        seen.push(c.surface.clone());
        out.push(c);
        if out.len() >= top_k {
            break;
        }
    }
    out
}

/// Constructs the backend described by `config`.
///
/// Spec: `docs/superpowers/specs/2026-04-24-kotoha-phase-1-design.md` §5.4.
///
/// # Errors
///
/// - [`KanjiError::FeatureDisabled`] if `config` names a backend whose Cargo
///   feature was not enabled at build time.
/// - Errors bubbled up from the backend's loader
///   (for example [`KanjiError::ModelNotFound`] from `ZenzBackend::load`
///   once P1-2 lands).
#[allow(unused_variables)]
pub fn load_backend(config: &BackendConfig) -> Result<Box<dyn KanjiBackend>, KanjiError> {
    match config {
        #[cfg(feature = "mock-backend")]
        BackendConfig::Mock => Ok(Box::new(crate::kanji::MockBackend::new())),

        #[cfg(not(feature = "mock-backend"))]
        BackendConfig::Mock => Err(KanjiError::FeatureDisabled {
            feature: "mock-backend",
        }),

        #[cfg(feature = "zenz")]
        BackendConfig::Zenz { model_path } => {
            Ok(Box::new(crate::kanji::ZenzBackend::load(model_path)?))
        }

        #[cfg(not(feature = "zenz"))]
        BackendConfig::Zenz { .. } => Err(KanjiError::FeatureDisabled { feature: "zenz" }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ======================================================================
    // validate_input
    // ======================================================================

    #[test]
    fn validate_input_accepts_empty() {
        assert!(validate_input("").is_ok());
    }

    #[test]
    fn validate_input_accepts_hiragana() {
        assert!(validate_input("にほんご").is_ok());
    }

    #[test]
    fn validate_input_accepts_choonpu() {
        assert!(validate_input("ばー").is_ok());
    }

    #[test]
    fn validate_input_rejects_latin() {
        let err = validate_input("abc").expect_err("latin input must be rejected");
        assert!(matches!(err, KanjiError::InvalidInput { .. }));
    }

    #[test]
    fn validate_input_rejects_kanji() {
        let err = validate_input("日本").expect_err("kanji input must be rejected");
        assert!(matches!(err, KanjiError::InvalidInput { .. }));
    }

    #[test]
    fn validate_input_rejects_katakana() {
        let err = validate_input("カタカナ").expect_err("katakana input must be rejected");
        assert!(matches!(err, KanjiError::InvalidInput { .. }));
    }

    #[test]
    fn validate_input_rejects_space() {
        let err = validate_input(" ").expect_err("space input must be rejected");
        assert!(matches!(err, KanjiError::InvalidInput { .. }));
    }

    #[test]
    fn validate_input_rejects_digit() {
        let err = validate_input("1").expect_err("digit input must be rejected");
        assert!(matches!(err, KanjiError::InvalidInput { .. }));
    }

    #[test]
    fn validate_input_rejects_over_128_chars() {
        let s = "あ".repeat(129);
        let err = validate_input(&s).expect_err("input of 129 chars must be rejected");
        assert!(matches!(err, KanjiError::InvalidInput { .. }));
    }

    #[test]
    fn validate_input_accepts_exactly_128_chars() {
        let s = "あ".repeat(128);
        assert!(validate_input(&s).is_ok());
    }

    // ======================================================================
    // score_sort_dedupe
    // ======================================================================

    #[test]
    fn score_sort_dedupe_empty_input_returns_empty() {
        let out = score_sort_dedupe(Vec::new(), 5);
        assert!(out.is_empty());
    }

    #[test]
    fn score_sort_dedupe_top_k_zero_returns_empty() {
        let input = vec![Candidate::new("日本語", 0.9)];
        let out = score_sort_dedupe(input, 0);
        assert!(out.is_empty());
    }

    #[test]
    fn score_sort_dedupe_sorts_descending_by_score() {
        let input = vec![
            Candidate::new("二本後", 0.3),
            Candidate::new("日本語", 0.9),
            Candidate::new("日本後", 0.5),
        ];
        let out = score_sort_dedupe(input, 5);
        assert_eq!(out.len(), 3);
        assert_eq!(out[0].surface, "日本語");
        assert_eq!(out[1].surface, "日本後");
        assert_eq!(out[2].surface, "二本後");
    }

    #[test]
    fn score_sort_dedupe_dedupes_surface_keeping_highest_score() {
        let input = vec![
            Candidate::new("日本語", 0.3),
            Candidate::new("日本語", 0.9),
            Candidate::new("二本後", 0.5),
        ];
        let out = score_sort_dedupe(input, 5);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].surface, "日本語");
        assert!((out[0].score - 0.9).abs() < 1e-6);
        assert_eq!(out[1].surface, "二本後");
    }

    #[test]
    fn score_sort_dedupe_truncates_to_top_k() {
        let input = vec![
            Candidate::new("a", 0.9),
            Candidate::new("b", 0.8),
            Candidate::new("c", 0.7),
            Candidate::new("d", 0.6),
        ];
        let out = score_sort_dedupe(input, 2);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].surface, "a");
        assert_eq!(out[1].surface, "b");
    }

    #[test]
    fn score_sort_dedupe_handles_single_candidate() {
        let input = vec![Candidate::new("日本語", 0.9)];
        let out = score_sort_dedupe(input, 5);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].surface, "日本語");
    }

    #[test]
    fn score_sort_dedupe_stable_for_equal_scores() {
        // Two candidates with identical score must retain input order.
        let input = vec![Candidate::new("first", 0.5), Candidate::new("second", 0.5)];
        let out = score_sort_dedupe(input, 5);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].surface, "first");
        assert_eq!(out[1].surface, "second");
    }

    // ======================================================================
    // BackendConfig
    // ======================================================================

    #[test]
    fn backend_config_is_clone() {
        let mock = BackendConfig::Mock;
        let _ = mock.clone();
        let zenz = BackendConfig::Zenz {
            model_path: PathBuf::from("/tmp/zenz.gguf"),
        };
        let _ = zenz.clone();
    }

    #[test]
    fn backend_config_debug_contains_variant_name() {
        let mock = BackendConfig::Mock;
        let msg = format!("{mock:?}");
        assert!(
            msg.contains("Mock"),
            "Debug for Mock must contain \"Mock\": {msg}"
        );

        let zenz = BackendConfig::Zenz {
            model_path: PathBuf::from("/tmp/zenz.gguf"),
        };
        let msg = format!("{zenz:?}");
        assert!(
            msg.contains("Zenz"),
            "Debug for Zenz must contain \"Zenz\": {msg}"
        );
    }

    // ======================================================================
    // load_backend
    // ======================================================================

    #[cfg(not(feature = "mock-backend"))]
    #[test]
    fn mock_config_without_feature_errors_feature_disabled() {
        match load_backend(&BackendConfig::Mock) {
            Err(KanjiError::FeatureDisabled { feature }) => {
                assert_eq!(feature, "mock-backend");
            }
            Err(other) => panic!("unexpected error: {other:?}"),
            Ok(_) => panic!("Mock must error when mock-backend feature is off"),
        }
    }

    #[cfg(not(feature = "zenz"))]
    #[test]
    fn zenz_config_without_feature_errors_feature_disabled() {
        match load_backend(&BackendConfig::Zenz {
            model_path: PathBuf::from("/tmp/zenz.gguf"),
        }) {
            Err(KanjiError::FeatureDisabled { feature }) => {
                assert_eq!(feature, "zenz");
            }
            Err(other) => panic!("unexpected error: {other:?}"),
            Ok(_) => panic!("Zenz must error when zenz feature is off"),
        }
    }

    #[cfg(feature = "mock-backend")]
    #[test]
    fn mock_config_with_feature_returns_box() {
        let backend = load_backend(&BackendConfig::Mock)
            .expect("Mock must construct when mock-backend feature is on");
        assert_eq!(backend.model_id(), "mock");
    }
}
