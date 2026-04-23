//! Property tests for RomajiConverter using proptest.
//!
//! Two properties (per spec §11.3 revision 2):
//!
//! 1. Retraction on committed output: re-running `convert` on the committed
//!    kana of a prior `convert` leaves the pending buffer empty. Kana output
//!    is non-ASCII and is dropped by the state machine as `Invalid` per
//!    ADR 0001 / spec §9.2, so it never regenerates romaji in the pending
//!    buffer. Strict `committed_second == committed_first` is explicitly
//!    out-of-contract per ADR 0001; only the pending invariant is asserted.
//! 2. Associativity via pending: `convert(a + b)` decomposes as `convert(a)`
//!    followed by `convert(left_pending + b)`, with committed parts
//!    concatenating and final pending matching.
//!
//! The input strategy is the full plan-spec alphabet
//! `[a-z\-'.,!?\[\]/]{0,12}`. It exercises lowercase romaji, long-vowel
//! mark, apostrophe (n-disambiguation), punctuation (`.,!?`), brackets
//! (`[]`), and middle-dot (`/`). Both properties hold on this domain
//! after PR #27 (ISSUE #23 buffer normalization) and ADR 0001
//! (ISSUE #22 non-ASCII retraction policy pinned to drop).

use kotoha_core::RomajiConverter;
use proptest::prelude::*;

/// Plan-spec romaji-input alphabet: lowercase ASCII plus punctuation and
/// brackets that the rule table maps to kana punctuation. Length bounded
/// at 12 to keep proptest shrinking tractable.
fn romaji_input() -> impl Strategy<Value = String> {
    "[a-z\\-'.,!?\\[\\]/]{0,12}"
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1024))]

    /// Retraction on committed output: converting the committed kana of a
    /// prior `convert` call leaves the pending buffer empty. Per ADR 0001
    /// (spec §9.2), non-ASCII is dropped as `Invalid` by `StateMachine::push`,
    /// so re-entering the committed kana produces no new romaji pending.
    #[test]
    fn prop_idempotence_on_committed(input in romaji_input()) {
        let c = RomajiConverter::new();
        let (committed_first, _pending_first) = c.convert(&input);
        let (_committed_second, pending_second) = c.convert(&committed_first);
        prop_assert!(
            pending_second.is_empty(),
            "pending should be empty after re-converting committed kana; \
             input={:?}, committed_first={:?}, pending_second={:?}",
            input,
            committed_first,
            pending_second
        );
    }

    /// Associativity via pending: for inputs `a` and `b` drawn from the
    /// plan-spec alphabet, `convert(a + b)` equals concatenating
    /// `convert(a).0` with `convert(convert(a).1 + b).0`, and the final
    /// pending matches `convert(convert(a).1 + b).1`.
    #[test]
    fn prop_associativity_via_pending(a in romaji_input(), b in romaji_input()) {
        let c = RomajiConverter::new();
        let ab = format!("{}{}", a, b);
        let (whole_committed, whole_pending) = c.convert(&ab);

        let (left_committed, left_pending) = c.convert(&a);
        let glued = format!("{}{}", left_pending, b);
        let (right_committed, right_pending) = c.convert(&glued);

        let reconstructed = format!("{}{}", left_committed, right_committed);
        prop_assert_eq!(
            &reconstructed,
            &whole_committed,
            "committed mismatch; a={:?}, b={:?}, ab={:?}",
            a,
            b,
            ab
        );
        prop_assert_eq!(
            &right_pending,
            &whole_pending,
            "pending mismatch; a={:?}, b={:?}, ab={:?}",
            a,
            b,
            ab
        );
    }
}

/// Pinned regression tests for inputs that were known counter-examples
/// before the broadening — if any of these fail on a future change,
/// proptest shrinking has a head-start target.
#[test]
fn regression_byb_associative() {
    let c = RomajiConverter::new();
    // whole vs split-at-2 vs split-at-1
    assert_eq!(c.convert("byb"), ("".to_string(), "b".to_string()));

    let (lc, lp) = c.convert("by");
    let (rc, rp) = c.convert(&format!("{}{}", lp, "b"));
    assert_eq!(format!("{}{}", lc, rc), "");
    assert_eq!(rp, "b");

    let (lc, lp) = c.convert("b");
    let (rc, rp) = c.convert(&format!("{}{}", lp, "yb"));
    assert_eq!(format!("{}{}", lc, rc), "");
    assert_eq!(rp, "b");
}

#[test]
fn regression_j_comma_associative() {
    // Before #23 hotfix + #22 ADR, j followed by comma exposed the
    // partial-then-invalid leak: convert("j,") gave ("", ",") while
    // convert(",") alone gave ("、", "").
    let c = RomajiConverter::new();
    assert_eq!(c.convert("j,"), ("、".to_string(), "".to_string()));

    let (lc, lp) = c.convert("j");
    let (rc, rp) = c.convert(&format!("{}{}", lp, ","));
    assert_eq!(format!("{}{}", lc, rc), "、");
    assert_eq!(rp, "");
}

#[test]
fn regression_bracket_idempotent_pending() {
    // "[" maps to 「 (non-ASCII). Re-converting 「 produces empty pending
    // per ADR 0001 (non-ASCII → Invalid, buffer untouched).
    let c = RomajiConverter::new();
    let (c1, p1) = c.convert("[");
    assert_eq!(c1, "「");
    assert_eq!(p1, "");

    let (_c2, p2) = c.convert(&c1);
    assert!(p2.is_empty());
}
