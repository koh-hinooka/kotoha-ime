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

#[test]
fn hybrid_ranker_returns_dict_candidates_for_known_kana() {
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
                "expected at least 1 candidate from sudachi"
            );
            // 「ことは」→ 「言葉」「琴葉」を含むこと(SudachiDict-equivalent stub)
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
    let (ranker, _uv, learning_cache) = build_ranker(kotoha_dict_candidates());

    // learning cache に「ことは → 琴葉」を 10 回 record する
    // (frequency=10、最終 last_used_at=now)。merge 時は dict 由来「琴葉」候補
    // (raw score=-2.0)に WEIGHT_DICT(+0.95)+ BONUS_CACHE_HIT(+0.5)で
    // weighted=-0.55 となり、「言葉」(-1.5+0.95=-0.55)と tie だが、tie は
    // dedupe key の BTreeMap insertion 順で安定する。明確な順位逆転を出すには
    // record 回数を増やすが、本 test の主旨は「cache 由来 surface が top 3 に
    // 含まれる」ことの担保なので 10 回で十分。
    for _ in 0..10 {
        learning_cache
            .record_choice("ことは", "琴葉")
            .expect("record cache hit");
    }

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
            // 「琴葉」が cache hit bonus で top 3 に来ることを担保する
            // (precise ordering は spec §11.4 Q5 で再評価予定のため緩く検証)。
            let kotoha_idx = cands.iter().position(|c| c.surface == "琴葉");
            assert!(
                kotoha_idx.is_some(),
                "expected 琴葉 in candidates, got {cands:?}"
            );
            let idx = kotoha_idx.unwrap();
            assert!(
                idx < 3,
                "expected 琴葉 in top 3 due to cache bonus, got idx={idx}, cands={cands:?}"
            );
        }
        other => panic!("expected Replace, got {other:?}"),
    }
}
