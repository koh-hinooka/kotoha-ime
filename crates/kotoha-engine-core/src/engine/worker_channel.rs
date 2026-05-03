//! `WorkerChannel` — `RankerWorker` 主 thread と engine 主 thread の channel pair
//! および worker thread join handle を 1 単位として保持する resource owner。
//!
//! Phase 3-B B0h-c-ii (ISSUE #149 / #163) で `KotohaEngine` から抽出された 3 field
//! (`tx_request: mpsc::Sender<RankRequest>` + `rx_event: mpsc::Receiver<EngineEvent>` +
//! `worker_handle: Option<JoinHandle<()>>`)を 1 sub-struct に集約する。
//!
//! 本 sub-struct の責務は **「worker thread の lifetime と紐付く 3 リソースを 1 単位
//! として保持し、所有権 drop で安全に worker を停止させる」** こと。`KotohaEngine::Drop`
//! 旧実装は本 sub-struct の `Drop` impl に移管された(下記 # Drop semantics 参照)。
//!
//! 本 PR (B0h-c-ii) では既存呼び出し側との diff を最小化するため、`tx_request` /
//! `rx_event` は依然 `pub(crate)` で直接 access する。method 経由への抽象化
//! (`send_request` / `try_recv_event` / `recv_event_timeout` 等)は B0h-c-iii で
//! `transitions.rs` の free function method 化と同時に進める。

use std::io;
use std::sync::mpsc;
use std::thread;

use super::event::{EngineEvent, RankRequest};
use super::worker;

/// `RankerWorker` 関連 3 リソースを 1 単位として保持する resource owner。
///
/// # Invariants
///
/// - `tx_request` / `rx_event` は worker thread と接続されている(本 struct を
///   経由しない別 channel に差し替えることはできない、constructor 経由のみ)。
/// - `handle.is_some()` ⇒ worker thread は spawn 済(`new()` の Postconditions)。
///   `Drop` 内部では `take()` で None に遷移する。
///
/// # Drop semantics
///
/// `Drop::drop` で `handle.take()` し、別 thread (`kotoha-ranker-worker-joiner`) で
/// 非 blocking join を行う。Drop::drop 自体が return した後、Rust の field drop 順
/// (declaration 順)で `tx_request` が drop され、worker `rx_request.recv()` が
/// `Err` を返して loop を抜ける。これにより:
///
/// - 親 thread は Drop で blocking しない(joiner thread に委譲)
/// - worker thread は `tx_request` drop 後に確実に exit する
/// - `rx_event` 受信側に取り残された event は drop と共に破棄される
///
/// 旧実装(B0h-c-ii 以前)は同等の logic を `KotohaEngine::Drop` に直接書いていた。
/// 本 PR で resource lifetime と engine state lifecycle を分離したことで、
/// engine 自体の Drop impl は撤去されている。
pub(crate) struct WorkerChannel {
    /// engine 主 thread → worker への `RankRequest` 送信 channel。
    pub(crate) tx_request: mpsc::Sender<RankRequest>,
    /// worker → engine 主 thread への `EngineEvent` 受信 channel。
    pub(crate) rx_event: mpsc::Receiver<EngineEvent>,
    /// Worker thread join handle。`Drop` で `take()` し joiner thread に委譲する。
    handle: Option<thread::JoinHandle<()>>,
}

impl WorkerChannel {
    /// `RankerWorker` を新規 spawn し、紐付く 3 リソースを 1 単位として返す。
    ///
    /// # Postconditions
    ///
    /// - `tx_request` / `rx_event` は spawn された worker thread と接続済
    /// - `handle.is_some()`
    ///
    /// # Errors
    ///
    /// - [`io::Error`] — OS が thread spawn を拒否した場合(thread resource 枯渇等)。
    ///   呼び出し側([`super::KotohaEngine::new`])で `Result` 経由 propagate する
    ///   (spec §9.1 row 5)。
    pub(crate) fn new() -> io::Result<Self> {
        let (tx_request, rx_event, handle) = worker::spawn_worker()?;
        Ok(Self {
            tx_request,
            rx_event,
            handle: Some(handle),
        })
    }
}

impl Drop for WorkerChannel {
    /// `tx_request` を drop することで worker thread が `recv() == Err` を
    /// 検出して loop を抜ける。worker は best-effort で join する(blocking
    /// したくないため、separate thread で待機し、main thread は即時 return)。
    fn drop(&mut self) {
        if let Some(h) = self.handle.take() {
            thread::Builder::new()
                .name("kotoha-ranker-worker-joiner".into())
                .spawn(move || {
                    let _ = h.join();
                })
                .ok();
        }
    }
}
