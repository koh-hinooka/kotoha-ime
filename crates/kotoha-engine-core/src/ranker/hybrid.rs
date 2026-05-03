//! `HybridRanker` — SudachiDict + UserVocab + LearningCache + LLM の統合 Ranker。
//!
//! 本 module は M2 で dict-only(SudachiDict + UserVocab + LearningCache)を実装し、
//! M3 で LLM backend 統合を追加する。
//!
//! Phase 3-A spec §4.3 で凍結された Ranker trait の concrete impl。

use std::sync::mpsc;
use std::sync::Arc;
use std::thread;

use kotoha_core::dict::MorphologicalEngine;
use kotoha_core::kanji::KanjiBackend;
use kotoha_core::{Candidate, ConvertOptions};
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
/// # Stale response の discard (Phase 3-B B0e で簡素化)
///
/// 以前は `request_id_seed` で内部 counter を維持し `RankerOutput.request_id`
/// に stamp していたが、`Ranker::rank` の trait signature が engine 側
/// `request_id` を引数で受けない設計のため、Ranker 側で stamp しても engine 側
/// で意味のある照合は不可能(B5 で worker レベルの id 照合を撤去した時点で
/// dead surface 化していた)。`RankerOutput.request_id` field を撤去した
/// (Important 10、ISSUE #140)。
///
/// Stale response の discard は engine 主 thread 側の `RankRequest`/`active_request`
/// ベース id 照合 + per-request channel 不変条件で十分に成立する(spec §7.5)。
pub struct HybridRanker {
    sudachi: Arc<dyn MorphologicalEngine + Send + Sync>,
    user_vocab: Arc<dyn UserVocabReader>,
    learning_cache: Arc<dyn LearningCacheReader>,
    /// LLM backend(Phase 1 Gemma-2-2B-jpn-it / MockBackend / 将来 backend)。
    /// `None` なら dict-only 動作(M2 path、lefthook pre-push 既定)。
    /// `Some(_)` を builder [`Self::with_llm`] で設定すると、`rank()` 内で
    /// dict 結果 push 後に LLM convert を呼び、結果 merge 後の 2 段 push が走る。
    ///
    /// `Send + Sync` を要求するのは `rank()` 内の `thread::spawn` で
    /// closure に move する必要があるため(`KanjiBackend` trait 自体は
    /// `Send + Sync` 不要、Phase 1 単 thread CLI 由来の歴史的設計、
    /// kotoha-core spec §5.3)。
    llm: Option<Arc<dyn KanjiBackend + Send + Sync>>,
}

impl HybridRanker {
    /// `HybridRanker` を構築する(dict-only 構成)。
    ///
    /// # Preconditions
    ///
    /// - `sudachi` は `MorphologicalEngine` 実装(production は `SudachiAdapter`、
    ///   test は in-tree stub)
    /// - `user_vocab` / `learning_cache` は production / test 双方の実装が
    ///   `kotoha-storage` 側で提供される(`Sqlite*Store` / `Mock*Store`)
    ///
    /// # Postconditions
    ///
    /// - 戻り値は LLM backend を持たない(`llm = None`)。LLM を有効化するには
    ///   [`Self::with_llm`] を chain する。
    pub fn new(
        sudachi: Arc<dyn MorphologicalEngine + Send + Sync>,
        user_vocab: Arc<dyn UserVocabReader>,
        learning_cache: Arc<dyn LearningCacheReader>,
    ) -> Self {
        Self {
            sudachi,
            user_vocab,
            learning_cache,
            llm: None,
        }
    }

    /// LLM backend を設定する builder method(consume self / return Self)。
    ///
    /// # Preconditions
    ///
    /// - `llm` は `KanjiBackend + Send + Sync` を満たす(`thread::spawn` 越しに
    ///   move されるため)。`MockBackend` / `LlamaCppBackend` は両者を満たす。
    ///
    /// # Postconditions
    ///
    /// - 戻り値の `rank()` は dict 結果 push 後に LLM convert を呼ぶ 2 段 push 動作に切り替わる。
    /// - LLM 失敗(`Err`)は `tracing::warn!` を残し、第 2 段 push は走らない
    ///   (dict-only fallback、Phase 3-A spec §10.4 graceful degradation)。
    pub fn with_llm(mut self, llm: Arc<dyn KanjiBackend + Send + Sync>) -> Self {
        self.llm = Some(llm);
        self
    }
}

impl Ranker for HybridRanker {
    /// 並列 backend 呼び出しは `std::thread::spawn` で行い、`rank()` は同期 return する
    /// (Phase 3-A spec §4.3 contract)。`cancel.is_cancelled()` を以下の **5 観測点** で
    /// check し、true ならば以降の sink push を停止する。spec §4.3 で要求される 4 phase
    /// (entry / dict 完了後 / LLM 前 / LLM 後)に加え、M2 既存の dict-stage send 直前
    /// guard を保持しているため計 5 観測点となる:
    ///
    /// 1. **entry** — thread 起動直後(spec phase 1)
    /// 2. **dict 完了後** — SudachiDict / UserVocab / LearningCache の並列実行完了直後(spec phase 2)
    /// 3. **dict-stage send 直前**(M2 互換 guard、redundant だが 1 段目 send の最終 gate)
    /// 4. **LLM 前** — `KanjiBackend::convert` 呼び出し直前(spec phase 3)
    /// 5. **LLM 後** — `convert()` 完了直後の send 前 gate(spec phase 4)
    ///
    /// SudachiDict / UserVocab / LearningCache は μs オーダーで完結するため backend
    /// 個別の token 確認は行わない(spec §4.3 動作モデル)。LLM convert の token-level
    /// (10 token 毎)cancel check は `KanjiBackend::convert` の signature 拡張要のため
    /// **Phase 3-A 本番** で対応する(spec §13 Open Q 4)。M3 では convert() 完了後の
    /// cancel check で止まる単純実装に留める。
    ///
    /// # Push 仕様
    ///
    /// - LLM 無効(`llm = None`):dict 結果のみ 1 段 push(M2 と同等)。
    /// - LLM 有効 + LLM 成功:dict 結果を 1 段目 push、続けて dict + llm を再 merge した
    ///   全置換 candidates を 2 段目 push。両段とも `CandidateUpdate::Replace`。
    /// - LLM 有効 + LLM 失敗:dict 結果のみ 1 段 push(graceful degradation)。
    ///   LLM 失敗は `tracing::warn!` で観測する(global feedback「silent_failure 禁止」)。
    ///
    /// # ConversionContext.commit_history
    ///
    /// LLM prompt への `commit_history` 注入は `ConvertOptions` への context field
    /// 追加が必要(現在 top_k / temperature / seed のみ)。M3 は注入なしで動作確認に
    /// 留め、context-aware prompt は P2-D 後続 ISSUE で別 spec として扱う。
    fn rank(
        &self,
        kana: &str,
        ctx: &ConversionContext,
        cancel: Arc<dyn CancellationToken>,
        sink: mpsc::Sender<RankerOutput>,
    ) -> Result<(), RankerError> {
        let kana_owned = kana.to_string();
        // ConversionContext.mode は debug 用 capture(将来 Live mode で LLM skip など
        // mode-dependent behavior を実装する余地)。commit_history 注入は M3 範囲外。
        let _mode = ctx.mode;
        let sudachi = Arc::clone(&self.sudachi);
        let user_vocab = Arc::clone(&self.user_vocab);
        let learning_cache = Arc::clone(&self.learning_cache);
        let llm_opt = self.llm.as_ref().map(Arc::clone);

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

            // merge / dedupe / sort(dict 段)
            // UserVocab と SudachiDict を同一 WEIGHT_DICT で merge する。同一 surface が
            // 競合した場合は max-score 採用のため、UserVocab 側の score が高ければ
            // 自然に優先される(挿入順は無関係、merge_candidates の dedupe コメント参照)。
            // caller(本関数)は UserVocab 由来 entry の score を意図的に高く付ける前提。
            //
            // LLM 段で再 merge するため、dict / user candidates の owned copy を
            // clone しておく(merge は move semantics で消費する)。
            let dict_cands_for_llm = dict_cands.clone();
            let user_cands_for_llm = user_cands.clone();
            let cache_surfaces_for_llm = cache_surfaces.clone();
            let merged = merge_candidates(
                vec![
                    (user_cands, CandidateSource::Dict),
                    (dict_cands, CandidateSource::Dict),
                ],
                &cache_surfaces,
                DEFAULT_TOP_K,
            );

            // Phase 3 (M2 互換): dict 結果 send 直前 cancel check
            if cancel.is_cancelled() {
                return;
            }

            // 1 段目 push:dict 結果。
            // receiver が drop されてたら send error が返るが、「receiver 側 thread が
            // 先に終わる」のは正常 path(engine 側で active request が更新済みのケース、
            // Phase 3-A spec §7.5)。Caller が cleanup 中のため tracing::trace で吸収する。
            if let Err(e) = sink.send(RankerOutput {
                update: CandidateUpdate::Replace(merged),
            }) {
                tracing::trace!(error = ?e, "ranker sink closed before dict send");
                // 1 段目 send で receiver 不在なら 2 段目 LLM 段は試みない
                // (resource waste 回避、cancellation の lazy effect)。
                return;
            }

            // LLM path(`llm = None` なら skip し dict-only 動作 = M2 完全互換)。
            let Some(llm) = llm_opt else {
                return;
            };

            // Phase 3 (LLM 前): LLM convert 呼び出し直前 cancel check
            if cancel.is_cancelled() {
                return;
            }

            // M3 段階では ConvertOptions::default() で呼ぶ(top_k=5 / greedy / seed=0)。
            // commit_history → prompt 注入は ConvertOptions 拡張要のため P2-D 後続
            // ISSUE で対応する(spec §13 Open Q 4 関連)。
            let opts = ConvertOptions::default();
            match llm.convert(&kana_owned, &opts) {
                Ok(llm_cands) => {
                    // Phase 4 (LLM 後): LLM convert 完了直後 cancel check
                    if cancel.is_cancelled() {
                        return;
                    }
                    // 既存 dict + user 結果に LLM 結果を append、改めて merge して全置換 push。
                    let combined = merge_candidates(
                        vec![
                            (user_cands_for_llm, CandidateSource::Dict),
                            (dict_cands_for_llm, CandidateSource::Dict),
                            (llm_cands, CandidateSource::Llm),
                        ],
                        &cache_surfaces_for_llm,
                        DEFAULT_TOP_K,
                    );
                    if let Err(e) = sink.send(RankerOutput {
                        update: CandidateUpdate::Replace(combined),
                    }) {
                        tracing::trace!(error = ?e, "ranker sink closed before LLM send");
                    }
                }
                Err(e) => {
                    // graceful degradation: dict-only fallback、第 2 段 push なし。
                    tracing::warn!(
                        error = ?e,
                        kana = %kana_owned,
                        "LLM backend failed, dict candidates only"
                    );
                }
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
