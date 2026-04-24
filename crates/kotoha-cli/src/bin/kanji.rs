//! `kotoha-kanji` — Phase 1 Layer 3 kanji-conversion CLI.
//!
//! Reads hiragana lines from stdin, feeds each line through a
//! [`kotoha_core::kanji::KanjiBackend`] backed by llama.cpp loading a
//! Gemma-2-2B-jpn-it (or Qwen2-Instruct) GGUF file, and emits the
//! top-k kanji candidates to stdout — one surface per line, terminated
//! with LF. An empty input line produces an empty line on stdout so
//! that stdin/stdout line alignment is preserved.
//!
//! The pure parsing / formatting logic lives in
//! [`kotoha_cli::kanji_cli`]; this binary is intentionally thin.
//!
//! Spec: `docs/superpowers/specs/2026-04-24-kotoha-phase-1-design.md`
//! §8.3 (feature-gated smoke policy), §9.2 (CLI behavior), §14
//! (input/output contract).

use std::io::{self, BufRead, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;
use kotoha_cli::kanji_cli::{
    build_options, format_candidates, parse_prompt_template, resolve_model_path, KanjiCliArgs,
};
use kotoha_core::kanji::{load_backend, BackendConfig};

/// Phase 1 CLI for kana→kanji conversion via the llama.cpp backend.
#[derive(Debug, Parser)]
#[command(
    name = "kotoha-kanji",
    version,
    about = "Phase 1 CLI for Kotoha's Layer 3 kana→kanji conversion (llama.cpp backend)"
)]
struct Cli {
    /// Absolute path to a Gemma-2-2B-jpn-it (Q5_K_M recommended) or
    /// Qwen2-Instruct GGUF file. Takes precedence over
    /// `KOTOHA_LLAMA_MODEL_PATH`.
    #[arg(long)]
    model_path: Option<PathBuf>,

    /// Prompt-template tag: `"gemma2-instruct-chat"` (default) or `"qwen2-chat"`.
    #[arg(long, default_value = "gemma2-instruct-chat")]
    prompt_template: String,

    /// Maximum number of candidates to emit per input line.
    #[arg(long, default_value_t = 1)]
    top_k: usize,

    /// Sampling temperature (`0.0` = greedy decoding).
    #[arg(long, default_value_t = 0.0)]
    temperature: f32,

    /// RNG seed for reproducible sampling.
    #[arg(long, default_value_t = 0)]
    seed: u64,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(msg) => {
            eprintln!("kotoha-kanji: {msg}");
            // Exit code 2 is reserved by convention for usage / configuration
            // errors (matches clap's own parse-error exit code). Runtime I/O
            // and inference failures also route here in Phase 1 because the
            // CLI is used only for manual verification and smoke scripts;
            // spec §14 does not distinguish sub-exit-codes at this stage.
            ExitCode::from(2)
        }
    }
}

/// Glue that wires `clap` output to the backend and stdin/stdout loop.
///
/// All string-level parsing lives in [`kotoha_cli::kanji_cli`]; this
/// function only composes those helpers with the runtime `env::var`
/// lookup and the actual `load_backend` call.
fn run(cli: Cli) -> Result<(), String> {
    let model_path = resolve_model_path(cli.model_path, || {
        std::env::var("KOTOHA_LLAMA_MODEL_PATH").ok()
    })?;
    let prompt_template = parse_prompt_template(&cli.prompt_template)?;

    let args = KanjiCliArgs {
        model_path: model_path.clone(),
        prompt_template: prompt_template.clone(),
        top_k: cli.top_k,
        temperature: cli.temperature,
        seed: cli.seed,
    };
    let options = build_options(&args);

    let backend = load_backend(&BackendConfig::LlamaCpp {
        model_path,
        prompt_template,
    })
    .map_err(|e| format!("backend load failed: {e}"))?;

    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut out = stdout.lock();

    for line_result in stdin.lock().lines() {
        let line = line_result.map_err(|e| format!("failed to read from stdin: {e}"))?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            // Preserve stdin/stdout line alignment: an empty input line
            // yields a single empty stdout line.
            writeln!(out).map_err(|e| format!("failed to write to stdout: {e}"))?;
            continue;
        }
        let candidates = backend
            .convert(trimmed, &options)
            .map_err(|e| format!("convert failed for input {trimmed:?}: {e}"))?;
        let formatted = format_candidates(&candidates);
        if formatted.is_empty() {
            // Backend returned zero candidates (top_k == 0, or model
            // chose to emit nothing). Still emit an empty line so the
            // caller can line-align input and output.
            writeln!(out).map_err(|e| format!("failed to write to stdout: {e}"))?;
        } else {
            write!(out, "{formatted}").map_err(|e| format!("failed to write to stdout: {e}"))?;
        }
    }

    Ok(())
}
