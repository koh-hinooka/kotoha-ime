//! Static romaji-to-kana conversion rules.
//!
//! Each entry maps a romaji sequence to a kana sequence.
//! Entries are grouped by category (gojuon, dakuten, handakuten, yoon, small kana, ...)
//! for readability. The consumer ([`crate::romaji::trie`]) indexes the entries into a
//! prefix trie, so the in-source order does not affect lookup semantics: the trie's
//! `lookup` method already returns the longest matching key at a given node by
//! distinguishing terminal matches from proper-prefix states.
//!
//! The table is derived from the Karukan project's rules (MIT/Apache-2.0 licensed)
//! with the Shift-key-specific rules omitted. Kotoha handles Shift via the
//! separate `InputContext` state machine (M4), not via romaji rules.

/// Romaji → kana rule entries.
///
/// # Invariants
/// - Every key is a non-empty ASCII byte string (enforced by
///   `rules_keys_are_all_ascii`).
/// - Keys are unique across the table (enforced by `rules_keys_are_unique`).
/// - Every value character is drawn from one of: the hiragana block
///   (U+3041..=U+309F), the katakana-block long-vowel mark ー (U+30FC)
///   and separator ・ (U+30FB), CJK Symbols and Punctuation
///   (U+3000..=U+303F, e.g. 、。「」), the fullwidth ASCII punctuation
///   subset (U+FF01..=U+FF5E, e.g. ！？), or the two ASCII passthroughs
///   `!` (U+0021) and `?` (U+003F) that the current table preserves in
///   their halfwidth form. This is enforced by
///   `rules_values_are_valid_kana_or_punctuation`.
pub(crate) const RULES: &[(&str, &str)] = &[
    // ----- basic gojuon (5 vowels + 45 CV + n) -----
    ("a", "あ"),
    ("i", "い"),
    ("u", "う"),
    ("e", "え"),
    ("o", "お"),
    ("ka", "か"),
    ("ki", "き"),
    ("ku", "く"),
    ("ke", "け"),
    ("ko", "こ"),
    ("sa", "さ"),
    ("si", "し"),
    ("su", "す"),
    ("se", "せ"),
    ("so", "そ"),
    ("shi", "し"),
    ("ta", "た"),
    ("ti", "ち"),
    ("tu", "つ"),
    ("te", "て"),
    ("to", "と"),
    ("chi", "ち"),
    ("tsu", "つ"),
    ("na", "な"),
    ("ni", "に"),
    ("nu", "ぬ"),
    ("ne", "ね"),
    ("no", "の"),
    ("ha", "は"),
    ("hi", "ひ"),
    ("hu", "ふ"),
    ("he", "へ"),
    ("ho", "ほ"),
    ("fu", "ふ"),
    ("ma", "ま"),
    ("mi", "み"),
    ("mu", "む"),
    ("me", "め"),
    ("mo", "も"),
    ("ya", "や"),
    ("yu", "ゆ"),
    ("yo", "よ"),
    ("ra", "ら"),
    ("ri", "り"),
    ("ru", "る"),
    ("re", "れ"),
    ("ro", "ろ"),
    ("wa", "わ"),
    ("wo", "を"),
    ("wi", "ゐ"),
    ("we", "ゑ"),
    // ----- dakuten (voiced) -----
    ("ga", "が"),
    ("gi", "ぎ"),
    ("gu", "ぐ"),
    ("ge", "げ"),
    ("go", "ご"),
    ("za", "ざ"),
    ("zi", "じ"),
    ("zu", "ず"),
    ("ze", "ぜ"),
    ("zo", "ぞ"),
    ("ji", "じ"),
    ("da", "だ"),
    ("di", "ぢ"),
    ("du", "づ"),
    ("de", "で"),
    ("do", "ど"),
    ("ba", "ば"),
    ("bi", "び"),
    ("bu", "ぶ"),
    ("be", "べ"),
    ("bo", "ぼ"),
    // ----- handakuten (half-voiced) -----
    ("pa", "ぱ"),
    ("pi", "ぴ"),
    ("pu", "ぷ"),
    ("pe", "ぺ"),
    ("po", "ぽ"),
    // ----- yoon (contracted) — 3-char forms -----
    ("kya", "きゃ"),
    ("kyi", "きぃ"),
    ("kyu", "きゅ"),
    ("kye", "きぇ"),
    ("kyo", "きょ"),
    ("sya", "しゃ"),
    ("syi", "しぃ"),
    ("syu", "しゅ"),
    ("sye", "しぇ"),
    ("syo", "しょ"),
    ("sha", "しゃ"),
    ("shu", "しゅ"),
    ("she", "しぇ"),
    ("sho", "しょ"),
    ("tya", "ちゃ"),
    ("tyi", "ちぃ"),
    ("tyu", "ちゅ"),
    ("tye", "ちぇ"),
    ("tyo", "ちょ"),
    ("cha", "ちゃ"),
    ("chu", "ちゅ"),
    ("che", "ちぇ"),
    ("cho", "ちょ"),
    ("nya", "にゃ"),
    ("nyi", "にぃ"),
    ("nyu", "にゅ"),
    ("nye", "にぇ"),
    ("nyo", "にょ"),
    ("hya", "ひゃ"),
    ("hyi", "ひぃ"),
    ("hyu", "ひゅ"),
    ("hye", "ひぇ"),
    ("hyo", "ひょ"),
    ("mya", "みゃ"),
    ("myi", "みぃ"),
    ("myu", "みゅ"),
    ("mye", "みぇ"),
    ("myo", "みょ"),
    ("rya", "りゃ"),
    ("ryi", "りぃ"),
    ("ryu", "りゅ"),
    ("rye", "りぇ"),
    ("ryo", "りょ"),
    ("gya", "ぎゃ"),
    ("gyi", "ぎぃ"),
    ("gyu", "ぎゅ"),
    ("gye", "ぎぇ"),
    ("gyo", "ぎょ"),
    ("zya", "じゃ"),
    ("zyi", "じぃ"),
    ("zyu", "じゅ"),
    ("zye", "じぇ"),
    ("zyo", "じょ"),
    ("ja", "じゃ"),
    ("ju", "じゅ"),
    ("je", "じぇ"),
    ("jo", "じょ"),
    // waapuro hybrid (j + y*): supported by MS-IME / Google IME / Mozc.
    // Bridges Hepburn-`j` and kunrei-`y` conventions for typists who mix them.
    ("jya", "じゃ"),
    ("jyi", "じぃ"),
    ("jyu", "じゅ"),
    ("jye", "じぇ"),
    ("jyo", "じょ"),
    // waapuro c-row yoon (alternative to t-row 拗音 chi-glide):
    // accepted by Google IME / Mozc as a parallel form to cha/chu/cho.
    ("cya", "ちゃ"),
    ("cyi", "ちぃ"),
    ("cyu", "ちゅ"),
    ("cye", "ちぇ"),
    ("cyo", "ちょ"),
    ("dya", "ぢゃ"),
    ("dyi", "ぢぃ"),
    ("dyu", "ぢゅ"),
    ("dye", "ぢぇ"),
    ("dyo", "ぢょ"),
    ("bya", "びゃ"),
    ("byi", "びぃ"),
    ("byu", "びゅ"),
    ("bye", "びぇ"),
    ("byo", "びょ"),
    ("pya", "ぴゃ"),
    ("pyi", "ぴぃ"),
    ("pyu", "ぴゅ"),
    ("pye", "ぴぇ"),
    ("pyo", "ぴょ"),
    // ----- extended sounds -----
    ("fa", "ふぁ"),
    ("fi", "ふぃ"),
    ("fe", "ふぇ"),
    ("fo", "ふぉ"),
    // Hepburn-extended f-row yoon (loanword 拡張): フュージョン etc.
    ("fya", "ふゃ"),
    ("fyu", "ふゅ"),
    ("fyo", "ふょ"),
    // Loanword "th-" (English) → て + small-vowel: ティーチ / テューバ etc.
    // Convention adopted by MS-IME / Google IME / Mozc.
    ("tha", "てゃ"),
    ("thi", "てぃ"),
    ("thu", "てゅ"),
    ("the", "てぇ"),
    ("tho", "てょ"),
    // Loanword "dh-" (English voiced) → で + small-vowel: ディスク etc.
    ("dha", "でゃ"),
    ("dhi", "でぃ"),
    ("dhu", "でゅ"),
    ("dhe", "でぇ"),
    ("dho", "でょ"),
    ("va", "ゔぁ"),
    ("vi", "ゔぃ"),
    ("vu", "ゔ"),
    ("ve", "ゔぇ"),
    ("vo", "ゔぉ"),
    ("tsa", "つぁ"),
    ("tsi", "つぃ"),
    ("tse", "つぇ"),
    ("tso", "つぉ"),
    ("kwa", "くぁ"),
    ("kwi", "くぃ"),
    ("kwe", "くぇ"),
    ("kwo", "くぉ"),
    ("gwa", "ぐぁ"),
    ("gwi", "ぐぃ"),
    ("gwe", "ぐぇ"),
    ("gwo", "ぐぉ"),
    ("wha", "うぁ"),
    ("whi", "うぃ"),
    ("whe", "うぇ"),
    ("who", "うぉ"),
    // ----- small-form explicit escapes -----
    ("la", "ぁ"),
    ("li", "ぃ"),
    ("lu", "ぅ"),
    ("le", "ぇ"),
    ("lo", "ぉ"),
    ("xa", "ぁ"),
    ("xi", "ぃ"),
    ("xu", "ぅ"),
    ("xe", "ぇ"),
    ("xo", "ぉ"),
    ("lya", "ゃ"),
    ("lyu", "ゅ"),
    ("lyo", "ょ"),
    ("xya", "ゃ"),
    ("xyu", "ゅ"),
    ("xyo", "ょ"),
    ("ltu", "っ"),
    ("xtu", "っ"),
    ("ltsu", "っ"),
    ("xtsu", "っ"),
    ("lwa", "ゎ"),
    ("xwa", "ゎ"),
    // ----- n-row special -----
    // "nn" is always ん (explicit), "n'" is also always ん (apostrophe escape).
    // Bare "n" is left pending and resolved by the state machine (see state.rs).
    ("nn", "ん"),
    ("n'", "ん"),
    ("xn", "ん"),
    // ----- symbols and punctuation -----
    ("-", "ー"),
    (",", "、"),
    (".", "。"),
    ("?", "?"),
    ("!", "!"),
    ("[", "「"),
    ("]", "」"),
    ("/", "・"),
];

#[cfg(test)]
mod tests {
    use super::RULES;
    use std::collections::HashSet;

    /// Invariant: every rule key must be pure ASCII (the state machine
    /// guards on this at the input layer; here we enforce it at build time).
    #[test]
    fn rules_keys_are_all_ascii() {
        for (key, _) in RULES.iter() {
            assert!(!key.is_empty(), "rule with empty key is not allowed");
            assert!(
                key.is_ascii(),
                "rule key {:?} contains non-ASCII characters",
                key
            );
        }
    }

    /// Invariant: rule keys are unique. Duplicates would silently overwrite
    /// each other in the trie, masking the earlier entry.
    #[test]
    fn rules_keys_are_unique() {
        let keys: HashSet<&str> = RULES.iter().map(|(k, _)| *k).collect();
        assert_eq!(
            keys.len(),
            RULES.len(),
            "duplicate rule key detected (RULES has {} entries but {} unique keys)",
            RULES.len(),
            keys.len()
        );
    }

    /// Invariant: every rule value contains only characters from one of:
    /// - Hiragana block: U+3041..=U+309F
    /// - Katakana block punctuation: U+30FB (・), and the long-vowel mark U+30FC (ー)
    /// - CJK Symbols and Punctuation: U+3000..=U+303F (、。「」etc.)
    /// - Fullwidth ASCII punctuation subset: U+FF01..=U+FF5E (！？etc.)
    /// - ASCII punctuation retained as-is: `!` (U+0021) and `?` (U+003F)
    ///
    /// Phase 0 does not emit katakana kana themselves, only the `ー` long-vowel
    /// mark and `・` separator from the katakana block. The two ASCII
    /// punctuation passthroughs (`!` and `?`) match the existing rule table,
    /// which intentionally preserves halfwidth `!?` rather than converting to
    /// their fullwidth forms.
    #[test]
    fn rules_values_are_valid_kana_or_punctuation() {
        for (key, value) in RULES.iter() {
            for ch in value.chars() {
                let code = ch as u32;
                let in_hiragana = (0x3041..=0x309F).contains(&code);
                let is_long_mark_or_dot = code == 0x30FC || code == 0x30FB;
                let is_cjk_punctuation = (0x3000..=0x303F).contains(&code);
                let is_fullwidth_ascii_punctuation = (0xFF01..=0xFF5E).contains(&code);
                let is_ascii_passthrough = code == 0x0021 || code == 0x003F;
                assert!(
                    in_hiragana
                        || is_long_mark_or_dot
                        || is_cjk_punctuation
                        || is_fullwidth_ascii_punctuation
                        || is_ascii_passthrough,
                    "rule ({:?} -> {:?}) contains char {:?} (U+{:04X}) outside the allowed value ranges",
                    key,
                    value,
                    ch,
                    code
                );
            }
        }
    }
}
