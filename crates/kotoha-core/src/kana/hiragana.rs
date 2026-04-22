//! Hiragana classification and conversion.

/// Returns `true` iff `ch` is a hiragana code point.
///
/// Supported ranges:
/// - `U+3041..=U+3096` (ぁ..ゖ, 86 code points)
/// - `U+309D..=U+309F` (ゝ, ゞ, ゟ)
pub fn is_hiragana(ch: char) -> bool {
    matches!(ch, '\u{3041}'..='\u{3096}' | '\u{309D}'..='\u{309F}')
}

/// Converts hiragana in `s` to katakana. Non-hiragana characters pass through unchanged.
pub fn hiragana_to_katakana(s: &str) -> String {
    s.chars()
        .map(|ch| {
            if is_hiragana(ch) {
                // Hiragana → Katakana: add the fixed +0x60 offset.
                // All 3 covered hiragana ranges land inside katakana ranges after this shift.
                char::from_u32(ch as u32 + 0x60).unwrap_or(ch)
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
    fn is_hiragana_basic_five_vowels() {
        assert!(is_hiragana('あ'));
        assert!(is_hiragana('い'));
        assert!(is_hiragana('う'));
        assert!(is_hiragana('え'));
        assert!(is_hiragana('お'));
    }

    #[test]
    fn is_hiragana_small_forms() {
        assert!(is_hiragana('ぁ'));
        assert!(is_hiragana('ゃ'));
        assert!(is_hiragana('っ'));
    }

    #[test]
    fn is_hiragana_obsolete_forms() {
        assert!(is_hiragana('ゐ'));
        assert!(is_hiragana('ゑ'));
        assert!(is_hiragana('ゖ'));
    }

    #[test]
    fn is_hiragana_repeat_marks() {
        assert!(is_hiragana('ゝ'));
        assert!(is_hiragana('ゞ'));
        assert!(is_hiragana('ゟ'));
    }

    #[test]
    fn is_hiragana_rejects_katakana_and_ascii() {
        assert!(!is_hiragana('ア'));
        assert!(!is_hiragana('カ'));
        assert!(!is_hiragana('a'));
        assert!(!is_hiragana('1'));
        assert!(!is_hiragana('漢'));
        assert!(!is_hiragana(' '));
    }

    #[test]
    fn hiragana_to_katakana_basic() {
        assert_eq!(hiragana_to_katakana("あいうえお"), "アイウエオ");
    }

    #[test]
    fn hiragana_to_katakana_mixed() {
        assert_eq!(hiragana_to_katakana("こんにちは"), "コンニチハ");
    }

    #[test]
    fn hiragana_to_katakana_passes_through_non_hiragana() {
        assert_eq!(hiragana_to_katakana("ABC"), "ABC");
        assert_eq!(hiragana_to_katakana("あA1"), "アA1");
    }

    #[test]
    fn hiragana_to_katakana_empty() {
        assert_eq!(hiragana_to_katakana(""), "");
    }

    #[test]
    fn hiragana_to_katakana_small_forms() {
        assert_eq!(hiragana_to_katakana("ぁっゃゅょ"), "ァッャュョ");
    }
}
