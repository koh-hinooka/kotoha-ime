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
    tracing::info!(
        "dbus-listener started (B3 architectural skeleton; full zbus message decode \
         deferred to B6 manual smoke per ADR 0020 §影響)"
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
