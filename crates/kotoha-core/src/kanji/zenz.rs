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

use llama_cpp_2::context::params::LlamaContextParams;
use llama_cpp_2::llama_backend::LlamaBackend;
use llama_cpp_2::llama_batch::LlamaBatch;
use llama_cpp_2::model::params::LlamaModelParams;
use llama_cpp_2::model::{AddBos, LlamaModel};
use llama_cpp_2::sampling::LlamaSampler;

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
    /// into the public API surface. Read by `convert` to spin up a per-call
    /// `LlamaContext` for inference.
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

    fn convert(&self, input: &str, options: &ConvertOptions) -> Result<Vec<Candidate>, KanjiError> {
        // Step 1: shared input contract (spec §5.6 — hiragana-only, ≤128 chars).
        crate::kanji::backend::validate_input(input)?;

        // Step 2: spec §5.7 short-circuit — empty input or `top_k == 0` returns
        // an empty Vec without ever touching the model.
        if input.is_empty() || options.top_k == 0 {
            return Ok(Vec::new());
        }

        // Step 3: Zenz expects katakana per the upstream dataset contract
        // (`Miwa-Keita/zenz-v2.5-dataset` README). Kotoha's public API is
        // hiragana-only, so the conversion is an internal concern.
        let katakana = crate::kana::hiragana_to_katakana(input);

        // Step 4: build the AzooKey-derived prompt (see WBS log).
        let prompt = build_prompt(&katakana);

        // Step 5: inference. Phase 1 produces a single greedy candidate;
        // Phase 2+ expands to beam search / n-best sampling once the Zenz
        // scoring story (per-token log-prob aggregation, diverse candidate
        // generation) is nailed down.
        let raw = infer(&self.model, &prompt, options)
            .map_err(|reason| KanjiError::Backend { reason })?;

        // Step 6: enforce spec §5.7 output guarantees (descending sort,
        // surface dedupe, top_k truncation) uniformly with MockBackend.
        Ok(crate::kanji::backend::score_sort_dedupe(raw, options.top_k))
    }
}

/// Builds the Zenz prompt for a katakana-preprocessed input.
///
/// # Prompt format (zenz-v3 / AzooKey Zenzai convention)
///
/// Derived from the AzooKey Zenzai reference implementation; full analysis
/// lives in the P1-2 WBS "prompt format 解析ログ" section. The prompt uses
/// Unicode PUA separator tokens that the GGUF tokenizer resolves to Zenz's
/// special-token ids:
///
/// - `U+EE00` (placeholder — exact codepoint pending P1-2-9 empirical
///   verification against the GGUF `tokenizer.ggml.added_tokens` list) acts
///   as the `<context>` opener. Empty context is acceptable for Phase 1.
/// - `U+EE01` acts as the `<input_katakana>` prefix.
/// - `U+EE02` acts as the `<output>` marker.
/// - `"</s>"` is the EOS marker.
///
/// Keep this function pure (`&str -> String`) so Phase 2+ can unit-test it
/// independently of llama-cpp-2 once the codepoints are locked down.
fn build_prompt(input_katakana: &str) -> String {
    // RESEARCH-DEPENDENT: exact PUA codepoints pending P1-2-9 empirical check.
    const CONTEXT: &str = "\u{EE00}";
    const INPUT: &str = "\u{EE01}";
    const OUTPUT: &str = "\u{EE02}";
    const EOS: &str = "</s>";
    format!("{CONTEXT}{INPUT}{input_katakana}{OUTPUT}{EOS}")
}

/// Runs greedy inference against the loaded model and returns a single raw
/// candidate.
///
/// Phase 2+ can expand this to beam search / n-best sampling. Phase 1 keeps
/// the loop minimal: one decoder pass per token, greedy sampler, no
/// length-normalized log-prob aggregation (the score is held at 0.0 because
/// extracting per-token logits from the greedy sampler requires reaching
/// into the lower-level `LlamaTokenDataArray` API; left for Phase 2).
///
/// # Errors
///
/// Returns `Err(reason)` for any FFI / decode / sampling failure. The caller
/// wraps the string into [`KanjiError::Backend`].
fn infer(
    model: &LlamaModel,
    prompt: &str,
    options: &ConvertOptions,
) -> Result<Vec<Candidate>, String> {
    // Phase 1 only supports greedy decoding. Reject `temperature > 0` early
    // so a future caller does not silently get the greedy path when they
    // asked for stochastic sampling.
    if options.temperature > f32::EPSILON {
        return Err(format!(
            "temperature > 0 sampling is not implemented in Phase 1 (requested {})",
            options.temperature
        ));
    }
    // `options.seed` is unused by greedy decoding but kept reachable for
    // Phase 2+ stochastic samplers; touching it here avoids an
    // `unused_variables` lint without `#[allow]`.
    let _ = options.seed;

    let backend = llama_backend().map_err(|e| format!("LlamaBackend init failed: {e}"))?;

    // Default context parameters are sufficient for ≤128-char hiragana inputs;
    // Phase 2+ may explicitly tune `n_ctx` once we benchmark longer queries.
    let ctx_params = LlamaContextParams::default();
    let mut ctx = model
        .new_context(backend, ctx_params)
        .map_err(|e| format!("context init failed: {e}"))?;

    // Tokenize the prompt. The Zenz prompt already encodes the
    // context/input/output markers explicitly, so we tell llama-cpp-2 to
    // skip its own BOS injection.
    let tokens = model
        .str_to_token(prompt, AddBos::Never)
        .map_err(|e| format!("tokenize failed: {e}"))?;
    if tokens.is_empty() {
        return Ok(Vec::new());
    }

    // Feed the prompt into the model. Only the last position needs logits
    // (the first sample step reads from there).
    let n_prompt = tokens.len();
    let mut batch = LlamaBatch::new(n_prompt, 1);
    for (i, token) in tokens.iter().enumerate() {
        let logits = i == n_prompt - 1;
        let pos = i32::try_from(i).map_err(|e| format!("prompt position overflow: {e}"))?;
        batch
            .add(*token, pos, &[0], logits)
            .map_err(|e| format!("batch add failed: {e}"))?;
    }
    ctx.decode(&mut batch)
        .map_err(|e| format!("prompt decode failed: {e}"))?;

    // Greedy sampler. Phase 2+ swaps in a temperature/top-k/top-p chain.
    let mut sampler = LlamaSampler::greedy();

    // Bound generation length. AzooKey-style heuristic: at most 3× input
    // chars, capped between 32 and 256 tokens. P1-2-9 records the empirically
    // observed maximum once a real model is loaded.
    let max_new_tokens: usize = (prompt.chars().count() * 3).clamp(32, 256);
    let eos = model.token_eos();
    // Accumulate raw bytes from each detokenized token, then UTF-8 decode at
    // the end. Zenz outputs Japanese characters whose UTF-8 encoding may span
    // multiple tokens, so per-token `String` decoding can split a multi-byte
    // sequence and produce replacement characters. Buffering bytes side-steps
    // that without pulling in `encoding_rs` (a transitive dep of llama-cpp-2
    // that we do not want to surface in Kotoha's direct dependency list).
    let mut surface_bytes: Vec<u8> = Vec::new();

    for step in 0..max_new_tokens {
        // Sample the next token from the most recent logits. `-1` selects the
        // last position whose logits were materialized in the previous decode.
        let next = sampler.sample(&ctx, -1);
        sampler.accept(next);

        if next == eos || model.is_eog_token(next) {
            break;
        }

        // Detokenize. `special = false` keeps PUA separator tokens (and any
        // other GGUF special tokens) from leaking into the candidate surface.
        // Buffer size 64 fits any single Japanese token (UTF-8 max 4 bytes per
        // code point, typical Zenz tokens are subword fragments ≤ a few code
        // points); `token_to_piece_bytes` retries internally if undersized.
        let piece = model
            .token_to_piece_bytes(next, 64, false, None)
            .map_err(|e| format!("detokenize failed at step {step}: {e}"))?;
        surface_bytes.extend_from_slice(&piece);

        // Feed the new token so the next step sees the updated KV cache.
        let mut step_batch = LlamaBatch::new(1, 1);
        let pos = i32::try_from(n_prompt + step)
            .map_err(|e| format!("step position overflow at {step}: {e}"))?;
        step_batch
            .add(next, pos, &[0], true)
            .map_err(|e| format!("batch add (step {step}) failed: {e}"))?;
        ctx.decode(&mut step_batch)
            .map_err(|e| format!("decode (step {step}) failed: {e}"))?;
    }

    if surface_bytes.is_empty() {
        return Ok(Vec::new());
    }
    // Lossy decode: replacement characters indicate a truncated final token,
    // which Phase 2+ can guard against with a stateful streaming decoder.
    let surface = String::from_utf8_lossy(&surface_bytes).into_owned();

    // Phase 1 placeholder score. Real per-token log-prob aggregation requires
    // pulling the `LlamaTokenDataArray` snapshot before each greedy pick;
    // left for Phase 2+ when beam search / n-best diversity needs comparable
    // scores across candidates.
    let score: f32 = 0.0;
    Ok(vec![Candidate::new(surface, score)])
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
