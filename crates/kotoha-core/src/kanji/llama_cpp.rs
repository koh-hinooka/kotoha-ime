//! llama.cpp-family GGUF backend (via llama-cpp-2).
//!
//! # Status (P1-2.5)
//!
//! This module holds the real kanji conversion backend. An instance owns a
//! llama-cpp-2 `LlamaModel` loaded from a GGUF file, re-used across every
//! `convert` call.
//!
//! # Input handling
//!
//! Phase 1 default (`PromptTemplate::Gemma2InstructChat` + Gemma-2-2B-jpn-it)
//! accepts hiragana directly as the user-turn content; no kana casing
//! adjustment is performed at this layer. Backends that require a different
//! input alphabet (for example, a hypothetical Zenz variant expecting
//! katakana) are responsible for their own preprocessing — spec §5.6 keeps
//! the Kotoha public API contract at hiragana-only, and backend-internal
//! preprocessing is explicitly out of scope of that contract.
//!
//! # Prompt format
//!
//! Prompts are built via llama-cpp-2's `apply_chat_template`, reading the
//! GGUF-embedded `tokenizer.chat_template` through `chat_template(None)`
//! for the [`PromptTemplate::Gemma2InstructChat`] and
//! [`PromptTemplate::Qwen2Chat`] variants. [`PromptTemplate::Custom`] falls
//! back to a hand-authored `{system?}{user_wrapper.0}{input}{user_wrapper.1}{assistant_prefix}`
//! string, letting integrators wire backends whose GGUF lacks embedded
//! chat_template metadata.
//!
//! # Deterministic output
//!
//! With `ConvertOptions::temperature == 0.0` and `ConvertOptions::seed == Some(0)`
//! the backend performs greedy decoding with a fixed seed so Layer 3 and
//! Layer 4 smoke tests are reproducible.
//!
//! Spec: `docs/superpowers/specs/2026-04-24-kotoha-phase-1-design.md` §3.1,
//! §3.2, §5.3, §5.6, §6.

use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use llama_cpp_2::context::params::LlamaContextParams;
use llama_cpp_2::llama_backend::LlamaBackend;
use llama_cpp_2::llama_batch::LlamaBatch;
use llama_cpp_2::model::params::LlamaModelParams;
use llama_cpp_2::model::{AddBos, LlamaChatMessage, LlamaChatTemplate, LlamaModel};
use llama_cpp_2::sampling::LlamaSampler;

use crate::kanji::{Candidate, ConvertOptions, KanjiBackend, KanjiError, PromptTemplate};

/// llama.cpp-family GGUF model backend, backed by llama-cpp-2. Enabled by
/// `feature = "llama-cpp"`.
///
/// Construct via [`crate::kanji::load_backend`] applied to
/// [`crate::kanji::BackendConfig::LlamaCpp`].
///
/// # Invariants
///
/// - `model` holds an initialized llama-cpp-2 `LlamaModel`.
/// - `model_path` is the absolute path that was used to load `model`.
/// - `model_id` is the file stem of `model_path` captured at load time.
/// - `prompt_template` dispatches the prompt construction path per request.
/// - The backend is single-threaded; wrap externally for concurrent use.
pub struct LlamaCppBackend {
    /// Loaded llama-cpp-2 model. Kept private so llama-cpp-2 types do not leak
    /// into the public API surface. Read by `convert` to spin up a per-call
    /// `LlamaContext` for inference.
    model: LlamaModel,
    /// Path the model was loaded from. Used by logging and error diagnostics.
    model_path: PathBuf,
    /// GGUF file stem captured at load time. Returned by
    /// [`KanjiBackend::model_id`] and used in Layer 3 smoke test assertions.
    model_id: String,
    /// Chat/prompt template to apply when constructing the inference prompt.
    /// Dispatched by `convert` via `infer` to either llama-cpp-2's
    /// `apply_chat_template` (for [`PromptTemplate::Gemma2InstructChat`] /
    /// [`PromptTemplate::Qwen2Chat`]) or a hand-authored format string (for
    /// [`PromptTemplate::Custom`]).
    prompt_template: PromptTemplate,
}

// Manual `Debug` impl: `LlamaModel` does not implement `Debug` (FFI-wrapping
// type backed by `NonNull<llama_model>` with no derived impl in llama-cpp-2
// 0.1.145), so `#[derive(Debug)]` on `LlamaCppBackend` would not compile.
// Print the three side fields and a placeholder for `model`.
impl fmt::Debug for LlamaCppBackend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LlamaCppBackend")
            .field("model", &"<LlamaModel>")
            .field("model_path", &self.model_path)
            .field("model_id", &self.model_id)
            .field("prompt_template", &self.prompt_template)
            .finish()
    }
}

impl LlamaCppBackend {
    /// Loads a GGUF model from `model_path` and captures the dispatch template.
    ///
    /// # Preconditions
    ///
    /// - `model_path` points to an existing file. Otherwise
    ///   [`KanjiError::ModelNotFound`] is returned without touching
    ///   llama-cpp-2.
    ///
    /// # Postconditions
    ///
    /// On success, the returned `LlamaCppBackend` owns a llama-cpp-2
    /// [`LlamaModel`] initialized from the GGUF file at `model_path`, the
    /// process-global [`LlamaBackend`] is guaranteed to be initialized, and
    /// `prompt_template` is the value supplied by the caller.
    ///
    /// # Errors
    ///
    /// - [`KanjiError::ModelNotFound`] if the file does not exist.
    /// - [`KanjiError::ModelLoadFailed`] if llama-cpp-2 fails to initialize
    ///   the global backend or parse / initialize the GGUF (wrapping the
    ///   underlying error as `source`).
    pub fn load(model_path: &Path, prompt_template: PromptTemplate) -> Result<Self, KanjiError> {
        if !model_path.exists() {
            return Err(KanjiError::ModelNotFound {
                path: model_path.to_path_buf(),
            });
        }

        let model = load_llama_model(model_path)
            .map_err(|source| KanjiError::ModelLoadFailed { source })?;
        let model_id = model_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("llama-cpp")
            .to_string();

        Ok(Self {
            model,
            model_path: model_path.to_path_buf(),
            model_id,
            prompt_template,
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

impl KanjiBackend for LlamaCppBackend {
    fn model_id(&self) -> &str {
        &self.model_id
    }

    fn convert(&self, input: &str, options: &ConvertOptions) -> Result<Vec<Candidate>, KanjiError> {
        // Step 1: shared input contract (spec §5.6 — hiragana-only, ≤128 chars).
        crate::kanji::backend::validate_input(input)?;

        // Step 2: spec §5.7 short-circuit — empty input or `top_k == 0` returns
        // an empty Vec without ever touching the model.
        if input.is_empty() || options.top_k == 0 {
            return Ok(Vec::new());
        }

        // hiragana→katakana preprocessing is removed in P1-2.5-7 — Phase 1 default
        // (Gemma-2-2B-jpn-it) accepts hiragana natively. Custom prompts handle any
        // kana casing in their own template strings.
        let raw = infer(&self.model, &self.prompt_template, input, options)
            .map_err(|reason| KanjiError::Backend { reason })?;

        Ok(crate::kanji::backend::score_sort_dedupe(raw, options.top_k))
    }
}

/// Produces the role/content tuples consumed by llama-cpp-2
/// `apply_chat_template`. Phase 1 submits a single user turn; the model is
/// expected to return the kanji string as its assistant turn.
///
/// For `Gemma2InstructChat` / `Qwen2Chat`, the user content is wrapped in an
/// IME-style instruction + 2 few-shot examples so the chat-tuned base model
/// performs kana→kanji conversion instead of responding conversationally.
/// Empirical observation (P1-2.5-8): without this instruction wrapper,
/// Gemma-2-2B-jpn-it echoes the hiragana input plus whitespace/emoji noise.
///
/// For [`PromptTemplate::Custom`] with `system: Some(s)`, a `"system"` turn
/// is prepended. `Custom` does not add the instruction wrapper — callers who
/// use `Custom` are expected to bake their own directive into the wrapper
/// strings.
fn build_chat_tuples(template: &PromptTemplate, user_input: &str) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::with_capacity(2);
    if let PromptTemplate::Custom {
        system: Some(system),
        ..
    } = template
    {
        out.push(("system".to_string(), system.clone()));
    }
    match template {
        PromptTemplate::Gemma2InstructChat | PromptTemplate::Qwen2Chat => {
            // Multi-turn few-shot: Gemma-2-2B-jpn-it follows the conversion
            // pattern more reliably when each example is a full user→assistant
            // exchange than when few-shot examples are embedded as plain text
            // in a single turn (empirical P1-2.5-8 observation).
            let directive = "あなたは日本語IMEです。ひらがな入力を漢字交じりの自然な日本語に変換して、変換結果のみを出力してください。";
            // Turn 1 example
            out.push(("user".to_string(), format!("{directive}\n\n入力: にほんご")));
            out.push(("assistant".to_string(), "日本語".to_string()));
            // Turn 2 example
            out.push(("user".to_string(), "入力: やまださん".to_string()));
            out.push(("assistant".to_string(), "山田さん".to_string()));
            // Turn 3 example
            out.push(("user".to_string(), "入力: わたしはがくせいです".to_string()));
            out.push(("assistant".to_string(), "私は学生です".to_string()));
            // Actual query
            out.push(("user".to_string(), format!("入力: {user_input}")));
        }
        PromptTemplate::Custom { .. } => {
            out.push(("user".to_string(), user_input.to_string()));
        }
    }
    out
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
/// The prompt is built per the supplied [`PromptTemplate`]:
/// [`PromptTemplate::Gemma2InstructChat`] and [`PromptTemplate::Qwen2Chat`]
/// dispatch to llama-cpp-2's `apply_chat_template` (reading the
/// GGUF-embedded `tokenizer.chat_template` via `chat_template(None)`);
/// [`PromptTemplate::Custom`] formats `{system?}{user_wrapper.0}{input}{user_wrapper.1}{assistant_prefix}`
/// directly.
///
/// # Errors
///
/// Returns `Err(reason)` for any FFI / decode / sampling / template failure.
/// The caller wraps the string into [`KanjiError::Backend`].
fn infer(
    model: &LlamaModel,
    template: &PromptTemplate,
    input: &str,
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

    // Dispatch prompt construction by template variant.
    let prompt = match template {
        PromptTemplate::Custom {
            user_wrapper,
            assistant_prefix,
            system,
        } => {
            let system_part = system
                .as_ref()
                .map(|s| format!("{s}\n"))
                .unwrap_or_default();
            let (prefix, suffix) = user_wrapper;
            format!("{system_part}{prefix}{input}{suffix}{assistant_prefix}")
        }
        PromptTemplate::Gemma2InstructChat | PromptTemplate::Qwen2Chat => {
            let tmpl: LlamaChatTemplate = model
                .chat_template(None)
                .map_err(|e| format!("chat_template(None) failed: {e}"))?;
            let chat_tuples = build_chat_tuples(template, input);
            let chat: Vec<LlamaChatMessage> = chat_tuples
                .into_iter()
                .map(|(r, c)| LlamaChatMessage::new(r, c))
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| format!("LlamaChatMessage::new failed: {e}"))?;
            model
                .apply_chat_template(&tmpl, &chat, true)
                .map_err(|e| format!("apply_chat_template failed: {e}"))?
        }
    };

    // Default context parameters are sufficient for ≤128-char hiragana inputs;
    // Phase 2+ may explicitly tune `n_ctx` once we benchmark longer queries.
    let ctx_params = LlamaContextParams::default();
    let mut ctx = model
        .new_context(backend, ctx_params)
        .map_err(|e| format!("context init failed: {e}"))?;

    // Tokenize the prompt. The chat template (GGUF-embedded or hand-authored
    // via `Custom`) already supplies any required BOS/role markers, so we
    // tell llama-cpp-2 to skip its own BOS injection.
    let tokens = model
        .str_to_token(&prompt, AddBos::Never)
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

    // Bound generation length: at most 3× the prompt char count, clamped to
    // [32, 256] tokens. The 3× factor accommodates kanji-heavy outputs that
    // expand beyond the hiragana input; the upper cap prevents runaway
    // generation when the model fails to emit EOS.
    let max_new_tokens: usize = (prompt.chars().count() * 3).clamp(32, 256);
    let eos = model.token_eos();
    // Accumulate raw bytes from each detokenized token, then UTF-8 decode at
    // the end. Japanese characters whose UTF-8 encoding spans multiple tokens
    // can have their multi-byte sequences split by per-token `String`
    // decoding, producing replacement characters. Buffering bytes side-steps
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

        // Detokenize. `special = false` keeps any GGUF special tokens (chat
        // template role markers, separator tokens, etc.) from leaking into
        // the candidate surface. Buffer size 64 fits any single Japanese
        // token (UTF-8 max 4 bytes per code point, typical subword fragments
        // are ≤ a few code points); `token_to_piece_bytes` retries
        // internally if undersized.
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
    fn llama_cpp_load_errors_on_missing_file() {
        let err = LlamaCppBackend::load(
            Path::new("/tmp/definitely-does-not-exist-kotoha-p1-2-5.gguf"),
            PromptTemplate::Gemma2InstructChat,
        )
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

    // Note: `llama_cpp_model_id_echoes_file_stem` is not covered here
    // because constructing a real backend requires a live GGUF file. Layer 3
    // smoke (`tests/kanji_llama_cpp_smoke.rs`) asserts that `model_id()`
    // returns a file-stem-derived string once a real backend is loaded via
    // `KOTOHA_LLAMA_MODEL_PATH`.
}
