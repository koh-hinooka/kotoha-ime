//! Pure helpers for the `kotoha-kanji` CLI binary.
//!
//! This module keeps the CLI binary (`src/bin/kanji.rs`) thin: the
//! binary performs only `clap` parsing, stdin/stdout I/O, and backend
//! construction. All string-level parsing and formatting logic lives
//! here so that it can be exercised by unit tests without spawning a
//! subprocess or loading a GGUF.
//!
//! # Responsibilities
//!
//! - Resolve the model path from either the `--model-path` CLI flag or
//!   the `KOTOHA_LLAMA_MODEL_PATH` environment variable.
//! - Parse the `--prompt-template` tag into a
//!   [`kotoha_core::kanji::PromptTemplate`] variant. Only
//!   `gemma2-instruct-chat` and `qwen2-chat` are accepted;
//!   [`PromptTemplate::Custom`](kotoha_core::kanji::PromptTemplate::Custom)
//!   stays programmatic-only per the P1-3 design decision.
//! - Build a [`kotoha_core::kanji::ConvertOptions`] from the parsed CLI
//!   arguments.
//! - Format a candidate slice into stdout-ready text (one surface per
//!   line, terminated with a newline).
//!
//! Spec: `docs/superpowers/specs/2026-04-24-kotoha-phase-1-design.md`
//! §9.2 (CLI binary behavior) and §14 (input/output contract).

use std::path::PathBuf;

use kotoha_core::kanji::{Candidate, ConvertOptions, PromptTemplate};

/// Parsed `kotoha-kanji` CLI arguments after `clap` conversion.
///
/// This struct exists so `build_options` and the binary's `run` glue
/// can share one typed representation. `clap` constructs its own
/// `#[derive(Parser)]` type in the binary; that type is converted into
/// `KanjiCliArgs` exactly once at startup.
///
/// # Invariants
///
/// - `top_k >= 0` (guaranteed by `usize`).
/// - `temperature >= 0.0` is recommended but NOT enforced here; the
///   backend enforces its own domain constraint.
///
/// # Trait derivations
///
/// `PartialEq` is intentionally omitted because
/// [`kotoha_core::kanji::PromptTemplate`] is `#[non_exhaustive]` and
/// does not derive it. Unit tests that need field-level equality
/// inspect individual fields instead.
#[derive(Debug, Clone)]
pub struct KanjiCliArgs {
    /// Absolute path to a Gemma-2-2B-jpn-it (or Qwen2-Instruct) GGUF file.
    pub model_path: PathBuf,
    /// Prompt template variant selected via the CLI tag (no `Custom`).
    pub prompt_template: PromptTemplate,
    /// Number of candidates to emit per input line. `0` yields no output lines.
    pub top_k: usize,
    /// Sampling temperature. `0.0` selects greedy decoding.
    pub temperature: f32,
    /// RNG seed for reproducible sampling.
    pub seed: u64,
}

/// Resolves the model path from either the `--model-path` CLI flag or
/// the `KOTOHA_LLAMA_MODEL_PATH` environment variable.
///
/// The `env_getter` parameter is injected as a closure so unit tests
/// can exercise the resolution logic without mutating process-wide
/// environment variables. The binary passes
/// `|| std::env::var("KOTOHA_LLAMA_MODEL_PATH").ok()` at runtime.
///
/// # Preconditions
///
/// None.
///
/// # Postconditions
///
/// - Returns `Ok(path)` if `cli_flag` is `Some(path)` (flag takes precedence).
/// - Returns `Ok(PathBuf::from(v.trim()))` if `cli_flag` is `None` and
///   `env_getter()` returns `Some(v)` with `v` non-empty after trim
///   (leading and trailing ASCII whitespace is trimmed before the
///   `PathBuf` is constructed).
/// - Returns `Err(_)` with a clear English message if both are absent
///   or the environment value is empty.
///
/// # Errors
///
/// Returns `Err` when neither the flag nor the environment variable
/// provides a usable path.
pub fn resolve_model_path(
    cli_flag: Option<PathBuf>,
    env_getter: impl FnOnce() -> Option<String>,
) -> Result<PathBuf, String> {
    if let Some(path) = cli_flag {
        return Ok(path);
    }
    match env_getter() {
        Some(v) if !v.trim().is_empty() => Ok(PathBuf::from(v.trim())),
        _ => Err(
            "--model-path not provided and KOTOHA_LLAMA_MODEL_PATH is unset or empty".to_string(),
        ),
    }
}

/// Parses a prompt-template tag string into a
/// [`PromptTemplate`] variant.
///
/// Accepted tags:
///
/// - `"gemma2-instruct-chat"` → [`PromptTemplate::Gemma2InstructChat`]
/// - `"qwen2-chat"`           → [`PromptTemplate::Qwen2Chat`]
///
/// [`PromptTemplate::Custom`] is intentionally not exposed through the
/// CLI surface: hand-authored templates require a three-field structure
/// (`system`, `user_wrapper`, `assistant_prefix`) that does not map
/// cleanly to a single `--prompt-template` flag, and the Phase 1
/// built-in tokenizers cover the two shipped models.
///
/// # Preconditions
///
/// None.
///
/// # Postconditions
///
/// - Returns `Ok(PromptTemplate::Gemma2InstructChat)` for the Gemma tag.
/// - Returns `Ok(PromptTemplate::Qwen2Chat)` for the Qwen tag.
/// - Returns `Err(_)` with an English message listing the accepted tags
///   for any other input.
///
/// # Errors
///
/// Returns `Err` for unknown tags.
pub fn parse_prompt_template(tag: &str) -> Result<PromptTemplate, String> {
    match tag {
        "gemma2-instruct-chat" => Ok(PromptTemplate::Gemma2InstructChat),
        "qwen2-chat" => Ok(PromptTemplate::Qwen2Chat),
        other => Err(format!(
            "unknown --prompt-template tag {other:?}; accepted values are \"gemma2-instruct-chat\" or \"qwen2-chat\""
        )),
    }
}

/// Formats a candidate slice into stdout-ready text.
///
/// Each candidate's `surface` is emitted on its own line (LF terminator).
/// An empty input slice produces an empty string — the caller may choose
/// to still emit a separator newline to preserve stdin/stdout line
/// alignment, but this helper does NOT add one.
///
/// # Preconditions
///
/// None.
///
/// # Postconditions
///
/// - Returns `""` for an empty slice.
/// - Returns `"<surface>\n"` for a single candidate.
/// - Returns `"<s0>\n<s1>\n...\n<sN-1>\n"` for N candidates, preserving
///   the input order (the caller is responsible for sorting by score).
///
/// # Examples
///
/// ```
/// use kotoha_cli::kanji_cli::format_candidates;
/// use kotoha_core::kanji::Candidate;
///
/// let out = format_candidates(&[Candidate::new("日本語", 0.9)]);
/// assert_eq!(out, "日本語\n");
/// ```
pub fn format_candidates(candidates: &[Candidate]) -> String {
    let mut out = String::new();
    for c in candidates {
        out.push_str(&c.surface);
        out.push('\n');
    }
    out
}

/// Constructs [`ConvertOptions`] from parsed CLI arguments.
///
/// Builds the options via `Default` + field mutation so the callsite
/// survives future `#[non_exhaustive]` additions to `ConvertOptions`
/// (spec §5.2 / ADR 0006). This matches the pattern used by
/// `crates/kotoha-core/tests/kanji_llama_cpp_smoke.rs`.
///
/// # Preconditions
///
/// None.
///
/// # Postconditions
///
/// - The returned `ConvertOptions` has `top_k == args.top_k`,
///   `temperature == args.temperature`, and `seed == Some(args.seed)`.
// `ConvertOptions` is `#[non_exhaustive]`, so struct update syntax
// (`..Default::default()`) is unavailable outside its defining crate.
// We build via `Default::default()` and field mutation instead.
#[allow(clippy::field_reassign_with_default)]
pub fn build_options(args: &KanjiCliArgs) -> ConvertOptions {
    let mut opt = ConvertOptions::default();
    opt.top_k = args.top_k;
    opt.temperature = args.temperature;
    opt.seed = Some(args.seed);
    opt
}

#[cfg(test)]
mod tests {
    use super::*;

    // ======================================================================
    // resolve_model_path
    // ======================================================================

    #[test]
    fn resolve_model_path_prefers_cli_flag_over_env() {
        let cli = Some(PathBuf::from("/flag/model.gguf"));
        let got = resolve_model_path(cli, || Some("/env/model.gguf".to_string()))
            .expect("flag path must resolve");
        assert_eq!(got, PathBuf::from("/flag/model.gguf"));
    }

    #[test]
    fn resolve_model_path_falls_back_to_env_when_flag_absent() {
        let got = resolve_model_path(None, || Some("/env/model.gguf".to_string()))
            .expect("env path must resolve when flag is None");
        assert_eq!(got, PathBuf::from("/env/model.gguf"));
    }

    #[test]
    fn resolve_model_path_errors_when_both_absent() {
        let err = resolve_model_path(None, || None).expect_err("both absent must error");
        assert!(
            err.contains("KOTOHA_LLAMA_MODEL_PATH"),
            "error must mention the env var name: {err}"
        );
    }

    #[test]
    fn resolve_model_path_errors_when_env_is_empty_string() {
        // An exported-but-empty env var must be treated as "unset" so that
        // operators who accidentally run `export KOTOHA_LLAMA_MODEL_PATH=`
        // get a clear diagnostic instead of a "file not found" deep inside
        // the backend loader.
        let err =
            resolve_model_path(None, || Some("   ".to_string())).expect_err("empty env must error");
        assert!(err.contains("KOTOHA_LLAMA_MODEL_PATH"));
    }

    // ======================================================================
    // parse_prompt_template
    // ======================================================================

    #[test]
    fn parse_prompt_template_accepts_gemma2_tag() {
        let t = parse_prompt_template("gemma2-instruct-chat").expect("gemma2 tag must be accepted");
        assert!(matches!(t, PromptTemplate::Gemma2InstructChat));
    }

    #[test]
    fn parse_prompt_template_accepts_qwen2_tag() {
        let t = parse_prompt_template("qwen2-chat").expect("qwen2 tag must be accepted");
        assert!(matches!(t, PromptTemplate::Qwen2Chat));
    }

    #[test]
    fn parse_prompt_template_rejects_unknown_tag() {
        let err = parse_prompt_template("phi4-instruct").expect_err("unknown tag must error");
        assert!(
            err.contains("gemma2-instruct-chat") && err.contains("qwen2-chat"),
            "error must list both accepted tags: {err}"
        );
    }

    #[test]
    fn parse_prompt_template_rejects_empty_tag() {
        let err = parse_prompt_template("").expect_err("empty tag must error");
        assert!(err.contains("gemma2-instruct-chat"));
    }

    // ======================================================================
    // format_candidates
    // ======================================================================

    #[test]
    fn format_candidates_empty_slice_returns_empty_string() {
        assert_eq!(format_candidates(&[]), "");
    }

    #[test]
    fn format_candidates_single_candidate_one_line() {
        let out = format_candidates(&[Candidate::new("日本語", 0.9)]);
        assert_eq!(out, "日本語\n");
    }

    #[test]
    fn format_candidates_preserves_input_order() {
        let out =
            format_candidates(&[Candidate::new("日本語", 0.9), Candidate::new("二本後", 0.3)]);
        assert_eq!(out, "日本語\n二本後\n");
    }

    // ======================================================================
    // build_options
    // ======================================================================

    #[test]
    fn build_options_round_trips_cli_values() {
        let args = KanjiCliArgs {
            model_path: PathBuf::from("/tmp/model.gguf"),
            prompt_template: PromptTemplate::Gemma2InstructChat,
            top_k: 3,
            temperature: 0.7,
            seed: 42,
        };
        let opt = build_options(&args);
        assert_eq!(opt.top_k, 3);
        assert!((opt.temperature - 0.7).abs() < 1e-6);
        assert_eq!(opt.seed, Some(42));
    }
}
