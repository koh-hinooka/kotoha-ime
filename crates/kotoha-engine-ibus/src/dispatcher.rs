//! `IBusEventDispatcher` — IBus daemon からの D-Bus signal を受信し、
//! `IMEEngine` method 呼び出しに変換する driving adapter。
//!
//! Phase 3-A spec §3.3 全体図 / §3.2 driving adapter。
//! 本 module は single-method dispatcher を provide し、生 signal listener は
//! kotoha-bin / Phase 3-B B3 で確立する。実際の D-Bus signal body decode + match
//! は Phase 3-B B2(spec §13 Open Q 9)で詳細化する。
//!
//! # Phase 3-B B0h-d (ISSUE #149 / #157):dispatcher を `Arc<Mutex<dyn IMEEngine>>`
//!
//! 旧 dispatcher は `IBusEventDispatcher<E: IMEEngine> { engine: E }` で engine を
//! 自身で own していた。Phase 3-B B3 (event loop) で D-Bus signal listener thread
//! と engine を multi-thread 越しに共有する瞬間に必ず `Arc<Mutex<dyn IMEEngine>>` 化
//! が必要となるため(spec §13 Open Q 9)、B3 直前に入れる先行作業として B0h-d で
//! 反転した。各 `dispatch_*` method 内は `self.engine.lock().unwrap_or_else
//! (PoisonError::into_inner)` 経由で MutexGuard を取り、その上で engine を呼ぶ。
//! Mutex poisoning policy は kotoha-storage の `LearningCacheStore` などと同じ
//! 「内部 invariant が無いので poison から復帰する」方針(B0g-a / PR #150)。

use std::panic::{self, AssertUnwindSafe};
use std::sync::{Arc, Mutex, PoisonError};

use kotoha_engine_core::IMEEngine;

use crate::keysym;

/// IBus daemon が送る engine signal を receive し engine に dispatch する。
///
/// 各 `dispatch_*` method は IBus daemon 側から受け取る生 args を受領し、
/// engine の対応 method を呼び出す薄い変換層として動く。`engine` は
/// [`Arc<Mutex<dyn IMEEngine>>`] として保持され、外部(将来の B3 event loop /
/// test)から `Arc::clone` で共有可能。`IMEEngine` trait は `Send` を要求し、
/// `Mutex<T>: Sync where T: Send` から `Arc<Mutex<dyn IMEEngine>>` は
/// `Send + Sync` を満たす。
pub struct IBusEventDispatcher {
    engine: Arc<Mutex<dyn IMEEngine>>,
}

impl IBusEventDispatcher {
    /// dispatcher を構築する。
    ///
    /// # Preconditions
    ///
    /// - `engine` は `IMEEngine` trait object の Mutex を共有する Arc。
    ///   呼び出し側(`kotoha-bin`)で `Arc::new(Mutex::new(KotohaEngine::new(...)))`
    ///   等として構築する。
    pub fn new(engine: Arc<Mutex<dyn IMEEngine>>) -> Self {
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
    pub fn dispatch_key(&self, keysym: u32, keycode: u32, state: u32) -> bool {
        let ev = keysym::from_ibus(keysym, keycode, state);
        let result = panic::catch_unwind(AssertUnwindSafe(|| {
            let mut guard = self.engine.lock().unwrap_or_else(PoisonError::into_inner);
            guard.process_key_event(ev)
        }));
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
                    let mut guard = self.engine.lock().unwrap_or_else(PoisonError::into_inner);
                    guard.reset();
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

    pub fn dispatch_focus_in(&self) {
        let mut guard = self.engine.lock().unwrap_or_else(PoisonError::into_inner);
        guard.focus_in();
    }

    pub fn dispatch_focus_out(&self) {
        let mut guard = self.engine.lock().unwrap_or_else(PoisonError::into_inner);
        guard.focus_out();
    }

    pub fn dispatch_enable(&self) {
        let mut guard = self.engine.lock().unwrap_or_else(PoisonError::into_inner);
        guard.enable();
    }

    pub fn dispatch_disable(&self) {
        let mut guard = self.engine.lock().unwrap_or_else(PoisonError::into_inner);
        guard.disable();
    }

    pub fn dispatch_reset(&self) {
        let mut guard = self.engine.lock().unwrap_or_else(PoisonError::into_inner);
        guard.reset();
    }
}

#[cfg(test)]
mod tests {
    //! Phase 3-B B0e (ISSUE #140 / Important 11): dispatcher の 6 method を
    //! `MockEngine` 注入で網羅する unit test。
    //!
    //! Phase 3-B B0h-d (ISSUE #149 / #157): dispatcher が `Arc<Mutex<dyn IMEEngine>>`
    //! ベースに変わったため、test fixture も `Arc<Mutex<MockEngine>>` を
    //! 共有する pattern に切り替えた(test 側で同 Arc を保持し直接観測する)。

    use super::*;
    use kotoha_engine_core::ime_engine::IMEEngine;
    use kotoha_engine_core::key_event::{KeyEvent, KeyEventResult};
    use std::sync::{Arc, Mutex};

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

    /// MockEngine を Arc<Mutex<>> で wrap して dispatcher と test code に共有
    /// 注入するための helper。返り値の `(dispatcher, mock)` 双方が同 inner を
    /// 共有しており、`mock.lock().unwrap()` で test 側が直接観測できる。
    fn build_with_mock(
        key_result: KeyEventResult,
    ) -> (IBusEventDispatcher, Arc<Mutex<MockEngine>>) {
        let mock = Arc::new(Mutex::new(MockEngine::new(key_result)));
        let engine: Arc<Mutex<dyn IMEEngine>> = mock.clone();
        (IBusEventDispatcher::new(engine), mock)
    }

    #[test]
    fn dispatch_key_consumed_returns_true_and_decodes_state() {
        let (d, mock) = build_with_mock(KeyEventResult::Consumed);
        // SHIFT_MASK (1 << 0) | CONTROL_MASK (1 << 2) = 0b101 = 5
        let consumed = d.dispatch_key(0x6b, 45, 5);
        assert!(consumed);
        let m = mock.lock().expect("mock lock");
        let ev = m.last_key.expect("key recorded");
        assert_eq!(ev.keysym, 0x6b);
        assert_eq!(ev.keycode, 45);
        assert!(ev
            .modifiers
            .contains(kotoha_engine_core::KeyModifiers::SHIFT));
        assert!(ev
            .modifiers
            .contains(kotoha_engine_core::KeyModifiers::CTRL));
        assert_eq!(m.lifecycle, vec!["process_key_event"]);
    }

    #[test]
    fn dispatch_key_forwarded_returns_false() {
        let (d, _mock) = build_with_mock(KeyEventResult::Forwarded);
        let consumed = d.dispatch_key(0x6b, 0, 0);
        assert!(!consumed);
    }

    #[test]
    fn dispatch_focus_in_invokes_focus_in() {
        let (d, mock) = build_with_mock(KeyEventResult::Forwarded);
        d.dispatch_focus_in();
        assert_eq!(mock.lock().expect("mock lock").lifecycle, vec!["focus_in"]);
    }

    #[test]
    fn dispatch_focus_out_invokes_focus_out() {
        let (d, mock) = build_with_mock(KeyEventResult::Forwarded);
        d.dispatch_focus_out();
        assert_eq!(mock.lock().expect("mock lock").lifecycle, vec!["focus_out"]);
    }

    #[test]
    fn dispatch_enable_invokes_enable() {
        let (d, mock) = build_with_mock(KeyEventResult::Forwarded);
        d.dispatch_enable();
        assert_eq!(mock.lock().expect("mock lock").lifecycle, vec!["enable"]);
    }

    #[test]
    fn dispatch_disable_invokes_disable() {
        let (d, mock) = build_with_mock(KeyEventResult::Forwarded);
        d.dispatch_disable();
        assert_eq!(mock.lock().expect("mock lock").lifecycle, vec!["disable"]);
    }

    #[test]
    fn dispatch_reset_invokes_reset() {
        let (d, mock) = build_with_mock(KeyEventResult::Forwarded);
        d.dispatch_reset();
        assert_eq!(mock.lock().expect("mock lock").lifecycle, vec!["reset"]);
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
    ///
    /// B0h-d:Mutex poison 後でも reset() が呼べ、test 側 observation が
    /// `unwrap_or_else(PoisonError::into_inner)` 経由で続行できることも合わせて
    /// 検証する。
    #[test]
    fn dispatch_key_catches_engine_panic_and_resets_state() {
        let mock = Arc::new(Mutex::new(PanickingEngine::new()));
        let engine: Arc<Mutex<dyn IMEEngine>> = mock.clone();
        let d = IBusEventDispatcher::new(engine);
        let result = d.dispatch_key(0x6b, 0, 0);
        assert!(
            !result,
            "dispatch_key should return false when engine.process_key_event panics"
        );
        // panic 発生で Mutex は poison 状態。test 側からも
        // `unwrap_or_else(PoisonError::into_inner)` で recover して観測する。
        let m = mock.lock().unwrap_or_else(PoisonError::into_inner);
        assert!(
            m.process_key_called,
            "process_key_event should have been called before the panic"
        );
        assert!(
            m.reset_called,
            "reset() should be called after panic recovery in dispatch_key"
        );
    }
}
