//! Stream state machine that drives romaji → kana conversion char by char.
//!
//! The state machine owns a small input buffer (ASCII bytes) and consults
//! [`crate::romaji::trie::Trie`] after each pushed char. Special cases that
//! cannot be expressed in the trie (double-consonant sokuon, bare `n` followed
//! by non-vowel → hatsuon) are handled here.

use crate::romaji::trie::{Lookup, Trie};

/// Outcome of feeding a single char to the state machine.
///
/// The enum is marked `#[non_exhaustive]` so additional variants may be
/// introduced without breaking downstream match sites inside the crate.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub(crate) enum PushResult {
    /// Some kana was committed this step. Contains the newly committed kana.
    Committed(String),
    /// The input was absorbed into the pending buffer; nothing committed yet.
    Pending,
    /// The input was rejected (no rule or prefix matches this char here).
    Invalid(char),
}

/// The stream state machine that converts romaji chars to kana.
///
/// # Invariants
/// - `buffer` contains only ASCII bytes (`push` rejects non-ASCII input).
/// - `buffer` never holds a string that is itself a complete trie match;
///   such matches are consumed immediately by [`StateMachine::settle`].
#[derive(Debug)]
pub(crate) struct StateMachine {
    trie: Trie,
    /// Pending input buffer, ASCII bytes only (push rejects non-ASCII).
    buffer: String,
}

impl StateMachine {
    /// Constructs a new state machine with an empty buffer.
    ///
    /// # Postconditions
    /// - `self.buffer()` returns the empty string.
    pub(crate) fn new() -> Self {
        Self {
            trie: Trie::from_rules(),
            buffer: String::new(),
        }
    }

    /// Returns the current unconverted pending buffer.
    ///
    /// Test-only inspection accessor: the public facade in
    /// [`crate::romaji::RomajiConverter`] does not need to read the raw
    /// buffer, so this method is gated to test builds to keep it out of the
    /// production surface area.
    #[cfg(test)]
    pub(crate) fn buffer(&self) -> &str {
        &self.buffer
    }

    /// Clears the pending buffer without emitting anything.
    ///
    /// # Postconditions
    /// - `self.buffer()` returns the empty string.
    pub(crate) fn reset(&mut self) {
        self.buffer.clear();
    }

    /// Drops the pending buffer and returns it as a plain-ASCII string.
    ///
    /// Intended as a fallback when the caller wants whatever could not be
    /// converted.
    ///
    /// # Postconditions
    /// - `self.buffer()` returns the empty string after the call.
    pub(crate) fn take_buffer(&mut self) -> String {
        std::mem::take(&mut self.buffer)
    }

    /// Feed one char to the machine.
    ///
    /// # Preconditions
    /// - Non-ASCII input is rejected with [`PushResult::Invalid`] without
    ///   modifying the buffer.
    pub(crate) fn push(&mut self, ch: char) -> PushResult {
        if !ch.is_ascii() {
            return PushResult::Invalid(ch);
        }
        self.buffer.push(ch);
        self.settle()
    }

    /// Settle as much of the buffer as possible into commits.
    ///
    /// Returns the [`PushResult`] representing the *last* push-visible effect.
    /// At most one visible outcome is returned per call even if multiple
    /// internal commits take place (e.g. "kk" pushed char-by-char: the second
    /// 'k' emits `Committed("っ")` and internally leaves "k" pending).
    ///
    /// # Preconditions
    /// - `self.buffer` is non-empty on entry. The only caller,
    ///   [`StateMachine::push`], enforces this by appending a char to
    ///   `self.buffer` before calling `settle`.
    ///
    /// # Panics
    /// Panics if called with an empty `self.buffer`. The fallback branch
    /// reads the leading char via `chars().next().expect(..)`, which relies
    /// on the precondition above.
    fn settle(&mut self) -> PushResult {
        match self.trie.lookup(&self.buffer) {
            Lookup::Match(kana) => {
                let out = kana.to_string();
                self.buffer.clear();
                PushResult::Committed(out)
            }
            Lookup::Partial => PushResult::Pending,
            Lookup::None => {
                // Backtrack: try special-case rescues first.
                if self.buffer.len() >= 2 {
                    let bytes = self.buffer.as_bytes();
                    let first = bytes[0];
                    let second = bytes[1];

                    // Double-consonant sokuon: e.g. "kk" → commit っ, keep "k".
                    if is_sokuon_consonant(first) && first == second {
                        self.buffer.remove(0);
                        return PushResult::Committed("っ".to_string());
                    }

                    // Bare n followed by non-vowel, non-y, non-n, non-':
                    // commit ん, keep the second char for the next push.
                    if first == b'n' && !is_n_continuation(second) {
                        self.buffer.remove(0);
                        return PushResult::Committed("ん".to_string());
                    }
                }
                // Fallback: drop the leading char as invalid.
                let leading = self.buffer.chars().next().expect("buffer non-empty");
                self.buffer.remove(0);
                PushResult::Invalid(leading)
            }
        }
    }
}

/// Consonants eligible for double-consonant sokuon.
fn is_sokuon_consonant(b: u8) -> bool {
    matches!(
        b,
        b'k' | b'g'
            | b's'
            | b'z'
            | b'j'
            | b't'
            | b'd'
            | b'c'
            | b'h'
            | b'f'
            | b'b'
            | b'p'
            | b'm'
            | b'y'
            | b'r'
            | b'w'
            | b'v'
    )
}

/// Chars that, following bare `n`, should *not* trigger ん commit
/// (the char might still combine with `n` into a trie-recognized form).
fn is_n_continuation(b: u8) -> bool {
    matches!(b, b'a' | b'i' | b'u' | b'e' | b'o' | b'y' | b'n' | b'\'')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_single_vowel_commits_immediately() {
        let mut sm = StateMachine::new();
        assert_eq!(sm.push('a'), PushResult::Committed("あ".to_string()));
        assert_eq!(sm.buffer(), "");
    }

    #[test]
    fn push_k_is_pending_then_a_commits_ka() {
        let mut sm = StateMachine::new();
        assert_eq!(sm.push('k'), PushResult::Pending);
        assert_eq!(sm.buffer(), "k");
        assert_eq!(sm.push('a'), PushResult::Committed("か".to_string()));
        assert_eq!(sm.buffer(), "");
    }

    #[test]
    fn push_kya_three_char_yoon() {
        let mut sm = StateMachine::new();
        assert_eq!(sm.push('k'), PushResult::Pending);
        assert_eq!(sm.push('y'), PushResult::Pending);
        assert_eq!(sm.push('a'), PushResult::Committed("きゃ".to_string()));
    }

    #[test]
    fn push_double_consonant_emits_sokuon() {
        let mut sm = StateMachine::new();
        assert_eq!(sm.push('k'), PushResult::Pending);
        // Second 'k' triggers sokuon: commit っ, keep one 'k' pending.
        assert_eq!(sm.push('k'), PushResult::Committed("っ".to_string()));
        assert_eq!(sm.buffer(), "k");
        assert_eq!(sm.push('a'), PushResult::Committed("か".to_string()));
    }

    #[test]
    fn push_bare_n_then_consonant_emits_hatsuon() {
        let mut sm = StateMachine::new();
        assert_eq!(sm.push('n'), PushResult::Pending);
        // 'k' is not in the n-continuation set and "nk" is not a trie prefix →
        // commit ん, keep 'k'.
        assert_eq!(sm.push('k'), PushResult::Committed("ん".to_string()));
        assert_eq!(sm.buffer(), "k");
    }

    #[test]
    fn push_nn_commits_n_explicitly() {
        let mut sm = StateMachine::new();
        assert_eq!(sm.push('n'), PushResult::Pending);
        // "nn" is an explicit rule → commit ん, buffer empty.
        assert_eq!(sm.push('n'), PushResult::Committed("ん".to_string()));
        assert_eq!(sm.buffer(), "");
    }

    #[test]
    fn push_n_apostrophe_commits_n() {
        let mut sm = StateMachine::new();
        assert_eq!(sm.push('n'), PushResult::Pending);
        // "n'" is an explicit rule → commit ん.
        assert_eq!(sm.push('\''), PushResult::Committed("ん".to_string()));
        assert_eq!(sm.buffer(), "");
    }

    #[test]
    fn push_long_vowel_mark() {
        let mut sm = StateMachine::new();
        assert_eq!(sm.push('-'), PushResult::Committed("ー".to_string()));
    }

    #[test]
    fn push_invalid_ascii_returns_invalid() {
        let mut sm = StateMachine::new();
        // 'Q' is not in any rule and not a prefix → invalid immediately.
        assert_eq!(sm.push('Q'), PushResult::Invalid('Q'));
        assert_eq!(sm.buffer(), "");
    }

    #[test]
    fn push_non_ascii_returns_invalid() {
        let mut sm = StateMachine::new();
        assert_eq!(sm.push('あ'), PushResult::Invalid('あ'));
        assert_eq!(sm.buffer(), "");
    }

    #[test]
    fn reset_clears_buffer() {
        let mut sm = StateMachine::new();
        sm.push('k');
        assert_eq!(sm.buffer(), "k");
        sm.reset();
        assert_eq!(sm.buffer(), "");
    }
}
