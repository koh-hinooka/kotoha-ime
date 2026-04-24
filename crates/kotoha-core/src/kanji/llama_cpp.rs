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
use llama_cpp_2::model::{AddBos, LlamaModel};
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

/// Builds the raw prompt string consumed by `infer`.
///
/// Phase 1 bypasses llama-cpp-2's `apply_chat_template` for Gemma / Qwen
/// instruct variants because the chat turn structure triggers their
/// conversational short-answer bias (empirical P1-2.5-8: Gemma-2-2B-jpn-it
/// under `apply_chat_template` stops at `日本` for input `にほんご`
/// instead of producing `日本語`). Instead, a plain-text few-shot prompt is
/// assembled and the model completes in text-completion mode — matching the
/// harness that produced 5/5 PASS in the original P1-2-9 empirical
/// verification (llama-cpp-python text completion).
///
/// The 7 few-shot pairs cover: short single kanji (えき → 駅) / multi-char
/// kanji compound (にほんご → 日本語, directly reinforcing the row that
/// regressed) / yōon (ちゃわん → 茶碗) / okurigana compound (たべもの →
/// 食べ物) / honorific suffix (やまださん → 山田さん) / loanword mid-phrase
/// (パソコンをつかう → パソコンを使う) / full sentence with particles
/// (わたしはがくせいです → 私は学生です).
///
/// For [`PromptTemplate::Custom`], the caller's `system` + `user_wrapper` +
/// `assistant_prefix` fields are composed verbatim; no few-shot scaffold is
/// injected.
fn build_prompt(template: &PromptTemplate, user_input: &str) -> String {
    match template {
        PromptTemplate::Gemma2InstructChat | PromptTemplate::Qwen2Chat => {
            format!(
                "あなたは正確な日本語IMEエンジンです。入力されたひらがな文字列の音韻をそのまま保った漢字表記に変換してください。これは音声的な1対1の写像であり、意味を同じくする別の語への翻訳・類義語置換・言い換えは行いません。例えば「あした」は「明日」であり「翌日」ではありません。「ぎゅうにゅう」は「牛乳」であり「ミルク」ではありません。「りょうり」は「料理」であり「クッキング」ではありません。入力の全ての文字を省略せず最後まで変換し、単独の単語であっても標準的な漢字表記に変換します。カタカナ由来の外来語は長音符「ー」を含めて正しくカタカナで復元します。変換結果のみを1行で出力し、説明・記号・引用符は付けません。\n\n\
                 入力: えき\n出力: 駅\n\n\
                 入力: にほんご\n出力: 日本語\n\n\
                 入力: ちゃわん\n出力: 茶碗\n\n\
                 入力: ぎゅうにゅう\n出力: 牛乳\n\n\
                 入力: きっぷ\n出力: 切符\n\n\
                 入力: こーひー\n出力: コーヒー\n\n\
                 入力: はっぴょう\n出力: 発表\n\n\
                 入力: りょうり\n出力: 料理\n\n\
                 入力: たべもの\n出力: 食べ物\n\n\
                 入力: やまださん\n出力: 山田さん\n\n\
                 入力: パソコンをつかう\n出力: パソコンを使う\n\n\
                 入力: わたしはがくせいです\n出力: 私は学生です\n\n\
                 入力: あした\n出力: 明日\n\n\
                 入力: {user_input}\n出力: "
            )
        }
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
            format!("{system_part}{prefix}{user_input}{suffix}{assistant_prefix}")
        }
    }
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

    // Build the raw prompt string. Phase 1 uses plain-text completion (not
    // chat-template wrapping) for llama.cpp-family instruct models; see the
    // `build_prompt` rustdoc for the empirical rationale.
    let prompt = build_prompt(template, input);

    // Default context parameters are sufficient for ≤128-char hiragana inputs;
    // Phase 2+ may explicitly tune `n_ctx` once we benchmark longer queries.
    // n_ctx 1024: default 512 is tight with the 7-pair few-shot prompt
    // (~350-400 tokens) plus generation headroom. 1024 absorbs fluctuation
    // while keeping memory footprint well under Phase 1 2 GB budget.
    let ctx_params = LlamaContextParams::default().with_n_ctx(std::num::NonZeroU32::new(1024));
    let mut ctx = model
        .new_context(backend, ctx_params)
        .map_err(|e| format!("context init failed: {e}"))?;

    // Tokenize the prompt. Plain-text completion mode: llama-cpp-2 must
    // prepend the model's `<bos>` token because the hand-built prompt does
    // not include it explicitly.
    let tokens = model
        .str_to_token(&prompt, AddBos::Always)
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
    // Plain-text completion stop signal: once the model has emitted any
    // non-whitespace content, the first subsequent newline terminates the
    // answer. Without this guard the model would continue generating the
    // next fake `入力: ... 出力: ...` example up to `max_new_tokens`.
    let mut has_non_whitespace = false;

    for step in 0..max_new_tokens {
        // Sample the next token from the most recent logits. `-1` selects the
        // last position whose logits were materialized in the previous decode.
        let next = sampler.sample(&ctx, -1);
        sampler.accept(next);

        if next == eos || model.is_eog_token(next) {
            break;
        }

        // Detokenize. `special = false` keeps any GGUF special tokens from
        // leaking into the candidate surface. Buffer size 64 fits any single
        // Japanese token (UTF-8 max 4 bytes per code point, typical subword
        // fragments are ≤ a few code points); `token_to_piece_bytes` retries
        // internally if undersized.
        let piece = model
            .token_to_piece_bytes(next, 64, false, None)
            .map_err(|e| format!("detokenize failed at step {step}: {e}"))?;

        // Stop on first newline after content. The plain-text few-shot
        // prompt ends with `出力: `; a single line of completion is the
        // expected answer. `\n` in ASCII is byte 0x0A and cannot appear as
        // a UTF-8 continuation byte (0x80-0xBF), so byte-level scan is safe.
        if has_non_whitespace {
            if let Some(nl_pos) = piece.iter().position(|&b| b == b'\n') {
                surface_bytes.extend_from_slice(&piece[..nl_pos]);
                break;
            }
        }
        if piece
            .iter()
            .any(|&b| !matches!(b, b' ' | b'\t' | b'\n' | b'\r'))
        {
            has_non_whitespace = true;
        }
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
    // Trim trailing ASCII whitespace (stop loop may include leading spaces
    // that the prompt's `出力: ` suffix did not absorb).
    let surface = String::from_utf8_lossy(&surface_bytes).trim().to_string();

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
