//! `KotohaEngine` 状態機械 — Phase 3-A spec §5 / §6 を実装する core engine。
//!
//! 本 module は M2 段階で **synchronous Ranker** path を実装する。M3 で
//! `RankerWorker` 背景 thread に置き換える(Ranker は `rank()` 内で sink
//! に送り、本 method はその受信を recv_timeout で drain する)。
//!
//! spec §5.2 の状態遷移 table を `process_key_event` 内の dispatch helper
//! ([`transitions::dispatch_key`])で網羅する。

use std::io;
use std::sync::mpsc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use kotoha_core::romaji::RomajiConverter;
use kotoha_core::Candidate;

use crate::cancel::{CancellationToken, StdCancellationToken};
use crate::host_bridge::IMEHostBridge;
use crate::ime_engine::IMEEngine;
use crate::key_event::{KeyEvent, KeyEventResult};
use crate::ranker::{CandidateUpdate, ConversionContext, ConversionMode, Ranker};

mod commit_history;
mod event;
#[doc(hidden)]
pub mod transitions;
mod worker;

pub use commit_history::CommitHistory;

/// `KotohaEngine` の現在状態(spec §5.1)。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EngineState {
    /// preedit 空、候補 hidden、active_request 無
    Idle,
    /// preedit に kana 有り、Live 変換 in-flight
    LiveConverting,
    /// space 後の commit-mode RankRequest in-flight、候補未到着
    CommitConverting,
    /// commit 候補表示中、user navigation / Enter / Esc 待ち
    CandidatesShown,
}

/// In-flight RankRequest の handle(`active_request` field 用、spec §7.1)。
///
/// `id` field は `RankerWorker` の `request_id` mismatch discard(spec §7.5)で
/// engine 主 thread side の safety net として `EngineEvent::Candidates` の
/// `request_id` と照合する。
pub(crate) struct RequestHandle {
    pub(crate) id: u64,
    pub(crate) cancel_token: Arc<StdCancellationToken>,
}

/// `IMEEngine` impl の本体。spec §3.3 全体図 / §5 状態機械 / §6 data flow に対応。
///
/// # Construction
///
/// `Box<dyn IMEHostBridge>` / `Arc<dyn Ranker>` /
/// `Arc<dyn LearningCacheWriter>` を構築時に DI で受け取る([`Self::new`])。
///
/// # Invariants
///
/// - `state == Idle` ⇒ `current_preedit.is_empty() && active_request.is_none()`
/// - `active_request.is_some()` ⇒ `cancel_token` が一意に存在
/// - `commit_history.total_chars() <= 200`([`CommitHistory::push`] で維持)
pub struct KotohaEngine {
    pub(crate) state: EngineState,
    pub(crate) host: Box<dyn IMEHostBridge>,
    pub(crate) ranker: Arc<dyn Ranker>,
    pub(crate) learning_writer: Arc<dyn kotoha_storage::learning_cache::LearningCacheWriter>,
    pub(crate) romaji: RomajiConverter,
    pub(crate) current_preedit: String,
    pub(crate) commit_history: CommitHistory,
    pub(crate) candidates: Vec<Candidate>,
    pub(crate) highlight_idx: usize,
    pub(crate) active_request: Option<RequestHandle>,
    pub(crate) request_id_seed: u64,
    pub(crate) last_commit_at: Instant,
    pub(crate) enabled: bool,
    pub(crate) focused: bool,
    /// `RankerWorker` 主 thread への request 送信 channel。
    pub(crate) tx_request: mpsc::Sender<event::RankRequest>,
    /// `RankerWorker` から engine 主 thread への event 受信 channel。
    pub(crate) rx_event: mpsc::Receiver<event::EngineEvent>,
    /// Worker thread join handle(`Drop` impl で best-effort 終了)。
    pub(crate) worker_handle: Option<std::thread::JoinHandle<()>>,
}

impl KotohaEngine {
    /// 新 engine を構築する。host / ranker / learning_writer を DI で受ける。
    ///
    /// # Postconditions
    ///
    /// - `state == EngineState::Idle`
    /// - `enabled == false`(host が `enable()` を呼ぶまで no-op + `Forwarded`)
    /// - `focused == false`
    ///
    /// # Errors
    ///
    /// - [`io::Error`] — `RankerWorker` 用 OS thread の spawn が失敗した場合
    ///   (thread resource 枯渇等)。spec §9.1 row 5 に従い、起動失敗は呼び出し側
    ///   (`kotoha-bin`)で `Err` 経由 propagate し process を中断させる。以前の
    ///   実装は `.expect()` で panic していたが、process-wide panic は
    ///   `catch_unwind` 経路に乗らないため非推奨。
    pub fn new(
        host: Box<dyn IMEHostBridge>,
        ranker: Arc<dyn Ranker>,
        learning_writer: Arc<dyn kotoha_storage::learning_cache::LearningCacheWriter>,
    ) -> io::Result<Self> {
        let (tx_request, rx_event, worker_handle) = worker::spawn_worker()?;
        Ok(Self {
            state: EngineState::Idle,
            host,
            ranker,
            learning_writer,
            romaji: RomajiConverter::new(),
            current_preedit: String::new(),
            commit_history: CommitHistory::new(),
            candidates: Vec::new(),
            highlight_idx: 0,
            active_request: None,
            request_id_seed: 0,
            last_commit_at: Instant::now(),
            enabled: false,
            focused: false,
            tx_request,
            rx_event,
            worker_handle: Some(worker_handle),
        })
    }

    /// 次 request_id を採番する(64-bit 単調増加、spec §7.5)。
    pub(crate) fn next_request_id(&mut self) -> u64 {
        self.request_id_seed = self.request_id_seed.wrapping_add(1);
        self.request_id_seed
    }

    /// active_request の cancel_token を fire し take する(状態遷移補助)。
    pub(crate) fn cancel_active(&mut self) {
        if let Some(handle) = self.active_request.take() {
            handle.cancel_token.cancel();
        }
    }

    /// Live or Commit mode の RankRequest を発行し、`active_request` を更新する。
    ///
    /// 本 method は M3 段階で `RankerWorker` 背景 thread に dispatch し、
    /// 第 1 候補 batch が `coalescing window + safety` 以内に到着するまで
    /// blocking で待機する。Commit mode の second window(LLM 後続結果)は
    /// 後続 `process_key_event` 呼び出しの先頭で `drain_pending_events()`
    /// が拾う設計で、本 method 内では待機しない(M3 簡略化、Phase 3-A
    /// 実装段階で擾乱検出して再評価)。
    pub(crate) fn dispatch_rank_request(&mut self, mode: ConversionMode) {
        let request_id = self.next_request_id();
        let cancel_token = Arc::new(StdCancellationToken::new());
        let ctx = ConversionContext {
            commit_history: self.commit_history.snapshot(),
            time_since_last_commit: self.last_commit_at.elapsed(),
            mode,
        };
        self.active_request = Some(RequestHandle {
            id: request_id,
            cancel_token: cancel_token.clone(),
        });

        let req = event::RankRequest {
            request_id,
            kana: self.current_preedit.clone(),
            ctx,
            cancel_token,
            ranker: self.ranker.clone(),
        };
        if self.tx_request.send(req).is_err() {
            // worker thread が死亡している。spec §9.1 row 2 に従い、
            // engine 状態を Idle に戻して候補ウィンドウを閉じ、stale な
            // active_request を残さない(後続 keystroke の cancel_active が
            // phantom request を握って残響しないようにする)。
            //
            // B0g #148 / I16: worker は連続 panic 上限到達で exit する circuit
            // breaker を持つ。本 path に到達したら、engine 全体を IME-disabled
            // に倒して keystroke ごとの ERROR log flood を停止し、user が
            // 「IME が無効化された」を察知できるようにする(spec §9.3
            // 「IME-disabled mode を user に通知」)。
            tracing::error!(
                request_id,
                "ranker worker channel closed; engine going to IME-disabled (consecutive panic threshold reached \
                 or worker exited unexpectedly)"
            );
            self.active_request = None;
            self.candidates.clear();
            self.highlight_idx = 0;
            self.host.hide_candidate_window();
            self.state = EngineState::Idle;
            self.enabled = false;
            return;
        }

        // 第 1 候補 batch を待つ。
        // safety margin: thread 起動 + Ranker.rank 同期 path + worker 処理。
        let max_wait = match mode {
            ConversionMode::Live => worker::LIVE_WINDOW + Duration::from_millis(5),
            ConversionMode::Commit => worker::COMMIT_WINDOW + Duration::from_millis(5),
        };
        self.candidates.clear();
        self.drain_events_blocking(max_wait, request_id);
    }

    /// spec §5.2 row 7 (CommitConverting + RankerOutput → CandidatesShown) の
    /// 反映 helper。`Candidates` event を apply_candidate_update した直後に
    /// 呼び、CommitConverting 中で候補非空なら CandidatesShown へ遷移して
    /// `update_candidates` を host に発行する。
    fn maybe_promote_commit_to_candidates_shown(&mut self) {
        if self.state == EngineState::CommitConverting && !self.candidates.is_empty() {
            self.host
                .update_candidates(CandidateUpdate::Replace(self.candidates.clone()));
            self.state = EngineState::CandidatesShown;
        }
    }

    /// worker 死亡 / WorkerError 発生時の共通 Idle 復帰処理。
    fn degrade_to_idle(&mut self) {
        self.active_request = None;
        self.candidates.clear();
        self.highlight_idx = 0;
        self.host.hide_candidate_window();
        self.state = EngineState::Idle;
    }

    /// `rx_event` から最大 `max_wait` まで待ち、第 1 Candidates(target_id 一致)を
    /// engine state に反映して return する。
    ///
    /// - WorkerError target_id 一致時は spec §9.1 row 2 に従い `host.hide_candidate_window()`
    ///   + state Idle に戻し、`tracing::error!` を残して return。
    /// - WorkerError target_id 不一致時は loop 継続(別 request の error)。
    /// - `RecvTimeoutError::Timeout` は normal path で return。
    /// - `RecvTimeoutError::Disconnected` は worker thread 死亡を意味し、spec §9.1 row 2
    ///   に従い engine 状態を Idle に戻し host を閉じて return。
    /// - `Candidates` で CommitConverting 中なら `CandidatesShown` 遷移を併発する
    ///   (spec §5.2 row 7、Phase 3-B B0d Critical 4 async path 修正)。
    pub(crate) fn drain_events_blocking(&mut self, max_wait: Duration, target_id: u64) {
        let deadline = Instant::now() + max_wait;
        while let Some(remaining) = deadline.checked_duration_since(Instant::now()) {
            match self.rx_event.recv_timeout(remaining) {
                Ok(event::EngineEvent::Candidates { request_id, update }) => {
                    if request_id != target_id {
                        continue;
                    }
                    self.apply_candidate_update(update);
                    self.maybe_promote_commit_to_candidates_shown();
                    return;
                }
                Ok(event::EngineEvent::WorkerError { request_id, error }) => {
                    tracing::error!(request_id, error, "ranker worker error");
                    if request_id == target_id {
                        self.degrade_to_idle();
                        return;
                    }
                }
                Err(mpsc::RecvTimeoutError::Timeout) => return,
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    tracing::error!(
                        target_id,
                        "ranker worker channel disconnected; engine degrading to Idle"
                    );
                    self.degrade_to_idle();
                    return;
                }
            }
        }
    }

    /// `rx_event` に蓄積されている event を非 blocking で全 drain する
    /// (`process_key_event` 先頭で呼び出し、Commit mode second window で
    /// 到着した LLM 結果等 + 遅延 dict 候補を反映する)。
    ///
    /// `try_recv` が `Disconnected` を返した場合は worker 死亡で、
    /// `drain_events_blocking` と同等の Idle 復帰処理を行う。
    /// `Candidates` で CommitConverting 中なら `CandidatesShown` へ遷移する
    /// (spec §5.2 row 7)。
    pub(crate) fn drain_pending_events(&mut self) {
        let active_id = self.active_request.as_ref().map(|h| h.id);
        loop {
            match self.rx_event.try_recv() {
                Ok(event::EngineEvent::Candidates { request_id, update }) => {
                    if active_id != Some(request_id) {
                        continue; // mismatch discard
                    }
                    self.apply_candidate_update(update);
                    self.maybe_promote_commit_to_candidates_shown();
                }
                Ok(event::EngineEvent::WorkerError { request_id, error }) => {
                    tracing::error!(request_id, error, "ranker worker error");
                    if active_id == Some(request_id) {
                        self.degrade_to_idle();
                    }
                }
                Err(mpsc::TryRecvError::Empty) => return,
                Err(mpsc::TryRecvError::Disconnected) => {
                    tracing::error!(
                        ?active_id,
                        "ranker worker channel disconnected; engine degrading to Idle"
                    );
                    self.degrade_to_idle();
                    return;
                }
            }
        }
    }

    /// `CandidateUpdate` を `self.candidates` に適用する(spec §4.2)。
    pub(crate) fn apply_candidate_update(&mut self, update: CandidateUpdate) {
        match update {
            CandidateUpdate::Replace(c) => {
                self.candidates = c;
                self.highlight_idx = 0;
            }
            CandidateUpdate::Append(c) => self.candidates.extend(c),
            CandidateUpdate::Remove(r) => {
                let len = self.candidates.len();
                let start = r.start.min(len);
                let end = r.end.min(len);
                if start < end {
                    self.candidates.drain(start..end);
                }
                if self.highlight_idx >= self.candidates.len() {
                    self.highlight_idx = self.candidates.len().saturating_sub(1);
                }
            }
            CandidateUpdate::Clear => {
                self.candidates.clear();
                self.highlight_idx = 0;
            }
        }
    }
}

impl IMEEngine for KotohaEngine {
    fn process_key_event(&mut self, key: KeyEvent) -> KeyEventResult {
        if !self.enabled || !self.focused {
            return KeyEventResult::Forwarded;
        }
        // RELEASE event は engine が処理しない(IBus は press/release 双方を渡すため、
        // press path のみで状態遷移を駆動する。Phase 3-B B4)。
        if key
            .modifiers
            .contains(crate::key_event::KeyModifiers::RELEASE)
        {
            return KeyEventResult::Forwarded;
        }
        // Commit mode second window などで遅延到着した event を最初に取り込む。
        self.drain_pending_events();
        transitions::dispatch_key(self, key)
    }

    fn focus_in(&mut self) {
        self.focused = true;
        // spec §5.3: focus_in は engine 状態を変えない(Idle 維持)
    }

    fn focus_out(&mut self) {
        // spec §5.2: focus_out は cancel + clear + Idle
        self.cancel_active();
        self.current_preedit.clear();
        self.romaji.reset_pending();
        self.commit_history.clear();
        self.candidates.clear();
        self.highlight_idx = 0;
        self.host.hide_candidate_window();
        self.host.update_preedit("", 0, false);
        self.state = EngineState::Idle;
        self.focused = false;
    }

    fn reset(&mut self) {
        // spec §5.2: reset は focus_out と同等処理
        self.cancel_active();
        self.current_preedit.clear();
        self.romaji.reset_pending();
        self.commit_history.clear();
        self.candidates.clear();
        self.highlight_idx = 0;
        self.host.hide_candidate_window();
        self.host.update_preedit("", 0, false);
        self.state = EngineState::Idle;
    }

    fn enable(&mut self) {
        self.enabled = true;
    }

    fn disable(&mut self) {
        self.cancel_active();
        self.current_preedit.clear();
        self.romaji.reset_pending();
        self.candidates.clear();
        self.highlight_idx = 0;
        self.host.hide_candidate_window();
        self.host.update_preedit("", 0, false);
        self.state = EngineState::Idle;
        self.enabled = false;
    }
}

impl Drop for KotohaEngine {
    /// `tx_request` を drop することで worker thread が `recv() == Err` を
    /// 検出して loop を抜ける。worker は best-effort で join する(blocking
    /// したくないため、separate thread で待機し、main thread は即時 return)。
    fn drop(&mut self) {
        if let Some(h) = self.worker_handle.take() {
            std::thread::Builder::new()
                .name("kotoha-ranker-worker-joiner".into())
                .spawn(move || {
                    let _ = h.join();
                })
                .ok();
        }
    }
}

#[cfg(feature = "test-helpers")]
impl KotohaEngine {
    /// Test-only inspector: 現在 preedit の clone を返す。
    pub fn preedit_for_test(&self) -> String {
        self.current_preedit.clone()
    }
    /// Test-only inspector: 現在 candidate 数を返す。
    pub fn candidate_count_for_test(&self) -> usize {
        self.candidates.len()
    }
    /// Test-only inspector: 現在 highlight idx を返す。
    pub fn highlight_idx_for_test(&self) -> usize {
        self.highlight_idx
    }
    /// Test-only inspector: 現在 state を返す。
    pub fn state_for_test(&self) -> EngineState {
        self.state
    }
    /// Test-only inspector: 現在 enabled flag を返す。
    ///
    /// B0g #148 / I16 で「worker 連続 panic → engine が IME-disabled に degrade」
    /// を assert するための accessor。
    pub fn enabled_for_test(&self) -> bool {
        self.enabled
    }
}
