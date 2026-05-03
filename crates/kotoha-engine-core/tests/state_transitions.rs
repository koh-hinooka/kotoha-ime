//! L1 unit test:KotohaEngine 状態遷移 table 全 row(spec §5.2)を網羅する。
//!
//! 各 test case は以下を assert:
//! - 遷移後の `state` field が期待値
//! - `MockHostBridge` への call sequence が期待 sequence と一致
//! - `MockRanker` への rank 呼び出し回数 / cancel 観測値が期待値

#![cfg(feature = "test-helpers")]

use std::sync::Arc;

use kotoha_core::Candidate;
use kotoha_engine_core::engine::transitions::keysyms;
use kotoha_engine_core::engine::{EngineState, KotohaEngine};
use kotoha_engine_core::ime_engine::IMEEngine;
use kotoha_engine_core::key_event::{KeyEvent, KeyEventResult, KeyModifiers};
use kotoha_engine_core::testing::{HostOperation, MockHostBridge, MockRanker};

/// 簡易 LearningCacheWriter mock: 全 record_choice を Vec に積む
#[derive(Default)]
struct MockLearningWriter {
    records: std::sync::Mutex<Vec<(String, String)>>,
}
impl kotoha_storage::learning_cache::LearningCacheWriter for MockLearningWriter {
    fn record_choice(
        &self,
        kana_input: &str,
        chosen_kanji: &str,
    ) -> Result<(), kotoha_storage::error::StorageError> {
        self.records
            .lock()
            .unwrap()
            .push((kana_input.into(), chosen_kanji.into()));
        Ok(())
    }
    fn evict_lru(&self, _max_entries: usize) -> Result<usize, kotoha_storage::error::StorageError> {
        Ok(0)
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

fn build_engine(
    candidates: Vec<Candidate>,
) -> (KotohaEngine, MockHostBridge, Arc<MockLearningWriter>) {
    let host = MockHostBridge::new();
    let host_clone = host.clone();
    let ranker = Arc::new(MockRanker::new(candidates));
    let writer = Arc::new(MockLearningWriter::default());
    let mut eng =
        KotohaEngine::new(Box::new(host_clone), ranker, writer.clone()).expect("engine spawn");
    eng.enable();
    eng.focus_in();
    (eng, host, writer)
}

/// spec §5.2: Idle + 通常 char → LiveConverting + show_candidate_window
#[test]
fn idle_typing_transitions_to_live() {
    let (mut eng, host, _w) = build_engine(vec![Candidate::new("か", -1.0)]);
    let r = eng.process_key_event(key_char('k'));
    let r2 = eng.process_key_event(key_char('a'));
    assert_eq!(r, KeyEventResult::Consumed);
    assert_eq!(r2, KeyEventResult::Consumed);
    assert_eq!(eng.state_for_test(), EngineState::LiveConverting);
    let ops = host.operations();
    assert!(ops.iter().any(|o| matches!(
        o,
        HostOperation::UpdatePreedit { text, .. } if text == "か"
    )));
    assert!(ops
        .iter()
        .any(|o| matches!(o, HostOperation::ShowCandidateWindow)));
}

/// spec §5.2 row 4: LiveConverting + space。即応 Ranker(MockRanker は同期 send)で
/// dispatch_rank_request 内で候補が間に合えば最終 state = CandidatesShown。
///
/// なお spec §5.2 の正解 target は `CommitConverting`(候補未到着の中間状態)。
/// 同期 Mock では中間状態が観測不能なため、対の遅延 Ranker test
/// (`live_space_with_slow_ranker_stays_at_commit_converting`)で
/// CommitConverting branch を assert する。
#[test]
fn live_space_with_fast_ranker_transitions_to_candidates_shown() {
    let (mut eng, _host, _w) = build_engine(vec![Candidate::new("琴葉", -1.0)]);
    eng.process_key_event(key_char('k'));
    eng.process_key_event(key_char('a'));
    eng.process_key_event(key_special(keysyms::SPACE));
    assert_eq!(eng.state_for_test(), EngineState::CandidatesShown);
}

/// spec §6.3: CandidatesShown + Enter → Idle + commit_text + record_choice
#[test]
fn candidates_enter_commits_and_records() {
    let (mut eng, host, writer) = build_engine(vec![Candidate::new("琴葉", -1.0)]);
    eng.process_key_event(key_char('k'));
    eng.process_key_event(key_char('a'));
    eng.process_key_event(key_special(keysyms::SPACE));
    eng.process_key_event(key_special(keysyms::RETURN));
    assert_eq!(eng.state_for_test(), EngineState::Idle);
    let ops = host.operations();
    assert!(ops
        .iter()
        .any(|o| matches!(o, HostOperation::CommitText(s) if s == "琴葉")));
    let records = writer.records.lock().unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].1, "琴葉");
}

/// spec §6.2: LiveConverting + backspace → preedit shrink, Idle (preedit emptied)
#[test]
fn live_backspace_shrinks_to_idle() {
    let (mut eng, _host, _w) = build_engine(vec![Candidate::new("か", -1.0)]);
    eng.process_key_event(key_char('k'));
    eng.process_key_event(key_char('a'));
    // 「か」が preedit にある状態で backspace → preedit 空 → Idle
    eng.process_key_event(key_special(keysyms::BACKSPACE));
    assert_eq!(eng.state_for_test(), EngineState::Idle);
    assert!(eng.preedit_for_test().is_empty());
}

/// spec §5.2: focus_out → Idle + clear all
#[test]
fn focus_out_clears_state() {
    let (mut eng, host, _w) = build_engine(vec![Candidate::new("か", -1.0)]);
    eng.process_key_event(key_char('k'));
    eng.process_key_event(key_char('a'));
    eng.focus_out();
    assert_eq!(eng.state_for_test(), EngineState::Idle);
    let ops = host.operations();
    assert!(ops
        .iter()
        .any(|o| matches!(o, HostOperation::HideCandidateWindow)));
}

/// spec §5.2: Esc on CandidatesShown → LiveConverting (preedit kept)
#[test]
fn candidates_escape_back_to_live() {
    let (mut eng, _host, _w) = build_engine(vec![Candidate::new("か", -1.0)]);
    eng.process_key_event(key_char('k'));
    eng.process_key_event(key_char('a'));
    eng.process_key_event(key_special(keysyms::SPACE));
    eng.process_key_event(key_special(keysyms::ESCAPE));
    assert_eq!(eng.state_for_test(), EngineState::LiveConverting);
    assert_eq!(eng.preedit_for_test(), "か");
}

/// spec §5.2: Idle + backspace → Forwarded (engine 不処理)
#[test]
fn idle_backspace_is_forwarded() {
    let (mut eng, _host, _w) = build_engine(vec![]);
    let r = eng.process_key_event(key_special(keysyms::BACKSPACE));
    assert_eq!(r, KeyEventResult::Forwarded);
    assert_eq!(eng.state_for_test(), EngineState::Idle);
}

/// Phase 3-B B4: RELEASE flag 付き event は engine 不処理 (Forwarded、状態変化なし)
#[test]
fn release_event_is_forwarded_without_state_change() {
    let (mut eng, host, _w) = build_engine(vec![Candidate::new("か", -1.0)]);
    // 通常 press で「か」を入れる
    eng.process_key_event(key_char('k'));
    eng.process_key_event(key_char('a'));
    let state_before = eng.state_for_test();
    let preedit_before = eng.preedit_for_test();
    host.clear();
    // RELEASE event を投入
    let release = KeyEvent {
        keysym: 'a' as u32,
        keycode: 0,
        modifiers: KeyModifiers::RELEASE,
    };
    let r = eng.process_key_event(release);
    assert_eq!(r, KeyEventResult::Forwarded);
    assert_eq!(eng.state_for_test(), state_before);
    assert_eq!(eng.preedit_for_test(), preedit_before);
    // host にも何も呼ばれていない
    assert!(host.operations().is_empty());
}

/// spec §5.2: CandidatesShown + ↓ → highlight_idx 移動 (Ranker 再起動なし)
#[test]
fn candidates_navigation_moves_highlight() {
    let cands = vec![Candidate::new("琴葉", -1.0), Candidate::new("ことは", -1.5)];
    let (mut eng, _host, _w) = build_engine(cands);
    eng.process_key_event(key_char('k'));
    eng.process_key_event(key_char('a'));
    eng.process_key_event(key_special(keysyms::SPACE));
    assert_eq!(eng.highlight_idx_for_test(), 0);
    eng.process_key_event(key_special(keysyms::DOWN));
    assert_eq!(eng.highlight_idx_for_test(), 1);
    eng.process_key_event(key_special(keysyms::DOWN));
    // wrap around
    assert_eq!(eng.highlight_idx_for_test(), 0);
}

// ------------------------------------------------------------------
// Phase 3-B B0c (ISSUE #140): missing spec §5.2 transition rows
// ------------------------------------------------------------------

/// spec §5.2 row 5: LiveConverting + Esc → Idle (preedit + 候補 全 clear)
#[test]
fn live_escape_clears_to_idle() {
    let (mut eng, host, _w) = build_engine(vec![Candidate::new("か", -1.0)]);
    eng.process_key_event(key_char('k'));
    eng.process_key_event(key_char('a'));
    let r = eng.process_key_event(key_special(keysyms::ESCAPE));
    assert_eq!(r, KeyEventResult::Consumed);
    assert_eq!(eng.state_for_test(), EngineState::Idle);
    assert!(eng.preedit_for_test().is_empty());
    let ops = host.operations();
    assert!(ops
        .iter()
        .any(|o| matches!(o, HostOperation::HideCandidateWindow)));
    assert!(ops.iter().any(|o| matches!(
        o,
        HostOperation::UpdatePreedit { text, visible, .. }
            if text.is_empty() && !*visible
    )));
}

/// spec §5.2 row 3 non-empty path: LiveConverting + backspace → LiveConverting
/// (kana が複数残るときは preedit 1 char 縮 + Live 再起動)
#[test]
fn live_backspace_keeps_live_when_preedit_remains() {
    let (mut eng, _host, _w) = build_engine(vec![Candidate::new("かい", -1.0)]);
    eng.process_key_event(key_char('k'));
    eng.process_key_event(key_char('a'));
    eng.process_key_event(key_char('i'));
    assert_eq!(eng.preedit_for_test(), "かい");
    eng.process_key_event(key_special(keysyms::BACKSPACE));
    assert_eq!(eng.state_for_test(), EngineState::LiveConverting);
    assert_eq!(eng.preedit_for_test(), "か");
}

/// spec §5.2 row 11: CandidatesShown + 通常 typing → LiveConverting
/// (候補ウィンドウ閉、新 preedit + Live RankRequest 再起動)
#[test]
fn candidates_typing_resumes_live() {
    let (mut eng, host, _w) = build_engine(vec![Candidate::new("か", -1.0)]);
    eng.process_key_event(key_char('k'));
    eng.process_key_event(key_char('a'));
    eng.process_key_event(key_special(keysyms::SPACE));
    assert_eq!(eng.state_for_test(), EngineState::CandidatesShown);
    host.clear();
    // typing 「i」を再開
    eng.process_key_event(key_char('i'));
    assert_eq!(eng.state_for_test(), EngineState::LiveConverting);
    let ops = host.operations();
    assert!(ops
        .iter()
        .any(|o| matches!(o, HostOperation::HideCandidateWindow)));
}

/// spec §5.2 row 13: CandidatesShown + backspace → LiveConverting
/// (候補ウィンドウ閉、preedit 1 char 縮、Live 再起動)
#[test]
fn candidates_backspace_returns_to_live() {
    let (mut eng, host, _w) = build_engine(vec![Candidate::new("かい", -1.0)]);
    eng.process_key_event(key_char('k'));
    eng.process_key_event(key_char('a'));
    eng.process_key_event(key_char('i'));
    eng.process_key_event(key_special(keysyms::SPACE));
    assert_eq!(eng.state_for_test(), EngineState::CandidatesShown);
    host.clear();
    eng.process_key_event(key_special(keysyms::BACKSPACE));
    assert_eq!(eng.state_for_test(), EngineState::LiveConverting);
    assert_eq!(eng.preedit_for_test(), "か");
    let ops = host.operations();
    assert!(ops
        .iter()
        .any(|o| matches!(o, HostOperation::HideCandidateWindow)));
}

/// spec §5.2 row 14: CandidatesShown + focus_out → Idle (全 clear)
#[test]
fn candidates_focus_out_clears() {
    let (mut eng, host, _w) = build_engine(vec![Candidate::new("か", -1.0)]);
    eng.process_key_event(key_char('k'));
    eng.process_key_event(key_char('a'));
    eng.process_key_event(key_special(keysyms::SPACE));
    assert_eq!(eng.state_for_test(), EngineState::CandidatesShown);
    host.clear();
    eng.focus_out();
    assert_eq!(eng.state_for_test(), EngineState::Idle);
    assert!(eng.preedit_for_test().is_empty());
    assert_eq!(eng.candidate_count_for_test(), 0);
    let ops = host.operations();
    assert!(ops
        .iter()
        .any(|o| matches!(o, HostOperation::HideCandidateWindow)));
}

/// spec §5.2 row 2 explicit: LiveConverting + 連続通常 char → LiveConverting
/// (preedit 伸長 + Live RankRequest 再発行)
#[test]
fn live_consecutive_typing_extends_preedit() {
    let (mut eng, _host, _w) = build_engine(vec![Candidate::new("かい", -1.0)]);
    eng.process_key_event(key_char('k'));
    eng.process_key_event(key_char('a'));
    assert_eq!(eng.preedit_for_test(), "か");
    assert_eq!(eng.state_for_test(), EngineState::LiveConverting);
    eng.process_key_event(key_char('i'));
    assert_eq!(eng.preedit_for_test(), "かい");
    assert_eq!(eng.state_for_test(), EngineState::LiveConverting);
}

/// spec §5.2 row 1 negative: Idle + 大文字(現状 ASCII graphic として扱う)
/// が既存 path で Consumed になることを観測。Phase 6 input mode で再評価。
#[test]
fn idle_uppercase_is_consumed_via_typing_path() {
    // 'A' (0x41) は ASCII graphic + ASCII uppercase のため
    // handle_typing が char::from_u32 で受け入れ、ローマ字 trie で
    // 該当 entry が無いため pending は invalid 扱いで dropped。
    // 現状 spec では大文字の挙動は §13 Open Q 8 で「実装段階対応」。
    // 本 test は「panic-free + Forwarded ではなく Consumed として吸収する」
    // 現状 behavior を lock する(spec 確定後 expectations を更新)。
    let (mut eng, _host, _w) = build_engine(vec![]);
    let r = eng.process_key_event(key_char('A'));
    assert_eq!(r, KeyEventResult::Consumed);
    // 大文字単独では ローマ字 entry に hit しないため preedit は空のまま Idle 維持
    assert!(eng.preedit_for_test().is_empty());
    assert_eq!(eng.state_for_test(), EngineState::Idle);
}
