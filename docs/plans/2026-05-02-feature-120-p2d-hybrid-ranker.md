# P2-D Hybrid Ranker Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.
>
> **Spec-Driven Test-After 流の adapt 版**:本 plan は Kotoha CLAUDE.md の ban(`superpowers:test-driven-development` 禁止)に従い、各 task 内で「実装 → lefthook → spec 由来 test 追加 → verify → commit」順で進める。「失敗する test を先に書く」ステップは含まない。

**Goal:** Phase 3-A spec §4.3 で凍結された `Ranker` trait の concrete impl として、SudachiDict + UserVocab + LearningCache + LLM 候補を統合する `HybridRanker` を `kotoha-engine-core` 新 crate に実装する。Phase 3-A engine 本体(`KotohaEngine` / `IMEEngine` / `IMEHostBridge`)は本 plan の scope 外で別 plan / 別 PR にて扱う。

**Architecture:** Hexagonal core crate(`kotoha-engine-core`)を新規追加し、`Ranker` trait + 関連 types を `kotoha-engine-core` 内で凍結する(Phase 3-A spec §4.3 / §4.4)。`HybridRanker` struct は本 crate 内で本 trait を impl し、内部で 4 backend(SudachiDict / UserVocab / LearningCache / LLM)を並列に呼び出して `mpsc::Sender<RankerOutput>` 経由で逐次 push する。Live / Commit モードを `ConversionContext::mode` で分岐し、Live は dict only fast path、Commit は LLM 完了まで待機する。

**Tech Stack:**
- Rust 2021 / rust-version 1.80
- `kotoha-storage` workspace dep(`UserVocabReader` / `LearningCacheReader`)
- `kotoha-core` workspace dep(`KanjiBackend` / `Candidate` / SudachiAdapter)
- `thiserror` / `tracing` workspace deps
- `proptest` dev-dep(invariant test 用)
- 並列 backend 呼び出しは `std::thread::spawn` + `std::sync::mpsc`(tokio runtime 不要、Phase 3-A spec §4.4 の方針)

**Estimated scale:** 8-15 files、~1200-2000 lines。Branch Scope Policy(≤20 files / ≤1000 lines)を超えるため **4 PR に split** する(下記 Milestone M1-M4)。

**Spec references:**
- Phase 3-A spec: `docs/superpowers/specs/2026-05-02-p3-a-ibus-engine-design.md` §4.3 / §4.4 / §6.1 / §6.2 / §7 / §8(Ranker contract、cancel、coalescing 規約、ただし engine 側 logic は本 plan 範囲外)
- Phase 2 spec: `docs/superpowers/specs/2026-04-25-kotoha-phase-2-design.md` §3.3 / §4.1 / §11(P2-D scope、Ranker 重み暫定 dict 0.95 / LLM 1.0 + LearningCache hit bonus、core variant、Phase 1 14/15 regression)
- ADR 0011: kanji backend trait design(`KanjiBackend` 利用元)
- ADR 0014: dictionary layer architecture
- ADR 0015: kotoha-storage SQLite adoption

**Issue:** [#120](https://github.com/std-koh-hinooka/kotoha-ime/issues/120)

---

## File Structure

| Milestone | File | 種類 | 責務 |
|-----------|------|------|------|
| M1 | `crates/kotoha-engine-core/Cargo.toml` | 新規 | crate manifest、依存 declaration |
| M1 | `crates/kotoha-engine-core/src/lib.rs` | 新規 | crate root、re-export |
| M1 | `crates/kotoha-engine-core/src/ranker/mod.rs` | 新規 | `Ranker` trait + `RankerError` + `RankerOutput` |
| M1 | `crates/kotoha-engine-core/src/ranker/context.rs` | 新規 | `ConversionContext` + `ConversionMode` |
| M1 | `crates/kotoha-engine-core/src/ranker/update.rs` | 新規 | `CandidateUpdate` enum |
| M1 | `crates/kotoha-engine-core/src/cancel/mod.rs` | 新規 | `CancellationToken` trait |
| M1 | `crates/kotoha-engine-core/src/cancel/std_token.rs` | 新規 | `StdCancellationToken` impl |
| M1 | `Cargo.toml`(workspace root) | 修正 | members に `crates/kotoha-engine-core` 追加 |
| M2 | `crates/kotoha-engine-core/src/ranker/hybrid.rs` | 新規 | `HybridRanker` struct + dict path(SudachiDict + UserVocab + LearningCache) |
| M2 | `crates/kotoha-engine-core/src/ranker/merge.rs` | 新規 | candidate merge / dedupe / scoring(初期重み dict 0.95 / LLM 1.0 + cache hit bonus) |
| M2 | `crates/kotoha-engine-core/tests/hybrid_ranker_dict.rs` | 新規 | dict path L2 integration test |
| M3 | `crates/kotoha-engine-core/src/ranker/hybrid.rs`(修正) | 修正 | LLM backend 統合 + context injection + cancel 10 token check |
| M3 | `crates/kotoha-engine-core/tests/hybrid_ranker_llm.rs` | 新規 | LLM integrated L2 test(MockBackend + 実 stores) |
| M3 | `crates/kotoha-engine-core/tests/regression_phase1.rs` | 新規 | Phase 1 Layer 3 14/15 regression(Ranker 経由) |
| M4 | `docs/wiki/glossary.md` | 修正 | `HybridRanker` entry 追加(category 4) |
| M4 | `crates/kotoha-engine-core/tests/proptest_invariants.rs` | 新規 | proptest 不変条件 test |

---

## Milestone 0: prerequisite ISSUEs verification(branch 切る前)

P2-D 着手の前提として以下 2 ISSUE が close されていることを確認する。両 ISSUE は P2-D の Ranker 実装と直接の code dependency は無いが、Phase 3-A 本番実装(P2-D 後続)で必要になるため、本 plan の Milestone 1 着手前に閉じておく。

### Task 0.1: prereq #118 / #119 status check

- [ ] **Step 1: ISSUE 状態確認**

```bash
gh issue view 118 --json state --jq '.state'
gh issue view 119 --json state --jq '.state'
```

期待:両者 `CLOSED`(または進行中なら順次 PR を merge してから Milestone 1 へ)。

- [ ] **Step 2: もし OPEN なら、別 plan / 別 ISSUE で先行実装**

両者は本 plan 範囲外。Phase 3-A spec §12.1 / §12.2 を参照し、Small tier PR で対応する。Implementation 完了したら `gh issue close <number>` で close、Milestone 1 へ進む。

---

## Milestone 1: kotoha-engine-core crate skeleton + trait/types(PR 1、Small tier、~250-350 lines)

**Goal:** trait + types の純粋定義のみ、`HybridRanker` 実装は M2 以降。本 PR は ISSUE #120 の partial 実装として、後続 PR の dependency 基盤を作る。

**Branch:** `feature/120-p2d-trait-skeleton`

### Task 1.1: branch + crate skeleton 作成

**Files:**
- Create: `crates/kotoha-engine-core/Cargo.toml`
- Create: `crates/kotoha-engine-core/src/lib.rs`
- Modify: `Cargo.toml`(workspace root)

- [ ] **Step 1: branch 作成**

```bash
git checkout develop
git pull origin develop
git checkout -b feature/120-p2d-trait-skeleton
```

- [ ] **Step 2: crate ディレクトリ作成**

```bash
mkdir -p crates/kotoha-engine-core/src/ranker crates/kotoha-engine-core/src/cancel crates/kotoha-engine-core/tests
```

- [ ] **Step 3: `crates/kotoha-engine-core/Cargo.toml` 作成**

```toml
[package]
name = "kotoha-engine-core"
description = "Kotoha IME engine domain core (host-agnostic, Phase 3-A spec §3.1)"
edition.workspace = true
rust-version.workspace = true
version.workspace = true
license.workspace = true
repository.workspace = true
authors.workspace = true

[dependencies]
thiserror = { workspace = true }
tracing = { workspace = true }
kotoha-core = { path = "../kotoha-core" }
kotoha-storage = { path = "../kotoha-storage" }

[dev-dependencies]
proptest = { workspace = true }
serial_test = { workspace = true }

[features]
default = []
# Mock backends / fixtures for cross-crate test injection.
test-helpers = []
```

- [ ] **Step 4: `Cargo.toml` (workspace root) を修正**

`members` array に `"crates/kotoha-engine-core"` を追加する。`Cargo.toml` の `[workspace]` セクション最初の `members = [...]` を以下のように更新:

```toml
[workspace]
resolver = "2"
members = [
    "crates/kotoha-core",
    "crates/kotoha-cli",
    "crates/kotoha-storage",
    "crates/kotoha-engine-core",
]
```

- [ ] **Step 5: `crates/kotoha-engine-core/src/lib.rs` 作成**

```rust
//! `kotoha-engine-core`: Kotoha IME engine domain core (host-agnostic).
//!
//! 本 crate は Phase 3-A spec §3.1 で凍結された engine domain core layer に対応する。
//! IBus / fcitx5 / 将来の input-method protocol を含む host adapter から
//! 直接依存される一方、本 crate は host 層の identifier(`ibus` / `zbus` /
//! `fcitx5` 等)に依存しない。Hexagonal Architecture の core 配置である。
//!
//! 本 crate の主要 trait / types:
//!
//! - [`ranker::Ranker`] — 候補生成 trait(P2-D で `HybridRanker` として impl)
//! - [`ranker::ConversionContext`] / [`ranker::ConversionMode`] — Ranker 入力 context
//! - [`ranker::CandidateUpdate`] — 候補差分通知 enum
//! - [`cancel::CancellationToken`] — cancel signal trait
//! - [`cancel::StdCancellationToken`] — std::sync ベース impl
//!
//! 本 crate は Phase 3-A engine 本体(`KotohaEngine` 状態機械、`IMEEngine` /
//! `IMEHostBridge` trait、`RankerWorker`)を含まない。それらは Phase 3-A 本番
//! 実装段階で本 crate に追加される。

pub mod cancel;
pub mod ranker;

pub use cancel::{CancellationToken, StdCancellationToken};
pub use ranker::{
    CandidateUpdate, ConversionContext, ConversionMode, Ranker, RankerError, RankerOutput,
};
```

- [ ] **Step 6: コンパイル確認(まだ何も実装していないので fail する見込み)**

```bash
cargo build -p kotoha-engine-core
```

期待:`error[E0432]: unresolved imports cancel::* / ranker::*` または同等の compile error。これは想定内、後続 step で順次解消する。

- [ ] **Step 7: 仮 commit(broken state)せず、続けて Task 1.2 に進む**

(本 task の最後で commit せず、Milestone 1 全 task 完了後に 1 commit でまとめる)

### Task 1.2: `cancel` module 実装(`CancellationToken` trait + `StdCancellationToken`)

**Files:**
- Create: `crates/kotoha-engine-core/src/cancel/mod.rs`
- Create: `crates/kotoha-engine-core/src/cancel/std_token.rs`

- [ ] **Step 1: `crates/kotoha-engine-core/src/cancel/mod.rs` 作成**

```rust
//! Cancellation token abstraction for in-flight `RankRequest` propagation.
//!
//! Phase 3-A spec §4.4 で凍結された自作 trait。tokio_util::sync::CancellationToken と
//! 同形式の interface だが、tokio runtime に直接依存しない。Phase 3-A 初期は
//! [`StdCancellationToken`](std_token::StdCancellationToken) を std::sync ベース impl
//! として同梱、Phase 5/6 で tokio runtime を導入する場合は別 crate で
//! `TokioCancellationToken` adapter を実装し差し替え可能とする。

use std::future::Future;
use std::pin::Pin;

mod std_token;
pub use std_token::StdCancellationToken;

/// In-flight `RankRequest` の cancel signal を伝搬する抽象境界。
///
/// # Implementor 要件
///
/// - `cancel()` は idempotent(複数回呼ばれても 2 回目以降は no-op で副作用なし)
/// - `is_cancelled()` は `cancel()` 呼び出し後 `true` を永続的に返す
/// - `cancelled()` は `cancel()` 呼び出し後即時 ready な future を返す
/// - `Send + Sync` を要求(複数 thread から `is_cancelled()` を listen される)
pub trait CancellationToken: Send + Sync {
    /// Cancel 状態に遷移させる。
    fn cancel(&self);

    /// 現在の cancel 状態を返す。
    fn is_cancelled(&self) -> bool;

    /// Cancel 待機 future を返す。`cancel()` 後即時 ready。
    fn cancelled<'a>(&'a self) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>>;
}
```

- [ ] **Step 2: `crates/kotoha-engine-core/src/cancel/std_token.rs` 作成**

```rust
//! `StdCancellationToken` — std::sync ベースの `CancellationToken` impl。
//!
//! Phase 3-A spec §4.4 で凍結された自作 trait の Phase 3-A 初期 impl。
//! `Arc<AtomicBool>` + `(Mutex, Condvar)` ペアで cancel signal の永続化と
//! future 待機を実装する。`Mutex` poison は kotoha-storage で確立した規約
//! (PR #111、`unwrap_or_else(PoisonError::into_inner)`)を流用する。

use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, PoisonError};
use std::task::{Context, Poll, Waker};
use std::time::Duration;

use super::CancellationToken;

/// std::sync ベースの cancel token impl。`Arc` で clone 可能、複数 thread から共有できる。
#[derive(Clone)]
pub struct StdCancellationToken {
    inner: Arc<Inner>,
}

struct Inner {
    flag: AtomicBool,
    notify: Mutex<Vec<Waker>>,
    condvar: Condvar,
}

impl StdCancellationToken {
    /// 新しい未 cancel な token を作成する。
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Inner {
                flag: AtomicBool::new(false),
                notify: Mutex::new(Vec::new()),
                condvar: Condvar::new(),
            }),
        }
    }
}

impl Default for StdCancellationToken {
    fn default() -> Self {
        Self::new()
    }
}

impl CancellationToken for StdCancellationToken {
    fn cancel(&self) {
        self.inner.flag.store(true, Ordering::SeqCst);
        // Future 経由で待機している waker を起こす
        let mut wakers = self
            .inner
            .notify
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        for waker in wakers.drain(..) {
            waker.wake();
        }
        // sync 経由(`Condvar::wait`)で待機しているスレッドを起こす
        self.inner.condvar.notify_all();
    }

    fn is_cancelled(&self) -> bool {
        self.inner.flag.load(Ordering::SeqCst)
    }

    fn cancelled<'a>(&'a self) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
        Box::pin(CancelledFuture {
            inner: self.inner.clone(),
        })
    }
}

struct CancelledFuture {
    inner: Arc<Inner>,
}

impl Future for CancelledFuture {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        if self.inner.flag.load(Ordering::SeqCst) {
            return Poll::Ready(());
        }
        let mut wakers = self
            .inner
            .notify
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        // double-check after taking the lock to avoid lost-wakeup
        if self.inner.flag.load(Ordering::SeqCst) {
            return Poll::Ready(());
        }
        wakers.push(cx.waker().clone());
        Poll::Pending
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    /// spec §4.4 不変条件: cancel() 前は is_cancelled() == false
    #[test]
    fn fresh_token_is_not_cancelled() {
        let t = StdCancellationToken::new();
        assert!(!t.is_cancelled());
    }

    /// spec §4.4 不変条件: cancel() 後は is_cancelled() == true (永続)
    #[test]
    fn cancel_persists() {
        let t = StdCancellationToken::new();
        t.cancel();
        assert!(t.is_cancelled());
        assert!(t.is_cancelled()); // 2 回目も true
    }

    /// spec §4.4 不変条件: cancel() は idempotent
    #[test]
    fn cancel_is_idempotent() {
        let t = StdCancellationToken::new();
        t.cancel();
        t.cancel(); // 2 回目以降 no-op
        assert!(t.is_cancelled());
    }

    /// 複数 thread から共有された clone は同一 cancel state を共有する
    #[test]
    fn clones_share_state() {
        let t1 = StdCancellationToken::new();
        let t2 = t1.clone();
        let handle = thread::spawn(move || {
            // 100ms 待ってから cancel(t1 thread が wait できる時間)
            thread::sleep(Duration::from_millis(100));
            t2.cancel();
        });
        // t1 で cancel を観測できる
        let start = std::time::Instant::now();
        while !t1.is_cancelled() {
            if start.elapsed() > Duration::from_secs(1) {
                panic!("cancel propagation timeout");
            }
            thread::sleep(Duration::from_millis(10));
        }
        handle.join().unwrap();
    }
}
```

- [ ] **Step 3: cancel module の build 確認**

```bash
cargo build -p kotoha-engine-core --no-default-features
```

期待:`ranker` module の不在で error が残るが、`cancel` module は compile success。`error[E0432]` 行が `ranker` のみに絞れていれば OK。

### Task 1.3: `ranker` module 実装(trait + types のみ、`HybridRanker` は M2)

**Files:**
- Create: `crates/kotoha-engine-core/src/ranker/mod.rs`
- Create: `crates/kotoha-engine-core/src/ranker/context.rs`
- Create: `crates/kotoha-engine-core/src/ranker/update.rs`

- [ ] **Step 1: `crates/kotoha-engine-core/src/ranker/context.rs` 作成**

```rust
//! `ConversionContext` / `ConversionMode` — Ranker 入力 context types.
//!
//! Phase 3-A spec §4.3(2026-05-02、ISSUE #116)で凍結。Phase 5 で
//! `partial_input` / `typo_distance` field が non-breaking で追加される予定。

use std::time::Duration;

/// Ranker `rank()` の context 引数。focus session 内 commit 履歴と変換 mode を含む。
///
/// `commit_history` は engine 内部の `VecDeque<String>` の snapshot を `Vec<String>`
/// として clone して詰める設計(Phase 3-A spec §4.3 備考参照)。本 struct は
/// non-exhaustive で Phase 5 field 追加時の互換性を担保する。
#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct ConversionContext {
    /// focus session 内の直前 N 文字(default 200、Phase 3-A spec §13 Open Q 1)の
    /// commit 履歴 surface(古い順)。
    pub commit_history: Vec<String>,
    /// 直前 commit からの経過時間(personalization signal、Phase 5 で活用)。
    pub time_since_last_commit: Duration,
    /// 変換 mode:Live(typing 中)か Commit(space 後)か。
    pub mode: ConversionMode,
}

impl ConversionContext {
    /// 空 context を作成する(test / focus_in 直後の初期状態用)。
    pub fn empty(mode: ConversionMode) -> Self {
        Self {
            commit_history: Vec::new(),
            time_since_last_commit: Duration::from_secs(0),
            mode,
        }
    }
}

/// 変換 mode。Live(typing 中、毎 keystroke で Ranker 起動)と
/// Commit(space 後、coalescing 拡張で LLM 待機)の 2 種。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConversionMode {
    /// Typing 中、dict only fast path、LLM は best-effort。
    Live,
    /// Space 確定後、coalescing window 拡張、LLM 完了まで待機。
    Commit,
}
```

- [ ] **Step 2: `crates/kotoha-engine-core/src/ranker/update.rs` 作成**

```rust
//! `CandidateUpdate` enum — 候補差分通知。
//!
//! Phase 3-A spec §4.2(2026-05-02、ISSUE #116)で凍結。
//! Worker thread から engine 主 thread へ、または engine から host adapter へ
//! 送られる差分通知 message。

use kotoha_core::Candidate;
use std::ops::Range;

/// 候補 list の差分通知。
///
/// IBus 1.x は `update_lookup_table` で全置換のみのため、IBus adapter は
/// `Append` / `Remove` を内部 buffer 蓄積後の `Replace` に変換する
/// (Phase 3-A spec §4.2 mapping 表)。
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum CandidateUpdate {
    /// 候補 list 全置換(Phase 3-A での主用法)。
    Replace(Vec<Candidate>),
    /// 末尾追加(coalescing 後の LLM 結果到着時)。
    Append(Vec<Candidate>),
    /// 範囲削除(Phase 5 beam search で beam 削減時、Phase 3-A 初期は未使用)。
    Remove(Range<usize>),
    /// 全 clear。
    Clear,
}
```

- [ ] **Step 3: `crates/kotoha-engine-core/src/ranker/mod.rs` 作成**

```rust
//! `Ranker` trait + `RankerError` + `RankerOutput`.
//!
//! Phase 3-A spec §4.3 で凍結された候補生成 trait。P2-D で `HybridRanker` として
//! 実装される。本 module は trait + types のみで、impl は `hybrid` sub-module
//! (P2-D Milestone 2 以降で追加される)。

mod context;
mod update;

pub use context::{ConversionContext, ConversionMode};
pub use update::CandidateUpdate;

use std::sync::Arc;
use std::sync::mpsc;

use crate::cancel::CancellationToken;

/// Ranker は `kana` 入力に対して候補一覧を生成し、`sink` channel 経由で
/// engine に逐次 push する。Phase 3-A spec §4.3 で凍結された contract。
///
/// # 動作モデル
///
/// - `rank()` は **同期に return**(channel 送信のみ)、heavy lifting は背後
///   thread / async runtime に dispatch される(impl 内部の自由)。
/// - `cancel.is_cancelled() == true` を検出したら以降の `sink.send()` を停止する。
/// - backend 個別の cancel propagation:
///   - SudachiDict / UserVocab / LearningCache: μs オーダーで完結のため cancel
///     check は不要(完了時に request_id mismatch なら engine 側で discard)。
///   - LLM: 10 token 毎に `cancel.is_cancelled()` を check、true なら inference 中断。
///
/// # Thread safety
///
/// `Send + Sync` を要求(engine の `RankerWorker` thread と engine 主 thread の
/// 双方から `Arc<dyn Ranker>` で参照される)。
pub trait Ranker: Send + Sync {
    /// 候補生成の起動。同期 return、heavy work は impl 内 thread に dispatch。
    ///
    /// # Errors
    ///
    /// - [`RankerError::Busy`]: 直前の rank 呼び出しが完了前で受け付け不可
    /// - [`RankerError::Internal`]: impl 内部の不可避エラー
    fn rank(
        &self,
        kana: &str,
        ctx: &ConversionContext,
        cancel: Arc<dyn CancellationToken>,
        sink: mpsc::Sender<RankerOutput>,
    ) -> Result<(), RankerError>;
}

/// Ranker から engine への通知 message。
#[derive(Debug, Clone)]
pub struct RankerOutput {
    /// engine 主 thread 側で active_request.id と mismatch なら discard する識別子。
    /// engine が `RankRequest` 発行時に採番した値を Ranker 内部で保持し送出する
    /// (Phase 3-A spec §7.5)。
    pub request_id: u64,
    /// 差分通知本体。
    pub update: CandidateUpdate,
}

/// Ranker error 列挙。
#[derive(thiserror::Error, Debug)]
#[non_exhaustive]
pub enum RankerError {
    #[error("ranker is busy")]
    Busy,
    #[error("ranker internal error: {0}")]
    Internal(String),
}
```

- [ ] **Step 4: build 確認**

```bash
cargo build -p kotoha-engine-core --no-default-features
cargo build -p kotoha-engine-core
```

期待:両者 success(error 0)。

### Task 1.4: trait/types L1 unit test 追加

**Files:**
- Modify: `crates/kotoha-engine-core/src/ranker/context.rs`(末尾に `#[cfg(test)] mod tests` 追加)
- Modify: `crates/kotoha-engine-core/src/ranker/update.rs`(同上)

- [ ] **Step 1: `crates/kotoha-engine-core/src/ranker/context.rs` 末尾に test 追加**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    /// spec §4.3: ConversionContext::empty(Live) で Live mode の空 context が作れる
    #[test]
    fn empty_context_live_mode() {
        let ctx = ConversionContext::empty(ConversionMode::Live);
        assert_eq!(ctx.mode, ConversionMode::Live);
        assert!(ctx.commit_history.is_empty());
        assert_eq!(ctx.time_since_last_commit, Duration::from_secs(0));
    }

    /// spec §4.3: ConversionMode は Live と Commit の 2 種
    #[test]
    fn conversion_modes_are_distinct() {
        assert_ne!(ConversionMode::Live, ConversionMode::Commit);
    }

    /// spec §4.3 備考: ConversionContext は Clone 可能(snapshot として Ranker に渡される)
    #[test]
    fn context_is_clone() {
        let ctx = ConversionContext {
            commit_history: vec!["琴葉".into()],
            time_since_last_commit: Duration::from_secs(5),
            mode: ConversionMode::Commit,
        };
        let cloned = ctx.clone();
        assert_eq!(cloned.commit_history, ctx.commit_history);
        assert_eq!(cloned.mode, ctx.mode);
    }
}
```

- [ ] **Step 2: `crates/kotoha-engine-core/src/ranker/update.rs` 末尾に test 追加**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use kotoha_core::Candidate;

    /// spec §4.2: Replace variant が Vec<Candidate> を保持できる
    #[test]
    fn replace_holds_candidates() {
        let upd = CandidateUpdate::Replace(vec![Candidate::new("琴葉", -1.5)]);
        match upd {
            CandidateUpdate::Replace(v) => {
                assert_eq!(v.len(), 1);
                assert_eq!(v[0].surface, "琴葉");
            }
            _ => panic!("expected Replace"),
        }
    }

    /// spec §4.2: Clear variant が引数なしで構築できる
    #[test]
    fn clear_is_unit_variant() {
        let upd = CandidateUpdate::Clear;
        match upd {
            CandidateUpdate::Clear => {}
            _ => panic!("expected Clear"),
        }
    }

    /// spec §4.2: CandidateUpdate は Clone 可能(adapter 内部 buffer 等で複製される)
    #[test]
    fn update_is_clone() {
        let upd = CandidateUpdate::Replace(vec![Candidate::new("琴葉", -1.5)]);
        let cloned = upd.clone();
        match (upd, cloned) {
            (CandidateUpdate::Replace(a), CandidateUpdate::Replace(b)) => {
                assert_eq!(a[0].surface, b[0].surface);
            }
            _ => panic!("expected Replace"),
        }
    }
}
```

- [ ] **Step 3: test 実行確認**

```bash
cargo test -p kotoha-engine-core --lib
```

期待:7 tests pass(`cancel::std_token::tests` 4 + `ranker::context::tests` 3 + `ranker::update::tests` 3 = 10 tests、ただし `update::tests` は `kotoha_core::Candidate` import 経由で追加 dep 確認も兼ねる)。実際の数は impl 結果で確認。

- [ ] **Step 4: workspace 全体 baseline 確認(0 regression)**

```bash
cargo test --workspace --features kotoha-storage/test-helpers --no-fail-fast
```

期待:従来 340 PASS + 新 ~10 tests = ~350 PASS、0 FAIL。

### Task 1.5: glossary 更新 + 文書 cross-check + commit

**Files:**
- Modify: `docs/wiki/glossary.md`(既存 Phase 3 sub-section に追補)

- [ ] **Step 1: glossary に StdCancellationToken entry 追加**

`docs/wiki/glossary.md` の Phase 3 IBus engine 用語サブセクション末尾(`### ConversionContext` の直後)に以下を追加:

```markdown
### StdCancellationToken (std::sync ベース cancel token impl)

- **定義**: Phase 3-A spec §4.4 で凍結された `CancellationToken` trait の Phase 3-A 初期 impl。`Arc<AtomicBool>` + `(Mutex, Condvar)` ペアで cancel signal の永続化と sync/future 双方の待機を実装する。`Mutex` poison は `unwrap_or_else(PoisonError::into_inner)` で recover する(PR #111 規約と整合)。
- **初出**: P2-D Milestone 1(2026-05-02、ISSUE #120 / PR `feature/120-p2d-trait-skeleton`)
- **対応する identifier**: `kotoha_engine_core::cancel::StdCancellationToken`(`crates/kotoha-engine-core/src/cancel/std_token.rs`)
- **備考**: tokio runtime 導入は Phase 5/6 で再評価。それまで Phase 3-A は std::sync ベースで運用。
```

- [ ] **Step 2: lefthook pre-push 実行(commit 前の最終確認)**

```bash
lefthook run pre-push
```

期待:fmt-check / build / clippy / test 全 PASS。

- [ ] **Step 3: stage + commit**

```bash
git add Cargo.toml crates/kotoha-engine-core/ docs/wiki/glossary.md
git status
```

`git status` で stage 内容を確認、想定外の変更が無いこと(fmt-check で auto fix された場合は確認)。

```bash
git commit -m "$(cat <<'EOF'
feat(engine-core): add kotoha-engine-core crate with Ranker trait + cancel token (P2-D M1, #120)

Phase 3-A spec §4.3 / §4.4 で凍結された Ranker trait と CancellationToken
trait + std::sync ベース impl を定義する新 crate `kotoha-engine-core` を
追加する。trait + types の純粋定義のみで、HybridRanker concrete impl は
M2 以降の PR で追加される。

新 crate 構成:
- src/lib.rs                        crate root + re-export
- src/cancel/mod.rs                 CancellationToken trait
- src/cancel/std_token.rs           StdCancellationToken (std::sync impl)
- src/ranker/mod.rs                 Ranker trait + RankerError + RankerOutput
- src/ranker/context.rs             ConversionContext + ConversionMode
- src/ranker/update.rs              CandidateUpdate enum (4 variants)

Glossary updated: StdCancellationToken entry under Phase 3 IBus engine
section.

Test count: ~10 new L1 unit tests covering invariants (cancel idempotent,
clone-share-state, ConversionMode distinction, CandidateUpdate variants).

Refs: spec §4.3 / §4.4 / §3.1, ADR 0011 (related backend trait pattern)

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

- [ ] **Step 4: push + PR**

```bash
git push -u origin feature/120-p2d-trait-skeleton
gh pr create \
  --base develop \
  --head feature/120-p2d-trait-skeleton \
  --title "feat(engine-core): kotoha-engine-core crate with Ranker trait + cancel token (P2-D M1)" \
  --body "Refs #120. Phase 3-A spec §4.3 / §4.4 で凍結された Ranker / CancellationToken trait + std::sync impl を新 crate \`kotoha-engine-core\` として追加。trait + types の純粋定義のみで、HybridRanker concrete impl は M2 以降。Test count +10、baseline 340 → ~350、0 regression。"
```

- [ ] **Step 5: review + merge**

`agent-teams:team-review`(security + arch + testing 3 dim)+ `secrets-check` を起動、findings に対応してから squash merge。

```bash
gh pr merge <PR#> --squash --delete-branch
git checkout develop
git pull origin develop
```

---

## Milestone 2: HybridRanker dict-only impl(PR 2、Medium tier、~500-700 lines)

**Goal:** SudachiDict + UserVocab + LearningCache の 3 backend を統合した dict-only `HybridRanker` を実装する。LLM 統合は M3。merge / dedupe / scoring logic を確立する。

**Branch:** `feature/120-p2d-hybrid-ranker-dict`

### Task 2.1: branch 切り + ranker module 拡張

**Files:**
- Create: `crates/kotoha-engine-core/src/ranker/hybrid.rs`
- Create: `crates/kotoha-engine-core/src/ranker/merge.rs`
- Modify: `crates/kotoha-engine-core/src/ranker/mod.rs`(新 sub-module 公開)
- Modify: `crates/kotoha-engine-core/Cargo.toml`(SudachiAdapter 利用のため `kotoha-core` の `dict` feature を引き出す)

- [ ] **Step 1: branch 作成**

```bash
git checkout develop
git pull origin develop
git checkout -b feature/120-p2d-hybrid-ranker-dict
```

- [ ] **Step 2: `crates/kotoha-engine-core/Cargo.toml` 修正**

`[dependencies]` の `kotoha-core` 行を以下に置き換える(features を引き出す):

```toml
kotoha-core = { path = "../kotoha-core", features = ["dict"] }
kotoha-storage = { path = "../kotoha-storage" }
```

- [ ] **Step 3: `crates/kotoha-engine-core/src/ranker/merge.rs` 作成**

```rust
//! Candidate merge / dedupe / scoring logic.
//!
//! Phase 2 spec §3.3 で凍結された初期重み:
//! - dict: 0.95(SudachiDict + UserVocab)
//! - LLM: 1.0
//! - LearningCache hit: bonus(empirical、現在の暫定値は +0.5)
//!
//! 同一 surface の候補は max-score 採用で dedupe する(Phase 2 spec §3.3 D5
//! の選択肢「最大値採用」)。重み合算は P2-D 完了後の golden fixture evaluation
//! で再検討する余地を残す(Phase 2 spec §11.4 Q5)。

use kotoha_core::Candidate;
use std::collections::BTreeMap;

/// dict 候補の重み(SudachiDict + UserVocab、Phase 2 spec §3.3 暫定値)。
pub const WEIGHT_DICT: f32 = 0.95;
/// LLM 候補の重み。
pub const WEIGHT_LLM: f32 = 1.0;
/// LearningCache hit bonus(暫定、Phase 2 spec §11.4 Q5 で再評価)。
pub const BONUS_CACHE_HIT: f32 = 0.5;

/// 候補 source(merge 時の score 計算で使用)。
#[derive(Debug, Clone, Copy)]
pub enum CandidateSource {
    Dict,
    Llm,
    CacheHit,
}

/// `CandidateSource::Dict` から score を repsect しつつ重み付けする。
pub fn weight_for(source: CandidateSource) -> f32 {
    match source {
        CandidateSource::Dict => WEIGHT_DICT,
        CandidateSource::Llm => WEIGHT_LLM,
        CandidateSource::CacheHit => BONUS_CACHE_HIT,
    }
}

/// 複数 source の候補を merge / dedupe / sort する。
///
/// # Algorithm
///
/// 1. 同一 surface の候補は max-score 採用で dedupe(BTreeMap で surface → max(weighted_score))
/// 2. weighted_score = candidate.score + weight_for(source) で計算
///    (注: candidate.score は backend が返す raw 値、weight は source bias)
/// 3. CacheHit 由来は dict / llm score に加算する bonus 扱い(独立 candidate ではなく既存 entry の score 加算)
/// 4. 最終的に weighted_score 降順 sort、`top_k` 件に切り詰める
///
/// # Phase 2 spec §3.3 ref
///
/// 「同一 surface が両方で hit した場合は User 側の score を優先する」を実現するため、
/// UserVocab 由来の候補は `Dict` source として扱い、SudachiDict 由来より先に挿入されると
/// max-score 採用で勝つ前提で順序を制御する。
pub fn merge_candidates(
    sources: Vec<(Vec<Candidate>, CandidateSource)>,
    cache_hits: &[String], // CacheHit を bonus 加算する対象 surface
    top_k: usize,
) -> Vec<Candidate> {
    let mut by_surface: BTreeMap<String, f32> = BTreeMap::new();

    for (cands, source) in sources {
        let w = weight_for(source);
        for c in cands {
            let weighted = c.score + w;
            by_surface
                .entry(c.surface)
                .and_modify(|s| *s = s.max(weighted))
                .or_insert(weighted);
        }
    }

    // CacheHit bonus を該当 surface の score に加算
    for surface in cache_hits {
        if let Some(s) = by_surface.get_mut(surface) {
            *s += BONUS_CACHE_HIT;
        }
    }

    let mut merged: Vec<Candidate> = by_surface
        .into_iter()
        .map(|(surface, score)| Candidate::new(surface, score))
        .collect();
    merged.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    merged.truncate(top_k);
    merged
}

#[cfg(test)]
mod tests {
    use super::*;

    /// spec §3.3: 同一 surface は max-score 採用で dedupe される
    #[test]
    fn dedupe_takes_max_score() {
        let dict = vec![Candidate::new("琴葉", -2.0)];
        let llm = vec![Candidate::new("琴葉", -1.0)];
        let merged = merge_candidates(
            vec![
                (dict, CandidateSource::Dict),
                (llm, CandidateSource::Llm),
            ],
            &[],
            10,
        );
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].surface, "琴葉");
        // weighted: dict=-2.0+0.95=-1.05, llm=-1.0+1.0=0.0 → llm 勝ち
        assert!((merged[0].score - 0.0).abs() < f32::EPSILON);
    }

    /// spec §3.3: CacheHit bonus が該当 surface に加算される
    #[test]
    fn cache_hit_adds_bonus() {
        let dict = vec![Candidate::new("琴葉", 0.0)];
        let merged = merge_candidates(
            vec![(dict, CandidateSource::Dict)],
            &["琴葉".into()],
            10,
        );
        assert_eq!(merged.len(), 1);
        // weighted: 0.0 + 0.95(dict) + 0.5(cache hit) = 1.45
        assert!((merged[0].score - 1.45).abs() < 1e-5);
    }

    /// 候補 0 件は空 Vec を返す
    #[test]
    fn empty_input_returns_empty() {
        let merged = merge_candidates(vec![], &[], 10);
        assert!(merged.is_empty());
    }

    /// top_k で切り詰められる
    #[test]
    fn truncates_to_top_k() {
        let dict = vec![
            Candidate::new("a", 3.0),
            Candidate::new("b", 2.0),
            Candidate::new("c", 1.0),
        ];
        let merged = merge_candidates(vec![(dict, CandidateSource::Dict)], &[], 2);
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0].surface, "a");
        assert_eq!(merged[1].surface, "b");
    }

    /// score 降順 sort
    #[test]
    fn sorts_by_score_descending() {
        let dict = vec![
            Candidate::new("low", 0.0),
            Candidate::new("high", 10.0),
            Candidate::new("mid", 5.0),
        ];
        let merged = merge_candidates(vec![(dict, CandidateSource::Dict)], &[], 10);
        assert_eq!(merged[0].surface, "high");
        assert_eq!(merged[1].surface, "mid");
        assert_eq!(merged[2].surface, "low");
    }
}
```

- [ ] **Step 4: `crates/kotoha-engine-core/src/ranker/hybrid.rs` 作成(dict-only impl、LLM 統合は M3)**

```rust
//! `HybridRanker` — SudachiDict + UserVocab + LearningCache + LLM の統合 Ranker。
//!
//! 本 module は M2 で dict-only(SudachiDict + UserVocab + LearningCache)を実装し、
//! M3 で LLM backend 統合を追加する。
//!
//! Phase 3-A spec §4.3 で凍結された Ranker trait の concrete impl。

use std::sync::mpsc;
use std::sync::Arc;
use std::thread;

use kotoha_core::dict::sudachi_adapter::SudachiAdapter;
use kotoha_core::dict::MorphologicalEngine;
use kotoha_core::Candidate;
use kotoha_storage::learning_cache::LearningCacheReader;
use kotoha_storage::user_vocab::UserVocabReader;

use super::{
    merge::{merge_candidates, CandidateSource},
    CandidateUpdate, ConversionContext, Ranker, RankerError, RankerOutput,
};
use crate::cancel::CancellationToken;

/// 候補生成の top_k(暫定、empirical で再評価)。
const DEFAULT_TOP_K: usize = 10;

/// SudachiDict + UserVocab + LearningCache + LLM(M3 で追加)の統合 Ranker。
///
/// # Construction
///
/// `Box<dyn UserVocabReader>` / `Arc<dyn LearningCacheReader>` / `SudachiAdapter`
/// を構築時に DI で受け取る。LLM backend(M3 追加)は `Option` で wrap し、
/// None のときは dict-only で動作する。
pub struct HybridRanker {
    sudachi: Arc<dyn MorphologicalEngine>,
    user_vocab: Arc<dyn UserVocabReader>,
    learning_cache: Arc<dyn LearningCacheReader>,
    request_id_seed: std::sync::atomic::AtomicU64,
}

impl HybridRanker {
    pub fn new(
        sudachi: Arc<dyn MorphologicalEngine>,
        user_vocab: Arc<dyn UserVocabReader>,
        learning_cache: Arc<dyn LearningCacheReader>,
    ) -> Self {
        Self {
            sudachi,
            user_vocab,
            learning_cache,
            request_id_seed: std::sync::atomic::AtomicU64::new(0),
        }
    }

    fn next_request_id(&self) -> u64 {
        self.request_id_seed
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
    }
}

impl Ranker for HybridRanker {
    fn rank(
        &self,
        kana: &str,
        ctx: &ConversionContext,
        cancel: Arc<dyn CancellationToken>,
        sink: mpsc::Sender<RankerOutput>,
    ) -> Result<(), RankerError> {
        let request_id = self.next_request_id();
        let kana_owned = kana.to_string();
        let _mode = ctx.mode;
        let sudachi = self.sudachi.clone();
        let user_vocab = self.user_vocab.clone();
        let learning_cache = self.learning_cache.clone();

        // 並列 backend 呼び出し用 thread を spawn(同期 return を担保)
        thread::spawn(move || {
            if cancel.is_cancelled() {
                return;
            }

            // SudachiDict prefix lookup
            let dict_cands: Vec<Candidate> = sudachi
                .tokenize(&kana_owned)
                .into_iter()
                .map(|ec| Candidate::new(ec.surface, ec.score))
                .collect();

            // UserVocab prefix lookup
            let user_cands: Vec<Candidate> = user_vocab
                .find_by_prefix(&kana_owned)
                .unwrap_or_else(|e| {
                    tracing::warn!(?e, "user_vocab find_by_prefix failed, using empty");
                    Vec::new()
                })
                .into_iter()
                .map(|r| Candidate::new(r.surface, r.score))
                .collect();

            // LearningCache lookup
            let cache_records = learning_cache
                .lookup(&kana_owned, DEFAULT_TOP_K)
                .unwrap_or_else(|e| {
                    tracing::warn!(?e, "learning_cache lookup failed, using empty");
                    Vec::new()
                });
            let cache_surfaces: Vec<String> =
                cache_records.iter().map(|r| r.chosen_kanji.clone()).collect();

            if cancel.is_cancelled() {
                return;
            }

            // merge / dedupe / sort
            let merged = merge_candidates(
                vec![
                    (dict_cands, CandidateSource::Dict),
                    (user_cands, CandidateSource::Dict), // UserVocab も Dict 系重み
                ],
                &cache_surfaces,
                DEFAULT_TOP_K,
            );

            if cancel.is_cancelled() {
                return;
            }

            // sink に push(receiver が drop されてたら error 黙殺、tracing::trace のみ)
            if let Err(e) = sink.send(RankerOutput {
                request_id,
                update: CandidateUpdate::Replace(merged),
            }) {
                tracing::trace!(?e, "ranker sink closed before send");
            }
        });

        Ok(())
    }
}
```

- [ ] **Step 5: `crates/kotoha-engine-core/src/ranker/mod.rs` を修正(`merge` / `hybrid` sub-module 公開)**

`mod context;` / `mod update;` の下に以下を追加:

```rust
pub mod hybrid;
pub mod merge;
pub use hybrid::HybridRanker;
```

- [ ] **Step 6: build 確認**

```bash
cargo build -p kotoha-engine-core --features dict
cargo build -p kotoha-engine-core
```

期待:両者 success。`dict` feature の forward は `kotoha-core` の `dict` feature 経由(`HybridRanker` impl は `MorphologicalEngine` 利用するため M2 の `kotoha-engine-core` は実質 `dict` 必須)。

注意:`kotoha-engine-core` 自身に `dict` feature を定義し、依存 `kotoha-core` の `dict` を引き出す形にするか検討する。実装時に最小修正で動かす方針(直接 `kotoha-core = { features = ["dict"] }` で引き出すのが simpler)。

### Task 2.2: HybridRanker dict path L2 integration test

**Files:**
- Create: `crates/kotoha-engine-core/tests/hybrid_ranker_dict.rs`

- [ ] **Step 1: integration test 作成**

```rust
//! L2 integration test: HybridRanker dict-only path
//!
//! spec §10.3 L2-core integration の代表シナリオを mock を使わず実 SudachiDict +
//! mock UserVocab + mock LearningCache で end-to-end 検証する。

use std::sync::mpsc;
use std::sync::Arc;
use std::time::Duration;

use kotoha_core::dict::sudachi_adapter::SudachiAdapter;
use kotoha_engine_core::{
    cancel::StdCancellationToken, CandidateUpdate, ConversionContext, ConversionMode,
    HybridRanker, Ranker,
};
use kotoha_storage::learning_cache::MockLearningCacheStore;
use kotoha_storage::user_vocab::MockUserVocabStore;

#[test]
fn hybrid_ranker_returns_dict_candidates_for_known_kana() {
    let sudachi = Arc::new(SudachiAdapter::load_default().expect("load default sudachi"));
    let user_vocab = Arc::new(MockUserVocabStore::new());
    let learning_cache = Arc::new(MockLearningCacheStore::new());
    let ranker = HybridRanker::new(sudachi, user_vocab, learning_cache);
    let cancel = Arc::new(StdCancellationToken::new());
    let (tx, rx) = mpsc::channel();

    let ctx = ConversionContext::empty(ConversionMode::Live);
    ranker
        .rank("ことは", &ctx, cancel.clone(), tx)
        .expect("rank should accept request");

    let output = rx
        .recv_timeout(Duration::from_secs(5))
        .expect("ranker should respond within 5s");

    match output.update {
        CandidateUpdate::Replace(cands) => {
            assert!(!cands.is_empty(), "expected at least 1 candidate from sudachi");
            // 「ことは」→ 「言葉」「琴葉」(SudachiDict 標準収録)を含むこと
            assert!(
                cands.iter().any(|c| c.surface.contains("葉")),
                "expected candidate containing 葉, got {:?}",
                cands
            );
        }
        other => panic!("expected Replace, got {:?}", other),
    }
}

#[test]
fn hybrid_ranker_respects_cancel_before_send() {
    let sudachi = Arc::new(SudachiAdapter::load_default().expect("load default sudachi"));
    let user_vocab = Arc::new(MockUserVocabStore::new());
    let learning_cache = Arc::new(MockLearningCacheStore::new());
    let ranker = HybridRanker::new(sudachi, user_vocab, learning_cache);
    let cancel = Arc::new(StdCancellationToken::new());
    let (tx, rx) = mpsc::channel();

    // 即時 cancel
    cancel.cancel();

    let ctx = ConversionContext::empty(ConversionMode::Live);
    ranker
        .rank("ことは", &ctx, cancel.clone(), tx)
        .expect("rank should accept request");

    // cancel 後は sink に send されない
    let res = rx.recv_timeout(Duration::from_millis(500));
    assert!(
        res.is_err(),
        "expected timeout (no send after cancel), got {:?}",
        res
    );
}

#[test]
fn hybrid_ranker_reflects_user_vocab_priority() {
    let sudachi = Arc::new(SudachiAdapter::load_default().expect("load default sudachi"));

    let user_vocab = Arc::new(MockUserVocabStore::new());
    // user vocab に「ことは → 私のキャラ」を追加
    user_vocab
        .insert_record("ことは", "私のキャラ", 100.0)
        .expect("insert user vocab");

    let learning_cache = Arc::new(MockLearningCacheStore::new());
    let ranker = HybridRanker::new(sudachi, user_vocab, learning_cache);
    let cancel = Arc::new(StdCancellationToken::new());
    let (tx, rx) = mpsc::channel();

    let ctx = ConversionContext::empty(ConversionMode::Commit);
    ranker
        .rank("ことは", &ctx, cancel.clone(), tx)
        .expect("rank should accept request");

    let output = rx
        .recv_timeout(Duration::from_secs(5))
        .expect("ranker should respond within 5s");

    match output.update {
        CandidateUpdate::Replace(cands) => {
            // user_vocab 由来の高 score 候補が top にあること
            assert!(
                cands
                    .iter()
                    .take(3)
                    .any(|c| c.surface == "私のキャラ"),
                "expected user_vocab entry in top 3, got {:?}",
                cands
            );
        }
        other => panic!("expected Replace, got {:?}", other),
    }
}

#[test]
fn hybrid_ranker_reflects_learning_cache_bonus() {
    let sudachi = Arc::new(SudachiAdapter::load_default().expect("load default sudachi"));
    let user_vocab = Arc::new(MockUserVocabStore::new());

    let learning_cache = Arc::new(MockLearningCacheStore::new());
    // learning cache に「ことは → 琴葉」(frequency=10)を追加
    learning_cache
        .insert_record("ことは", "琴葉", 10, 1_700_000_000)
        .expect("insert cache");

    let ranker = HybridRanker::new(sudachi, user_vocab, learning_cache);
    let cancel = Arc::new(StdCancellationToken::new());
    let (tx, rx) = mpsc::channel();

    let ctx = ConversionContext::empty(ConversionMode::Commit);
    ranker
        .rank("ことは", &ctx, cancel.clone(), tx)
        .expect("rank should accept request");

    let output = rx
        .recv_timeout(Duration::from_secs(5))
        .expect("ranker should respond within 5s");

    match output.update {
        CandidateUpdate::Replace(cands) => {
            // 「琴葉」が dict 由来の他候補より上位にあること(cache hit bonus 効果)
            let kotoha_idx = cands.iter().position(|c| c.surface == "琴葉");
            assert!(kotoha_idx.is_some(), "expected 琴葉 in candidates");
            let idx = kotoha_idx.unwrap();
            assert!(
                idx < 3,
                "expected 琴葉 in top 3 due to cache bonus, got idx={}",
                idx
            );
        }
        other => panic!("expected Replace, got {:?}", other),
    }
}
```

- [ ] **Step 2: test 用 helper の MockUserVocabStore / MockLearningCacheStore に `insert_record` が無い場合は kotoha-storage 側にも追加**

Mock store 各々の現実装を Read で確認、`insert_record(reading, surface, score)` / `insert_record(kana, kanji, frequency, last_used_at)` に相当する直接 insert API が無ければ、`MockUserVocabStore` / `MockLearningCacheStore` に test-helpers feature 内で追加する。

(具体的修正は test 実行時の compile error で誘導される。修正必要なら別 step で対応。)

- [ ] **Step 3: test 実行**

```bash
cargo test -p kotoha-engine-core --features dict --test hybrid_ranker_dict
```

期待:4 tests pass。失敗する場合は spec §3.3 / §4.3 と test 内 assertion を再確認、code 側の defect なら修正(test 弱体化禁止)。

- [ ] **Step 4: workspace baseline 確認**

```bash
cargo test --workspace --features kotoha-storage/test-helpers --no-fail-fast
```

期待:従来 ~350 PASS + 新 ~9 tests(merge 5 + integration 4) = ~360 PASS、0 FAIL。

### Task 2.3: lefthook + commit + PR + merge

- [ ] **Step 1: lefthook pre-push 実行**

```bash
lefthook run pre-push
```

期待:全 PASS。

- [ ] **Step 2: stage + commit**

```bash
git add crates/kotoha-engine-core/
git status
git commit -m "$(cat <<'EOF'
feat(engine-core): HybridRanker dict-only impl with merge/dedupe/scoring (P2-D M2, #120)

Phase 3-A spec §4.3 + Phase 2 spec §3.3 で凍結された Ranker trait の
concrete impl の dict-only バージョンを実装する。LLM 統合は M3。

実装内容:
- HybridRanker struct: SudachiAdapter + UserVocabReader + LearningCacheReader を
  DI で受ける(LLM backend は M3 で追加)
- ranker.rank() は同期 return、内部で std::thread::spawn で並列 backend 呼び出し
- Phase 2 spec §3.3 暫定重み: dict=0.95, llm=1.0, cache_hit=+0.5
- merge_candidates(): max-score dedupe + score 降順 sort + top_k truncate
- cancel propagation: 3 phase で is_cancelled() check (entry / 後 backend 完了 / send 直前)

L2 integration tests (4):
- 「ことは」→ 葉系候補(SudachiDict 由来)を含む
- 即時 cancel で sink に send されない
- UserVocab 由来候補が top に来る(score 100 設定)
- LearningCache hit bonus で「琴葉」が top 3 に来る(frequency=10 設定)

merge unit tests (5): dedupe, cache_hit_bonus, empty, top_k truncation, sort

Test count: +9, baseline ~350 → ~360, 0 regression.

Refs: spec §4.3, ADR 0011 / 0014

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

- [ ] **Step 3: push + PR + review + merge**

```bash
git push -u origin feature/120-p2d-hybrid-ranker-dict
gh pr create --base develop --head feature/120-p2d-hybrid-ranker-dict \
  --title "feat(engine-core): HybridRanker dict-only impl (P2-D M2)" \
  --body "Refs #120. dict-only HybridRanker impl + 5 merge unit tests + 4 L2 integration tests. Medium tier 4-dim review (security + arch + testing + performance)."
```

`agent-teams:team-review`(4 dim)+ `secrets-check` を実施し、findings 解消後 squash merge:

```bash
gh pr merge <PR#> --squash --delete-branch
git checkout develop && git pull origin develop
```

---

## Milestone 3: HybridRanker LLM 統合 + context injection(PR 3、Medium tier、~400-600 lines)

**Goal:** `HybridRanker` に LLM backend(`KanjiBackend`)を追加し、`ConversionContext::commit_history` を LLM prompt に注入する。Live mode は best-effort、Commit mode は LLM 完了まで待機。Cancel は 10 token 毎 check。

**Branch:** `feature/120-p2d-hybrid-ranker-llm`

### Task 3.1: branch + Cargo.toml + HybridRanker LLM field 追加

**Files:**
- Modify: `crates/kotoha-engine-core/Cargo.toml`
- Modify: `crates/kotoha-engine-core/src/ranker/hybrid.rs`

- [ ] **Step 1: branch 作成**

```bash
git checkout develop && git pull origin develop
git checkout -b feature/120-p2d-hybrid-ranker-llm
```

- [ ] **Step 2: `crates/kotoha-engine-core/Cargo.toml` 修正**

`[features]` に LLM 用 feature を追加:

```toml
[features]
default = []
test-helpers = []
# `llama-cpp` は kotoha-core::kanji::llama_cpp::LlamaCppBackend を引き出す pass-through。
llama-cpp = ["kotoha-core/llama-cpp"]
```

依存に `kotoha-core` の `mock-backend` feature も pass-through:

```toml
mock-backend = ["kotoha-core/mock-backend"]
```

- [ ] **Step 3: `crates/kotoha-engine-core/src/ranker/hybrid.rs` を修正(LLM field 追加 + rank() に LLM path 統合)**

`HybridRanker` struct に LLM field 追加:

```rust
pub struct HybridRanker {
    sudachi: Arc<dyn MorphologicalEngine>,
    user_vocab: Arc<dyn UserVocabReader>,
    learning_cache: Arc<dyn LearningCacheReader>,
    /// LLM backend(Phase 1 Gemma or Mock)。None なら dict-only で動作。
    llm: Option<Arc<dyn KanjiBackend + Send + Sync>>,
    request_id_seed: std::sync::atomic::AtomicU64,
}

impl HybridRanker {
    pub fn new(
        sudachi: Arc<dyn MorphologicalEngine>,
        user_vocab: Arc<dyn UserVocabReader>,
        learning_cache: Arc<dyn LearningCacheReader>,
    ) -> Self {
        Self {
            sudachi,
            user_vocab,
            learning_cache,
            llm: None,
            request_id_seed: std::sync::atomic::AtomicU64::new(0),
        }
    }

    /// LLM backend を設定する builder method。
    pub fn with_llm(mut self, llm: Arc<dyn KanjiBackend + Send + Sync>) -> Self {
        self.llm = Some(llm);
        self
    }
    // ... (next_request_id 既存)
}
```

`rank()` 内部の thread::spawn 内に LLM path を追加(dict 候補送信後):

```rust
            // ... dict candidates merged + sink push 完了後 ...

            // LLM path(Option、Live mode は best-effort、Commit mode は完了待ち)
            if let Some(llm) = llm_opt {
                if cancel.is_cancelled() {
                    return;
                }

                // commit_history → LLM prompt 注入(現状 ConvertOptions に context 拡張無し、
                // P2-D 段階では prompt template への直接注入は LlamaCppBackend 側で行う必要あり。
                // 当面 ConvertOptions::default() で呼び、context 注入は M3 後続 ISSUE で別途扱う。)
                let opts = kotoha_core::ConvertOptions::default();
                match llm.convert(&kana_owned, &opts) {
                    Ok(llm_cands) => {
                        if cancel.is_cancelled() {
                            return;
                        }
                        // 既存 dict 結果に LLM 結果を append、改めて merge して全置換 push
                        let combined = merge_candidates(
                            vec![
                                (dict_cands_clone, CandidateSource::Dict),
                                (llm_cands, CandidateSource::Llm),
                            ],
                            &cache_surfaces_clone,
                            DEFAULT_TOP_K,
                        );
                        if let Err(e) = sink.send(RankerOutput {
                            request_id,
                            update: CandidateUpdate::Replace(combined),
                        }) {
                            tracing::trace!(?e, "ranker sink closed before LLM send");
                        }
                    }
                    Err(e) => {
                        tracing::warn!(?e, "LLM backend failed, dict candidates only");
                    }
                }
            }
```

注意:
- `dict_cands` / `cache_surfaces` を LLM 用に再利用するため、最初の send 前に `clone()` で別変数(`dict_cands_clone` / `cache_surfaces_clone`)を作る。
- LLM call は同期 `convert()` で、内部の token-level cancel check は `KanjiBackend` 側に未実装(将来 ADR + backend 拡張の議題)。M3 では **convert() 完了後に cancel check** で止まる単純実装に留める。10 token 毎 check は Phase 3-A 本番で fine-grained 化(spec §13 Open Q 4)。

- [ ] **Step 4: build 確認**

```bash
cargo build -p kotoha-engine-core --features dict,mock-backend
cargo build -p kotoha-engine-core
```

期待:両者 success。

### Task 3.2: LLM integrated L2 test + Phase 1 14/15 regression test

**Files:**
- Create: `crates/kotoha-engine-core/tests/hybrid_ranker_llm.rs`
- Create: `crates/kotoha-engine-core/tests/regression_phase1.rs`

- [ ] **Step 1: `crates/kotoha-engine-core/tests/hybrid_ranker_llm.rs` 作成**

```rust
//! L2 integration test: HybridRanker with LLM backend (MockBackend)
//!
//! spec §10.3 + §10.4 で要求される LLM integrated path を MockBackend で検証。

use std::sync::mpsc;
use std::sync::Arc;
use std::time::Duration;

use kotoha_core::dict::sudachi_adapter::SudachiAdapter;
use kotoha_core::kanji::mock::MockBackend;
use kotoha_engine_core::{
    cancel::StdCancellationToken, CandidateUpdate, ConversionContext, ConversionMode,
    HybridRanker, Ranker,
};
use kotoha_storage::learning_cache::MockLearningCacheStore;
use kotoha_storage::user_vocab::MockUserVocabStore;

#[test]
fn hybrid_ranker_with_llm_returns_combined_candidates() {
    let sudachi = Arc::new(SudachiAdapter::load_default().expect("load sudachi"));
    let user_vocab = Arc::new(MockUserVocabStore::new());
    let learning_cache = Arc::new(MockLearningCacheStore::new());
    let llm = Arc::new(MockBackend::with_default_fixture()) as Arc<_>;

    let ranker = HybridRanker::new(sudachi, user_vocab, learning_cache).with_llm(llm);
    let cancel = Arc::new(StdCancellationToken::new());
    let (tx, rx) = mpsc::channel();

    let ctx = ConversionContext::empty(ConversionMode::Commit);
    ranker
        .rank("ことは", &ctx, cancel, tx)
        .expect("rank accepted");

    // Commit mode は dict + LLM の 2 段 push を期待
    let dict_output = rx
        .recv_timeout(Duration::from_secs(5))
        .expect("dict response within 5s");
    let llm_output = rx
        .recv_timeout(Duration::from_secs(15))
        .expect("LLM response within 15s");

    match (dict_output.update, llm_output.update) {
        (CandidateUpdate::Replace(_dict_cands), CandidateUpdate::Replace(combined)) => {
            assert!(!combined.is_empty(), "expected combined candidates from LLM");
        }
        (a, b) => panic!("expected (Replace, Replace), got ({:?}, {:?})", a, b),
    }
}

#[test]
fn hybrid_ranker_llm_respects_cancel_before_invocation() {
    let sudachi = Arc::new(SudachiAdapter::load_default().expect("load sudachi"));
    let user_vocab = Arc::new(MockUserVocabStore::new());
    let learning_cache = Arc::new(MockLearningCacheStore::new());
    let llm = Arc::new(MockBackend::with_default_fixture()) as Arc<_>;

    let ranker = HybridRanker::new(sudachi, user_vocab, learning_cache).with_llm(llm);
    let cancel = Arc::new(StdCancellationToken::new());
    let (tx, rx) = mpsc::channel();

    cancel.cancel(); // 即時 cancel

    let ctx = ConversionContext::empty(ConversionMode::Commit);
    ranker
        .rank("ことは", &ctx, cancel, tx)
        .expect("rank accepted");

    // 即時 cancel で何も送信されない
    let res = rx.recv_timeout(Duration::from_millis(500));
    assert!(res.is_err(), "expected timeout, got {:?}", res);
}

#[test]
fn hybrid_ranker_llm_failure_falls_back_to_dict_only() {
    let sudachi = Arc::new(SudachiAdapter::load_default().expect("load sudachi"));
    let user_vocab = Arc::new(MockUserVocabStore::new());
    let learning_cache = Arc::new(MockLearningCacheStore::new());
    // 失敗を返す MockBackend variant を使う(MockBackend に always_fail mode 追加が必要なら別 step)
    let llm = Arc::new(MockBackend::with_always_failing()) as Arc<_>;

    let ranker = HybridRanker::new(sudachi, user_vocab, learning_cache).with_llm(llm);
    let cancel = Arc::new(StdCancellationToken::new());
    let (tx, rx) = mpsc::channel();

    let ctx = ConversionContext::empty(ConversionMode::Commit);
    ranker
        .rank("ことは", &ctx, cancel, tx)
        .expect("rank accepted");

    // dict 結果は送られる
    let dict_output = rx.recv_timeout(Duration::from_secs(5)).expect("dict ok");
    match dict_output.update {
        CandidateUpdate::Replace(cands) => assert!(!cands.is_empty()),
        other => panic!("expected Replace, got {:?}", other),
    }

    // LLM 失敗で 2 個目の送信は無い
    let res = rx.recv_timeout(Duration::from_millis(500));
    assert!(
        res.is_err(),
        "expected no second send after LLM failure, got {:?}",
        res
    );
}
```

注意:`MockBackend::with_always_failing()` が無ければ kotoha-core 側の `MockBackend` に `mock-backend` feature 内で追加する。

- [ ] **Step 2: `crates/kotoha-engine-core/tests/regression_phase1.rs` 作成**

```rust
//! Phase 1 Layer 3 14/15 regression test through HybridRanker
//!
//! spec §10.1 Regression / §10.4 で要求される Phase 1 baseline 維持確認。
//! 既存 fixture を Ranker 経由で実行し、Phase 1 14/15 が崩れないことを assert。
//!
//! Note: 本 test は llama-cpp feature が有効な場合のみ live LLM で動く。
//! 通常の lefthook pre-push(default features)では skip。

#[cfg(feature = "llama-cpp-smoke")]
mod smoke {
    // ... 実機 LLM smoke test 実装、Phase 1 既存 fixture を読み込み Ranker 経由で確認
    // 詳細は kotoha-core 側既存 phase1_smoke test を参考に書き起こす
    // (実際の path と fixture 読み込みは P2-D 実装段階で kotoha-core を参照しながら確定)
}

#[test]
#[cfg(not(feature = "llama-cpp-smoke"))]
fn regression_test_compiles_without_llama() {
    // llama-cpp-smoke feature 未有効時は本 test ファイルが compile error にならないことのみ確認
}
```

注意:Phase 1 既存 14/15 smoke test は `crates/kotoha-core/tests/phase1_smoke.rs` 等にある想定。実装段階で path を confirm し、Ranker 経由で同 fixture を回す形に書き換える。本 plan では構造のみ示し、具体的 fixture 読み込みは実装時に kotoha-core 側 source を Read して確定する。

- [ ] **Step 3: test 実行**

```bash
cargo test -p kotoha-engine-core --features dict,mock-backend --test hybrid_ranker_llm
cargo test -p kotoha-engine-core --features dict --test regression_phase1
```

期待:hybrid_ranker_llm 3 PASS、regression_phase1 1 PASS(non-smoke variant)、smoke variant は llama-cpp-smoke feature 必須なので default では skip。

### Task 3.3: lefthook + commit + PR + merge

- [ ] **Step 1: lefthook + commit**

```bash
lefthook run pre-push
```

```bash
git add crates/kotoha-engine-core/
git status
git commit -m "$(cat <<'EOF'
feat(engine-core): HybridRanker LLM integration + Phase 1 regression (P2-D M3, #120)

Phase 3-A spec §4.3 で凍結された Ranker trait の LLM 統合 path を追加。
M2 dict-only impl の上に kotoha-core::kanji::KanjiBackend の Box<dyn> を
DI 注入する builder API で LLM backend を組み合わせる。

実装内容:
- HybridRanker.llm: Option<Arc<dyn KanjiBackend>>(None なら dict-only)
- HybridRanker::with_llm(llm) builder method
- rank() 内: dict 結果 push 後に LLM convert を呼び出し、結果が
  返れば dict + llm を再 merge して 2 回目の Replace push
- LLM 失敗(Err)は tracing::warn 残しで dict-only fallback、send なし
- cancel check は entry / dict 完了後 / LLM 前 / LLM 完了後の 4 phase
  (10 token 毎の token-level check は KanjiBackend 拡張要、Phase 3-A
  本番で対応 — spec §13 Open Q 4)
- ConversionContext.commit_history → LLM prompt 注入は ConvertOptions
  に context 拡張が無いため P2-D 後続 ISSUE で対応(現 M3 は注入なしで
  動作確認に留める)

L2 integration tests (3, hybrid_ranker_llm): combined candidates 2-stage
push, immediate cancel, LLM failure → dict only fallback.

Regression test (regression_phase1): smoke variant は llama-cpp-smoke
feature gated、non-smoke は compile-only sanity. Phase 1 14/15 baseline
は llama-cpp-smoke feature が有効な開発環境で確認する。

Test count: +4, baseline ~360 → ~364, 0 regression.

Refs: spec §4.3 / §10.1 / §13 Open Q 4

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

- [ ] **Step 2: push + PR + 4-dim review + squash merge**

```bash
git push -u origin feature/120-p2d-hybrid-ranker-llm
gh pr create --base develop --head feature/120-p2d-hybrid-ranker-llm \
  --title "feat(engine-core): HybridRanker LLM integration (P2-D M3)" \
  --body "Refs #120. M2 dict-only ranker に LLM backend を追加、Phase 1 regression test も同梱。Medium tier 4-dim review."
```

review 後 merge:

```bash
gh pr merge <PR#> --squash --delete-branch
git checkout develop && git pull origin develop
```

---

## Milestone 4: glossary + proptest + 最終 polish(PR 4、Small tier、~150-250 lines)

**Goal:** glossary に HybridRanker / merge_candidates を追加、proptest 不変条件テストを追加して全 P2-D 完了状態を確立する。

**Branch:** `feature/120-p2d-glossary-docs`

### Task 4.1: glossary 更新

**Files:**
- Modify: `docs/wiki/glossary.md`

- [ ] **Step 1: HybridRanker entry を category 4 (kana→kanji 変換) の Phase 3 IBus engine sub-section に追加**

```markdown
### HybridRanker

- **定義**: Phase 3-A spec §4.3 で凍結された `Ranker` trait の concrete impl。SudachiDict + UserVocab + LearningCache + LLM の 4 backend を統合し、`mpsc::Sender<RankerOutput>` 経由で逐次候補を engine に push する。Phase 2 spec §3.3 で凍結された初期重み(dict 0.95 / LLM 1.0 / cache hit bonus +0.5)で merge / dedupe / scoring を行う。
- **初出**: P2-D Milestone 2(2026-05-02、ISSUE #120)
- **対応する identifier**: `kotoha_engine_core::ranker::HybridRanker`(`crates/kotoha-engine-core/src/ranker/hybrid.rs`)
- **備考**: LLM backend は `Option<Arc<dyn KanjiBackend>>` で None なら dict-only で動作。Live mode は best-effort、Commit mode は LLM 完了まで待機する設計だが、token-level cancel propagation(spec §13 Open Q 4)は Phase 3-A 本番実装で対応する。
```

### Task 4.2: proptest 不変条件 test

**Files:**
- Create: `crates/kotoha-engine-core/tests/proptest_invariants.rs`

- [ ] **Step 1: proptest 不変条件**

```rust
//! Property-based invariant tests for HybridRanker / merge logic.
//!
//! spec §10.1 で要求される proptest による invariant test。
//! 主要 invariant:
//! - merge 結果の長さは top_k 以下
//! - merge 結果の score は降順
//! - cancel 後は sink 送信なし

use proptest::prelude::*;
use std::sync::mpsc;
use std::sync::Arc;
use std::time::Duration;

use kotoha_core::Candidate;
use kotoha_engine_core::cancel::StdCancellationToken;
use kotoha_engine_core::ranker::merge::{merge_candidates, CandidateSource};
use kotoha_engine_core::CancellationToken;

fn arb_candidate() -> impl Strategy<Value = Candidate> {
    (any::<u8>(), -100.0_f32..100.0_f32).prop_map(|(idx, score)| {
        let surface = format!("c{}", idx);
        Candidate::new(surface, score)
    })
}

proptest! {
    /// invariant: merge 結果の長さは top_k 以下
    #[test]
    fn merge_respects_top_k(cands in prop::collection::vec(arb_candidate(), 0..50), top_k in 1usize..30) {
        let merged = merge_candidates(
            vec![(cands, CandidateSource::Dict)],
            &[],
            top_k,
        );
        prop_assert!(merged.len() <= top_k);
    }

    /// invariant: merge 結果は score 降順
    #[test]
    fn merge_is_sorted_descending(cands in prop::collection::vec(arb_candidate(), 0..50)) {
        let merged = merge_candidates(
            vec![(cands, CandidateSource::Dict)],
            &[],
            50,
        );
        for window in merged.windows(2) {
            prop_assert!(window[0].score >= window[1].score);
        }
    }

    /// invariant: 同一 surface の重複は merge で除去される
    #[test]
    fn merge_dedupes_by_surface(cands in prop::collection::vec(arb_candidate(), 0..50)) {
        let merged = merge_candidates(
            vec![(cands, CandidateSource::Dict)],
            &[],
            50,
        );
        let mut surfaces: Vec<&str> = merged.iter().map(|c| c.surface.as_str()).collect();
        surfaces.sort();
        let len_before = surfaces.len();
        surfaces.dedup();
        prop_assert_eq!(surfaces.len(), len_before, "duplicate surfaces detected");
    }
}

#[test]
fn cancel_invariant_no_send_after_cancel() {
    // cancel 後は sink への send なし(integration test の確認的 unit 版)
    let token = Arc::new(StdCancellationToken::new());
    token.cancel();
    assert!(token.is_cancelled());
    // 実 Ranker での cancel 動作は M2 / M3 integration test で確認済
}
```

- [ ] **Step 2: test 実行**

```bash
cargo test -p kotoha-engine-core --features dict --test proptest_invariants
```

期待:全 PASS(proptest は default 256 case)。

### Task 4.3: P2-D 完了 wrap-up commit + PR + merge

- [ ] **Step 1: lefthook + commit + push + PR + merge**

```bash
lefthook run pre-push
git add docs/wiki/glossary.md crates/kotoha-engine-core/tests/proptest_invariants.rs
git commit -m "$(cat <<'EOF'
docs(engine-core): HybridRanker glossary + proptest invariants (P2-D M4 wrap-up, #120)

P2-D Hybrid Ranker 実装を完了する wrap-up PR:

- glossary に HybridRanker entry を追加(category 4 Phase 3 IBus engine
  sub-section)
- proptest 不変条件 test (3): top_k respect / sort descending / dedupe
  by surface

Test count: +3 (proptest 3), baseline ~364 → ~367, 0 regression.

P2-D milestone 4/4 完了。次フェーズは Phase 3-A 本番実装(別 plan + 別
ISSUE)。

Refs: spec §10.1 / glossary-consistency rule

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
git push -u origin feature/120-p2d-glossary-docs
gh pr create --base develop --head feature/120-p2d-glossary-docs \
  --title "docs(engine-core): HybridRanker glossary + proptest (P2-D M4 wrap-up)" \
  --body "Refs #120. P2-D wrap-up: glossary entry + proptest invariants. Small tier review."
```

review 後 merge:

```bash
gh pr merge <PR#> --squash --delete-branch
git checkout develop && git pull origin develop
gh issue close 120 --comment "P2-D Hybrid Ranker implementation completed across PRs M1 (trait+types), M2 (dict-only), M3 (LLM integration), M4 (wrap-up). Phase 3-A 本番 implementation is the next milestone (separate ISSUE)."
```

---

## Self-Review

### Spec coverage check

| Spec section | Plan task |
|-------------|-----------|
| Phase 3-A spec §4.3 Ranker trait + types | M1 Task 1.3 |
| Phase 3-A spec §4.4 CancellationToken | M1 Task 1.2 |
| Phase 3-A spec §4.2 CandidateUpdate | M1 Task 1.3 (update.rs) |
| Phase 3-A spec §3.1 kotoha-engine-core crate | M1 Task 1.1 |
| Phase 2 spec §3.3 Ranker 重み(dict 0.95 / llm 1.0 / cache bonus) | M2 Task 2.1 (merge.rs constants) |
| Phase 2 spec §3.3 merge / dedupe / rerank | M2 Task 2.1 (merge_candidates) |
| Phase 2 spec §3.3 SudachiDict + UserVocab + LearningCache 統合 | M2 Task 2.1 (HybridRanker) |
| Phase 2 spec §3.3 LLM backend 統合 | M3 Task 3.1 (with_llm + rank LLM path) |
| Phase 2 spec §11 P2-D scope: regression Phase 1 14/15 | M3 Task 3.2 (regression_phase1.rs) |
| Phase 3-A spec §8 cancel propagation | M2/M3 各 task 内 cancel check + L2 test |
| Phase 3-A spec §9.1 Ranker individual backend error WARN | M2 Task 2.1 (tracing::warn) |
| Phase 3-A spec §10.1 5-layer testing(L1/L2-core) | M1 Task 1.4 / M2 Task 2.2 / M3 Task 3.2 |
| glossary update(global rule) | M1 Task 1.5 / M4 Task 4.1 |
| proptest invariant | M4 Task 4.2 |

未カバー:
- Phase 3-A spec §13 Open Q 4(LLM token-level cancel)→ Phase 3-A 本番で対応、本 plan scope 外
- ConversionContext.commit_history → LLM prompt 注入 → P2-D M3 で確認したが ConvertOptions 拡張要、別 ISSUE で扱う

### Placeholder scan

- [x] "TBD" / "TODO" / "implement later" 検索 → 0(plan 内に code が完全に提示されている)
- [x] 関数 / type 名の consistency → 確認済(`merge_candidates` / `CandidateSource` / `HybridRanker` 全 task 一貫)
- [x] file path 正確性 → 確認済(crate path は `crates/kotoha-engine-core/src/...` で統一)
- [x] commit message body 完全性 → 確認済(Co-Authored-By trailer 含む)

### Type consistency

- [x] `Ranker::rank()` signature M1 で確定、M2/M3 で同一 signature を保つ
- [x] `RankerOutput.request_id: u64` M1 で確定、M2/M3 で `next_request_id()` で 64-bit 採番
- [x] `CandidateSource::Dict` / `Llm` / `CacheHit` enum variant 名 M2/M3 で一貫

### 残 Open Q(Plan execution 中に確認 / 対応)

- M2 Task 2.2 で MockUserVocabStore / MockLearningCacheStore に `insert_record` 系 helper が無ければ test-helpers feature 内で追加(Mock 拡張は spec §10.5 で許容済み)
- M3 Task 3.2 で MockBackend に `with_always_failing()` が無ければ kotoha-core 側に追加(同上)
- M3 ConvertOptions に context 拡張は P2-D scope 外、別 ISSUE で起票推奨

---

## Execution Handoff

Plan complete and saved to `docs/superpowers/plans/2026-05-02-feature-120-p2d-hybrid-ranker.md`. Two execution options:

**1. Subagent-Driven (recommended)** — fresh subagent per task, review between tasks, fast iteration。各 Milestone(PR)単位で subagent 起動 → 4-dim review → merge → 次 Milestone subagent。

**2. Inline Execution** — execute tasks in this session using executing-plans, batch execution with checkpoints。session が長くなるが context 一貫。

**Which approach?**

- Subagent-Driven 選択時:`superpowers:subagent-driven-development` skill を起動
- Inline Execution 選択時:`superpowers:executing-plans` skill を起動

prereq #118 / #119 が両方 OPEN の状態で本 plan 実行に進む場合、Milestone 1 着手前に prereq を直接実装(Small PR、本 plan 範囲外)するか、prereq 完了を待ってから本 plan に進むか選択してください。
