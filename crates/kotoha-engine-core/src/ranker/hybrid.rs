//! `HybridRanker` — SudachiDict + UserVocab + LearningCache + LLM の統合 Ranker。
//!
//! 本 module は M2 で dict-only(SudachiDict + UserVocab + LearningCache)を実装し、
//! M3 で LLM backend 統合を追加する。
//!
//! Phase 3-A spec §4.3 で凍結された Ranker trait の concrete impl。

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::thread;

use kotoha_core::dict::MorphologicalEngine;
use kotoha_core::Candidate;
use kotoha_storage::learning_cache::LearningCacheReader;
use kotoha_storage::user_vocab::UserVocabReader;

use super::merge::{merge_candidates, CandidateSource};
use super::{CandidateUpdate, ConversionContext, Ranker, RankerError, RankerOutput};
use crate::cancel::CancellationToken;

/// 候補生成の top_k(暫定、empirical で再評価)。
const DEFAULT_TOP_K: usize = 10;

/// SudachiDict + UserVocab + LearningCache + LLM(M3 で追加)の統合 Ranker。
///
/// # Construction
///
/// `Arc<dyn MorphologicalEngine>` / `Arc<dyn UserVocabReader>` /
/// `Arc<dyn LearningCacheReader>` を構築時に DI で受け取る。LLM backend は
/// M3 で追加し、`Option` 化することで dict-only(M2)動作を保つ。
///
/// # Invariants
///
/// - `request_id_seed` は `rank()` 呼び出し毎に単調増加で採番される
///   (`RankerOutput.request_id` の uniqueness を担保し、engine 主 thread
///   側で stale response の discard に使う、Phase 3-A spec §7.5)
pub struct HybridRanker {
    sudachi: Arc<dyn MorphologicalEngine + Send + Sync>,
    user_vocab: Arc<dyn UserVocabReader>,
    learning_cache: Arc<dyn LearningCacheReader>,
    request_id_seed: AtomicU64,
}

impl HybridRanker {
    /// `HybridRanker` を構築する。
    ///
    /// # Preconditions
    ///
    /// - `sudachi` は `MorphologicalEngine` 実装(production は `SudachiAdapter`、
    ///   test は in-tree stub)
    /// - `user_vocab` / `learning_cache` は production / test 双方の実装が
    ///   `kotoha-storage` 側で提供される(`Sqlite*Store` / `Mock*Store`)
    pub fn new(
        sudachi: Arc<dyn MorphologicalEngine + Send + Sync>,
        user_vocab: Arc<dyn UserVocabReader>,
        learning_cache: Arc<dyn LearningCacheReader>,
    ) -> Self {
        Self {
            sudachi,
            user_vocab,
            learning_cache,
            request_id_seed: AtomicU64::new(0),
        }
    }

    fn next_request_id(&self) -> u64 {
        self.request_id_seed.fetch_add(1, Ordering::SeqCst)
    }
}

impl Ranker for HybridRanker {
    /// 並列 backend 呼び出しは `std::thread::spawn` で行い、`rank()` は同期 return する
    /// (Phase 3-A spec §4.3 contract)。`cancel.is_cancelled()` を 3 phase
    /// (entry / backend 完了直後 / send 直前)で観測し、true ならば以降の sink push を
    /// 停止する。SudachiDict / UserVocab / LearningCache は μs オーダーで完結するため
    /// backend 個別の token 確認は行わない(spec §4.3 動作モデル)。
    fn rank(
        &self,
        kana: &str,
        ctx: &ConversionContext,
        cancel: Arc<dyn CancellationToken>,
        sink: mpsc::Sender<RankerOutput>,
    ) -> Result<(), RankerError> {
        let request_id = self.next_request_id();
        let kana_owned = kana.to_string();
        // ConversionContext は M3 LLM 統合で活用する。M2 では mode のみ debug 用に capture。
        let _mode = ctx.mode;
        let sudachi = Arc::clone(&self.sudachi);
        let user_vocab = Arc::clone(&self.user_vocab);
        let learning_cache = Arc::clone(&self.learning_cache);

        // 並列 backend 呼び出し用 thread を spawn(`rank()` は即時 return)。
        thread::spawn(move || {
            // Phase 1: entry cancel check(early return で thread を即終了)
            if cancel.is_cancelled() {
                return;
            }

            // SudachiDict tokenize
            // backend 個別 error は global feedback「silent_failure 禁止」遵守のため
            // tracing::warn! で観測、empty Vec で fallback(全 backend を block しない)。
            let dict_cands: Vec<Candidate> = match sudachi.tokenize(&kana_owned) {
                Ok(ecs) => ecs
                    .into_iter()
                    .map(|ec| Candidate::new(ec.surface, ec.score))
                    .collect(),
                Err(e) => {
                    tracing::warn!(
                        error = ?e,
                        kana = %kana_owned,
                        "sudachi tokenize failed; using empty dict candidates"
                    );
                    Vec::new()
                }
            };

            // UserVocab prefix lookup
            // `find_by_prefix(reading_prefix, limit)` は `score DESC` で最大 `limit` 件返す
            // (UserVocabReader trait contract)。
            let user_cands: Vec<Candidate> =
                match user_vocab.find_by_prefix(&kana_owned, DEFAULT_TOP_K) {
                    Ok(records) => records
                        .into_iter()
                        .map(|r| Candidate::new(r.surface, r.score))
                        .collect(),
                    Err(e) => {
                        tracing::warn!(
                            error = ?e,
                            kana = %kana_owned,
                            "user_vocab find_by_prefix failed; using empty user candidates"
                        );
                        Vec::new()
                    }
                };

            // LearningCache lookup
            let cache_records = match learning_cache.lookup(&kana_owned, DEFAULT_TOP_K) {
                Ok(records) => records,
                Err(e) => {
                    tracing::warn!(
                        error = ?e,
                        kana = %kana_owned,
                        "learning_cache lookup failed; using empty cache hits"
                    );
                    Vec::new()
                }
            };
            let cache_surfaces: Vec<String> = cache_records
                .iter()
                .map(|r| r.chosen_kanji.clone())
                .collect();

            // Phase 2: backend 完了直後 cancel check
            if cancel.is_cancelled() {
                return;
            }

            // merge / dedupe / sort
            // UserVocab と SudachiDict を同一 WEIGHT_DICT で merge する。同一 surface が
            // 競合した場合は max-score 採用のため、UserVocab 側の score が高ければ
            // 自然に優先される(挿入順は無関係、merge_candidates の dedupe コメント参照)。
            // caller(本関数)は UserVocab 由来 entry の score を意図的に高く付ける前提。
            let merged = merge_candidates(
                vec![
                    (user_cands, CandidateSource::Dict),
                    (dict_cands, CandidateSource::Dict),
                ],
                &cache_surfaces,
                DEFAULT_TOP_K,
            );

            // Phase 3: send 直前 cancel check
            if cancel.is_cancelled() {
                return;
            }

            // sink に push。receiver が drop されてたら send error が返るが、
            // 「receiver 側 thread が先に終わる」のは正常 path(engine 側で
            // active request が更新済みのケース、Phase 3-A spec §7.5)。
            // Caller が cleanup 中のため tracing::trace で吸収する。
            if let Err(e) = sink.send(RankerOutput {
                request_id,
                update: CandidateUpdate::Replace(merged),
            }) {
                tracing::trace!(error = ?e, "ranker sink closed before send");
            }
        });

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cancel::StdCancellationToken;
    use crate::ranker::ConversionMode;
    use kotoha_core::dict::{EngineCandidate, MorphologicalEngine};
    use kotoha_core::kanji::KanjiError;
    use kotoha_storage::learning_cache::MockLearningCacheStore;
    use kotoha_storage::user_vocab::MockUserVocabStore;

    /// In-test stub: dict ファイル不要で `MorphologicalEngine` を満たす。
    /// プロジェクト全体で `dict_backend` の StubEngine と同 pattern。
    struct StubEngine {
        canned: Vec<EngineCandidate>,
    }

    impl MorphologicalEngine for StubEngine {
        fn tokenize(&self, reading: &str) -> Result<Vec<EngineCandidate>, KanjiError> {
            if reading.is_empty() {
                return Ok(Vec::new());
            }
            Ok(self.canned.clone())
        }

        fn engine_id(&self) -> &str {
            "stub-engine"
        }
    }

    fn make_ranker(
        engine_cands: Vec<EngineCandidate>,
    ) -> (
        HybridRanker,
        Arc<MockUserVocabStore>,
        Arc<MockLearningCacheStore>,
    ) {
        let sudachi: Arc<dyn MorphologicalEngine + Send + Sync> = Arc::new(StubEngine {
            canned: engine_cands,
        });
        let user_vocab = Arc::new(MockUserVocabStore::new());
        let learning_cache = Arc::new(MockLearningCacheStore::new());
        let ranker = HybridRanker::new(
            sudachi,
            user_vocab.clone() as Arc<dyn UserVocabReader>,
            learning_cache.clone() as Arc<dyn LearningCacheReader>,
        );
        (ranker, user_vocab, learning_cache)
    }

    /// 別 `rank()` 呼び出しで `request_id` が単調増加することを確認(spec §7.5)。
    #[test]
    fn request_id_is_monotonically_increasing() {
        let (ranker, _uv, _lc) = make_ranker(vec![EngineCandidate {
            surface: "言葉".into(),
            reading: "ことば".into(),
            score: -1.0,
        }]);
        let id1 = ranker.next_request_id();
        let id2 = ranker.next_request_id();
        assert!(id2 > id1, "expected id2 > id1, got id1={id1} id2={id2}");
    }

    /// dict-only path: stub engine の候補が sink に到達する。
    #[test]
    fn rank_returns_dict_candidates_via_stub_engine() {
        let (ranker, _uv, _lc) = make_ranker(vec![EngineCandidate {
            surface: "言葉".into(),
            reading: "ことば".into(),
            score: -1.0,
        }]);
        let cancel = Arc::new(StdCancellationToken::new());
        let (tx, rx) = mpsc::channel();
        let ctx = ConversionContext::empty(ConversionMode::Live);
        ranker.rank("ことば", &ctx, cancel, tx).expect("rank ok");
        let output = rx
            .recv_timeout(std::time::Duration::from_secs(2))
            .expect("ranker should respond");
        match output.update {
            CandidateUpdate::Replace(cands) => {
                assert!(cands.iter().any(|c| c.surface == "言葉"));
            }
            other => panic!("expected Replace, got {other:?}"),
        }
    }
}
