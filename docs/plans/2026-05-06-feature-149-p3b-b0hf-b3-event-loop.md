# Phase 3-B B0h-f + B3 event-loop architecture Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.
>
> **Project-specific override:** Kotoha は **Spec-Driven, Test-After** flow を採用する(global CLAUDE.md §Development Flow)。`superpowers:test-driven-development` は **banned**。本 plan の各 Phase は「実装 → cargo build/clippy 検証 → spec §6/§10/§13 acceptance criteria 由来の test を追加 → 検証 → commit」の順で進める。

**Goal:** B0h-f(I3 dispatch async-ification)と B3(D-Bus signal listener loop)を一体化し、ADR 0020 の 4-thread lock-free event-loop architecture に切り替える。`drain_events_blocking` と `Arc<Mutex<dyn IMEEngine>>`(B0h-d 導入)を撤去し、`KotohaEngine` を engine-loop thread が単独所有する。

**Architecture:** `kotoha-engine-core` に OS 非依存の `EventReactor` trait + `Event` sum 型を追加し、新 crate `kotoha-engine-reactor-linux` に Linux 専用実装(crossbeam-channel `select!` ベース)を置く。`kotoha-bin` に main / dbus-listener / engine-loop / ranker-worker の 4 thread を spawn し、bridge channel と worker channel を fan-in で engine-loop に届ける。詳細は ADR 0020 / spec `p3-a-ibus-engine.md` §6.0 / §7。

**Tech Stack:** Rust 2021 (rust-version 1.80) / Cargo workspace / crossbeam-channel 0.5 / zbus 5 / std::thread / std::sync (no tokio per ADR 0017 rev2)

**File ownership / change summary (ADR 0020 §影響範囲 と一致)**

| Path | 種別 | 想定 LOC |
|---|---|---|
| `Cargo.toml` (workspace root) | Modify | +3 |
| `crates/kotoha-engine-core/Cargo.toml` | Modify | +1 |
| `crates/kotoha-engine-core/src/reactor/mod.rs` | Create | ~80 |
| `crates/kotoha-engine-core/src/reactor/event.rs` | Create | ~70 |
| `crates/kotoha-engine-core/src/lib.rs` | Modify | +5 |
| `crates/kotoha-engine-core/src/engine/mod.rs` | Modify | -50 / +30 |
| `crates/kotoha-engine-core/src/engine/event.rs` | Modify | conversion to `From<EngineEvent> for Event` |
| `crates/kotoha-engine-core/src/engine/worker_channel.rs` | Modify | crossbeam Sender/Receiver 化、Drop ordering 維持 |
| `crates/kotoha-engine-reactor-linux/Cargo.toml` | Create | ~30 |
| `crates/kotoha-engine-reactor-linux/src/lib.rs` | Create | ~250 |
| `crates/kotoha-engine-ibus/Cargo.toml` | Modify | +1(crossbeam) |
| `crates/kotoha-engine-ibus/src/listener.rs` | Create | ~120 |
| `crates/kotoha-engine-ibus/src/dispatcher.rs` | Modify | -40(Mutex 撤去) |
| `crates/kotoha-engine-ibus/src/lib.rs` | Modify | +2 (listener mod 追加) |
| `crates/kotoha-bin/Cargo.toml` | Modify | +1(crossbeam) |
| `crates/kotoha-bin/src/main.rs` | Modify | -20 / +60(thread spawn / join、`anyhow::bail!` 撤去) |
| `crates/kotoha-bin/src/engine_loop.rs` | Create | ~100 |
| `crates/kotoha-engine-core/src/reactor/mock.rs` | Create | ~50(test-helpers feature gate) |
| 既存 test ファイル群 | Modify | ~80 (drain_events_blocking 撤去対応) |
| `crates/kotoha-engine-reactor-linux/tests/integration.rs` | Create | ~120 |

**合計** 約 920 LOC、約 13-15 file。global rule §PR Review Matrix の Large 上限(≤20 files / ≤1000 lines)内。

---

## Phase A: Foundation — engine-core に reactor module を追加

依存追加と OS 非依存 trait / sum 型を最初に確定する。本 phase 完了時点では既存挙動に影響なし(新 type を導入するのみ)。

### Task A1: workspace root に crossbeam-channel を pin

**Files:**
- Modify: `Cargo.toml`(workspace root)

- [ ] **Step 1: workspace dependency を追加**

`[workspace.dependencies]` セクション末尾に追記する(serde の commented-out 行のすぐ後ろ):

```toml
# crossbeam-channel pinned via P3-B B0h-f + B3 一体化 (2026-05-06、ADR 0020 §採択 Q2)。
# Adopted reason: ADR 0017 rev2「core は tokio 非依存」原則を維持しつつ
# multi-source event multiplex を select! macro で型安全に表現可能。
# Alternatives rejected (ADR 0020 §採択 Q2 参照):
#   - mio: source 数 3-4 で表現力過剰、Linux/BSD 限定
#   - tokio: ADR 0017 全面 revise + std::thread 大量書換、scope 膨張
#   - kanal 0.2.0-beta1: pre-1.0 で API 安定性懸念
#   - flume 0.12: crossbeam ほど成熟していない
crossbeam-channel = "0.5"
```

- [ ] **Step 2: cargo metadata で workspace dep が認識されるか確認**

```bash
cargo metadata --format-version 1 --no-deps > /dev/null
```
Expected: exit 0(error なし)。

- [ ] **Step 3: コミット**

```bash
git add Cargo.toml
git commit -m "build(workspace): pin crossbeam-channel 0.5 for B0h-f event-loop (ADR 0020)"
```

### Task A2: engine-core が crossbeam-channel を参照する

**Files:**
- Modify: `crates/kotoha-engine-core/Cargo.toml:22`

- [ ] **Step 1: dependency 追加**

`[dependencies]` セクションの `bitflags = { workspace = true }` の直後に挿入:

```toml
crossbeam-channel = { workspace = true }
```

- [ ] **Step 2: ビルド確認**

```bash
cargo build -p kotoha-engine-core
```
Expected: `Compiling crossbeam-channel ...` を経て `Compiling kotoha-engine-core ...` で成功(warning 0)。

- [ ] **Step 3: コミット**

```bash
git add crates/kotoha-engine-core/Cargo.toml
git commit -m "build(engine-core): depend on crossbeam-channel for reactor module (ADR 0020)"
```

### Task A3: `reactor/event.rs` を新規作成

**Files:**
- Create: `crates/kotoha-engine-core/src/reactor/event.rs`

- [ ] **Step 1: ファイルを作成**

```rust
//! `Event` sum 型と関連 enum 定義。
//!
//! 本 module は engine-loop thread が `EventReactor::recv()` で受け取る
//! 全 event source(D-Bus / worker / shutdown / 将来の notification)を
//! 単一 sum 型に集約する。詳細は spec `p3-a-ibus-engine.md` §7.1 / ADR 0020 §採択 Q4。

use crate::engine::event::CandidateUpdate;
use crate::key_event::KeyEvent;

/// engine-loop thread が単一 thread で multiplex する全 event source の sum 型。
///
/// # Invariants
///
/// - 全 variant は `Send + 'static`(crossbeam channel 越しに送るため)
/// - `#[non_exhaustive]` で将来の variant 追加に備える(Phase 5 notification 等)
#[non_exhaustive]
#[derive(Debug)]
pub enum Event {
    /// IBus session bus で受信した key event。
    /// dbus-listener thread が `Sender<Event>::send` で engine-loop に届ける。
    IBusKey(KeyEvent),

    /// IBus focus_out / reset / disable 系。
    IBusReset(IBusResetKind),

    /// ranker-worker thread が生成した候補 update + worker error。
    /// 旧 `EngineEvent` の `Candidates` / `WorkerError` variant を統合。
    WorkerOutput {
        request_id: u64,
        payload: WorkerPayload,
    },

    /// 全 thread に shutdown を通知する sentinel。
    ///
    /// # Postconditions
    /// - engine-loop は本 variant を観測した直後に loop から `Ok(())` で抜ける
    /// - 全 `Sender<Event>` が drop されても `Receiver::recv()` が `Err(Disconnected)`
    ///   を返すため、shutdown 経路は二重に冗長化されている
    Shutdown,
}

/// IBus からの focus_out / reset / disable 系 signal の種別。
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IBusResetKind {
    FocusOut,
    Reset,
    Disable,
}

/// `Event::WorkerOutput` の payload。
///
/// 旧 `EngineEvent::Candidates` / `EngineEvent::WorkerError` を統合した形。
#[non_exhaustive]
#[derive(Debug)]
pub enum WorkerPayload {
    /// ranker が生成した候補差分。
    Candidates(CandidateUpdate),

    /// worker 内部 panic / 致命 error 検出時に engine が再生成判断するための signal。
    Error(String),
}
```

- [ ] **Step 2: コンパイル確認**

```bash
cargo build -p kotoha-engine-core
```
Expected: `module declared but not used` の warning が出る可能性あり(次 Task で `mod reactor;` を追加する)。エラーは無いこと。

### Task A4: `reactor/mod.rs` で `EventReactor` trait を定義

**Files:**
- Create: `crates/kotoha-engine-core/src/reactor/mod.rs`

- [ ] **Step 1: ファイルを作成**

```rust
//! `EventReactor` — multi-source event multiplexer の OS 非依存 port。
//!
//! 本 module は ADR 0020 §採択 Q3 の「engine-core 側 trait のみ」境界に対応する。
//! Linux 実装は `kotoha-engine-reactor-linux` crate に置く。本 module には
//! `select!` macro / OS 依存 primitive を含めない。

pub mod event;

#[cfg(any(test, feature = "test-helpers"))]
pub mod mock;

pub use event::{Event, IBusResetKind, WorkerPayload};

use std::time::Duration;

/// engine-loop thread が単一 thread で multiplex する event 受信機構の port。
///
/// # Invariants
///
/// - 実装は `Send + 'static`(engine-loop thread に move される)
/// - `recv` は `Event::Shutdown` または全 Sender drop を観測したら以降
///   `Err(crossbeam_channel::RecvError)` を返してよい
///
/// # Examples
///
/// ```ignore
/// use kotoha_engine_core::reactor::{Event, EventReactor};
///
/// fn run<R: EventReactor>(reactor: &R) -> Result<(), crossbeam_channel::RecvError> {
///     loop {
///         match reactor.recv()? {
///             Event::Shutdown => break Ok(()),
///             other => { /* dispatch */ }
///         }
///     }
/// }
/// ```
pub trait EventReactor: Send + 'static {
    /// 単一 event を blocking 受信する。
    ///
    /// # Errors
    ///
    /// - 全 Sender が drop された場合 `crossbeam_channel::RecvError`
    fn recv(&self) -> Result<Event, crossbeam_channel::RecvError>;

    /// timeout 付き受信。`coalescing window` 等で利用する。
    ///
    /// # Errors
    ///
    /// - timeout: `crossbeam_channel::RecvTimeoutError::Timeout`
    /// - 全 Sender drop: `crossbeam_channel::RecvTimeoutError::Disconnected`
    fn recv_timeout(
        &self,
        timeout: Duration,
    ) -> Result<Event, crossbeam_channel::RecvTimeoutError>;
}
```

- [ ] **Step 2: コンパイル確認**

```bash
cargo build -p kotoha-engine-core
```
Expected: `module reactor declared but not exported` の warning(次 Task で対応)。エラーなし。

### Task A5: `lib.rs` から reactor module を export

**Files:**
- Modify: `crates/kotoha-engine-core/src/lib.rs:26-32`

- [ ] **Step 1: pub mod 行を追加**

`pub mod ranker;` の直後、`pub mod sanitize;` の前に挿入:

```rust
pub mod reactor;
```

そして lib.rs の `pub use` 群末尾に追記:

```rust
pub use reactor::{Event, EventReactor, IBusResetKind, WorkerPayload};
```

- [ ] **Step 2: ビルド確認**

```bash
cargo build -p kotoha-engine-core --all-targets
```
Expected: warning 0、error 0。

- [ ] **Step 3: clippy 確認**

```bash
cargo clippy -p kotoha-engine-core --all-targets -- -D warnings
```
Expected: pass。

- [ ] **Step 4: コミット**

```bash
git add crates/kotoha-engine-core/src/reactor crates/kotoha-engine-core/src/lib.rs
git commit -m "feat(engine-core): add reactor module with EventReactor trait + Event sum (ADR 0020)"
```

---

## Phase B: 新 crate `kotoha-engine-reactor-linux`

Linux 専用 reactor 実装を新 crate に隔離する。OS 依存(`select!` macro / eventfd 等)は本 crate のみが知る。

### Task B1: 新 crate ディレクトリと Cargo.toml を作成

**Files:**
- Create: `crates/kotoha-engine-reactor-linux/Cargo.toml`
- Create: `crates/kotoha-engine-reactor-linux/src/lib.rs`(空 placeholder で OK)
- Modify: `Cargo.toml`(workspace root)

- [ ] **Step 1: ディレクトリ作成と Cargo.toml**

```bash
mkdir -p crates/kotoha-engine-reactor-linux/src
```

`crates/kotoha-engine-reactor-linux/Cargo.toml`:

```toml
[package]
name = "kotoha-engine-reactor-linux"
description = "Linux event reactor implementation for Kotoha engine-loop (ADR 0020)"
edition.workspace = true
rust-version.workspace = true
version.workspace = true
license.workspace = true
repository.workspace = true
authors.workspace = true

[dependencies]
crossbeam-channel = { workspace = true }
kotoha-engine-core = { path = "../kotoha-engine-core" }
tracing = { workspace = true }

[dev-dependencies]
kotoha-engine-core = { path = "../kotoha-engine-core", features = ["test-helpers"] }
```

- [ ] **Step 2: workspace members に追加**

`Cargo.toml`(root)の `[workspace] members = [...]` に `"crates/kotoha-engine-reactor-linux",` を追加(`kotoha-engine-ibus` の前)。

- [ ] **Step 3: 空の lib.rs**

`crates/kotoha-engine-reactor-linux/src/lib.rs`:

```rust
//! `kotoha-engine-reactor-linux` — Linux 専用 `EventReactor` 実装。
//!
//! ADR 0020 §採択 Q3 で確定した crate 境界に従い、本 crate は OS 依存
//! (`crossbeam-channel::select!` macro)を内部に閉じる。`kotoha-engine-core`
//! 側は `EventReactor` trait のみを提供する。

// 本体は次 Task で追加。
```

- [ ] **Step 4: ビルド確認**

```bash
cargo build -p kotoha-engine-reactor-linux
```
Expected: `Compiling kotoha-engine-reactor-linux ...` 成功、warning 0。

### Task B2: `LinuxReactor` を実装

**Files:**
- Modify: `crates/kotoha-engine-reactor-linux/src/lib.rs`

- [ ] **Step 1: 実装を書き換える**

```rust
//! `kotoha-engine-reactor-linux` — Linux 専用 `EventReactor` 実装。
//!
//! ADR 0020 §採択 Q3 で確定した crate 境界に従い、本 crate は OS 依存
//! (`crossbeam-channel::select!` macro)を内部に閉じる。`kotoha-engine-core`
//! 側は `EventReactor` trait のみを提供する。
//!
//! # Architecture
//!
//! `LinuxReactor` は 3 つの内部 channel を持ち、`select!` で multiplex する:
//!
//! - `bridge_rx: Receiver<Event>` — dbus-listener thread から
//! - `worker_rx: Receiver<Event>` — ranker-worker thread から
//! - `shutdown_rx: Receiver<()>` — main thread から(close で発火)
//!
//! `bridge_tx` / `worker_tx` を別 channel に保つ理由は、将来の優先度制御
//! (`select!` の bias)を可能にするため。fan-in 統一でも機能上は問題ないが、
//! ADR 0020 §採択 Q4 の通り拡張点として分離する。

use std::time::Duration;

use crossbeam_channel::{select, unbounded, Receiver, RecvError, RecvTimeoutError, Sender};
use kotoha_engine_core::reactor::{Event, EventReactor};

/// Linux 専用の event reactor 実装。
///
/// # Invariants
///
/// - `select!` の arm 順序は `bridge_rx` → `worker_rx` → `shutdown_rx`。
///   crossbeam-channel の `select!` は当落結果を疑似乱数で選ぶため、
///   並列到着時の公平性は保証される(順序自体は documentation のため)。
/// - shutdown は `shutdown_rx` の close または全 Sender drop で伝搬する。
pub struct LinuxReactor {
    bridge_rx: Receiver<Event>,
    worker_rx: Receiver<Event>,
    shutdown_rx: Receiver<()>,
}

/// `LinuxReactor` 構築時に同時に得られる producer 側 handle 群。
///
/// main thread が DI wiring 時に保持し、各 producer thread に move する。
#[must_use = "ReactorHandles を drop すると engine-loop が即時 shutdown する"]
pub struct ReactorHandles {
    pub reactor: LinuxReactor,
    pub bridge_tx: Sender<Event>,
    pub worker_tx: Sender<Event>,
    pub shutdown_tx: Sender<()>,
}

impl LinuxReactor {
    /// 新 reactor + producer handle 群を生成する。
    ///
    /// # Postconditions
    ///
    /// - すべての channel は unbounded(IME 用途で send-side back-pressure は不要)
    /// - producer 側 Sender は `clone()` 可能
    #[must_use]
    pub fn new() -> ReactorHandles {
        let (bridge_tx, bridge_rx) = unbounded::<Event>();
        let (worker_tx, worker_rx) = unbounded::<Event>();
        let (shutdown_tx, shutdown_rx) = unbounded::<()>();
        ReactorHandles {
            reactor: Self {
                bridge_rx,
                worker_rx,
                shutdown_rx,
            },
            bridge_tx,
            worker_tx,
            shutdown_tx,
        }
    }
}

impl EventReactor for LinuxReactor {
    fn recv(&self) -> Result<Event, RecvError> {
        select! {
            recv(self.bridge_rx) -> ev => ev,
            recv(self.worker_rx) -> ev => ev,
            recv(self.shutdown_rx) -> r => {
                // shutdown_rx は () を送る、または close (Err) で発火する。
                // どちらの場合も Event::Shutdown に正規化する。
                let _ = r;
                Ok(Event::Shutdown)
            }
        }
    }

    fn recv_timeout(&self, timeout: Duration) -> Result<Event, RecvTimeoutError> {
        select! {
            recv(self.bridge_rx) -> ev => ev.map_err(RecvTimeoutError::from),
            recv(self.worker_rx) -> ev => ev.map_err(RecvTimeoutError::from),
            recv(self.shutdown_rx) -> r => {
                let _ = r;
                Ok(Event::Shutdown)
            }
            default(timeout) => Err(RecvTimeoutError::Timeout),
        }
    }
}
```

- [ ] **Step 2: ビルド + clippy 確認**

```bash
cargo build -p kotoha-engine-reactor-linux --all-targets
cargo clippy -p kotoha-engine-reactor-linux --all-targets -- -D warnings
```
Expected: 両方 pass、warning 0。

- [ ] **Step 3: コミット**

```bash
git add crates/kotoha-engine-reactor-linux Cargo.toml
git commit -m "feat(engine-reactor-linux): introduce LinuxReactor via crossbeam select! (ADR 0020)"
```

---

## Phase C: KotohaEngine refactor — `drain_events_blocking` 撤去 + worker_channel を crossbeam 化

`drain_events_blocking` を engine 側から完全に剥がし、`dispatch_rank_request` を send-only に縮める。`worker_channel.rs` の Drop ordering invariant(`tx_request` を最初に drop)は維持する。

### Task C1: `engine/event.rs` の `EngineEvent` を `Event` の re-export に置換

**Files:**
- Read: `crates/kotoha-engine-core/src/engine/event.rs` (現状確認)
- Modify: `crates/kotoha-engine-core/src/engine/event.rs`

- [ ] **Step 1: 現状を確認**

```bash
cat crates/kotoha-engine-core/src/engine/event.rs
```

- [ ] **Step 2: `EngineEvent` を `Event::WorkerOutput` への変換 helper に置換**

旧 `EngineEvent::Candidates { request_id, update }` と `EngineEvent::WorkerError { request_id, error }` の send 経路は worker → engine-loop だけが利用するため、本 task では:

- `EngineEvent` 型は内部互換のため一旦維持
- `From<EngineEvent> for crate::reactor::Event` の `impl` を追加し、worker 側から流す際に変換する

```rust
// engine/event.rs 末尾に追加
impl From<EngineEvent> for crate::reactor::Event {
    fn from(value: EngineEvent) -> Self {
        match value {
            EngineEvent::Candidates { request_id, update } => crate::reactor::Event::WorkerOutput {
                request_id,
                payload: crate::reactor::WorkerPayload::Candidates(update),
            },
            EngineEvent::WorkerError { request_id, error } => crate::reactor::Event::WorkerOutput {
                request_id,
                payload: crate::reactor::WorkerPayload::Error(error),
            },
        }
    }
}
```

- [ ] **Step 3: ビルド確認**

```bash
cargo build -p kotoha-engine-core
```
Expected: warning 0、error 0。

### Task C2: `worker_channel.rs` を crossbeam-channel に移行(Drop ordering 維持)

**Files:**
- Modify: `crates/kotoha-engine-core/src/engine/worker_channel.rs`

⚠️ **重要**: 現行 `worker_channel.rs:48-58` の field 宣言順は `tx_request` → `rx_event` → `handle` で **load-bearing**(`Drop` 完了後の field drop 順で worker thread が exit する)。本 Task では型のみ差し替え、宣言順を **絶対に変更しない**。

- [ ] **Step 1: import を crossbeam に切替**

`use std::sync::mpsc;` を削除し、以下に置換:

```rust
use crossbeam_channel::{Receiver, Sender, unbounded};
```

- [ ] **Step 2: `WorkerChannel` 内 field 型を変更**

```rust
pub(crate) struct WorkerChannel {
    // 注意: 本 struct の field 宣言順序は **load-bearing**(`Drop` semantics に直接影響)。
    // `tx_request` を必ず最初に宣言すること(crossbeam 化後も同じ理由で維持)。
    pub(crate) tx_request: Sender<RankRequest>,
    pub(crate) rx_event: Receiver<EngineEvent>,
    handle: Option<thread::JoinHandle<()>>,
}
```

- [ ] **Step 3: 内部 channel 構築コードを `unbounded()` 化**

旧 `mpsc::channel()` を `unbounded()` に置換。tx/rx ペアは crossbeam の戻り値に合わせる。

- [ ] **Step 4: ビルド + clippy 確認**

```bash
cargo build -p kotoha-engine-core --all-targets
cargo clippy -p kotoha-engine-core --all-targets -- -D warnings
```
Expected: pass。

### Task C3: `worker.rs` を crossbeam-channel signature に追従

**Files:**
- Modify: `crates/kotoha-engine-core/src/engine/worker.rs`

- [ ] **Step 1: worker thread 関数の signature を更新**

worker は内部で `EngineEvent::Candidates { ... }` を生成して `Sender<EngineEvent>` で送る。本 Task では `mpsc::Sender` → `crossbeam_channel::Sender` の差し替えのみ行う(挙動は同一)。

```rust
use crossbeam_channel::{Receiver, Sender};

pub(crate) fn run_worker_loop(
    rx_request: Receiver<RankRequest>,
    tx_event: Sender<EngineEvent>,
) {
    /* 既存ロジックを維持。recv/send 呼び出しは crossbeam の signature と互換 */
}
```

- [ ] **Step 2: ビルド確認**

```bash
cargo build -p kotoha-engine-core --all-targets
```
Expected: pass。

### Task C4: `dispatch_rank_request` を send-only 化、`drain_events_blocking` を削除

**Files:**
- Modify: `crates/kotoha-engine-core/src/engine/mod.rs:174` (dispatch_rank_request)
- Modify: `crates/kotoha-engine-core/src/engine/mod.rs:276` (drain_events_blocking 削除)
- Modify: `crates/kotoha-engine-core/src/engine/mod.rs:238` (drain 呼び出し箇所を削除)

- [ ] **Step 1: `dispatch_rank_request` から `drain_events_blocking` 呼び出しを除去**

現行 `engine/mod.rs:238` 付近の `self.drain_events_blocking(max_wait, request_id);` を削除する。`dispatch_rank_request` は worker への request 送信のみ行い、結果は engine-loop で `Event::WorkerOutput` として受け取る形に縮める。method の signature(`pub(crate) fn dispatch_rank_request(&mut self, mode: ConversionMode)`)と返り値は維持(callers の `transitions.rs:77 / 131 / 168 / 270` は無修正で済む)。

- [ ] **Step 2: `drain_events_blocking` method を削除**

`engine/mod.rs:276` 付近の `pub(crate) fn drain_events_blocking(&mut self, max_wait: Duration, target_id: u64)` を全削除。続く doc comment(`drain_events_blocking と同等の Idle 復帰処理を行う`)も refer 先消失のため修正する。

- [ ] **Step 3: `apply_candidate_update` メソッドを engine 公開 API に追加**

engine-loop が `Event::WorkerOutput` 受信時に呼び出す entry point。既存の drain 内部処理(候補 buffer 更新 + host_bridge.update_lookup_table 呼び出し)を本 method に移植する。

```rust
impl KotohaEngine {
    /// engine-loop が `Event::WorkerOutput` を受信した際に呼び出す entry point。
    ///
    /// # Preconditions
    /// - `request_id` は engine が `dispatch_rank_request` で発行した id
    ///
    /// # Postconditions
    /// - `request_id` が active_request と mismatch なら早期 return(stale 結果は破棄)
    /// - 候補 buffer が更新され、host_bridge への `update_lookup_table` が発火する
    pub fn apply_candidate_update(
        &mut self,
        request_id: u64,
        payload: WorkerPayload,
    ) -> Result<(), KotohaEngineError> {
        // 旧 drain_events_blocking 内部の dispatch logic をここに移植
    }
}
```

- [ ] **Step 4: ビルド + clippy 確認**

```bash
cargo build -p kotoha-engine-core --all-targets
cargo clippy -p kotoha-engine-core --all-targets -- -D warnings
```
Expected: pass。drain 撤去で参照が消えた既存 test は次 Task で修正する。

- [ ] **Step 5: 既存 test の compile 通過確認**

```bash
cargo test -p kotoha-engine-core --no-run
```
Expected: 全 test ファイルが build できる(`drain_events_blocking` 直接呼び出し test があれば error)。エラーが出た場合は関連 test を `apply_candidate_update` 経由に書き換える。

- [ ] **Step 6: コミット**

```bash
git add crates/kotoha-engine-core/src/engine
git commit -m "refactor(engine-core): drop drain_events_blocking, expose apply_candidate_update (B0h-f)"
```

---

## Phase D: kotoha-bin に engine-loop thread を追加

`KotohaEngine` を engine-loop thread が単独所有し、`EventReactor::recv()` で event を dispatch する loop を実装する。

### Task D1: `kotoha-bin` が crossbeam-channel + reactor crate を参照

**Files:**
- Modify: `crates/kotoha-bin/Cargo.toml`

- [ ] **Step 1: dependency 追加**

`[dependencies]` に追記:

```toml
crossbeam-channel = { workspace = true }
kotoha-engine-reactor-linux = { path = "../kotoha-engine-reactor-linux" }
```

- [ ] **Step 2: ビルド確認**

```bash
cargo build -p kotoha-bin
```
Expected: pass。

### Task D2: `kotoha-bin/src/engine_loop.rs` を新規作成

**Files:**
- Create: `crates/kotoha-bin/src/engine_loop.rs`

- [ ] **Step 1: ファイル作成**

```rust
//! engine-loop thread の main loop。
//!
//! ADR 0020 §採択 Q4 の 4-thread topology のうち `kotoha-engine-loop` thread を担う。
//! `KotohaEngine` を `move` で単独所有し、lock を一切使わずに状態を更新する。

use kotoha_engine_core::reactor::{Event, EventReactor, IBusResetKind, WorkerPayload};
use kotoha_engine_core::{IMEEngine, KotohaEngine};

/// engine-loop の main loop。本関数は engine-loop thread の `spawn` 時に呼ばれる。
///
/// # Preconditions
///
/// - `engine` は本関数が単独所有する
/// - `reactor` の `EventReactor::recv()` は engine-loop thread からのみ呼ばれる
///
/// # Postconditions
///
/// - `Event::Shutdown` を観測する、または全 `Sender<Event>` が drop されると
///   `Ok(())` で return する
///
/// # Errors
///
/// - `KotohaEngine` 内部 method からの `KotohaEngineError`(stop-the-world)
pub fn run<R: EventReactor>(
    mut engine: KotohaEngine,
    reactor: R,
) -> anyhow::Result<()> {
    loop {
        match reactor.recv() {
            Ok(Event::IBusKey(key)) => {
                let _ = engine.process_key_event(key);
            }
            Ok(Event::IBusReset(kind)) => match kind {
                IBusResetKind::FocusOut => engine.focus_out(),
                IBusResetKind::Reset | IBusResetKind::Disable => engine.reset(),
            },
            Ok(Event::WorkerOutput { request_id, payload }) => {
                engine.apply_candidate_update(request_id, payload)?;
            }
            Ok(Event::Shutdown) => {
                tracing::info!("engine-loop received Shutdown event, exiting");
                break Ok(());
            }
            Err(_) => {
                // 全 Sender drop = 自然な shutdown
                tracing::info!("engine-loop reactor disconnected, exiting");
                break Ok(());
            }
        }
    }
}
```

- [ ] **Step 2: `main.rs` から module 公開**

`crates/kotoha-bin/src/main.rs` の上部 `mod host_detect;` の隣に `mod engine_loop;` を追加。

- [ ] **Step 3: ビルド確認**

```bash
cargo build -p kotoha-bin --all-targets
cargo clippy -p kotoha-bin --all-targets -- -D warnings
```
Expected: pass。

- [ ] **Step 4: コミット**

```bash
git add crates/kotoha-bin
git commit -m "feat(bin): introduce engine-loop run() over EventReactor (ADR 0020)"
```

---

## Phase E: D-Bus listener thread (B3) + dispatcher 整理

`kotoha-engine-ibus` 側に listener module を追加し、dispatcher から `Arc<Mutex<dyn IMEEngine>>` を撤去する。`kotoha-bin::run_ibus()` を 4-thread topology に組み替え、`anyhow::bail!` を撤去する。

### Task E1: `kotoha-engine-ibus` が crossbeam-channel を参照

**Files:**
- Modify: `crates/kotoha-engine-ibus/Cargo.toml`

- [ ] **Step 1: dependency 追加**

```toml
crossbeam-channel = { workspace = true }
```

- [ ] **Step 2: ビルド確認**

```bash
cargo build -p kotoha-engine-ibus
```
Expected: pass。

### Task E2: `kotoha-engine-ibus/src/listener.rs` を新規作成

**Files:**
- Create: `crates/kotoha-engine-ibus/src/listener.rs`
- Modify: `crates/kotoha-engine-ibus/src/lib.rs`(`pub mod listener;`)

- [ ] **Step 1: listener thread function を作成**

```rust
//! D-Bus signal listener loop(Phase 3-B B3 本体)。
//!
//! ADR 0020 §採択 Q4 の `kotoha-dbus-listener` thread を担う。
//! `zbus::blocking::MessageStream` で IBus engine が受ける D-Bus method
//! (`ProcessKeyEvent` / `Reset` / `FocusOut` / `Disable`)を受信し、
//! `Event::IBusKey` / `Event::IBusReset` に decode して bridge channel に送る。

use crossbeam_channel::Sender;
use kotoha_engine_core::reactor::{Event, IBusResetKind};
use kotoha_engine_core::key_event::KeyEvent;

/// D-Bus listener thread の main loop。
///
/// # Preconditions
///
/// - `connection` は IBus session bus に接続済(B0g 既存 host_bridge.rs と同じ前提)
/// - `bridge_tx` は engine-loop thread が保持する `EventReactor` の bridge channel
///
/// # Postconditions
///
/// - `MessageStream` が end-of-stream に達するか、または `bridge_tx.send()` が
///   `Err(SendError)`(engine-loop が落ちた)を返したら `Ok(())` で return する
///
/// # Errors
///
/// - zbus 内部 protocol error は `anyhow::Error` で propagate(main thread の
///   join 時に集約される)
pub fn run(
    connection: zbus::blocking::Connection,
    bridge_tx: Sender<Event>,
) -> anyhow::Result<()> {
    let stream = zbus::blocking::MessageStream::from(&connection);
    for msg in stream {
        let msg = msg?;
        let header = msg.header();
        // IBus IBusEngineService method を decode する。member 名で分岐する。
        let Some(member) = header.member() else { continue; };
        let event_opt: Option<Event> = match member.as_str() {
            "ProcessKeyEvent" => Some(Event::IBusKey(decode_process_key_event(&msg)?)),
            "Reset" => Some(Event::IBusReset(IBusResetKind::Reset)),
            "FocusOut" => Some(Event::IBusReset(IBusResetKind::FocusOut)),
            "Disable" => Some(Event::IBusReset(IBusResetKind::Disable)),
            _ => None,
        };
        if let Some(ev) = event_opt {
            if bridge_tx.send(ev).is_err() {
                tracing::info!("engine-loop disconnected, dbus-listener exiting");
                break;
            }
        }
    }
    Ok(())
}

/// `ProcessKeyEvent(u keyval, u keycode, u state) -> b` を decode する。
fn decode_process_key_event(msg: &zbus::Message) -> anyhow::Result<KeyEvent> {
    let body = msg.body();
    let (keyval, keycode, state): (u32, u32, u32) = body.deserialize()?;
    Ok(KeyEvent::from_ibus_triple(keyval, keycode, state))
}
```

注: `KeyEvent::from_ibus_triple` は engine-core 既存 helper(`crates/kotoha-engine-core/src/key_event.rs`)を利用する。存在しない場合は本 Task に同 helper を追加する step を挿入する。

- [ ] **Step 2: `lib.rs` から module 公開**

`crates/kotoha-engine-ibus/src/lib.rs` に以下を追記:

```rust
pub mod listener;
```

- [ ] **Step 3: ビルド + clippy 確認**

```bash
cargo build -p kotoha-engine-ibus --all-targets
cargo clippy -p kotoha-engine-ibus --all-targets -- -D warnings
```
Expected: pass。

### Task E3: `dispatcher.rs` から `Arc<Mutex<dyn IMEEngine>>` を撤去

**Files:**
- Modify: `crates/kotoha-engine-ibus/src/dispatcher.rs:26-32`

- [ ] **Step 1: `IBusEventDispatcher` の Mutex 参照を削除**

`Arc<Mutex<dyn IMEEngine>>` を保持する責務は engine-loop に移ったため、dispatcher は host-side helper(IBus method の signature 検証等)に縮小する。**完全削除でも可**。本 task では:

- 削除しても compile が通る場合(B5 integration test など他参照がない場合)、`dispatcher.rs` ファイル全体を削除し `lib.rs` から `pub mod dispatcher;` を除く
- 残す価値がある場合は `Arc<Mutex<dyn IMEEngine>>` を持たない最小構造体にダウングレードする

- [ ] **Step 2: 影響箇所を確認**

```bash
rg "IBusEventDispatcher" crates/ tests/
```
- 残参照箇所をすべて grep で列挙
- 各箇所を listener / engine_loop 経路に置換

- [ ] **Step 3: ビルド確認**

```bash
cargo build --workspace --all-targets
```
Expected: pass。compile error が出たら参照箇所をすべて修正してから次 step へ。

### Task E4: `kotoha-bin/src/main.rs::run_ibus()` を 4-thread topology に組み替え

**Files:**
- Modify: `crates/kotoha-bin/src/main.rs:186-290`

- [ ] **Step 1: 既存 `_dispatcher` + `anyhow::bail!` を削除**

現行 `main.rs:276-289` の以下ブロックを丸ごと削除:

```rust
let engine_shared: std::sync::Arc<std::sync::Mutex<dyn kotoha_engine_core::IMEEngine>> =
    std::sync::Arc::new(std::sync::Mutex::new(engine));
let _dispatcher = kotoha_engine_ibus::IBusEventDispatcher::new(engine_shared);

tracing::error!(
    ranker_backend,
    host_bridge_backend,
    "kotoha engine wired up but the IBus event loop is not yet implemented (Phase 3-B B3); \
     refusing to silently exit"
);
anyhow::bail!(
    "IBus event loop not yet implemented (tracked in Phase 3-B B3 / ISSUE #146); \
     kotoha-bin cannot serve as an IME yet"
);
```

- [ ] **Step 2: ReactorHandles + thread spawn + join に置換**

```rust
use kotoha_engine_reactor_linux::{LinuxReactor, ReactorHandles};

let ReactorHandles { reactor, bridge_tx, worker_tx, shutdown_tx } = LinuxReactor::new();

// engine への worker_tx 注入(KotohaEngine::new シグネチャに合わせて加える)
// 既存の `KotohaEngine::new(host_bridge, ranker, learning_recorder)` に
// 第 4 引数として `worker_tx: Sender<Event>` を追加する(別 Task で engine-core 側に
// signature 変更を反映)。

let connection = host_bridge.connection().clone();  // listener と main で session bus を共有
let listener_handle = std::thread::Builder::new()
    .name("kotoha-dbus-listener".into())
    .spawn(move || kotoha_engine_ibus::listener::run(connection, bridge_tx))
    .context("spawn dbus-listener thread")?;

let engine_handle = std::thread::Builder::new()
    .name("kotoha-engine-loop".into())
    .spawn(move || engine_loop::run(engine, reactor))
    .context("spawn engine-loop thread")?;

install_signal_handler(shutdown_tx);

let engine_result = engine_handle.join().map_err(panic_to_anyhow)?;
let listener_result = listener_handle.join().map_err(panic_to_anyhow)?;
engine_result?;
listener_result?;

Ok(())
```

`install_signal_handler` / `panic_to_anyhow` ヘルパは本 Task で同 main.rs 末尾に追加する:

```rust
fn install_signal_handler(shutdown_tx: crossbeam_channel::Sender<()>) {
    // SIGTERM / SIGINT で shutdown_tx を 1 回 send する。signal-hook crate を
    // 使うか、Phase 3-B 時点で簡易実装(Ctrl+C 1 回押下を ignore する場合は
    // 二度押し対応も spec §9.3 fail-loud に従い検討)。本 Task では:
    //   - signal-hook を使う場合: dependency 追加 + std::thread::spawn で iterator 監視
    //   - 暫定: Ctrl+C trap として `ctrlc::set_handler` を使う(`ctrlc = "3"` 追加)
    let _ = ctrlc::set_handler(move || {
        let _ = shutdown_tx.try_send(());
    });
}

fn panic_to_anyhow(payload: Box<dyn std::any::Any + Send>) -> anyhow::Error {
    let msg = kotoha_engine_core::sanitize::panic_message_from(&payload);
    anyhow::anyhow!("thread panicked: {msg}")
}
```

注: `ctrlc` crate 追加が必要。`crates/kotoha-bin/Cargo.toml` に `ctrlc = "3"` を追加(既存 dependency 整理は別 ISSUE で扱う)。

- [ ] **Step 3: KotohaEngine の signature 変更を engine-core 側にも反映**

`crates/kotoha-engine-core/src/engine/mod.rs` の `KotohaEngine::new` 宣言を以下に変更:

```rust
pub fn new(
    host_bridge: Box<dyn IMEHostBridge>,
    ranker: Arc<dyn Ranker>,
    learning_recorder: Arc<dyn LearningRecorder>,
    worker_event_tx: crossbeam_channel::Sender<Event>,
) -> Result<Self, std::io::Error> { /* ... */ }
```

worker thread 内部の `EngineEvent` 送信先は本 `worker_event_tx` 経由で `Event::WorkerOutput` を送るように内部で変換する。

- [ ] **Step 4: ビルド + clippy + test 確認**

```bash
cargo fmt --all
cargo build --workspace --all-targets
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --no-run
```
Expected: 全部 pass(test 実行は次 Phase F で行う)。

- [ ] **Step 5: コミット**

```bash
git add crates/kotoha-engine-ibus crates/kotoha-bin crates/kotoha-engine-core
git commit -m "feat(bin,ibus): wire 4-thread topology with dbus-listener + engine-loop (B3 + ADR 0020)"
```

---

## Phase F: Test-After — spec acceptance criteria 由来の test を追加

global rule §Development Flow に従い、impl 完了後に spec §6 / §10 / §13 acceptance criteria から test を派生させる。

### Task F1: `MockReactor` を engine-core test infra に追加

**Files:**
- Create: `crates/kotoha-engine-core/src/reactor/mock.rs`(Phase A Task A4 で `mod mock;` 宣言済)

- [ ] **Step 1: file 作成**

```rust
//! `MockReactor` — engine-loop unit test 用の `EventReactor` 実装。
//!
//! 旧 PR #113 の `MockLearningCacheStore` 流儀(feedback_mock_owns_invariants.md)に
//! 従い、SQLite / zbus / OS primitive に一切依存しない。

use std::collections::VecDeque;
use std::sync::Mutex;
use std::time::Duration;

use crate::reactor::{Event, EventReactor};
use crossbeam_channel::{RecvError, RecvTimeoutError};

/// queue ベースの mock。`push` で event を投入、`recv` で 1 件取り出す。
///
/// # Invariants
///
/// - queue が空 + `closed = true` ⇒ `recv` は `Err(RecvError)` を返す
/// - `recv_timeout` は queue 空のときに timeout を待たず即時 `Timeout` を返す
///   (deterministic test のため、real reactor とは挙動が異なる)
pub struct MockReactor {
    inner: Mutex<MockState>,
}

struct MockState {
    queue: VecDeque<Event>,
    closed: bool,
}

impl MockReactor {
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(MockState {
                queue: VecDeque::new(),
                closed: false,
            }),
        }
    }

    pub fn push(&self, ev: Event) {
        let mut s = self.inner.lock().expect("MockReactor poisoned");
        s.queue.push_back(ev);
    }

    /// 全 producer が drop された状態を simulate する。
    pub fn close(&self) {
        let mut s = self.inner.lock().expect("MockReactor poisoned");
        s.closed = true;
    }
}

impl Default for MockReactor {
    fn default() -> Self {
        Self::new()
    }
}

impl EventReactor for MockReactor {
    fn recv(&self) -> Result<Event, RecvError> {
        loop {
            let mut s = self.inner.lock().expect("MockReactor poisoned");
            if let Some(ev) = s.queue.pop_front() {
                return Ok(ev);
            }
            if s.closed {
                return Err(RecvError);
            }
            // test では blocking すべきでないため、closed でない限りは spin
            // 想定外の usage を防ぐため、debug_assert で警告
            debug_assert!(false, "MockReactor::recv called on empty queue without close()");
            return Err(RecvError);
        }
    }

    fn recv_timeout(&self, _timeout: Duration) -> Result<Event, RecvTimeoutError> {
        let mut s = self.inner.lock().expect("MockReactor poisoned");
        if let Some(ev) = s.queue.pop_front() {
            Ok(ev)
        } else if s.closed {
            Err(RecvTimeoutError::Disconnected)
        } else {
            Err(RecvTimeoutError::Timeout)
        }
    }
}
```

- [ ] **Step 2: `engine-core/src/reactor/mod.rs` で feature gate を確認**

Phase A Task A4 で `#[cfg(any(test, feature = "test-helpers"))] pub mod mock;` を既に宣言済。再確認のみ。

- [ ] **Step 3: ビルド確認**

```bash
cargo build -p kotoha-engine-core --features test-helpers --all-targets
```
Expected: pass。

### Task F2: 既存 test の修正(`drain_events_blocking` 撤去対応)

**Files:**
- Modify: `crates/kotoha-engine-core/tests/worker_coalescing.rs`(`drain_events_blocking` 直呼び test)
- Modify: `crates/kotoha-engine-core/tests/state_transitions.rs`(必要に応じて)

- [ ] **Step 1: 影響 test を grep**

```bash
rg "drain_events_blocking" crates/kotoha-engine-core/tests/
```

- [ ] **Step 2: 各 test を `apply_candidate_update` 直呼び形式に書き換え**

旧 test では engine 内部で worker → drain で event を処理していた箇所を、test 側で worker からの `EngineEvent`(または `Event::WorkerOutput`)を取り出して `engine.apply_candidate_update(request_id, payload)` を呼ぶ形に変更する。

- [ ] **Step 3: 既存 baseline 数 (536 / 547) を維持できる test 数で完了**

```bash
cargo test -p kotoha-engine-core
cargo test -p kotoha-engine-core --features test-helpers
```
Expected: 全 pass、ベースライン 0 regression。

### Task F3: `kotoha-engine-reactor-linux` integration test

**Files:**
- Create: `crates/kotoha-engine-reactor-linux/tests/integration.rs`

- [ ] **Step 1: shutdown ordering test**

```rust
//! ADR 0020 §影響 「shutdown ordering」 acceptance criteria に対応。
//! Spec: docs/specs/_uncategorized/p3-a-ibus-engine.md §9.3 fail-loud
//!
//! 4-thread topology の shutdown が 100ms 以内に正しく終了することを verify。

use std::time::{Duration, Instant};
use kotoha_engine_core::reactor::{Event, EventReactor, IBusResetKind};
use kotoha_engine_reactor_linux::{LinuxReactor, ReactorHandles};

#[test]
fn shutdown_signal_terminates_recv_loop() {
    let ReactorHandles { reactor, bridge_tx: _, worker_tx: _, shutdown_tx } = LinuxReactor::new();
    let start = Instant::now();
    let handle = std::thread::spawn(move || {
        loop {
            match reactor.recv() {
                Ok(Event::Shutdown) | Err(_) => break,
                Ok(_) => continue,
            }
        }
    });
    drop(shutdown_tx);  // Sender drop で shutdown_rx が close
    handle.join().unwrap();
    assert!(start.elapsed() < Duration::from_millis(100));
}

#[test]
fn fan_in_preserves_arrival_order_within_single_channel() {
    let ReactorHandles { reactor, bridge_tx, worker_tx: _, shutdown_tx: _ } = LinuxReactor::new();
    bridge_tx.send(Event::IBusReset(IBusResetKind::FocusOut)).unwrap();
    bridge_tx.send(Event::IBusReset(IBusResetKind::Reset)).unwrap();
    let first = reactor.recv().unwrap();
    let second = reactor.recv().unwrap();
    assert!(matches!(first, Event::IBusReset(IBusResetKind::FocusOut)));
    assert!(matches!(second, Event::IBusReset(IBusResetKind::Reset)));
}

#[test]
fn recv_timeout_returns_timeout_when_quiet() {
    let ReactorHandles { reactor, bridge_tx: _, worker_tx: _, shutdown_tx: _ } = LinuxReactor::new();
    let res = reactor.recv_timeout(Duration::from_millis(10));
    assert!(matches!(res, Err(crossbeam_channel::RecvTimeoutError::Timeout)));
}
```

- [ ] **Step 2: テスト実行**

```bash
cargo test -p kotoha-engine-reactor-linux
```
Expected: 3 tests passed。

### Task F4: バーストイベント / panic 伝搬 test(統合)

**Files:**
- Add to: `crates/kotoha-engine-reactor-linux/tests/integration.rs`

- [ ] **Step 1: bursty event test を追加**

```rust
#[test]
fn bursty_events_preserve_per_sender_order() {
    let ReactorHandles { reactor, bridge_tx, worker_tx, shutdown_tx: _ } = LinuxReactor::new();
    let bridge_clone = bridge_tx.clone();
    let worker_clone = worker_tx.clone();
    let producer = std::thread::spawn(move || {
        for i in 0..500 {
            bridge_clone.send(Event::IBusReset(IBusResetKind::Reset)).unwrap();
            worker_clone.send(Event::WorkerOutput {
                request_id: i as u64,
                payload: kotoha_engine_core::reactor::WorkerPayload::Error("test".into()),
            }).unwrap();
        }
    });
    let mut count = 0;
    while count < 1000 {
        match reactor.recv_timeout(Duration::from_millis(100)) {
            Ok(_) => count += 1,
            Err(_) => break,
        }
    }
    producer.join().unwrap();
    assert_eq!(count, 1000);
}
```

- [ ] **Step 2: テスト実行**

```bash
cargo test -p kotoha-engine-reactor-linux
```
Expected: 4 tests passed。

- [ ] **Step 3: コミット**

```bash
git add crates/kotoha-engine-core/src/reactor/mock.rs crates/kotoha-engine-reactor-linux/tests crates/kotoha-engine-core/tests
git commit -m "test(reactor): add MockReactor + LinuxReactor integration tests"
```

---

## Phase G: 最終検証 + PR 作成

global rule §Development Flow §6「Verify via lefthook pre-push」に従い、全 quality gate を pass させてから PR 作成へ進む。

### Task G1: フォーマット + lint

- [ ] **Step 1: cargo fmt**

```bash
cargo fmt --all
git diff --name-only | head
```
Expected: フォーマット差分 0(diff なし)。差分があればここで commit:

```bash
git add -u
git commit -m "style: cargo fmt --all"
```

- [ ] **Step 2: clippy 警告ゼロ**

```bash
cargo clippy --workspace --all-targets -- -D warnings
cargo clippy --workspace --all-targets --features kotoha-storage/test-helpers,kotoha-engine-core/test-helpers -- -D warnings
```
Expected: warning 0、error 0。

### Task G2: 全 test pass(0 regression)

- [ ] **Step 1: default features**

```bash
cargo test --workspace 2>&1 | tail -20
```
Expected: 既存 baseline 536 PASS 以上(reactor 整合 test の追加で増加見込み)、0 FAIL。

- [ ] **Step 2: test-helpers features**

```bash
cargo test --workspace --features kotoha-storage/test-helpers,kotoha-engine-core/test-helpers 2>&1 | tail -20
```
Expected: 547 PASS 以上、0 FAIL。

- [ ] **Step 3: Phase 1 14/15 baseline 確認**

```bash
cargo test --workspace --features kotoha-storage/test-helpers,kotoha-engine-core/test-helpers 2>&1 | rg "phase1|layer3|14/15"
```
spec §13 受容基準「Phase 1 14/15 regression が engine 経由でも保たれる」が満たされること。

### Task G3: stub symbol release default 0 link を再確認

ADR 0020 影響範囲外だが、B0h-e で確立された invariant を維持する:

- [ ] **Step 1: release build で stub symbol が 0 件であることを確認**

```bash
cargo build --release -p kotoha-bin
nm target/release/kotoha 2>/dev/null | rg -i "stubranker|stubhostbridge" | head
```
Expected: 何も出力されない(0 hit)。

### Task G4: lefthook pre-push を手動 trigger

- [ ] **Step 1: pre-push hook 全体を実行**

```bash
lefthook run pre-push --all-files 2>&1 | tail -40
```
Expected: 全 step pass、exit 0。

### Task G5: PR 作成

- [ ] **Step 1: branch を origin に push**

```bash
git push -u origin feature/149-p3b-b0hf-b3-event-loop
```

- [ ] **Step 2: PR 作成**

```bash
gh pr create --base develop --title "P3-B B0h-f + B3: event-loop architecture (ADR 0020)" --body "$(cat <<'EOF'
## Summary

Unify B0h-f (I3 dispatch async-ification) and B3 (D-Bus signal listener loop) into a single architectural rework, per ADR 0020.

## What changes

- New crate `kotoha-engine-reactor-linux` providing `LinuxReactor` (crossbeam-channel `select!`-based)
- New module `kotoha-engine-core::reactor` with `EventReactor` trait + `Event` sum type (`#[non_exhaustive]`)
- Removed `KotohaEngine::drain_events_blocking` and `Arc<Mutex<dyn IMEEngine>>` (B0h-d superseded)
- 4-thread lock-free topology in `kotoha-bin::run_ibus()`: `main` / `kotoha-dbus-listener` / `kotoha-engine-loop` / `kotoha-ranker-worker`
- New `kotoha-engine-ibus::listener::run` (B3 zbus blocking::MessageStream loop)
- ADR 0017 bumped to rev2 (crossbeam-channel adoption)
- Spec `p3-a-ibus-engine.md` r3 (§6.0 / §7 / §13 / §14.1 updates)
- ROADMAP entry merged into B0h-f + B3 unified row

## Test plan

- [ ] `cargo fmt --all` clean
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` clean
- [ ] `cargo test --workspace` 0 regression vs baseline 536
- [ ] `cargo test --workspace --features test-helpers` 0 regression vs baseline 547
- [ ] Phase 1 14/15 layer-3 smoke preserved
- [ ] release build stub symbol 0 link
- [ ] lefthook pre-push pass

Closes #149 (B0h-f portion)
Refs #136 (B3 portion; B6 manual smoke remains)
EOF
)"
```

- [ ] **Step 3: PR セルフレビュー(global rule §PR Review Matrix Large 適用)**

PR 規模(~920 lines / 13-15 files)から **Large tier** に該当する。以下を順次実行:

```bash
# 各 review skill を 1 セッションで連続実行(並列可)
# - team-review (5 dim) + secrets-check
# - owasp-security
# - security-scanning:security-sast
# - pr-review-toolkit:review-pr (large tier add-on)
# - audit (pre-release add-on)
```

各 finding を本 PR 内で resolve(global rule §PR Review Matrix Mandatory rules)。

- [ ] **Step 4: PR merge 後の post-merge follow-up**

global CLAUDE.md §post-merge follow-up checklist に従い:
- spec status を `draft` から `implemented` へ昇格(Phase 3-A spec が B0h-f + B3 完了で実装完了状態に達するか判定。残 B6 manual smoke が未実施なら `draft` 維持で別 PR 待ち)
- glossary sync(本 PR で `event-reactor` / `event-loop` / `fan-in` 追加済、再確認)
- ROADMAP の B0h-f + B3 entry を `[x]` に flip(B6 残のため milestone 完了は not yet)
- vault sync 確認(`/check-direnv` 実行)
- milestone 完了判定: B6 残のため v0.3.0 milestone は未完了

---

## Self-Review (writing-plans skill §Self-Review に従い実施)

### 1. Spec coverage

- ADR 0020 §決定 §構成要素 → Phase A (foundation) + Phase B (reactor crate)
- ADR 0020 §決定 §削除する API/type → Phase C
- ADR 0020 §決定 §影響範囲 → Phase D + Phase E + Phase F
- spec p3-a-ibus-engine.md §6.0 4-thread topology → Phase E Task E4
- spec §7.1 `Event` sum 型 → Phase A Task A3
- spec §7.2 worker thread モデル(rev3)→ Phase C Task C2 / C3
- spec §7.4 worker_loop 擬似 code → Phase C Task C3(crossbeam Sender 化)
- spec §13 #10 / #11 closure → Phase C Task C4(drain 撤去) + Phase E Task E3(Mutex 撤去)
- spec §10 testing strategy → Phase F 全体
- spec §13 acceptance criteria(Phase 1 14/15)→ Task G2 Step 3

✓ 全 acceptance criteria に対応 task あり

### 2. Placeholder scan

- "TBD" / "TODO" / "implement later" / "fill in details" → grep で 0 hit を確認

### 3. Type consistency

- `Event` enum variant: `IBusKey(KeyEvent)` / `IBusReset(IBusResetKind)` / `WorkerOutput { request_id, payload }` / `Shutdown` を Phase A / Phase D / Phase E で一貫使用 ✓
- `WorkerPayload`: `Candidates(CandidateUpdate)` / `Error(String)` を Phase A / Phase C / Phase F で一貫使用 ✓
- `EventReactor::recv` / `recv_timeout` の signature を Phase A / Phase B / Phase D / Phase F mock で一致 ✓
- `ReactorHandles { reactor, bridge_tx, worker_tx, shutdown_tx }` を Phase B / Phase E で同一 destructure ✓

### 4. 既存 invariant 保護

- `worker_channel.rs` の field 宣言順 load-bearing(`tx_request` 最初)→ Phase C Task C2 で明示警告 ✓
- ADR 0017 「core は tokio 非依存」原則 → 全 Phase で std + crossbeam-channel のみ、tokio 不導入 ✓
- B0h-e stub symbol 0 link release default → Task G3 で再確認 ✓
- `panic_message_from` helper(B0g-c で promote)→ Phase E Task E4 で `panic_to_anyhow` 内に再利用 ✓

問題なしと判断。
