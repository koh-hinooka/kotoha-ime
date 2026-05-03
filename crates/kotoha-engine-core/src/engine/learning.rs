//! `LearningSink` — commit lifecycle に紐付く 3 リソースの SRP-pure value 単位。
//!
//! Phase 3-B B0h-c-iii (ISSUE #149 / #165) で `KotohaEngine` から抽出された 3 field
//! (`learning_writer: Arc<dyn LearningRecorder>` + `commit_history: CommitHistory` +
//! `last_commit_at: Instant`)を 1 sub-struct に集約する。
//!
//! 本 sub-struct の責務は **「commit 確定 path で lock-step 更新される 3 リソース
//! (学習 sink への記録 / 直近 commit 文字列 buffer / 最終 commit 時刻)を 1 単位として
//! 保持し、`focus_out` / `reset` 時の clear 制御を一括で行えるようにする」** こと。
//!
//! 本 PR (B0h-c-iii) では既存呼び出し側との diff を最小化するため、`recorder` /
//! `commit_history` / `last_commit_at` は依然 `pub(crate)` で直接 access する。
//! method 経由(`record_commit(kana, surface)` で 3 操作を 1 呼び出しに consolidate
//! する等)への抽象化は将来 PR(transitions.rs free-function method 化と併せ B0h-c-iv)
//! で進める。

use std::sync::Arc;
use std::time::Instant;

use super::commit_history::CommitHistory;
use crate::learning_port::LearningRecorder;

/// commit lifecycle の 3 リソースを 1 単位として持つ value 型。
///
/// # Invariants
///
/// - `commit_history.total_chars() <= 200`(spec §5.3、`CommitHistory::push` 内で
///   維持)。
/// - `last_commit_at` は最後に commit が確定した瞬間の `Instant`。`new()` 直後は
///   `Instant::now()` で初期化される(まだ commit が無くても `time_since_last_commit`
///   が呼べるよう sentinel)。
/// - `recorder` は `Arc<dyn LearningRecorder>` で複数の adapter にも shared 可能
///   (現状は `kotoha-engine-adapter::arc_sqlite_learning_cache` 経由 1 件のみ)。
pub(crate) struct LearningSink {
    /// 学習 cache への commit 記録 sink(Phase 3-A spec §3.3 driven port)。
    /// `record_choice(kana, surface)` を commit 確定時に呼ぶ。
    pub(crate) recorder: Arc<dyn LearningRecorder>,
    /// 直近 commit 文字列の rolling buffer。`Ranker::rank` の
    /// `ConversionContext::commit_history` snapshot として渡される(spec §5.3)。
    pub(crate) commit_history: CommitHistory,
    /// 最終 commit 時刻。`Ranker::rank` の `ConversionContext::time_since_last_commit`
    /// に渡される(spec §5.3、long-idle で context decay の hint に使う)。
    pub(crate) last_commit_at: Instant,
}

impl LearningSink {
    /// `Arc<dyn LearningRecorder>` を DI で受け、空の commit_history と
    /// 現在時刻 sentinel で初期化された sink を構築する。
    ///
    /// # Postconditions
    ///
    /// - `commit_history` は空(`commit_history.total_chars() == 0`)
    /// - `last_commit_at` は `Instant::now()`(sentinel、まだ commit 無し)
    pub(crate) fn new(recorder: Arc<dyn LearningRecorder>) -> Self {
        Self {
            recorder,
            commit_history: CommitHistory::new(),
            last_commit_at: Instant::now(),
        }
    }

    /// `commit_history` を clear する(`focus_out` / `reset` で呼ばれる)。
    ///
    /// `last_commit_at` は意図的に reset しない:context decay は時刻基準で測られ、
    /// focus 切替で「新規入力 session が始まった」と扱う必要が無いため(現行仕様の
    /// 維持。spec §5.3 で focus_out が context をどこまで巻き戻すかは今後の Open Q)。
    ///
    /// # Postconditions
    ///
    /// - `commit_history.total_chars() == 0`
    pub(crate) fn clear_history(&mut self) {
        self.commit_history.clear();
    }
}
