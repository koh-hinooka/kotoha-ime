//! `WorkerChannel` — `RankerWorker` thread の lifetime 管理 sub-struct。
//!
//! Phase 3-B B0h-c-ii (ISSUE #149 / #163) で `KotohaEngine` から抽出された
//! 3 field を 1 単位に集約した。Phase 3-B B0h-f + B3 (ADR 0020) で worker output
//! 経路は `tx_event: Sender<Event>` で engine-loop に直接送る形に変更され、
//! 旧 `rx_event: Receiver<EngineEvent>` field は撤去された。
//!
//! 本 sub-struct の責務は「worker thread の lifetime と紐付くリソースを 1 単位
//! として保持し、所有権 drop で安全に worker を停止させる」こと。`KotohaEngine`
//! の旧 `Drop` impl は本 sub-struct の `Drop` に移管されている。

use std::io;
use std::thread;

use crossbeam_channel::Sender;

use super::event::RankRequest;
use super::worker;
use crate::reactor::Event;

/// `RankerWorker` の lifetime 管理 sub-struct。
///
/// # Invariants
///
/// - `tx_request` は spawn された worker thread の receiver と接続される
///   (本 struct を経由しない別 channel に差し替えることはできない、
///   constructor 経由のみ)
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
pub(crate) struct WorkerChannel {
    // 注意: 本 struct の field 宣言順序は **load-bearing**(`Drop` semantics に
    // 直接影響する)。`Drop::drop` 完了後の field drop は宣言順で行われるため、
    // `tx_request` が **必ず最初** に drop される必要がある(worker
    // `rx_request.recv()` を Err 復帰させ、joiner thread の `h.join()` を完了
    // 可能にするため)。field を並び替える PR は本 invariant を再検証すること。
    /// engine 主 thread → worker への `RankRequest` 送信 channel。
    pub(crate) tx_request: Sender<RankRequest>,
    /// Worker thread join handle。`Drop` で `take()` し joiner thread に委譲する。
    handle: Option<thread::JoinHandle<()>>,
}

impl WorkerChannel {
    /// `RankerWorker` を新規 spawn し、紐付くリソースを 1 単位として返す。
    ///
    /// # Arguments
    ///
    /// - `worker_event_tx`: worker → engine-loop の `Event` 送信先。caller
    ///   (`KotohaEngine::new`)が `Sender<Event>` の clone を保持する。
    ///
    /// # Postconditions
    ///
    /// - `tx_request` は spawn された worker thread と接続済
    /// - `handle.is_some()`
    ///
    /// # Errors
    ///
    /// - [`io::Error`] — OS が thread spawn を拒否した場合(thread resource 枯渇等)。
    ///   呼び出し側([`super::KotohaEngine::new`])で `Result` 経由 propagate する
    ///   (spec §9.1 row 5)。
    pub(crate) fn new(worker_event_tx: Sender<Event>) -> io::Result<Self> {
        let (tx_request, handle) = worker::spawn_worker(worker_event_tx)?;
        Ok(Self {
            tx_request,
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
