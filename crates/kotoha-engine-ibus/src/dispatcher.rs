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

use std::panic::{self, AssertUnwindSafe};

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
    ///
    /// # Panic recovery
    ///
    /// B0g #148 / 第 2 回 review I17 + spec §9.1 row 5: `engine.process_key_event`
    /// 内で発生した panic を catch_unwind で受け、engine state を `reset()` で
    /// Idle に戻し、本 dispatch では `false`(forward)を返す。これにより:
    ///
    /// - IBus daemon 側 thread が panic で死んで keystroke が永久 hang する
    ///   (D-Bus signal handler thread の `process_key_event` 経由 unwind)を
    ///   防ぐ
    /// - panic 検出は `tracing::error!` で観測される
    /// - reset() 自体が panic した場合は二重 panic を避けるため再 catch_unwind
    ///   で囲い、それも失敗したら最後の手段として `false` だけ返す
    pub fn dispatch_key(&mut self, keysym: u32, keycode: u32, state: u32) -> bool {
        let ev = keysym::from_ibus(keysym, keycode, state);
        let engine = &mut self.engine;
        let result = panic::catch_unwind(AssertUnwindSafe(|| engine.process_key_event(ev)));
        match result {
            Ok(kotoha_engine_core::KeyEventResult::Consumed) => true,
            Ok(kotoha_engine_core::KeyEventResult::Forwarded) => false,
            Err(payload) => {
                // self-review F3 (B0g-b):TypeId opaque hex dump を `&'static str`
                // / `String` / `panic_any(...)` 各 payload に対応した message に
                // 拡張する共有 helper を再利用。
                let msg = kotoha_engine_core::engine::panic_message_from(&payload);
                tracing::error!(
                    keysym,
                    keycode,
                    state,
                    panic = %msg,
                    "engine.process_key_event panicked; resetting engine state"
                );
                // reset() 自体の二重 panic は session 全死亡相当で recovery 不能
                // だが、**reset 失敗の事実** は ERROR log に必ず残す(self-review #5
                // 指摘:payload 中身は捨てても fact は残さないと後続 keystroke で
                // 再 panic ループが起きた時に root cause traceability が失われる)。
                let reset_result = panic::catch_unwind(AssertUnwindSafe(|| {
                    self.engine.reset();
                }));
                if let Err(reset_payload) = reset_result {
                    let reset_msg = kotoha_engine_core::engine::panic_message_from(&reset_payload);
                    tracing::error!(
                        panic = %reset_msg,
                        "engine.reset() panicked during dispatch_key recovery; \
                         engine state corruption likely; subsequent keystrokes may panic again"
                    );
                }
                false
            }
        }
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

    // --------------------------------------------------------------
    // Phase 3-B B0g (ISSUE #148 / I17): dispatch_key panic catch
    // --------------------------------------------------------------

    /// `process_key_event` が panic する MockEngine。dispatcher の
    /// catch_unwind 経路 + reset() invocation を観測する fixture。
    struct PanickingEngine {
        process_key_called: bool,
        reset_called: bool,
    }
    impl PanickingEngine {
        fn new() -> Self {
            Self {
                process_key_called: false,
                reset_called: false,
            }
        }
    }
    impl IMEEngine for PanickingEngine {
        fn process_key_event(&mut self, _key: KeyEvent) -> KeyEventResult {
            self.process_key_called = true;
            panic!("intentional process_key_event panic for I17 regression");
        }
        fn focus_in(&mut self) {}
        fn focus_out(&mut self) {}
        fn reset(&mut self) {
            self.reset_called = true;
        }
        fn enable(&mut self) {}
        fn disable(&mut self) {}
    }

    /// I17: dispatch_key 内で process_key_event が panic した場合、dispatcher
    /// は catch_unwind で受けて reset() を呼び、戻り値は `false`(forward)を
    /// 返す。IBus daemon 側 thread の永久 hang(spec §9.1 row 5)を防ぐ。
    #[test]
    fn dispatch_key_catches_engine_panic_and_resets_state() {
        let mut d = IBusEventDispatcher::new(PanickingEngine::new());
        let result = d.dispatch_key(0x6b, 0, 0);
        assert!(
            !result,
            "dispatch_key should return false when engine.process_key_event panics"
        );
        assert!(
            d.engine_mut().process_key_called,
            "process_key_event should have been called before the panic"
        );
        assert!(
            d.engine_mut().reset_called,
            "reset() should be called after panic recovery in dispatch_key"
        );
    }
}
