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
use std::sync::OnceLock;

use llama_cpp_2::llama_backend::LlamaBackend;
use llama_cpp_2::model::params::LlamaModelParams;
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
pub struct ZenzBackend {
    /// Loaded llama-cpp-2 model. Kept private so llama-cpp-2 types do not leak
    /// into the public API surface.
    ///
    /// `#[allow(dead_code)]` is a temporary marker while `convert` still
    /// carries `todo!()`; the subsequent P1-2 tasks (tokenizer wrapper +
    /// inference loop) read `model` and the attribute will then be removed.
    #[allow(dead_code)]
    model: LlamaModel,
    /// Path the model was loaded from. Used by logging and error diagnostics.
    model_path: PathBuf,
}

// Manual `Debug` impl: `LlamaModel` does not implement `Debug` (FFI-wrapping
// type backed by `NonNull<llama_model>` with no derived impl in upstream
// version 0.1.145), so `#[derive(Debug)]` on `ZenzBackend` would not compile.
// The tests use `Result::expect_err`, which requires `T: Debug` on the `Ok`
// variant. Print only `model_path` and replace the `model` field with a
// placeholder string.
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
    /// # Preconditions
    ///
    /// - `model_path` points to an existing file. Otherwise
    ///   [`KanjiError::ModelNotFound`] is returned without touching
    ///   llama-cpp-2.
    ///
    /// # Postconditions
    ///
    /// On success, the returned `ZenzBackend` owns a llama-cpp-2
    /// [`LlamaModel`] initialized from the GGUF file at `model_path`, and
    /// the process-global [`LlamaBackend`] is guaranteed to be initialized.
    ///
    /// # Errors
    ///
    /// - [`KanjiError::ModelNotFound`] if the file does not exist.
    /// - [`KanjiError::ModelLoadFailed`] if llama-cpp-2 fails to initialize
    ///   the global backend or parse / initialize the GGUF (wrapping the
    ///   underlying error as `source`).
    pub fn load(model_path: &Path) -> Result<Self, KanjiError> {
        if !model_path.exists() {
            return Err(KanjiError::ModelNotFound {
                path: model_path.to_path_buf(),
            });
        }

        let model = load_llama_model(model_path)
            .map_err(|source| KanjiError::ModelLoadFailed { source })?;

        Ok(Self {
            model,
            model_path: model_path.to_path_buf(),
        })
    }
}

/// Returns the process-global [`LlamaBackend`], initializing it on the first
/// call.
///
/// # Invariants
///
/// - `llama_cpp_sys_2::llama_backend_init` is invoked at most once per
///   process; `LlamaBackend::init()` itself enforces that via an internal
///   `AtomicBool` and returns
///   [`llama_cpp_2::LlamaCppError::BackendAlreadyInitialized`] on a second
///   call.
/// - A successful first init is cached in the `OnceLock`; subsequent callers
///   receive the shared `&LlamaBackend` reference and do not pay FFI cost.
/// - A failed first init is *not* cached — the caller receives the error
///   directly from `LlamaBackend::init()` so a retry can observe a newly
///   resolved cause (for example after loading a missing shared library).
fn llama_backend() -> Result<&'static LlamaBackend, llama_cpp_2::LlamaCppError> {
    static BACKEND: OnceLock<LlamaBackend> = OnceLock::new();
    if let Some(existing) = BACKEND.get() {
        return Ok(existing);
    }
    let backend = LlamaBackend::init()?;
    // `set` returns `Err(backend)` only if another thread won the race; in
    // that case we discard our instance and return the cached one. The
    // second `LlamaBackend` value from `init()` is safe to drop because it
    // is a zero-sized type and `llama_backend_init` is guarded by the
    // crate-internal `AtomicBool`.
    Ok(BACKEND.get_or_init(|| backend))
}

/// Initializes the global [`LlamaBackend`] (once per process) and loads the
/// given GGUF file.
///
/// Errors are type-erased into `Box<dyn Error + Send + Sync>` so that
/// [`KanjiError::ModelLoadFailed`] can wrap either a backend-init failure or
/// a model-parse failure under a single `source` without leaking llama-cpp-2
/// types into the public API surface.
fn load_llama_model(
    model_path: &Path,
) -> Result<LlamaModel, Box<dyn std::error::Error + Send + Sync>> {
    let backend = llama_backend()?;
    let params = LlamaModelParams::default();
    let model = LlamaModel::load_from_file(backend, model_path, &params)?;
    Ok(model)
}

impl KanjiBackend for ZenzBackend {
    fn model_id(&self) -> &str {
        // NOTE: llama-cpp-2 0.1.145 does not expose a clean public API for
        // reading GGUF `general.name` metadata from `LlamaModel`; the underlying
        // `llama_model_meta_val_str` FFI is not re-exported in this version.
        // For Phase 1 we return the spec §3.2 default Zenz-v2.5-medium literal.
        // Phase 2 can revisit once llama-cpp-2 surfaces a Rust-side metadata
        // accessor or we reach for the `-sys` crate directly.
        "zenz-v2.5-medium"
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
    fn zenz_load_errors_on_missing_file() {
        let err = ZenzBackend::load(Path::new("/tmp/definitely-does-not-exist-kotoha-p1-2.gguf"))
            .expect_err("load must error when the path does not exist");
        match err {
            KanjiError::ModelNotFound { path } => {
                assert!(
                    path.to_string_lossy().contains("definitely-does-not-exist"),
                    "ModelNotFound path should echo the input: {}",
                    path.display()
                );
            }
            other => panic!("expected ModelNotFound, got: {other:?}"),
        }
    }

    // Note: `zenz_model_id_is_zenz_prefix` is intentionally absent from the
    // in-source test module because constructing a `ZenzBackend` for the test
    // requires a real GGUF file, which is out of scope here. Layer 3 smoke
    // (`tests/kanji_zenz_smoke.rs`) asserts that `model_id()` returns a
    // "zenz"-prefixed string once a real backend is loaded via
    // `KOTOHA_ZENZ_MODEL_PATH`.
}
