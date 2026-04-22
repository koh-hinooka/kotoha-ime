//! kotoha-core: the core library for the Kotoha Japanese IME.
//!
//! Phase 0 scope: error types and kana utilities.
//! Later milestones add romaji conversion (M3) and input mode state machine (M4).

pub mod error;
pub mod kana;

pub use error::{Error, Result};
