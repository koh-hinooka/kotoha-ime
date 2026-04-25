//! Validation module(spec §9)。

use crate::error::StorageError;

// ====================
// 判定 helper(§9.1.1)
// ====================

/// PUA(Private Use Area)を reject する判定。Phase 5 Mixed JP/EN allowlist
/// (U+EE00..=U+EE03) は Karukan 互換の特例として許容する(spec §9.1.1 / §9.1.2)。
fn is_disallowed_pua(c: char) -> bool {
    let cp = c as u32;
    let is_pua = (0xE000..=0xF8FF).contains(&cp) || (0xF0000..=0x10FFFD).contains(&cp);
    let is_phase5_allowlist = (0xEE00..=0xEE03).contains(&cp);
    is_pua && !is_phase5_allowlist
}

fn is_variation_selector(c: char) -> bool {
    (0xFE00..=0xFE0F).contains(&(c as u32))
}

fn is_tag_char(c: char) -> bool {
    (0xE0000..=0xE007F).contains(&(c as u32))
}

fn is_bidi_char(c: char) -> bool {
    let cp = c as u32;
    cp == 0x200E
        || cp == 0x200F
        || (0x202A..=0x202E).contains(&cp)
        || (0x2066..=0x2069).contains(&cp)
}

fn is_hiragana_or_long_sound(c: char) -> bool {
    let cp = c as u32;
    (0x3040..=0x309F).contains(&cp) || cp == 0x30FC || cp == 0x30FB
}

/// `name` field の共通制約を検査する(spec §9.1)。
pub fn validate_field(name: &str, value: &str) -> Result<(), StorageError> {
    if value.is_empty() {
        return Err(StorageError::InvalidField {
            name: name.to_string(),
            reason: "empty".to_string(),
        });
    }
    if value.len() > 256 {
        return Err(StorageError::InvalidField {
            name: name.to_string(),
            reason: "byte size > 256".to_string(),
        });
    }
    for c in value.chars() {
        if c.is_control() {
            return Err(StorageError::InvalidField {
                name: name.to_string(),
                reason: "control char".to_string(),
            });
        }
        if is_bidi_char(c) {
            return Err(StorageError::InvalidField {
                name: name.to_string(),
                reason: "bidi character".to_string(),
            });
        }
        if is_disallowed_pua(c) {
            return Err(StorageError::InvalidField {
                name: name.to_string(),
                reason: "PUA char".to_string(),
            });
        }
        if is_variation_selector(c) {
            return Err(StorageError::InvalidField {
                name: name.to_string(),
                reason: "Variation Selector".to_string(),
            });
        }
        if is_tag_char(c) {
            return Err(StorageError::InvalidField {
                name: name.to_string(),
                reason: "Tag char".to_string(),
            });
        }
    }
    Ok(())
}

/// `reading` field の制約を検査する(spec §9.2)。
///
/// 共通制約 + hiragana-only(U+3040..=U+309F + U+30FC + U+30FB)。
pub fn validate_reading(value: &str) -> Result<(), StorageError> {
    validate_field("reading", value)?;
    for c in value.chars() {
        if !is_hiragana_or_long_sound(c) {
            return Err(StorageError::InvalidField {
                name: "reading".to_string(),
                reason: "non-hiragana reading".to_string(),
            });
        }
    }
    Ok(())
}

/// `score` field の制約を検査する(spec §9.3)。
pub fn validate_score(value: f32) -> Result<(), StorageError> {
    if !value.is_finite() {
        return Err(StorageError::InvalidField {
            name: "score".to_string(),
            reason: "score not finite".to_string(),
        });
    }
    if value < 0.0 {
        return Err(StorageError::InvalidField {
            name: "score".to_string(),
            reason: "score negative".to_string(),
        });
    }
    Ok(())
}

/// `surface` 用の wrapper。spec §9.1 の共通制約のみを適用する(hiragana 制約は無し)。
pub fn validate_surface(value: &str) -> Result<(), StorageError> {
    validate_field("surface", value)
}

/// `pos` 用の wrapper。spec §9.1 の共通制約のみを適用する。
pub fn validate_pos(value: &str) -> Result<(), StorageError> {
    validate_field("pos", value)
}

#[cfg(test)]
mod tests {
    use super::*;

    // ====================
    // validate_field 共通制約(§9.1)
    // ====================

    #[test]
    fn validate_field_rejects_empty() {
        let err = validate_field("surface", "").unwrap_err();
        match err {
            StorageError::InvalidField { name, reason } => {
                assert_eq!(name, "surface");
                assert_eq!(reason, "empty");
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn validate_field_rejects_byte_size_over_256() {
        let big = "あ".repeat(100); // 100*3 = 300 bytes
        let err = validate_field("surface", &big).unwrap_err();
        match err {
            StorageError::InvalidField { reason, .. } => {
                assert_eq!(reason, "byte size > 256");
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn validate_field_rejects_control_char_nul() {
        let err = validate_field("surface", "abc\0def").unwrap_err();
        match err {
            StorageError::InvalidField { reason, .. } => {
                assert_eq!(reason, "control char");
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn validate_field_rejects_control_char_tab() {
        let err = validate_field("surface", "abc\tdef").unwrap_err();
        assert!(matches!(err, StorageError::InvalidField { .. }));
    }

    #[test]
    fn validate_field_rejects_bidi_chars() {
        let err = validate_field("surface", "ab\u{202E}cd").unwrap_err();
        match err {
            StorageError::InvalidField { reason, .. } => {
                assert_eq!(reason, "bidi character");
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn validate_field_rejects_pua_char() {
        let err = validate_field("surface", "ab\u{E000}cd").unwrap_err();
        match err {
            StorageError::InvalidField { reason, .. } => {
                assert_eq!(reason, "PUA char");
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn validate_field_allows_phase5_pua_allowlist() {
        // U+EE00..=U+EE03 は Karukan 互換の例外として許容(spec §9.1.1)
        assert!(validate_field("surface", "ab\u{EE00}cd").is_ok());
        assert!(validate_field("surface", "ab\u{EE03}cd").is_ok());
    }

    #[test]
    fn validate_field_rejects_variation_selector() {
        let err = validate_field("surface", "ab\u{FE0F}cd").unwrap_err();
        match err {
            StorageError::InvalidField { reason, .. } => {
                assert_eq!(reason, "Variation Selector");
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn validate_field_rejects_tag_char() {
        let err = validate_field("surface", "ab\u{E0001}cd").unwrap_err();
        match err {
            StorageError::InvalidField { reason, .. } => {
                assert_eq!(reason, "Tag char");
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn validate_field_accepts_normal_kanji() {
        assert!(validate_field("surface", "日野岡").is_ok());
        assert!(validate_field("surface", "abc").is_ok());
    }

    // ====================
    // validate_reading(§9.2)
    // ====================

    #[test]
    fn validate_reading_accepts_pure_hiragana() {
        assert!(validate_reading("ひのおか").is_ok());
        assert!(validate_reading("あいうえお").is_ok());
    }

    #[test]
    fn validate_reading_accepts_long_sound_mark() {
        assert!(validate_reading("こーひー").is_ok()); // U+30FC
    }

    #[test]
    fn validate_reading_accepts_middle_dot() {
        assert!(validate_reading("あ・い").is_ok()); // U+30FB
    }

    #[test]
    fn validate_reading_rejects_katakana() {
        let err = validate_reading("カタカナ").unwrap_err();
        match err {
            StorageError::InvalidField { reason, .. } => {
                assert_eq!(reason, "non-hiragana reading");
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn validate_reading_rejects_kanji() {
        let err = validate_reading("漢字").unwrap_err();
        assert!(matches!(err, StorageError::InvalidField { .. }));
    }

    #[test]
    fn validate_reading_rejects_ascii() {
        let err = validate_reading("abc").unwrap_err();
        assert!(matches!(err, StorageError::InvalidField { .. }));
    }

    // ====================
    // validate_score(§9.3)
    // ====================

    #[test]
    fn validate_score_accepts_finite_non_negative() {
        assert!(validate_score(0.0).is_ok());
        assert!(validate_score(1.0).is_ok());
        assert!(validate_score(100.5).is_ok());
    }

    #[test]
    fn validate_score_rejects_nan() {
        let err = validate_score(f32::NAN).unwrap_err();
        match err {
            StorageError::InvalidField { reason, .. } => {
                assert_eq!(reason, "score not finite");
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn validate_score_rejects_infinity() {
        let err = validate_score(f32::INFINITY).unwrap_err();
        assert!(matches!(err, StorageError::InvalidField { .. }));
    }

    #[test]
    fn validate_score_rejects_negative_infinity() {
        let err = validate_score(f32::NEG_INFINITY).unwrap_err();
        assert!(matches!(err, StorageError::InvalidField { .. }));
    }

    #[test]
    fn validate_score_rejects_negative() {
        let err = validate_score(-0.001).unwrap_err();
        match err {
            StorageError::InvalidField { reason, .. } => {
                assert_eq!(reason, "score negative");
            }
            other => panic!("unexpected: {other:?}"),
        }
    }
}
