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

use crate::cancel::{CancellationToken, StdCancellationToken};
use crate::host_bridge::IMEHostBridge;
use crate::ime_engine::IMEEngine;
use crate::key_event::{KeyEvent, KeyEventResult};
use crate::ranker::{CandidateUpdate, ConversionContext, ConversionMode, Ranker};

mod candidates;
mod commit_history;
mod event;
mod preedit;
#[doc(hidden)]
pub mod transitions;
mod worker;
mod worker_channel;

use candidates::CandidateBuffer;
use preedit::PreeditBuffer;
use worker_channel::WorkerChannel;

pub use commit_history::CommitHistory;
pub use worker::panic_message_from;

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
/// `Arc<dyn LearningRecorder>` を構築時に DI で受け取る([`Self::new`])。
///
/// # Invariants
///
/// - `state == Idle` ⇒ `preedit.current.is_empty() && active_request.is_none()`
/// - `active_request.is_some()` ⇒ `cancel_token` が一意に存在
/// - `commit_history.total_chars() <= 200`([`CommitHistory::push`] で維持)
///
/// # Phase 3-B B0h-c-i (ISSUE #149 / #161) / B0h-c-ii (#163) field 整理
///
/// 旧 7 field を 3 sub-struct に集約:
/// - `current_preedit: String` + `romaji: RomajiConverter` →
///   [`preedit::PreeditBuffer`] (`engine.preedit.current` / `engine.preedit.romaji`)
/// - `candidates: Vec<Candidate>` + `highlight_idx: usize` →
///   [`candidates::CandidateBuffer`] (`engine.candidates.items` /
///   `engine.candidates.highlight`)
/// - `tx_request` + `rx_event` + `worker_handle` →
///   [`worker_channel::WorkerChannel`] (`engine.worker.tx_request` /
///   `engine.worker.rx_event`)。本 sub-struct が `Drop` impl を持つため
///   `KotohaEngine::Drop` は撤去された。
///
/// 残 8 field (`state` / `host` / `ranker` / `learning_writer` /
/// `commit_history` / `active_request` / `request_id_seed` / `last_commit_at` /
/// `enabled` / `focused`) は B0h-c-iii (`LearningSink` +
/// transitions.rs method 化) で順次抽出する。
pub struct KotohaEngine {
    pub(crate) state: EngineState,
    pub(crate) host: Box<dyn IMEHostBridge>,
    pub(crate) ranker: Arc<dyn Ranker>,
    pub(crate) learning_writer: Arc<dyn crate::learning_port::LearningRecorder>,
    pub(crate) preedit: PreeditBuffer,
    pub(crate) commit_history: CommitHistory,
    pub(crate) candidates: CandidateBuffer,
    pub(crate) active_request: Option<RequestHandle>,
    pub(crate) request_id_seed: u64,
    pub(crate) last_commit_at: Instant,
    pub(crate) enabled: bool,
    pub(crate) focused: bool,
    /// `RankerWorker` 主 thread と engine 主 thread を結ぶ channel pair および
    /// worker thread join handle。本 sub-struct の `Drop` impl が `tx_request`
    /// drop → worker exit → 別 thread で join の順で safe shutdown を行う。
    pub(crate) worker: WorkerChannel,
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
        learning_writer: Arc<dyn crate::learning_port::LearningRecorder>,
    ) -> io::Result<Self> {
        let worker = WorkerChannel::new()?;
        Ok(Self {
            state: EngineState::Idle,
            host,
            ranker,
            learning_writer,
            preedit: PreeditBuffer::new(),
            commit_history: CommitHistory::new(),
            candidates: CandidateBuffer::new(),
            active_request: None,
            request_id_seed: 0,
            last_commit_at: Instant::now(),
            enabled: false,
            focused: false,
            worker,
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
            kana: self.preedit.current.clone(),
            ctx,
            cancel_token,
            ranker: self.ranker.clone(),
        };
        if self.worker.tx_request.send(req).is_err() {
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
        // B0g-c #148 / silent_ranker e2e fix:dispatch 入口時点で表示中だった
        // 候補があり、本 dispatch を経ても engine.candidates が空のまま終わった
        // 場合(silent ranker / 全 backend 失敗 + timeout / filter all-unsafe
        // 等)、host 側 lookup table も明示的に Clear + hide する(spec §9.3
        // 「変換失敗で前回候補が画面に残る」防止)。
        //
        // 注意:`apply_candidate_update` Replace path にも prior_was_nonempty
        // 検出があるが、本関数の `self.candidates.clear()` で apply 時点では
        // prior=empty になるため、runtime path では apply 側の host 通知は
        // 発火しない。direct-call test (`apply_candidate_update_for_test`) が
        // 単体 API として apply boundary を assert するために apply 側の
        // 検出は維持する(両者の発火条件は排他、二重通知はしない)。
        let had_displayed_before_dispatch = !self.candidates.items.is_empty();
        self.candidates.clear();
        self.drain_events_blocking(max_wait, request_id);
        if had_displayed_before_dispatch && self.candidates.items.is_empty() {
            self.host.update_candidates(CandidateUpdate::Clear);
            self.host.hide_candidate_window();
        }
    }

    /// spec §5.2 row 7 (CommitConverting + RankerOutput → CandidatesShown) の
    /// 反映 helper。`Candidates` event を apply_candidate_update した直後に
    /// 呼び、CommitConverting 中で候補非空なら CandidatesShown へ遷移して
    /// `update_candidates` を host に発行する。
    fn maybe_promote_commit_to_candidates_shown(&mut self) {
        if self.state == EngineState::CommitConverting && !self.candidates.items.is_empty() {
            self.host
                .update_candidates(CandidateUpdate::Replace(self.candidates.items.clone()));
            self.state = EngineState::CandidatesShown;
        }
    }

    /// worker 死亡 / WorkerError 発生時の共通 Idle 復帰処理。
    fn degrade_to_idle(&mut self) {
        self.active_request = None;
        self.candidates.clear();
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
            match self.worker.rx_event.recv_timeout(remaining) {
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
            match self.worker.rx_event.try_recv() {
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
    ///
    /// B0g-b #148 / 第 2 回 review I8: `Replace` / `Append` 経由で流入する
    /// candidate は ranker 内部の SudachiDict / UserVocab / LearningCache /
    /// LLM 由来。改ざん辞書 / 悪意ある LLM 出力 / Phase 5 custom model 等から
    /// ANSI escape / NUL byte / RTL override が混入しないよう engine 境界で
    /// reject filter する(`crate::sanitize::is_safe_for_host`)。
    ///
    /// # Empty Replace の host 通知 (spec §9.3「前回候補画面残留」防止)
    ///
    /// 旧 `transitions.rs` 各 caller は `if !engine.candidates.is_empty() {
    /// host.update_candidates(...) }` で gate していたため、本 method 単体で
    /// **直前の候補が host 側に残留したまま** となる 2 つの silent failure
    /// 経路が存在した:
    ///
    /// - B0g-b F1: filter で全候補 drop された場合(I8)
    /// - B0g-c I10: Ranker.rank が `sink.send` を 1 回も呼ばず、worker が空 Replace
    ///   を engine に転送した場合(silent ranker)
    ///
    /// 統一基準として「**直前 `self.candidates` が non-empty で、適用後に
    /// 空になった場合**」に host へ `update_candidates(Clear)` + `hide_candidate_window`
    /// を発行する。直前から空の場合は host 側もすでに hide 状態のため発火しない。
    pub(crate) fn apply_candidate_update(&mut self, update: CandidateUpdate) {
        match update {
            CandidateUpdate::Replace(c) => {
                let prior_was_nonempty = !self.candidates.items.is_empty();
                let filtered = filter_safe_candidates(c);
                self.candidates.items = filtered;
                self.candidates.highlight = 0;
                if prior_was_nonempty && self.candidates.items.is_empty() {
                    self.host.update_candidates(CandidateUpdate::Clear);
                    self.host.hide_candidate_window();
                }
            }
            CandidateUpdate::Append(c) => {
                // Append は self.candidates.items に extend するため:
                // - prior=non-empty + extend(任意) → post=non-empty(Clear 不要)
                // - prior=empty + extend(空) → post=empty(host も既に hide 状態のため通知不要)
                // - prior=empty + extend(非空) → post=non-empty(caller transitions.rs が
                //   `if !candidates.items.is_empty()` で host.update_candidates を出す経路が存在)
                // よって本 path 内に host.Clear + hide を fire する必要のある branch は
                // 存在しない(self-review#3 で dead code 撤去、Replace path 限定の規約)。
                let filtered = filter_safe_candidates(c);
                self.candidates.items.extend(filtered);
            }
            CandidateUpdate::Remove(r) => {
                let len = self.candidates.items.len();
                let start = r.start.min(len);
                let end = r.end.min(len);
                if start < end {
                    self.candidates.items.drain(start..end);
                }
                if self.candidates.highlight >= self.candidates.items.len() {
                    self.candidates.highlight = self.candidates.items.len().saturating_sub(1);
                }
            }
            CandidateUpdate::Clear => {
                self.candidates.clear();
            }
        }
    }
}

/// candidate Vec から `surface` が unsafe な要素を除去する filter。
///
/// 1 つでも reject した場合は `tracing::error!` で件数を観測する(silent
/// failure 禁止 / spec §9.3)。
fn filter_safe_candidates(input: Vec<kotoha_core::Candidate>) -> Vec<kotoha_core::Candidate> {
    let original_len = input.len();
    let filtered: Vec<_> = input
        .into_iter()
        .filter(|c| crate::sanitize::is_safe_for_host(&c.surface))
        .collect();
    let dropped = original_len - filtered.len();
    if dropped > 0 {
        tracing::error!(
            dropped,
            kept = filtered.len(),
            "dropped candidates with unsafe control/bidi/escape characters at engine boundary; \
             check ranker source (dict / LLM / user vocab) for tainted input"
        );
    }
    filtered
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
        self.preedit.clear();
        self.commit_history.clear();
        self.candidates.clear();
        self.host.hide_candidate_window();
        self.host.update_preedit("", 0, false);
        self.state = EngineState::Idle;
        self.focused = false;
    }

    fn reset(&mut self) {
        // spec §5.2: reset は focus_out と同等処理
        self.cancel_active();
        self.preedit.clear();
        self.commit_history.clear();
        self.candidates.clear();
        self.host.hide_candidate_window();
        self.host.update_preedit("", 0, false);
        self.state = EngineState::Idle;
    }

    fn enable(&mut self) {
        self.enabled = true;
    }

    fn disable(&mut self) {
        self.cancel_active();
        self.preedit.clear();
        self.candidates.clear();
        self.host.hide_candidate_window();
        self.host.update_preedit("", 0, false);
        self.state = EngineState::Idle;
        self.enabled = false;
    }
}

// worker thread の shutdown は `worker: WorkerChannel` の `Drop` impl が担う
// (B0h-c-ii #163 で `KotohaEngine::Drop` から移管)。

#[cfg(feature = "test-helpers")]
impl KotohaEngine {
    /// Test-only inspector: 現在 preedit の clone を返す。
    pub fn preedit_for_test(&self) -> String {
        self.preedit.current.clone()
    }
    /// Test-only inspector: 現在 candidate 数を返す。
    pub fn candidate_count_for_test(&self) -> usize {
        self.candidates.items.len()
    }
    /// Test-only inspector: 現在 highlight idx を返す。
    pub fn highlight_idx_for_test(&self) -> usize {
        self.candidates.highlight
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
    /// Test-only inspector: 現在 candidate Vec の clone を返す。
    ///
    /// B0g-b #148 / I8 sanitization filter が正しく Candidate を drop している
    /// か観測するための accessor。
    pub fn candidates_for_test(&self) -> Vec<kotoha_core::Candidate> {
        self.candidates.items.clone()
    }

    /// Test-only: `drain_pending_events()` を直接呼んで rx_event の pending な
    /// `Candidates` / `WorkerError` を即時消化する。
    ///
    /// B0g-c #148 / I12 polling helper と組合わせ、worker が late に送った
    /// 空 Replace 等を test 側で明示的に flush して assert する用途。
    /// `process_key_event` 経由でしか `drain_pending_events` を呼ばないため、
    /// keystroke を発火させずに pending event を吸い出したい case で必要。
    pub fn flush_pending_events_for_test(&mut self) {
        self.drain_pending_events();
    }

    /// Test-only: `apply_candidate_update()` を外部 integration test から呼ぶ
    /// ための wrapper(I10 silent ranker theater fix の direct-apply pattern 用)。
    ///
    /// `apply_candidate_update` 自体は `pub(crate)` で同 crate 内 module 限定。
    /// Worker chain の timing 依存を回避して boundary 規約を unit-style assert
    /// する test に必要。
    pub fn apply_candidate_update_for_test(&mut self, update: CandidateUpdate) {
        self.apply_candidate_update(update);
    }
}

#[cfg(test)]
mod tests {
    //! Phase 3-B B0g-b (ISSUE #148 / I8): apply_candidate_update が unsafe な
    //! candidate を engine 境界で filter することを確認する unit test。
    //! engine 全体の lifecycle は要らないので filter_safe_candidates を直接呼ぶ。

    use super::*;
    use kotoha_core::Candidate;

    #[test]
    fn filter_drops_candidate_with_control_char() {
        let input = vec![
            Candidate::new("clean", 0.0),
            Candidate::new("contains\u{001B}escape", 0.0),
            Candidate::new("\u{0000}null", 0.0),
        ];
        let kept = filter_safe_candidates(input);
        assert_eq!(kept.len(), 1, "only clean candidate should survive");
        assert_eq!(kept[0].surface, "clean");
    }

    #[test]
    fn filter_drops_candidate_with_bidi_override() {
        let input = vec![
            Candidate::new("safe", 0.0),
            Candidate::new("ab\u{202E}cd", 0.0), // RTL override
        ];
        let kept = filter_safe_candidates(input);
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].surface, "safe");
    }

    #[test]
    fn filter_keeps_all_when_clean() {
        let input = vec![
            Candidate::new("hello", 0.0),
            Candidate::new("こんにちは", 0.0),
            Candidate::new("漢字 + emoji 🦀", 0.0),
        ];
        let kept = filter_safe_candidates(input);
        assert_eq!(kept.len(), 3);
    }

    #[test]
    fn filter_returns_empty_when_all_dirty() {
        let input = vec![
            Candidate::new("\u{001B}[2J", 0.0),
            Candidate::new("\u{0000}", 0.0),
        ];
        let kept = filter_safe_candidates(input);
        assert!(kept.is_empty());
    }
}

#[cfg(all(test, feature = "test-helpers"))]
mod boundary_notify_tests {
    //! Phase 3-B B0g-b self-review F1 (Critical) regression:
    //! `apply_candidate_update` で filter 全 drop された Replace を受けた時、
    //! engine 内部 state だけでなく **host にも明示的に Clear + hide が発行**
    //! されることを assert する。host call 経路まで観測しないと、I8 filter
    //! 自体が新たに「前回候補画面残留」silent failure を作る。

    use super::*;
    use crate::testing::{HostOperation, MockCandidateUpdate, MockHostBridge};
    use kotoha_core::Candidate;

    #[test]
    fn replace_with_all_unsafe_candidates_clears_host_when_prior_was_nonempty() {
        let host = MockHostBridge::new();
        let host_handle = host.clone();
        let host_box: Box<dyn IMEHostBridge> = Box::new(host);

        // engine spawn を避けて test 速度確保のため、apply_candidate_update を
        // 直接 call できる構造で engine を組み立てる。Ranker / writer は本 test
        // で発火しないので minimal stub。
        struct NoopRanker;
        impl crate::Ranker for NoopRanker {
            fn rank(
                &self,
                _kana: &str,
                _ctx: &crate::ConversionContext,
                _cancel: std::sync::Arc<dyn crate::CancellationToken>,
                _sink: std::sync::mpsc::Sender<crate::RankerOutput>,
            ) -> Result<(), crate::RankerError> {
                Ok(())
            }
        }
        #[derive(Default)]
        struct StubWriter;
        impl crate::learning_port::LearningRecorder for StubWriter {
            fn record_choice(
                &self,
                _kana_input: &str,
                _chosen_kanji: &str,
            ) -> Result<(), crate::learning_port::LearningError> {
                Ok(())
            }
            fn evict_lru(
                &self,
                _max_entries: usize,
            ) -> Result<usize, crate::learning_port::LearningError> {
                Ok(0)
            }
        }

        let mut engine = KotohaEngine::new(
            host_box,
            std::sync::Arc::new(NoopRanker),
            std::sync::Arc::new(StubWriter),
        )
        .expect("engine spawn");

        // (1) 事前に safe candidates を投入して engine 内 state を non-empty にする
        //     (= 直前 host 側に候補が表示されている状態を simulate)。
        engine.apply_candidate_update(CandidateUpdate::Replace(vec![Candidate::new(
            "こんにちは",
            0.0,
        )]));
        assert_eq!(engine.candidate_count_for_test(), 1);
        host_handle.clear(); // 事前 setup の operation 履歴を捨てる。

        // (2) 全 unsafe な candidate を Replace で投入 → filter で空に。
        engine.apply_candidate_update(CandidateUpdate::Replace(vec![
            Candidate::new("\u{001B}[2J", 0.0),
            Candidate::new("\u{0000}null", 0.0),
        ]));
        assert_eq!(engine.candidate_count_for_test(), 0);

        // (3) host への通知経路を観測:filter 全 drop で Clear + hide が送られている。
        let ops = host_handle.operations();
        assert!(
            ops.iter().any(|op| matches!(
                op,
                HostOperation::UpdateCandidates(MockCandidateUpdate::Clear)
            )),
            "expected UpdateCandidates(Clear) but got {ops:?}"
        );
        assert!(
            ops.iter()
                .any(|op| matches!(op, HostOperation::HideCandidateWindow)),
            "expected HideCandidateWindow but got {ops:?}"
        );
    }

    #[test]
    fn replace_with_unsafe_candidates_skips_host_when_prior_was_empty() {
        // 直前 candidates が空(host 側にも候補が表示されていない)状態で
        // unsafe candidates を投入しても、host 側に余計な Clear / hide は送らない
        // (host は既に hide 状態のため意味なし)。本 test は spec §9.3 の
        // 「前回候補画面残留」防止規約を **過度に通知しない** 不変条件を pin する。
        let host = MockHostBridge::new();
        let host_handle = host.clone();
        let host_box: Box<dyn IMEHostBridge> = Box::new(host);

        struct NoopRanker;
        impl crate::Ranker for NoopRanker {
            fn rank(
                &self,
                _kana: &str,
                _ctx: &crate::ConversionContext,
                _cancel: std::sync::Arc<dyn crate::CancellationToken>,
                _sink: std::sync::mpsc::Sender<crate::RankerOutput>,
            ) -> Result<(), crate::RankerError> {
                Ok(())
            }
        }
        #[derive(Default)]
        struct StubWriter;
        impl crate::learning_port::LearningRecorder for StubWriter {
            fn record_choice(
                &self,
                _kana_input: &str,
                _chosen_kanji: &str,
            ) -> Result<(), crate::learning_port::LearningError> {
                Ok(())
            }
            fn evict_lru(
                &self,
                _max_entries: usize,
            ) -> Result<usize, crate::learning_port::LearningError> {
                Ok(0)
            }
        }

        let mut engine = KotohaEngine::new(
            host_box,
            std::sync::Arc::new(NoopRanker),
            std::sync::Arc::new(StubWriter),
        )
        .expect("engine spawn");

        engine.apply_candidate_update(CandidateUpdate::Replace(vec![Candidate::new(
            "\u{001B}[2J",
            0.0,
        )]));

        let ops = host_handle.operations();
        assert!(
            !ops.iter()
                .any(|op| matches!(op, HostOperation::UpdateCandidates(_))),
            "host should NOT receive update_candidates when prior buffer was already empty; ops={ops:?}"
        );
        assert!(
            !ops.iter()
                .any(|op| matches!(op, HostOperation::HideCandidateWindow)),
            "host should NOT receive HideCandidateWindow when prior buffer was already empty; ops={ops:?}"
        );
    }

    #[test]
    fn replace_with_clean_candidates_does_not_emit_extra_clear() {
        let host = MockHostBridge::new();
        let host_handle = host.clone();
        let host_box: Box<dyn IMEHostBridge> = Box::new(host);

        struct NoopRanker;
        impl crate::Ranker for NoopRanker {
            fn rank(
                &self,
                _kana: &str,
                _ctx: &crate::ConversionContext,
                _cancel: std::sync::Arc<dyn crate::CancellationToken>,
                _sink: std::sync::mpsc::Sender<crate::RankerOutput>,
            ) -> Result<(), crate::RankerError> {
                Ok(())
            }
        }
        #[derive(Default)]
        struct StubWriter;
        impl crate::learning_port::LearningRecorder for StubWriter {
            fn record_choice(
                &self,
                _kana_input: &str,
                _chosen_kanji: &str,
            ) -> Result<(), crate::learning_port::LearningError> {
                Ok(())
            }
            fn evict_lru(
                &self,
                _max_entries: usize,
            ) -> Result<usize, crate::learning_port::LearningError> {
                Ok(0)
            }
        }

        let mut engine = KotohaEngine::new(
            host_box,
            std::sync::Arc::new(NoopRanker),
            std::sync::Arc::new(StubWriter),
        )
        .expect("engine spawn");

        // clean candidate を Replace。
        engine.apply_candidate_update(CandidateUpdate::Replace(vec![
            Candidate::new("こんにちは", 0.0),
            Candidate::new("hello", 0.0),
        ]));

        assert_eq!(engine.candidate_count_for_test(), 2);

        // host への通知経路を観測:Clear / Hide は送られない(caller が出すもの)。
        let ops = host_handle.operations();
        assert!(
            !ops.iter()
                .any(|op| matches!(op, HostOperation::UpdateCandidates(_))),
            "apply_candidate_update should not emit host update_candidates for clean Replace; \
             host call is the caller's responsibility (transitions.rs). Observed: {ops:?}"
        );
    }
}
