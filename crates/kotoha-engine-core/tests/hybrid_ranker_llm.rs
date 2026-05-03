//! L2 integration test: HybridRanker with LLM backend (MockBackend / local failing stub).
//!
//! Phase 3-A spec §10.3 + §10.4 で要求される LLM integrated path を、kotoha-core の
//! `MockBackend`(deterministic fixture)と本テストファイル内 `FailingBackend`
//! (常に `Err` を返す stub)で検証する。
//!
//! # MockBackend と SudachiAdapter を直接構築しない理由
//!
//! - SudachiAdapter は `pub(crate)` のため外部 crate から構築不可。本 test は
//!   M2 dict-only 系と同じ in-test `StubEngine` で `MorphologicalEngine` を満たす
//!   (`hybrid_ranker_dict.rs` と同 pattern)。dict-smoke 系は別 ISSUE で扱う。
//! - LLM 失敗 path 用の "always failing" MockBackend variant は kotoha-core 側に
//!   未実装(`MockBackend::new()` のみ公開)。MockBackend 拡張は P2-D 範囲外と
//!   判断し、本 test ファイル内に最小 `FailingBackend` を local impl で書き起こす
//!   (Plan 2026-05-02-feature-120-p2d-hybrid-ranker.md §M3 Adaptation 5)。
//!
//! # Feature gate
//!
//! `MockBackend` は `kotoha-core/mock-backend` feature gated のため、本ファイル
//! 全体を `#[cfg(feature = "mock-backend")]` で gate する。default features
//! (lefthook pre-push)では本 test は **compile されない / 実行されない**。
//! 実行は `cargo test -p kotoha-engine-core --features mock-backend
//! --test hybrid_ranker_llm` で明示する。

#![cfg(feature = "mock-backend")]

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::time::Duration;

use kotoha_core::dict::{EngineCandidate, MorphologicalEngine};
use kotoha_core::kanji::{KanjiBackend, KanjiError, MockBackend};
use kotoha_core::{Candidate, ConvertOptions};
use kotoha_engine_core::{
    cancel::StdCancellationToken, CancellationToken, CandidateUpdate, ConversionContext,
    ConversionMode, HybridRanker, Ranker,
};
use kotoha_storage::learning_cache::MockLearningCacheStore;
use kotoha_storage::user_vocab::MockUserVocabStore;

/// dict-smoke 不要な stub engine。`hybrid_ranker_dict.rs` と同 pattern。
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

/// 常に `KanjiError::Backend` を返す LLM stub。LLM 失敗 fallback path 検証用。
///
/// 設計判断:`MockBackend` に `with_always_failing()` を生やす拡張を kotoha-core
/// 側に追加する案も検討したが、production surface(`pub use mock::MockBackend`)
/// に test-only API を漏らすコストが大きいため、本 test ファイルに局所 impl を
/// 置く方を採用した(`FailingBackend` は本ファイル外に export しない)。
///
/// `convert()` の呼び出し回数を `Arc<AtomicUsize>` で公開し、test 側で「実際に
/// 呼ばれた事実」を直接観測する(theater pattern 防止、Kotoha
/// `feedback_test_theater_pattern.md` 準拠)。
struct FailingBackend {
    call_count: Arc<AtomicUsize>,
}

impl FailingBackend {
    fn new() -> (Self, Arc<AtomicUsize>) {
        let count = Arc::new(AtomicUsize::new(0));
        (
            Self {
                call_count: count.clone(),
            },
            count,
        )
    }
}

impl KanjiBackend for FailingBackend {
    fn model_id(&self) -> &str {
        "failing-stub"
    }

    fn convert(
        &self,
        _input: &str,
        _options: &ConvertOptions,
    ) -> Result<Vec<Candidate>, KanjiError> {
        self.call_count.fetch_add(1, Ordering::SeqCst);
        Err(KanjiError::Backend {
            reason: "FailingBackend always fails (test stub)".into(),
        })
    }
}

/// 呼び出し回数を観測可能な MockBackend wrapper。cancel test で「LLM が呼ばれて
/// いない」事実を直接観測するため(theater pattern 防止)。
///
/// MockBackend が public API として返すのと同じ deterministic fixture を委譲で
/// 通しつつ、call_count を内部 `Arc<AtomicUsize>` で expose する。
struct CountingMockBackend {
    inner: MockBackend,
    call_count: Arc<AtomicUsize>,
}

impl CountingMockBackend {
    fn new() -> (Self, Arc<AtomicUsize>) {
        let count = Arc::new(AtomicUsize::new(0));
        (
            Self {
                inner: MockBackend::new(),
                call_count: count.clone(),
            },
            count,
        )
    }
}

impl KanjiBackend for CountingMockBackend {
    fn model_id(&self) -> &str {
        self.inner.model_id()
    }

    fn convert(&self, input: &str, options: &ConvertOptions) -> Result<Vec<Candidate>, KanjiError> {
        self.call_count.fetch_add(1, Ordering::SeqCst);
        self.inner.convert(input, options)
    }
}

/// 「にほんご」に対する代表的な dict 候補(`MockBackend` の hiragana fixture と
/// reading が一致する句)。MockBackend は `"にほんご"` で `[日本語, 二本後]` を返す。
fn nihongo_dict_candidates() -> Vec<EngineCandidate> {
    vec![
        EngineCandidate {
            surface: "日本語".into(),
            reading: "にほんご".into(),
            score: -1.5,
        },
        EngineCandidate {
            surface: "二本語".into(),
            reading: "にほんご".into(),
            score: -3.0,
        },
    ]
}

fn build_ranker_with_backend(
    engine_cands: Vec<EngineCandidate>,
    llm: Arc<dyn KanjiBackend + Send + Sync>,
) -> HybridRanker {
    let sudachi: Arc<dyn MorphologicalEngine + Send + Sync> = Arc::new(StubEngine {
        canned: engine_cands,
    });
    let user_vocab =
        kotoha_engine_adapter::arc_mock_user_vocab(Arc::new(MockUserVocabStore::new()));
    let (_recorder, learning_cache) =
        kotoha_engine_adapter::arc_mock_learning_cache(Arc::new(MockLearningCacheStore::new()));
    HybridRanker::new(sudachi, user_vocab, learning_cache).with_llm(llm)
}

/// LLM 成功 path:dict 段(1 段目)の後に LLM 統合段(2 段目)が push される。
///
/// 確認ポイント:
/// - sink には 2 個の `RankerOutput` が届く(両方 `CandidateUpdate::Replace`)
/// - 2 段目には MockBackend が返した「日本語」(LLM 由来 score 0.9 + WEIGHT_LLM=1.0=1.9)
///   または dict 由来「日本語」(stub score -1.5 + WEIGHT_DICT=0.95=-0.55)が
///   max-score 採用 で merge され、weighted score = 1.9(LLM 勝ち)で top に来る
#[test]
fn hybrid_ranker_with_llm_returns_combined_candidates_in_two_stages() {
    let llm: Arc<dyn KanjiBackend + Send + Sync> = Arc::new(MockBackend::new());
    let ranker = build_ranker_with_backend(nihongo_dict_candidates(), llm);
    let cancel = Arc::new(StdCancellationToken::new());
    let (tx, rx) = mpsc::channel();

    let ctx = ConversionContext::empty(ConversionMode::Commit);
    ranker
        .rank("にほんご", &ctx, cancel.clone(), tx)
        .expect("rank should accept request");

    // 1 段目: dict 段(LLM 待ちなし、即時)
    let dict_output = rx
        .recv_timeout(Duration::from_secs(5))
        .expect("dict response within 5s");
    let dict_cands = match dict_output.update {
        CandidateUpdate::Replace(cands) => cands,
        other => panic!("expected Replace for dict stage, got {other:?}"),
    };
    assert!(
        !dict_cands.is_empty(),
        "dict stage should have at least one candidate (stub returned 2)"
    );
    // 1 段目には MockBackend の「日本語」(score=0.9)はまだ含まれていない
    // (1 段目は dict only)。
    let dict_top_score = dict_cands[0].score;

    // 2 段目: dict + LLM 結合 段
    let llm_output = rx
        .recv_timeout(Duration::from_secs(5))
        .expect("LLM-merged response within 5s");
    let combined = match llm_output.update {
        CandidateUpdate::Replace(cands) => cands,
        other => panic!("expected Replace for LLM stage, got {other:?}"),
    };

    assert!(
        !combined.is_empty(),
        "LLM stage should not be empty when MockBackend returns a fixture"
    );
    // MockBackend が「日本語」(0.9) を返すため、combined top は LLM 由来の
    // 「日本語」(weighted = 0.9 + 1.0 = 1.9)になる。
    let combined_top = &combined[0];
    assert_eq!(
        combined_top.surface, "日本語",
        "expected LLM-derived 日本語 at top of combined stage, got {combined:?}"
    );
    assert!(
        combined_top.score > dict_top_score,
        "combined top score ({}) should exceed dict-only top score ({})",
        combined_top.score,
        dict_top_score
    );

    // 3 個目以降は来ない(sink close 後の追加 push は無い)
    let res = rx.recv_timeout(Duration::from_millis(200));
    assert!(
        res.is_err(),
        "expected only 2 RankerOutput messages, got 3rd: {res:?}"
    );
}

/// 即時 cancel:`rank()` 起動前に cancel すると dict 段すら send されない
/// (Phase 1: entry cancel check が effective)。LLM が **呼ばれていない事実**
/// を call counter で直接観測する(theater pattern 防止)。
#[test]
fn hybrid_ranker_with_llm_respects_cancel_before_invocation() {
    let (counting_llm, llm_call_count) = CountingMockBackend::new();
    let llm: Arc<dyn KanjiBackend + Send + Sync> = Arc::new(counting_llm);
    let ranker = build_ranker_with_backend(nihongo_dict_candidates(), llm);
    let cancel = Arc::new(StdCancellationToken::new());
    let (tx, rx) = mpsc::channel();

    cancel.cancel(); // rank() 起動前に cancel

    let ctx = ConversionContext::empty(ConversionMode::Commit);
    ranker
        .rank("にほんご", &ctx, cancel.clone(), tx)
        .expect("rank should accept request even if pre-cancelled");

    // dict / LLM どちらも send されない
    let res = rx.recv_timeout(Duration::from_millis(500));
    assert!(
        res.is_err(),
        "expected timeout (no send after pre-cancel), got {res:?}"
    );

    // theater pattern 防止:LLM convert が **一度も呼ばれていない** 事を直接観測
    // (entry cancel check が pre-LLM phase より前に effective であることを担保)
    assert_eq!(
        llm_call_count.load(Ordering::SeqCst),
        0,
        "LLM convert must NOT have been invoked when cancel fires before rank()"
    );
}

/// LLM 失敗 fallback:`KanjiBackend::convert` が `Err` を返した場合、
/// dict 段(1 段目)のみ push し、2 段目は **送らない**(graceful degradation)。
/// FailingBackend が **実際に呼ばれた事実** を call counter で直接観測する
/// (theater pattern 防止)。
#[test]
fn hybrid_ranker_llm_failure_falls_back_to_dict_only() {
    let (failing, llm_call_count) = FailingBackend::new();
    let llm: Arc<dyn KanjiBackend + Send + Sync> = Arc::new(failing);
    let ranker = build_ranker_with_backend(nihongo_dict_candidates(), llm);
    let cancel = Arc::new(StdCancellationToken::new());
    let (tx, rx) = mpsc::channel();

    let ctx = ConversionContext::empty(ConversionMode::Commit);
    ranker
        .rank("にほんご", &ctx, cancel.clone(), tx)
        .expect("rank should accept request");

    // 1 段目 dict 段は届く
    let dict_output = rx
        .recv_timeout(Duration::from_secs(5))
        .expect("dict response within 5s");
    let dict_cands = match dict_output.update {
        CandidateUpdate::Replace(cands) => cands,
        other => panic!("expected Replace for dict stage, got {other:?}"),
    };
    assert!(
        !dict_cands.is_empty(),
        "dict stage should have candidates regardless of LLM outcome"
    );

    // 2 段目は LLM 失敗で **送られない**(silent_failure 防止のため warn が出るが
    // sink には送らない、graceful degradation)。
    let res = rx.recv_timeout(Duration::from_millis(500));
    assert!(
        res.is_err(),
        "expected no second send after LLM failure, got {res:?}"
    );

    // theater pattern 防止:FailingBackend が **実際に 1 回呼ばれた事実** を
    // 直接観測(LLM path に到達せず fallback したケースと区別する)
    assert_eq!(
        llm_call_count.load(Ordering::SeqCst),
        1,
        "FailingBackend.convert() must have been invoked exactly once before fallback"
    );
}
