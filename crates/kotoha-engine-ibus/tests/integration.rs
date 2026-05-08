//! Phase 3-B B6-b (#195) — listener thread + engine_loop 経路の L2 integration test。
//!
//! Spec: `docs/specs/_uncategorized/p3-b-ibus-listener.md` §10.1 L2 row。
//!
//! 本 test は dbus session bus に依存するため `#[ignore]` で gate し、
//! `cargo test -p kotoha-engine-ibus -- --ignored` で opt-in 実行する。
//! 通常 CI / pre-push hook では実行されない(L3 manual smoke #196 で実機検証する)。

use std::sync::Arc;
use std::time::Duration;

use crossbeam_channel::unbounded;
use kotoha_engine_core::key_event::KeyEventResult;
use kotoha_engine_core::reactor::Event;
use kotoha_engine_ibus::listener::{run, ListenerShutdown};

/// listener::run が session bus に接続し、Event::IBusKey が bridge_tx 経由で
/// 受け取れる経路を pin する(decode + dispatch の最小経路)。
///
/// 本 test は実 IBus daemon を要求しない:listener が serve_at + name で
/// publish した状態で test 側が D-Bus client として `ProcessKeyEvent` を
/// 呼び出し、bridge_rx で `Event::IBusKey` を観測する。
#[test]
#[ignore = "依存: dbus session bus available; CI で flaky のため opt-in"]
fn listener_decodes_process_key_event() {
    let (bridge_tx, bridge_rx) = unbounded::<Event>();
    let (shutdown_trigger, shutdown_observer) = ListenerShutdown::new();

    let listener_handle = std::thread::Builder::new()
        .name("test-dbus-listener".into())
        .spawn(move || run(bridge_tx, shutdown_observer))
        .expect("spawn listener");

    // listener が bus name 取得を完了するまで短時間待つ(Builder::build() は
    // 同期的に完了するが、別 thread spawn 直後の race を避けるための margin)
    std::thread::sleep(Duration::from_millis(100));

    // test 側が D-Bus method を呼ぶ proxy を組む
    let conn = zbus::blocking::Connection::session().expect("open test session bus");
    let proxy = zbus::blocking::Proxy::new(
        &conn,
        "org.freedesktop.IBus.Engine.Kotoha",
        "/org/freedesktop/IBus/Engine/Kotoha",
        "org.freedesktop.IBus.Engine",
    )
    .expect("build proxy");

    // ProcessKeyEvent 呼び出しは listener 側で respond_rx.recv_timeout を blocking で
    // 待つため、別 thread で event を engine 役として返す
    let bridge_rx_for_engine = Arc::new(bridge_rx);
    let bridge_rx_clone = bridge_rx_for_engine.clone();
    let engine_thread = std::thread::Builder::new()
        .name("test-engine-loop-stub".into())
        .spawn(move || {
            // listener の process_key_event がここに event を送ってくる想定
            match bridge_rx_clone.recv_timeout(Duration::from_millis(500)) {
                Ok(Event::IBusKey { event: _, respond }) => {
                    respond
                        .send(KeyEventResult::Consumed)
                        .expect("send Consumed back");
                }
                other => panic!("engine-loop stub expected IBusKey, got {other:?}"),
            }
        })
        .expect("spawn engine stub");

    // method を blocking で呼び出す(返り値 b)
    let result: bool = proxy
        .call("ProcessKeyEvent", &(0x6b_u32, 45_u32, 0_u32))
        .expect("call ProcessKeyEvent");

    engine_thread.join().expect("engine stub join");
    assert!(result, "expected true (Consumed) from listener");

    // shutdown
    shutdown_trigger.request();
    drop(conn);
    let listener_result = listener_handle.join().expect("listener join");
    assert!(
        listener_result.is_ok(),
        "listener exited with error: {listener_result:?}"
    );
}
