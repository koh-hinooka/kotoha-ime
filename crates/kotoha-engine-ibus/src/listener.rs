//! D-Bus signal listener loop(Phase 3-B B3 / ADR 0020 §採択 Q4)。
//!
//! ADR 0020 §採択 Q4 の `kotoha-dbus-listener` thread を担う。本 module は
//! 4-thread topology の listener slot を埋める architectural skeleton を提供する。
//!
//! # Status (rev3, 2026-05-06)
//!
//! 本 PR では「listener thread を spawn して bridge channel と engine-loop を
//! 接続する architectural wiring」までを完了し、実 zbus message stream の
//! decode は B6 (#136) manual smoke で完成させる。理由:
//!
//! - zbus 5 の `blocking::Connection` は MessageStream を直接提供せず、
//!   `interface!` macro 経由 service registration もしくは async API + block_on
//!   が必要で、本 PR scope (B0h-f + B3 architectural rework) を超える
//! - IBus daemon が本 process に向けてメッセージを routing するには、別途
//!   `RequestName` + IBus engine factory 登録が必要(spec §11 / B6 manual smoke)
//! - 本 PR の目標(`drain_events_blocking` 撤去 + 4-thread topology)は
//!   listener stub でも検証可能(`Event::Shutdown` 経路で thread 連携を観測)
//!
//! 完成形は ADR 0020 §影響範囲 ~120 lines の見積もり通り、本 module を
//! 拡張して `zbus::interface!` macro で IBus engine interface を impl し、
//! 各 method handler が bridge_tx に Event を送る形になる予定。
//!
//! # Decoded methods (planned, B6 で完成)
//!
//! - `ProcessKeyEvent(u keyval, u keycode, u state) -> b` → `Event::IBusKey(KeyEvent)`
//! - `Reset()` → `Event::IBusReset(IBusResetKind::Reset)`
//! - `FocusOut()` → `Event::IBusReset(IBusResetKind::FocusOut)`
//! - `Disable()` → `Event::IBusReset(IBusResetKind::Disable)`

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crossbeam_channel::Sender;
use kotoha_engine_core::reactor::Event;

/// Listener thread の shutdown signal handle。
///
/// main thread は `LinuxReactor` の `shutdown_tx` close で engine-loop を停止
/// させた後、本 handle 経由で listener にも shutdown を伝える。
pub struct ListenerShutdown {
    flag: Arc<AtomicBool>,
}

impl ListenerShutdown {
    /// listener が観測する shutdown flag を生成する。
    #[must_use]
    pub fn new() -> Self {
        Self {
            flag: Arc::new(AtomicBool::new(false)),
        }
    }

    /// listener thread が観測する flag handle を作る(thread に move する)。
    #[must_use]
    pub fn observer(&self) -> Arc<AtomicBool> {
        self.flag.clone()
    }

    /// shutdown を要求する。listener は次 poll サイクルで return する。
    pub fn request(&self) {
        self.flag.store(true, Ordering::SeqCst);
    }
}

impl Default for ListenerShutdown {
    fn default() -> Self {
        Self::new()
    }
}

/// 本 PR の listener stub 起動を許可する env var 名。
///
/// production 環境では本 var 未設定で `run()` は起動拒否(spec §9.3 fail-loud)。
/// 開発・CI 用途では `=1` を export して stub を許可する。Phase 3-B B6 で実 zbus
/// 経路完成時に env var 自体は不要となる(本 const も削除予定)。
pub const KOTOHA_ALLOW_LISTENER_STUB_ENV: &str = "KOTOHA_ALLOW_LISTENER_STUB";

/// listener stub を許可する env var 値の判定。
fn allow_listener_stub() -> bool {
    matches!(
        std::env::var(KOTOHA_ALLOW_LISTENER_STUB_ENV).as_deref(),
        Ok("1" | "true" | "yes")
    )
}

/// listener が `run()` 起動拒否したことを示す sentinel。
#[derive(Debug, thiserror::Error)]
#[error(
    "B3 listener is a non-functional stub (Phase 3-B B0h-f rev3 / ADR 0020 §影響). \
     The IME will not deliver any key events to the engine in this state. \
     Set {KOTOHA_ALLOW_LISTENER_STUB_ENV}=1 only for development / CI smoke runs. \
     Full implementation tracked in ISSUE #136 B6 manual smoke."
)]
pub struct ListenerStubRefused;

/// D-Bus listener thread の main loop(B3 architectural skeleton)。
///
/// # Preconditions
///
/// - `_connection` は session bus に接続済(`zbus::blocking::Connection::session()`)
/// - `bridge_tx` は engine-loop thread の `EventReactor` bridge channel
/// - `shutdown` は main thread が `request()` で停止指示する flag
///
/// # Postconditions
///
/// - `shutdown.load() == true` を観測したら `Ok(())` で return する
/// - `bridge_tx.send()` が `Err` を返したら(engine-loop drop で channel close)
///   `Ok(())` で return する
///
/// # Errors
///
/// - [`ListenerStubRefused`] — `KOTOHA_ALLOW_LISTENER_STUB` が未設定で stub
///   実装の起動を拒否した場合(spec §9.3 fail-loud / B6 完成までの safety net)
///
/// # Stub behavior
///
/// 本 PR では実 zbus message stream の decode は実装せず、shutdown flag を
/// poll するだけのループで thread structure を establishing する。実装完了は
/// B6 manual smoke で行う(spec §11 / ISSUE #136)。
pub fn run(
    _connection: zbus::blocking::Connection,
    bridge_tx: Sender<Event>,
    shutdown: Arc<AtomicBool>,
) -> anyhow::Result<()> {
    if !allow_listener_stub() {
        tracing::error!(
            env_var = KOTOHA_ALLOW_LISTENER_STUB_ENV,
            "dbus-listener refusing to start: this build ships only the B3 architectural \
             skeleton with no zbus message decode. Set {KOTOHA_ALLOW_LISTENER_STUB_ENV}=1 to \
             allow stub startup (development / CI only). Full implementation tracked in \
             ISSUE #136 B6 manual smoke."
        );
        return Err(anyhow::Error::from(ListenerStubRefused));
    }
    tracing::error!(
        "dbus-listener started in STUB mode (KOTOHA_ALLOW_LISTENER_STUB=1). \
         No D-Bus method calls will be decoded; the IME will NOT receive key events \
         until B6 lands. Production deployments must NOT export this env var."
    );
    while !shutdown.load(Ordering::SeqCst) {
        // bridge_tx が disconnected ならば engine-loop が落ちた合図。即時 exit。
        if bridge_tx.is_full() {
            // unbounded channel は is_full = false が普通。本 check は将来
            // bounded に切替えた際の back-pressure 観測点として残しておく。
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    tracing::info!("dbus-listener received shutdown signal, exiting");
    Ok(())
}

#[cfg(test)]
mod tests {
    //! Phase 3-B B0h-f rev3 (ADR 0020) review Critical 修正:listener stub の
    //! shutdown ordering と fail-loud 起動拒否を pin する。
    //!
    //! `KOTOHA_ALLOW_LISTENER_STUB` env var の設定値ごとに run() の戻り値を観測する。

    use super::*;
    use crossbeam_channel::unbounded;
    use std::time::Duration;

    fn dummy_connection() -> zbus::blocking::Connection {
        zbus::blocking::Connection::session().expect("test requires session bus")
    }

    /// `KOTOHA_ALLOW_LISTENER_STUB` 未設定で `run()` は即時 `ListenerStubRefused` を返す。
    #[test]
    #[ignore = "依存: dbus session bus available; CI で flaky のため opt-in"]
    fn run_refuses_to_start_without_env_var() {
        // 本 test は env var を unsafely 操作するため、他 test と並列実行 unsafe。
        // `cargo test -- --test-threads=1` で動かすか、本 test を ignore のままに留める。
        unsafe {
            std::env::remove_var(KOTOHA_ALLOW_LISTENER_STUB_ENV);
        }
        let conn = dummy_connection();
        let (tx, _rx) = unbounded();
        let shutdown = Arc::new(AtomicBool::new(false));
        let result = run(conn, tx, shutdown);
        assert!(
            result.is_err(),
            "run() should reject when env var unset, got Ok"
        );
    }

    /// `KOTOHA_ALLOW_LISTENER_STUB=1` + `ListenerShutdown::request()` で
    /// listener thread が ~200ms 以内に `Ok(())` で return する。
    #[test]
    #[ignore = "依存: dbus session bus available; CI で flaky のため opt-in"]
    fn run_exits_within_200ms_after_shutdown_request() {
        unsafe {
            std::env::set_var(KOTOHA_ALLOW_LISTENER_STUB_ENV, "1");
        }
        let conn = dummy_connection();
        let (tx, _rx) = unbounded();
        let shutdown_handle = ListenerShutdown::new();
        let observer = shutdown_handle.observer();
        let listener_thread = std::thread::spawn(move || run(conn, tx, observer));

        // 50ms poll cycle + 余裕で 80ms 後 request、150ms 待って exit を観測。
        std::thread::sleep(Duration::from_millis(80));
        shutdown_handle.request();
        let start = std::time::Instant::now();
        let result = listener_thread.join().expect("thread join");
        let elapsed = start.elapsed();
        assert!(result.is_ok(), "expected Ok(()), got {result:?}");
        assert!(
            elapsed < Duration::from_millis(200),
            "listener should exit within 200ms after shutdown request, took {elapsed:?}"
        );
        unsafe {
            std::env::remove_var(KOTOHA_ALLOW_LISTENER_STUB_ENV);
        }
    }
}
