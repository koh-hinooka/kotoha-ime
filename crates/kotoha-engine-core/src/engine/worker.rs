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

/// 連続 panic 上限。Ranker.rank panic と worker body panic を統合 counter で
/// 監視し、上限到達で worker exit する circuit breaker。
///
/// B0g #148 / 第 2 回 review I9 + I16:
/// - I9 (worker body panic): `apply_to_buffer` / `drain_window` の自前 code が
///   panic した場合、旧実装は worker thread 死亡 → engine 側永続 IME-disabled。
/// - I16 (deterministic Ranker panic): `Ranker::rank` が同一入力で繰り返し
///   panic する場合(例: Mutex poison cascade)、旧実装は keystroke 毎に
///   ERROR log flood + degrade_to_idle ループで user に signal が届かない。
///
/// 本 const は両系統の panic を 1 counter で見て、5 連続で worker thread を
/// exit させる。exit 後は engine 主 thread が `tx_request.send` の Err を
/// 観測し、enabled = false に degrade(spec §9.3「IME-disabled mode を
/// user に通知」)。
const MAX_CONSECUTIVE_PANICS: u32 = 5;

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

/// 1 iteration の処理結果。`worker_loop` の連続 panic counter 制御に使う。
enum IterationOutcome {
    /// Ranker.rank も worker body も panic せず通常終了。counter reset 対象。
    Clean,
    /// Ranker.rank が panic し WorkerError event を送信済。counter increment 対象
    /// (deterministic Ranker panic を I16 として検出する)。
    RankerPanicked,
    /// `tx_event.send` が Err を返した(engine 主 thread 側 receiver drop)。
    /// 即時 worker exit する。
    EngineDisconnected,
}

fn worker_loop(rx_request: mpsc::Receiver<RankRequest>, tx_event: mpsc::Sender<EngineEvent>) {
    let mut consecutive_panics: u32 = 0;
    while let Ok(req) = rx_request.recv() {
        let request_id = req.request_id;

        // B0g #148 / I9: worker body 全体を catch_unwind で wrap。
        // `apply_to_buffer` / `drain_window` 等の自前 code が panic しても
        // worker thread が即死せず、connections / consecutive_panics 越しに
        // 段階的 degrade させる。
        let outcome = panic::catch_unwind(AssertUnwindSafe(|| handle_one_request(req, &tx_event)));

        let panicked = match outcome {
            Ok(IterationOutcome::Clean) => false,
            Ok(IterationOutcome::RankerPanicked) => true,
            Ok(IterationOutcome::EngineDisconnected) => return,
            Err(payload) => {
                let msg = panic_message(&payload);
                tracing::error!(
                    request_id,
                    panic = %msg,
                    "worker_loop body panicked outside Ranker::rank"
                );
                if try_send_event(
                    &tx_event,
                    EngineEvent::WorkerError {
                        request_id,
                        error: format!("worker body panicked: {msg}"),
                    },
                    request_id,
                ) {
                    return;
                }
                true
            }
        };

        if panicked {
            consecutive_panics += 1;
            if consecutive_panics >= MAX_CONSECUTIVE_PANICS {
                tracing::error!(
                    consecutive_panics,
                    "ranker worker thread exiting after {MAX_CONSECUTIVE_PANICS} consecutive panics; \
                     engine main thread will observe channel disconnect on next dispatch"
                );
                return;
            }
        } else {
            consecutive_panics = 0;
        }
    }
}

/// 1 件の `RankRequest` を処理する。`worker_loop` から call される。
///
/// # Returns
///
/// - [`IterationOutcome::Clean`] — 通常終了
/// - [`IterationOutcome::RankerPanicked`] — Ranker.rank が panic し WorkerError 送信済
/// - [`IterationOutcome::EngineDisconnected`] — engine 主 thread の receiver drop
///
/// 本関数自体が panic した場合は `worker_loop` の outer catch_unwind が拾い、
/// I9 として連続 panic counter に加算される。
fn handle_one_request(req: RankRequest, tx_event: &mpsc::Sender<EngineEvent>) -> IterationOutcome {
    let request_id = req.request_id;
    let mode = req.ctx.mode;
    let cancel = req.cancel_dyn();

    let (tx_ranker, rx_ranker) = mpsc::channel::<RankerOutput>();

    // spec §9.1 row 2: Ranker::rank の panic を catch し WorkerError として
    // 報告。worker thread 自体は loop continue で生存させる。`AssertUnwindSafe`
    // は Ranker / kana / ctx / cancel / sink が panic 越しに不変である
    // ことを caller(本 module)が引き受ける明示。
    let kana = req.kana.clone();
    let ctx = req.ctx.clone();
    let ranker = req.ranker.clone();
    let cancel_for_call = cancel.clone();
    let rank_result = panic::catch_unwind(AssertUnwindSafe(move || {
        ranker.rank(&kana, &ctx, cancel_for_call, tx_ranker)
    }));

    let (rank_outcome, ranker_panicked) = match rank_result {
        Ok(Ok(())) => (Ok(()), false),
        Ok(Err(e)) => (Err(format!("ranker error: {e}")), false),
        Err(panic_payload) => {
            let msg = panic_message(&panic_payload);
            tracing::error!(request_id, panic = %msg, "ranker panicked");
            (Err(format!("ranker panicked: {msg}")), true)
        }
    };

    if let Err(error) = rank_outcome {
        if try_send_event(
            tx_event,
            EngineEvent::WorkerError { request_id, error },
            request_id,
        ) {
            return IterationOutcome::EngineDisconnected;
        }
        return if ranker_panicked {
            IterationOutcome::RankerPanicked
        } else {
            IterationOutcome::Clean
        };
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
            tx_event,
            EngineEvent::Candidates {
                request_id,
                update: CandidateUpdate::Replace(buffer.clone()),
            },
            request_id,
        )
    {
        return IterationOutcome::EngineDisconnected;
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
            if try_send_event(
                tx_event,
                EngineEvent::Candidates {
                    request_id,
                    update: CandidateUpdate::Replace(buffer),
                },
                request_id,
            ) {
                return IterationOutcome::EngineDisconnected;
            }
        }
    }
    IterationOutcome::Clean
}

/// `catch_unwind` payload から表示用 message を best-effort で抽出する。
///
/// B0g #148 / 第 2 回 review C4: 旧 impl は `&'static str` / `String` のみ
/// downcast し、`panic_any(anyhow::Error)` や user-defined error 経由の
/// payload が `(non-string panic payload)` で消失していた。本版では
/// payload type id を含めて返し、`kotoha-bin` 側の panic_hook が捕捉した
/// location 情報と組合わせて root cause を辿れるようにする。
///
/// `kotoha-engine-core` は domain crate のため `anyhow::Error` への downcast
/// は実装しない(crate dependency を増やさない)。実際の `anyhow` 表示は
/// `kotoha-bin::panic_message` 側で行う。
fn panic_message(payload: &Box<dyn std::any::Any + Send>) -> String {
    if let Some(s) = payload.downcast_ref::<&'static str>() {
        return (*s).to_string();
    }
    if let Some(s) = payload.downcast_ref::<String>() {
        return s.clone();
    }
    format!(
        "(non-string panic payload, type_id={:?})",
        (**payload).type_id()
    )
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

#[cfg(test)]
mod tests {
    //! Phase 3-B B0g (ISSUE #148 / C4): `panic_message` の payload type 拡充
    //! が `&'static str` / `String` 経由 panic を取りこぼさないこと、未知 type の
    //! payload でも `(non-string panic payload, type_id=...)` で type 情報が
    //! 残ることを観測する。
    use super::*;

    fn capture_panic<F: FnOnce() + std::panic::UnwindSafe>(f: F) -> Box<dyn std::any::Any + Send> {
        panic::catch_unwind(f).expect_err("expected the closure to panic")
    }

    #[test]
    fn panic_message_extracts_static_str() {
        let payload = capture_panic(|| panic!("static panic message"));
        assert_eq!(panic_message(&payload), "static panic message");
    }

    #[test]
    fn panic_message_extracts_owned_string() {
        let owned = String::from("owned panic message");
        let payload = capture_panic(move || panic!("{}", owned));
        assert_eq!(panic_message(&payload), "owned panic message");
    }

    #[test]
    fn panic_message_includes_type_id_for_unknown_payload() {
        // panic_any でカスタム型を投げる。`u32` 自体は std type だが
        // `&'static str` / `String` のいずれにも該当しないため未知 type
        // arm が発火し、type_id が message に含まれることを観測する。
        let payload = capture_panic(|| std::panic::panic_any(42_u32));
        let msg = panic_message(&payload);
        assert!(
            msg.starts_with("(non-string panic payload, type_id="),
            "unexpected panic message: {msg}"
        );
    }
}
