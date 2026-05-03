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
impl kotoha_storage::learning_cache::LearningCacheWriter for StubWriter {
    fn record_choice(
        &self,
        _kana_input: &str,
        _chosen_kanji: &str,
    ) -> Result<(), kotoha_storage::error::StorageError> {
        Ok(())
    }
    fn evict_lru(&self, _max_entries: usize) -> Result<usize, kotoha_storage::error::StorageError> {
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
#[test]
fn worker_recovers_after_ranker_panic() {
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
    let host = Box::new(MockHostBridge::new());
    let writer = Arc::new(StubWriter);
    let mut eng = KotohaEngine::new(host, ranker, writer).expect("engine spawn");
    eng.enable();
    eng.focus_in();

    // 第 1 回 keystroke:Ranker が panic するが、worker は WorkerError を engine に
    // 送って継続。engine 主 thread はそれを観測して Idle 復帰する(spec §9.1 row 2)。
    eng.process_key_event(key('a'));

    // 第 2 回 keystroke:worker が生存していれば 2 度目の rank が走り
    // second_calls がインクリメントされる。
    eng.process_key_event(key('i'));

    // worker thread の処理を待つ。
    sleep(Duration::from_millis(50));
    assert!(
        second_calls.load(std::sync::atomic::Ordering::SeqCst) >= 1,
        "worker should have survived first-call panic and serviced second key, got {}",
        second_calls.load(std::sync::atomic::Ordering::SeqCst)
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

/// `Ranker::rank` が 1 回も `sink.send` せず Ok 復帰する場合、worker は空 Replace を
/// engine に送る(spec §9.3「変換失敗で前回候補が画面に残る」を防ぐ)。本 test は
/// 「typing で候補表示」→「next typing で別 reading の SilentRanker が走り
/// 候補が clear される」end-to-end shape で verify する。
#[test]
fn silent_ranker_clears_engine_candidates_via_empty_replace() {
    use kotoha_engine_core::engine::KotohaEngine;
    use kotoha_engine_core::ime_engine::IMEEngine;

    /// `rank` で何も送らずに Ok 復帰する Ranker。
    struct SilentRanker;
    impl Ranker for SilentRanker {
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

    let host = Box::new(MockHostBridge::new());
    let writer = Arc::new(StubWriter);
    let mut eng = KotohaEngine::new(host, Arc::new(SilentRanker), writer).expect("engine spawn");
    eng.enable();
    eng.focus_in();

    // 'a' typing で SilentRanker が走る → worker から空 Replace が来て engine の
    // candidates は空のまま、state は LiveConverting(preedit 「あ」)で安定する。
    eng.process_key_event(key('a'));
    assert_eq!(eng.candidate_count_for_test(), 0);
    assert_eq!(eng.preedit_for_test(), "あ");

    // 待機して worker の処理を確実に消化する。
    sleep(Duration::from_millis(20));
    eng.process_key_event(key('i'));
    // 第 2 keystroke 入口の drain_pending_events で前 request の空 Replace が
    // 適用済み(engine 側の candidates は既に空)。新 SilentRanker request も
    // 同じ Empty Replace を返す → candidates 空のまま。
    assert_eq!(eng.candidate_count_for_test(), 0);
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
