//! Error type for the kanji subsystem.
//!
//! Spec: `docs/superpowers/specs/2026-04-24-kotoha-phase-1-design.md` §5.5.
//!
//! All error messages are in English per the project convention for backend-facing
//! diagnostics.

use std::path::PathBuf;

use thiserror::Error;

/// Errors returned from the kanji subsystem.
///
/// Marked `#[non_exhaustive]` per ADR 0006 so that Phase 2+ can add variants
/// (for example cache / learning failures) without breaking external match
/// sites.
#[non_exhaustive]
#[derive(Debug, Error)]
pub enum KanjiError {
    /// The caller supplied input that violates the contract (hiragana-only, max 128 chars).
    #[error("input violates contract: {reason}")]
    InvalidInput {
        /// Human-readable reason for the rejection.
        reason: String,
    },

    /// The requested model file does not exist at the given path.
    #[error("model file not found: {}", path.display())]
    ModelNotFound {
        /// Absolute path that was probed.
        path: PathBuf,
    },

    /// Loading the model file succeeded at the filesystem level but failed at
    /// parse / initialization time (wrapped by the underlying backend library).
    #[error("model load failed: {source}")]
    ModelLoadFailed {
        /// Underlying error reported by the backend library.
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },

    /// Inference failed inside the backend.
    #[error("backend inference failed: {reason}")]
    Backend {
        /// Human-readable reason reported by the backend.
        reason: String,
    },

    /// The caller asked for a backend whose feature flag is not enabled at build time.
    #[error("required feature not enabled at build time: {feature}")]
    FeatureDisabled {
        /// Name of the Cargo feature that must be enabled.
        feature: &'static str,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_input_display_contains_reason() {
        let err = KanjiError::InvalidInput {
            reason: "non-hiragana character".to_string(),
        };
        let msg = format!("{err}");
        assert!(
            msg.contains("non-hiragana character"),
            "display should include the reason: {msg}"
        );
    }

    #[test]
    fn model_not_found_display_contains_path() {
        let err = KanjiError::ModelNotFound {
            path: PathBuf::from("/tmp/gemma-2-2b-jpn-it.gguf"),
        };
        let msg = format!("{err}");
        assert!(
            msg.contains("/tmp/gemma-2-2b-jpn-it.gguf"),
            "display should include the path: {msg}"
        );
    }

    #[test]
    fn feature_disabled_display_contains_feature_name() {
        let err = KanjiError::FeatureDisabled {
            feature: "llama-cpp",
        };
        let msg = format!("{err}");
        assert!(
            msg.contains("llama-cpp"),
            "display should include the feature name: {msg}"
        );
    }

    #[test]
    fn backend_display_contains_reason() {
        let err = KanjiError::Backend {
            reason: "token decode failed".to_string(),
        };
        let msg = format!("{err}");
        assert!(
            msg.contains("token decode failed"),
            "display should include the reason: {msg}"
        );
    }

    #[test]
    fn kanji_error_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<KanjiError>();
    }
}
