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
pub fn katakana_to_hiragana(_s: &str) -> String {
    unimplemented!("implemented in Task M2-5 Step 6")
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
}
