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
pub fn hiragana_to_katakana(_s: &str) -> String {
    unimplemented!("implemented in Task M2-5 Step 3")
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
}
