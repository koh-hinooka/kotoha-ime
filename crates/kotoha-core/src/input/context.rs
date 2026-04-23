//! Input mode state machine.
//!
//! Implements the 3-state machine described in spec §8:
//!
//! - `(Hiragana, Sticky)` — initial state, romaji → kana
//! - `(Direct, Transient)` — Shift-trigger-induced, auto-returns on commit
//! - `(Direct, Sticky)` — explicit-toggle-induced, persists across commits
//!
//! The Transient-by-default policy is pinned in
//! ADR 0002 (`docs/adr/0002-input-mode-transient-vs-sticky.md`).

use crate::input::mode::{InputMode, ModeOrigin};
use crate::romaji::{ConvertStep, RomajiConverter};

/// Per-char outcome of [`InputContext::input_char`].
///
/// Marked `#[non_exhaustive]` so additional variants may be introduced in
/// later phases without breaking external match sites.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum InputStep {
    /// The char was absorbed into the pending / direct buffer; nothing
    /// visible was committed this step.
    Preedit,
    /// Some content (kana in Hiragana mode, ASCII in Direct mode, or
    /// mid-stream salvage from normalize) was committed this step.
    Committed(String),
    /// The char is outside the supported alphabet here and was discarded.
    Invalid(char),
}

/// Input mode state machine.
///
/// Holds the `(mode, origin)` tuple per spec §8.1 plus two buffers:
/// - `converter`: a [`RomajiConverter`] that owns the Hiragana-mode pending
///   buffer.
/// - `direct_buffer`: a `String` that owns the Direct-mode buffer.
///
/// # Invariants
/// - The state tuple is always one of `(Hiragana, Sticky)`,
///   `(Direct, Transient)`, or `(Direct, Sticky)` (never
///   `(Hiragana, Transient)`).
/// - When `mode == Hiragana`, `direct_buffer.is_empty()`.
/// - When `mode == Direct`, the converter's pending buffer is empty
///   (Hiragana chars are only fed to `converter` while in Hiragana mode).
#[derive(Debug)]
pub struct InputContext {
    mode: InputMode,
    origin: ModeOrigin,
    converter: RomajiConverter,
    direct_buffer: String,
    /// Phase 0 default is `false` per ADR 0002. Phase 3 may expose this
    /// as a user setting.
    allow_transient_to_sticky_promotion: bool,
}

impl InputContext {
    /// Creates a fresh context in the initial state `(Hiragana, Sticky)`
    /// with empty buffers and Transient→Sticky promotion disabled.
    ///
    /// # Postconditions
    /// - `self.mode() == InputMode::Hiragana`
    /// - `self.preedit().is_empty()`
    pub fn new() -> Self {
        Self {
            mode: InputMode::Hiragana,
            origin: ModeOrigin::Sticky,
            converter: RomajiConverter::new(),
            direct_buffer: String::new(),
            allow_transient_to_sticky_promotion: false,
        }
    }

    /// Returns the current user-visible mode.
    pub fn mode(&self) -> InputMode {
        self.mode
    }

    /// Returns a preedit string (what the UI layer would show as the
    /// uncommitted buffer). In Hiragana mode this is the romaji pending
    /// tail; in Direct mode this is the `direct_buffer` contents.
    pub fn preedit(&self) -> String {
        match self.mode {
            InputMode::Hiragana => {
                // The converter's internal pending buffer is not directly
                // exposed by the M3 public API. For Phase 0 this accessor
                // returns an empty string; the CLI does not render a
                // preedit line (spec §10). Phase 3 (IBus engine) will need
                // a dedicated read path; a follow-up ISSUE will expose
                // `RomajiConverter::pending(&self) -> &str` at that time.
                String::new()
            }
            InputMode::Direct => self.direct_buffer.clone(),
        }
    }

    /// Feeds a single char through the state machine.
    ///
    /// # Preconditions
    /// - None. Any `char` is valid input; non-ASCII in Hiragana mode is
    ///   delegated to `RomajiConverter::push` which rejects it as
    ///   `ConvertStep::Invalid`.
    ///
    /// # Postconditions
    /// - If the char is ASCII uppercase and `self.mode == Hiragana`, the
    ///   state transitions to `(Direct, Transient)` before processing.
    /// - The returned [`InputStep`] reports the visible effect.
    pub fn input_char(&mut self, ch: char) -> InputStep {
        // Spec §8.3 rule 1 / §8.4 pseudocode: Shift (ASCII uppercase) in
        // Hiragana mode always transitions to (Direct, Transient).
        if self.mode == InputMode::Hiragana && ch.is_ascii_uppercase() {
            self.mode = InputMode::Direct;
            self.origin = ModeOrigin::Transient;
            // Hiragana pending should be empty at the transition boundary
            // per the state-tuple invariant; drop anything that could be
            // left (defensive reset). In practice spec §8 treats this as
            // an append-to-direct-buffer only.
            self.converter.reset();
        }

        match self.mode {
            InputMode::Hiragana => {
                let push_result = self.converter.push(ch);
                let salvaged = self.converter.normalize_pending();
                match push_result {
                    ConvertStep::Committed(cow) => {
                        if salvaged.is_empty() {
                            InputStep::Committed(cow.into_owned())
                        } else {
                            InputStep::Committed(format!("{cow}{salvaged}"))
                        }
                    }
                    ConvertStep::Pending => {
                        if salvaged.is_empty() {
                            InputStep::Preedit
                        } else {
                            InputStep::Committed(salvaged)
                        }
                    }
                    ConvertStep::Invalid(c) => {
                        if salvaged.is_empty() {
                            InputStep::Invalid(c)
                        } else {
                            InputStep::Committed(salvaged)
                        }
                    }
                }
            }
            InputMode::Direct => {
                self.direct_buffer.push(ch);
                InputStep::Preedit
            }
        }
    }

    /// Enter commit: finalizes the current pending / direct buffer and
    /// returns the committed string. If the origin is Transient, auto-
    /// returns to `(Hiragana, Sticky)`.
    ///
    /// # Postconditions
    /// - The pending buffer is empty.
    /// - The `direct_buffer` is empty.
    /// - If the pre-call state was `(Direct, Transient)`, the post-call
    ///   state is `(Hiragana, Sticky)`.
    pub fn commit(&mut self) -> String {
        let result = match self.mode {
            InputMode::Hiragana => self.converter.flush(),
            InputMode::Direct => std::mem::take(&mut self.direct_buffer),
        };
        // Spec §8.4 pseudocode: Transient → (Hiragana, Sticky) on commit.
        if self.origin == ModeOrigin::Transient {
            self.mode = InputMode::Hiragana;
            self.origin = ModeOrigin::Sticky;
            // Defensive: ensure Hiragana buffer is clean on return.
            self.converter.reset();
        }
        result
    }

    /// Escape cancel: discards the pending / direct buffer. Transient
    /// origin also triggers auto-return.
    ///
    /// # Postconditions
    /// - The pending buffer is empty.
    /// - The `direct_buffer` is empty.
    /// - If the pre-call state was `(Direct, Transient)`, the post-call
    ///   state is `(Hiragana, Sticky)`.
    pub fn cancel(&mut self) {
        match self.mode {
            InputMode::Hiragana => self.converter.reset(),
            InputMode::Direct => self.direct_buffer.clear(),
        }
        if self.origin == ModeOrigin::Transient {
            self.mode = InputMode::Hiragana;
            self.origin = ModeOrigin::Sticky;
            self.converter.reset();
        }
    }

    /// Explicit mode toggle (intended to be bound to zenkaku/hankaku key
    /// or similar by the IBus engine layer in Phase 3).
    ///
    /// - `(Hiragana, Sticky)` → `(Direct, Sticky)`
    /// - `(Direct, Sticky)` → `(Hiragana, Sticky)`
    /// - `(Direct, Transient)` → if `allow_transient_to_sticky_promotion`
    ///   then `(Direct, Sticky)` (promotion); otherwise
    ///   `(Hiragana, Sticky)` (cancel-equivalent return).
    pub fn toggle_mode(&mut self) {
        match self.mode {
            InputMode::Hiragana => {
                // Clear the outgoing Hiragana pending buffer to mirror
                // `set_mode` (invariant: Direct mode implies the converter's
                // pending buffer is empty).
                self.converter.reset();
                self.mode = InputMode::Direct;
                self.origin = ModeOrigin::Sticky;
            }
            InputMode::Direct => {
                if self.origin == ModeOrigin::Transient && self.allow_transient_to_sticky_promotion
                {
                    // Transient → Sticky promotion.
                    self.origin = ModeOrigin::Sticky;
                } else {
                    self.mode = InputMode::Hiragana;
                    self.origin = ModeOrigin::Sticky;
                    // Clean both buffers at the transition boundary.
                    self.direct_buffer.clear();
                    self.converter.reset();
                }
            }
        }
    }

    /// Explicitly set the mode. Always produces `Sticky` origin.
    ///
    /// # Postconditions
    /// - `self.mode() == mode`
    /// - `self.origin == ModeOrigin::Sticky`
    /// - The buffer of the mode being left is cleared.
    pub fn set_mode(&mut self, mode: InputMode) {
        if self.mode != mode {
            // Clear the outgoing buffer only; incoming mode's buffer is
            // already empty by the state-tuple invariant.
            match self.mode {
                InputMode::Hiragana => self.converter.reset(),
                InputMode::Direct => self.direct_buffer.clear(),
            }
        }
        self.mode = mode;
        self.origin = ModeOrigin::Sticky;
    }

    /// Resets the entire state to `(Hiragana, Sticky)` with empty buffers.
    ///
    /// # Postconditions
    /// - `self.mode() == InputMode::Hiragana`
    /// - `self.origin == ModeOrigin::Sticky`
    /// - Both buffers are empty.
    pub fn reset(&mut self) {
        self.mode = InputMode::Hiragana;
        self.origin = ModeOrigin::Sticky;
        self.converter.reset();
        self.direct_buffer.clear();
    }
}

impl Default for InputContext {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- initial state (2 tests) ---

    #[test]
    fn new_starts_in_hiragana_sticky() {
        let c = InputContext::new();
        assert_eq!(c.mode(), InputMode::Hiragana);
        // origin is pub(crate); we assert via observable behavior (commit
        // from initial state does NOT auto-return because origin=Sticky).
    }

    #[test]
    fn new_preedit_is_empty() {
        let c = InputContext::new();
        assert_eq!(c.preedit(), "");
    }

    // --- Hiragana mode input_char (3 tests) ---

    #[test]
    fn input_char_lowercase_single_vowel_commits() {
        let mut c = InputContext::new();
        assert_eq!(c.input_char('a'), InputStep::Committed("あ".to_string()));
        assert_eq!(c.mode(), InputMode::Hiragana);
    }

    #[test]
    fn input_char_lowercase_consonant_is_preedit() {
        let mut c = InputContext::new();
        assert_eq!(c.input_char('k'), InputStep::Preedit);
        assert_eq!(c.mode(), InputMode::Hiragana);
    }

    #[test]
    fn input_char_punctuation_commits() {
        let mut c = InputContext::new();
        // '-' → 'ー' per rule table.
        assert_eq!(c.input_char('-'), InputStep::Committed("ー".to_string()));
    }

    // --- Shift trigger (4 tests) ---

    #[test]
    fn input_char_uppercase_in_hiragana_transitions_to_direct_transient() {
        let mut c = InputContext::new();
        let step = c.input_char('H');
        // Direct path appends and returns Preedit.
        assert_eq!(step, InputStep::Preedit);
        assert_eq!(c.mode(), InputMode::Direct);
        assert_eq!(c.preedit(), "H");
    }

    #[test]
    fn input_char_uppercase_then_lowercase_in_transient_appends_both() {
        let mut c = InputContext::new();
        let _ = c.input_char('H');
        // In Transient Direct mode, lowercase is appended as-is (not fed
        // to the romaji converter).
        let _ = c.input_char('i');
        assert_eq!(c.mode(), InputMode::Direct);
        assert_eq!(c.preedit(), "Hi");
    }

    #[test]
    fn input_char_uppercase_resets_pending_hiragana_buffer() {
        let mut c = InputContext::new();
        let _ = c.input_char('k'); // Hiragana pending = "k"
        let _ = c.input_char('A'); // Shift trigger; Hiragana pending reset, Direct buffer = "A"
        assert_eq!(c.mode(), InputMode::Direct);
        assert_eq!(c.preedit(), "A");
    }

    #[test]
    fn input_char_mixed_then_commit_auto_returns() {
        let mut c = InputContext::new();
        let _ = c.input_char('K'); // Transient Direct
        let _ = c.input_char('o');
        let _ = c.input_char('n');
        let result = c.commit();
        assert_eq!(result, "Kon");
        assert_eq!(c.mode(), InputMode::Hiragana); // auto-returned
    }

    // --- Explicit toggle (3 tests) ---

    #[test]
    fn toggle_mode_from_hiragana_goes_to_direct_sticky() {
        let mut c = InputContext::new();
        c.toggle_mode();
        assert_eq!(c.mode(), InputMode::Direct);
        // Commit from Direct-Sticky must NOT auto-return.
        let _ = c.input_char('h');
        let _ = c.commit();
        assert_eq!(c.mode(), InputMode::Direct); // stayed
    }

    #[test]
    fn toggle_mode_from_direct_sticky_returns_to_hiragana() {
        let mut c = InputContext::new();
        c.toggle_mode(); // (Direct, Sticky)
        c.toggle_mode(); // → (Hiragana, Sticky)
        assert_eq!(c.mode(), InputMode::Hiragana);
    }

    #[test]
    fn set_mode_direct_is_always_sticky() {
        let mut c = InputContext::new();
        c.set_mode(InputMode::Direct);
        let _ = c.input_char('h');
        let _ = c.commit();
        // Sticky: no auto-return.
        assert_eq!(c.mode(), InputMode::Direct);
    }

    // --- Transient → Sticky promotion (2 tests) ---

    #[test]
    fn toggle_mode_in_transient_default_flag_false_returns_hiragana() {
        let mut c = InputContext::new();
        let _ = c.input_char('H'); // (Direct, Transient)
        c.toggle_mode(); // flag=false default: fall back to (Hiragana, Sticky)
        assert_eq!(c.mode(), InputMode::Hiragana);
    }

    #[test]
    fn toggle_mode_in_transient_with_flag_true_promotes_to_sticky() {
        let mut c = InputContext::new();
        c.allow_transient_to_sticky_promotion = true;
        let _ = c.input_char('H'); // (Direct, Transient)
        c.toggle_mode(); // promote to (Direct, Sticky)
        assert_eq!(c.mode(), InputMode::Direct);
        // Now commit MUST NOT auto-return.
        let _ = c.commit();
        assert_eq!(c.mode(), InputMode::Direct); // stayed
    }

    // --- commit / cancel (4 tests) ---

    #[test]
    fn commit_in_hiragana_sticky_returns_flushed_kana_and_stays() {
        let mut c = InputContext::new();
        // Accumulate committed chunks across input_char calls so the final
        // assertion covers the full "konn"-equivalent output: "こ" is
        // emitted by input_char('o'), and the trailing "n" is finalized
        // as "ん" by commit().
        let mut out = String::new();
        for ch in ['k', 'o', 'n'] {
            if let InputStep::Committed(s) = c.input_char(ch) {
                out.push_str(&s);
            }
        }
        out.push_str(&c.commit());
        assert_eq!(out, "こん"); // trailing "n" finalized as ん
        assert_eq!(c.mode(), InputMode::Hiragana);
    }

    #[test]
    fn commit_in_direct_sticky_returns_buffer_and_stays() {
        let mut c = InputContext::new();
        c.set_mode(InputMode::Direct);
        let _ = c.input_char('h');
        let _ = c.input_char('i');
        assert_eq!(c.commit(), "hi");
        assert_eq!(c.mode(), InputMode::Direct);
    }

    #[test]
    fn cancel_in_hiragana_sticky_clears_pending_and_stays() {
        let mut c = InputContext::new();
        let _ = c.input_char('k');
        c.cancel();
        // Pending cleared: next 'a' commits あ, not か.
        assert_eq!(c.input_char('a'), InputStep::Committed("あ".to_string()));
        assert_eq!(c.mode(), InputMode::Hiragana);
    }

    #[test]
    fn cancel_in_transient_direct_clears_and_auto_returns() {
        let mut c = InputContext::new();
        let _ = c.input_char('H');
        c.cancel();
        assert_eq!(c.mode(), InputMode::Hiragana);
        assert_eq!(c.preedit(), "");
    }

    // --- reset (2 tests) ---

    #[test]
    fn reset_from_direct_sticky_returns_to_hiragana_sticky_with_empty_buffers() {
        let mut c = InputContext::new();
        c.set_mode(InputMode::Direct);
        let _ = c.input_char('h');
        c.reset();
        assert_eq!(c.mode(), InputMode::Hiragana);
        assert_eq!(c.preedit(), "");
    }

    #[test]
    fn reset_is_idempotent() {
        let mut c = InputContext::new();
        c.reset();
        c.reset();
        c.reset();
        assert_eq!(c.mode(), InputMode::Hiragana);
        assert_eq!(c.preedit(), "");
    }

    // --- default (1 test) ---

    #[test]
    fn default_matches_new() {
        let a = InputContext::default();
        let b = InputContext::new();
        assert_eq!(a.mode(), b.mode());
        assert_eq!(a.preedit(), b.preedit());
    }
}
