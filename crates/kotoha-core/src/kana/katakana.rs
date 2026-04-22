//! Katakana classification and conversion.

/// Returns `true` iff `ch` is a katakana code point.
///
/// Supported ranges:
/// - `U+30A1..=U+30FA` (ァ..ヺ, 90 code points)
/// - `U+30FC` (ー, prolonged sound mark)
/// - `U+30FD..=U+30FF` (ヽ, ヾ, ヿ)
pub fn is_katakana(ch: char) -> bool {
    matches!(
        ch,
        '\u{30A1}'..='\u{30FA}' | '\u{30FC}' | '\u{30FD}'..='\u{30FF}'
    )
}

/// Converts katakana in `s` to hiragana. Non-katakana characters pass through unchanged.
/// Note: `ヷ`, `ヸ`, `ヹ`, `ヺ` (U+30F7..=U+30FA) and `ー` (U+30FC) have no hiragana counterpart
/// and pass through unchanged.
pub fn katakana_to_hiragana(s: &str) -> String {
    s.chars()
        .map(|ch| {
            // Convert only katakana that have a hiragana counterpart:
            // - U+30A1..=U+30F6 maps to U+3041..=U+3096
            // - U+30FD..=U+30FF maps to U+309D..=U+309F
            // U+30F7..=U+30FA (ヷヸヹヺ) and U+30FC (ー) have no hiragana peer and pass through.
            if matches!(ch, '\u{30A1}'..='\u{30F6}' | '\u{30FD}'..='\u{30FF}') {
                char::from_u32(ch as u32 - 0x60).unwrap_or(ch)
            } else {
                ch
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_katakana_basic_five_vowels() {
        assert!(is_katakana('ア'));
        assert!(is_katakana('イ'));
        assert!(is_katakana('ウ'));
        assert!(is_katakana('エ'));
        assert!(is_katakana('オ'));
    }

    #[test]
    fn is_katakana_small_forms_and_prolonged_mark() {
        assert!(is_katakana('ァ'));
        assert!(is_katakana('ャ'));
        assert!(is_katakana('ッ'));
        assert!(is_katakana('ー'));
    }

    #[test]
    fn is_katakana_v_row_and_repeat_marks() {
        assert!(is_katakana('ヴ'));
        assert!(is_katakana('ヶ'));
        assert!(is_katakana('ヽ'));
        assert!(is_katakana('ヾ'));
        assert!(is_katakana('ヿ'));
    }

    #[test]
    fn is_katakana_rejects_hiragana_and_ascii() {
        assert!(!is_katakana('あ'));
        assert!(!is_katakana('か'));
        assert!(!is_katakana('a'));
        assert!(!is_katakana('1'));
        assert!(!is_katakana('漢'));
    }

    #[test]
    fn katakana_to_hiragana_basic() {
        assert_eq!(katakana_to_hiragana("アイウエオ"), "あいうえお");
    }

    #[test]
    fn katakana_to_hiragana_mixed() {
        assert_eq!(katakana_to_hiragana("コンニチハ"), "こんにちは");
    }

    #[test]
    fn katakana_to_hiragana_keeps_prolonged_mark() {
        // U+30FC has no hiragana counterpart and must pass through unchanged.
        assert_eq!(katakana_to_hiragana("コーヒー"), "こーひー");
    }

    #[test]
    fn katakana_to_hiragana_keeps_v_row_without_counterpart() {
        // U+30F7..=U+30FA have no hiragana counterpart and must pass through unchanged.
        assert_eq!(katakana_to_hiragana("ヷヸヹヺ"), "ヷヸヹヺ");
    }

    #[test]
    fn katakana_to_hiragana_handles_v_with_counterpart() {
        // ヴ U+30F4 → ゔ U+3094 (within the convertible range).
        assert_eq!(katakana_to_hiragana("ヴ"), "ゔ");
    }

    #[test]
    fn katakana_to_hiragana_empty() {
        assert_eq!(katakana_to_hiragana(""), "");
    }
}
