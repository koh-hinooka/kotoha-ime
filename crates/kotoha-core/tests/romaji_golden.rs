//! Golden test harness for RomajiConverter.
//!
//! Reads `tests/fixtures/romaji_cases.tsv` and asserts that
//! `RomajiConverter::convert(input)` equals `(expected_committed, expected_pending)`
//! for every non-comment, non-empty row.

use std::collections::HashSet;
use std::fs;
use std::path::Path;

use kotoha_core::RomajiConverter;

struct Case {
    line_no: usize,
    input: String,
    expected_committed: String,
    expected_pending: String,
}

fn load_cases() -> Vec<Case> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("romaji_cases.tsv");
    let text = fs::read_to_string(&path).unwrap_or_else(|e| panic!("failed to read {path:?}: {e}"));
    let mut cases = Vec::new();
    for (i, raw) in text.lines().enumerate() {
        let line_no = i + 1;
        let line = raw;
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let cols: Vec<&str> = line.split('\t').collect();
        assert!(
            cols.len() >= 3,
            "line {line_no}: expected 3 TAB-separated columns, got {}: {raw:?}",
            cols.len()
        );
        cases.push(Case {
            line_no,
            input: cols[0].to_string(),
            expected_committed: cols[1].to_string(),
            expected_pending: cols[2].to_string(),
        });
    }
    cases
}

/// Ensures the golden fixture exercises every key declared in the
/// `romaji::rules::RULES` table.
///
/// Without this cross-check, a newly added (or regressed) rule entry
/// that lacks a fixture row would silently escape the
/// `every_fixture_row_matches_converter` check, because that test
/// only iterates the fixture — not the rule table. The check below
/// turns the invariant "every RULES key has golden coverage" into a
/// compile-and-run-time guard: any uncovered key is reported with
/// its kana value to simplify adding the missing fixture row.
#[test]
fn every_rule_key_has_golden_coverage() {
    let cases = load_cases();
    let fixture_inputs: HashSet<&str> = cases.iter().map(|c| c.input.as_str()).collect();
    let rules = kotoha_core::__test_only_romaji_rules();
    let missing: Vec<(&'static str, &'static str)> = rules
        .iter()
        .filter(|(k, _)| !fixture_inputs.contains(*k))
        .copied()
        .collect();
    assert!(
        missing.is_empty(),
        "{} rule key(s) missing from golden fixture (each entry is (romaji_key, expected_kana)): {:?}",
        missing.len(),
        missing
    );
}

#[test]
fn every_fixture_row_matches_converter() {
    let cases = load_cases();
    let converter = RomajiConverter::new();
    let mut failures: Vec<String> = Vec::new();
    for case in &cases {
        let (got_committed, got_pending) = converter.convert(&case.input);
        if got_committed != case.expected_committed || got_pending != case.expected_pending {
            failures.push(format!(
                "line {}: input={:?}\n  expected: committed={:?}, pending={:?}\n  got:      committed={:?}, pending={:?}",
                case.line_no,
                case.input,
                case.expected_committed,
                case.expected_pending,
                got_committed,
                got_pending,
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} golden cases failed:\n{}",
        failures.len(),
        failures.join("\n")
    );
}
