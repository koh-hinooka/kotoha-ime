//! Property tests for InputContext using proptest.
//!
//! Four properties per spec §11.3 and ADR 0002:
//!
//! 1. `prop_hiragana_sticky_stability`: from `(Hiragana, Sticky)`, driving
//!    any string of non-uppercase chars + `commit` keeps the state at
//!    `(Hiragana, Sticky)`. Uppercase chars are excluded from this
//!    property because they are the Shift-trigger; the trigger behavior
//!    is verified by property 2 separately.
//! 2. `prop_transient_must_return`: from `(Direct, Transient)` (induced by
//!    a single uppercase char from `(Hiragana, Sticky)`), driving any
//!    string + `commit` returns to `(Hiragana, Sticky)`.
//! 3. `prop_sticky_direct_persistence`: from `(Direct, Sticky)` (induced
//!    by `set_mode(Direct)`), driving any string + `commit` keeps the
//!    state at `(Direct, Sticky)`. The `allow_transient_to_sticky_promotion`
//!    flag is irrelevant here (origin is already Sticky).
//! 4. `prop_reset_idempotence`: calling `reset()` any number of times
//!    (1..=8) ends in `(Hiragana, Sticky)` with `preedit() == ""`.

use kotoha_core::{InputContext, InputMode};
use proptest::prelude::*;

/// Lowercase romaji + punctuation, up to length 12. Uppercase letters are
/// excluded to avoid the Shift-trigger side-effect in property 1.
fn non_uppercase_input() -> impl Strategy<Value = String> {
    "[a-z!?.,\\-]{0,12}"
}

/// Any ASCII printable char sequence including uppercase, length 0..=12.
/// Used for property 2 and 3.
fn any_input() -> impl Strategy<Value = String> {
    "[a-zA-Z!?.,\\-]{0,12}"
}

/// Helper: drive an InputContext with every char in `input`, then commit.
fn drive_and_commit(ctx: &mut InputContext, input: &str) -> String {
    for ch in input.chars() {
        let _ = ctx.input_char(ch);
    }
    ctx.commit()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Property 1: (Hiragana, Sticky) is stable under any non-uppercase
    /// input + commit loop.
    #[test]
    fn prop_hiragana_sticky_stability(
        inputs in prop::collection::vec(non_uppercase_input(), 0..=4),
    ) {
        let mut ctx = InputContext::new();
        prop_assert_eq!(ctx.mode(), InputMode::Hiragana);
        for input in &inputs {
            let _ = drive_and_commit(&mut ctx, input);
            prop_assert_eq!(
                ctx.mode(),
                InputMode::Hiragana,
                "state drifted after input={:?} (non-uppercase)",
                input
            );
        }
    }

    /// Property 2: (Direct, Transient) always returns to (Hiragana, Sticky)
    /// after any input + commit.
    #[test]
    fn prop_transient_must_return(
        trigger in "[A-Z]",
        tail in any_input(),
    ) {
        let mut ctx = InputContext::new();
        // Enter Transient via Shift-trigger.
        for ch in trigger.chars() {
            let _ = ctx.input_char(ch);
        }
        prop_assert_eq!(ctx.mode(), InputMode::Direct);
        // Drive the tail and commit.
        let _ = drive_and_commit(&mut ctx, &tail);
        prop_assert_eq!(
            ctx.mode(),
            InputMode::Hiragana,
            "Transient Direct did not return after trigger={:?} tail={:?}",
            trigger,
            tail
        );
    }

    /// Property 3: (Direct, Sticky) persists across any input + commit.
    #[test]
    fn prop_sticky_direct_persistence(
        inputs in prop::collection::vec(any_input(), 0..=4),
    ) {
        let mut ctx = InputContext::new();
        ctx.set_mode(InputMode::Direct);
        prop_assert_eq!(ctx.mode(), InputMode::Direct);
        for input in &inputs {
            let _ = drive_and_commit(&mut ctx, input);
            prop_assert_eq!(
                ctx.mode(),
                InputMode::Direct,
                "Direct Sticky drifted after input={:?}",
                input
            );
        }
    }

    /// Property 4: reset() is idempotent and always ends in
    /// (Hiragana, Sticky) with empty preedit.
    #[test]
    fn prop_reset_idempotence(count in 1u32..=8) {
        let mut ctx = InputContext::new();
        // Contaminate the state a bit before reset.
        let _ = ctx.input_char('H');
        let _ = ctx.input_char('i');
        // Reset `count` times.
        for _ in 0..count {
            ctx.reset();
        }
        prop_assert_eq!(ctx.mode(), InputMode::Hiragana);
        prop_assert_eq!(ctx.preedit(), "");
    }
}

/// Pinned regression: Transient -> auto-return on commit with empty tail
/// (the simplest Transient path).
#[test]
fn regression_transient_empty_tail_returns() {
    let mut ctx = InputContext::new();
    let _ = ctx.input_char('H');
    assert_eq!(ctx.mode(), InputMode::Direct);
    let _ = ctx.commit();
    assert_eq!(ctx.mode(), InputMode::Hiragana);
}

/// Pinned regression: Sticky Direct persists across multiple commits.
#[test]
fn regression_sticky_direct_across_multiple_commits() {
    let mut ctx = InputContext::new();
    ctx.set_mode(InputMode::Direct);
    for _ in 0..5 {
        let _ = ctx.input_char('h');
        let _ = ctx.commit();
        assert_eq!(ctx.mode(), InputMode::Direct);
    }
}
