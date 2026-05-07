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
/// - `tx_request.is_some()` ⇒ engine 主 thread は worker に request 送信可能。
///   `Drop` 内部で `take()` → drop されると worker `rx_request.recv()` が Err 復帰する。
///
/// # Drop semantics(rev3、ADR 0020 review M architecture fix)
///
/// 旧版は field declaration order に依存して `tx_request` の drop 順を保証して
/// いたが、本 invariant は compile-time enforce 不可能で、`cargo fmt` 等の
/// 機械的 reorder 1 行で永続 thread leak を引き起こすリスクがあった。本版は
/// `Drop::drop` 内部で **explicit に `tx_request.take()` してから handle を join 委譲**
/// する形に切替え、field 順依存を排除する。
///
/// 流れ:
/// 1. `tx_request: Option<Sender<RankRequest>>` を `take()` → drop
/// 2. worker `rx_request.recv()` が `Err` 復帰
/// 3. `handle.take()` → joiner thread で非 blocking join
///
/// これにより:
/// - 親 thread は Drop で blocking しない(joiner thread に委譲)
/// - worker thread は `tx_request` drop 後に確実に exit する
/// - field 順 reorder への耐性を獲得(invariant が compile path 上に明示)
pub(crate) struct WorkerChannel {
    /// engine 主 thread → worker への `RankRequest` 送信 channel。
    /// `Drop` で `take()` して explicit drop する。`Option` ラップにより
    /// 「`Drop::drop` の中で先に手放す」順序を compile-time に表現できる。
    pub(crate) tx_request: Option<Sender<RankRequest>>,
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
    /// - `tx_request.is_some()` で spawn された worker thread と接続済
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
            tx_request: Some(tx_request),
            handle: Some(handle),
        })
    }

    /// engine 主 thread から worker への request 送信。
    ///
    /// `Option` 経由なので `tx_request.take()` 後の send 試行は確実に
    /// `Err(crossbeam_channel::SendError)` を返す(panic ではない)。これにより
    /// shutdown 中の race を deterministic に扱える。
    pub(crate) fn send_request(
        &self,
        req: RankRequest,
    ) -> Result<(), crossbeam_channel::SendError<RankRequest>> {
        match self.tx_request.as_ref() {
            Some(tx) => tx.send(req),
            None => Err(crossbeam_channel::SendError(req)),
        }
    }
}

impl Drop for WorkerChannel {
    /// rev3 (ADR 0020 review fix):field 順依存を排した explicit shutdown。
    /// `tx_request.take()` で先に sender を drop → worker `rx_request.recv()`
    /// が Err 復帰 → joiner thread で非 blocking join、の順を compile path 上に
    /// 明示する。
    fn drop(&mut self) {
        // Step 1: tx_request を明示 drop。これで worker は本 drop 完了を待たずに
        //         exit を開始できる。
        let _ = self.tx_request.take();
        // Step 2: handle を joiner thread で受け取る(親 thread は blocking しない)。
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

#[cfg(test)]
mod tests {
    //! Phase 3-B B0h-f rev3 (ADR 0020) review architecture fix:
    //! `WorkerChannel::Drop` が field 順 reorder に依存せず確実に worker を
    //! 停止させることを deterministic に検証する。
    //!
    //! 旧実装は field 宣言順 (`tx_request` first) に依存していたため、`cargo fmt`
    //! 等で field を並び替えると永続 thread leak が発生していた。本 test は
    //! Drop 完了後に worker thread が exit していることを `tx_event` close 経由
    //! で観測する。

    use super::*;
    use crossbeam_channel::unbounded;
    use std::time::{Duration, Instant};

    use crate::reactor::Event;

    /// `WorkerChannel` を drop してから 200ms 以内に worker thread が exit する。
    /// worker 側は `rx_request.recv()` が `Err(Disconnected)` を返した時点で
    /// loop を抜けるため、`tx_request` の drop が確実に伝わっていることを
    /// 検証する。
    #[test]
    fn drop_terminates_worker_within_timeout() {
        let (worker_event_tx, worker_event_rx) = unbounded::<Event>();
        let channel = WorkerChannel::new(worker_event_tx).expect("spawn worker");
        // worker は tx_event の clone を保持している。channel drop → tx_request
        // drop → worker exit → worker が保持している tx_event 1 件の clone も
        // 自然に drop され、最終的に `worker_event_rx.recv()` が Err を返す。
        drop(channel);

        let start = Instant::now();
        // worker thread の exit を観測する手段:`worker_event_rx.recv_timeout`。
        // worker が生きている間は何も来ない(送る request 無し)、worker が
        // exit して tx_event の最後の clone が drop された時に Err 復帰する。
        let result = worker_event_rx.recv_timeout(Duration::from_millis(500));
        let elapsed = start.elapsed();

        assert!(
            result.is_err(),
            "worker_event_rx should receive Err once worker thread exits, got Ok"
        );
        assert!(
            elapsed < Duration::from_millis(300),
            "worker shutdown should complete within 300ms, took {elapsed:?}"
        );
    }
}
