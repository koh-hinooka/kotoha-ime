//! Input mode enums for the Kotoha IME.
//!
//! This module defines two enums that together describe the
//! [`InputContext`](crate::input::context::InputContext) state tuple per
//! spec §8.1:
//!
//! - [`InputMode`] — the user-visible mode (Hiragana / Direct). Public.
//! - [`ModeOrigin`] — the internal origin marker (Sticky / Transient) that
//!   decides whether a commit triggers auto-return to `(Hiragana, Sticky)`.
//!   Crate-private.
//!
//! The Transient-by-default policy for Shift-induced Direct mode is pinned
//! in ADR 0002 (`docs/adr/0002-input-mode-transient-vs-sticky.md`).

/// User-visible input mode.
///
/// Marked `#[non_exhaustive]` so additional modes (for example Katakana or
/// Zenkaku) may be introduced in later phases without breaking external
/// match sites.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum InputMode {
    /// Romaji → kana conversion mode. Default state.
    Hiragana,
    /// Direct ASCII / punctuation passthrough mode.
    Direct,
}

/// Internal origin marker for the current mode.
///
/// Decides whether a commit / cancel triggers auto-return to
/// `(Hiragana, Sticky)`. See spec §8 and ADR 0002.
///
/// Crate-private; not part of the public API. The public API exposes only
/// `InputMode` and observable behavior.
// NOTE: `allow(dead_code)` until `context::InputContext` (added by
// Task M4b-4) starts consuming this enum. Removed in the M4b-4 commit.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ModeOrigin {
    /// Initial state, or explicit-toggle-induced sticky mode. Persists
    /// across commit / cancel.
    Sticky,
    /// Shift-trigger-induced transient mode. Reverts to
    /// `(Hiragana, Sticky)` on commit / cancel.
    Transient,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn input_mode_equality_is_per_variant() {
        assert_eq!(InputMode::Hiragana, InputMode::Hiragana);
        assert_eq!(InputMode::Direct, InputMode::Direct);
        assert_ne!(InputMode::Hiragana, InputMode::Direct);
    }

    #[test]
    fn input_mode_is_copy() {
        let m = InputMode::Hiragana;
        let n = m; // Copy, not move
        assert_eq!(m, n);
    }

    #[test]
    fn mode_origin_equality_is_per_variant() {
        assert_eq!(ModeOrigin::Sticky, ModeOrigin::Sticky);
        assert_eq!(ModeOrigin::Transient, ModeOrigin::Transient);
        assert_ne!(ModeOrigin::Sticky, ModeOrigin::Transient);
    }
}
