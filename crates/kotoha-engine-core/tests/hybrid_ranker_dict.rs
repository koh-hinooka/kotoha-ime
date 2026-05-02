//! L2 integration test: HybridRanker dict-only path
//!
//! P2-D spec §10.3 L2-core integration の代表シナリオを、stub `MorphologicalEngine`
//! + 実 `MockUserVocabStore` + 実 `MockLearningCacheStore` を組合せた end-to-end で検証する。
//!
//! # SudachiAdapter を直接構築しない理由
//!
//! `kotoha-core::dict::sudachi_adapter::SudachiAdapter` は `pub(crate)` で外部
//! crate からは構築できず、唯一の公開経路は `KOTOHA_SYSTEM_DICT_PATH` 経由の
//! `DictionaryBackend::load(...)`(disk から SudachiDict-core file を読む)。
//! この依存は P2-A の `dict-smoke` feature 側で `kanji_dictionary_unit.rs` 等が
//! 既に網羅しており、本 test は HybridRanker の merge / cancel propagation /
//! UserVocab priority / LearningCache bonus の正しさのみに focus する
//! (Plan 2026-05-02-feature-120-p2d-hybrid-ranker.md §M2 Adaptation 3)。
//!
//! 実 SudachiDict と HybridRanker の end-to-end 連動は Phase 1 14/15 regression
//! 復活時(M3)に `dict-smoke` feature で別 test ファイルとして追加する。

use std::sync::mpsc;
use std::sync::Arc;
use std::time::Duration;

use kotoha_core::dict::{EngineCandidate, MorphologicalEngine};
use kotoha_core::kanji::KanjiError;
use kotoha_engine_core::{
    cancel::StdCancellationToken, CancellationToken, CandidateUpdate, ConversionContext,
    ConversionMode, HybridRanker, Ranker,
};
use kotoha_storage::learning_cache::{
    LearningCacheReader, LearningCacheWriter, MockLearningCacheStore,
};
use kotoha_storage::user_vocab::{
    MockUserVocabStore, UserVocabReader, UserVocabRecord, UserVocabWriter,
};

/// dict-smoke 不要な stub。`SudachiAdapter::tokenize` の呼び出し contract を
/// 満たし、reading に対して固定 `EngineCandidate` を返す。
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

/// 「ことは」に対する代表的な dict 候補(SudachiDict 標準収録に近い形を再現)。
fn kotoha_dict_candidates() -> Vec<EngineCandidate> {
    vec![
        EngineCandidate {
            surface: "言葉".into(),
            reading: "ことば".into(),
            score: -1.5,
        },
        EngineCandidate {
            surface: "琴葉".into(),
            reading: "ことは".into(),
            score: -2.0,
        },
        EngineCandidate {
            surface: "ことは".into(),
            reading: "ことは".into(),
            score: -3.0,
        },
    ]
}

fn build_ranker(
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

/// **Stub-engine pass-through test, NOT a SudachiDict end-to-end test.**
///
/// 本 test は `HybridRanker::rank()` が stub engine から受け取った candidates を
/// merge layer 経由で正しく sink まで pass-through することのみを検証する。
/// SudachiDict 自体の lexicographic 正しさ(「ことは」→「言葉」「琴葉」収録)は
/// 本 test では検証されず、`KOTOHA_SYSTEM_DICT_PATH` 経由の dict-smoke gated test
/// (M3 / Phase 3-A 本番で追加予定、follow-up ISSUE 候補)で別途扱う。
#[test]
fn hybrid_ranker_propagates_stub_engine_candidates_to_sink() {
    let (ranker, _uv, _lc) = build_ranker(kotoha_dict_candidates());
    let cancel = Arc::new(StdCancellationToken::new());
    let (tx, rx) = mpsc::channel();

    let ctx = ConversionContext::empty(ConversionMode::Live);
    ranker
        .rank("ことは", &ctx, cancel.clone(), tx)
        .expect("rank should accept request");

    let output = rx
        .recv_timeout(Duration::from_secs(5))
        .expect("ranker should respond within 5s");

    match output.update {
        CandidateUpdate::Replace(cands) => {
            assert!(
                !cands.is_empty(),
                "expected at least 1 candidate from stub engine"
            );
            // stub engine が返した「葉」を含む candidates が sink まで pass-through
            // されることを確認(merge layer の transparent 動作の確認)。
            assert!(
                cands.iter().any(|c| c.surface.contains("葉")),
                "expected candidate containing 葉, got {cands:?}"
            );
        }
        other => panic!("expected Replace, got {other:?}"),
    }
}

#[test]
fn hybrid_ranker_respects_cancel_before_send() {
    let (ranker, _uv, _lc) = build_ranker(kotoha_dict_candidates());
    let cancel = Arc::new(StdCancellationToken::new());
    let (tx, rx) = mpsc::channel();

    // 即時 cancel: rank() 起動前にトークンを cancel する。
    cancel.cancel();

    let ctx = ConversionContext::empty(ConversionMode::Live);
    ranker
        .rank("ことは", &ctx, cancel.clone(), tx)
        .expect("rank should accept request");

    // cancel 後は sink に send されないため、recv_timeout が timeout する。
    let res = rx.recv_timeout(Duration::from_millis(500));
    assert!(
        res.is_err(),
        "expected timeout (no send after cancel), got {res:?}"
    );
}

#[test]
fn hybrid_ranker_reflects_user_vocab_priority() {
    let (ranker, user_vocab, _lc) = build_ranker(kotoha_dict_candidates());
    // user vocab に「ことは → 私のキャラ」を score=100 で追加。
    // weighted_score = 100.0 + WEIGHT_DICT(0.95) = 100.95 で dict 由来
    // (-1.5 + 0.95 = -0.55)を圧倒し top に来る。
    let now = 1_700_000_000;
    user_vocab
        .insert(UserVocabRecord {
            id: None,
            surface: "私のキャラ".to_string(),
            reading: "ことは".to_string(),
            pos: "名詞".to_string(),
            score: 100.0,
            created_at: now,
            updated_at: now,
        })
        .expect("insert user vocab");

    let cancel = Arc::new(StdCancellationToken::new());
    let (tx, rx) = mpsc::channel();
    let ctx = ConversionContext::empty(ConversionMode::Commit);
    ranker
        .rank("ことは", &ctx, cancel.clone(), tx)
        .expect("rank should accept request");

    let output = rx
        .recv_timeout(Duration::from_secs(5))
        .expect("ranker should respond within 5s");

    match output.update {
        CandidateUpdate::Replace(cands) => {
            assert!(
                cands.iter().take(3).any(|c| c.surface == "私のキャラ"),
                "expected user_vocab entry in top 3, got {cands:?}"
            );
        }
        other => panic!("expected Replace, got {other:?}"),
    }
}

#[test]
fn hybrid_ranker_reflects_learning_cache_bonus() {
    // Theater pattern 回避(spec compliance reviewer Issue 4 対応):
    // 「上位 N 件に出る」だけだと alphabetical / 安定 sort の偶然で pass する可能性が
    // あるため、本 test は **cache 有り / 無しの 2 回 rank を実行し、bonus 加算で
    // 「琴葉」の score が +BONUS_CACHE_HIT 程度上がること** を直接観測する。
    //
    // raw score(stub):「言葉」=-1.5,「琴葉」=-2.0、両者とも WEIGHT_DICT(+0.95)
    // - cache 無し:「言葉」=-0.55、「琴葉」=-1.05 → 「言葉」上位
    // - cache 有り:「琴葉」=-1.05+0.5=-0.55、「言葉」=-0.55 → 同 score だが
    //   「琴葉」は cache bonus を受けたことが score 値で確認可能
    use kotoha_engine_core::ranker::merge::BONUS_CACHE_HIT;

    let (ranker_no_cache, _uv1, _lc1) = build_ranker(kotoha_dict_candidates());
    let cancel1 = Arc::new(StdCancellationToken::new());
    let (tx1, rx1) = mpsc::channel();
    let ctx1 = ConversionContext::empty(ConversionMode::Commit);
    ranker_no_cache
        .rank("ことは", &ctx1, cancel1.clone(), tx1)
        .expect("rank baseline");
    let baseline = rx1
        .recv_timeout(Duration::from_secs(5))
        .expect("baseline response");

    let baseline_kotoha_score = match baseline.update {
        CandidateUpdate::Replace(ref cands) => cands
            .iter()
            .find(|c| c.surface == "琴葉")
            .map(|c| c.score)
            .expect("琴葉 in baseline"),
        ref other => panic!("expected Replace, got {other:?}"),
    };

    // 第 2 ranker: learning_cache に「ことは → 琴葉」を 10 回 record
    let (ranker_with_cache, _uv2, learning_cache) = build_ranker(kotoha_dict_candidates());
    for _ in 0..10 {
        learning_cache
            .record_choice("ことは", "琴葉")
            .expect("record cache hit");
    }

    let cancel2 = Arc::new(StdCancellationToken::new());
    let (tx2, rx2) = mpsc::channel();
    let ctx2 = ConversionContext::empty(ConversionMode::Commit);
    ranker_with_cache
        .rank("ことは", &ctx2, cancel2.clone(), tx2)
        .expect("rank with cache");
    let with_cache = rx2
        .recv_timeout(Duration::from_secs(5))
        .expect("with-cache response");

    let with_cache_kotoha_score = match with_cache.update {
        CandidateUpdate::Replace(ref cands) => cands
            .iter()
            .find(|c| c.surface == "琴葉")
            .map(|c| c.score)
            .expect("琴葉 in with-cache result"),
        ref other => panic!("expected Replace, got {other:?}"),
    };

    // 直接観測:bonus 加算分の差が score 値に出ること
    let delta = with_cache_kotoha_score - baseline_kotoha_score;
    assert!(
        (delta - BONUS_CACHE_HIT).abs() < 1e-5,
        "expected cache bonus delta = {BONUS_CACHE_HIT}, got delta={delta} (baseline={baseline_kotoha_score}, with_cache={with_cache_kotoha_score})"
    );
}
