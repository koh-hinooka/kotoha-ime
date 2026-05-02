//! `RankerWorker` — Phase 3-A spec §7 の dedicated background thread。
//!
//! engine 主 thread からの `RankRequest` を mpsc 経由で受け取り、
//! `Ranker::rank` を起動。Ranker 内部の sink 受信を coalescing window で
//! 集約し、`EngineEvent::Candidates` で engine に push する。
//!
//! coalescing window (spec §7.3):
//! - Live: 7ms (5-10ms range の中央値、実装段階 empirical 確定)
//! - Commit: 30ms (LLM 結果待機、second window 150ms で追加 push 受信)

use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use kotoha_core::Candidate;

use crate::cancel::CancellationToken;
use crate::ranker::{CandidateUpdate, ConversionMode, RankerOutput};

use super::event::{EngineEvent, RankRequest};

/// Live mode coalescing window(spec §7.3 暫定 7ms)。
pub const LIVE_WINDOW: Duration = Duration::from_millis(7);
/// Commit mode primary coalescing window(spec §7.3 暫定 30ms)。
pub const COMMIT_WINDOW: Duration = Duration::from_millis(30);
/// Commit mode の second window(LLM 後続結果待機、暫定 150ms)。
pub const COMMIT_SECOND_WINDOW: Duration = Duration::from_millis(150);

/// `RankerWorker` を spawn する。
///
/// # Returns
///
/// `(tx_request, rx_event, JoinHandle)`: engine 主 thread が tx_request に
/// `RankRequest` を送り、rx_event から `EngineEvent` を受信する。
///
/// # Thread lifecycle
///
/// - `tx_request` が drop されると worker は loop を抜けて return
/// - `JoinHandle` は engine 側 `Drop` impl で best-effort join
pub(crate) fn spawn_worker() -> (
    mpsc::Sender<RankRequest>,
    mpsc::Receiver<EngineEvent>,
    thread::JoinHandle<()>,
) {
    let (tx_request, rx_request) = mpsc::channel::<RankRequest>();
    let (tx_event, rx_event) = mpsc::channel::<EngineEvent>();
    let handle = thread::Builder::new()
        .name("kotoha-ranker-worker".into())
        .spawn(move || worker_loop(rx_request, tx_event))
        .expect("spawn ranker worker thread");
    (tx_request, rx_event, handle)
}

fn worker_loop(rx_request: mpsc::Receiver<RankRequest>, tx_event: mpsc::Sender<EngineEvent>) {
    while let Ok(req) = rx_request.recv() {
        let request_id = req.request_id;
        let mode = req.ctx.mode;
        let cancel = req.cancel_dyn();

        let (tx_ranker, rx_ranker) = mpsc::channel::<RankerOutput>();
        let rank_result = req
            .ranker
            .rank(&req.kana, &req.ctx, cancel.clone(), tx_ranker);
        if let Err(e) = rank_result {
            let _ = tx_event.send(EngineEvent::WorkerError {
                request_id,
                error: format!("{e}"),
            });
            continue;
        }

        // 1st window: dict 候補集約
        let window = match mode {
            ConversionMode::Live => LIVE_WINDOW,
            ConversionMode::Commit => COMMIT_WINDOW,
        };
        let mut buffer: Vec<Candidate> = Vec::new();
        drain_window(&rx_ranker, &cancel, window, request_id, &mut buffer);

        if !cancel.is_cancelled() && !buffer.is_empty() {
            let _ = tx_event.send(EngineEvent::Candidates {
                request_id,
                update: CandidateUpdate::Replace(buffer.clone()),
            });
        }

        // 2nd window for Commit mode: LLM 後続結果
        if mode == ConversionMode::Commit && !cancel.is_cancelled() {
            let mut second_buffer: Vec<Candidate> = Vec::new();
            drain_window(
                &rx_ranker,
                &cancel,
                COMMIT_SECOND_WINDOW,
                request_id,
                &mut second_buffer,
            );
            if !cancel.is_cancelled() && !second_buffer.is_empty() {
                buffer.extend(second_buffer);
                let _ = tx_event.send(EngineEvent::Candidates {
                    request_id,
                    update: CandidateUpdate::Replace(buffer),
                });
            }
        }
    }
}

/// 指定 window 内に Ranker から届いた `RankerOutput` を `buffer` に集約する。
/// cancel detect で即時 break。
fn drain_window(
    rx_ranker: &mpsc::Receiver<RankerOutput>,
    cancel: &Arc<dyn CancellationToken>,
    window: Duration,
    request_id: u64,
    buffer: &mut Vec<Candidate>,
) {
    let deadline = Instant::now() + window;
    loop {
        let timeout = deadline.saturating_duration_since(Instant::now());
        match rx_ranker.recv_timeout(timeout) {
            Ok(out) => {
                if cancel.is_cancelled() {
                    break;
                }
                if out.request_id != request_id && out.request_id != 0 {
                    continue;
                }
                apply_to_buffer(buffer, out.update);
            }
            Err(RecvTimeoutError::Timeout) | Err(RecvTimeoutError::Disconnected) => break,
        }
    }
}

fn apply_to_buffer(buffer: &mut Vec<Candidate>, update: CandidateUpdate) {
    match update {
        CandidateUpdate::Replace(c) => *buffer = c,
        CandidateUpdate::Append(c) => buffer.extend(c),
        CandidateUpdate::Remove(r) => {
            let len = buffer.len();
            let start = r.start.min(len);
            let end = r.end.min(len);
            if start < end {
                buffer.drain(start..end);
            }
        }
        CandidateUpdate::Clear => buffer.clear(),
    }
}
