//! Utilities for manipulating Japanese kana characters.
//!
//! The functions in this module classify characters (hiragana vs katakana)
//! and convert between the two scripts using the fixed `0x60` code-point offset
//! between the Hiragana (`U+3040`) and Katakana (`U+30A0`) Unicode blocks.

mod hiragana;
mod katakana;

pub use hiragana::{hiragana_to_katakana, is_hiragana};
pub use katakana::{is_katakana, katakana_to_hiragana};
