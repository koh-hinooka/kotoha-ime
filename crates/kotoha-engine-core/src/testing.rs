//! Test infra: `MockHostBridge` / `MockRanker` / `await_until` for L1/L2 test DI.
//!
//! `test-helpers` feature gate 下で公開する。本 module は production binary に
//! 含まれない(P2-D `MockLearningCacheStore` と同 pattern、spec §10.5)。

#![cfg(feature = "test-helpers")]

use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

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
///
/// # 引数 verification (B0g-c #148 / 第 2 回 review I14)
///
/// engine が正しい `kana` / `ConversionMode` を Ranker に渡しているかを
/// catch するため、各 rank 呼び出しの `kana` と `mode` を **最後の 1 件**
/// 内部に保持する。`last_kana()` / `last_mode()` で test から取得可能。
#[derive(Debug)]
pub struct MockRanker {
    candidates: Vec<Candidate>,
    rank_calls: Arc<std::sync::atomic::AtomicU64>,
    cancel_observed: Arc<std::sync::atomic::AtomicU64>,
    last_kana: Arc<std::sync::Mutex<Option<String>>>,
    last_mode: Arc<std::sync::Mutex<Option<crate::ranker::ConversionMode>>>,
}

impl MockRanker {
    pub fn new(candidates: Vec<Candidate>) -> Self {
        Self {
            candidates,
            rank_calls: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            cancel_observed: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            last_kana: Arc::new(std::sync::Mutex::new(None)),
            last_mode: Arc::new(std::sync::Mutex::new(None)),
        }
    }

    pub fn rank_calls(&self) -> u64 {
        self.rank_calls.load(std::sync::atomic::Ordering::SeqCst)
    }

    pub fn cancel_observed(&self) -> u64 {
        self.cancel_observed
            .load(std::sync::atomic::Ordering::SeqCst)
    }

    /// 最後の `rank()` 呼び出しで受け取った `kana` 引数の clone。
    /// 一度も呼ばれていない場合 `None`。
    pub fn last_kana(&self) -> Option<String> {
        self.last_kana
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    /// 最後の `rank()` 呼び出しで受け取った `ConversionContext.mode` の copy。
    pub fn last_mode(&self) -> Option<crate::ranker::ConversionMode> {
        *self
            .last_mode
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

impl Ranker for MockRanker {
    fn rank(
        &self,
        kana: &str,
        ctx: &ConversionContext,
        cancel: Arc<dyn CancellationToken>,
        sink: mpsc::Sender<RankerOutput>,
    ) -> Result<(), RankerError> {
        self.rank_calls
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        // I14: 引数 verification 用に最後の 1 件を記録。
        if let Ok(mut guard) = self.last_kana.lock() {
            *guard = Some(kana.to_string());
        }
        if let Ok(mut guard) = self.last_mode.lock() {
            *guard = Some(ctx.mode);
        }
        if cancel.is_cancelled() {
            self.cancel_observed
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            return Ok(());
        }
        // `RankerOutput.request_id` field は B0e (ISSUE #140 / Important 10)で撤去済。
        // Stale response の discard は engine 主 thread 側で `RankRequest`/`active_request`
        // ベースの id 照合 + per-request channel 不変条件で成立する(spec §7.5)。
        let _ = sink.send(RankerOutput {
            update: CandidateUpdate::Replace(self.candidates.clone()),
        });
        Ok(())
    }
}

/// Polling-based wait helper(B0g-c #148 / 第 2 回 review I12)。
///
/// `predicate` が true を返すまで最大 `timeout` まで polling し、ミリ秒
/// 単位の sleep で busy-wait を抑える。固定 `thread::sleep(N)` ベースの
/// timing assertion(CI scheduler 圧迫時に flaky)を polling 化する。
///
/// # Preconditions
///
/// - `predicate` は **副作用なしで複数回呼ばれて safe**(`AtomicU64::load`、
///   `MockHostBridge::operations()`、`KotohaEngine::*_for_test()` 等)
/// - `timeout` は production timing budget を上回る margin を含む(典型: 500ms)
///
/// # Postconditions
///
/// - 戻り値 `Ok(())`:`predicate` が true を返した(timeout 前に成立)
/// - 戻り値 `Err(timeout_elapsed)`:timeout 経過しても false のまま
///
/// # Errors
///
/// - timeout 超過時に経過時間を返す。test 側は `expect("...")` で panic させ
///   失敗時に context message で原因特定する pattern を推奨。
///
/// # Examples
///
/// ```ignore
/// use kotoha_engine_core::testing::await_until;
/// use std::time::Duration;
///
/// // counter が 1 以上になるまで待つ(最大 500ms)
/// await_until(
///     || counter.load(Ordering::SeqCst) >= 1,
///     Duration::from_millis(500),
/// )
/// .expect("counter should reach 1 within 500ms");
/// ```
pub fn await_until<F: FnMut() -> bool>(
    mut predicate: F,
    timeout: Duration,
) -> Result<(), Duration> {
    let start = Instant::now();
    loop {
        if predicate() {
            return Ok(());
        }
        if start.elapsed() >= timeout {
            return Err(start.elapsed());
        }
        std::thread::sleep(Duration::from_millis(2));
    }
}
