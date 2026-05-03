//! `LookupTable` — IBus 1.x の `update_lookup_table` 全置換制約に対応する
//! `Mutex<Vec<Candidate>>` 内部 buffer。
//!
//! Phase 3-A spec §4.2 mapping 表に従い、`CandidateUpdate::Append` /
//! `Remove` / `Clear` を internal buffer mutate + `Replace` 相当の全置換
//! call に変換する。

use std::sync::Mutex;

use kotoha_core::Candidate;
use kotoha_engine_core::CandidateUpdate;

/// IBus 用 lookup table buffer。`Mutex` poison は
/// `unwrap_or_else(PoisonError::into_inner)` で取扱う(PR #111 規約)。
#[derive(Debug, Default)]
pub struct LookupTable {
    candidates: Mutex<Vec<Candidate>>,
}

impl LookupTable {
    pub fn new() -> Self {
        Self {
            candidates: Mutex::new(Vec::new()),
        }
    }

    /// `CandidateUpdate` を内部 buffer に適用する。
    ///
    /// # Postconditions
    ///
    /// - 戻り値は IBus に send すべき全置換 candidate Vec(visible 判定とは別 flag)
    /// - `Clear` は空 Vec を返す
    pub fn apply(&self, update: CandidateUpdate) -> Vec<Candidate> {
        let mut guard = self
            .candidates
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        match update {
            CandidateUpdate::Replace(c) => *guard = c,
            CandidateUpdate::Append(c) => guard.extend(c),
            CandidateUpdate::Remove(r) => {
                let len = guard.len();
                let start = r.start.min(len);
                let end = r.end.min(len);
                if start < end {
                    guard.drain(start..end);
                }
            }
            CandidateUpdate::Clear => guard.clear(),
            // `CandidateUpdate` は non_exhaustive(`crates/kotoha-engine-core/src/ranker/update.rs` 凍結)
            // のため、外部 crate である本 module には wildcard arm が syntax 上必須。
            // 未知 variant 到達時は **production では buffer 維持で no-op** に倒すが、
            // 「lookup table が新 variant の意図通り更新されない silent failure」
            // (B0g #148 / 第 2 回 review C5)を防ぐため以下を強制する:
            //
            // - `tracing::error!`(WARN ではなく ERROR、`KOTOHA_LOG=info` default で観測可)
            // - `debug_assert!` で dev / test build では即 panic(CI 通過前に検出)
            // - log には variant 内容(候補 surface 等)を一切載せず、`std::mem::discriminant`
            //   のみを記録(S-N9 future log leak の予防)
            other => {
                let disc = std::mem::discriminant(&other);
                tracing::error!(
                    variant_discriminant = ?disc,
                    "unhandled CandidateUpdate variant; lookup_table.rs no-op fallback fired; \
                     update crates/kotoha-engine-ibus/src/lookup_table.rs to support the new variant"
                );
                debug_assert!(
                    false,
                    "unhandled CandidateUpdate variant in kotoha-engine-ibus::LookupTable::apply"
                );
            }
        }
        guard.clone()
    }

    /// 内部 buffer を直接 clear する(focus_out 等で host 側から強制 reset)。
    pub fn clear(&self) {
        self.candidates
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// spec §4.2 mapping: Replace で全置換
    #[test]
    fn replace_fully_replaces_buffer() {
        let t = LookupTable::new();
        let r1 = t.apply(CandidateUpdate::Replace(vec![Candidate::new("a", 0.0)]));
        assert_eq!(r1.len(), 1);
        let r2 = t.apply(CandidateUpdate::Replace(vec![Candidate::new("b", 0.0)]));
        assert_eq!(r2.len(), 1);
        assert_eq!(r2[0].surface, "b");
    }

    /// spec §4.2 mapping: Append で末尾追加
    #[test]
    fn append_extends_buffer() {
        let t = LookupTable::new();
        t.apply(CandidateUpdate::Replace(vec![Candidate::new("a", 0.0)]));
        let r = t.apply(CandidateUpdate::Append(vec![Candidate::new("b", 0.0)]));
        assert_eq!(r.len(), 2);
    }

    /// spec §4.2 mapping: Remove で範囲削除
    #[test]
    fn remove_drops_range() {
        let t = LookupTable::new();
        t.apply(CandidateUpdate::Replace(vec![
            Candidate::new("a", 0.0),
            Candidate::new("b", 0.0),
            Candidate::new("c", 0.0),
        ]));
        let r = t.apply(CandidateUpdate::Remove(1..3));
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].surface, "a");
    }

    /// spec §4.2 mapping: Clear で空化
    #[test]
    fn clear_empties_buffer() {
        let t = LookupTable::new();
        t.apply(CandidateUpdate::Replace(vec![Candidate::new("a", 0.0)]));
        let r = t.apply(CandidateUpdate::Clear);
        assert!(r.is_empty());
    }

    /// 範囲 out-of-bounds は clamp(panic 禁止)
    #[test]
    fn remove_out_of_bounds_clamps() {
        let t = LookupTable::new();
        t.apply(CandidateUpdate::Replace(vec![Candidate::new("a", 0.0)]));
        let r = t.apply(CandidateUpdate::Remove(5..10));
        assert_eq!(r.len(), 1); // 変化なし
    }
}
