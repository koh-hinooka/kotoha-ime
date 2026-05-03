//! `RankerWorker` — Phase 3-A spec §7 の dedicated background thread。
//!
//! engine 主 thread からの `RankRequest` を mpsc 経由で受け取り、
//! `Ranker::rank` を起動。Ranker 内部の sink 受信を coalescing window で
//! 集約し、`EngineEvent::Candidates` で engine に push する。
//!
//! coalescing window (spec §7.3):
//! - Live: 7ms (5-10ms range の中央値、実装段階 empirical 確定)
//! - Commit: 30ms (LLM 結果待機、second window 150ms で追加 push 受信)

use std::io;
use std::panic::{self, AssertUnwindSafe};
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
///
/// # Errors
///
/// - [`io::Error`] — OS が thread spawn を拒否した場合(thread resource 枯渇等)。
///   呼び出し側([`super::KotohaEngine::new`])で `Result` 経由 propagate し、
///   process 起動を中断させる(spec §9.1 row 5)。
pub(crate) fn spawn_worker() -> io::Result<(
    mpsc::Sender<RankRequest>,
    mpsc::Receiver<EngineEvent>,
    thread::JoinHandle<()>,
)> {
    let (tx_request, rx_request) = mpsc::channel::<RankRequest>();
    let (tx_event, rx_event) = mpsc::channel::<EngineEvent>();
    let handle = thread::Builder::new()
        .name("kotoha-ranker-worker".into())
        .spawn(move || worker_loop(rx_request, tx_event))?;
    Ok((tx_request, rx_event, handle))
}

/// engine 主 thread の receiver が drop された場合に worker_loop を即時終了させる
/// helper。`tx_event.send` が `Err` を返したら debug log を残して `true` を返す。
/// 呼び出し側は `if try_send_event(...) { return; }` の pattern で抜ける。
fn try_send_event(tx_event: &mpsc::Sender<EngineEvent>, ev: EngineEvent, request_id: u64) -> bool {
    if tx_event.send(ev).is_err() {
        tracing::debug!(request_id, "engine receiver dropped; worker exiting");
        return true;
    }
    false
}

fn worker_loop(rx_request: mpsc::Receiver<RankRequest>, tx_event: mpsc::Sender<EngineEvent>) {
    while let Ok(req) = rx_request.recv() {
        let request_id = req.request_id;
        let mode = req.ctx.mode;
        let cancel = req.cancel_dyn();

        let (tx_ranker, rx_ranker) = mpsc::channel::<RankerOutput>();

        // spec §9.1 row 2: Ranker::rank の panic を catch し WorkerError として
        // 報告。worker thread 自体は loop continue で生存させる(セッション全断
        // を避ける)。`AssertUnwindSafe` は Ranker / kana / ctx / cancel / sink
        // が panic 越しに不変であることを caller(本 module)が引き受ける明示。
        let kana = req.kana.clone();
        let ctx = req.ctx.clone();
        let ranker = req.ranker.clone();
        let cancel_for_call = cancel.clone();
        let rank_result = panic::catch_unwind(AssertUnwindSafe(move || {
            ranker.rank(&kana, &ctx, cancel_for_call, tx_ranker)
        }));

        let rank_outcome = match rank_result {
            Ok(Ok(())) => Ok(()),
            Ok(Err(e)) => Err(format!("ranker error: {e}")),
            Err(panic_payload) => {
                let msg = panic_message(&panic_payload);
                tracing::error!(request_id, panic = msg, "ranker panicked");
                Err(format!("ranker panicked: {msg}"))
            }
        };

        if let Err(error) = rank_outcome {
            if try_send_event(
                &tx_event,
                EngineEvent::WorkerError { request_id, error },
                request_id,
            ) {
                return;
            }
            continue;
        }

        // 1st window: dict 候補集約
        let window = match mode {
            ConversionMode::Live => LIVE_WINDOW,
            ConversionMode::Commit => COMMIT_WINDOW,
        };
        let mut buffer: Vec<Candidate> = Vec::new();
        drain_window(&rx_ranker, &cancel, window, &mut buffer);

        // Phase 3-B B0d (Important 8): cancel されていなければ buffer が空でも
        // Replace を送る。engine 側は前回 dispatch の stale 候補を本 Replace で
        // 確実に clear できる。spec §9.3「変換失敗で前回候補が画面に残る」を防ぐ。
        if !cancel.is_cancelled()
            && try_send_event(
                &tx_event,
                EngineEvent::Candidates {
                    request_id,
                    update: CandidateUpdate::Replace(buffer.clone()),
                },
                request_id,
            )
        {
            return;
        }

        // 2nd window for Commit mode: LLM 後続結果
        if mode == ConversionMode::Commit && !cancel.is_cancelled() {
            let mut second_buffer: Vec<Candidate> = Vec::new();
            drain_window(
                &rx_ranker,
                &cancel,
                COMMIT_SECOND_WINDOW,
                &mut second_buffer,
            );
            if !cancel.is_cancelled() && !second_buffer.is_empty() {
                buffer.extend(second_buffer);
                let send_failed = try_send_event(
                    &tx_event,
                    EngineEvent::Candidates {
                        request_id,
                        update: CandidateUpdate::Replace(buffer),
                    },
                    request_id,
                );
                if send_failed {
                    return;
                }
            }
        }
    }
}

/// `catch_unwind` payload から表示用 message を取り出す best-effort helper。
fn panic_message(payload: &Box<dyn std::any::Any + Send>) -> &str {
    if let Some(s) = payload.downcast_ref::<&'static str>() {
        s
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.as_str()
    } else {
        "(non-string panic payload)"
    }
}

/// 指定 window 内に Ranker から届いた `RankerOutput` を `buffer` に集約する。
/// cancel detect で即時 break。
///
/// 本 `rx_ranker` channel は **request 毎に新規作成** される(worker_loop 参照)
/// 前提で、ここに来る output はすべて current request のものとして受け入れる。
/// `RankerOutput` 自体に id は持たない設計(B0e で `request_id` field を撤去)。
/// Stale response の discard は engine 主 thread 側の `RankRequest`/`active_request`
/// ベース id 照合(spec §7.5)で実施する。
fn drain_window(
    rx_ranker: &mpsc::Receiver<RankerOutput>,
    cancel: &Arc<dyn CancellationToken>,
    window: Duration,
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
