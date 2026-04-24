//! Zenz GGUF model backend (via llama-cpp-2).
//!
//! **P1-1 status: SKELETON ONLY.** The real `load` / `convert` implementation
//! ships in milestone P1-2, together with the `llama-cpp-2` dependency being
//! added to `Cargo.toml` and wired to the `zenz` feature (which is currently
//! an empty flag, not `["dep:llama-cpp-2"]`).
//!
//! Spec: `docs/superpowers/specs/2026-04-24-kotoha-phase-1-design.md` §3.1, §5.3.

use std::path::{Path, PathBuf};

use crate::kanji::{Candidate, ConvertOptions, KanjiBackend, KanjiError};

/// Zenz model backend. Enabled by `feature = "zenz"`.
///
/// **P1-1: all methods panic via `todo!()`. P1-2 supplies the real
/// implementation backed by llama-cpp-2.**
#[allow(dead_code)]
pub struct ZenzBackend {
    model_path: PathBuf,
    _placeholder: (),
}

impl ZenzBackend {
    /// Loads a Zenz GGUF model from `model_path`.
    ///
    /// # P1-1
    ///
    /// Panics with `todo!()` unconditionally.
    ///
    /// # P1-2 (planned)
    ///
    /// Opens the GGUF file via llama-cpp-2, validates the architecture, and
    /// caches the resulting context for subsequent `convert` calls.
    ///
    /// # Errors
    ///
    /// After P1-2 lands:
    ///
    /// - [`KanjiError::ModelNotFound`] if `model_path` does not exist.
    /// - [`KanjiError::ModelLoadFailed`] if llama-cpp-2 rejects the file.
    #[allow(unused_variables)]
    pub fn load(model_path: &Path) -> Result<Self, KanjiError> {
        todo!("P1-2: implement via llama-cpp-2")
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
