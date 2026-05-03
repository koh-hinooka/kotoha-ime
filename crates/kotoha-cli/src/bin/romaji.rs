//! `kotoha-romaji` — Phase 0 manual-verification CLI for the Kotoha IME.
//!
//! Reads stdin line by line, feeds each char through
//! [`kotoha_core::InputContext`], and prints the committed string (with
//! an optional mode suffix) to stdout.
//!
//! See spec §10 for the full contract and spec §13.2 for the 10
//! acceptance assertions covered by `scripts/phase0-smoke.sh`.

use std::io::{self, BufRead, Write};
use std::process::ExitCode;

use clap::{Parser, ValueEnum};
use kotoha_cli::{format_line_output, process_line};
use kotoha_core::{InputContext, InputMode};
use tracing_subscriber::EnvFilter;

/// Env var controlling the `tracing` filter directive. Mirrors `kotoha-bin`'s
/// `KOTOHA_LOG` for consistency across the workspace's binaries.
const KOTOHA_LOG_ENV: &str = "KOTOHA_LOG";

/// CLI mode selector.
///
/// Mirrors [`kotoha_core::InputMode`] but is declared locally so we can
/// attach `#[derive(ValueEnum)]`. The conversion to the library type
/// happens in [`CliMode::into_input_mode`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum CliMode {
    /// Romaji → kana conversion mode (default).
    Hiragana,
    /// Direct ASCII / punctuation passthrough mode (Sticky origin).
    Direct,
}

impl CliMode {
    fn into_input_mode(self) -> InputMode {
        match self {
            CliMode::Hiragana => InputMode::Hiragana,
            CliMode::Direct => InputMode::Direct,
        }
    }
}

/// Phase 0 CLI for manual verification of Kotoha's romaji → kana core.
#[derive(Debug, Parser)]
#[command(
    name = "kotoha-romaji",
    version,
    about = "Phase 0 CLI for manual verification of Kotoha's romaji → kana core"
)]
struct Cli {
    /// Initial input mode.
    #[arg(short = 'm', long = "mode", value_enum, default_value_t = CliMode::Hiragana)]
    mode: CliMode,

    /// Append ` [H]` or ` [D]` to each output line to show the current mode.
    #[arg(long = "show-mode", default_value_t = false)]
    show_mode: bool,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    init_tracing();

    let mut ctx = InputContext::new();
    // `--mode direct` sets Sticky Direct before entering the stdin loop
    // so commit() does NOT auto-return to Hiragana (plan 既知懸念 3).
    if cli.mode == CliMode::Direct {
        ctx.set_mode(cli.mode.into_input_mode());
    }

    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut stdout = stdout.lock();

    for line_result in stdin.lock().lines() {
        let line = match line_result {
            Ok(l) => l,
            Err(e) => {
                eprintln!("kotoha-romaji: failed to read from stdin: {e}");
                // Spec §10.5 maps read failure to exit 1. (Arg-parse errors
                // exit 2 via clap default; see plan 既知懸念 4.)
                return ExitCode::from(1);
            }
        };
        let committed = process_line(&mut ctx, &line);
        // `--show-mode` reflects the mode AFTER commit (i.e., what the
        // next line would start in). Transient → Hiragana auto-return
        // has already happened inside process_line.
        let formatted = format_line_output(&committed, ctx.mode(), cli.show_mode);
        if let Err(e) = writeln!(stdout, "{formatted}") {
            eprintln!("kotoha-romaji: failed to write to stdout: {e}");
            return ExitCode::from(1);
        }
    }

    ExitCode::SUCCESS
}

/// `tracing_subscriber` を stderr 向けに初期化する。`KOTOHA_LOG` 未設定時は
/// `warn` default(CLI 通常運用で flooding しない)、設定済 + parse 失敗時は
/// `eprintln!` で warning を出して `warn` に fallback する。
///
/// ISSUE #39 / PR #168 self-review High:本 binary は以前 subscriber 未初期化で
/// `tracing::warn!` が `NoSubscriber` で drop されていた(BufferFull arm の
/// observability 主張が CLI 経由で成立しなかった)。kotoha-bin の `init_tracing`
/// と同 pattern で配線する。
fn init_tracing() {
    let filter = match std::env::var(KOTOHA_LOG_ENV) {
        Ok(directive) => match EnvFilter::try_new(&directive) {
            Ok(f) => f,
            Err(e) => {
                eprintln!(
                    "warning: invalid {KOTOHA_LOG_ENV}={directive:?} ({e}); falling back to warn"
                );
                EnvFilter::new("warn")
            }
        },
        Err(_) => EnvFilter::new("warn"),
    };
    tracing_subscriber::fmt()
        .with_writer(io::stderr)
        .with_env_filter(filter)
        .init();
}
