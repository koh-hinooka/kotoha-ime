# ISSUE #208 — IBus private bus + Factory interface Implementation Plan

**Goal:** `kotoha-engine-ibus` の D-Bus 接続先を session bus から IBus private bus(address discovery 経由)に変更し、`org.freedesktop.IBus.Factory` interface を serve して ibus-daemon との handshake(`SetGlobalEngine`)を成立させる。signal 発信経路(`IBusEngineSignals`)も同一 connection に統一する。

**Spec:** `docs/specs/_uncategorized/p3-b-ibus-listener.md` §4.5 / §7(#208 改訂版)/ §10.1 / §11.1
**ADR:** `docs/adr/0021-zbus-integration-architecture-for-ibus-listener.md` Amendment 2026-06-11
**ISSUE:** [#208](https://github.com/std-koh-hinooka/kotoha-ime/issues/208)(parent: #136 B6-c)
**Branch:** `bugfix/208-ibus-private-bus-factory`(Spec 改訂 commit `a106a5d` 済)

---

## マイルストーン位置付け

| 観点 | 内容 |
|---|---|
| Phase | 3-B(IBus integration)、B6-c blocker 解消 |
| 前提 | PR #207(dict size cap)merge 済、Spec/ADR 改訂 commit 済 |
| 後続 | B6-c L3 manual smoke 再開(`docs/runbooks/2026-06-11-phase3b-b6-l3-smoke-result.md` 引継) |
| 検証 | L1 unit(無条件)+ L2 handshake(`#[ignore]`)+ lefthook pre-push |

## ファイル構成

### 新規作成(2 ファイル)

```
crates/kotoha-engine-ibus/src/
├── discovery.rs        # private bus address discovery(純関数 + env/file 入力)
└── factory.rs          # KotohaFactoryService(org.freedesktop.IBus.Factory)
```

### 変更(6 ファイル)

```
crates/kotoha-engine-ibus/src/
├── lib.rs              # mod discovery; mod factory; 追加
├── proxy.rs            # IBUS_FACTORY_OBJECT_PATH 追加、IBusEngineSignals::new signature 変更
├── service.rs          # no-op stub 3 method 追加
├── listener.rs         # build_connection() 新設、run() を connection 受領形に変更
└── host_bridge.rs      # IBusHostBridge::new(connection, object_path) に変更
crates/kotoha-bin/src/
└── main.rs             # DI wiring 順序入替(reactor → connection → host_bridge → engine → listener)
crates/kotoha-engine-ibus/tests/
└── integration.rs      # L2 handshake test(#[ignore])
```

---

## Step 1: discovery.rs 新規(Spec §7.1)

- [ ] `crates/kotoha-engine-ibus/src/discovery.rs` を作成する

```rust
pub(crate) const KOTOHA_IBUS_ADDRESS_ENV: &str = "KOTOHA_IBUS_ADDRESS";
pub(crate) const IBUS_ADDRESS_ENV: &str = "IBUS_ADDRESS";

#[derive(Debug, thiserror::Error)]
pub(crate) enum DiscoveryError {
    #[error("machine-id not readable: {0}")]
    MachineId(std::io::Error),
    #[error("DISPLAY not set; cannot derive IBus address file name")]
    DisplayUnset,
    #[error("IBus address file not readable: {path}: {source}")]
    AddressFile { path: std::path::PathBuf, source: std::io::Error },
    #[error("IBUS_ADDRESS= line not found in address file: {path}")]
    AddressLineMissing { path: std::path::PathBuf },
}
```

関数 4 つ(`discover_ibus_address` 以外は純関数または入力分離形):

1. `pub(crate) fn discover_ibus_address() -> Result<String, DiscoveryError>` — 3 段 fallback: `KOTOHA_IBUS_ADDRESS` env → `IBUS_ADDRESS` env → `address_file_path()?` を read して `parse_address_file`
2. `fn address_file_path() -> Result<PathBuf, DiscoveryError>` — `$XDG_CONFIG_HOME`(未設定時 `$HOME/.config`)`/ibus/bus/<machine-id>-unix-<display>`。machine-id は `/var/lib/dbus/machine-id` → `/etc/machine-id` fallback、trim。display は `display_number(&env DISPLAY)?`
3. `pub(crate) fn parse_address_file(content: &str) -> Option<&str>` — 行頭 `IBUS_ADDRESS=` の最初の行の値部分を返す(コメント行 `#` 無視)
4. `fn display_number(display: &str) -> &str` — `":0"` → `"0"`、`":0.0"` → `"0"`(先頭 `:` 除去 + 最初の `.` 以降切捨て)

- [ ] L1 unit tests(同ファイル `#[cfg(test)]`、純関数のみ・env 非依存):
  - `parse_address_file`: 正常(`IBUS_ADDRESS=unix:abstract=...` 行 + コメント行 + `IBUS_DAEMON_PID=` 行の混在)/ `IBUS_ADDRESS=` 行欠落で `None` / 空文字列で `None`
  - `display_number`: `":0"` / `":0.0"` / `":12"` の 3 case

## Step 2: proxy.rs — 定数追加 + IBusEngineSignals signature 変更(Spec §7.2 / §7.4)

- [ ] `IBUS_FACTORY_OBJECT_PATH` 定数を追加する

```rust
/// IBus 1.5.x factory の固定 object path(spec §7.2、#208)。
pub(crate) const IBUS_FACTORY_OBJECT_PATH: &str = "/org/freedesktop/IBus/Factory";
```

- [ ] `IBusEngineSignals::new` を connection 受領形に変更する(独自 `Connection::session()` を削除)

```rust
pub(crate) fn new(connection: Connection, object_path: &str) -> Result<Self> {
    let path = build_object_path(object_path)?;
    Ok(Self { connection, object_path: path })
}
```

- [ ] module doc の「session bus に発信」記述を「private bus(listener と共有する connection)に発信」へ更新する(threat model 段落の「同 UID プロセス信頼」前提は private bus でも同一なので維持)
- [ ] 既存の `build_object_path` unit tests は無修正で pass することを確認する

## Step 3: factory.rs 新規(Spec §4.5)

- [ ] `crates/kotoha-engine-ibus/src/factory.rs` を作成する

```rust
use zbus::interface;
use crate::proxy::IBUS_ENGINE_OBJECT_PATH;

pub(crate) struct KotohaFactoryService;

#[interface(name = "org.freedesktop.IBus.Factory")]
impl KotohaFactoryService {
    /// `CreateEngine(s engine_name) -> o`(IBus 1.5.x `ibusfactory.c` 仕様)。
    fn create_engine(
        &self,
        engine_name: &str,
    ) -> zbus::fdo::Result<zbus::zvariant::OwnedObjectPath> {
        if engine_name != "kotoha" {
            tracing::warn!(
                error_id = "factory.create_engine.unknown_engine",
                engine_name,
                "CreateEngine called with unknown engine name"
            );
            return Err(zbus::fdo::Error::InvalidArgs(format!(
                "unknown engine name: {engine_name}"
            )));
        }
        // 静的単一 path(spec §7.3): engine object は connection build 時に
        // serve_at 済みのため、path の返却のみを行う。
        Ok(zbus::zvariant::OwnedObjectPath::try_from(IBUS_ENGINE_OBJECT_PATH)
            .expect("IBUS_ENGINE_OBJECT_PATH is a valid D-Bus object path"))
    }
}
```

- [ ] L1 unit tests: `create_engine("kotoha")` が `IBUS_ENGINE_OBJECT_PATH` を返す / `create_engine("anthy")` が `InvalidArgs` を返す(`#[interface]` の inherent method を直接呼ぶ)
- [ ] `lib.rs` に `mod discovery; mod factory;` を追加する

## Step 4: service.rs — daemon 互換 no-op stub 追加(Spec §2.1)

- [ ] `#[interface]` impl に 3 method を追加する(いずれも `tracing::debug!` のみ)

```rust
/// daemon が engine 生成直後に呼ぶ capability 通知。Phase 3-B では未使用(no-op)。
fn set_capabilities(&self, caps: u32) { tracing::debug!(caps, "SetCapabilities (no-op)"); }
/// 候補 window 配置用 cursor 座標通知。Phase 3-B では未使用(no-op)。
fn set_cursor_location(&self, x: i32, y: i32, w: i32, h: i32) {
    tracing::debug!(x, y, w, h, "SetCursorLocation (no-op)");
}
/// `org.freedesktop.IBus.Service.Destroy` 互換。静的単一 path(spec §7.3)のため
/// object 解放は行わない(no-op)。
fn destroy(&self) { tracing::debug!("Destroy (no-op)"); }
```

- [ ] 既存 6 method の unit tests が無修正で pass することを確認する

## Step 5: listener.rs — build_connection 分離 + run の connection 受領化(Spec §3.3 / §7.2)

- [ ] `pub fn build_connection(bridge_tx: Sender<Event>) -> anyhow::Result<zbus::blocking::Connection>` を新設する

```rust
pub fn build_connection(
    bridge_tx: Sender<Event>,
) -> anyhow::Result<zbus::blocking::Connection> {
    use crate::discovery::discover_ibus_address;
    use crate::factory::KotohaFactoryService;
    use crate::proxy::{IBUS_ENGINE_BUS_NAME, IBUS_ENGINE_OBJECT_PATH, IBUS_FACTORY_OBJECT_PATH};
    use crate::service::KotohaEngineService;

    let address = discover_ibus_address()?;
    let connection = zbus::blocking::connection::Builder::address(address.as_str())?
        .serve_at(IBUS_FACTORY_OBJECT_PATH, KotohaFactoryService)?
        .serve_at(IBUS_ENGINE_OBJECT_PATH, KotohaEngineService { bridge_tx })?
        .name(IBUS_ENGINE_BUS_NAME)?
        .build()?;
    Ok(connection)
}
```

- [ ] `run` の signature を `pub fn run(connection: zbus::blocking::Connection, shutdown: ShutdownObserver) -> anyhow::Result<()>` に変更する — connection 構築コードを除去し、shutdown poll loop + drop のみ残す
- [ ] module doc / `run` の doc comment(Preconditions / Postconditions / Errors)を private bus + main 構築前提に更新する
- [ ] `ShutdownObserver` 系 unit tests 3 件は無修正で pass することを確認する

## Step 6: host_bridge.rs — connection 注入(Spec §7.4)

- [ ] `IBusHostBridge::new` を変更する

```rust
pub fn new(connection: zbus::blocking::Connection, object_path: &str) -> zbus::Result<Self> {
    Ok(Self {
        lookup_table: LookupTable::new(),
        signals: IBusEngineSignals::new(connection, object_path)?,
    })
}
```

- [ ] struct / `new` の doc comment(「session bus 接続」)を更新する

## Step 7: main.rs — DI wiring 順序入替(Spec §3.1 / §3.3)

現行順序: 8. host_bridge → 9. reactor → 10. engine → 11-12. listener spawn。
新順序: **8. reactor → 9. connection 構築 → 10. host_bridge → 11. engine → 12-13. listener spawn**(connection 構築が `bridge_tx` を要するため)。

- [ ] step 9(現 8)の host_bridge 構築を reactor の後ろに移動し、以下の形にする

```rust
// 9. private bus connection(#208 / ADR 0021 Amendment): main が構築し、
//    clone を host bridge へ、本体を listener thread へ配布する。
let connection = match kotoha_engine_ibus::listener::build_connection(bridge_tx.clone()) {
    Ok(c) => Some(c),
    #[cfg(feature = "dev-stubs")]
    Err(e) if allow_stub => {
        tracing::error!(error = ?e,
            "IBus private bus connect failed; using StubHostBridge (KOTOHA_ALLOW_STUB=1)");
        None
    }
    Err(e) => {
        return Err(e).context(
            "IBus private bus connect failed (is ibus-daemon running?) \
             and KOTOHA_ALLOW_STUB is not set",
        );
    }
};

// 10. host bridge: connection clone を注入。
let (host_bridge, host_bridge_backend): (Box<dyn kotoha_engine_core::IMEHostBridge>, &'static str) =
    match &connection {
        Some(c) => (
            Box::new(kotoha_engine_ibus::IBusHostBridge::new(c.clone(), ENGINE_OBJECT_PATH)?),
            "IBusHostBridge",
        ),
        #[cfg(feature = "dev-stubs")]
        None => (Box::new(StubHostBridge), "StubHostBridge"),
        #[cfg(not(feature = "dev-stubs"))]
        None => unreachable!("connection is always Some when dev-stubs is disabled"),
    };
```

- [ ] listener spawn を `connection` が `Some` の場合のみ実行し、`listener_handle: Option<JoinHandle<_>>` に変更する(stub 経路では listener 不在。join 側も `if let Some(h)` 化)
- [ ] module doc(line 15 付近「session bus 接続」)と step 番号 comment を更新する
- [ ] `cargo build --workspace` + `cargo clippy --workspace --all-targets -- -D warnings` を通す

## Step 8: L2 handshake test(Spec §10.1)

- [ ] `crates/kotoha-engine-ibus/tests/integration.rs` に `#[ignore]` test を追加する:
  1. `dbus-daemon --session --print-address --fork` で使い捨て bus を起動(test 終了時 kill)
  2. `KOTOHA_IBUS_ADDRESS` にその address を設定し `build_connection(bridge_tx)` 実行
  3. fake daemon 側 connection(同 address)から destination `org.freedesktop.IBus.Engine.Kotoha` の `/org/freedesktop/IBus/Factory` に `CreateEngine("kotoha")` を call → 返却 path が `/org/freedesktop/IBus/Engine/Kotoha` であること
  4. 同 path に `ProcessKeyEvent(0x61, 38, 0)` を call → bridge channel の `Receiver` 側で `Event::IBusKey` を受信し `respond.send(Consumed)` → call 戻り値 `true` であること
  5. `CreateEngine("anthy")` が D-Bus error を返すこと
- [ ] env var を扱う test なので `#[ignore]` + 単一 test 関数に直列化(test 間 env 競合回避)

## Step 9: 検証 + PR

- [ ] `cargo fmt --all` / `cargo clippy --workspace --all-targets -- -D warnings` / `cargo test --workspace`(+ `cargo test -p kotoha-engine-ibus -- --ignored` を dbus-daemon 在環境で手動 1 回)
- [ ] lefthook pre-push 通過
- [ ] PR 作成(base: develop、English)。size 見積: ~500-700 lines / 9 files → **Medium-Large tier**。IBus との接続境界 = input validation 隣接につき security-touching 扱い: `team-review`(全 5 dims)+ `owasp-security` + `secrets-check`
- [ ] merge 後: `/post-merge` → B6-c smoke 再開(handoff `2026-06-11-issue-208-ibus-bus-factory` §3 の環境固有手順)

## Out of scope

- per-CreateEngine 動的 path(Phase 6 再評価、Spec §13.4)
- component XML packaging(別 ISSUE、Spec §7.5)
- runbook errata 3 件(smoke result PR に同梱、handoff §4)
