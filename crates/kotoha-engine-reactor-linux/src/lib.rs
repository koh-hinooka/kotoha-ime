//! `kotoha-engine-reactor-linux` — Linux 専用 `EventReactor` 実装。
//!
//! ADR 0020 §採択 Q3 で確定した crate 境界に従い、本 crate は OS 依存
//! (`crossbeam-channel::select!` macro)を内部に閉じる。`kotoha-engine-core`
//! 側は `EventReactor` trait のみを提供する。
//!
//! # Architecture
//!
//! `LinuxReactor` は 3 つの内部 channel を持ち、`select!` で multiplex する:
//!
//! - `bridge_rx: Receiver<Event>` — `kotoha-dbus-listener` thread から
//! - `worker_rx: Receiver<Event>` — `kotoha-ranker-worker` thread から
//! - `shutdown_rx: Receiver<()>` — main thread から(close で発火)
//!
//! `bridge_tx` / `worker_tx` を別 channel に保つ理由は将来の優先度制御
//! (`select!` の bias)を可能にするため(ADR 0020 §採択 Q4 拡張点)。

use std::time::Duration;

use crossbeam_channel::{select, unbounded, Receiver, RecvError, RecvTimeoutError, Sender};
use kotoha_engine_core::reactor::{Event, EventReactor};

/// Linux 専用の event reactor 実装。
///
/// # Invariants
///
/// - `select!` は crossbeam の semantics(複数 channel 同時 ready 時は擬似乱数選択)
///   により公平性を保つ。arm 順序は documentation 用で、優先度を表現しない
/// - shutdown は `shutdown_rx` の close または全 Sender drop で伝搬する
pub struct LinuxReactor {
    bridge_rx: Receiver<Event>,
    worker_rx: Receiver<Event>,
    shutdown_rx: Receiver<()>,
}

/// `LinuxReactor::new` で同時に得られる producer 側 handle 群。
///
/// main thread が DI wiring 時に保持し、各 producer thread に move する。
/// drop されると `engine-loop` thread が即時 shutdown するため、
/// `#[must_use]` で誤破棄を予防する。
#[must_use = "ReactorHandles を drop すると engine-loop が即時 shutdown する"]
pub struct ReactorHandles {
    pub reactor: LinuxReactor,
    pub bridge_tx: Sender<Event>,
    pub worker_tx: Sender<Event>,
    pub shutdown_tx: Sender<()>,
}

/// 新 `LinuxReactor` + producer handle 群を生成する。
///
/// `LinuxReactor::new()` ではなく自由関数として提供する理由:`new()` は
/// 慣例的に `Self` を返すため、`ReactorHandles` を返すコンストラクタは
/// 関数で表現するのが clippy の `new-ret-no-self` lint 的にも整合する。
///
/// # Postconditions
///
/// - すべての channel は unbounded(IME 用途で send-side back-pressure は不要)
/// - producer 側 Sender は `clone()` 可能(crossbeam-channel 標準)
pub fn start() -> ReactorHandles {
    let (bridge_tx, bridge_rx) = unbounded::<Event>();
    let (worker_tx, worker_rx) = unbounded::<Event>();
    let (shutdown_tx, shutdown_rx) = unbounded::<()>();
    ReactorHandles {
        reactor: LinuxReactor {
            bridge_rx,
            worker_rx,
            shutdown_rx,
        },
        bridge_tx,
        worker_tx,
        shutdown_tx,
    }
}

impl EventReactor for LinuxReactor {
    fn recv(&self) -> Result<Event, RecvError> {
        select! {
            recv(self.bridge_rx) -> ev => ev,
            recv(self.worker_rx) -> ev => ev,
            recv(self.shutdown_rx) -> _ => Ok(Event::Shutdown),
        }
    }

    fn recv_timeout(&self, timeout: Duration) -> Result<Event, RecvTimeoutError> {
        select! {
            recv(self.bridge_rx) -> ev => ev.map_err(RecvTimeoutError::from),
            recv(self.worker_rx) -> ev => ev.map_err(RecvTimeoutError::from),
            recv(self.shutdown_rx) -> _ => Ok(Event::Shutdown),
            default(timeout) => Err(RecvTimeoutError::Timeout),
        }
    }
}
