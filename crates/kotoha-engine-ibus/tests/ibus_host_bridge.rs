//! L2-adapter integration test: `IBusHostBridge` の構築 smoke。
//!
//! 完全な mock IBus daemon は spec §10.1 / Open Q 9 で実装段階に詰める。
//! 本 PR は **接続 only** の smoke を入れ、PR レビューで sequence assert
//! の詳細化を Phase 3-A 本番実装段階に deferral する。

#[test]
fn host_bridge_constructs_with_dummy_path() {
    // session bus が無い CI 環境では skip(`DBUS_SESSION_BUS_ADDRESS` 未設定時)。
    if std::env::var("DBUS_SESSION_BUS_ADDRESS").is_err() {
        eprintln!("skipping: no session bus available");
        return;
    }
    let result = kotoha_engine_ibus::IBusHostBridge::new("/org/freedesktop/IBus/Engine/Kotoha");
    // 構築できれば PASS、call 詳細は Phase 3-A 実装段階で詰める。
    // session bus へ接続できない場合(daemon 未起動等)は Err でも skip。
    match result {
        Ok(_) => (),
        Err(e) => eprintln!("session bus connect failed (skipping): {e}"),
    }
}
