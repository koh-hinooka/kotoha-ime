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
#[derive(Debug)]
pub struct ZenzBackend {
    model_path: PathBuf,
    _placeholder: (),
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
