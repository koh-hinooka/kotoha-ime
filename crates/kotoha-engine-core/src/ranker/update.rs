//! `CandidateUpdate` enum — 候補差分通知。
//!
//! Phase 3-A spec §4.2(2026-05-02、ISSUE #116)で凍結。
//! Worker thread から engine 主 thread へ、または engine から host adapter へ
//! 送られる差分通知 message。

use kotoha_core::Candidate;
use std::ops::Range;

/// 候補 list の差分通知。
///
/// IBus 1.x は `update_lookup_table` で全置換のみのため、IBus adapter は
/// `Append` / `Remove` を内部 buffer 蓄積後の `Replace` に変換する
/// (Phase 3-A spec §4.2 mapping 表)。
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum CandidateUpdate {
    /// 候補 list 全置換(Phase 3-A での主用法)。
    Replace(Vec<Candidate>),
    /// 末尾追加(coalescing 後の LLM 結果到着時)。
    Append(Vec<Candidate>),
    /// 範囲削除(Phase 5 beam search で beam 削減時、Phase 3-A 初期は未使用)。
    Remove(Range<usize>),
    /// 全 clear。
    Clear,
}

#[cfg(test)]
mod tests {
    use super::*;
    use kotoha_core::Candidate;

    /// spec §4.2: Replace variant が Vec<Candidate> を保持できる
    #[test]
    fn replace_holds_candidates() {
        let upd = CandidateUpdate::Replace(vec![Candidate::new("琴葉", -1.5)]);
        match upd {
            CandidateUpdate::Replace(v) => {
                assert_eq!(v.len(), 1);
                assert_eq!(v[0].surface, "琴葉");
            }
            _ => panic!("expected Replace"),
        }
    }

    /// spec §4.2: Clear variant が引数なしで構築できる
    #[test]
    fn clear_is_unit_variant() {
        let upd = CandidateUpdate::Clear;
        match upd {
            CandidateUpdate::Clear => {}
            _ => panic!("expected Clear"),
        }
    }

    /// spec §4.2: CandidateUpdate は Clone 可能(adapter 内部 buffer 等で複製される)
    #[test]
    fn update_is_clone() {
        let upd = CandidateUpdate::Replace(vec![Candidate::new("琴葉", -1.5)]);
        let cloned = upd.clone();
        match (upd, cloned) {
            (CandidateUpdate::Replace(a), CandidateUpdate::Replace(b)) => {
                assert_eq!(a[0].surface, b[0].surface);
            }
            _ => panic!("expected Replace"),
        }
    }
}
