//! `KeyEvent` / `KeyEventResult` / `KeyModifiers` — host から engine への
//! keystroke 入力 + engine から host への consume/forward 判断。
//!
//! Phase 3-A spec §4.1 で凍結。bitflags 表現は IBus IBusModifierType と同型で、
//! adapter 層で 1:1 mapping される(spec §13 Open Q 8 で完全 mapping は実装段階対応)。

use bitflags::bitflags;

/// 1 keystroke の表現。host adapter が IBus / fcitx5 等の event から構築して
/// [`crate::ime_engine::IMEEngine::process_key_event`] に渡す。
///
/// # Invariants
///
/// - `keysym` は X11 keysym(`XK_*`)を u32 で保持する。spec §13 Open Q 8 で
///   full mapping を adapter 段階で確定する。
/// - `keycode` は physical keycode(layout 非依存判定用、Phase 3-A 初期は
///   未使用、forward-compat のため field 確保)。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyEvent {
    /// X11 keysym(IBus が KeyPress event で渡す sym と同一の u32 値)。
    pub keysym: u32,
    /// 物理 keycode(layout 非依存判定用、Phase 3-A 初期は未使用)。
    pub keycode: u32,
    /// modifier 状態(Shift / Ctrl / Alt / Super)。
    pub modifiers: KeyModifiers,
}

bitflags! {
    /// Keystroke 同伴 modifier。IBus `IBusModifierType` の主要 flag を網羅する
    /// (Phase 3-B B4、ISSUE #136、spec §13 Open Q 8 解決)。bit 位置は IBus 由来
    /// ではなく自前序数で持ち、IBus → 本 enum mapping は host adapter 層で行う。
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct KeyModifiers: u32 {
        const SHIFT   = 1 << 0;
        const LOCK    = 1 << 1;  // Caps Lock
        const CTRL    = 1 << 2;
        const ALT     = 1 << 3;  // Mod1 (Alt)
        const MOD2    = 1 << 4;  // Num Lock(典型)
        const MOD3    = 1 << 5;
        const MOD4    = 1 << 6;
        const MOD5    = 1 << 7;
        const SUPER   = 1 << 8;
        const HYPER   = 1 << 9;
        const META    = 1 << 10;
        /// IBus `IBUS_RELEASE_MASK` 相当。`process_key_event` 受領側は本 flag が
        /// 立っている event を **release event** として識別し、典型的には
        /// `KeyEventResult::Forwarded` に短絡する。
        const RELEASE = 1 << 11;
    }
}

/// [`crate::ime_engine::IMEEngine::process_key_event`] の戻り値。
///
/// # Invariants
///
/// - `Consumed`:engine が keystroke を吸収。host は application に key を渡してはならない。
/// - `Forwarded`:engine は不処理。host は application に key を渡してよい。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyEventResult {
    Consumed,
    Forwarded,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// spec §4.1: KeyModifiers の bitflag 合成
    #[test]
    fn modifiers_combine_via_bitor() {
        let m = KeyModifiers::SHIFT | KeyModifiers::CTRL;
        assert!(m.contains(KeyModifiers::SHIFT));
        assert!(m.contains(KeyModifiers::CTRL));
        assert!(!m.contains(KeyModifiers::ALT));
    }

    /// spec §4.1: empty modifier は no flag
    #[test]
    fn modifiers_empty_has_no_flags() {
        let m = KeyModifiers::empty();
        assert!(!m.contains(KeyModifiers::SHIFT));
        assert!(!m.contains(KeyModifiers::CTRL));
    }

    /// spec §4.1: KeyEvent は Copy(adapter 層で頻繁に複製される)
    #[test]
    fn key_event_is_copy() {
        let ev = KeyEvent {
            keysym: 0x6b, // 'k'
            keycode: 45,
            modifiers: KeyModifiers::empty(),
        };
        let ev2 = ev;
        assert_eq!(ev, ev2);
    }

    /// spec §4.1: KeyEventResult variants
    #[test]
    fn key_event_result_distinct_variants() {
        assert_ne!(KeyEventResult::Consumed, KeyEventResult::Forwarded);
    }
}
