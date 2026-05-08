# P3-B B6-b IBus Engine Listener Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.
>
> **Project flow note:** This project uses **Spec-Driven, Test-After** (see `~/.claude/CLAUDE.md` §Development Flow). TDD's "write failing test first" pattern is banned per global rule. Each task implements code first, then tests are derived from the spec acceptance criteria, then verified.

**Goal:** Replace the `kotoha-engine-ibus` listener stub with a production `zbus #[interface]` dispatcher that decodes 4 IBus engine methods (+2 daemon-introspection no-op stubs) and forwards them as `Event` to the engine-loop, with a response channel for `ProcessKeyEvent`'s bool return.

**Architecture:** zbus 5 `#[interface]` macro on `KotohaEngineService` struct, registered via `blocking::connection::Builder::serve_at + name`. `Event::IBusKey` extends to carry a response `Sender<KeyEventResult>` with 100ms timeout fallback. `KOTOHA_ALLOW_LISTENER_STUB` safety net is removed entirely.

**Tech Stack:** Rust 1.80, edition 2021, zbus 5 (blocking API), crossbeam-channel 0.5, kotoha-engine-core (`Event` / `KeyEvent` / `KeyEventResult`), kotoha-engine-ibus (existing `types` / `proxy` / `keysym` / `host_bridge`).

**Spec:** `docs/specs/_uncategorized/p3-b-ibus-listener.md` (commits `79da087` + `48c7d6d` on this branch).

**Branch:** `feature/195-zbus-listener-decode` (created from develop, includes the 2 spec commits).

---

## File map

| Action | Path | Purpose |
|---|---|---|
| Modify | `crates/kotoha-engine-ibus/src/proxy.rs` | Add `IBUS_ENGINE_BUS_NAME` + `IBUS_ENGINE_OBJECT_PATH` constants |
| Modify | `crates/kotoha-engine-core/src/reactor/event.rs` | Change `Event::IBusKey(KeyEvent)` → `Event::IBusKey { event, respond }` |
| Modify | `crates/kotoha-bin/src/engine_loop.rs` | Destructure new variant + send response |
| Modify | `crates/kotoha-engine-core/Cargo.toml` (if needed) | (already has crossbeam-channel; expose `KeyEventResult`) |
| Create | `crates/kotoha-engine-ibus/src/service.rs` | New `KotohaEngineService` struct + `#[interface]` impl + L1 unit tests |
| Modify | `crates/kotoha-engine-ibus/src/lib.rs` | Register new `service` module |
| Modify | `crates/kotoha-engine-ibus/src/listener.rs` | Delete stub safety net, rewrite `run()` to build connection |
| Create | `crates/kotoha-engine-ibus/tests/integration.rs` | L2 integration test gated with `#[ignore]` (dbus session bus required) |
| Create | `docs/adr/0021-zbus-integration-architecture-for-ibus-listener.md` | ADR for zbus 5 `#[interface]` + blocking Builder + bounded response channel + 100ms timeout |
| Modify | `docs/specs/_uncategorized/p3-a-ibus-engine.md` | Update §6.1 step [1] / §11 acceptance criteria + frontmatter `last_reviewed` |
| Modify | `docs/ROADMAP.md` | Mark #195 `[x]` in v0.3.0 active table; update Phase 3-B sub-milestone B6 entry; revision history row |

---

## Task 1: Add IBus engine bus name + object path constants

**Files:**
- Modify: `crates/kotoha-engine-ibus/src/proxy.rs`

- [ ] **Step 1: Add the two constants below the existing `IBUS_ENGINE_INTERFACE`**

After line 53 (`pub(crate) const IBUS_ENGINE_INTERFACE: &str = "org.freedesktop.IBus.Engine";`), insert:

```rust
/// Kotoha が `RequestName` で取得する IBus engine bus name(spec §7.1)。
///
/// 同 UID で同名 process が既に publish していると `Builder::name(...)` が `Err` を返し、
/// listener thread の起動が失敗する。
pub(crate) const IBUS_ENGINE_BUS_NAME: &str = "org.freedesktop.IBus.Engine.Kotoha";

/// Kotoha engine service の D-Bus object path(spec §7.1)。
///
/// `Builder::serve_at(...)` でこの path に `KotohaEngineService` を登録する。
pub(crate) const IBUS_ENGINE_OBJECT_PATH: &str = "/org/freedesktop/IBus/Engine/Kotoha";
```

- [ ] **Step 2: Verify build**

Run:
```bash
cargo build -p kotoha-engine-ibus
```
Expected: `Finished` with no warnings (constants are `pub(crate)` and not yet referenced — fine for now, will be used in Task 4).

If `unused constant` warning fires, suppress with `#[allow(dead_code)]` on each constant only if necessary. Defer the suppression to verify it's actually warned (rust 1.80 typically allows pub(crate) without warning).

- [ ] **Step 3: Commit**

```bash
git add crates/kotoha-engine-ibus/src/proxy.rs
git commit -m "feat(engine-ibus): add IBus engine bus name + object path constants (#195)"
```

---

## Task 2: Extend `Event::IBusKey` with response channel

**Files:**
- Modify: `crates/kotoha-engine-core/src/reactor/event.rs`
- Modify: `crates/kotoha-bin/src/engine_loop.rs`
- Modify: any caller / test that constructs `Event::IBusKey(...)`

This is a **breaking change** within the workspace: the variant signature changes from tuple to struct. All sites must be updated atomically. Build verification at the end ensures no consumer is missed.

- [ ] **Step 1: Change `Event::IBusKey` to struct variant**

Edit `crates/kotoha-engine-core/src/reactor/event.rs`:

Add import at the top of the file (after the existing imports):

```rust
use crossbeam_channel::Sender;

use crate::key_event::{KeyEvent, KeyEventResult};
use crate::ranker::CandidateUpdate;
use crate::request_id::RequestId;
```

Note: `KeyEvent` and `CandidateUpdate` may already be imported; preserve existing imports and ADD `Sender` and `KeyEventResult`. Do NOT duplicate.

Replace the variant:

```rust
    /// IBus session bus で受信した key event。
    /// `kotoha-dbus-listener` thread が `Sender<Event>::send` で engine-loop に届ける。
    ///
    /// `respond` は engine-loop が `KeyEventResult`(`Consumed` / `Forwarded`)を返す
    /// bounded(1) channel。listener は recv_timeout(100ms) で待機し timeout 時は
    /// `Forwarded` 相当として bool false を返す(spec §6 / `docs/specs/_uncategorized/p3-b-ibus-listener.md`)。
    IBusKey {
        event: KeyEvent,
        respond: Sender<KeyEventResult>,
    },
```

- [ ] **Step 2: Update engine_loop.rs match arm**

Edit `crates/kotoha-bin/src/engine_loop.rs:43-46`. Replace:

```rust
            Ok(Event::IBusKey(key)) => {
                let _result = engine.process_key_event(key);
            }
```

with:

```rust
            Ok(Event::IBusKey { event, respond }) => {
                let result = engine.process_key_event(event);
                // listener が timeout した場合 send Err は ignore (= keystroke は app に forward 済)
                let _ = respond.send(result);
            }
```

- [ ] **Step 3: Find and update other callers**

Run a grep to surface any remaining tuple-variant constructions:

```bash
rg -n "Event::IBusKey\(" crates/ docs/
```

Expected: only references inside specs / plans (markdown). If any source file references appear, update them to the struct form. (The reactor-linux integration tests should NOT touch `IBusKey` — they use `IBusReset` and `WorkerOutput`.)

- [ ] **Step 4: Build verify**

Run:
```bash
cargo build --workspace --all-features
```
Expected: `Finished` with no errors. Any compile error from missed callers must be fixed in this step.

- [ ] **Step 5: Test verify**

Run:
```bash
cargo test --workspace --features kotoha-storage/test-helpers,kotoha-engine-core/test-helpers 2>&1 | /usr/bin/grep -E "FAILED|^test result:" | tail -10
```
Expected: 0 FAILED. Any test that previously constructed `Event::IBusKey(...)` directly must be updated.

- [ ] **Step 6: Commit**

```bash
git add crates/kotoha-engine-core/src/reactor/event.rs crates/kotoha-bin/src/engine_loop.rs
# add other changed test files if any
git commit -m "refactor(engine-core,bin): extend Event::IBusKey with respond channel (#195)"
```

---

## Task 3: Create `KotohaEngineService` with `#[interface]` impl

**Files:**
- Create: `crates/kotoha-engine-ibus/src/service.rs`
- Modify: `crates/kotoha-engine-ibus/src/lib.rs`

- [ ] **Step 1: Verify zbus 5 `#[interface]` blocking API surface**

Open Context7 if available, OR run:
```bash
cargo doc -p zbus --open 2>&1 | head -3
```
Expected API (verify in actual source — adjust if docs differ):
- `zbus::interface` macro (re-exported from zbus root)
- `zbus::blocking::connection::Builder` chain: `session()?`, `serve_at(path, instance)?`, `name(name)?`, `build()?`

If the macro path differs in the installed zbus 5 version (e.g., `zbus::dbus_interface` in 4.x vs `zbus::interface` in 5.x), use the symbol from the installed crate.

- [ ] **Step 2: Write `service.rs`**

Create `crates/kotoha-engine-ibus/src/service.rs`:

```rust
//! `KotohaEngineService` — IBus engine method dispatcher。
//!
//! Phase 3-B B6-b (#195) で導入。zbus 5 `#[interface]` macro 経由で
//! `org.freedesktop.IBus.Engine` interface の 6 method を実装する。
//!
//! 詳細仕様: `docs/specs/_uncategorized/p3-b-ibus-listener.md` §4 / §5。

use std::time::Duration;

use crossbeam_channel::Sender;
use zbus::interface;

use kotoha_engine_core::key_event::KeyEventResult;
use kotoha_engine_core::reactor::{Event, IBusResetKind};

use crate::keysym;

/// IBus engine method dispatcher。
///
/// `bridge_tx` 経由で engine-loop thread に Event を届ける。state を保持しない
/// (ADR 0020 §採択 Q4 lock-free 原則)。zbus が internal smol executor で本 struct
/// の method を dispatch する。
///
/// # 配置
///
/// `pub(crate)` で crate 外部に export しない。`listener::run` 内で構築・登録される。
pub(crate) struct KotohaEngineService {
    /// engine-loop thread に Event を送る単方向 channel。
    pub(crate) bridge_tx: Sender<Event>,
}

/// `process_key_event` の response 待機 timeout。
///
/// 通常 ~1ms / heavy ~10ms / recovery ~50ms 全てを吸収しつつ、bug 状態 (>100ms) のみ
/// fallback fire するように設定(spec §6.1)。
const PROCESS_KEY_EVENT_TIMEOUT: Duration = Duration::from_millis(100);

#[interface(name = "org.freedesktop.IBus.Engine")]
impl KotohaEngineService {
    /// `ProcessKeyEvent(u keyval, u keycode, u state) -> b`
    ///
    /// IBus 1.5.x 仕様準拠。戻り値 true は Kotoha が消費した、false は app に forward する。
    fn process_key_event(&self, keyval: u32, keycode: u32, state: u32) -> bool {
        let event = keysym::from_ibus(keyval, keycode, state);
        let (resp_tx, resp_rx) = crossbeam_channel::bounded::<KeyEventResult>(1);
        if self
            .bridge_tx
            .send(Event::IBusKey {
                event,
                respond: resp_tx,
            })
            .is_err()
        {
            tracing::warn!(
                error_id = "listener.process_key_event.bridge_disconnected",
                "engine-loop disconnected; forwarding key to app"
            );
            return false;
        }
        match resp_rx.recv_timeout(PROCESS_KEY_EVENT_TIMEOUT) {
            Ok(KeyEventResult::Consumed) => true,
            Ok(KeyEventResult::Forwarded) => false,
            Err(_) => {
                tracing::warn!(
                    error_id = "listener.process_key_event.timeout",
                    timeout_ms = PROCESS_KEY_EVENT_TIMEOUT.as_millis() as u64,
                    "engine response timeout; forwarding key to app"
                );
                false
            }
        }
    }

    /// `FocusOut()` — engine 側 reset 経路を発火する。応答不要。
    fn focus_out(&self) {
        let _ = self
            .bridge_tx
            .send(Event::IBusReset(IBusResetKind::FocusOut));
    }

    /// `Reset()` — engine 側 reset 経路。応答不要。
    fn reset(&self) {
        let _ = self.bridge_tx.send(Event::IBusReset(IBusResetKind::Reset));
    }

    /// `Disable()` — engine 側 disable 経路。応答不要。
    fn disable(&self) {
        let _ = self
            .bridge_tx
            .send(Event::IBusReset(IBusResetKind::Disable));
    }

    /// `FocusIn()` — daemon 側 introspection 互換のための no-op stub(spec §4.3)。
    fn focus_in(&self) {
        tracing::debug!("KotohaEngineService::focus_in (no-op)");
    }

    /// `Enable()` — daemon 側 introspection 互換のための no-op stub(spec §4.3)。
    fn enable(&self) {
        tracing::debug!("KotohaEngineService::enable (no-op)");
    }
}

#[cfg(test)]
mod tests {
    //! Spec: `docs/specs/_uncategorized/p3-b-ibus-listener.md` §10.1
    //! L1 unit tests。dbus を使わず crossbeam channel 直接観測で 4 経路を pin する。
    //!
    //! - process_key_event normal: respond.send(Consumed) → true
    //! - process_key_event normal: respond.send(Forwarded) → false
    //! - process_key_event timeout (engine が応答しない) → false
    //! - process_key_event disconnect (engine_loop drop) → false
    //! - reset 系 3 method の Event 送信
    //! - no-op stub 2 method の non-panic

    use super::*;
    use crossbeam_channel::{unbounded, TryRecvError};
    use std::thread;

    fn setup() -> (
        KotohaEngineService,
        crossbeam_channel::Receiver<Event>,
    ) {
        let (tx, rx) = unbounded::<Event>();
        let service = KotohaEngineService { bridge_tx: tx };
        (service, rx)
    }

    #[test]
    fn process_key_event_returns_true_on_consumed() {
        let (service, rx) = setup();
        let join = thread::spawn(move || service.process_key_event(0x6b, 45, 0));
        // engine-loop に成り代わって respond.send(Consumed) する
        match rx.recv() {
            Ok(Event::IBusKey { event: _, respond }) => {
                respond
                    .send(KeyEventResult::Consumed)
                    .expect("send Consumed");
            }
            other => panic!("expected IBusKey, got {other:?}"),
        }
        assert!(join.join().expect("thread join"));
    }

    #[test]
    fn process_key_event_returns_false_on_forwarded() {
        let (service, rx) = setup();
        let join = thread::spawn(move || service.process_key_event(0xff1b, 9, 0));
        match rx.recv() {
            Ok(Event::IBusKey { event: _, respond }) => {
                respond
                    .send(KeyEventResult::Forwarded)
                    .expect("send Forwarded");
            }
            other => panic!("expected IBusKey, got {other:?}"),
        }
        assert!(!join.join().expect("thread join"));
    }

    #[test]
    fn process_key_event_returns_false_on_timeout() {
        let (service, _rx) = setup();
        // _rx を drop しないことで bridge_tx は alive、engine-loop が hang した状態を simulate
        // (rx は受信するが respond.send は呼ばない)
        let start = std::time::Instant::now();
        let result = service.process_key_event(0x20, 65, 0);
        let elapsed = start.elapsed();
        assert!(!result, "timeout should return false (Forwarded)");
        // 100ms ~ 200ms (timing tolerance) 内に return することを観測
        assert!(
            elapsed >= Duration::from_millis(100) && elapsed < Duration::from_millis(300),
            "timeout should fire near 100ms, got {elapsed:?}"
        );
    }

    #[test]
    fn process_key_event_returns_false_on_disconnect() {
        let (tx, rx) = unbounded::<Event>();
        let service = KotohaEngineService { bridge_tx: tx };
        drop(rx); // engine-loop drop simulation
        let result = service.process_key_event(0x20, 65, 0);
        assert!(!result, "disconnect should return false");
    }

    #[test]
    fn focus_out_sends_focus_out_event() {
        let (service, rx) = setup();
        service.focus_out();
        match rx.try_recv() {
            Ok(Event::IBusReset(IBusResetKind::FocusOut)) => {}
            other => panic!("expected IBusReset(FocusOut), got {other:?}"),
        }
    }

    #[test]
    fn reset_sends_reset_event() {
        let (service, rx) = setup();
        service.reset();
        match rx.try_recv() {
            Ok(Event::IBusReset(IBusResetKind::Reset)) => {}
            other => panic!("expected IBusReset(Reset), got {other:?}"),
        }
    }

    #[test]
    fn disable_sends_disable_event() {
        let (service, rx) = setup();
        service.disable();
        match rx.try_recv() {
            Ok(Event::IBusReset(IBusResetKind::Disable)) => {}
            other => panic!("expected IBusReset(Disable), got {other:?}"),
        }
    }

    #[test]
    fn focus_in_does_not_panic_or_send() {
        let (service, rx) = setup();
        service.focus_in();
        assert!(matches!(rx.try_recv(), Err(TryRecvError::Empty)));
    }

    #[test]
    fn enable_does_not_panic_or_send() {
        let (service, rx) = setup();
        service.enable();
        assert!(matches!(rx.try_recv(), Err(TryRecvError::Empty)));
    }
}
```

- [ ] **Step 3: Register module in lib.rs**

Edit `crates/kotoha-engine-ibus/src/lib.rs`. Find the `pub(crate) mod` section and add:

```rust
pub(crate) mod service;
```

(Place adjacent to other `pub(crate) mod` declarations like `keysym` / `proxy`. Do NOT make it `pub`.)

- [ ] **Step 4: Build verify**

```bash
cargo build -p kotoha-engine-ibus 2>&1 | tail -5
```
Expected: `Finished`. If `#[interface]` macro fails, verify zbus 5 macro path (Step 1).

- [ ] **Step 5: Test verify**

```bash
cargo test -p kotoha-engine-ibus service:: 2>&1 | tail -15
```
Expected: 9 tests passed (4 process_key_event + 3 reset + 2 stub).

- [ ] **Step 6: Commit**

```bash
git add crates/kotoha-engine-ibus/src/service.rs crates/kotoha-engine-ibus/src/lib.rs
git commit -m "feat(engine-ibus): add KotohaEngineService with #[interface] dispatcher (#195)"
```

---

## Task 4: Rewrite `listener::run()` to build the connection

**Files:**
- Modify: `crates/kotoha-engine-ibus/src/listener.rs`

- [ ] **Step 1: Delete stub safety net**

In `crates/kotoha-engine-ibus/src/listener.rs`, **remove**:

- `pub const KOTOHA_ALLOW_LISTENER_STUB_ENV: &str = ...;`
- `fn allow_listener_stub() -> bool { ... }`
- `pub struct ListenerStubRefused;` and its `#[error]` block
- The 2 stub `#[ignore]` tests `run_refuses_to_start_without_env_var` and `run_exits_within_200ms_after_shutdown_request`

Keep:
- `ListenerShutdown` + `ShutdownObserver` structs and their impls
- The 3 sync unit tests `observer_reflects_trigger_state` / `cloned_observer_shares_state` / `cloned_trigger_shares_state`
- The module-level rustdoc comment (update it though — see Step 3)

- [ ] **Step 2: Rewrite `run()`**

Replace the existing `pub fn run(...) -> anyhow::Result<()> { ... }` with:

```rust
/// D-Bus listener thread の main loop。
///
/// # Preconditions
///
/// - `bridge_tx` は engine-loop thread の `EventReactor` bridge channel
/// - `shutdown` は main thread が `ListenerShutdown::request()` で停止指示する observer handle
///   (#187 split-handle、#197 SIGTERM hook 経由でも request される)
///
/// # Postconditions
///
/// - `shutdown.is_shutting_down()` を観測したら Connection を drop して `Ok(())` で return する
///   (Connection drop で zbus internal smol executor が exit、bus name が release)
/// - Connection 構築失敗(session bus 不在、bus name 占有等)時は Err propagate
///
/// # Errors
///
/// - `zbus::Error` — session bus 接続 / `serve_at` / `RequestName` 失敗
///
/// 詳細仕様: `docs/specs/_uncategorized/p3-b-ibus-listener.md` §3 / §7。
pub fn run(bridge_tx: Sender<Event>, shutdown: ShutdownObserver) -> anyhow::Result<()> {
    use crate::proxy::{IBUS_ENGINE_BUS_NAME, IBUS_ENGINE_OBJECT_PATH};
    use crate::service::KotohaEngineService;

    tracing::info!(
        bus_name = IBUS_ENGINE_BUS_NAME,
        object_path = IBUS_ENGINE_OBJECT_PATH,
        "dbus-listener starting (zbus blocking::Builder + #[interface] dispatcher)"
    );

    let service = KotohaEngineService { bridge_tx };
    let _connection = zbus::blocking::connection::Builder::session()?
        .serve_at(IBUS_ENGINE_OBJECT_PATH, service)?
        .name(IBUS_ENGINE_BUS_NAME)?
        .build()?;

    // Connection's internal smol executor dispatches incoming method calls in a
    // background thread. We just hold the connection alive and poll shutdown.
    while !shutdown.is_shutting_down() {
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    tracing::info!("dbus-listener received shutdown signal, dropping connection and exiting");
    // _connection drops here: zbus releases bus name + stops dispatching.
    Ok(())
}
```

Update the module-level rustdoc (top of `listener.rs`) to reflect production status:

```rust
//! D-Bus signal listener loop(Phase 3-B B3 + B6-b / ADR 0020 + ADR 0021)。
//!
//! `kotoha-dbus-listener` thread の main を担う。zbus 5 `blocking::connection::Builder`
//! 経由で `org.freedesktop.IBus.Engine` interface に [`crate::service::KotohaEngineService`]
//! を登録し、IBus daemon からの method 呼び出しを 6 method に分岐 dispatch する。
//!
//! # Decoded methods
//!
//! - `ProcessKeyEvent(u keyval, u keycode, u state) -> b` → `Event::IBusKey { event, respond }`
//! - `Reset()` → `Event::IBusReset(IBusResetKind::Reset)`
//! - `FocusOut()` → `Event::IBusReset(IBusResetKind::FocusOut)`
//! - `Disable()` → `Event::IBusReset(IBusResetKind::Disable)`
//! - `FocusIn()` / `Enable()` — daemon 側 introspection 互換 no-op stub
```

Also update or remove the `Connection` parameter — the new `run()` does **not** take a pre-built `_connection: zbus::blocking::Connection` parameter (it builds its own via `Builder`). This is a signature change for callers.

- [ ] **Step 3: Update kotoha-bin/src/main.rs caller**

Find the call site (`kotoha-bin/src/main.rs:285-298`) and remove the externally-built `listener_connection`:

Before:
```rust
    let listener_connection = zbus::blocking::Connection::session()
        .context("open session bus for D-Bus listener thread")?;
    let (listener_shutdown, listener_observer) =
        kotoha_engine_ibus::listener::ListenerShutdown::new();
```

After:
```rust
    let (listener_shutdown, listener_observer) =
        kotoha_engine_ibus::listener::ListenerShutdown::new();
```

And the spawn closure:

Before:
```rust
        .spawn(move || {
            kotoha_engine_ibus::listener::run(listener_connection, bridge_tx, listener_observer)
        })
```

After:
```rust
        .spawn(move || {
            kotoha_engine_ibus::listener::run(bridge_tx, listener_observer)
        })
```

Update the comment at lines 283-284 to remove the "(IBusHostBridge は内部で別 connection を持つため独立)" justification — no longer relevant.

- [ ] **Step 4: Build verify**

```bash
cargo build --workspace --all-features 2>&1 | tail -5
```
Expected: `Finished`.

- [ ] **Step 5: Test verify**

```bash
cargo test -p kotoha-engine-ibus 2>&1 | /usr/bin/grep -E "FAILED|^test result:" | tail -10
```
Expected: 0 FAILED. The 2 obsolete stub tests should be gone, the 3 ShutdownObserver tests + 9 service tests pass.

```bash
cargo test --workspace --features kotoha-storage/test-helpers,kotoha-engine-core/test-helpers 2>&1 | /usr/bin/grep -E "FAILED|^test result:" | sort | uniq -c | tail -3
```
Expected: 0 FAILED workspace-wide.

- [ ] **Step 6: Commit**

```bash
git add crates/kotoha-engine-ibus/src/listener.rs crates/kotoha-bin/src/main.rs
git commit -m "feat(engine-ibus,bin): rewrite listener::run with blocking::Builder; remove stub safety net (#195)"
```

---

## Task 5: Add L2 integration test (gated with `#[ignore]`)

**Files:**
- Create: `crates/kotoha-engine-ibus/tests/integration.rs`

- [ ] **Step 1: Write integration test**

Create `crates/kotoha-engine-ibus/tests/integration.rs`:

```rust
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
    assert!(listener_result.is_ok(), "listener exited with error: {listener_result:?}");
}
```

- [ ] **Step 2: Verify test exists and is ignored by default**

```bash
cargo test -p kotoha-engine-ibus 2>&1 | /usr/bin/grep -E "ignored|^test result:" | head -10
```
Expected: at least 1 ignored test (`listener_decodes_process_key_event`). The default test run does NOT execute it.

- [ ] **Step 3 (optional, requires dbus session bus): Run the ignored test**

```bash
cargo test -p kotoha-engine-ibus -- --ignored listener_decodes_process_key_event 2>&1 | tail -10
```
Expected on dev machine with `DBUS_SESSION_BUS_ADDRESS` set: PASS. Skip this step in CI / containers without session bus.

- [ ] **Step 4: Commit**

```bash
git add crates/kotoha-engine-ibus/tests/integration.rs
git commit -m "test(engine-ibus): add L2 integration test for listener decode path (#195)"
```

---

## Task 6: File ADR 0021

**Files:**
- Create: `docs/adr/0021-zbus-integration-architecture-for-ibus-listener.md`

- [ ] **Step 1: Write ADR**

Create `docs/adr/0021-zbus-integration-architecture-for-ibus-listener.md`:

```markdown
# ADR 0021: zbus integration architecture for IBus engine listener

| 項目 | 値 |
|------|----|
| Status | Accepted |
| Date | 2026-05-08 |
| Phase | Phase 3-B B6-b (#195) |
| 関連 spec | `docs/specs/_uncategorized/p3-b-ibus-listener.md` |
| 関連 ADR | ADR 0017(IBus engine API surface and async modality)、ADR 0020(event-loop architecture) |

## Context

Phase 3-B B6-b で `kotoha-engine-ibus::listener::run` の stub を production 実装に置き換える。listener thread は IBus daemon から発信される `org.freedesktop.IBus.Engine` interface の method 呼び出し(`ProcessKeyEvent` ほか 5 件)を decode し、`Event` enum で engine-loop thread に伝達する責務を持つ。

zbus 5 における method dispatch には複数の実現経路があり、本 ADR はその選定を記録する。

## Decision

以下を採択する:

1. **decode pattern**: zbus 5 `#[interface]` macro を `KotohaEngineService` struct に適用する。`blocking::connection::Builder::serve_at(path, service)` + `name(bus_name)` で session bus に publish する。
2. **`process_key_event` の戻り値プラミング**: `Event::IBusKey { event, respond: Sender<KeyEventResult> }` で respond channel を同梱し、listener は `recv_timeout(100ms)` で待機する。timeout 時は `Forwarded` 相当として bool false を返す。
3. **threading**: listener thread は単独で `blocking::Connection` を保持し、内部 smol executor が dispatch を回す。本 crate コードは std::sync 同期 primitive のみ使用(ADR 0017「core は tokio 非依存」原則維持)。
4. **stub safety net 削除**: `KOTOHA_ALLOW_LISTENER_STUB_ENV` / `allow_listener_stub` / `ListenerStubRefused` を完全削除する。listener が production 実装になるため不要。

詳細実装契約は spec §4-§7 を参照。

## Rejected alternatives

### A. `MessageStream` + 手動 match dispatch

zbus 5 の async API 主導であり、blocking 系から `MessageStream` を直接 receive する経路は限定的。`#[interface]` macro が defacto standard で記述量も少ない。本 use case で manual dispatch を採る理由は薄い。

### B. `Arc<AtomicBool>` snapshot による listener-side enabled cache

`process_key_event` の bool 戻り値を listener が独自判定する案。
- 利点: 同期 channel 不要、低 latency
- 致命的欠点: spec §5.2 row が要求する `Forwarded` 精度(Ctrl+C 等の特殊 key の forward 判定)を提供できない。「enabled だが Forwarded すべき key」を listener が常に true と返す → アプリが受け取らない誤動作

### C. `ForwardKeyEvent` signal による二相 pattern

`process_key_event` を常に true で claim し、後続で engine から `ForwardKeyEvent` signal を発信して daemon に keystroke 再 routing させる。
- 利点: listener 完全 async、blocking なし
- 欠点: API surface 拡大(`IMEHostBridge` trait に method 追加 + IBusEngineSignals 追加)、二相セマンティクスの複雑性、daemon round-trip を含む race condition の debug 経路、integration test が daemon 必須となり regression 検出網が薄くなる
- 結論: maintenance vs ミリ秒単位の理論的応答性差を秤にかけ、Phase 3-B 段階では (i)+(A) を優先

## Consequences

### Positive

- 4 thread topology(ADR 0020 §採択 Q4)を維持しつつ listener thread が production 実装になる
- `Event::IBusKey { event, respond }` 拡張は engine_loop の既存 dispatch 経路に最小変更で統合される
- ADR 0017 の "core は tokio 非依存" 原則を維持(zbus internal smol executor は本 crate コードに propagate しない)

### Negative

- `process_key_event` 経路で listener thread が最大 100ms blocking する。fast typist (10 keys/sec) でも通常 ~1ms / heavy ~10ms / recovery ~50ms で吸収できる範囲(spec §6.1)。
- timeout fire(>100ms)時に engine が eventual に Consumed 判定すると double-input が原理的に起こりうる(spec §6.2 で受容、Phase 5/6 で deadline check 拡張余地)

### Neutral

- IBus engine factory registration の正規経路(`/usr/share/ibus/component/<name>.xml`)は本 ADR scope 外。packaging task として別 ISSUE で扱う。

## Future revisit triggers

- Phase 5 custom romaji-base model の inference latency が増えた場合、100ms timeout 値の引き上げ / `Event::IBusKey` への deadline 拡張 / `async fn` method 化 + smol::unblock を再評価する
- Phase 4 fcitx5 adapter で異なる protocol への generalization が必要な場合、`KotohaEngineService` を trait 化する余地
```

- [ ] **Step 2: Verify pre-commit doc-naming hook**

```bash
git add docs/adr/0021-zbus-integration-architecture-for-ibus-listener.md
```
The pre-commit hook (lefthook) validates ADR file naming. Verify the filename matches `NNNN-<title>.md` pattern.

- [ ] **Step 3: Commit**

```bash
git commit -m "docs(adr): ADR 0021 zbus integration architecture for IBus listener (#195)"
```

---

## Task 7: Update spec p3-a-ibus-engine.md

**Files:**
- Modify: `docs/specs/_uncategorized/p3-a-ibus-engine.md`

- [ ] **Step 1: Update §6.1 step [1] listener stub description**

Find §6.1 step [1] in the spec. The current text describes the listener as a stub awaiting B6 manual smoke. Replace with a sentence describing the production implementation:

```markdown
**[1] D-Bus listener thread**: `kotoha-engine-ibus::listener::run` が `blocking::connection::Builder::serve_at + name` 経由で `org.freedesktop.IBus.Engine.Kotoha` bus name + `/org/freedesktop/IBus/Engine/Kotoha` object path を publish し、`KotohaEngineService` struct(`#[interface]` macro)が IBus 1.5.x の 4 主要 method(`ProcessKeyEvent` / `FocusOut` / `Reset` / `Disable`)+ 2 no-op stub(`FocusIn` / `Enable`)を decode する。`process_key_event` は `Event::IBusKey { event, respond }` 経由で engine-loop に dispatch し、`recv_timeout(100ms)` で `KeyEventResult` を待つ。詳細は `docs/specs/_uncategorized/p3-b-ibus-listener.md` および ADR 0021 を参照。
```

(adapt wording to fit the surrounding text — preserve the existing §6.1 numbered structure)

- [ ] **Step 2: Update §11 acceptance criteria checkboxes**

Find §11 (or the equivalent section listing Phase 3-B acceptance criteria). Mark items B6-b satisfies as complete:

- listener が IBus daemon registration を行う: `[x]` (B6-b 実装済、ADR 0021)
- 4 IBus methods decoded: `[x]`
- `KOTOHA_ALLOW_LISTENER_STUB_ENV` 削除: `[x]`

Items still pending (B6-c #196 manual smoke):
- L3 manual smoke 10 件: `[ ]` (#196 で扱う)
- coalescing-window 値 empirical 確定: `[ ]` (#196 manual smoke 内で観測)

(scan §11 carefully, mark only what B6-b actually delivers — defer manual smoke / empirical observations to #196)

- [ ] **Step 3: Update frontmatter `last_reviewed`**

In the spec frontmatter, change `last_reviewed: 2026-05-06` (or whatever current value) to `last_reviewed: 2026-05-08`.

- [ ] **Step 4: Append to revision history (if exists in spec, otherwise skip)**

If `## 改訂履歴` table exists at the end:

```markdown
| 2026-05-08 | Phase 3-B B6-b(#195)merge 後の §6.1 step [1] / §11 acceptance criteria 更新。listener 実装が production 化、ADR 0021 起票 |
```

- [ ] **Step 5: Verify pre-commit hooks**

```bash
git add docs/specs/_uncategorized/p3-a-ibus-engine.md
```

Run `lefthook run pre-commit` if needed; expect doc-naming + frontmatter validation to pass.

- [ ] **Step 6: Commit**

```bash
git commit -m "docs(spec): mark P3-B B6-b acceptance criteria complete in p3-a-ibus-engine (#195)"
```

---

## Task 8: Update ROADMAP

**Files:**
- Modify: `docs/ROADMAP.md`

- [ ] **Step 1: Update Active milestone v0.3.0 table**

Find the table under `### v0.3.0 — Phase 3 IBus integration`. The row for #136 currently reads:

```markdown
| [#136](...) P3-B B3 + B6: signal listener loop + L3 manual smoke | `docs/specs/_uncategorized/p3-a-ibus-engine.md` | [ ] |
```

Status remains `[ ]` (B6-c #196 manual smoke is still pending). Add a sub-bullet or note clarifying B6-a/b complete.

Also add a new row for #195:

```markdown
| [#195](https://github.com/std-koh-hinooka/kotoha-ime/issues/195) P3-B B6-b: zbus MessageStream decode + IBus engine factory registration | `docs/specs/_uncategorized/p3-b-ibus-listener.md` | [x] |
```

(Insert this row above the #136 row to maintain logical ordering.)

- [ ] **Step 2: Update Phase 3-B sub-milestone table**

Find `## Phase 3 マイルストーン分割` section. Update the B6 row:

Before:
```markdown
| **B6 (#136 残)** | L3 manual smoke on GNOME Wayland(...)+ zbus `MessageStream` 完全 decode + SIGTERM/SIGINT hook(ADR 0020 §影響 rev2 で deferral 明記) | **未着手** |
```

After:
```markdown
| **B6-a (#197)** | SIGTERM/SIGINT shutdown hook in kotoha-bin (ctrlc crate) | **完了**(PR #198 squash `4ea777c`) |
| **B6-b (#195)** | zbus `blocking::Builder` + `#[interface]` 経由の IBus engine method decode + factory registration、ADR 0021 | **完了**(PR #<TBD>) |
| **B6-c (#196 残)** | L3 manual smoke on GNOME Wayland(Firefox / GNOME Text Editor / VS Code で典型変換 10 件)、coalescing window empirical 確定 | **進行中**(B6-b merge 後、user-driven 実機検証) |
```

(replace the single B6 row with these 3 rows)

- [ ] **Step 3: Update Phase 3 status row in `## Phase 一覧` table**

Before:
```markdown
| 3 | IBus integration | IBus engine(GNOME Mutter 用) | **進行中** (P3-A draft + P3-B B0/B0g/B0h 全/B1/B2/B3/B4/B5 完了。残 B6 = L3 manual smoke + zbus decode + SIGTERM hook) |
```

After:
```markdown
| 3 | IBus integration | IBus engine(GNOME Mutter 用) | **進行中** (P3-A draft + P3-B B0/B0g/B0h 全/B1/B2/B3/B4/B5/B6-a/B6-b 完了。残 B6-c = L3 manual smoke #196 のみ) |
```

- [ ] **Step 4: Add revision history entry**

Find the `## 改訂履歴` table at the bottom. Add a new top row:

```markdown
| 2026-05-08 | Phase 3-B B6-b(#195、PR #<TBD>)merge 完了。listener stub から production 実装(zbus `#[interface]` + `blocking::Builder`)へ置換、ADR 0021 起票。Phase 3-B sub-milestone table を B6-a / B6-b / B6-c に分割記載。Phase 3 status row 更新(残 B6-c のみ)|
```

(`<TBD>` は実 PR 番号で commit 直前に置換するか、commit 後の amend で更新)

- [ ] **Step 5: Commit**

```bash
git add docs/ROADMAP.md
git commit -m "docs(roadmap): mark P3-B B6-a/B6-b complete; split B6 sub-milestone table (#195)"
```

---

## Task 9: Final verification + push + PR

- [ ] **Step 1: Workspace-wide build**

```bash
cargo build --workspace --all-features 2>&1 | tail -3
```
Expected: `Finished`.

- [ ] **Step 2: Workspace-wide clippy with -D warnings**

```bash
cargo clippy --workspace --all-targets --all-features -- -D warnings 2>&1 | tail -3
```
Expected: `Finished` with no `warning:` or `error:` lines.

- [ ] **Step 3: Workspace-wide test**

```bash
cargo test --workspace --features kotoha-storage/test-helpers,kotoha-engine-core/test-helpers 2>&1 | /usr/bin/grep -E "FAILED|^failures:" | head -5
echo "---all clear if no FAILED above---"
```
Expected: empty output above the marker (no failures). If `kotoha-storage` SQLite parallel test flake fires, retry once.

- [ ] **Step 4: Format check**

```bash
cargo fmt --all -- --check 2>&1 | tail -3
```
Expected: empty output (no diff). If diff prints, run `cargo fmt --all` and amend the most recent commit.

- [ ] **Step 5: Push (triggers pre-push hook chain)**

```bash
git push -u origin feature/195-zbus-listener-decode 2>&1 | tail -10
```

The pre-push hook runs `build` + `clippy` + `manifest-check` + `obsidian-sync` + `test` + `test-dict` + `test-dict-persist`. All must pass. If `test-dict-persist` flakes (kotoha-storage SQLite parallel), retry the push.

- [ ] **Step 6: Switch gh account if needed and create PR**

```bash
gh auth switch --user std-koh-hinooka
gh pr create --base develop \
  --title "feat(engine-ibus,bin): production IBus listener via zbus #[interface] + 100ms response channel (#195)" \
  --body "$(cat <<'EOF'
## Summary

Replace the kotoha-engine-ibus listener stub with a production zbus 5 `#[interface]` dispatcher that decodes 4 IBus engine methods (+ 2 introspection no-op stubs) and forwards them as `Event` to engine-loop. `Event::IBusKey` extends with a bounded response `Sender<KeyEventResult>` so `process_key_event` returns the IME-correct bool with a 100ms timeout fallback. `KOTOHA_ALLOW_LISTENER_STUB` env var safety net is removed entirely. Closes #195.

## Spec & ADR

- Spec: `docs/specs/_uncategorized/p3-b-ibus-listener.md`
- ADR: 0021 (zbus integration architecture for IBus listener)
- Plan: `docs/plans/2026-05-08-feature-195-zbus-listener-decode.md`

## Change scope

(filled in from `git diff --stat develop..HEAD` at commit time — expect ~500 lines)

## Decisions captured

- zbus 5 `#[interface]` macro on `KotohaEngineService` (rejected: `MessageStream` manual match)
- `Event::IBusKey { event, respond }` synchronous response channel (rejected: `Arc<AtomicBool>` snapshot, `ForwardKeyEvent` two-phase signal)
- 100ms timeout (rejected: 50ms — too tight for worker recovery case)

## Self-review

- **Architecture**: aligns ADR 0017 (tokio non-dependence) and ADR 0020 (4-thread lock-free); zbus internal smol executor does not propagate to crate code
- **Type design**: `Event::IBusKey` struct variant makes the response channel mandatory at the type level; bounded(1) channel enforces 1:1 send/recv
- **Testing**: 9 L1 unit tests cover the 4 process_key_event paths + 3 reset paths + 2 stub paths; 1 L2 integration test (`#[ignore]` for dbus session bus) exercises decode end-to-end
- **Silent failure**: timeout / disconnect both emit `tracing::warn!` with `error_id` for log filtering
- **Performance**: typical 1ms response; pathological case fallback at 100ms still under 10 keys/sec inter-keystroke interval

## Test plan

- [x] cargo build --workspace --all-features clean
- [x] cargo clippy --workspace --all-targets --all-features -- -D warnings clean
- [x] cargo test --workspace --features kotoha-storage/test-helpers,kotoha-engine-core/test-helpers (0 failures)
- [x] cargo fmt --all -- --check clean
- [x] lefthook pre-commit + pre-push pass
- [ ] L2 integration test under `cargo test -- --ignored` (manual, dev machine)
- [ ] L3 manual smoke deferred to #196

🤖 Generated with [Claude Code](https://claude.com/claude-code)
EOF
)"
```

Note: PR creation may need `gh auth switch --user std-koh-hinooka` first if active account differs.

- [ ] **Step 7: Self-review the diff**

Open the PR in browser. Skim the diff for:
- Any commented-out code that should be deleted
- Unused imports surfaced by clippy --all-targets
- Doc comments referencing removed types (`ListenerStubRefused`, `KOTOHA_ALLOW_LISTENER_STUB_ENV`)
- ADR 0021 internal cross-references resolving correctly

If issues found, push fix commits before merge.

- [ ] **Step 8: Merge PR (under (c) cadence pre-authorization)**

```bash
gh pr merge <PR-number> --squash --delete-branch
```

- [ ] **Step 9: Sync develop locally**

```bash
git checkout develop
git pull --ff-only
git branch -d feature/195-zbus-listener-decode 2>&1 || true
```

- [ ] **Step 10: Update todos**

Mark #195 implementation complete in TodoWrite. Note the next pending task is #196 (L3 manual smoke), which is user-driven.

---

## Self-review of this plan

**1. Spec coverage**

Walking spec sections vs plan tasks:

- §2.1 scope: 6 methods + bus name + Event::IBusKey + safety-net removal + ADR — covered Tasks 1-6
- §3 architecture: covered in Task 4 listener.rs rewrite
- §4 KotohaEngineService API: Task 3
- §5 data flow: Task 3 (service code) + Task 2 (engine_loop)
- §6 timeout: Task 3 (PROCESS_KEY_EVENT_TIMEOUT constant)
- §7 daemon integration: Task 4 (Builder chain)
- §8 deletion: Task 4 step 1
- §9 error: Task 3 (tracing) + Task 4 (Connection drop on shutdown)
- §10 testing: Task 3 (L1 unit) + Task 5 (L2 ignored)
- §11 implementation roadmap: this plan is the operational version
- §12 ADR + spec updates: Task 6 (ADR 0021) + Task 7 (spec p3-a) + Task 8 (ROADMAP)
- §13 forward direction: documentation only, no task

✓ All sections mapped.

**2. Placeholder scan**

Searching for "TBD" / "TODO" / "implement later":

- "PR #<TBD>" appears in Task 8 (ROADMAP entry) — intentional, replaced at commit time
- Step 7 of Task 9 says "Open the PR in browser. Skim the diff" — this is a real action, not a placeholder
- Otherwise no `TBD` / `add appropriate error handling` / similar placeholders

✓ No placeholder violations.

**3. Type consistency**

- `Event::IBusKey { event, respond }` — same field names in event.rs, engine_loop.rs, service.rs throughout
- `KeyEventResult::{Consumed, Forwarded}` — used consistently in service.rs unit tests + ADR + spec
- `IBUS_ENGINE_BUS_NAME` / `IBUS_ENGINE_OBJECT_PATH` — referenced consistently in Task 1 (define) + Task 4 (use)
- `KotohaEngineService` — same struct name throughout
- `ShutdownObserver` / `ListenerShutdown` — consistent (defined in #187, kept in Task 4 with 3 unit tests preserved)
- `run(bridge_tx, shutdown)` signature change in Task 4 step 2 + main.rs caller update in Task 4 step 3 — consistent

✓ Names match across tasks.

**4. Sequence soundness**

- Task 1 (constants): independent, no dependency
- Task 2 (Event variant change): breaks build until callers updated; updated atomically within Task 2
- Task 3 (service.rs): depends on Task 2 (uses new variant); references constants from Task 1
- Task 4 (listener.rs rewrite): depends on Task 1 (constants) + Task 3 (service struct)
- Task 5 (L2 test): depends on Task 4 (run signature)
- Task 6 (ADR): independent, can run any time after Task 4 implementation is fixed
- Task 7 (spec p3-a): independent
- Task 8 (ROADMAP): independent
- Task 9 (final verify + PR): depends on all preceding

✓ Sequence is sound.
