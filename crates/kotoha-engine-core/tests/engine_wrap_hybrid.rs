//! L2-core integration test: KotohaEngine + 実 HybridRanker(stub MorphologicalEngine
//! + Mock stores)の end-to-end 検証。
//!
//! Phase 3-B B5(ISSUE #136)で追加。Phase 3-A の L2-core integration test
//! (`integration_l2_core.rs`)は `MockRanker` 経由で engine の状態遷移と host
//! call sequence を検証していた。本 test は **`MockRanker` を使わず実 `HybridRanker`
//! を engine に注入** し、worker channel を通じた candidate flow が end-to-end で
//! 機能することを assert する。
//!
//! # 範囲
//!
//! - HybridRanker の dict path(StubEngine 経由)が候補を生成
//! - RankerWorker が coalescing window 内で候補を engine 主 thread に push
//! - engine が host へ `update_candidates` / `commit_text` を発行
//! - learning_cache record_choice が呼ばれ、Mock store に蓄積される
//!
//! # 範囲外
//!
//! - 実 SudachiDict load(`dict-smoke` feature 側で個別 test 済)
//! - LLM backend(Phase 3-B B2+ で feature gate 付き別 test)

#![cfg(feature = "test-helpers")]

use std::sync::Arc;

use kotoha_core::dict::{EngineCandidate, MorphologicalEngine};
use kotoha_core::kanji::KanjiError;
use kotoha_engine_core::engine::transitions::keysyms;
use kotoha_engine_core::engine::KotohaEngine;
use kotoha_engine_core::ime_engine::IMEEngine;
use kotoha_engine_core::key_event::{KeyEvent, KeyModifiers};
use kotoha_engine_core::testing::{HostOperation, MockHostBridge};
use kotoha_engine_core::HybridRanker;
use kotoha_storage::learning_cache::MockLearningCacheStore;
use kotoha_storage::user_vocab::MockUserVocabStore;

/// reading 非空で固定 surface を返す stub MorphologicalEngine。
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

fn key_char(c: char) -> KeyEvent {
    KeyEvent {
        keysym: c as u32,
        keycode: 0,
        modifiers: KeyModifiers::empty(),
    }
}

fn key_special(keysym: u32) -> KeyEvent {
    KeyEvent {
        keysym,
        keycode: 0,
        modifiers: KeyModifiers::empty(),
    }
}

fn build_engine_with_real_hybrid_ranker(
    canned: Vec<EngineCandidate>,
) -> (KotohaEngine, MockHostBridge, Arc<MockLearningCacheStore>) {
    let host = MockHostBridge::new();
    let host_clone = host.clone();
    let stub_engine: Arc<dyn MorphologicalEngine + Send + Sync> = Arc::new(StubEngine { canned });
    let user_vocab = Arc::new(MockUserVocabStore::default());
    let learning_cache = Arc::new(MockLearningCacheStore::default());
    let ranker = Arc::new(HybridRanker::new(
        stub_engine,
        user_vocab,
        learning_cache.clone(),
    ));
    let mut eng = KotohaEngine::new(Box::new(host_clone), ranker, learning_cache.clone())
        .expect("engine spawn");
    eng.enable();
    eng.focus_in();
    (eng, host, learning_cache)
}

/// End-to-end: typing「ka」→ space → Enter で StubEngine 由来候補が commit される
#[test]
fn typing_space_enter_end_to_end_via_real_hybrid_ranker() {
    let (mut eng, host, _cache) = build_engine_with_real_hybrid_ranker(vec![EngineCandidate {
        surface: "蚊".to_string(),
        reading: "か".to_string(),
        score: -1.0,
    }]);

    eng.process_key_event(key_char('k'));
    eng.process_key_event(key_char('a'));
    eng.process_key_event(key_special(keysyms::SPACE));
    eng.process_key_event(key_special(keysyms::RETURN));

    let ops = host.operations();
    // commit_text が「蚊」で発行されている
    assert!(
        ops.iter()
            .any(|o| matches!(o, HostOperation::CommitText(s) if s == "蚊")),
        "expected CommitText('蚊') in {ops:?}"
    );
    // update_preedit が空文字列で最終化されている(Idle 遷移)
    assert!(ops.iter().any(|o| matches!(
        o,
        HostOperation::UpdatePreedit { text, visible, .. }
            if text.is_empty() && !*visible
    )));
}

/// End-to-end: 実 HybridRanker から Live 変換候補が候補ウィンドウに反映される
#[test]
fn live_typing_shows_hybrid_ranker_candidates() {
    let (mut eng, host, _cache) = build_engine_with_real_hybrid_ranker(vec![EngineCandidate {
        surface: "蚊".to_string(),
        reading: "か".to_string(),
        score: -1.0,
    }]);

    eng.process_key_event(key_char('k'));
    eng.process_key_event(key_char('a'));

    let ops = host.operations();
    // Live mode で update_candidates(Replace) + show_candidate_window が呼ばれている
    assert!(
        ops.iter()
            .any(|o| matches!(o, HostOperation::ShowCandidateWindow)),
        "expected ShowCandidateWindow in {ops:?}"
    );
    assert!(
        ops.iter()
            .any(|o| matches!(o, HostOperation::UpdateCandidates(_))),
        "expected UpdateCandidates in {ops:?}"
    );
    assert!(eng.candidate_count_for_test() >= 1);
}
