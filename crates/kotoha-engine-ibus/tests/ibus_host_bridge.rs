//! L2-adapter integration test: `IBusHostBridge` の構築 smoke。
//!
//! #208 改訂で `IBusHostBridge::new` は connection 注入形になった(独自接続を
//! 廃止、spec §7.4)。本 test は注入用 connection の確立に dbus session bus を
//! 流用する(`IBusHostBridge` 自体は bus 種別に依存しない — production では
//! private bus connection が注入される)。
//!
//! # B0e (ISSUE #140 / Important review feedback)
//!
//! 旧版は `Err(e)` で全 path を skip しており assertion ゼロだった。本 fix で:
//!
//! - session bus が無い場合は `#[ignore]` で skip(green check で誤誘導しない)
//! - session bus が在る場合は `result.is_ok()` を assert

#[test]
#[ignore = "requires DBUS_SESSION_BUS_ADDRESS; run with `cargo test -- --ignored`"]
fn host_bridge_constructs_with_injected_connection() {
    let _addr = std::env::var("DBUS_SESSION_BUS_ADDRESS")
        .expect("DBUS_SESSION_BUS_ADDRESS required for this test");
    let connection = zbus::blocking::Connection::session().expect("open session bus for injection");
    let result =
        kotoha_engine_ibus::IBusHostBridge::new(connection, "/org/freedesktop/IBus/Engine/Kotoha");
    assert!(
        result.is_ok(),
        "IBusHostBridge::new should succeed with a healthy injected connection: {:?}",
        result.err()
    );
}
