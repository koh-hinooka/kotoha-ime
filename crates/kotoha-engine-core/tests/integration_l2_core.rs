//! L2-core integration test: KotohaEngine 4 代表シナリオ(spec §10.3)。
//!
//! - typing → space → commit (Enter)
//! - backspace
//! - focus_out
//! - Esc on candidates

#![cfg(feature = "test-helpers")]

use std::sync::Arc;
use std::time::Duration;

use kotoha_core::Candidate;
use kotoha_engine_core::engine::transitions::keysyms;
use kotoha_engine_core::engine::KotohaEngine;
use kotoha_engine_core::ime_engine::IMEEngine;
use kotoha_engine_core::key_event::{KeyEvent, KeyModifiers};
use kotoha_engine_core::testing::{HostOperation, MockHostBridge, MockRanker};

#[derive(Default)]
struct StubWriter;
impl kotoha_engine_core::learning_port::LearningRecorder for StubWriter {
    fn record_choice(
        &self,
        _kana: &str,
        _kanji: &str,
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

fn build(
    candidates: Vec<Candidate>,
) -> (
    KotohaEngine,
    MockHostBridge,
    crossbeam_channel::Receiver<kotoha_engine_core::reactor::Event>,
) {
    let host = MockHostBridge::new();
    let host_clone = host.clone();
    let ranker = Arc::new(MockRanker::new(candidates));
    let writer = Arc::new(StubWriter);
    let (worker_event_tx, worker_event_rx) = crossbeam_channel::unbounded();
    let mut eng = KotohaEngine::new(Box::new(host_clone), ranker, writer, worker_event_tx)
        .expect("engine spawn");
    eng.enable();
    eng.focus_in();
    (eng, host, worker_event_rx)
}

/// Scenario A: typing「kotoha」(7 chars)→ space → top 候補 Enter で commit_text 観測
#[test]
fn scenario_typing_space_enter() {
    let (mut eng, host, worker_event_rx) = build(vec![Candidate::new("琴葉", -1.0)]);
    for c in "kotoha".chars() {
        eng.process_key_event(key_char(c));
    }
    eng.process_key_event(key_special(keysyms::SPACE));
    // Phase 3-B B0h-f rev3 (ADR 0020):worker output が engine-loop role の
    // pump で engine state に反映されないと CandidatesShown 遷移しないため、
    // RETURN 前に pump で候補を吸い上げる。
    std::thread::sleep(Duration::from_millis(80));
    eng.pump_worker_events_for_test(&worker_event_rx);
    eng.process_key_event(key_special(keysyms::RETURN));
    let ops = host.operations();
    assert!(
        ops.iter()
            .any(|o| matches!(o, HostOperation::CommitText(s) if s == "琴葉")),
        "expected CommitText('琴葉') in {ops:?}"
    );
}

/// Scenario B: typing → backspace で preedit shrink、再 typing で kana 出力が継続する
#[test]
fn scenario_typing_backspace_typing() {
    let (mut eng, _host, _worker_event_rx) = build(vec![Candidate::new("か", -1.0)]);
    eng.process_key_event(key_char('k'));
    eng.process_key_event(key_char('a'));
    eng.process_key_event(key_special(keysyms::BACKSPACE));
    eng.process_key_event(key_char('a'));
    // preedit が空→「あ」に変化(『か』→『』→『あ』)
    assert_eq!(eng.preedit_for_test(), "あ");
}

/// Scenario C: focus_out で全 clear + state Idle
#[test]
fn scenario_focus_out_clears() {
    let (mut eng, host, _worker_event_rx) = build(vec![Candidate::new("か", -1.0)]);
    eng.process_key_event(key_char('k'));
    eng.process_key_event(key_char('a'));
    eng.process_key_event(key_special(keysyms::SPACE));
    eng.focus_out();
    let ops = host.operations();
    assert!(ops
        .iter()
        .any(|o| matches!(o, HostOperation::HideCandidateWindow)));
    assert!(eng.preedit_for_test().is_empty());
}

/// Scenario D: CandidatesShown で Esc → preedit kana 維持で LiveConverting に戻る
#[test]
fn scenario_esc_on_candidates_back_to_live() {
    let (mut eng, host, worker_event_rx) = build(vec![Candidate::new("か", -1.0)]);
    eng.process_key_event(key_char('k'));
    eng.process_key_event(key_char('a'));
    eng.process_key_event(key_special(keysyms::SPACE));
    // Phase 3-B B0h-f rev3:CommitConverting → CandidatesShown 遷移には pump が要る
    std::thread::sleep(Duration::from_millis(80));
    eng.pump_worker_events_for_test(&worker_event_rx);
    host.clear();
    eng.process_key_event(key_special(keysyms::ESCAPE));
    assert_eq!(eng.preedit_for_test(), "か");
    let ops = host.operations();
    assert!(ops
        .iter()
        .any(|o| matches!(o, HostOperation::HideCandidateWindow)));
}
