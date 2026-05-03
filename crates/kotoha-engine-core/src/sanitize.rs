//! Engine ↔ host (IBus daemon / fcitx5 / etc) boundary 用の output sanitization。
//!
//! B0g-b #148 / 第 2 回 review I8: `IMEHostBridge::commit_text` /
//! `update_preedit` / `update_candidates` を経由して application に届く文字列が
//! 攻撃者制御の input(改ざん辞書 / 悪意ある LLM 出力 / Phase 5 custom model)
//! を含む可能性がある。production user の terminal / chat client / git commit
//! editor 等に **ANSI escape / NUL byte / RTL override / control char** が
//! 注入されると application 側の信頼境界が破られる。
//!
//! 本 module は **trust boundary に最も近い engine 側で reject filter** を
//! 提供する(spec §9.3 silent failure 禁止 + Phase 4/5 multi-adapter 設計)。
//! 既存の `kotoha-storage::validation` は **write-side**(SQLite 入力)用で
//! 同 crate を engine-core から import するのは hexagonal 違反(C3)のため、
//! 重複実装を許容する。B0h で hexagonal port 反転後に共通 crate に統合する。

/// `commit_text` / `update_preedit` 等 host 出力に許可する文字判定。
///
/// 許可しない:
///
/// - すべての ASCII control(`\u{0000}..=\u{001F}` + `\u{007F}`)を reject。
///   通常の改行(`\n`)は preedit / commit に出ない設計のため、含まれていれば
///   攻撃 / バグ。
/// - bidi override(`\u{202A}..=\u{202E}` + `\u{2066}..=\u{2069}`)を reject。
///   "Trojan Source" 攻撃(CVE-2021-42574)を防ぐ。
/// - C1 control(`\u{0080}..=\u{009F}`)を reject。
/// - tab character (`\u{0009}`) は control char として上記範囲で reject される。
///
/// 許可する:
///
/// - 通常の hiragana / katakana / kanji / latin / digit / 一般記号
/// - emoji / kanji variation selector(`\u{FE00}..=\u{FE0F}` + `\u{E0100}..=\u{E01EF}`)
///   は将来の Phase 5 で必要となる可能性が高いため許可
///
/// # Returns
///
/// `true` なら 1 char すべて safe、`false` なら 1 つでも unsafe 文字を含む。
pub fn is_safe_for_host(text: &str) -> bool {
    text.chars().all(is_safe_char)
}

/// 単一 char の host 出力許可判定。`is_safe_for_host` の helper。
fn is_safe_char(c: char) -> bool {
    let cp = c as u32;
    // ASCII control 0x00..=0x1F + DEL 0x7F
    if cp <= 0x1F || cp == 0x7F {
        return false;
    }
    // C1 control 0x80..=0x9F
    if (0x80..=0x9F).contains(&cp) {
        return false;
    }
    // bidi override + isolate
    if (0x202A..=0x202E).contains(&cp) || (0x2066..=0x2069).contains(&cp) {
        return false;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allows_normal_japanese_and_latin() {
        assert!(is_safe_for_host("hello world"));
        assert!(is_safe_for_host("こんにちは"));
        assert!(is_safe_for_host("漢字テキスト"));
        assert!(is_safe_for_host("Hello, 世界! 🦀"));
    }

    #[test]
    fn rejects_nul_byte() {
        assert!(!is_safe_for_host("abc\0def"));
    }

    #[test]
    fn rejects_ansi_escape_sequence_starter() {
        // ESC (0x1B) + [2J would clear screen in a terminal
        assert!(!is_safe_for_host("\u{001B}[2J"));
    }

    #[test]
    fn rejects_bidi_override() {
        // Trojan Source (CVE-2021-42574) RTL override
        assert!(!is_safe_for_host("ab\u{202E}cd"));
        // LRO
        assert!(!is_safe_for_host("ab\u{202D}cd"));
        // RLI
        assert!(!is_safe_for_host("ab\u{2067}cd"));
    }

    #[test]
    fn rejects_c1_control() {
        // CSI U+009B
        assert!(!is_safe_for_host("ab\u{009B}cd"));
    }

    #[test]
    fn rejects_del() {
        assert!(!is_safe_for_host("ab\u{007F}cd"));
    }

    #[test]
    fn rejects_tab() {
        // Tab is a control char (0x09); reject in IME output context.
        assert!(!is_safe_for_host("ab\tcd"));
    }
}
