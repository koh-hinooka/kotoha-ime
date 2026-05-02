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
