//! `IBusEventDispatcher` — IBus daemon からの D-Bus signal を受信し、
//! `IMEEngine` method 呼び出しに変換する driving adapter。
//!
//! Phase 3-A spec §3.3 全体図 / §3.2 driving adapter。
//! 本 PR (M5) では blocking loop で `IBusEngine` interface の signal を
//! receive し、`process_key_event` / `focus_in` / `focus_out` / `enable` /
//! `disable` / `reset` に dispatch する。
//!
//! 実際の D-Bus signal body decode + match は Phase 3-A 実装段階で
//! 詳細詰め(spec §13 Open Q 9)。本 PR では single-method dispatcher を
//! provide し、生 signal listener は kotoha-bin (M6) で確立する。

use kotoha_engine_core::IMEEngine;

use crate::keysym;

/// IBus daemon が送る engine signal を receive し engine に dispatch する。
///
/// 各 `dispatch_*` method は IBus daemon 側から受け取る生 args を受領し、
/// engine の対応 method を呼び出す薄い変換層として動く。
///
/// # Generic 引数
///
/// `E: IMEEngine` を受けるため、test では `MockEngine` を直接注入できる
/// (kotoha-engine-core::testing::MockHostBridge と同方針)。
pub struct IBusEventDispatcher<E: IMEEngine> {
    engine: E,
}

impl<E: IMEEngine> IBusEventDispatcher<E> {
    pub fn new(engine: E) -> Self {
        Self { engine }
    }

    /// IBus `ProcessKeyEvent(keyval, keycode, state)` signal を engine に dispatch する。
    ///
    /// # Returns
    ///
    /// `true` なら engine が消費(IBus への return 値として `true` を返す)、
    /// `false` なら IBus は default 処理(application に key を渡す)。
    pub fn dispatch_key(&mut self, keysym: u32, keycode: u32, state: u32) -> bool {
        let ev = keysym::from_ibus(keysym, keycode, state);
        matches!(
            self.engine.process_key_event(ev),
            kotoha_engine_core::KeyEventResult::Consumed
        )
    }

    pub fn dispatch_focus_in(&mut self) {
        self.engine.focus_in();
    }

    pub fn dispatch_focus_out(&mut self) {
        self.engine.focus_out();
    }

    pub fn dispatch_enable(&mut self) {
        self.engine.enable();
    }

    pub fn dispatch_disable(&mut self) {
        self.engine.disable();
    }

    pub fn dispatch_reset(&mut self) {
        self.engine.reset();
    }

    /// engine への可変参照を返す(test での状態確認用)。
    pub fn engine_mut(&mut self) -> &mut E {
        &mut self.engine
    }
}

#[cfg(test)]
mod tests {
    //! Phase 3-B B0e (ISSUE #140 / Important 11): dispatcher の 6 method を
    //! `MockEngine` 注入で網羅する unit test。

    use super::*;
    use kotoha_engine_core::ime_engine::IMEEngine;
    use kotoha_engine_core::key_event::{KeyEvent, KeyEventResult};

    /// dispatcher 単体 test 専用の最小 IMEEngine 実装。
    ///
    /// 各 method 呼び出しを名前文字列として `lifecycle` Vec に記録する。
    /// `process_key_event` の戻り値は `key_result` で制御可能。
    struct MockEngine {
        lifecycle: Vec<&'static str>,
        last_key: Option<KeyEvent>,
        key_result: KeyEventResult,
    }

    impl MockEngine {
        fn new(key_result: KeyEventResult) -> Self {
            Self {
                lifecycle: Vec::new(),
                last_key: None,
                key_result,
            }
        }
    }

    impl IMEEngine for MockEngine {
        fn process_key_event(&mut self, key: KeyEvent) -> KeyEventResult {
            self.lifecycle.push("process_key_event");
            self.last_key = Some(key);
            self.key_result
        }
        fn focus_in(&mut self) {
            self.lifecycle.push("focus_in");
        }
        fn focus_out(&mut self) {
            self.lifecycle.push("focus_out");
        }
        fn reset(&mut self) {
            self.lifecycle.push("reset");
        }
        fn enable(&mut self) {
            self.lifecycle.push("enable");
        }
        fn disable(&mut self) {
            self.lifecycle.push("disable");
        }
    }

    #[test]
    fn dispatch_key_consumed_returns_true_and_decodes_state() {
        let mut d = IBusEventDispatcher::new(MockEngine::new(KeyEventResult::Consumed));
        // SHIFT_MASK (1 << 0) | CONTROL_MASK (1 << 2) = 0b101 = 5
        let consumed = d.dispatch_key(0x6b, 45, 5);
        assert!(consumed);
        let ev = d.engine_mut().last_key.expect("key recorded");
        assert_eq!(ev.keysym, 0x6b);
        assert_eq!(ev.keycode, 45);
        assert!(ev
            .modifiers
            .contains(kotoha_engine_core::KeyModifiers::SHIFT));
        assert!(ev
            .modifiers
            .contains(kotoha_engine_core::KeyModifiers::CTRL));
        assert_eq!(d.engine_mut().lifecycle, vec!["process_key_event"]);
    }

    #[test]
    fn dispatch_key_forwarded_returns_false() {
        let mut d = IBusEventDispatcher::new(MockEngine::new(KeyEventResult::Forwarded));
        let consumed = d.dispatch_key(0x6b, 0, 0);
        assert!(!consumed);
    }

    #[test]
    fn dispatch_focus_in_invokes_focus_in() {
        let mut d = IBusEventDispatcher::new(MockEngine::new(KeyEventResult::Forwarded));
        d.dispatch_focus_in();
        assert_eq!(d.engine_mut().lifecycle, vec!["focus_in"]);
    }

    #[test]
    fn dispatch_focus_out_invokes_focus_out() {
        let mut d = IBusEventDispatcher::new(MockEngine::new(KeyEventResult::Forwarded));
        d.dispatch_focus_out();
        assert_eq!(d.engine_mut().lifecycle, vec!["focus_out"]);
    }

    #[test]
    fn dispatch_enable_invokes_enable() {
        let mut d = IBusEventDispatcher::new(MockEngine::new(KeyEventResult::Forwarded));
        d.dispatch_enable();
        assert_eq!(d.engine_mut().lifecycle, vec!["enable"]);
    }

    #[test]
    fn dispatch_disable_invokes_disable() {
        let mut d = IBusEventDispatcher::new(MockEngine::new(KeyEventResult::Forwarded));
        d.dispatch_disable();
        assert_eq!(d.engine_mut().lifecycle, vec!["disable"]);
    }

    #[test]
    fn dispatch_reset_invokes_reset() {
        let mut d = IBusEventDispatcher::new(MockEngine::new(KeyEventResult::Forwarded));
        d.dispatch_reset();
        assert_eq!(d.engine_mut().lifecycle, vec!["reset"]);
    }
}
