//! Phase 3-B B6-b (#195) + #208 — listener thread + engine_loop 経路の
//! L2 integration test。
//!
//! Spec: `docs/specs/_uncategorized/p3-b-ibus-listener.md` §10.1 L2 rows。
//!
//! 使い捨て `dbus-daemon --session` を IBus private bus に見立て、
//! `KOTOHA_IBUS_ADDRESS` 注入で `build_connection` に解決させる(spec §7.1
//! 優先度 1 の override 経路)。test 自身が fake ibus-daemon として
//! `CreateEngine` → `ProcessKeyEvent` の handshake を再現する。
//!
//! 本 test は `dbus-daemon` binary に依存するため `#[ignore]` で gate し、
//! `cargo test -p kotoha-engine-ibus -- --ignored` で opt-in 実行する。
//! 通常 CI / pre-push hook では実行されない(L3 manual smoke #196 で実機検証する)。
//! env var(`KOTOHA_IBUS_ADDRESS`)を扱うため、handshake 検証は単一 test 関数に
//! 直列化している(test 間 env 競合回避)。

use std::sync::Arc;
use std::time::Duration;

use crossbeam_channel::unbounded;
use kotoha_engine_core::key_event::KeyEventResult;
use kotoha_engine_core::reactor::Event;
use kotoha_engine_ibus::listener::{build_connection, run, ListenerShutdown};

/// 使い捨て dbus-daemon の lifecycle guard(Drop で kill)。
struct ThrowawayBus {
    address: String,
    pid: u32,
}

impl ThrowawayBus {
    fn spawn() -> Self {
        let output = std::process::Command::new("dbus-daemon")
            // `--print-address` / `--print-pid` は空白区切りの次引数を fd として
            // 解釈するため `=1`(stdout)を明示する
            .args(["--session", "--fork", "--print-address=1", "--print-pid=1"])
            .output()
            .expect("spawn dbus-daemon (binary required for this test)");
        assert!(
            output.status.success(),
            "dbus-daemon failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8(output.stdout).expect("dbus-daemon stdout utf8");
        let mut lines = stdout.lines();
        let address = lines.next().expect("address line").trim().to_owned();
        let pid: u32 = lines
            .next()
            .expect("pid line")
            .trim()
            .parse()
            .expect("pid number");
        Self { address, pid }
    }
}

impl Drop for ThrowawayBus {
    fn drop(&mut self) {
        let _ = std::process::Command::new("kill")
            .arg(self.pid.to_string())
            .status();
    }
}

/// fake daemon として handshake 全経路を 1 関数で検証する:
///
/// 1. `build_connection` が `KOTOHA_IBUS_ADDRESS` 経由で使い捨て bus に接続し
///    Factory + Engine を serve する
/// 2. `CreateEngine("kotoha")` が静的 engine path を返す(spec §4.5 / §7.3)
/// 3. 返却 path への `ProcessKeyEvent` が bridge channel 経由で
///    `Event::IBusKey` に decode され、`Consumed` 応答が `true` で返る
/// 4. `CreateEngine("anthy")` が D-Bus error で reject される(spec §9.1)
#[test]
#[ignore = "依存: dbus-daemon binary; CI で flaky のため opt-in"]
fn handshake_create_engine_and_process_key_event() {
    let bus = ThrowawayBus::spawn();
    std::env::set_var("KOTOHA_IBUS_ADDRESS", &bus.address);

    let (bridge_tx, bridge_rx) = unbounded::<Event>();
    let (shutdown_trigger, shutdown_observer) = ListenerShutdown::new();

    // 1. component 側: connection 構築(Factory + Engine serve + RequestName)
    let connection = build_connection(bridge_tx).expect("build_connection via KOTOHA_IBUS_ADDRESS");
    let listener_handle = std::thread::Builder::new()
        .name("test-dbus-listener".into())
        .spawn(move || run(connection, shutdown_observer))
        .expect("spawn listener");

    // 2. fake daemon 側: 同じ bus に接続し CreateEngine を call
    let daemon_conn = zbus::blocking::connection::Builder::address(bus.address.as_str())
        .expect("daemon-side address")
        .build()
        .expect("daemon-side connection");
    let factory_proxy = zbus::blocking::Proxy::new(
        &daemon_conn,
        "org.freedesktop.IBus.Engine.Kotoha",
        "/org/freedesktop/IBus/Factory",
        "org.freedesktop.IBus.Factory",
    )
    .expect("build factory proxy");

    let engine_path: zbus::zvariant::OwnedObjectPath = factory_proxy
        .call("CreateEngine", &("kotoha",))
        .expect("CreateEngine(kotoha)");
    assert_eq!(
        engine_path.as_str(),
        "/org/freedesktop/IBus/Engine/Kotoha",
        "CreateEngine must return the static engine path (spec §7.3)"
    );

    // 3. 返却された path に ProcessKeyEvent を call(engine 役 stub が応答)
    let bridge_rx = Arc::new(bridge_rx);
    let bridge_rx_clone = bridge_rx.clone();
    let engine_thread = std::thread::Builder::new()
        .name("test-engine-loop-stub".into())
        .spawn(
            move || match bridge_rx_clone.recv_timeout(Duration::from_millis(500)) {
                Ok(Event::IBusKey { event: _, respond }) => {
                    respond
                        .send(KeyEventResult::Consumed)
                        .expect("send Consumed back");
                }
                other => panic!("engine-loop stub expected IBusKey, got {other:?}"),
            },
        )
        .expect("spawn engine stub");

    let engine_proxy = zbus::blocking::Proxy::new(
        &daemon_conn,
        "org.freedesktop.IBus.Engine.Kotoha",
        engine_path.as_str(),
        "org.freedesktop.IBus.Engine",
    )
    .expect("build engine proxy");
    let consumed: bool = engine_proxy
        .call("ProcessKeyEvent", &(0x6b_u32, 45_u32, 0_u32))
        .expect("call ProcessKeyEvent");
    engine_thread.join().expect("engine stub join");
    assert!(consumed, "expected true (Consumed) from listener");

    // 4. 未知 engine 名は reject される
    let unknown: Result<zbus::zvariant::OwnedObjectPath, zbus::Error> =
        factory_proxy.call("CreateEngine", &("anthy",));
    assert!(
        unknown.is_err(),
        "CreateEngine with unknown engine name must fail (spec §9.1)"
    );

    // shutdown
    shutdown_trigger.request();
    drop(daemon_conn);
    let listener_result = listener_handle.join().expect("listener join");
    assert!(
        listener_result.is_ok(),
        "listener exited with error: {listener_result:?}"
    );
    std::env::remove_var("KOTOHA_IBUS_ADDRESS");
}
