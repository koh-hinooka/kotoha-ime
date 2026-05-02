//! Test infra: `MockHostBridge` / `MockRanker` for L1 unit test DI.
//!
//! `test-helpers` feature gate 下で公開する。本 module は production binary に
//! 含まれない(P2-D `MockLearningCacheStore` と同 pattern、spec §10.5)。

#![cfg(feature = "test-helpers")]

use std::sync::mpsc;
use std::sync::{Arc, Mutex};

use kotoha_core::Candidate;

use crate::cancel::CancellationToken;
use crate::host_bridge::IMEHostBridge;
use crate::ranker::{CandidateUpdate, ConversionContext, Ranker, RankerError, RankerOutput};

/// `IMEHostBridge` への呼び出しを `Vec<HostOperation>` に記録する mock。
///
/// L1 unit test で engine の状態遷移ごとに host call sequence を assert する
/// (spec §10.2 / §10.5)。
#[derive(Debug, Clone, Default)]
pub struct MockHostBridge {
    operations: Arc<Mutex<Vec<HostOperation>>>,
}

/// `MockHostBridge` が記録する 1 操作。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostOperation {
    UpdatePreedit {
        text: String,
        cursor: usize,
        visible: bool,
    },
    CommitText(String),
    UpdateCandidates(MockCandidateUpdate),
    ShowCandidateWindow,
    HideCandidateWindow,
}

/// `CandidateUpdate` の test 比較用 simplified clone。`Range` の `Eq` 実装が
/// 不安定のため自前 enum を用意する。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MockCandidateUpdate {
    Replace(Vec<String>),
    Append(Vec<String>),
    Remove { start: usize, end: usize },
    Clear,
}

impl From<&CandidateUpdate> for MockCandidateUpdate {
    fn from(u: &CandidateUpdate) -> Self {
        match u {
            CandidateUpdate::Replace(c) => {
                MockCandidateUpdate::Replace(c.iter().map(|x| x.surface.clone()).collect())
            }
            CandidateUpdate::Append(c) => {
                MockCandidateUpdate::Append(c.iter().map(|x| x.surface.clone()).collect())
            }
            CandidateUpdate::Remove(r) => MockCandidateUpdate::Remove {
                start: r.start,
                end: r.end,
            },
            CandidateUpdate::Clear => MockCandidateUpdate::Clear,
        }
    }
}

impl MockHostBridge {
    pub fn new() -> Self {
        Self::default()
    }

    /// 記録された全 operation の clone を返す。
    pub fn operations(&self) -> Vec<HostOperation> {
        self.operations
            .lock()
            .expect("MockHostBridge mutex poisoned")
            .clone()
    }

    /// 記録を全 clear する。複数シナリオの分離 assert に使う。
    pub fn clear(&self) {
        self.operations
            .lock()
            .expect("MockHostBridge mutex poisoned")
            .clear();
    }

    fn push(&self, op: HostOperation) {
        self.operations
            .lock()
            .expect("MockHostBridge mutex poisoned")
            .push(op);
    }
}

impl IMEHostBridge for MockHostBridge {
    fn update_preedit(&self, text: &str, cursor: usize, visible: bool) {
        self.push(HostOperation::UpdatePreedit {
            text: text.to_string(),
            cursor,
            visible,
        });
    }

    fn commit_text(&self, text: &str) {
        self.push(HostOperation::CommitText(text.to_string()));
    }

    fn update_candidates(&self, update: CandidateUpdate) {
        self.push(HostOperation::UpdateCandidates((&update).into()));
    }

    fn show_candidate_window(&self) {
        self.push(HostOperation::ShowCandidateWindow);
    }

    fn hide_candidate_window(&self) {
        self.push(HostOperation::HideCandidateWindow);
    }
}

/// `Ranker` の mock impl。固定候補を即時 sink.send + cancel observer。
///
/// L1 unit test で engine が rank を呼ぶ回数 / cancel 検出を assert する
/// (spec §10.2 / §10.5)。
#[derive(Debug)]
pub struct MockRanker {
    candidates: Vec<Candidate>,
    rank_calls: Arc<std::sync::atomic::AtomicU64>,
    cancel_observed: Arc<std::sync::atomic::AtomicU64>,
}

impl MockRanker {
    pub fn new(candidates: Vec<Candidate>) -> Self {
        Self {
            candidates,
            rank_calls: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            cancel_observed: Arc::new(std::sync::atomic::AtomicU64::new(0)),
        }
    }

    pub fn rank_calls(&self) -> u64 {
        self.rank_calls.load(std::sync::atomic::Ordering::SeqCst)
    }

    pub fn cancel_observed(&self) -> u64 {
        self.cancel_observed
            .load(std::sync::atomic::Ordering::SeqCst)
    }
}

impl Ranker for MockRanker {
    fn rank(
        &self,
        _kana: &str,
        _ctx: &ConversionContext,
        cancel: Arc<dyn CancellationToken>,
        sink: mpsc::Sender<RankerOutput>,
    ) -> Result<(), RankerError> {
        self.rank_calls
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        if cancel.is_cancelled() {
            self.cancel_observed
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            return Ok(());
        }
        // request_id は engine 主 thread 採番ではなく、Mock では 0 固定とし、
        // M3 で `RankerWorker` 経由になった時に worker が `request_id` を
        // `RankerOutput.request_id` に上書きする設計。
        let _ = sink.send(RankerOutput {
            request_id: 0,
            update: CandidateUpdate::Replace(self.candidates.clone()),
        });
        Ok(())
    }
}
