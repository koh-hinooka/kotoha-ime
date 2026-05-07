//! `LinuxReactor` 統合テスト(Phase 3-B B0h-f + B3 / ADR 0020 §影響「shutdown
//! ordering」 acceptance criteria)。
//!
//! - shutdown 経路:`shutdown_tx` drop / send で `Event::Shutdown` が届くこと
//! - fan-in 経路:bridge_tx / worker_tx の単一 channel 内 send 順は保たれる
//! - recv_timeout の挙動:event 無時に Timeout を返すこと
//! - bursty event 経路:多数 event を流しても全て届くこと
//!
//! 対応 spec: `docs/specs/_uncategorized/p3-a-ibus-engine.md` §6.0 / §7

use std::time::{Duration, Instant};

use kotoha_engine_core::reactor::{Event, EventReactor, IBusResetKind, ReactorError};
use kotoha_engine_reactor_linux::{start, ReactorHandles};

/// `shutdown_tx` を drop するだけで `Event::Shutdown` が `recv()` から
/// 観測されること(全 Sender drop の片側を起動経路として確認)。
#[test]
fn shutdown_signal_terminates_recv_loop() {
    let ReactorHandles {
        reactor,
        bridge_tx,
        worker_tx,
        shutdown_tx,
    } = start();
    // bridge_tx / worker_tx を保持したまま shutdown_tx だけ drop すると
    // `select!` の shutdown_rx arm で Err 復帰しても OK の event を送る経路と
    // 同等になる(impl 側で `Ok(Event::Shutdown)` に正規化)。
    drop(shutdown_tx);

    let start_t = Instant::now();
    let handle = std::thread::spawn(move || loop {
        match reactor.recv() {
            Ok(Event::Shutdown) => break,
            Ok(_) => continue,
            Err(_) => break,
        }
    });
    handle.join().expect("recv thread join");
    let elapsed = start_t.elapsed();
    // shutdown_tx drop は同期的に observable のため、blocking recv は即時抜ける想定
    assert!(
        elapsed < Duration::from_millis(500),
        "shutdown should be observed within 500ms but took {elapsed:?}"
    );
    // bridge_tx / worker_tx は drop せず保持したまま join するため、
    // この時点で channel は alive。test 終了で自動 drop される。
    drop(bridge_tx);
    drop(worker_tx);
}

/// `bridge_tx` 経路で 2 件 send → reactor.recv() で同順に取り出せる
/// (single channel 内の順序保存)。
#[test]
fn fan_in_preserves_arrival_order_within_single_channel() {
    let ReactorHandles {
        reactor,
        bridge_tx,
        worker_tx: _worker_tx,
        shutdown_tx: _shutdown_tx,
    } = start();
    bridge_tx
        .send(Event::IBusReset(IBusResetKind::FocusOut))
        .expect("send 1");
    bridge_tx
        .send(Event::IBusReset(IBusResetKind::Reset))
        .expect("send 2");

    let first = reactor.recv().expect("recv 1");
    let second = reactor.recv().expect("recv 2");
    assert!(
        matches!(first, Event::IBusReset(IBusResetKind::FocusOut)),
        "first event should be FocusOut, got {first:?}"
    );
    assert!(
        matches!(second, Event::IBusReset(IBusResetKind::Reset)),
        "second event should be Reset, got {second:?}"
    );
}

/// `recv_timeout` は queue が空のとき `Timeout` を返すこと。
#[test]
fn recv_timeout_returns_timeout_when_quiet() {
    let ReactorHandles {
        reactor,
        bridge_tx: _bridge_tx,
        worker_tx: _worker_tx,
        shutdown_tx: _shutdown_tx,
    } = start();
    let res = reactor.recv_timeout(Duration::from_millis(20));
    assert!(
        matches!(res, Err(ReactorError::Timeout)),
        "expected ReactorError::Timeout, got {res:?}"
    );
}

/// bursty event:bridge / worker 両 channel に多数 send → reactor.recv で
/// 全件届く(crossbeam select! の公平性 / starvation 無し)。
#[test]
fn bursty_events_are_all_delivered() {
    let ReactorHandles {
        reactor,
        bridge_tx,
        worker_tx,
        shutdown_tx: _shutdown_tx,
    } = start();
    let bridge_clone = bridge_tx.clone();
    let worker_clone = worker_tx.clone();

    let producer = std::thread::spawn(move || {
        for i in 0..500_u64 {
            bridge_clone
                .send(Event::IBusReset(IBusResetKind::Reset))
                .expect("bridge send");
            worker_clone
                .send(Event::WorkerOutput {
                    request_id: i,
                    payload: kotoha_engine_core::reactor::WorkerPayload::Error("bench".into()),
                })
                .expect("worker send");
        }
    });

    let mut count = 0_u32;
    while count < 1000 {
        match reactor.recv_timeout(Duration::from_millis(500)) {
            Ok(_) => count += 1,
            Err(_) => break,
        }
    }
    producer.join().expect("producer join");
    assert_eq!(count, 1000, "expected 1000 events, got {count}");
}

/// `Event::Shutdown` を bridge_tx 経由で受信できる(Shutdown variant 自体の
/// 配送経路としても使える)。
#[test]
fn explicit_shutdown_event_is_received() {
    let ReactorHandles {
        reactor,
        bridge_tx,
        worker_tx: _worker_tx,
        shutdown_tx: _shutdown_tx,
    } = start();
    bridge_tx.send(Event::Shutdown).expect("send Shutdown");
    let ev = reactor.recv().expect("recv Shutdown");
    assert!(
        matches!(ev, Event::Shutdown),
        "expected Event::Shutdown, got {ev:?}"
    );
}
