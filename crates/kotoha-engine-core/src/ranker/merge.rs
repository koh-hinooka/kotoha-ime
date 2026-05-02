//! Candidate merge / dedupe / scoring logic.
//!
//! Phase 2 spec §3.3 で凍結された初期重み:
//! - dict: 0.95(SudachiDict + UserVocab)
//! - LLM: 1.0
//! - LearningCache hit: bonus(empirical、現在の暫定値は +0.5)
//!
//! 同一 surface の候補は max-score 採用で dedupe する(Phase 2 spec §3.3 D5
//! の選択肢「最大値採用」)。重み合算は P2-D 完了後の golden fixture evaluation
//! で再検討する余地を残す(Phase 2 spec §11.4 Q5)。

use kotoha_core::Candidate;
use std::collections::BTreeMap;

/// dict 候補の重み(SudachiDict + UserVocab、Phase 2 spec §3.3 暫定値)。
pub const WEIGHT_DICT: f32 = 0.95;
/// LLM 候補の重み。
pub const WEIGHT_LLM: f32 = 1.0;
/// LearningCache hit bonus(暫定、Phase 2 spec §11.4 Q5 で再評価)。
pub const BONUS_CACHE_HIT: f32 = 0.5;

/// 候補 source(merge 時の score 計算で使用)。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CandidateSource {
    /// SudachiDict + UserVocab 由来(両者は dict 系として同一重みを使用)。
    Dict,
    /// LLM 由来(M3 以降)。
    Llm,
    /// LearningCache hit。merge 内では既存 entry への bonus 加算に使う(独立候補にしない)。
    CacheHit,
}

/// `CandidateSource` に対応する重み(score 加算量)を返す。
///
/// # Postconditions
///
/// - `Dict` → [`WEIGHT_DICT`]
/// - `Llm` → [`WEIGHT_LLM`]
/// - `CacheHit` → [`BONUS_CACHE_HIT`]
pub fn weight_for(source: CandidateSource) -> f32 {
    match source {
        CandidateSource::Dict => WEIGHT_DICT,
        CandidateSource::Llm => WEIGHT_LLM,
        CandidateSource::CacheHit => BONUS_CACHE_HIT,
    }
}

/// 複数 source の候補を merge / dedupe / sort する。
///
/// # Algorithm
///
/// 1. 同一 surface の候補は max-score 採用で dedupe(BTreeMap で surface → max(weighted_score))
/// 2. weighted_score = candidate.score + weight_for(source) で計算
///    (注: candidate.score は backend が返す raw 値、weight は source bias)
/// 3. CacheHit 由来は dict / llm score に加算する bonus 扱い(独立 candidate ではなく既存 entry の score 加算)
/// 4. 最終的に weighted_score 降順 sort、`top_k` 件に切り詰める
///
/// # Phase 2 spec §3.3 ref
///
/// 「同一 surface が両方で hit した場合は User 側の score を優先する」を実現するため、
/// UserVocab 由来の候補は `Dict` source として扱い、SudachiDict 由来より先に挿入されると
/// max-score 採用で勝つ前提で順序を制御する。
///
/// # Preconditions
///
/// - `top_k` は候補上限。0 を渡した場合は空 `Vec` を返す(spec §3.3 mirror)
///
/// # Postconditions
///
/// - 戻り値は weighted_score 降順 sort 済みで、長さ ≤ `top_k`
/// - 同一 surface は最大 1 件まで(max-score 採用で集約)
pub fn merge_candidates(
    sources: Vec<(Vec<Candidate>, CandidateSource)>,
    cache_hits: &[String],
    top_k: usize,
) -> Vec<Candidate> {
    // 早期 return: top_k=0 で全件 truncate しても結果は同じだが、cap=0 を渡すケースを
    // 明示的に拾うことで callers の意図(「結果不要」)を可視化する。
    if top_k == 0 {
        return Vec::new();
    }

    let mut by_surface: BTreeMap<String, f32> = BTreeMap::new();

    for (cands, source) in sources {
        let w = weight_for(source);
        for c in cands {
            let weighted = c.score + w;
            by_surface
                .entry(c.surface)
                .and_modify(|s| *s = s.max(weighted))
                .or_insert(weighted);
        }
    }

    // CacheHit bonus は当該 surface が dict / llm の merge 結果に存在するときだけ
    // 加算する(spec §3.3 mirror、独立 candidate にはしない)。
    for surface in cache_hits {
        if let Some(s) = by_surface.get_mut(surface) {
            *s += BONUS_CACHE_HIT;
        }
    }

    let mut merged: Vec<Candidate> = by_surface
        .into_iter()
        .map(|(surface, score)| Candidate::new(surface, score))
        .collect();
    merged.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    merged.truncate(top_k);
    merged
}

#[cfg(test)]
mod tests {
    use super::*;

    /// spec §3.3: 同一 surface は max-score 採用で dedupe される
    #[test]
    fn dedupe_takes_max_score() {
        let dict = vec![Candidate::new("琴葉", -2.0)];
        let llm = vec![Candidate::new("琴葉", -1.0)];
        let merged = merge_candidates(
            vec![(dict, CandidateSource::Dict), (llm, CandidateSource::Llm)],
            &[],
            10,
        );
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].surface, "琴葉");
        // weighted: dict=-2.0+0.95=-1.05, llm=-1.0+1.0=0.0 → llm 勝ち
        assert!((merged[0].score - 0.0).abs() < 1e-5);
    }

    /// spec §3.3: CacheHit bonus が該当 surface に加算される
    #[test]
    fn cache_hit_adds_bonus() {
        let dict = vec![Candidate::new("琴葉", 0.0)];
        let merged = merge_candidates(vec![(dict, CandidateSource::Dict)], &["琴葉".into()], 10);
        assert_eq!(merged.len(), 1);
        // weighted: 0.0 + 0.95(dict) + 0.5(cache hit) = 1.45
        assert!((merged[0].score - 1.45).abs() < 1e-5);
    }

    /// 候補 0 件は空 Vec を返す
    #[test]
    fn empty_input_returns_empty() {
        let merged = merge_candidates(vec![], &[], 10);
        assert!(merged.is_empty());
    }

    /// top_k で切り詰められる
    #[test]
    fn truncates_to_top_k() {
        let dict = vec![
            Candidate::new("a", 3.0),
            Candidate::new("b", 2.0),
            Candidate::new("c", 1.0),
        ];
        let merged = merge_candidates(vec![(dict, CandidateSource::Dict)], &[], 2);
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0].surface, "a");
        assert_eq!(merged[1].surface, "b");
    }

    /// score 降順 sort
    #[test]
    fn sorts_by_score_descending() {
        let dict = vec![
            Candidate::new("low", 0.0),
            Candidate::new("high", 10.0),
            Candidate::new("mid", 5.0),
        ];
        let merged = merge_candidates(vec![(dict, CandidateSource::Dict)], &[], 10);
        assert_eq!(merged[0].surface, "high");
        assert_eq!(merged[1].surface, "mid");
        assert_eq!(merged[2].surface, "low");
    }
}
