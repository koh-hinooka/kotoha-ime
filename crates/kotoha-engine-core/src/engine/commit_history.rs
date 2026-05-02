//! `CommitHistory` — focus session 内 commit 履歴の `VecDeque` ラッパー。
//!
//! Phase 3-A spec §5.3 不変条件: `commit_history.len() <= 200`(超過時は最古を pop_front)。
//! Open Q 1 で 100 / 200 / 400 を AB test 予定だが、初期値は 200 chars。

use std::collections::VecDeque;

/// 200 chars 上限を不変に保つ commit 履歴。
///
/// # Invariants
///
/// - `iter().map(|s| s.chars().count()).sum::<usize>() <= MAX_CHARS`
/// - `push()` 後に invariant を満たさない場合、古い entry から pop_front
///
/// # 単位
///
/// chars(grapheme cluster ではなく Rust `char` count、spec §5.3 暫定)。
#[derive(Debug, Default, Clone)]
pub struct CommitHistory {
    entries: VecDeque<String>,
}

impl CommitHistory {
    /// spec §5.3:200 chars 上限(empirical 確定は spec §13 Open Q 1)。
    pub const MAX_CHARS: usize = 200;

    pub fn new() -> Self {
        Self {
            entries: VecDeque::new(),
        }
    }

    /// 新 commit を末尾に追加し、`MAX_CHARS` を超過するまで古い entry を pop_front。
    ///
    /// # Postconditions
    ///
    /// - `self.total_chars() <= MAX_CHARS`(invariant 維持)
    pub fn push(&mut self, surface: String) {
        self.entries.push_back(surface);
        while self.total_chars() > Self::MAX_CHARS {
            if self.entries.pop_front().is_none() {
                break;
            }
        }
    }

    /// 全 entry の合計 char 数。
    pub fn total_chars(&self) -> usize {
        self.entries.iter().map(|s| s.chars().count()).sum()
    }

    /// 全 entry を clear(focus_out 時など)。
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// snapshot を `Vec<String>` で返す(Ranker `ConversionContext` 構築用)。
    pub fn snapshot(&self) -> Vec<String> {
        self.entries.iter().cloned().collect()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// spec §5.3: 200 chars 以下なら全 entry を保持
    #[test]
    fn push_within_limit_keeps_all() {
        let mut h = CommitHistory::new();
        h.push("琴葉".into());
        h.push("です".into());
        assert_eq!(h.snapshot(), vec!["琴葉".to_string(), "です".to_string()]);
        assert_eq!(h.total_chars(), 4);
    }

    /// spec §5.3: 200 chars 超過時は最古 entry が pop_front される
    #[test]
    fn push_evicts_oldest_when_exceeding_limit() {
        let mut h = CommitHistory::new();
        let chunk = "あ".repeat(199);
        h.push(chunk.clone());
        assert_eq!(h.total_chars(), 199);
        h.push("いう".into()); // +2 chars → 201、最古が pop される
        assert!(h.total_chars() <= CommitHistory::MAX_CHARS);
        // 最古の chunk が消えて "いう" のみ
        assert_eq!(h.snapshot(), vec!["いう".to_string()]);
    }

    /// spec §5.3: clear 後は空
    #[test]
    fn clear_empties_all() {
        let mut h = CommitHistory::new();
        h.push("a".into());
        h.clear();
        assert!(h.is_empty());
        assert_eq!(h.total_chars(), 0);
    }
}
