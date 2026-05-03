//! L2-adapter integration test: `IBusHostBridge` の構築 smoke。
//!
//! 完全な mock IBus daemon は spec §10.1 / Open Q 9 で実装段階に詰める。
//! 本 PR は **接続 only** の smoke を入れ、PR レビューで sequence assert
//! の詳細化を Phase 3-A 本番実装段階に deferral する。
//!
//! # B0e (ISSUE #140 / Important review feedback)
//!
//! 旧版は `Err(e)` で全 path を skip しており assertion ゼロだった。本 fix で:
//!
//! - session bus が無い場合は `#[ignore]` で skip(green check で誤誘導しない)
//! - session bus が在る場合は `result.is_ok()` を assert
//! - session bus 在 + 接続失敗(IBus daemon 未起動等)時は明示 panic で fail surface

#[test]
#[ignore = "requires DBUS_SESSION_BUS_ADDRESS; run with `cargo test -- --ignored`"]
fn host_bridge_constructs_with_session_bus() {
    let _addr = std::env::var("DBUS_SESSION_BUS_ADDRESS")
        .expect("DBUS_SESSION_BUS_ADDRESS required for this test");
    let result = kotoha_engine_ibus::IBusHostBridge::new("/org/freedesktop/IBus/Engine/Kotoha");
    assert!(
        result.is_ok(),
        "IBusHostBridge::new should succeed on a healthy session bus: {:?}",
        result.err()
    );
}
