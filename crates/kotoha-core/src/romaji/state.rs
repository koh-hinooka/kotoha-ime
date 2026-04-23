//! Stream state machine that drives romaji → kana conversion char by char.
//!
//! The state machine owns a small input buffer (ASCII bytes) and consults
//! [`crate::romaji::trie::Trie`] after each pushed char. Special cases that
//! cannot be expressed in the trie (double-consonant sokuon, bare `n` followed
//! by non-vowel → hatsuon) are handled here.
//!
//! The pending-buffer backtrack rule implemented in [`StateMachine::settle`]
//! (sokuon double-consonant, bare-`n` hatsuon, single-char invalid peel-off)
//! is specified normatively in spec §9.3 (pending バッファの backtrack 規則)
//! at `docs/superpowers/specs/2026-04-22-kotoha-phase-0-design.md`. The
//! source of truth for the contract lives in that spec subsection; this
//! module implements it.

use std::borrow::Cow;
use std::sync::OnceLock;

use crate::romaji::trie::{Lookup, Trie};

/// Returns a reference to the process-wide cached [`Trie`].
///
/// Builds the trie from [`crate::romaji::rules::RULES`] on first call via
/// [`OnceLock::get_or_init`] and returns the same `&'static Trie` on every
/// subsequent call. Thread-safe by `OnceLock`'s documented contract
/// (exactly one initializer wins, other threads block until it completes).
///
/// # Rationale
/// Prior to ISSUE #19, [`StateMachine::new`] called
/// [`Trie::from_rules`] on every construction, which happened once per
/// [`crate::romaji::RomajiConverter::new`] and once per
/// [`crate::romaji::RomajiConverter::convert`] call (the batch path
/// allocates an internal `StateMachine`). The rebuild is μs-order for the
/// current 207-entry rule table but is strictly wasted work and puts
/// allocator pressure on any keystroke-frequency hot path (for example
/// the Phase 3 IBus engine). Caching the trie in a `static OnceLock`
/// eliminates all rebuild cost after the first call.
///
/// # Postconditions
/// - Every call returns a reference that compares equal (by pointer) to
///   every other call's return value within the same process.
fn global_trie() -> &'static Trie {
    static TRIE: OnceLock<Trie> = OnceLock::new();
    TRIE.get_or_init(Trie::from_rules)
}

/// Outcome of feeding a single char to the state machine.
///
/// The enum is marked `#[non_exhaustive]` so additional variants may be
/// introduced without breaking downstream match sites inside the crate.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub(crate) enum PushResult {
    /// Some kana was committed this step. Contains the newly committed kana.
    ///
    /// Uses `Cow<'static, str>` so that common commit values (coming from the
    /// static rule table and the `"っ"` / `"ん"` literals) are zero-allocation.
    /// Rarely-needed owned strings (from future computed-commit paths, should
    /// any appear) would go through `Cow::Owned`.
    Committed(Cow<'static, str>),
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
    /// Reference to the process-wide cached trie obtained via
    /// [`global_trie`]. The trie itself is immutable after
    /// initialization; only the pending buffer mutates per push.
    trie: &'static Trie,
    /// Pending input buffer, ASCII bytes only (push rejects non-ASCII).
    buffer: String,
}

impl StateMachine {
    /// Constructs a new state machine with an empty buffer.
    ///
    /// Takes a reference to the globally cached trie (see [`global_trie`]);
    /// the trie is not rebuilt per construction.
    ///
    /// # Postconditions
    /// - `self.buffer()` returns the empty string.
    pub(crate) fn new() -> Self {
        Self {
            trie: global_trie(),
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
    /// 参照: spec §9.3 pending バッファの backtrack 規則。
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
    ///
    /// # Normative spec
    /// The backtrack rule implemented here (sokuon double-consonant,
    /// bare-`n` hatsuon, single-char invalid peel-off) is specified in
    /// spec §9.3 pending バッファの backtrack 規則
    /// (`docs/superpowers/specs/2026-04-22-kotoha-phase-0-design.md`).
    /// Do not change this function's behavior without also updating §9.3
    /// so the spec and implementation stay in lockstep.
    fn settle(&mut self) -> PushResult {
        match self.trie.lookup(&self.buffer) {
            Lookup::Match(kana) => {
                // `kana` is `&'static str` from the rule table, so the Cow
                // stays Borrowed and no allocation takes place.
                self.buffer.clear();
                PushResult::Committed(Cow::Borrowed(kana))
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
                        return PushResult::Committed(Cow::Borrowed("っ"));
                    }

                    // Bare n followed by non-vowel, non-y, non-n, non-':
                    // commit ん, keep the second char for the next push.
                    if first == b'n' && !is_n_continuation(second) {
                        self.buffer.remove(0);
                        return PushResult::Committed(Cow::Borrowed("ん"));
                    }
                }
                // Fallback: drop the leading char as invalid.
                let leading = self.buffer.chars().next().expect("buffer non-empty");
                self.buffer.remove(0);
                PushResult::Invalid(leading)
            }
        }
    }

    /// Normalizes the pending buffer to a stable form.
    ///
    /// After this call, the buffer is one of:
    /// - Empty (all content was dropped as Invalid or committed).
    /// - A trie partial prefix (stable pending, will accept more input).
    ///
    /// Any commits salvaged during normalization (double-consonant sokuon,
    /// bare-`n` hatsuon, or a completed rule that was hiding in the buffer)
    /// are accumulated into the returned string.
    ///
    /// # Postconditions
    /// - `self.buffer()` is either empty or a trie partial prefix.
    /// - The returned `String` contains any salvaged kana, in order.
    ///
    /// # Rationale
    /// [`Self::settle`] stops after one visible effect per call, so a
    /// non-matching buffer of length ≥ 2 may be left with a leading char
    /// dropped but the rest unchanged (e.g. "byb" → "yb"). That residual
    /// "yb" is itself non-matching and would be further reduced by a fresh
    /// [`Self::settle`] call. Without this normalization, the pending
    /// buffer returned by [`crate::romaji::RomajiConverter::convert`]
    /// fails to be idempotent under re-conversion, breaking associativity.
    pub(crate) fn normalize(&mut self) -> String {
        let mut committed = String::new();
        loop {
            if self.buffer.is_empty() {
                break;
            }
            match self.trie.lookup(&self.buffer) {
                Lookup::Match(kana) => {
                    committed.push_str(kana);
                    self.buffer.clear();
                    break;
                }
                Lookup::Partial => break,
                Lookup::None => {
                    if self.buffer.len() >= 2 {
                        let bytes = self.buffer.as_bytes();
                        let first = bytes[0];
                        let second = bytes[1];
                        if is_sokuon_consonant(first) && first == second {
                            self.buffer.remove(0);
                            committed.push('っ');
                            continue;
                        }
                        if first == b'n' && !is_n_continuation(second) {
                            self.buffer.remove(0);
                            committed.push('ん');
                            continue;
                        }
                    }
                    // Drop leading invalid char and retry.
                    self.buffer.remove(0);
                }
            }
        }
        committed
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
        assert_eq!(sm.push('a'), PushResult::Committed(Cow::Borrowed("あ")));
        assert_eq!(sm.buffer(), "");
    }

    #[test]
    fn push_k_is_pending_then_a_commits_ka() {
        let mut sm = StateMachine::new();
        assert_eq!(sm.push('k'), PushResult::Pending);
        assert_eq!(sm.buffer(), "k");
        assert_eq!(sm.push('a'), PushResult::Committed(Cow::Borrowed("か")));
        assert_eq!(sm.buffer(), "");
    }

    #[test]
    fn push_kya_three_char_yoon() {
        let mut sm = StateMachine::new();
        assert_eq!(sm.push('k'), PushResult::Pending);
        assert_eq!(sm.push('y'), PushResult::Pending);
        assert_eq!(sm.push('a'), PushResult::Committed(Cow::Borrowed("きゃ")));
    }

    #[test]
    fn push_double_consonant_emits_sokuon() {
        let mut sm = StateMachine::new();
        assert_eq!(sm.push('k'), PushResult::Pending);
        // Second 'k' triggers sokuon: commit っ, keep one 'k' pending.
        assert_eq!(sm.push('k'), PushResult::Committed(Cow::Borrowed("っ")));
        assert_eq!(sm.buffer(), "k");
        assert_eq!(sm.push('a'), PushResult::Committed(Cow::Borrowed("か")));
    }

    #[test]
    fn push_bare_n_then_consonant_emits_hatsuon() {
        let mut sm = StateMachine::new();
        assert_eq!(sm.push('n'), PushResult::Pending);
        // 'k' is not in the n-continuation set and "nk" is not a trie prefix →
        // commit ん, keep 'k'.
        assert_eq!(sm.push('k'), PushResult::Committed(Cow::Borrowed("ん")));
        assert_eq!(sm.buffer(), "k");
    }

    #[test]
    fn push_nn_commits_n_explicitly() {
        let mut sm = StateMachine::new();
        assert_eq!(sm.push('n'), PushResult::Pending);
        // "nn" is an explicit rule → commit ん, buffer empty.
        assert_eq!(sm.push('n'), PushResult::Committed(Cow::Borrowed("ん")));
        assert_eq!(sm.buffer(), "");
    }

    #[test]
    fn push_n_apostrophe_commits_n() {
        let mut sm = StateMachine::new();
        assert_eq!(sm.push('n'), PushResult::Pending);
        // "n'" is an explicit rule → commit ん.
        assert_eq!(sm.push('\''), PushResult::Committed(Cow::Borrowed("ん")));
        assert_eq!(sm.buffer(), "");
    }

    #[test]
    fn push_long_vowel_mark() {
        let mut sm = StateMachine::new();
        assert_eq!(sm.push('-'), PushResult::Committed(Cow::Borrowed("ー")));
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

    #[test]
    fn normalize_stabilizes_partial_then_invalid_sequence() {
        // "byb" leaves "yb" in buffer after the Invalid drop of leading 'b'.
        // normalize() should further reduce "yb" to "b" (valid partial prefix).
        let mut sm = StateMachine::new();
        sm.push('b'); // pending "b"
        sm.push('y'); // pending "by"
        let _ = sm.push('b'); // Invalid drop of leading 'b', buffer = "yb"
        assert_eq!(sm.buffer(), "yb");
        let committed = sm.normalize();
        assert_eq!(committed, "", "no kana should be salvaged from yb");
        assert_eq!(sm.buffer(), "b", "yb should reduce to b after normalize");
    }

    #[test]
    fn normalize_is_noop_after_settle_already_emitted_sokuon() {
        // This test verifies that `normalize()` is a no-op on a trie-partial
        // buffer ("k") AFTER `settle()` has already emitted the "っ" via
        // the sokuon branch of push(). It does NOT exercise normalize's own
        // sokuon salvage branch; that branch is covered indirectly via the
        // convert-level regression tests in mod.rs (e.g. convert("byb")).
        let mut sm = StateMachine::new();
        sm.push('k'); // pending "k"
        let result = sm.push('k'); // settle emits っ via sokuon, buffer="k"
        assert_eq!(
            result,
            PushResult::Committed(std::borrow::Cow::Borrowed("っ"))
        );
        assert_eq!(sm.buffer(), "k");
        // normalize on the remaining "k" sees a Partial prefix → no commit,
        // no buffer change.
        let committed = sm.normalize();
        assert_eq!(committed, "");
        assert_eq!(sm.buffer(), "k");
    }

    #[test]
    fn normalize_on_empty_buffer_is_noop() {
        let mut sm = StateMachine::new();
        let committed = sm.normalize();
        assert_eq!(committed, "");
        assert_eq!(sm.buffer(), "");
    }

    #[test]
    fn normalize_on_partial_prefix_is_noop() {
        let mut sm = StateMachine::new();
        sm.push('k');
        sm.push('y'); // buffer = "ky", a partial prefix
        let committed = sm.normalize();
        assert_eq!(committed, "");
        assert_eq!(sm.buffer(), "ky");
    }

    #[test]
    fn normalize_drops_chain_of_invalid_chars() {
        // Push chars that form a multi-step invalid chain.
        // Start with "ky", then push 'k' which leaves "yk" after the Invalid drop
        // (settle's fallback drops the leading char once). normalize should
        // further drop 'y' (since "yk" is still invalid with k at tail),
        // leaving "k" as a valid partial prefix.
        let mut sm = StateMachine::new();
        sm.push('k'); // "k"
        sm.push('y'); // "ky"
        let _ = sm.push('k'); // settle drops leading 'k', buffer = "yk"
        assert_eq!(sm.buffer(), "yk");
        let committed = sm.normalize();
        assert_eq!(committed, "", "no kana salvaged from yk");
        assert_eq!(sm.buffer(), "k", "yk should reduce to k");
    }

    #[test]
    fn global_trie_returns_same_instance_across_calls() {
        // Regression for ISSUE #19: global_trie() must return the cached
        // &'static Trie, never a freshly built one. Pointer equality is the
        // strongest observable signal of cache behavior — a rebuilt Trie
        // would live at a different address.
        let ptr1 = std::ptr::from_ref(super::global_trie());
        let ptr2 = std::ptr::from_ref(super::global_trie());
        assert_eq!(
            ptr1, ptr2,
            "global_trie() must return the cached &'static Trie, not a rebuilt one"
        );
    }

    #[test]
    fn state_machine_new_shares_trie_with_global_cache() {
        // Companion to the above: construct two StateMachines and confirm
        // both hold the same &'static Trie as global_trie(). Guards against
        // a future regression where StateMachine::new() accidentally falls
        // back to per-call construction.
        let sm1 = StateMachine::new();
        let sm2 = StateMachine::new();
        let global_ptr = std::ptr::from_ref(super::global_trie());
        assert_eq!(std::ptr::from_ref(sm1.trie), global_ptr);
        assert_eq!(std::ptr::from_ref(sm2.trie), global_ptr);
    }
}
