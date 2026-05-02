//! `ConversionContext` / `ConversionMode` — Ranker 入力 context types.
//!
//! Phase 3-A spec §4.3(2026-05-02、ISSUE #116)で凍結。Phase 5 で
//! `partial_input` / `typo_distance` field が non-breaking で追加される予定。

use std::time::Duration;

/// Ranker `rank()` の context 引数。focus session 内 commit 履歴と変換 mode を含む。
///
/// `commit_history` は engine 内部の `VecDeque<String>` の snapshot を `Vec<String>`
/// として clone して詰める設計(Phase 3-A spec §4.3 備考参照)。本 struct は
/// non-exhaustive で Phase 5 field 追加時の互換性を担保する。
#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct ConversionContext {
    /// focus session 内の直前 N 文字(default 200、Phase 3-A spec §13 Open Q 1)の
    /// commit 履歴 surface(古い順)。
    pub commit_history: Vec<String>,
    /// 直前 commit からの経過時間(personalization signal、Phase 5 で活用)。
    pub time_since_last_commit: Duration,
    /// 変換 mode:Live(typing 中)か Commit(space 後)か。
    pub mode: ConversionMode,
}

impl ConversionContext {
    /// 空 context を作成する(test / focus_in 直後の初期状態用)。
    pub fn empty(mode: ConversionMode) -> Self {
        Self {
            commit_history: Vec::new(),
            time_since_last_commit: Duration::from_secs(0),
            mode,
        }
    }
}

/// 変換 mode。Live(typing 中、毎 keystroke で Ranker 起動)と
/// Commit(space 後、coalescing 拡張で LLM 待機)の 2 種。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConversionMode {
    /// Typing 中、dict only fast path、LLM は best-effort。
    Live,
    /// Space 確定後、coalescing window 拡張、LLM 完了まで待機。
    Commit,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// spec §4.3: ConversionContext::empty(Live) で Live mode の空 context が作れる
    #[test]
    fn empty_context_live_mode() {
        let ctx = ConversionContext::empty(ConversionMode::Live);
        assert_eq!(ctx.mode, ConversionMode::Live);
        assert!(ctx.commit_history.is_empty());
        assert_eq!(ctx.time_since_last_commit, Duration::from_secs(0));
    }

    /// spec §4.3: ConversionMode は Live と Commit の 2 種
    #[test]
    fn conversion_modes_are_distinct() {
        assert_ne!(ConversionMode::Live, ConversionMode::Commit);
    }

    /// spec §4.3 備考: ConversionContext は Clone 可能(snapshot として Ranker に渡される)
    #[test]
    fn context_is_clone() {
        let ctx = ConversionContext {
            commit_history: vec!["琴葉".into()],
            time_since_last_commit: Duration::from_secs(5),
            mode: ConversionMode::Commit,
        };
        let cloned = ctx.clone();
        assert_eq!(cloned.commit_history, ctx.commit_history);
        assert_eq!(cloned.mode, ctx.mode);
    }
}
