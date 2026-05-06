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

/// 簡易 LearningRecorder mock: 全 record_choice を Vec に積む
#[derive(Default)]
struct MockLearningWriter {
    records: std::sync::Mutex<Vec<(String, String)>>,
}
impl kotoha_engine_core::learning_port::LearningRecorder for MockLearningWriter {
    fn record_choice(
        &self,
        kana_input: &str,
        chosen_kanji: &str,
    ) -> Result<(), kotoha_engine_core::learning_port::LearningError> {
        self.records
            .lock()
            .unwrap()
            .push((kana_input.into(), chosen_kanji.into()));
        Ok(())
    }
    fn evict_lru(
        &self,
        _max_entries: usize,
    ) -> Result<usize, kotoha_engine_core::learning_port::LearningError> {
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
) -> (
    KotohaEngine,
    MockHostBridge,
    Arc<MockLearningWriter>,
    crossbeam_channel::Receiver<kotoha_engine_core::reactor::Event>,
) {
    let host = MockHostBridge::new();
    let host_clone = host.clone();
    let ranker = Arc::new(MockRanker::new(candidates));
    let writer = Arc::new(MockLearningWriter::default());
    let (worker_event_tx, worker_event_rx) = crossbeam_channel::unbounded();
    let mut eng = KotohaEngine::new(
        Box::new(host_clone),
        ranker,
        writer.clone(),
        worker_event_tx,
    )
    .expect("engine spawn");
    eng.enable();
    eng.focus_in();
    (eng, host, writer, worker_event_rx)
}

/// spec §5.2: Idle + 通常 char → LiveConverting + show_candidate_window
#[test]
fn idle_typing_transitions_to_live() {
    let (mut eng, host, _w, _worker_event_rx) = build_engine(vec![Candidate::new("か", -1.0)]);
    let r = eng.process_key_event(key_char('k'));
    let r2 = eng.process_key_event(key_char('a'));
    assert_eq!(r, KeyEventResult::Consumed);
    assert_eq!(r2, KeyEventResult::Consumed);
    assert_eq!(eng.state_for_test(), EngineState::LiveConverting);
    let ops = host.operations();
    // B0g-c #148 / I14: cursor 値も含めて contract 違反を catch する。
    //
    // # cursor 単位
    //
    // production 側 `engine/transitions.rs::handle_typing` は
    // `engine.current_preedit.chars().count()` を cursor として host.update_preedit
    // に渡す(= **Unicode scalar 単位**、UTF-8 byte 数や grapheme cluster 数では
    // ない)。「か」1 文字 = 1 scalar、`len()=3` (UTF-8 3 bytes) ではないこと
    // を本 assert で pin する。byte-length-vs-char-count drift で `cursor=3` に
    // regress する production bug を即時 detect。grapheme cluster 単位への変更
    // を将来検討する場合は spec 側で凍結 + 本 test を新単位に追従させる。
    assert!(
        ops.iter().any(|o| matches!(
            o,
            HostOperation::UpdatePreedit { text, cursor, visible }
                if text == "か" && *cursor == 1 && *visible
        )),
        "expected UpdatePreedit(text=\"か\", cursor=1 (Unicode scalar count), visible=true) \
         but got {ops:?}"
    );
    assert!(ops
        .iter()
        .any(|o| matches!(o, HostOperation::ShowCandidateWindow)));
}

/// I14 demonstration: MockRanker.last_kana / last_mode で `dispatch_rank_request`
/// が正しい kana / mode を Ranker に渡しているかを直接観測する。
///
/// `cursor` 値検証(`idle_typing_transitions_to_live`)と組合わせて、engine →
/// Ranker 境界の引数 contract を二重に守る。
#[test]
fn idle_typing_passes_correct_kana_and_mode_to_ranker() {
    use kotoha_engine_core::ranker::ConversionMode;
    let host = MockHostBridge::new();
    let host_clone = host.clone();
    let ranker = Arc::new(kotoha_engine_core::testing::MockRanker::new(vec![
        Candidate::new("か", -1.0),
    ]));
    let ranker_handle = ranker.clone();
    let writer = Arc::new(MockLearningWriter::default());
    let (worker_event_tx, _worker_event_rx) = crossbeam_channel::unbounded();
    let mut eng = KotohaEngine::new(Box::new(host_clone), ranker, writer, worker_event_tx)
        .expect("engine spawn");
    eng.enable();
    eng.focus_in();
    let _ = host;

    eng.process_key_event(key_char('k'));
    eng.process_key_event(key_char('a'));
    // Phase 3-B B0h-f rev3 (ADR 0020):dispatch は async なので worker thread が
    // Ranker::rank を呼ぶまで少し待つ。本 test は engine state を見ないため pump は不要。
    std::thread::sleep(std::time::Duration::from_millis(40));

    // Live mode で kana="か" が渡る(spec §6.1)。
    assert_eq!(
        ranker_handle.last_kana(),
        Some("か".to_string()),
        "Ranker should receive the current preedit as kana arg"
    );
    assert_eq!(
        ranker_handle.last_mode(),
        Some(ConversionMode::Live),
        "Live keystroke should dispatch in ConversionMode::Live"
    );

    // space で commit mode に切替わる。
    eng.process_key_event(key_special(keysyms::SPACE));
    // rev3 (ADR 0020): worker thread が Ranker::rank を呼ぶまで待機
    std::thread::sleep(std::time::Duration::from_millis(40));
    assert_eq!(
        ranker_handle.last_mode(),
        Some(ConversionMode::Commit),
        "space keystroke should dispatch in ConversionMode::Commit"
    );
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
    let (mut eng, _host, _w, worker_event_rx) = build_engine(vec![Candidate::new("琴葉", -1.0)]);
    eng.process_key_event(key_char('k'));
    eng.process_key_event(key_char('a'));
    eng.process_key_event(key_special(keysyms::SPACE));
    // Phase 3-B B0h-f rev3 (ADR 0020):worker → engine-loop の async path。
    std::thread::sleep(std::time::Duration::from_millis(80));
    eng.pump_worker_events_for_test(&worker_event_rx);
    assert_eq!(eng.state_for_test(), EngineState::CandidatesShown);
}

/// spec §6.3: CandidatesShown + Enter → Idle + commit_text + record_choice
#[test]
fn candidates_enter_commits_and_records() {
    let (mut eng, host, writer, worker_event_rx) = build_engine(vec![Candidate::new("琴葉", -1.0)]);
    eng.process_key_event(key_char('k'));
    eng.process_key_event(key_char('a'));
    eng.process_key_event(key_special(keysyms::SPACE));
    // Phase 3-B B0h-f rev3 (ADR 0020):RETURN で commit するには CandidatesShown
    // 状態が前提。worker output を pump して engine を CandidatesShown に遷移させる。
    std::thread::sleep(std::time::Duration::from_millis(80));
    eng.pump_worker_events_for_test(&worker_event_rx);
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
    let (mut eng, _host, _w, _worker_event_rx) = build_engine(vec![Candidate::new("か", -1.0)]);
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
    let (mut eng, host, _w, _worker_event_rx) = build_engine(vec![Candidate::new("か", -1.0)]);
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
    let (mut eng, _host, _w, worker_event_rx) = build_engine(vec![Candidate::new("か", -1.0)]);
    eng.process_key_event(key_char('k'));
    eng.process_key_event(key_char('a'));
    eng.process_key_event(key_special(keysyms::SPACE));
    // rev3 (ADR 0020): worker → engine-loop async dispatch
    std::thread::sleep(std::time::Duration::from_millis(80));
    eng.pump_worker_events_for_test(&worker_event_rx);
    eng.process_key_event(key_special(keysyms::ESCAPE));
    assert_eq!(eng.state_for_test(), EngineState::LiveConverting);
    assert_eq!(eng.preedit_for_test(), "か");
}

/// spec §5.2: Idle + backspace → Forwarded (engine 不処理)
#[test]
fn idle_backspace_is_forwarded() {
    let (mut eng, _host, _w, _worker_event_rx) = build_engine(vec![]);
    let r = eng.process_key_event(key_special(keysyms::BACKSPACE));
    assert_eq!(r, KeyEventResult::Forwarded);
    assert_eq!(eng.state_for_test(), EngineState::Idle);
}

/// Phase 3-B B4: RELEASE flag 付き event は engine 不処理 (Forwarded、状態変化なし)
#[test]
fn release_event_is_forwarded_without_state_change() {
    let (mut eng, host, _w, _worker_event_rx) = build_engine(vec![Candidate::new("か", -1.0)]);
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
    let (mut eng, _host, _w, worker_event_rx) = build_engine(cands);
    eng.process_key_event(key_char('k'));
    eng.process_key_event(key_char('a'));
    eng.process_key_event(key_special(keysyms::SPACE));
    // rev3 (ADR 0020): worker → engine-loop async dispatch
    std::thread::sleep(std::time::Duration::from_millis(80));
    eng.pump_worker_events_for_test(&worker_event_rx);
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
    let (mut eng, host, _w, _worker_event_rx) = build_engine(vec![Candidate::new("か", -1.0)]);
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
    let (mut eng, _host, _w, _worker_event_rx) = build_engine(vec![Candidate::new("かい", -1.0)]);
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
    let (mut eng, host, _w, worker_event_rx) = build_engine(vec![Candidate::new("か", -1.0)]);
    eng.process_key_event(key_char('k'));
    eng.process_key_event(key_char('a'));
    eng.process_key_event(key_special(keysyms::SPACE));
    std::thread::sleep(std::time::Duration::from_millis(80));
    eng.pump_worker_events_for_test(&worker_event_rx);
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
    let (mut eng, host, _w, worker_event_rx) = build_engine(vec![Candidate::new("かい", -1.0)]);
    eng.process_key_event(key_char('k'));
    eng.process_key_event(key_char('a'));
    eng.process_key_event(key_char('i'));
    eng.process_key_event(key_special(keysyms::SPACE));
    std::thread::sleep(std::time::Duration::from_millis(80));
    eng.pump_worker_events_for_test(&worker_event_rx);
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
    let (mut eng, host, _w, worker_event_rx) = build_engine(vec![Candidate::new("か", -1.0)]);
    eng.process_key_event(key_char('k'));
    eng.process_key_event(key_char('a'));
    eng.process_key_event(key_special(keysyms::SPACE));
    std::thread::sleep(std::time::Duration::from_millis(80));
    eng.pump_worker_events_for_test(&worker_event_rx);
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
    let (mut eng, _host, _w, _worker_event_rx) = build_engine(vec![Candidate::new("かい", -1.0)]);
    eng.process_key_event(key_char('k'));
    eng.process_key_event(key_char('a'));
    assert_eq!(eng.preedit_for_test(), "か");
    assert_eq!(eng.state_for_test(), EngineState::LiveConverting);
    eng.process_key_event(key_char('i'));
    assert_eq!(eng.preedit_for_test(), "かい");
    assert_eq!(eng.state_for_test(), EngineState::LiveConverting);
}

// ------------------------------------------------------------------
// Phase 3-B B0d (ISSUE #140): Critical 4 async path 修正の検証
// ------------------------------------------------------------------
// spec §5.2 row 4: LiveConverting + space → CommitConverting
// spec §5.2 row 7: CommitConverting + RankerOutput → CandidatesShown
// 同期 MockRanker では中間状態 CommitConverting が観測不能なため、遅延 Ranker
// (SlowRanker)を使って中間状態を捕捉する。

/// 指定 delay 後に固定候補を sink.send する Ranker。
struct SlowRanker {
    delay: std::time::Duration,
    candidates: Vec<Candidate>,
}

impl kotoha_engine_core::ranker::Ranker for SlowRanker {
    fn rank(
        &self,
        _kana: &str,
        _ctx: &kotoha_engine_core::ranker::ConversionContext,
        cancel: std::sync::Arc<dyn kotoha_engine_core::cancel::CancellationToken>,
        sink: std::sync::mpsc::Sender<kotoha_engine_core::ranker::RankerOutput>,
    ) -> Result<(), kotoha_engine_core::ranker::RankerError> {
        let delay = self.delay;
        let cands = self.candidates.clone();
        std::thread::spawn(move || {
            std::thread::sleep(delay);
            if cancel.is_cancelled() {
                return;
            }
            let _ = sink.send(kotoha_engine_core::ranker::RankerOutput {
                update: kotoha_engine_core::ranker::CandidateUpdate::Replace(cands),
            });
        });
        Ok(())
    }
}

#[derive(Default)]
struct StubWriterB0d;
impl kotoha_engine_core::learning_port::LearningRecorder for StubWriterB0d {
    fn record_choice(
        &self,
        _kana_input: &str,
        _chosen_kanji: &str,
    ) -> Result<(), kotoha_engine_core::learning_port::LearningError> {
        Ok(())
    }
    fn evict_lru(
        &self,
        _max_entries: usize,
    ) -> Result<usize, kotoha_engine_core::learning_port::LearningError> {
        Ok(0)
    }
}

fn build_engine_with_slow_ranker(
    delay_ms: u64,
    candidates: Vec<Candidate>,
) -> (
    KotohaEngine,
    MockHostBridge,
    crossbeam_channel::Receiver<kotoha_engine_core::reactor::Event>,
) {
    let host = MockHostBridge::new();
    let host_clone = host.clone();
    let ranker = std::sync::Arc::new(SlowRanker {
        delay: std::time::Duration::from_millis(delay_ms),
        candidates,
    });
    let writer = std::sync::Arc::new(StubWriterB0d);
    let (worker_event_tx, worker_event_rx) = crossbeam_channel::unbounded();
    let mut eng = KotohaEngine::new(Box::new(host_clone), ranker, writer, worker_event_tx)
        .expect("engine spawn");
    eng.enable();
    eng.focus_in();
    (eng, host, worker_event_rx)
}

/// spec §5.2 row 4 strict: LiveConverting + space で Ranker が遅延すると state は
/// `CommitConverting` で抜ける(候補未到着の中間状態が観測される)。
/// spec §5.2 strict 適合の対 test。
#[test]
fn live_space_with_slow_ranker_stays_at_commit_converting() {
    // delay を Commit window 35ms より長くして、dispatch 内 drain で候補が
    // 間に合わないようにする。
    let (mut eng, host, _worker_event_rx) =
        build_engine_with_slow_ranker(80, vec![Candidate::new("か", -1.0)]);
    eng.process_key_event(key_char('k'));
    eng.process_key_event(key_char('a'));
    eng.process_key_event(key_special(keysyms::SPACE));
    // 第 1 batch がまだ届いていない → state == CommitConverting
    assert_eq!(eng.state_for_test(), EngineState::CommitConverting);
    // show_candidate_window は spec §5.2 row 4 通り即発行されている
    assert!(host
        .operations()
        .iter()
        .any(|o| matches!(o, HostOperation::ShowCandidateWindow)));
}

// ------------------------------------------------------------------
// Phase 3-B B0g-c (ISSUE #148 / I11): spec §5.2 row 8 — CommitConverting
// 中間状態での Esc / backspace / focus_out / typing 各 trigger
// ------------------------------------------------------------------

/// row 8 path-1: CommitConverting + Esc → Idle + preedit clear
///
/// SlowRanker で CommitConverting に留めた状態で Esc を撃ち、Idle に戻る
/// + preedit が空になることを観測。spec §5.2 row 8 / §6.4。
///
/// self-review C1:旧版は `sleep(100ms) + DOWN keystroke` で「遅れて到着した
/// 候補が CandidatesShown 昇格しない」を assert していたが、これは
/// (a) `sleep(100ms)` 固定 wait が CI scheduler 圧迫で flaky を新規導入し、
/// (b) SlowRanker thread が cancel 経由で sink.send をスキップする path と
/// 「100ms 経っても何も起こらない」path が観測上区別不能(timing dependent
/// theater pattern)、という二重問題があったため削除。Esc→Idle 直後の
/// primary 不変条件のみを残す。
#[test]
fn commit_converting_escape_returns_to_idle() {
    let (mut eng, _host, _worker_event_rx) =
        build_engine_with_slow_ranker(80, vec![Candidate::new("か", -1.0)]);
    eng.process_key_event(key_char('k'));
    eng.process_key_event(key_char('a'));
    eng.process_key_event(key_special(keysyms::SPACE));
    assert_eq!(eng.state_for_test(), EngineState::CommitConverting);
    eng.process_key_event(key_special(keysyms::ESCAPE));
    assert_eq!(eng.state_for_test(), EngineState::Idle);
    assert!(eng.preedit_for_test().is_empty());
}

/// row 8 path-2: CommitConverting + backspace → Idle(preedit 全消去で空に)。
///
/// kana 「か」(1 文字)を持つ CommitConverting で backspace を撃つと
/// preedit が空になり Idle に戻る。
#[test]
fn commit_converting_backspace_returns_to_idle() {
    let (mut eng, _host, _worker_event_rx) =
        build_engine_with_slow_ranker(80, vec![Candidate::new("か", -1.0)]);
    eng.process_key_event(key_char('k'));
    eng.process_key_event(key_char('a'));
    eng.process_key_event(key_special(keysyms::SPACE));
    assert_eq!(eng.state_for_test(), EngineState::CommitConverting);
    eng.process_key_event(key_special(keysyms::BACKSPACE));
    assert_eq!(eng.state_for_test(), EngineState::Idle);
    assert!(eng.preedit_for_test().is_empty());
}

/// row 8 path-3: CommitConverting + focus_out → Idle + preedit clear。
///
/// IME-host が focus 喪失を通知した時、進行中の commit converting を破棄して
/// state を Idle に戻す(spec §5.3 lifecycle)。
#[test]
fn commit_converting_focus_out_returns_to_idle() {
    use kotoha_engine_core::ime_engine::IMEEngine as _;
    let (mut eng, _host, _worker_event_rx) =
        build_engine_with_slow_ranker(80, vec![Candidate::new("か", -1.0)]);
    eng.process_key_event(key_char('k'));
    eng.process_key_event(key_char('a'));
    eng.process_key_event(key_special(keysyms::SPACE));
    assert_eq!(eng.state_for_test(), EngineState::CommitConverting);
    eng.focus_out();
    assert_eq!(eng.state_for_test(), EngineState::Idle);
    assert!(eng.preedit_for_test().is_empty());
}

/// row 8 path-4: CommitConverting + 通常 char → 古い request を cancel し
/// 拡張 preedit で LiveConverting に戻る。
///
/// CommitConverting 中に user が typing を続けた場合、user は変換結果を
/// 待たずに次文字を打ち始めた = commit を諦めた、と解釈する。spec §5.2
/// row 8 / §6.1 で Live mode に降格する。
#[test]
fn commit_converting_typing_returns_to_live_with_extended_preedit() {
    let (mut eng, _host, _worker_event_rx) =
        build_engine_with_slow_ranker(80, vec![Candidate::new("か", -1.0)]);
    eng.process_key_event(key_char('k'));
    eng.process_key_event(key_char('a'));
    eng.process_key_event(key_special(keysyms::SPACE));
    assert_eq!(eng.state_for_test(), EngineState::CommitConverting);
    // 'i' を打つ → preedit が「かい」に拡張、state は LiveConverting に降格。
    eng.process_key_event(key_char('i'));
    assert_eq!(eng.state_for_test(), EngineState::LiveConverting);
    assert_eq!(eng.preedit_for_test(), "かい");
}

/// spec §5.2 row 7: CommitConverting で RankerOutput が遅れて到着すると、
/// engine-loop が `Event::WorkerOutput` を観測した時点で `CandidatesShown` へ遷移する。
///
/// Phase 3-B B0h-f rev3 (ADR 0020):旧 rev2 では process_key_event 入口で
/// `drain_pending_events` が走り CandidatesShown 遷移していた。rev3 では engine-loop
/// thread の `EventReactor` 経路が同役割を担う(test では `pump_worker_events_for_test`
/// で模擬する)。
#[test]
fn commit_converting_transitions_to_candidates_shown_on_late_arrival() {
    let (mut eng, host, worker_event_rx) =
        build_engine_with_slow_ranker(50, vec![Candidate::new("か", -1.0)]);
    eng.process_key_event(key_char('k'));
    eng.process_key_event(key_char('a'));
    eng.process_key_event(key_special(keysyms::SPACE));
    assert_eq!(eng.state_for_test(), EngineState::CommitConverting);
    // 候補到着まで sleep
    std::thread::sleep(std::time::Duration::from_millis(80));
    host.clear();
    // engine-loop 役で worker output を pump → CommitConverting → CandidatesShown 遷移
    eng.pump_worker_events_for_test(&worker_event_rx);
    assert_eq!(eng.state_for_test(), EngineState::CandidatesShown);
    assert!(host
        .operations()
        .iter()
        .any(|o| matches!(o, HostOperation::UpdateCandidates(_))));
}

// ------------------------------------------------------------------
// 既存 §5.2 transitions 続き
// ------------------------------------------------------------------

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
    let (mut eng, _host, _w, _worker_event_rx) = build_engine(vec![]);
    let r = eng.process_key_event(key_char('A'));
    assert_eq!(r, KeyEventResult::Consumed);
    // 大文字単独では ローマ字 entry に hit しないため preedit は空のまま Idle 維持
    assert!(eng.preedit_for_test().is_empty());
    assert_eq!(eng.state_for_test(), EngineState::Idle);
}
