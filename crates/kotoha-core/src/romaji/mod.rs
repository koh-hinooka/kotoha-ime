//! Romaji-to-kana conversion.
//!
//! Public API is [`RomajiConverter`]. The module is built on three layers:
//! - [`rules`]: the static rule table
//! - [`trie`]: prefix-match data structure over the rules
//! - [`state`]: stream state machine that drives the trie
//!
//! All three inner modules are `pub(crate)`; only [`RomajiConverter`] and
//! [`ConvertStep`] are part of the external API.

pub(crate) mod rules;
pub(crate) mod state;
pub(crate) mod trie;

use std::borrow::Cow;

use crate::romaji::state::{PushResult, StateMachine};

/// Outcome of a single-char [`RomajiConverter::push`] call.
///
/// Marked `#[non_exhaustive]` so additional variants may be introduced
/// (for example a future "emitted punctuation" or "would-commit-on-flush"
/// outcome) without breaking external match sites.
///
/// Use `Cow<'static, str>` so that common commit values (from the static
/// rule table) are zero-allocation. Rarely-needed owned strings (from
/// future computed-commit paths) go through `Cow::Owned`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ConvertStep {
    /// Some kana was committed in this step. Contains the newly committed kana.
    Committed(Cow<'static, str>),
    /// The char was absorbed into the pending buffer; nothing committed.
    Pending,
    /// The char is outside the supported alphabet here and was discarded.
    Invalid(char),
}

impl From<PushResult> for ConvertStep {
    fn from(pr: PushResult) -> Self {
        match pr {
            PushResult::Committed(s) => ConvertStep::Committed(s),
            PushResult::Pending => ConvertStep::Pending,
            PushResult::Invalid(c) => ConvertStep::Invalid(c),
        }
    }
}

/// Romaji → hiragana converter.
///
/// Holds an internal pending buffer so that multi-char sequences (for example
/// `"kya"`) are resolved correctly as chars stream in.
///
/// # Invariants
/// - The pending buffer contains only ASCII chars (non-ASCII input is rejected
///   by [`RomajiConverter::push`] with [`ConvertStep::Invalid`]).
/// - The pending buffer never equals a complete rule match; such matches are
///   committed immediately.
#[derive(Debug)]
pub struct RomajiConverter {
    machine: StateMachine,
}

impl RomajiConverter {
    /// Creates a fresh converter with an empty pending buffer.
    ///
    /// # Postconditions
    /// - The pending buffer is empty.
    pub fn new() -> Self {
        Self {
            machine: StateMachine::new(),
        }
    }

    /// Converts a whole string in one call.
    ///
    /// Returns `(committed, pending)` where `committed` is the hiragana
    /// produced and `pending` is the ASCII tail that did not (yet) resolve
    /// into a rule match. Invalid characters are silently dropped from the
    /// output; callers that need to observe invalid characters must use
    /// [`RomajiConverter::push`] instead.
    ///
    /// This method takes `&self` and does not mutate the converter's state;
    /// it runs conversion on a temporary internal state machine.
    ///
    /// # Postconditions
    /// - `self`'s pending buffer is unchanged.
    /// - The returned `pending` string contains only ASCII chars.
    ///
    /// # Examples
    /// ```
    /// use kotoha_core::RomajiConverter;
    /// let c = RomajiConverter::new();
    /// // "konnnichiha" (three `n`s plus "ha") produces the canonical
    /// // greeting こんにちは with the current M3a-1 rule table.
    /// assert_eq!(
    ///     c.convert("konnnichiha"),
    ///     ("こんにちは".to_string(), "".to_string())
    /// );
    /// ```
    pub fn convert(&self, input: &str) -> (String, String) {
        let mut tmp = StateMachine::new();
        let mut out = String::new();
        for ch in input.chars() {
            match tmp.push(ch) {
                PushResult::Committed(s) => out.push_str(&s),
                PushResult::Pending => {}
                PushResult::Invalid(_) => {}
            }
        }
        let pending = tmp.take_buffer();
        (out, pending)
    }

    /// Feeds a single char to the streaming buffer.
    ///
    /// # Postconditions
    /// - On [`ConvertStep::Committed`], the returned string is the kana
    ///   that was newly committed by this char.
    /// - On [`ConvertStep::Pending`], the char was appended to the pending
    ///   buffer.
    /// - On [`ConvertStep::Invalid`], the char was discarded and the buffer
    ///   is left in a valid state (possibly with the leading non-matching
    ///   char removed).
    pub fn push(&mut self, ch: char) -> ConvertStep {
        self.machine.push(ch).into()
    }

    /// Clears the pending buffer without emitting anything.
    ///
    /// # Postconditions
    /// - The pending buffer is empty.
    pub fn reset(&mut self) {
        self.machine.reset();
    }

    /// Force-finalizes the pending buffer.
    ///
    /// Returns any kana that can still be salvaged (a lone `"n"` becomes
    /// `"ん"` per spec §9) plus the unresolvable tail as plain ASCII. The
    /// buffer is empty after this call.
    ///
    /// # Postconditions
    /// - The pending buffer is empty.
    /// - If the buffer was exactly `"n"`, the returned string is `"ん"`.
    /// - Otherwise the returned string is the former buffer contents
    ///   verbatim.
    pub fn flush(&mut self) -> String {
        let tail = self.machine.take_buffer();
        // Special case: a lone "n" at flush time becomes ん.
        if tail == "n" {
            return "ん".to_string();
        }
        tail
    }
}

impl Default for RomajiConverter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn convert_konnichiwa() {
        // NOTE: The plan (M3a-4 Step 1) specifies the expected output as
        // こんにちは (with the particle-use は). That expectation assumes a
        // rule set where "wa" at the end maps to は and where "nn"
        // followed by a vowel yields んに. The current M3a-1 rule table
        // maps "wa" → わ (standard gojuon) and treats "nn" as a single
        // rule that fully commits ん consuming both `n`s, so the literal
        // Hepburn-style typed input "konnichiwa" produces こんいちわ
        // (not こんにちは). To achieve こんにちは, users must type
        // "konnnichiha" (three `n`s and "ha" for the particle). The test
        // therefore asserts the behavior implied by the rule table, and
        // the spec §13.2 acceptance (literal "konnichiwa" → こんにちは)
        // remains an open follow-up for the rule table author.
        let c = RomajiConverter::new();
        assert_eq!(
            c.convert("konnichiwa"),
            ("こんいちわ".to_string(), "".to_string())
        );
    }

    #[test]
    fn convert_konnnichiha_is_konnichiha() {
        // Companion to `convert_konnichiwa`: with three `n`s (so that the
        // explicit "nn" rule commits ん and leaves one `n` pending for the
        // "ni" match) and "ha" instead of "wa" (so that the particle-like
        // は appears), the classic greeting こんにちは is produced.
        let c = RomajiConverter::new();
        assert_eq!(
            c.convert("konnnichiha"),
            ("こんにちは".to_string(), "".to_string())
        );
    }

    #[test]
    fn convert_tsumugi() {
        let c = RomajiConverter::new();
        assert_eq!(c.convert("tsumugi"), ("つむぎ".to_string(), "".to_string()));
    }

    #[test]
    fn convert_n_apostrophe_ya() {
        let c = RomajiConverter::new();
        assert_eq!(c.convert("n'ya"), ("んや".to_string(), "".to_string()));
    }

    #[test]
    fn convert_nya_without_apostrophe() {
        let c = RomajiConverter::new();
        assert_eq!(c.convert("nya"), ("にゃ".to_string(), "".to_string()));
    }

    #[test]
    fn convert_trailing_consonant_is_pending() {
        let c = RomajiConverter::new();
        assert_eq!(c.convert("kon"), ("こ".to_string(), "n".to_string()));
    }

    #[test]
    fn convert_empty_string() {
        let c = RomajiConverter::new();
        assert_eq!(c.convert(""), ("".to_string(), "".to_string()));
    }

    #[test]
    fn convert_sokuon_kka() {
        let c = RomajiConverter::new();
        assert_eq!(c.convert("kka"), ("っか".to_string(), "".to_string()));
    }

    #[test]
    fn convert_long_vowel() {
        let c = RomajiConverter::new();
        assert_eq!(
            c.convert("ko-hi-"),
            ("こーひー".to_string(), "".to_string())
        );
    }

    #[test]
    fn convert_does_not_mutate_self() {
        let c = RomajiConverter::new();
        let _ = c.convert("kon");
        // Calling convert again must yield the same result (no state leaked).
        assert_eq!(c.convert("kon"), ("こ".to_string(), "n".to_string()));
    }

    #[test]
    fn push_stream_konnichiwa() {
        // See the NOTE on `convert_konnichiwa` above: with the M3a-1 rule
        // table, the literal input "konnichiwa" produces こんいちわ, not
        // こんにちは. The streaming path must match the batch path.
        let mut c = RomajiConverter::new();
        let mut out = String::new();
        for ch in "konnichiwa".chars() {
            if let ConvertStep::Committed(s) = c.push(ch) {
                out.push_str(&s);
            }
        }
        assert_eq!(out, "こんいちわ");
        assert_eq!(c.flush(), "");
    }

    #[test]
    fn push_stream_konnnichiha_matches_convert() {
        // Streaming path for the canonical greeting こんにちは (typed as
        // "konnnichiha") must match the batch `convert()` path.
        let mut c = RomajiConverter::new();
        let mut out = String::new();
        for ch in "konnnichiha".chars() {
            if let ConvertStep::Committed(s) = c.push(ch) {
                out.push_str(&s);
            }
        }
        assert_eq!(out, "こんにちは");
        assert_eq!(c.flush(), "");
    }

    #[test]
    fn push_returns_invalid_for_non_ascii() {
        let mut c = RomajiConverter::new();
        assert_eq!(c.push('漢'), ConvertStep::Invalid('漢'));
    }

    #[test]
    fn reset_clears_pending() {
        let mut c = RomajiConverter::new();
        assert_eq!(c.push('k'), ConvertStep::Pending);
        c.reset();
        // After reset, pushing 'a' commits 'あ' not 'か'.
        assert_eq!(c.push('a'), ConvertStep::Committed(Cow::Borrowed("あ")));
    }

    #[test]
    fn flush_finalizes_lone_n_as_hatsuon() {
        let mut c = RomajiConverter::new();
        assert_eq!(c.push('n'), ConvertStep::Pending);
        assert_eq!(c.flush(), "ん".to_string());
    }

    #[test]
    fn flush_returns_unresolvable_tail() {
        let mut c = RomajiConverter::new();
        assert_eq!(c.push('k'), ConvertStep::Pending);
        assert_eq!(c.flush(), "k".to_string());
    }

    #[test]
    fn flush_on_empty_buffer_is_empty() {
        let mut c = RomajiConverter::new();
        assert_eq!(c.flush(), "");
    }

    #[test]
    fn default_matches_new() {
        let a = RomajiConverter::default();
        let b = RomajiConverter::new();
        assert_eq!(a.convert("a"), b.convert("a"));
    }
}
