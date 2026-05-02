//! `KotohaEngine` 状態機械 — Phase 3-A spec §5 / §6 を実装する core engine。
//!
//! 本 module は M2 段階で **synchronous Ranker** path を実装する。M3 で
//! `RankerWorker` 背景 thread に置き換える(Ranker は `rank()` 内で sink
//! に送り、本 method はその受信を recv_timeout で drain する)。
//!
//! spec §5.2 の状態遷移 table を `process_key_event` 内の dispatch helper
//! ([`transitions::dispatch_key`])で網羅する。

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
#[doc(hidden)]
pub mod transitions;

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
/// `id` field は M3 で `RankerWorker` の `request_id` mismatch discard
/// (spec §7.5)に使う。M2 段階では同期 Ranker のため引数 path 上で
/// 検査済みだが、API 互換のため field を確保する。
pub(crate) struct RequestHandle {
    #[allow(dead_code)]
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
}

impl KotohaEngine {
    /// 新 engine を構築する。host / ranker / learning_writer を DI で受ける。
    ///
    /// # Postconditions
    ///
    /// - `state == EngineState::Idle`
    /// - `enabled == false`(host が `enable()` を呼ぶまで no-op + `Forwarded`)
    /// - `focused == false`
    pub fn new(
        host: Box<dyn IMEHostBridge>,
        ranker: Arc<dyn Ranker>,
        learning_writer: Arc<dyn kotoha_storage::learning_cache::LearningCacheWriter>,
    ) -> Self {
        Self {
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
        }
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

    /// Live or Commit mode の RankRequest を発行し、active_request を更新する。
    /// M2 段階では同期的に Ranker を呼び、即時受信した `RankerOutput` を
    /// engine の `candidates` に反映する(M3 で background thread 化する)。
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

        let (tx, rx) = mpsc::channel();
        let cancel_dyn: Arc<dyn CancellationToken> = cancel_token;
        let _ = self
            .ranker
            .rank(&self.current_preedit, &ctx, cancel_dyn, tx);

        // sink から候補を drain し engine の candidates を更新。
        // M2 では MockRanker の同期 send + drop を前提とするため、最初の
        // recv_timeout で Ok 受信、第 2 回で Disconnected で抜ける。
        // 50ms timeout は MockRanker での通常 path では発火せず、安全網として残す。
        self.candidates.clear();
        while let Ok(out) = rx.recv_timeout(Duration::from_millis(50)) {
            if out.request_id != 0 && out.request_id != request_id {
                continue;
            }
            self.apply_candidate_update(out.update);
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
}
