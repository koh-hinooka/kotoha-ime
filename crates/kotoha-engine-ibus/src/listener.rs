//! D-Bus signal listener loop(Phase 3-B B3 + B6-b / ADR 0020 + ADR 0021)。
//!
//! `kotoha-dbus-listener` thread の main を担う。[`build_connection`] が
//! IBus private bus(address discovery 経由、#208 / spec §7.1)への
//! `blocking::Connection` を確立し、`org.freedesktop.IBus.Factory` と
//! `org.freedesktop.IBus.Engine` の 2 interface を serve する。connection は
//! main thread の DI wiring が構築し、clone を signal 発信側
//! (`IBusEngineSignals`)と共有する(spec §3.3 / §7.4)。
//! [`run`] は connection を受領して dispatch 稼働を保持し、shutdown を監視する。
//!
//! # Decoded methods
//!
//! - `ProcessKeyEvent(u keyval, u keycode, u state) -> b` → `Event::IBusKey { event, respond }`
//! - `Reset()` → `Event::IBusReset(IBusResetKind::Reset)`
//! - `FocusOut()` → `Event::IBusReset(IBusResetKind::FocusOut)`
//! - `Disable()` → `Event::IBusReset(IBusResetKind::Disable)`
//! - `FocusIn()` / `Enable()` — daemon 側 introspection 互換 no-op stub
//!
//! 詳細仕様: `docs/specs/_uncategorized/p3-b-ibus-listener.md`、ADR 0021。

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crossbeam_channel::Sender;
use kotoha_engine_core::reactor::Event;

/// Listener thread の shutdown signal の **trigger 側** handle。
///
/// main thread が保持し、`request()` で shutdown を発火する。観測側の
/// [`ShutdownObserver`] は同一の atomic flag を共有し、`is_shutting_down()`
/// で読み取り専用の view を提供する(crossbeam の `(Sender, Receiver)`
/// split-handle pattern と同じ思想)。
///
/// # 旧 API からの移行(#187)
///
/// 旧版は `ListenerShutdown::new() -> Self` + `observer() -> Arc<AtomicBool>`
/// で観測側に `Arc<AtomicBool>` を leak させていた。本版は
/// [`ListenerShutdown::new`] が `(Self, ShutdownObserver)` を返す
/// split-handle に変更し、newtype の中で flag に対する操作 (`store` /
/// `load`) をカプセル化する。これにより observer 側からは `is_shutting_down()`
/// しか呼べず、誤って `flag.store(false, ...)` で trigger 側を打ち消す事故を
/// 構造的に防ぐ。
#[derive(Clone)]
pub struct ListenerShutdown {
    flag: Arc<AtomicBool>,
}

/// Listener thread の shutdown signal の **観測側** handle。
///
/// listener thread が `move` で受け取り、`is_shutting_down()` を polling する
/// 形で利用する。`Clone` を実装するので複数 observer に分配可能だが、本 PR
/// 段階では単一 listener thread のみが消費する。
///
/// 本 newtype は内部 [`Arc<AtomicBool>`] への直接アクセスを公開しない:
/// `Deref` も `as_inner()` も提供せず、`store` で flag を打ち消す手段を
/// 構造的に塞ぐ。
pub struct ShutdownObserver {
    flag: Arc<AtomicBool>,
}

impl ListenerShutdown {
    /// shutdown signal の trigger / observer ペアを生成する。
    ///
    /// 戻り値の `(trigger, observer)` は同一の atomic flag を共有する。
    /// trigger は main thread が保持し、observer は listener thread に
    /// `move` で渡す(典型 usage は本 module の test 参照)。
    #[must_use]
    pub fn new() -> (Self, ShutdownObserver) {
        let flag = Arc::new(AtomicBool::new(false));
        (Self { flag: flag.clone() }, ShutdownObserver { flag })
    }

    /// shutdown を要求する。listener は次 poll サイクルで return する。
    pub fn request(&self) {
        self.flag.store(true, Ordering::SeqCst);
    }
}

impl ShutdownObserver {
    /// listener thread の poll loop が呼ぶ read-only check。
    #[must_use]
    pub fn is_shutting_down(&self) -> bool {
        self.flag.load(Ordering::SeqCst)
    }
}

impl Clone for ShutdownObserver {
    fn clone(&self) -> Self {
        Self {
            flag: self.flag.clone(),
        }
    }
}

/// IBus private bus への connection を確立し、Factory + Engine service を
/// serve する(#208 / spec §7.2)。
///
/// main thread の DI wiring が呼び、clone を `IBusEngineSignals` へ、本体を
/// [`run`] へ配布する(spec §3.3)。
///
/// # Preconditions
///
/// - `bridge_tx` は engine-loop thread の `EventReactor` bridge channel
/// - ibus-daemon が起動済みで private bus address が解決可能であること
///   (env override または address file、spec §7.1)
///
/// # Postconditions
///
/// - private bus への connection 確立、Factory + Engine service publish、
///   bus name `RequestName` 完了(daemon が component 起動完了を検知する)
///
/// # Errors
///
/// - [`crate::discovery::DiscoveryError`] — address discovery 失敗(spec §9.1)
/// - `zbus::Error` — private bus 接続 / `serve_at` / `RequestName` 失敗
pub fn build_connection(bridge_tx: Sender<Event>) -> anyhow::Result<zbus::blocking::Connection> {
    use crate::discovery::discover_ibus_address;
    use crate::factory::KotohaFactoryService;
    use crate::proxy::{IBUS_ENGINE_BUS_NAME, IBUS_ENGINE_OBJECT_PATH, IBUS_FACTORY_OBJECT_PATH};
    use crate::service::KotohaEngineService;

    let address = discover_ibus_address()?;
    tracing::info!(
        bus_name = IBUS_ENGINE_BUS_NAME,
        engine_path = IBUS_ENGINE_OBJECT_PATH,
        factory_path = IBUS_FACTORY_OBJECT_PATH,
        "connecting to IBus private bus (zbus blocking::Builder + #[interface] dispatcher)"
    );
    let connection = zbus::blocking::connection::Builder::address(address.as_str())?
        .serve_at(IBUS_FACTORY_OBJECT_PATH, KotohaFactoryService)?
        .serve_at(IBUS_ENGINE_OBJECT_PATH, KotohaEngineService { bridge_tx })?
        .name(IBUS_ENGINE_BUS_NAME)?
        .build()?;
    Ok(connection)
}

/// D-Bus listener thread の main loop。
///
/// # Preconditions
///
/// - `connection` は [`build_connection`] で構築済み(main thread の DI wiring、
///   #208 / spec §3.3)
/// - `shutdown` は main thread が `ListenerShutdown::request()` で停止指示する
///   observer handle(#187 split-handle、#197 SIGTERM hook 経由でも request される)
///
/// # Postconditions
///
/// - `shutdown.is_shutting_down()` を観測したら保持 handle を drop して `Ok(())`
///   で return する(`IBusEngineSignals` 側 clone を含む全 handle drop で zbus
///   internal smol executor が exit、bus name が release)
///
/// 詳細仕様: `docs/specs/_uncategorized/p3-b-ibus-listener.md` §3 / §7。
pub fn run(
    connection: zbus::blocking::Connection,
    shutdown: ShutdownObserver,
) -> anyhow::Result<()> {
    // Connection's internal smol executor dispatches incoming method calls in a
    // background thread. We just hold the connection alive and poll shutdown.
    while !shutdown.is_shutting_down() {
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    tracing::info!("dbus-listener received shutdown signal, dropping connection and exiting");
    drop(connection); // zbus releases bus name + stops dispatching (last handle).
    Ok(())
}

#[cfg(test)]
mod tests {
    //! Phase 3-B B6-b (#195) — ShutdownObserver split-handle (#187) と
    //! `ListenerShutdown::clone` (#197 SIGTERM hook) の不変条件 pin。
    //!
    //! `run()` 自体の挙動 test は dbus session bus を要求するため
    //! `crates/kotoha-engine-ibus/tests/integration.rs` (`#[ignore]`) で扱う。

    use super::*;

    /// `ShutdownObserver::is_shutting_down()` は trigger からの `request()` を
    /// 観測する。本 test は dbus session bus に依存せず実行可能。
    #[test]
    fn observer_reflects_trigger_state() {
        let (trigger, observer) = ListenerShutdown::new();
        assert!(
            !observer.is_shutting_down(),
            "fresh observer should not signal shutdown"
        );
        trigger.request();
        assert!(
            observer.is_shutting_down(),
            "after trigger.request(), observer must signal shutdown"
        );
    }

    /// `ShutdownObserver::clone()` は同一 atomic flag を共有する。
    #[test]
    fn cloned_observer_shares_state() {
        let (trigger, observer1) = ListenerShutdown::new();
        let observer2 = observer1.clone();
        assert!(!observer1.is_shutting_down());
        assert!(!observer2.is_shutting_down());
        trigger.request();
        assert!(observer1.is_shutting_down());
        assert!(observer2.is_shutting_down());
    }

    /// `ListenerShutdown::clone()` は同一 atomic flag を共有する(#197 SIGTERM
    /// hook で signal handler closure に trigger を move するため Clone を要求)。
    #[test]
    fn cloned_trigger_shares_state() {
        let (trigger1, observer) = ListenerShutdown::new();
        let trigger2 = trigger1.clone();
        assert!(!observer.is_shutting_down());
        // trigger1 が request しても trigger2 経由でも observer に伝わる
        trigger2.request();
        assert!(
            observer.is_shutting_down(),
            "request via cloned trigger must reach observer"
        );
    }
}
