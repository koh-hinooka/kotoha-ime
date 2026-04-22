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
