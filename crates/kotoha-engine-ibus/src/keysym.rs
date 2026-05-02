//! IBus keysym(X11 keysym 互換)→ `KeyEvent` 変換 helper。
//!
//! IBus は KeyPress / KeyRelease signal で `(keysym: u32, keycode: u32, state: u32)`
//! を渡す。`state` は `IBusModifierType` flag bitfield。本 module は state を
//! `KeyModifiers` に変換する。

use kotoha_engine_core::{KeyEvent, KeyModifiers};

/// `IBusModifierType` の主要 flag(spec §13 Open Q 8、Phase 3-B B4 で網羅)。
///
/// 参照:`ibus/ibus/ibustypes.h` の `IBusModifierType` enum
mod ibus_state {
    pub const SHIFT_MASK: u32 = 1 << 0;
    pub const LOCK_MASK: u32 = 1 << 1; // Caps Lock
    pub const CONTROL_MASK: u32 = 1 << 2;
    pub const MOD1_MASK: u32 = 1 << 3; // Alt
    pub const MOD2_MASK: u32 = 1 << 4; // Num Lock(典型)
    pub const MOD3_MASK: u32 = 1 << 5;
    pub const MOD4_MASK: u32 = 1 << 6;
    pub const MOD5_MASK: u32 = 1 << 7;
    pub const SUPER_MASK: u32 = 1 << 26;
    pub const HYPER_MASK: u32 = 1 << 27;
    pub const META_MASK: u32 = 1 << 28;
    pub const RELEASE_MASK: u32 = 1 << 30;
}

/// IBus KeyPress / KeyRelease signal の生 args から `KeyEvent` を構築する。
///
/// # Preconditions
///
/// - `state` は `IBusModifierType` flag bitfield(IBus daemon 由来)
///
/// # Postconditions
///
/// - 戻り値の `modifiers` は IBus state flag を `KeyModifiers` に 1:1 mapping した結果
/// - `RELEASE_MASK` が set されている場合は `KeyModifiers::RELEASE` を立てる
///   (engine 側で release event を Forwarded に短絡する判断材料)
pub fn from_ibus(keysym: u32, keycode: u32, state: u32) -> KeyEvent {
    let mut modifiers = KeyModifiers::empty();
    if state & ibus_state::SHIFT_MASK != 0 {
        modifiers |= KeyModifiers::SHIFT;
    }
    if state & ibus_state::LOCK_MASK != 0 {
        modifiers |= KeyModifiers::LOCK;
    }
    if state & ibus_state::CONTROL_MASK != 0 {
        modifiers |= KeyModifiers::CTRL;
    }
    if state & ibus_state::MOD1_MASK != 0 {
        modifiers |= KeyModifiers::ALT;
    }
    if state & ibus_state::MOD2_MASK != 0 {
        modifiers |= KeyModifiers::MOD2;
    }
    if state & ibus_state::MOD3_MASK != 0 {
        modifiers |= KeyModifiers::MOD3;
    }
    if state & ibus_state::MOD4_MASK != 0 {
        modifiers |= KeyModifiers::MOD4;
    }
    if state & ibus_state::MOD5_MASK != 0 {
        modifiers |= KeyModifiers::MOD5;
    }
    if state & ibus_state::SUPER_MASK != 0 {
        modifiers |= KeyModifiers::SUPER;
    }
    if state & ibus_state::HYPER_MASK != 0 {
        modifiers |= KeyModifiers::HYPER;
    }
    if state & ibus_state::META_MASK != 0 {
        modifiers |= KeyModifiers::META;
    }
    if state & ibus_state::RELEASE_MASK != 0 {
        modifiers |= KeyModifiers::RELEASE;
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

    /// Caps Lock は LOCK flag に
    #[test]
    fn maps_caps_lock() {
        let ev = from_ibus(0x6b, 45, ibus_state::LOCK_MASK);
        assert!(ev.modifiers.contains(KeyModifiers::LOCK));
    }

    /// Num Lock(典型 MOD2)は MOD2 flag に
    #[test]
    fn maps_mod2_num_lock() {
        let ev = from_ibus(0x6b, 45, ibus_state::MOD2_MASK);
        assert!(ev.modifiers.contains(KeyModifiers::MOD2));
    }

    /// Super, Hyper, Meta の高位 bit
    #[test]
    fn maps_super_hyper_meta() {
        let ev = from_ibus(
            0x6b,
            45,
            ibus_state::SUPER_MASK | ibus_state::HYPER_MASK | ibus_state::META_MASK,
        );
        assert!(ev.modifiers.contains(KeyModifiers::SUPER));
        assert!(ev.modifiers.contains(KeyModifiers::HYPER));
        assert!(ev.modifiers.contains(KeyModifiers::META));
    }

    /// RELEASE_MASK は KeyModifiers::RELEASE に mapping(engine 側 Forwarded 判断材料)
    #[test]
    fn maps_release_mask() {
        let ev = from_ibus(0x6b, 45, ibus_state::RELEASE_MASK);
        assert!(ev.modifiers.contains(KeyModifiers::RELEASE));
    }

    /// 全 flag 同時 set でも漏れなく mapping される
    #[test]
    fn maps_all_flags_simultaneously() {
        let state = ibus_state::SHIFT_MASK
            | ibus_state::LOCK_MASK
            | ibus_state::CONTROL_MASK
            | ibus_state::MOD1_MASK
            | ibus_state::MOD2_MASK
            | ibus_state::MOD3_MASK
            | ibus_state::MOD4_MASK
            | ibus_state::MOD5_MASK
            | ibus_state::SUPER_MASK
            | ibus_state::HYPER_MASK
            | ibus_state::META_MASK
            | ibus_state::RELEASE_MASK;
        let ev = from_ibus(0x6b, 45, state);
        for m in [
            KeyModifiers::SHIFT,
            KeyModifiers::LOCK,
            KeyModifiers::CTRL,
            KeyModifiers::ALT,
            KeyModifiers::MOD2,
            KeyModifiers::MOD3,
            KeyModifiers::MOD4,
            KeyModifiers::MOD5,
            KeyModifiers::SUPER,
            KeyModifiers::HYPER,
            KeyModifiers::META,
            KeyModifiers::RELEASE,
        ] {
            assert!(ev.modifiers.contains(m), "missing flag: {m:?}");
        }
    }
}
