//! `KotohaEngineService` — IBus engine method dispatcher。
//!
//! Phase 3-B B6-b (#195) で導入。zbus 5 `#[interface]` macro 経由で
//! `org.freedesktop.IBus.Engine` interface の 6 method を実装する。
//!
//! 詳細仕様: `docs/specs/_uncategorized/p3-b-ibus-listener.md` §4 / §5。

use std::time::Duration;

use crossbeam_channel::Sender;
use zbus::interface;

use kotoha_engine_core::key_event::KeyEventResult;
use kotoha_engine_core::reactor::{Event, IBusResetKind};

use crate::keysym;

/// IBus engine method dispatcher。
///
/// `bridge_tx` 経由で engine-loop thread に Event を届ける。state を保持しない
/// (ADR 0020 §採択 Q4 lock-free 原則)。zbus が internal smol executor で本 struct
/// の method を dispatch する。
///
/// # 配置
///
/// `pub(crate)` で crate 外部に export しない。`listener::run` 内で構築・登録される。
pub(crate) struct KotohaEngineService {
    /// engine-loop thread に Event を送る単方向 channel。
    pub(crate) bridge_tx: Sender<Event>,
}

/// `process_key_event` の response 待機 timeout。
///
/// 通常 ~1ms / heavy ~10ms / recovery ~50ms 全てを吸収しつつ、bug 状態 (>100ms) のみ
/// fallback fire するように設定(spec §6.1)。
const PROCESS_KEY_EVENT_TIMEOUT: Duration = Duration::from_millis(100);

#[interface(name = "org.freedesktop.IBus.Engine")]
impl KotohaEngineService {
    /// `ProcessKeyEvent(u keyval, u keycode, u state) -> b`
    ///
    /// IBus 1.5.x 仕様準拠。戻り値 true は Kotoha が消費した、false は app に forward する。
    fn process_key_event(&self, keyval: u32, keycode: u32, state: u32) -> bool {
        let event = keysym::from_ibus(keyval, keycode, state);
        let (resp_tx, resp_rx) = crossbeam_channel::bounded::<KeyEventResult>(1);
        if self
            .bridge_tx
            .send(Event::IBusKey {
                event,
                respond: resp_tx,
            })
            .is_err()
        {
            tracing::warn!(
                error_id = "listener.process_key_event.bridge_disconnected",
                "engine-loop disconnected; forwarding key to app"
            );
            return false;
        }
        match resp_rx.recv_timeout(PROCESS_KEY_EVENT_TIMEOUT) {
            Ok(KeyEventResult::Consumed) => true,
            Ok(KeyEventResult::Forwarded) => false,
            Err(_) => {
                tracing::warn!(
                    error_id = "listener.process_key_event.timeout",
                    timeout_ms = PROCESS_KEY_EVENT_TIMEOUT.as_millis() as u64,
                    "engine response timeout; forwarding key to app"
                );
                false
            }
        }
    }

    /// `FocusOut()` — engine 側 reset 経路を発火する。応答不要。
    fn focus_out(&self) {
        let _ = self
            .bridge_tx
            .send(Event::IBusReset(IBusResetKind::FocusOut));
    }

    /// `Reset()` — engine 側 reset 経路。応答不要。
    fn reset(&self) {
        let _ = self.bridge_tx.send(Event::IBusReset(IBusResetKind::Reset));
    }

    /// `Disable()` — engine 側 disable 経路。応答不要。
    fn disable(&self) {
        let _ = self
            .bridge_tx
            .send(Event::IBusReset(IBusResetKind::Disable));
    }

    /// `FocusIn()` — daemon 側 introspection 互換のための no-op stub(spec §4.3)。
    fn focus_in(&self) {
        tracing::debug!("KotohaEngineService::focus_in (no-op)");
    }

    /// `Enable()` — daemon 側 introspection 互換のための no-op stub(spec §4.3)。
    fn enable(&self) {
        tracing::debug!("KotohaEngineService::enable (no-op)");
    }
}

#[cfg(test)]
mod tests {
    //! Spec: `docs/specs/_uncategorized/p3-b-ibus-listener.md` §10.1
    //! L1 unit tests。dbus を使わず crossbeam channel 直接観測で 4 経路を pin する。
    //!
    //! - process_key_event normal: respond.send(Consumed) → true
    //! - process_key_event normal: respond.send(Forwarded) → false
    //! - process_key_event timeout (engine が応答しない) → false
    //! - process_key_event disconnect (engine_loop drop) → false
    //! - reset 系 3 method の Event 送信
    //! - no-op stub 2 method の non-panic

    use super::*;
    use crossbeam_channel::{unbounded, TryRecvError};
    use std::thread;

    fn setup() -> (KotohaEngineService, crossbeam_channel::Receiver<Event>) {
        let (tx, rx) = unbounded::<Event>();
        let service = KotohaEngineService { bridge_tx: tx };
        (service, rx)
    }

    #[test]
    fn process_key_event_returns_true_on_consumed() {
        let (service, rx) = setup();
        let join = thread::spawn(move || service.process_key_event(0x6b, 45, 0));
        // engine-loop に成り代わって respond.send(Consumed) する
        match rx.recv() {
            Ok(Event::IBusKey { event: _, respond }) => {
                respond
                    .send(KeyEventResult::Consumed)
                    .expect("send Consumed");
            }
            other => panic!("expected IBusKey, got {other:?}"),
        }
        assert!(join.join().expect("thread join"));
    }

    #[test]
    fn process_key_event_returns_false_on_forwarded() {
        let (service, rx) = setup();
        let join = thread::spawn(move || service.process_key_event(0xff1b, 9, 0));
        match rx.recv() {
            Ok(Event::IBusKey { event: _, respond }) => {
                respond
                    .send(KeyEventResult::Forwarded)
                    .expect("send Forwarded");
            }
            other => panic!("expected IBusKey, got {other:?}"),
        }
        assert!(!join.join().expect("thread join"));
    }

    #[test]
    fn process_key_event_returns_false_on_timeout() {
        let (service, _rx) = setup();
        // _rx を drop しないことで bridge_tx は alive、engine-loop が hang した状態を simulate
        // (rx は受信するが respond.send は呼ばない)
        let start = std::time::Instant::now();
        let result = service.process_key_event(0x20, 65, 0);
        let elapsed = start.elapsed();
        assert!(!result, "timeout should return false (Forwarded)");
        // 100ms ~ 200ms (timing tolerance) 内に return することを観測
        assert!(
            elapsed >= Duration::from_millis(100) && elapsed < Duration::from_millis(300),
            "timeout should fire near 100ms, got {elapsed:?}"
        );
    }

    #[test]
    fn process_key_event_returns_false_on_disconnect() {
        let (tx, rx) = unbounded::<Event>();
        let service = KotohaEngineService { bridge_tx: tx };
        drop(rx); // engine-loop drop simulation
        let result = service.process_key_event(0x20, 65, 0);
        assert!(!result, "disconnect should return false");
    }

    #[test]
    fn focus_out_sends_focus_out_event() {
        let (service, rx) = setup();
        service.focus_out();
        match rx.try_recv() {
            Ok(Event::IBusReset(IBusResetKind::FocusOut)) => {}
            other => panic!("expected IBusReset(FocusOut), got {other:?}"),
        }
    }

    #[test]
    fn reset_sends_reset_event() {
        let (service, rx) = setup();
        service.reset();
        match rx.try_recv() {
            Ok(Event::IBusReset(IBusResetKind::Reset)) => {}
            other => panic!("expected IBusReset(Reset), got {other:?}"),
        }
    }

    #[test]
    fn disable_sends_disable_event() {
        let (service, rx) = setup();
        service.disable();
        match rx.try_recv() {
            Ok(Event::IBusReset(IBusResetKind::Disable)) => {}
            other => panic!("expected IBusReset(Disable), got {other:?}"),
        }
    }

    #[test]
    fn focus_in_does_not_panic_or_send() {
        let (service, rx) = setup();
        service.focus_in();
        assert!(matches!(rx.try_recv(), Err(TryRecvError::Empty)));
    }

    #[test]
    fn enable_does_not_panic_or_send() {
        let (service, rx) = setup();
        service.enable();
        assert!(matches!(rx.try_recv(), Err(TryRecvError::Empty)));
    }
}
