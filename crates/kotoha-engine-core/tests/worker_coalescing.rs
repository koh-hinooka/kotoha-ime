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
                request_id: 0,
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
    let mut eng = KotohaEngine::new(host, ranker, writer);
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
    let mut eng = KotohaEngine::new(host, ranker, writer);
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
