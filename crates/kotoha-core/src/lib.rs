//! kotoha-core: the core library for the Kotoha Japanese IME.
//!
//! Phase 0 scope: error types and kana utilities.
//! Later milestones add romaji conversion (M3) and input mode state machine (M4).

pub mod error;
pub mod input;
pub mod kana;
pub mod kanji;
pub mod romaji;

#[cfg(feature = "dict")]
pub mod dict;

pub use error::{Error, Result};
pub use input::{InputContext, InputMode, InputStep};
pub use kanji::{load_backend, BackendConfig, Candidate, ConvertOptions, KanjiBackend, KanjiError};
pub use romaji::{ConvertStep, RomajiConverter};

/// Test-only accessor for the romaji rule table.
///
/// Returns the raw `(romaji_key, kana_value)` pairs as declared in
/// `romaji::rules::RULES`. Exposed for integration tests that need to
/// assert that the golden fixture (`tests/fixtures/romaji_cases.tsv`)
/// covers every rule key, without leaking the `pub(crate)` visibility
/// of the underlying `romaji::rules` module to downstream consumers.
///
/// # Stability
/// Not part of the stable public API. The `__test_only_` prefix and
/// `#[doc(hidden)]` attribute signal that external crates must not
/// depend on this function; it exists solely for the `kotoha-core`
/// integration test suite.
#[doc(hidden)]
pub fn __test_only_romaji_rules() -> &'static [(&'static str, &'static str)] {
    crate::romaji::rules::RULES
}
