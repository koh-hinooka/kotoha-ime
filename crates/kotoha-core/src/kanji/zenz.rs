//! Zenz GGUF model backend (via llama-cpp-2).
//!
//! # Status (P1-2)
//!
//! This module holds the real kanji conversion backend. An instance owns a
//! llama-cpp-2 `LlamaModel` loaded from a GGUF file, re-used across every
//! `convert` call.
//!
//! # Prompt format
//!
//! Zenz was trained on katakana input per the upstream dataset contract
//! (`Miwa-Keita/zenz-v2.5-dataset` README: `"input": 入力のカタカナ文字列`).
//! `ZenzBackend::convert` therefore applies `crate::kana::hiragana_to_katakana`
//! to the caller-supplied hiragana before building the prompt; Kotoha's
//! public input contract (spec §5.6) stays hiragana-only so the preprocessing
//! is an internal concern of this backend.
//!
//! The wider prompt template (context / input / output separators, EOS token
//! `</s>`, the PUA separator tokens `U+EE00..=U+EE06`) follows the AzooKey
//! Zenzai reference implementation and is documented in the WBS
//! "prompt format 解析ログ" section.
//!
//! # Deterministic output
//!
//! When `ConvertOptions::temperature == 0.0` and `ConvertOptions::seed == Some(0)`
//! (the default), the backend performs greedy decoding with a fixed seed so
//! the Layer 3 smoke tests and the E2E smoke tests are reproducible.
//!
//! Spec: `docs/superpowers/specs/2026-04-24-kotoha-phase-1-design.md` §3.1,
//! §3.2, §3.3, §5.3, §6.

use std::fmt;
use std::path::{Path, PathBuf};

use llama_cpp_2::model::LlamaModel;

use crate::kanji::{Candidate, ConvertOptions, KanjiBackend, KanjiError};

/// Zenz GGUF model backend, backed by llama-cpp-2. Enabled by `feature = "zenz"`.
///
/// Construct with [`ZenzBackend::load`] or via
/// [`crate::kanji::load_backend`] applied to [`crate::kanji::BackendConfig::Zenz`].
///
/// # Invariants
///
/// - `model` holds an initialized llama-cpp-2 `LlamaModel`.
/// - `model_path` is the absolute path that was used to load `model`.
/// - The backend is single-threaded; wrap externally for concurrent use.
///
/// `#[allow(dead_code)]` is a temporary marker: Task P1-2-5 replaces
/// `ZenzBackend::load`'s current Err stub with a real constructor that
/// populates `model`, at which point the attribute is removed.
#[allow(dead_code)]
pub struct ZenzBackend {
    /// Loaded llama-cpp-2 model. Kept private so llama-cpp-2 types do not leak
    /// into the public API surface.
    model: LlamaModel,
    /// Path the model was loaded from. Used by logging and error diagnostics.
    model_path: PathBuf,
}

// Manual `Debug` impl: `LlamaModel` does not implement `Debug` (FFI-wrapping
// type backed by `NonNull<llama_model>` with no derived impl in upstream
// version 0.1.145), so `#[derive(Debug)]` on `ZenzBackend` would not compile.
// The test `zenz_load_returns_backend_error_in_p1_1_skeleton` uses
// `Result::expect_err`, which requires `T: Debug` on the `Ok` variant even
// though the `Ok` branch is never taken in the P1-1 skeleton. Print only
// `model_path` and replace the `model` field with a placeholder string.
impl fmt::Debug for ZenzBackend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ZenzBackend")
            .field("model", &"<LlamaModel>")
            .field("model_path", &self.model_path)
            .finish()
    }
}

impl ZenzBackend {
    /// Loads a Zenz GGUF model from `model_path`.
    ///
    /// # P1-1 status
    ///
    /// Returns [`KanjiError::Backend`] with a "not yet implemented (P1-2)"
    /// reason. This is a safe error — not a panic — so that callers going
    /// through [`crate::kanji::load_backend`] with the `zenz` feature enabled
    /// receive a typed error instead of a process abort.
    ///
    /// # P1-2 (planned)
    ///
    /// Opens the GGUF file via llama-cpp-2, validates the architecture, and
    /// caches the resulting context for subsequent `convert` calls.
    ///
    /// # Errors
    ///
    /// - P1-1: [`KanjiError::Backend`] unconditionally.
    /// - P1-2 (planned): [`KanjiError::ModelNotFound`] if `model_path` does
    ///   not exist; [`KanjiError::ModelLoadFailed`] if llama-cpp-2 rejects
    ///   the file.
    #[allow(unused_variables)]
    pub fn load(model_path: &Path) -> Result<Self, KanjiError> {
        Err(KanjiError::Backend {
            reason: "ZenzBackend is a P1-1 skeleton; real implementation lands in Phase 1 milestone P1-2".to_string(),
        })
    }
}

impl KanjiBackend for ZenzBackend {
    fn model_id(&self) -> &str {
        todo!("P1-2: implement via llama-cpp-2")
    }

    #[allow(unused_variables)]
    fn convert(&self, input: &str, options: &ConvertOptions) -> Result<Vec<Candidate>, KanjiError> {
        todo!("P1-2: implement via llama-cpp-2")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zenz_load_returns_backend_error_in_p1_1_skeleton() {
        let err = ZenzBackend::load(Path::new("/tmp/nonexistent.gguf"))
            .expect_err("ZenzBackend::load must error in P1-1 skeleton, not panic");
        match err {
            KanjiError::Backend { reason } => {
                assert!(
                    reason.contains("P1-2"),
                    "reason should point to P1-2 implementation: {reason}"
                );
            }
            other => panic!("unexpected error variant: {other:?}"),
        }
    }
}
