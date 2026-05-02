//! Property-based invariant tests for `merge_candidates`.
//!
//! Phase 3-A spec §10.1 で要求される proptest による invariant test。
//! 主要 invariant:
//!
//! - merge 結果の長さは `top_k` 以下
//! - merge 結果の score は降順
//! - 同 surface の重複は除去される
//!
//! `arb_candidate` の score range は `-100.0..100.0` に制限し NaN / +-Inf を
//! 排除する。`merge_candidates` 内の sort は `partial_cmp` を `Ordering::Equal`
//! fallback で扱うため NaN が混入すると invariant が崩れるが、生成段階で
//! 排除することで test の root cause を「merge logic 側」に局所化する。

use proptest::prelude::*;

use kotoha_core::Candidate;
use kotoha_engine_core::ranker::merge::{merge_candidates, CandidateSource};

/// `Candidate` の任意値生成 strategy。
///
/// - `surface`: `c{0..=255}` の 256 種で衝突が起こるように設計(dedupe invariant の
///   「同 surface が複数生成された場合の重複除去」を test 可能にする)。
/// - `score`: `-100.0..100.0` の bounded f32(NaN / +-Inf を排除)。
fn arb_candidate() -> impl Strategy<Value = Candidate> {
    (any::<u8>(), -100.0_f32..100.0_f32).prop_map(|(idx, score)| {
        let surface = format!("c{idx}");
        Candidate::new(surface, score)
    })
}

proptest! {
    /// invariant: merge 結果の長さは `top_k` 以下
    #[test]
    fn merge_respects_top_k(
        cands in prop::collection::vec(arb_candidate(), 0..50),
        top_k in 1usize..30,
    ) {
        let merged = merge_candidates(
            vec![(cands, CandidateSource::Dict)],
            &[],
            top_k,
        );
        prop_assert!(merged.len() <= top_k);
    }

    /// invariant: merge 結果は score 降順
    #[test]
    fn merge_is_sorted_descending(cands in prop::collection::vec(arb_candidate(), 0..50)) {
        let merged = merge_candidates(
            vec![(cands, CandidateSource::Dict)],
            &[],
            50,
        );
        for window in merged.windows(2) {
            prop_assert!(window[0].score >= window[1].score);
        }
    }

    /// invariant: 同一 surface の重複は merge で除去される
    #[test]
    fn merge_dedupes_by_surface(cands in prop::collection::vec(arb_candidate(), 0..50)) {
        let merged = merge_candidates(
            vec![(cands, CandidateSource::Dict)],
            &[],
            50,
        );
        let mut surfaces: Vec<&str> = merged.iter().map(|c| c.surface.as_str()).collect();
        surfaces.sort();
        let len_before = surfaces.len();
        surfaces.dedup();
        prop_assert_eq!(surfaces.len(), len_before, "duplicate surfaces detected");
    }
}
