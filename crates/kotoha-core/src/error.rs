//! Error types for kotoha-core.

use thiserror::Error;

/// The error type returned from kotoha-core operations.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum Error {
    /// A character outside the supported conversion rules was encountered.
    #[error("invalid character: {0:?}")]
    InvalidCharacter(char),

    /// The internal state machine reached an impossible configuration. Likely a bug.
    #[error("internal state inconsistency")]
    InvalidState,
}

/// Convenience alias: `Result<T, kotoha_core::Error>`.
pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_character_display_contains_char() {
        let err = Error::InvalidCharacter('Q');
        let msg = format!("{}", err);
        assert!(msg.contains('Q'), "display should include the char: {msg}");
    }

    #[test]
    fn invalid_state_display_is_stable() {
        let err = Error::InvalidState;
        let msg = format!("{}", err);
        assert_eq!(msg, "internal state inconsistency");
    }

    #[test]
    fn result_alias_matches_std() {
        fn returns_err() -> Result<()> {
            Err(Error::InvalidState)
        }
        assert!(returns_err().is_err());
    }
}
