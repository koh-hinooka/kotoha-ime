//! Golden test harness for InputContext.
//!
//! Reads `tests/fixtures/mode_cases.tsv` (70 cases) and
//! `tests/fixtures/mode_cases_karukan_diff.tsv` (10 cases) and asserts
//! that driving `InputContext` with the `input` column produces the
//! `expected_output` column and ends in `final_mode`.
//!
//! TSV format per spec §11.2:
//! - 4 TAB-separated columns: `initial_mode`, `input`, `expected_output`,
//!   `final_mode`.
//! - Within `input` / `expected_output`, the literal two characters `\n`
//!   mean a `commit()` boundary. Between `\n` chunks, each char is fed to
//!   `input_char` in order.
//! - Lines starting with `#` are comments. Empty lines are ignored.
//! - Trailing ` # ...` (one or more spaces then `#`) inside a data row is
//!   stripped from the row as inline documentation.
//!
//! `initial_mode` / `final_mode` are `hiragana` or `direct`.

use std::fs;
use std::path::{Path, PathBuf};

use kotoha_core::{InputContext, InputMode, InputStep};

#[derive(Debug)]
struct Case {
    fixture: &'static str,
    line_no: usize,
    initial_mode: InputMode,
    input: String,
    expected_output: String,
    final_mode: InputMode,
}

fn parse_mode(s: &str, ctx: &str) -> InputMode {
    match s {
        "hiragana" => InputMode::Hiragana,
        "direct" => InputMode::Direct,
        other => panic!("{ctx}: unknown mode {other:?} (expected 'hiragana' or 'direct')"),
    }
}

fn strip_inline_comment(line: &str) -> &str {
    if let Some(idx) = line.find(" #") {
        line[..idx].trim_end()
    } else if let Some(idx) = line.find("\t#") {
        line[..idx].trim_end()
    } else {
        line
    }
}

fn load_cases_from(path: &Path, fixture_name: &'static str) -> Vec<Case> {
    let text = fs::read_to_string(path).unwrap_or_else(|e| panic!("failed to read {path:?}: {e}"));
    let mut cases = Vec::new();
    for (i, raw) in text.lines().enumerate() {
        let line_no = i + 1;
        if raw.is_empty() || raw.starts_with('#') {
            continue;
        }
        let line = strip_inline_comment(raw);
        if line.is_empty() {
            continue;
        }
        let cols: Vec<&str> = line.split('\t').collect();
        assert!(
            cols.len() >= 4,
            "{fixture_name} line {line_no}: expected 4 TAB-separated columns, got {}: {raw:?}",
            cols.len()
        );
        cases.push(Case {
            fixture: fixture_name,
            line_no,
            initial_mode: parse_mode(cols[0], &format!("{fixture_name} line {line_no} col1")),
            input: cols[1].to_string(),
            expected_output: cols[2].to_string(),
            final_mode: parse_mode(cols[3], &format!("{fixture_name} line {line_no} col4")),
        });
    }
    cases
}

fn fixture_path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

fn load_all_cases() -> Vec<Case> {
    let mut all = Vec::new();
    all.extend(load_cases_from(
        &fixture_path("mode_cases.tsv"),
        "mode_cases.tsv",
    ));
    all.extend(load_cases_from(
        &fixture_path("mode_cases_karukan_diff.tsv"),
        "mode_cases_karukan_diff.tsv",
    ));
    all
}

/// Drive an InputContext with a fixture row's `input` column and return the
/// observed output string.
///
/// The visible output at each chunk is the concatenation of every
/// `InputStep::Committed(s)` emitted by `input_char` within the chunk
/// followed by the `commit()` return value (the flushed residual). In
/// Hiragana mode kana chars are typically emitted step-by-step through
/// `InputStep::Committed`; in Direct mode the whole buffer is returned
/// by `commit()` with every `input_char` returning `Preedit`.
///
/// Chunks are delimited by the literal two-char `\n` marker inside the
/// TSV `input` column. Between chunks the accumulated per-chunk output is
/// emitted and a literal `\n` marker is appended to `out`.
fn run_input(initial_mode: InputMode, input: &str) -> (String, InputMode) {
    let mut ctx = InputContext::new();
    if initial_mode != InputMode::Hiragana {
        ctx.set_mode(initial_mode);
    }
    let mut out = String::new();
    // Split on the literal two-char "\n" marker (NOT on the single char
    // '\n'). Each chunk is fed to input_char one char at a time, then
    // commit() is called at the chunk boundary.
    let chunks: Vec<&str> = input.split("\\n").collect();
    let chunk_count = chunks.len();
    for (i, chunk) in chunks.iter().enumerate() {
        let mut chunk_out = String::new();
        for ch in chunk.chars() {
            if let InputStep::Committed(s) = ctx.input_char(ch) {
                chunk_out.push_str(&s);
            }
        }
        // The TSV convention is that every `input` ends with literal "\n",
        // so `split` always yields a trailing empty chunk that should NOT
        // emit a commit. All non-final chunks are followed by a commit()
        // boundary.
        let is_last_chunk = i == chunk_count - 1;
        if !is_last_chunk {
            chunk_out.push_str(&ctx.commit());
            out.push_str(&chunk_out);
            out.push_str("\\n");
        } else {
            // Anything captured from input_char in the trailing empty
            // chunk would be nothing (empty chunk has no chars to feed),
            // but append it for symmetry; no trailing \n marker since
            // this chunk follows the last \n marker already written.
            out.push_str(&chunk_out);
        }
    }
    (out, ctx.mode())
}

#[test]
fn mode_fixture_has_at_least_70_main_cases() {
    let main = load_cases_from(&fixture_path("mode_cases.tsv"), "mode_cases.tsv");
    assert!(
        main.len() >= 70,
        "mode_cases.tsv must have >= 70 cases, got {}",
        main.len()
    );
}

#[test]
fn karukan_diff_fixture_has_at_least_10_cases() {
    let diff = load_cases_from(
        &fixture_path("mode_cases_karukan_diff.tsv"),
        "mode_cases_karukan_diff.tsv",
    );
    assert!(
        diff.len() >= 10,
        "mode_cases_karukan_diff.tsv must have >= 10 cases, got {}",
        diff.len()
    );
}

#[test]
fn every_mode_fixture_row_matches_context() {
    let cases = load_all_cases();
    let mut failures: Vec<String> = Vec::new();
    for case in &cases {
        let (got_output, got_final_mode) = run_input(case.initial_mode, &case.input);
        if got_output != case.expected_output || got_final_mode != case.final_mode {
            failures.push(format!(
                "{} line {}: initial_mode={:?} input={:?}\n  expected: output={:?}, final_mode={:?}\n  got:      output={:?}, final_mode={:?}",
                case.fixture,
                case.line_no,
                case.initial_mode,
                case.input,
                case.expected_output,
                case.final_mode,
                got_output,
                got_final_mode,
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} mode golden cases failed:\n{}",
        failures.len(),
        failures.join("\n")
    );
}
