//! IBus keysym(X11 keysym 互換)→ `KeyEvent` 変換 helper。
//!
//! IBus は KeyPress signal で `(keysym: u32, keycode: u32, state: u32)` を渡す。
//! `state` は `IBusModifierType` flag bitfield。本 module は state を
//! `KeyModifiers` に変換する。

use kotoha_engine_core::{KeyEvent, KeyModifiers};

/// IBusModifierType の主要 flag(spec §13 Open Q 8 で完全 mapping は
/// 後続 task)。
mod ibus_state {
    pub const SHIFT_MASK: u32 = 1 << 0;
    pub const CONTROL_MASK: u32 = 1 << 2;
    pub const MOD1_MASK: u32 = 1 << 3; // Alt
    pub const SUPER_MASK: u32 = 1 << 26;
}

/// IBus KeyPress signal の生 args から `KeyEvent` を構築する。
///
/// # Preconditions
///
/// - `state` は `IBusModifierType` flag bitfield(IBus daemon 由来)
///
/// # Postconditions
///
/// - 主要 4 flag(Shift / Ctrl / Alt / Super)が `KeyModifiers` に mapping される
pub fn from_ibus(keysym: u32, keycode: u32, state: u32) -> KeyEvent {
    let mut modifiers = KeyModifiers::empty();
    if state & ibus_state::SHIFT_MASK != 0 {
        modifiers |= KeyModifiers::SHIFT;
    }
    if state & ibus_state::CONTROL_MASK != 0 {
        modifiers |= KeyModifiers::CTRL;
    }
    if state & ibus_state::MOD1_MASK != 0 {
        modifiers |= KeyModifiers::ALT;
    }
    if state & ibus_state::SUPER_MASK != 0 {
        modifiers |= KeyModifiers::SUPER;
    }
    KeyEvent {
        keysym,
        keycode,
        modifiers,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// modifier mapping: Shift + Ctrl
    #[test]
    fn maps_shift_and_ctrl() {
        let ev = from_ibus(0x6b, 45, ibus_state::SHIFT_MASK | ibus_state::CONTROL_MASK);
        assert!(ev.modifiers.contains(KeyModifiers::SHIFT));
        assert!(ev.modifiers.contains(KeyModifiers::CTRL));
        assert!(!ev.modifiers.contains(KeyModifiers::ALT));
    }

    /// 0 state は modifier 無し
    #[test]
    fn maps_no_modifier() {
        let ev = from_ibus(0x6b, 45, 0);
        assert!(ev.modifiers.is_empty());
    }

    /// keysym と keycode はそのまま KeyEvent に渡る
    #[test]
    fn preserves_keysym_and_keycode() {
        let ev = from_ibus(0x6b, 45, 0);
        assert_eq!(ev.keysym, 0x6b);
        assert_eq!(ev.keycode, 45);
    }
}
