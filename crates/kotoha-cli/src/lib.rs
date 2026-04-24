//! Line-based helpers for the Kotoha CLI binaries.
//!
//! This crate exposes pure functions that the binary entry points call
//! so that the binaries themselves stay thin and the behavior is
//! covered by unit tests without spawning a subprocess.
//!
//! # Module surface
//!
//! - [`process_line`] / [`format_line_output`] — Phase 0 romaji
//!   (`src/bin/romaji.rs`).
//! - [`kanji_cli`] — Phase 1 kanji (`src/bin/kanji.rs`, gated on the
//!   `llama-cpp` feature).
//!
//! # Behavior pins (see plan M6 §『本 M6 plan 内で解決する既知の懸念』)
//!
//! - [`InputStep::Preedit`] is dropped silently (Phase 0 CLI is not a
//!   REPL; spec §10.4 defers interactive preedit to Phase 3).
//! - [`InputStep::Invalid`] is dropped silently (matches spec §9.3.2
//!   `InputContext` internal behavior).
//! - `--show-mode` suffix uses `[H]` for [`InputMode::Hiragana`] and
//!   `[D]` for [`InputMode::Direct`] with no Transient / Sticky
//!   distinction (`ModeOrigin` is `pub(crate)`).

pub mod kanji_cli;

use kotoha_core::{InputContext, InputMode, InputStep};

/// Feeds every char of `line` through the mode state machine, then
/// commits at end-of-line. Returns the committed-per-line string,
/// excluding any trailing newline.
///
/// # Preconditions
/// - `ctx` is in any valid state (the caller guarantees the state-tuple
///   invariant documented on [`InputContext`]).
///
/// # Postconditions
/// - `ctx` has its per-line buffer drained (Hiragana pending flushed,
///   Direct buffer cleared).
/// - If `ctx.mode()` was `(Direct, Transient)` at the start of the
///   call and a commit actually occurred, the post-call mode is
///   [`InputMode::Hiragana`] per ADR 0002.
pub fn process_line(ctx: &mut InputContext, line: &str) -> String {
    let mut out = String::new();
    for ch in line.chars() {
        match ctx.input_char(ch) {
            InputStep::Committed(s) => out.push_str(&s),
            InputStep::Preedit | InputStep::Invalid(_) => {
                // Intentionally silent per plan M6 既知懸念 1.
            }
            // `InputStep` is `#[non_exhaustive]`; future variants default to silent drop.
            _ => {}
        }
    }
    out.push_str(&ctx.commit());
    out
}

/// Formats the per-line CLI output with the optional mode suffix.
///
/// # Preconditions
/// - `committed` is the value returned from [`process_line`] (no trailing newline).
///
/// # Postconditions
/// - If `show_mode == false`, returns `committed` verbatim.
/// - If `show_mode == true`, appends ` [H]` for [`InputMode::Hiragana`]
///   or ` [D]` for [`InputMode::Direct`] (separator = single ASCII space).
pub fn format_line_output(committed: &str, mode: InputMode, show_mode: bool) -> String {
    if !show_mode {
        return committed.to_string();
    }
    let tag = match mode {
        InputMode::Hiragana => "H",
        InputMode::Direct => "D",
        _ => "?", // `InputMode` is `#[non_exhaustive]`; defensive fallback.
    };
    format!("{committed} [{tag}]")
}

#[cfg(test)]
mod tests {
    use super::*;

    // T1
    #[test]
    fn process_line_hiragana_basic() {
        let mut ctx = InputContext::new();
        let out = process_line(&mut ctx, "konnnichiha");
        assert_eq!(out, "こんにちは");
        assert_eq!(ctx.mode(), InputMode::Hiragana);
    }

    // T2
    #[test]
    fn process_line_shift_trigger_auto_returns() {
        let mut ctx = InputContext::new();
        let out = process_line(&mut ctx, "Hello");
        assert_eq!(out, "Hello");
        // Transient Direct → commit auto-returns to (Hiragana, Sticky).
        assert_eq!(ctx.mode(), InputMode::Hiragana);
    }

    // T3
    #[test]
    fn process_line_sticky_direct_persists() {
        let mut ctx = InputContext::new();
        ctx.set_mode(InputMode::Direct);
        let out = process_line(&mut ctx, "hello");
        assert_eq!(out, "hello");
        // Sticky Direct → commit does NOT auto-return.
        assert_eq!(ctx.mode(), InputMode::Direct);
    }

    // T4
    #[test]
    fn process_line_empty_is_empty() {
        let mut ctx = InputContext::new();
        let out = process_line(&mut ctx, "");
        assert_eq!(out, "");
        assert_eq!(ctx.mode(), InputMode::Hiragana);
    }

    // T5
    #[test]
    fn format_line_output_with_hiragana_shows_h() {
        let s = format_line_output("こん", InputMode::Hiragana, true);
        assert_eq!(s, "こん [H]");
    }

    // T6
    #[test]
    fn format_line_output_with_direct_shows_d() {
        let s = format_line_output("hi", InputMode::Direct, true);
        assert_eq!(s, "hi [D]");
    }

    // T7
    #[test]
    fn format_line_output_without_show_mode_no_suffix() {
        let s = format_line_output("こん", InputMode::Hiragana, false);
        assert_eq!(s, "こん");
    }

    // T8
    #[test]
    fn process_line_carries_mode_between_calls() {
        let mut ctx = InputContext::new();
        ctx.set_mode(InputMode::Direct);
        // Line 1 in Sticky Direct.
        let o1 = process_line(&mut ctx, "hello");
        assert_eq!(o1, "hello");
        assert_eq!(ctx.mode(), InputMode::Direct);
        // Line 2 also in Sticky Direct.
        let o2 = process_line(&mut ctx, "world");
        assert_eq!(o2, "world");
        assert_eq!(ctx.mode(), InputMode::Direct);
    }

    // T9
    #[test]
    fn process_line_invalid_chars_are_dropped_silently() {
        // Non-ASCII single char in Hiragana mode: InputContext drops it
        // via Invalid(char) path (spec §9.3.2). CLI must not surface it.
        // Feeding U+3042 'あ' first then 'a':
        // 'あ' is Invalid in Hiragana context and is dropped.
        // 'a' follows the romaji 'a' rule → 'あ'.
        let mut ctx = InputContext::new();
        let out = process_line(&mut ctx, "\u{3042}a");
        assert_eq!(out, "あ");
    }
}
