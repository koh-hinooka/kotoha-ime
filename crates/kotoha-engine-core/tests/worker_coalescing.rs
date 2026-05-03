//! L2-core integration test:`RankerWorker` の coalescing window /
//! `request_id` mismatch / cancel propagation を assert する。
//!
//! spec §7 / §8 の規約検証。Phase 3-A 本番実装段階で coalescing window
//! 値の empirical 調整と合わせて再評価される(spec §13 Open Q 2)。

#![cfg(feature = "test-helpers")]

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::thread::sleep;
use std::time::Duration;

use kotoha_core::Candidate;
use kotoha_engine_core::cancel::CancellationToken;
use kotoha_engine_core::engine::KotohaEngine;
use kotoha_engine_core::ime_engine::IMEEngine;
use kotoha_engine_core::key_event::{KeyEvent, KeyModifiers};
use kotoha_engine_core::ranker::{
    CandidateUpdate, ConversionContext, Ranker, RankerError, RankerOutput,
};
use kotoha_engine_core::testing::MockHostBridge;

/// Cancel observation を assert するための Ranker。
///
/// `rank()` は spawn した thread で `delay` だけ sleep してから cancel を check
/// する。cancel が立っていれば `cancel_observed` をインクリメント、
/// 立っていなければ通常の Candidates を sink.send する。
struct CancelObservingRanker {
    cancel_observed: Arc<AtomicU64>,
    delay: Duration,
}

impl Ranker for CancelObservingRanker {
    fn rank(
        &self,
        _kana: &str,
        _ctx: &ConversionContext,
        cancel: Arc<dyn CancellationToken>,
        sink: std::sync::mpsc::Sender<RankerOutput>,
    ) -> Result<(), RankerError> {
        let counter = self.cancel_observed.clone();
        let delay = self.delay;
        std::thread::spawn(move || {
            sleep(delay);
            if cancel.is_cancelled() {
                counter.fetch_add(1, Ordering::SeqCst);
                return;
            }
            let _ = sink.send(RankerOutput {
                update: CandidateUpdate::Replace(vec![Candidate::new("か", -1.0)]),
            });
        });
        Ok(())
    }
}

#[derive(Default)]
struct StubWriter;
impl kotoha_engine_core::learning_port::LearningRecorder for StubWriter {
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

fn key(c: char) -> KeyEvent {
    KeyEvent {
        keysym: c as u32,
        keycode: 0,
        modifiers: KeyModifiers::empty(),
    }
}

/// spec §8.2: 連続 keypress で 1 つ目の RankRequest が cancel されることを観測する。
#[test]
fn consecutive_typing_cancels_previous_rank_request() {
    let cancel_count = Arc::new(AtomicU64::new(0));
    let ranker = Arc::new(CancelObservingRanker {
        cancel_observed: cancel_count.clone(),
        delay: Duration::from_millis(40),
    });
    let host = Box::new(MockHostBridge::new());
    let writer = Arc::new(StubWriter);
    let mut eng = KotohaEngine::new(host, ranker, writer).expect("engine spawn");
    eng.enable();
    eng.focus_in();

    // 各 'a' が「あ」を commit するため毎回 RankRequest が走る。
    // 1 つ目の RankRequest は 2 つ目で cancel されるはず。
    eng.process_key_event(key('a'));
    eng.process_key_event(key('a'));

    sleep(Duration::from_millis(100));
    assert!(
        cancel_count.load(Ordering::SeqCst) >= 1,
        "expected at least 1 cancel observation, got {}",
        cancel_count.load(Ordering::SeqCst)
    );
}

/// spec §9.1 row 2: `Ranker::rank` が panic しても worker thread は生存し、
/// engine は次 keystroke を引き続き処理できる(catch_unwind による recovery)。
///
/// # B0g-c #148 / I13 half-dead engine guard
///
/// 旧 test は `second_calls >= 1` のみ assert していた。worker が生きていて
/// 第 2 keystroke で rank が走ったことは確認できるが、**engine 全体の state
/// が正常か**(preedit が想定通り構築されているか / 候補が host に届いている
/// か / state machine が LiveConverting に到達しているか)は観測していない。
/// 本 fix で以下 3 件を AND 追加:
///
/// - `eng.preedit_for_test() == "あい"`(第 1 「あ」+ 第 2 「い」)
/// - `eng.state_for_test() == LiveConverting`
/// - host 側 operations に `UpdatePreedit { text: "あい", ... }` が含まれる
#[test]
fn worker_recovers_after_ranker_panic() {
    use kotoha_engine_core::testing::{await_until, HostOperation};
    use std::sync::atomic::{AtomicBool, AtomicU64};

    /// 第 1 回 rank で panic、第 2 回以降は正常 send する Ranker。
    struct FlakyRanker {
        first_call: AtomicBool,
        second_calls: Arc<AtomicU64>,
    }
    impl Ranker for FlakyRanker {
        fn rank(
            &self,
            _kana: &str,
            _ctx: &ConversionContext,
            _cancel: Arc<dyn CancellationToken>,
            sink: std::sync::mpsc::Sender<RankerOutput>,
        ) -> Result<(), RankerError> {
            if self
                .first_call
                .swap(false, std::sync::atomic::Ordering::SeqCst)
            {
                panic!("intentional ranker panic for catch_unwind regression");
            }
            self.second_calls
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            let _ = sink.send(RankerOutput {
                update: CandidateUpdate::Replace(vec![Candidate::new("あ", -1.0)]),
            });
            Ok(())
        }
    }

    let second_calls = Arc::new(AtomicU64::new(0));
    let ranker = Arc::new(FlakyRanker {
        first_call: AtomicBool::new(true),
        second_calls: second_calls.clone(),
    });
    let host = MockHostBridge::new();
    let host_handle = host.clone();
    let host_box: Box<dyn kotoha_engine_core::IMEHostBridge> = Box::new(host);
    let writer = Arc::new(StubWriter);
    let mut eng = KotohaEngine::new(host_box, ranker, writer).expect("engine spawn");
    eng.enable();
    eng.focus_in();

    // 第 1 回 keystroke:Ranker が panic するが、worker は WorkerError を engine に
    // 送って継続。engine 主 thread はそれを観測して Idle 復帰する(spec §9.1 row 2)。
    eng.process_key_event(key('a'));

    // 第 2 回 keystroke:worker が生存していれば 2 度目の rank が走り
    // second_calls がインクリメントされる。
    eng.process_key_event(key('i'));

    // worker thread の処理を polling で待つ(CI flaky 防止、I12 helper)。
    await_until(
        || second_calls.load(std::sync::atomic::Ordering::SeqCst) >= 1,
        Duration::from_millis(500),
    )
    .expect("worker should service second key within 500ms");

    // I13 三重 AND assert:half-dead engine pass を防ぐ。
    assert_eq!(
        eng.preedit_for_test(),
        "あい",
        "preedit should be built across panic recovery: あ + い"
    );
    assert_eq!(
        eng.state_for_test(),
        kotoha_engine_core::EngineState::LiveConverting,
        "engine should be in LiveConverting after second keystroke"
    );
    let ops = host_handle.operations();
    assert!(
        ops.iter().any(|op| matches!(
            op,
            HostOperation::UpdatePreedit { text, .. } if text == "あい"
        )),
        "host should have received UpdatePreedit(text=\"あい\"); ops={ops:?}"
    );
}

/// spec §5.2: `focus_out` で `active_request` の cancel_token が fire される。
#[test]
fn focus_out_cancels_active_request() {
    let cancel_count = Arc::new(AtomicU64::new(0));
    let ranker = Arc::new(CancelObservingRanker {
        cancel_observed: cancel_count.clone(),
        delay: Duration::from_millis(40),
    });
    let host = Box::new(MockHostBridge::new());
    let writer = Arc::new(StubWriter);
    let mut eng = KotohaEngine::new(host, ranker, writer).expect("engine spawn");
    eng.enable();
    eng.focus_in();

    // 'a' が「あ」を commit して RankRequest が走る → focus_out で cancel。
    eng.process_key_event(key('a'));
    eng.focus_out();

    sleep(Duration::from_millis(100));
    assert!(
        cancel_count.load(Ordering::SeqCst) >= 1,
        "expected focus_out to cancel active request, got {}",
        cancel_count.load(Ordering::SeqCst)
    );
}

// ------------------------------------------------------------------
// Phase 3-B B0c (ISSUE #140): missing spec §8.1 cancel triggers
// ------------------------------------------------------------------

/// 共通 helper:CancelObservingRanker + StubWriter で engine 構築
fn build_engine_for_cancel_test(delay_ms: u64) -> (KotohaEngine, Arc<AtomicU64>) {
    let cancel_count = Arc::new(AtomicU64::new(0));
    let ranker = Arc::new(CancelObservingRanker {
        cancel_observed: cancel_count.clone(),
        delay: Duration::from_millis(delay_ms),
    });
    let host = Box::new(MockHostBridge::new());
    let writer = Arc::new(StubWriter);
    let mut eng = KotohaEngine::new(host, ranker, writer).expect("engine spawn");
    eng.enable();
    eng.focus_in();
    (eng, cancel_count)
}

/// spec §8.1 trigger 4: `Esc` key on LiveConverting で cancel 観測
#[test]
fn escape_on_live_cancels_active_request() {
    let (mut eng, cancel_count) = build_engine_for_cancel_test(40);

    eng.process_key_event(key('a'));
    eng.process_key_event(key_special_for_cancel(
        kotoha_engine_core::engine::transitions::keysyms::ESCAPE,
    ));

    sleep(Duration::from_millis(100));
    assert!(
        cancel_count.load(Ordering::SeqCst) >= 1,
        "expected Esc to cancel active request, got {}",
        cancel_count.load(Ordering::SeqCst)
    );
}

/// spec §8.1 trigger 5: `IMEEngine::reset()` で cancel 観測
#[test]
fn reset_cancels_active_request() {
    use kotoha_engine_core::ime_engine::IMEEngine as _;
    let (mut eng, cancel_count) = build_engine_for_cancel_test(40);

    eng.process_key_event(key('a'));
    eng.reset();

    sleep(Duration::from_millis(100));
    assert!(
        cancel_count.load(Ordering::SeqCst) >= 1,
        "expected reset() to cancel active request, got {}",
        cancel_count.load(Ordering::SeqCst)
    );
}

/// spec §8.1 trigger 3: 連続 space(別 RankRequest 開始)で先 request の cancel
#[test]
fn consecutive_space_cancels_previous_request() {
    let (mut eng, cancel_count) = build_engine_for_cancel_test(40);

    eng.process_key_event(key('a'));
    eng.process_key_event(key_special_for_cancel(
        kotoha_engine_core::engine::transitions::keysyms::SPACE,
    ));
    eng.process_key_event(key_special_for_cancel(
        kotoha_engine_core::engine::transitions::keysyms::SPACE,
    ));

    sleep(Duration::from_millis(100));
    assert!(
        cancel_count.load(Ordering::SeqCst) >= 1,
        "expected consecutive space to cancel previous request, got {}",
        cancel_count.load(Ordering::SeqCst)
    );
}

/// spec §8.1 trigger 2 (backspace path): backspace で preedit を空にする際の cancel
#[test]
fn backspace_to_idle_cancels_active_request() {
    let (mut eng, cancel_count) = build_engine_for_cancel_test(40);

    eng.process_key_event(key('a'));
    eng.process_key_event(key_special_for_cancel(
        kotoha_engine_core::engine::transitions::keysyms::BACKSPACE,
    ));

    sleep(Duration::from_millis(100));
    assert!(
        cancel_count.load(Ordering::SeqCst) >= 1,
        "expected backspace-to-idle to cancel active request, got {}",
        cancel_count.load(Ordering::SeqCst)
    );
}

fn key_special_for_cancel(keysym: u32) -> KeyEvent {
    KeyEvent {
        keysym,
        keycode: 0,
        modifiers: KeyModifiers::empty(),
    }
}

// ------------------------------------------------------------------
// Phase 3-B B0d (Important 8): worker 空 buffer 時の Replace 送信
// ------------------------------------------------------------------

/// `apply_candidate_update` の prior_was_nonempty 経路 → host に Clear + hide
/// という **boundary 規約** を engine 直接呼び出しで assert する unit-style test。
///
/// # B0g-c #148 / I10 theater fix(unit boundary 部、self-review C2 で split)
///
/// worker chain end-to-end は別 test
/// (`silent_ranker_end_to_end_clears_host_via_worker_chain`)で cover する。
/// 本 test は `apply_candidate_update_for_test` 直接呼びで chain timing から
/// 独立に boundary 規約のみを pin する。
#[test]
fn silent_ranker_apply_boundary_clears_host_when_prior_was_nonempty() {
    use kotoha_engine_core::engine::KotohaEngine;
    use kotoha_engine_core::testing::{HostOperation, MockCandidateUpdate};

    /// engine 構築のためだけの最小 Ranker(本 test では rank() は呼ばれない)。
    struct NoopRanker;
    impl Ranker for NoopRanker {
        fn rank(
            &self,
            _kana: &str,
            _ctx: &ConversionContext,
            _cancel: Arc<dyn CancellationToken>,
            _sink: std::sync::mpsc::Sender<RankerOutput>,
        ) -> Result<(), RankerError> {
            Ok(())
        }
    }

    let host = MockHostBridge::new();
    let host_handle = host.clone();
    let host_box: Box<dyn kotoha_engine_core::IMEHostBridge> = Box::new(host);
    let writer = Arc::new(StubWriter);
    let mut eng = KotohaEngine::new(host_box, Arc::new(NoopRanker), writer).expect("engine spawn");

    // (1) prior_was_nonempty path 用の setup:engine 内 candidates を直接 populate。
    eng.apply_candidate_update_for_test(CandidateUpdate::Replace(vec![Candidate::new("あ", -1.0)]));
    assert_eq!(eng.candidate_count_for_test(), 1);
    host_handle.clear();

    // (2) silent ranker 由来の空 Replace を simulate(apply 直接呼びで chain timing
    //     から独立)。
    eng.apply_candidate_update_for_test(CandidateUpdate::Replace(Vec::new()));

    // (3) host への通知:prior=non-empty, new=empty → Clear + hide が必須(spec §9.3)。
    let ops = host_handle.operations();
    assert!(
        ops.iter().any(|op| matches!(
            op,
            HostOperation::UpdateCandidates(MockCandidateUpdate::Clear)
        )),
        "expected UpdateCandidates(Clear) but got {ops:?}"
    );
    assert!(
        ops.iter()
            .any(|op| matches!(op, HostOperation::HideCandidateWindow)),
        "expected HideCandidateWindow but got {ops:?}"
    );

    // (4) engine 内 candidates も空。
    assert_eq!(eng.candidate_count_for_test(), 0);
}

/// silent ranker(`Ranker::rank` が `sink.send` を 1 度も呼ばずに Ok 復帰)を
/// 経由した worker → engine → host の **end-to-end chain** で、前回表示の
/// 候補が IBus 側 lookup table から確実に消える(spec §9.3「変換失敗で
/// 前回候補が画面に残る」防止)を assert する。
///
/// # B0g-c #148 / I10 theater fix(end-to-end 部、self-review C2 で split)
///
/// 本 test の核心:`dispatch_rank_request` 内で導入した「pre-clear に表示中
/// 候補があり、drain 後も engine.candidates が空のままなら host に Clear +
/// hide」path を polling helper(I12)で検証する。CI scheduler 圧迫を吸収する。
#[test]
fn silent_ranker_end_to_end_clears_host_via_worker_chain() {
    use kotoha_engine_core::engine::KotohaEngine;
    use kotoha_engine_core::ime_engine::IMEEngine;
    use kotoha_engine_core::testing::{await_until, HostOperation, MockCandidateUpdate};
    use std::sync::atomic::{AtomicBool, Ordering};

    /// 第 1 回 rank で固定候補、第 2 回以降は silent path。
    struct ToggleRanker {
        first_call: AtomicBool,
    }
    impl Ranker for ToggleRanker {
        fn rank(
            &self,
            _kana: &str,
            _ctx: &ConversionContext,
            _cancel: Arc<dyn CancellationToken>,
            sink: std::sync::mpsc::Sender<RankerOutput>,
        ) -> Result<(), RankerError> {
            if self.first_call.swap(false, Ordering::SeqCst) {
                let _ = sink.send(RankerOutput {
                    update: CandidateUpdate::Replace(vec![Candidate::new("あ", -1.0)]),
                });
            }
            Ok(())
        }
    }

    let host = MockHostBridge::new();
    let host_handle = host.clone();
    let host_box: Box<dyn kotoha_engine_core::IMEHostBridge> = Box::new(host);
    let writer = Arc::new(StubWriter);
    let ranker = Arc::new(ToggleRanker {
        first_call: AtomicBool::new(true),
    });
    let mut eng = KotohaEngine::new(host_box, ranker, writer).expect("engine spawn");
    eng.enable();
    eng.focus_in();

    // 第 1 keystroke で候補を populate(prior path のための setup)。
    eng.process_key_event(key('a'));
    await_until(
        || eng.candidate_count_for_test() >= 1,
        Duration::from_millis(500),
    )
    .expect("first keystroke should populate candidates within 500ms");
    host_handle.clear();

    // 第 2 keystroke で silent ranker → worker 空 Replace → dispatch_rank_request
    // post-drain 経路で host.update_candidates(Clear) + hide が発火する。
    eng.process_key_event(key('i'));
    await_until(
        || {
            eng.flush_pending_events_for_test();
            let ops = host_handle.operations();
            ops.iter().any(|op| {
                matches!(
                    op,
                    HostOperation::UpdateCandidates(MockCandidateUpdate::Clear)
                )
            }) && ops
                .iter()
                .any(|op| matches!(op, HostOperation::HideCandidateWindow))
        },
        Duration::from_millis(1000),
    )
    .expect(
        "worker chain should propagate silent-ranker empty Replace and engine should emit \
         host.Clear + hide within 1s",
    );

    assert_eq!(eng.candidate_count_for_test(), 0);
    assert_eq!(eng.preedit_for_test(), "あい");
}

// ------------------------------------------------------------------
// Phase 3-B B0g (ISSUE #148): I16 deterministic ranker panic circuit breaker
// ------------------------------------------------------------------

/// 毎回 panic する Ranker。worker 連続 panic 上限到達 → worker exit →
/// engine が IME-disabled に degrade することを観測する fixture。
struct AlwaysPanicRanker;
impl Ranker for AlwaysPanicRanker {
    fn rank(
        &self,
        _kana: &str,
        _ctx: &ConversionContext,
        _cancel: Arc<dyn CancellationToken>,
        _sink: std::sync::mpsc::Sender<RankerOutput>,
    ) -> Result<(), RankerError> {
        panic!("intentional ranker panic for circuit-breaker regression");
    }
}

/// I16: Ranker.rank が deterministic に panic する場合、worker は連続 5 回で
/// exit し、engine 主 thread は次 dispatch で `tx_request.send` の Err を
/// 観測 → `enabled = false` に degrade する(spec §9.3「IME-disabled mode を
/// user に通知」)。
///
/// MAX_CONSECUTIVE_PANICS=5 のため、6 回目以降の dispatch で channel
/// disconnect が観測される設計。本 test では 10 回 keystroke を送って
/// final state が IME-disabled であることを確認する。
///
/// # Theater pattern 防御(self-review #4)
///
/// 本 test の `enabled = false` 観測は、production code 中で **`tx_request.send`
/// Err path 経由でのみ** 立つ前提に依存する。将来別経路(例:`focus_out` で
/// disable 同等処理を追加する refactor)で `enabled = false` を立てる変更が
/// 入ると本 test の検証根拠が変わる:
///
/// - 補助 assert 1:`state == Idle` を併せて assert(circuit breaker 経由は
///   degrade_to_idle で必ず Idle に倒れるが、別経路は state を変えないかも)
/// - 補助 assert 2:`candidate_count == 0` を併せて assert(同上)
///
/// flaky 化防止のため sleep margin を 20ms → 50ms に拡大(CI scheduler 圧迫
/// 時の worker thread schedule 遅延吸収)。
#[test]
fn worker_circuit_breaker_disables_engine_after_repeated_ranker_panics() {
    let ranker = Arc::new(AlwaysPanicRanker);
    let host = Box::new(MockHostBridge::new());
    let writer = Arc::new(StubWriter);
    let mut eng = KotohaEngine::new(host, ranker, writer).expect("engine spawn");
    eng.enable();
    eng.focus_in();

    // 10 keystroke 連続(MAX_CONSECUTIVE_PANICS=5 を超えて margin 確保)。
    // 最初の 5 回は WorkerError event 経由で degrade_to_idle、6 回目以降に
    // worker が channel close 済で tx_request.send Err → enabled=false。
    for c in ['a', 'i', 'u', 'e', 'o', 'k', 's', 't', 'n', 'h'].iter() {
        eng.process_key_event(key(*c));
        // worker thread に panic + channel close 反映の余裕を与える。
        sleep(Duration::from_millis(50));
    }

    // 三重 AND assert:circuit breaker 経由 degrade を特定。
    assert!(
        !eng.enabled_for_test(),
        "engine should have degraded to IME-disabled after consecutive Ranker panics; \
         enabled_for_test() returned true"
    );
    assert_eq!(
        eng.state_for_test(),
        kotoha_engine_core::EngineState::Idle,
        "circuit breaker degrade path must end at Idle (degrade_to_idle invariant)"
    );
    assert_eq!(
        eng.candidate_count_for_test(),
        0,
        "circuit breaker degrade path must clear candidate buffer"
    );
}
