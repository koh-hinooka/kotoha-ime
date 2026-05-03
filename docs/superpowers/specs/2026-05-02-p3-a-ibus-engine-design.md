# Phase 3-A: IBus engine integration design spec

| 項目 | 値 |
|------|----|
| Phase | Phase 3 (IBus integration) — milestone P3-A |
| ISSUE | [#116](https://github.com/std-koh-hinooka/kotoha-ime/issues/116) |
| 起票日 | 2026-05-02 |
| Status | Draft |
| 関連 ADR(候補) | 0017(IBus engine API surface and async modality)、0018(Ranker invocation contract) |
| 前提 spec | `docs/superpowers/specs/2026-04-25-kotoha-phase-2-design.md` §3.3 / §11、`docs/superpowers/specs/2026-04-26-p2-c-learning-cache-design.md` |
| 関連 handoff | `.claude/projects/-home-kohshiro-develops-student-kotoha-ime/memory/project_session_handoff_2026-04-28.md` |

## 目次

1. [§1 概要](#1-概要)
2. [§2 スコープと範囲外](#2-スコープと範囲外)
3. [§3 architecture](#3-architecture)
4. [§4 trait API contract](#4-trait-api-contract)
5. [§5 KotohaEngine 状態機械](#5-kotohaengine-状態機械)
6. [§6 data flow](#6-data-flow)
7. [§7 RankerWorker と coalescing 規約](#7-rankerworker-と-coalescing-規約)
8. [§8 cancel propagation 規約](#8-cancel-propagation-規約)
9. [§9 error handling と observability](#9-error-handling-と-observability)
10. [§10 testing strategy](#10-testing-strategy)
11. [§11 implementation roadmap(高レベル sequence)](#11-implementation-roadmap高レベル-sequence)
12. [§12 prerequisite tasks](#12-prerequisite-tasks)
13. [§13 Open Questions](#13-open-questions)
14. [§14 forward direction(Phase 4 / 5 / 6)](#14-forward-directionphase-4--5--6)

---

## §1 概要

本 spec は Phase 3-A milestone 「IBus engine integration design」 の **設計書 draft** である。Phase 3-A の目的は GNOME Wayland 上で Kotoha を IBus engine として動作させ、user の typing → preedit 表示 → 候補生成 → commit という IME core flow を確立することである。本 spec は Phase 3-A 本番実装の前段で、API contract / state machine / data flow / error handling / testing strategy を確定し、後続の P2-D Ranker 実装と Phase 3-A 本番実装が同一方針で進められる状態に整えることを役割とする。

本 spec の workflow 上の位置:

```text
[本 spec 起票] → [P2-D Ranker 実装] → [Phase 3-A 本番実装]
   ↑                ↑                    ↑
   API 契約凍結      Ranker trait 実装    Engine + IBus adapter 実装
```

P2-D Ranker は本 spec で凍結する `Ranker` trait の contract を実装する。Phase 3-A 本番実装は本 spec で凍結する `IMEEngine` / `IMEHostBridge` / `KotohaEngine` 構造を実装する。

本 spec の adaptive 設計原則は以下である:

- **boundary-first**:adapter 境界(Hexagonal port)を最初から正しく置き、IBus / fcitx5 / 将来の IME host を adapter crate の追加 / 差し替えだけで対応可能にする
- **YAGNI 厳守**:Phase 3-A は IBus 単独 target、Phase 4 fcitx5 や Phase 6 advanced は trait surface の forward-compat で吸収し実装は後続 phase へ deferral
- **empirical-first(Phase 3-A 内のみ)**:Phase 3-A 実装段階での latency budget / IBus flicker / LLM cancel granularity は spec で範囲を定め実装で empirical 確定する

## §2 スコープと範囲外

### §2.1 本 spec 内(In-scope)

- IBus 1.x protocol 経由で GNOME Wayland 上で Kotoha を起動する engine 設計
- 3 crate(`kotoha-engine-core` / `kotoha-engine-ibus` / `kotoha-bin`)の責務分担と inter-crate 依存関係
- `IMEEngine` / `IMEHostBridge` / `Ranker` / `ConversionContext` / `CandidateUpdate` / `CancellationToken` の trait API contract 凍結
- `KotohaEngine` 状態機械の遷移 table(Live 変換 first design)
- 4 data flow path(typing / backspace / commit / cancel-or-focus_out)の確定
- `RankerWorker` の background thread 設計と dynamic coalescing window 規約
- Cancel propagation の 5 trigger 規約
- error handling 規約と observability(`tracing`)要件
- 5-layer testing strategy(L1 unit / L2-core integration / L2-adapter integration / L3 manual smoke / Regression)
- Phase 0 RomajiConverter に対する prerequisite task 列挙(`reset_pending()` 追加、kunrei/Hepburn/waapuro 3 方式並立確認)
- 6 architectural decisions + 4 sub-decisions の根拠と trade-off 記録

### §2.2 本 spec 範囲外(Out-of-scope、他 spec / 他 Phase で扱う)

- **Ranker 実装詳細**: `Ranker` trait の concrete impl は P2-D spec で扱う。本 spec は trait contract と入力 / 出力 / cancel propagation 規約のみを凍結する
- **fcitx5 adapter**: Phase 4 で別 spec を起票する。本 spec の trait 設計は forward-compat 前提だが Phase 4 spec で empirical 検証する
- **再変換 / reconvert**: 確定済 kanji の選び直し(IBus `text-input-v3` SetSurroundingText 経由)は Phase 6 advanced features へ deferral
- **per-application context separation**: application 別の commit_history / personalization は Phase 6 advanced features へ deferral
- **Live 変換の Phase 5 partial-input + beam search**: 本 spec は Live 変換の枠組みを採るが、partial-input 対応の本格実装は Phase 5 / Phase 6 で扱う
- **host detection logic 詳細**: Phase 3-A は IBus 固定起動、Phase 4 で fcitx5 adapter 増設時に `kotoha-bin` 内 detection logic を確定する
- **systemd / IBus daemon registration の OS-level packaging**: Phase 3-A 本番実装段階で扱う(本 spec は entry point binary の責務のみ規定)

## §3 architecture

### §3.1 crate 構成と責務

| crate | 責務 | 既存 / 新規 | host 依存 |
|-------|------|-----------|----------|
| `kotoha-core` | romaji / kana / kanji backend、Phase 0/1 確立済 | 既存(微修正のみ) | 無 |
| `kotoha-storage` | LearningCache / UserVocab、Phase 2-B/C 確立済 | 既存(変更なし) | 無 |
| `kotoha-cli` | CLI 3 binary(romaji / dict / kanji) | 既存(変更なし) | 無 |
| `kotoha-engine-core` | IME engine domain core(state machine、trait 定義、worker、context、cancel)、host 非依存 | **新規** | 無 |
| `kotoha-engine-ibus` | IBus protocol adapter(`IMEHostBridge` impl + IBus event dispatcher) | **新規** | IBus(`ibus` crate or `zbus` 経由) |
| `kotoha-bin` | entry point binary(host 検出、DB open、Engine 構築、event loop 起動) | **新規** | adapter 経由 |
| `kotoha-engine-fcitx5` | fcitx5 protocol adapter(Phase 4 で追加) | Phase 4 で新規 | fcitx5 |

依存方向:

```text
kotoha-bin
  ├─ kotoha-engine-ibus
  │    └─ kotoha-engine-core
  │         ├─ kotoha-core
  │         └─ kotoha-storage
  └─ (future) kotoha-engine-fcitx5
```

`kotoha-engine-core` は host 非依存原則に従い、`ibus` / `fcitx5` / `tokio` のいずれにも直接依存しない。adapter が `IMEHostBridge` trait を impl する。

### §3.2 architectural pattern

本 spec は **Hexagonal Architecture(Ports and Adapters)+ Dependency Injection** を採用する。

| 概念 | 配置 | 例 |
|------|------|----|
| Driving port(host → engine) | `kotoha-engine-core::IMEEngine` trait | adapter が `engine.process_key_event(key)` を呼ぶ |
| Driven port(engine → host) | `kotoha-engine-core::IMEHostBridge` trait | engine が `host.commit_text("")` を呼ぶ |
| Domain core | `kotoha-engine-core::KotohaEngine` struct | state machine と RankerWorker の orchestration |
| Adapter(driving)| `kotoha-engine-ibus::IBusEventDispatcher` | IBus D-Bus event を `IMEEngine` method 呼び出しに変換 |
| Adapter(driven)| `kotoha-engine-ibus::IBusHostBridge` | `IMEHostBridge` を IBus D-Bus call に変換(Adapter pattern、core から見ると IBus サブシステムへの単一 entry point として Facade 機能を兼ねる) |
| DI | `kotoha-bin::main` | `Box<dyn IMEHostBridge>` / `Box<dyn Ranker>` / `Arc<dyn LearningCacheReader+Writer>` を構築時に注入 |

この構造により以下が成立する:

- IBus / fcitx5 のアップデートは adapter crate の version 更新のみで吸収、core は変更不要
- 新 IME host(将来の native Wayland input-method protocol 等)は新 adapter crate を追加し `kotoha-bin::main` の host 検出 logic に case を追加するだけで対応
- `KotohaEngine` の状態機械 / Ranker 呼び出し / cancel propagation は host 非依存で unit test 可能(`MockHostBridge` 注入)

### §3.3 全体図

```text
                 ┌─────────────────────────────────────────────────┐
                 │ kotoha-bin (entry point)                        │
                 │  main():                                        │
                 │    1. host 検出(P3-A: IBus 固定 hard-code)       │
                 │    2. Database::open(kotoha.db, WAL)            │
                 │    3. SqliteUserVocabStore + SqliteLearningCacheStore │
                 │    4. Ranker (P2-D で実装される concrete impl)   │
                 │    5. IBusHostBridge::new(zbus_session)         │
                 │    6. KotohaEngine::new(host, ranker, stores)   │
                 │    7. IBus event loop 起動                      │
                 └──────────┬──────────────────────────────────────┘
                            │ DI(Box<dyn ...> / Arc<dyn ...>)
                            ▼
   ┌──────────────────────────────────────────────────────────────┐
   │ kotoha-engine-core (domain core, host 非依存)                │
   │ ├─ trait IMEEngine               (driving port)              │
   │ ├─ trait IMEHostBridge           (driven port)               │
   │ ├─ trait Ranker                  (consumed contract)         │
   │ ├─ trait CancellationToken       (自作抽象、std::sync impl)  │
   │ ├─ struct KotohaEngine           (IMEEngine impl)            │
   │ │    ├─ commit_history: VecDeque<String>(N=200, empirical) │
   │ │    ├─ current_preedit: String                              │
   │ │    ├─ active_request: Option<RequestHandle>                │
   │ │    ├─ host: Box<dyn IMEHostBridge>                         │
   │ │    ├─ ranker: Box<dyn Ranker>                              │
   │ │    └─ learning_writer: Arc<dyn LearningCacheWriter>        │
   │ ├─ struct RankerWorker           (background thread)         │
   │ │    ├─ coalescing buffer(typing 5-10ms / commit 30ms)     │
   │ │    └─ request_id mismatch discard                          │
   │ ├─ struct ConversionContext      (Ranker 入力)               │
   │ ├─ enum CandidateUpdate          (差分通知)                 │
   │ └─ enum KeyEventResult           (Consumed / Forwarded)      │
   └──────────────┬───────────────────────────────────────────────┘
                  │ IMEHostBridge impl
                  ▼
   ┌──────────────────────────────────────────────────────────────┐
   │ kotoha-engine-ibus (adapter)                                 │
   │ ├─ struct IBusHostBridge         (IMEHostBridge impl)        │
   │ ├─ struct IBusEventDispatcher    (D-Bus → IMEEngine 変換)    │
   │ └─ ibus crate / zbus 経由の D-Bus 結線                        │
   └──────────────────────────────────────────────────────────────┘
```

## §4 trait API contract

本節で凍結する trait は P2-D Ranker 実装と Phase 3-A 本番実装の双方が依存する契約点である。本節の API は spec merge 後に変更が必要な場合 ADR 起票 + spec revision を要する。

### §4.1 `IMEEngine`(driving port)

```rust
/// IME host(adapter 側)が engine に対して呼び出す API。
///
/// # Lifecycle
///
/// - host が `enable()` を呼ぶ → engine が active 状態
/// - host が `process_key_event()` で keystroke を渡す
/// - host が `focus_in()` / `focus_out()` で application フォーカス変化を通知
/// - host が `disable()` を呼ぶ → engine が inactive 状態
///
/// # Send + ! Sync
///
/// adapter event loop は単一 thread から engine を呼び出す前提のため `Sync` は要求しない。
/// 並行性は engine 内部の `RankerWorker` thread と channel で扱う。
pub trait IMEEngine: Send {
    fn process_key_event(&mut self, key: KeyEvent) -> KeyEventResult;
    fn focus_in(&mut self);
    fn focus_out(&mut self);
    fn reset(&mut self);
    fn enable(&mut self);
    fn disable(&mut self);
}

pub enum KeyEventResult {
    /// engine が key を消費、host は application に key を渡してはならない
    Consumed,
    /// engine が key を処理しない、host は application に key を渡してよい
    Forwarded,
}

pub struct KeyEvent {
    pub keysym: u32,
    pub keycode: u32,
    pub modifiers: KeyModifiers,
}

bitflags::bitflags! {
    pub struct KeyModifiers: u32 {
        const SHIFT = 1 << 0;
        const CTRL  = 1 << 1;
        const ALT   = 1 << 2;
        const SUPER = 1 << 3;
        // 他 IBus IBusModifierType に対応する flag を必要時に追加
    }
}
```

#### `process_key_event` の Contract

- **Preconditions**: `enable()` 後 / `disable()` 前 / focus 状態(`focus_in()` 後 / `focus_out()` 前)
- **Postconditions**: `KeyEventResult::Consumed` を返した場合、`IMEHostBridge::update_preedit` または `update_candidates` または `commit_text` のいずれかが engine 内部で呼び出された(0 回以上)
- **Errors**: panic-free を保証(panic は top-level catch_unwind で reset 経由)、recoverable error は engine 内部で `tracing::warn` し `Consumed` で吸収

### §4.2 `IMEHostBridge`(driven port)

```rust
/// engine が IME host(adapter 側)に対して呼び出す API。
///
/// # Send + Sync
///
/// engine の主 thread と RankerWorker thread の双方から呼ばれる可能性があるため
/// 両 trait を要求する。adapter 側 impl は内部 D-Bus call の thread safety を担保する。
pub trait IMEHostBridge: Send + Sync {
    fn update_preedit(&self, text: &str, cursor: usize, visible: bool);
    fn commit_text(&self, text: &str);
    fn update_candidates(&self, update: CandidateUpdate);
    fn show_candidate_window(&self);
    fn hide_candidate_window(&self);
}

pub enum CandidateUpdate {
    /// 候補 list 全置換(Phase 3-A での主用法、IBus update_lookup_table 全置換と直接対応)
    Replace(Vec<Candidate>),
    /// 末尾追加(coalescing 後の LLM 結果到着時、Phase 3-A では adapter 側で内部 buffer に蓄積し
    /// Replace に変換する)
    Append(Vec<Candidate>),
    /// 範囲削除(Phase 5 beam search で beam 削減時、Phase 3-A 初期は未使用)
    Remove(std::ops::Range<usize>),
    /// 全 clear
    Clear,
}
```

#### IBus adapter での `CandidateUpdate` mapping

IBus 1.x の `update_lookup_table` は全置換のみであるため、adapter は以下の mapping を実装する:

| `CandidateUpdate` variant | adapter 内部処理 | IBus call |
|---------------------------|-----------------|-----------|
| `Replace(c)` | internal buffer = c | `update_lookup_table(buffer, visible=true)` |
| `Append(c)` | internal buffer.extend(c) | `update_lookup_table(buffer, visible=true)` |
| `Remove(r)` | internal buffer.drain(r) | `update_lookup_table(buffer, visible=true)` |
| `Clear` | internal buffer.clear() | `update_lookup_table([], visible=false)` |

adapter は internal buffer を `Mutex<Vec<Candidate>>` で保持し、`update_candidates` 呼び出しごとに mutate + IBus call を発行する。

#### Phase 3-B B2 での wire format 実装方針

Phase 3-A は `IBusEngineSignals`(`crates/kotoha-engine-ibus/src/proxy.rs`)の 5 method を `Err(zbus::Error::Failure(NOT_YET_IMPLEMENTED))` の fail-loud stub で残置していた(B0f, ISSUE #146 review で silent no-op stub から fail-loud に再分類)。Phase 3-B B2 では本 stub を実 D-Bus signal emit に置換し、IBus 1.x 仕様の wire format 確定を以下の方針で実装する。

- IBus 1.x の `IBusText` / `IBusAttribute` / `IBusLookupTable` を `crates/kotoha-engine-ibus/src/types.rs`(新設)に Rust struct + `#[derive(zbus::zvariant::Type, serde::Serialize)]` で定義する。`IBusSerializable` 由来の `(sv)` variant ラッピングは `zbus::zvariant::Value` で表現する。
- `IBusEngineSignals` の 5 method(`update_preedit` / `commit_text` / `update_lookup_table` / `show_lookup_table` / `hide_lookup_table`)は `connection.send_signal(...)` で実 D-Bus signal を session bus に発信する。signal interface name は `org.freedesktop.IBus.Engine`。
- 失敗時は `Result<(), zbus::Error>` を caller(`host_bridge.rs`)へ propagate、host_bridge は `tracing::warn!(error = ?e, ...)` で集約観測する。engine state には影響を与えない(spec §9.3「silent_failure 禁止」と「signal failure を engine state に伝播させない non-propagating」の両立)。
- L1 unit test は wire format round-trip(`zvariant::to_bytes` → `deserialize`)を `proxy.rs` 内部 `#[cfg(test)]` で検証する。Connection mock は CI 不安定要因(D-Bus daemon 依存)のため避け、実 D-Bus daemon 検証は B6 L3 manual smoke で実施する。
- proxy 内 method 入口の `tracing::warn!("not yet wired ...")` は実装後に削除し、`tracing::debug!` で per-signal trace に降格する。`KOTOHA_LOG=debug` 起動時のみ観測される。

### §4.3 `Ranker`(P2-D で実装される consumed contract)

```rust
/// 候補生成の domain port。P2-D で SudachiDict + UserVocab + LearningCache + LLM の
/// 統合 ranker として実装される。Phase 3-A engine は本 trait を Box<dyn> 経由で消費する。
///
/// # 動作モデル
///
/// `rank()` は同期に「request 受理」のみ完了し、候補は `sink` channel 経由で
/// 非同期に push される。`cancel` token が fire したら以降の push を停止する。
pub trait Ranker: Send + Sync {
    fn rank(
        &self,
        kana: &str,
        ctx: &ConversionContext,
        cancel: Arc<dyn CancellationToken>,
        sink: mpsc::Sender<RankerOutput>,
    ) -> Result<(), RankerError>;
}

pub struct ConversionContext {
    /// focus session 内 commit 履歴の直前 N 文字 surface(N の初期値は 200、empirical)
    pub commit_history: Vec<String>,
    /// 直前 commit からの経過時間(personalization signal、Phase 5 で活用)
    pub time_since_last_commit: std::time::Duration,
    /// 変換 mode:typing 中(Live)か space 確定後(Commit)か
    pub mode: ConversionMode,
    // Phase 5 で追加する想定 field(本 spec では未定義、forward-compat のため非破壊):
    // pub partial_input: Option<String>,
    // pub typo_distance: u32,
}

pub enum ConversionMode {
    /// typing 中、dict only fast path、LLM は best-effort
    Live,
    /// space 確定後、coalescing window 拡張、LLM 完了まで待機
    Commit,
}

pub struct RankerOutput {
    pub request_id: u64,           // engine 側で mismatch discard に使用
    pub update: CandidateUpdate,
}

#[derive(thiserror::Error, Debug)]
pub enum RankerError {
    #[error("ranker is busy")]
    Busy,
    #[error("internal: {0}")]
    Internal(String),
}
```

#### Ranker contract の必須要件

- `rank()` は **同期に return**(channel 送信のみ)、heavy lifting は背後 thread / async runtime に dispatch
- `cancel.is_cancelled() == true` を検出したら以降の `sink.send()` を停止
- backend 個別の cancel propagation:
  - SudachiDict / UserVocab / LearningCache: μs オーダーで完結のため cancel check 不要(完了時に request_id mismatch なら engine 側で discard)
  - LLM: 10 token 毎に `cancel.is_cancelled()` を check、true なら inference 中断
- `sink.send()` は best-effort、receiver が drop された(engine 側で channel close)場合は `Err` を返すが Ranker は無視して continue(error は `tracing::trace` のみ)

### §4.4 `CancellationToken`(自作抽象)

```rust
use std::future::Future;
use std::pin::Pin;

/// in-flight RankRequest の cancel signal を伝搬する抽象境界。
///
/// # 設計理由
///
/// tokio_util::sync::CancellationToken と同形式の interface だが、tokio runtime に
/// 直接依存しない自作 trait として `kotoha-engine-core` に置く。Phase 3-A 初期は
/// `StdCancellationToken` (std::sync ベース) を同梱、Phase 5/6 で tokio runtime を
/// 導入する場合は `TokioCancellationToken` adapter を別 crate で実装し差し替える。
pub trait CancellationToken: Send + Sync {
    fn cancel(&self);
    fn is_cancelled(&self) -> bool;
    /// future-based 待機(自作 impl では std::sync::Condvar、tokio impl では Notify)
    fn cancelled<'a>(&'a self) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>>;
}

pub struct StdCancellationToken {
    flag: std::sync::Arc<std::sync::atomic::AtomicBool>,
    notify: std::sync::Arc<(std::sync::Mutex<()>, std::sync::Condvar)>,
}

impl StdCancellationToken {
    pub fn new() -> Self { /* ... */ }
}

impl CancellationToken for StdCancellationToken {
    fn cancel(&self) {
        self.flag.store(true, std::sync::atomic::Ordering::SeqCst);
        let (lock, cvar) = &*self.notify;
        let _guard = lock.lock().unwrap();
        cvar.notify_all();
    }
    fn is_cancelled(&self) -> bool {
        self.flag.load(std::sync::atomic::Ordering::SeqCst)
    }
    fn cancelled<'a>(&'a self) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
        Box::pin(async move { /* Condvar wait ... */ })
    }
}
```

`Mutex` poison 取扱は kotoha-storage で確立した規約(PR #111、`unwrap_or_else(PoisonError::into_inner)`)を流用する。

## §5 KotohaEngine 状態機械

### §5.1 状態定義

| 状態 | 意味 |
|------|------|
| `Idle` | preedit 空、候補ウィンドウ非表示、active_request なし |
| `LiveConverting` | preedit に kana あり、Live 変換 RankRequest が in-flight、候補ウィンドウは候補到着時から表示 |
| `CommitConverting` | space 押下後の commit-mode RankRequest が in-flight、candidates 未到着 |
| `CandidatesShown` | commit-mode 候補表示中、user navigation / Enter / Esc 待ち |

### §5.2 遷移 table

| 現状態 | trigger | 次状態 | 動作 |
|--------|---------|--------|------|
| `Idle` | 通常文字 keypress | `LiveConverting` | RomajiConverter 経由で kana 1+ 文字追加 → `update_preedit` → cancel previous(なし)→ Live RankRequest 送信 |
| `LiveConverting` | 通常文字 keypress | `LiveConverting` | RomajiConverter 経由で kana 追加 → `update_preedit` → cancel previous → Live RankRequest 送信 |
| `LiveConverting` | backspace | `LiveConverting` or `Idle` | `current_preedit.pop()` + `RomajiConverter::reset_pending()` → `update_preedit` → cancel previous → preedit 空なら `Idle` 遷移 + `hide_candidate_window`、非空なら新 Live RankRequest |
| `LiveConverting` | space | `CommitConverting` | cancel previous Live request → commit-mode RankRequest 送信(coalescing 30ms 拡張)→ `show_candidate_window`(ウィンドウ表示、内容は到着待ち)|
| `LiveConverting` | Esc | `Idle` | cancel previous → preedit clear → `update_preedit("", 0, false)` → `hide_candidate_window` |
| `LiveConverting` | focus_out | `Idle` | cancel previous → preedit clear → commit_history clear → `update_preedit("", 0, false)` → `hide_candidate_window` |
| `CommitConverting` | RankerOutput 到着(候補非空)| `CandidatesShown` | `update_candidates(Replace)` 呼び出し |
| `CommitConverting` | preedit edit / Esc / focus_out | `Idle` or `LiveConverting` | cancel propagation、preedit / 候補ウィンドウ処理は `LiveConverting` の同 trigger 動作と同じ |
| `CandidatesShown` | navigation key(↑↓←→ / Tab / num key)| `CandidatesShown` | highlight idx 移動 → `update_candidates(Replace)` で highlight 反映、Ranker 再起動なし |
| `CandidatesShown` | Enter | `Idle` | 選択 candidate を `commit_text` → `record_choice(kana, surface)` → `commit_history.push_back(surface)` → 200 chars 超過 pop_front → `hide_candidate_window` → preedit clear |
| `CandidatesShown` | typing 再開(通常文字 keypress)| `LiveConverting` | `hide_candidate_window` → cancel previous → 新 preedit + Live RankRequest 送信 |
| `CandidatesShown` | Esc | `LiveConverting` | `hide_candidate_window` → preedit kana 復元(空でなければ)→ Live RankRequest 再送信 |
| `CandidatesShown` | backspace | `LiveConverting` | `hide_candidate_window` → preedit kana から 1 文字削除 → Live RankRequest 再送信 |
| `CandidatesShown` | focus_out | `Idle` | cancel + preedit / 候補ウィンドウ / commit_history 全 clear |

### §5.3 不変条件

- `Idle` 状態で `current_preedit.is_empty() == true && active_request.is_none()`
- いずれの状態でも `active_request.is_some()` ならば対応する cancel_token が一意に存在する
- `commit_history.len() <= 200`(超過時は最古を pop_front、§13 Open Q 1 で empirical 調整)
- `focus_out` 後は必ず `Idle` 状態(focus_in/out が unbalanced で渡される耐性は engine 側で保証)

## §6 data flow

### §6.1 typing path(Live 変換 first)

```text
[1] keypress (通常文字 'k') → IBusEventDispatcher → IMEEngine::process_key_event(KeyEvent{...})
[2] KotohaEngine 内:
    - RomajiConverter::convert_char('k') → pending="k", kana 出力なし
    - current_preedit に変化なし(まだ k のみ)
    - update_preedit("", cursor=0, visible=false) ← preedit 空継続
[3] cancel previous active_request(あれば)
[4] 新 RankRequest 構築:
    - request_id = next_id()
    - cancel_token = StdCancellationToken::new()
    - ctx = ConversionContext { commit_history, time_since_last_commit, mode: Live }
    - ranker.rank(kana=current_preedit, ctx, cancel_token, sink)
[5] active_request = Some(RequestHandle { request_id, cancel_token })

[6] keypress (通常文字 'a') → 同 path:
    - RomajiConverter::convert_char('a') → pending="", kana="か" 出力
    - current_preedit = "か"
    - update_preedit("か", cursor=1, visible=true)
    - cancel previous → 新 RankRequest(kana="か", mode=Live)

[worker 内、Live mode]
    - SudachiDict prefix lookup("か") → < 1ms 完了 → sink.send(Replace(c1))
    - UserVocab find_by_prefix("か") → < 1ms 完了 → sink.send(Replace(c2))
    - LearningCache lookup("か") → < 1ms 完了 → sink.send(Replace(c3))
    - LLM convert("か") → 50-150ms、typing 中なので best-effort
    
[7] worker coalescing buffer(typing 5-10ms window):
    - 5ms 内に c1, c2, c3 到着 → merge → sink から engine へ Replace(merged)
[8] engine 受信:
    - request_id 確認(active_request.id と一致) → 適用
    - update_candidates(Replace(merged)) → IBus update_lookup_table
    - show_candidate_window()(typing 中も候補表示)
    
[9] 次 keystroke 'i' 到着前:
    - LLM 結果がまだなら次 keystroke でも cancel + 再開、empirical で「実用的に LLM が typing 中に間に合わない」なら ConversionMode::Live で LLM skip する option を将来追加(§13 Open Q 7)

Total / keystroke: < 10ms(dict 候補表示まで)、125ms 余裕
```

### §6.2 backspace path

```text
[1] keypress (BackSpace) → process_key_event
[2] KotohaEngine 内、現状態に応じて分岐:
    
    case LiveConverting / CandidatesShown:
        - 責務分担:
            - RomajiConverter::reset_pending()  ← pending romaji buffer を clear(romaji 状態側)
            - current_preedit.pop()             ← engine 側で kana 末尾 1 文字削除(kana 状態側)
        - update_preedit(current_preedit, cursor, visible=非空)
        - 状態 if current_preedit.is_empty() { Idle } else { LiveConverting }
        - cancel previous active_request
        - 非空なら 新 Live RankRequest 送信(ctx.mode=Live)
        - 空なら hide_candidate_window
    
    case Idle:
        - 何もしない、KeyEventResult::Forwarded を返す(application が backspace を処理)
```

### §6.3 commit path

```text
[1] keypress (Return / Enter) → process_key_event(状態 = CandidatesShown)
[2] KotohaEngine 内:
    - selected = current_candidates[highlight_idx]
    - host.commit_text(&selected.surface)
    - learning_writer.record_choice(&kana_at_request_time, &selected.surface)
       └─ Err 時は tracing::warn(commit は成功させる、global feedback policy)
    - commit_history.push_back(selected.surface.clone())
    - while commit_history.iter().map(|s| s.chars().count()).sum::<usize>() > 200:
          commit_history.pop_front()
    - host.hide_candidate_window()
    - host.update_preedit("", 0, false)
    - current_preedit.clear()
    - active_request = None  ← Ranker は既に終了
    - 状態 → Idle
```

### §6.4 cancel / focus_out path

```text
focus_out → IMEEngine::focus_out → KotohaEngine:
    if let Some(req) = active_request.take() {
        req.cancel_token.cancel();
    }
    current_preedit.clear();
    commit_history.clear();
    host.hide_candidate_window();
    host.update_preedit("", 0, false);
    状態 → Idle

[worker 側、cancel 受領後]
    - in-flight backend call(LLM)が cancel.is_cancelled() を check して中断
    - 既に sink.send 済の RankerOutput は engine 側で request_id mismatch により discard
    - SudachiDict / UserVocab / LearningCache の short-running call は cancel 不要、結果は破棄

[他 cancel trigger]
    - preedit edit(typing / backspace)で in-flight cancel(active_request 入れ替え)
    - Esc key で in-flight cancel + 候補ウィンドウ閉
    - 別 RankRequest 開始(連続 space / candidate 表示中の typing 再開)で in-flight cancel
    - IMEEngine::reset()(host から強制 reset、focus_out と同等)
```

## §7 RankerWorker と coalescing 規約

### §7.1 内部型定義(engine-internal、public API ではない)

本 trait 群の background plumbing で `kotoha-engine-core` 内部で使用する型を定義する。これらは crate 外に export しない:

```rust
/// engine 主 thread から worker thread へ送る request。
struct RankRequest {
    request_id: u64,
    kana: String,
    ctx: ConversionContext,
    cancel_token: Arc<dyn CancellationToken>,
    ranker: Arc<dyn Ranker>,  // engine 構築時に注入された ranker の clone
}

/// engine 主 thread が active_request として保持する。
struct RequestHandle {
    id: u64,
    cancel_token: Arc<dyn CancellationToken>,
}

/// worker thread から engine 主 thread への通知 channel message。
enum EngineEvent {
    Candidates { request_id: u64, update: CandidateUpdate },
    /// worker 内部 panic 検出時に engine が thread 再生成判断するための signal
    WorkerError { request_id: u64, error: String },
}
```

### §7.2 worker thread モデル

`RankerWorker` は engine 構築時に 1 個 spawn される dedicated background thread である。engine 主 thread は `RankRequest` を mpsc channel で worker に送る。worker は内部で以下の責務を持つ:

- `Ranker::rank()` を呼び出す(同期 return、heavy work は Ranker 内部の更に下位 thread / async に dispatch される想定)
- Ranker からの `RankerOutput` を受け取る intermediate channel(`mpsc::Receiver<RankerOutput>`)を保持
- Coalescing buffer に蓄積し、window 経過後または閾値到達で `EngineEvent::Candidates` を engine 主 thread に送る
- request_id mismatch を検出して engine 側で discard(§7.5)

### §7.3 Coalescing window(動的)

| `ConversionMode` | window | 動機 |
|------------------|--------|------|
| `Live`(typing 中)| **5-10ms** | typing 応答性優先、dict 候補だけでも即描画 |
| `Commit`(space 後)| **30ms** | LLM 結果を待って 1 回統合描画、flicker 最小化 |

window の正確な値は §13 Open Q 2 で empirical 確定する。

### §7.4 worker 主 loop(疑似 code)

```rust
fn worker_loop(rx_request: mpsc::Receiver<RankRequest>, tx_engine: mpsc::Sender<EngineEvent>) {
    while let Ok(req) = rx_request.recv() {
        let (tx_ranker, rx_ranker) = mpsc::channel();
        let _ = req.ranker.rank(&req.kana, &req.ctx, req.cancel_token.clone(), tx_ranker);
        
        let window = match req.ctx.mode {
            ConversionMode::Live   => Duration::from_millis(7),
            ConversionMode::Commit => Duration::from_millis(30),
        };
        let deadline = Instant::now() + window;
        let mut buffer: Vec<Candidate> = Vec::new();
        
        loop {
            let timeout = deadline.saturating_duration_since(Instant::now());
            match rx_ranker.recv_timeout(timeout) {
                Ok(out) => {
                    if req.cancel_token.is_cancelled() { break; }
                    apply_to_buffer(&mut buffer, out.update);
                }
                Err(RecvTimeoutError::Timeout) => break,
                Err(RecvTimeoutError::Disconnected) => break,
            }
        }
        
        if !req.cancel_token.is_cancelled() && !buffer.is_empty() {
            let _ = tx_engine.send(EngineEvent::Candidates {
                request_id: req.request_id,
                update: CandidateUpdate::Replace(buffer.clone()),
            });
        }
        
        // commit-mode の場合は LLM 後続結果が来る可能性、second window で待機
        if req.ctx.mode == ConversionMode::Commit {
            let second_deadline = Instant::now() + Duration::from_millis(150);
            // 同様の loop で追加 update を engine に送る、cancel 時は break
        }
    }
}
```

### §7.5 request_id mismatch discard

- engine 主 thread が新 RankRequest を発行するごとに `next_id()` で 64-bit 単調増加 ID を採番
- engine 受信側で `EngineEvent::Candidates { request_id, update }` を受け取った時、`active_request.id` と一致しない場合は discard
- これにより worker が cancel 漏れで送ってきた古い結果は engine 側で確実に discard される

## §8 cancel propagation 規約

### §8.1 cancel trigger 5 種

| Trigger | 動作 | active_request 扱い |
|---------|------|---------------------|
| `focus_out` | engine state を Idle へ強制遷移、commit_history clear | take + cancel_token.cancel() |
| preedit 内編集(typing / backspace)| 直前 RankRequest を cancel し新 RankRequest 開始 | 入れ替え |
| 連続 space(別 RankRequest 開始)| 同上 | 入れ替え |
| `Esc` key | 候補ウィンドウ閉、preedit kana 維持(`LiveConverting` 状態へ復帰) | take + cancel_token.cancel() |
| `IMEEngine::reset()` | host からの強制 reset、`focus_out` と同等処理 | take + cancel_token.cancel() |

### §8.2 backend 別 cancel propagation

| backend | cancel 受領 method | cancel 受領 latency |
|---------|-------------------|--------------------|
| SudachiDict prefix lookup | check 不要(完了まで実行)| 即(< 1ms 内に自然完了) |
| UserVocab find_by_prefix | 同上 | 即 |
| LearningCacheReader::lookup | 同上 | 即 |
| LLM convert(`llama.cpp`)| `cancel.is_cancelled()` を 10 token 毎に check | 最悪 100-500ms(10 token = 10-50ms × 10、§13 Open Q 4) |

### §8.3 cancel 後の result handling

- worker 側で cancel 検出後の `sink.send` は **engine 主 thread に到達してから** request_id mismatch で discard される(§7.5)
- engine 主 thread の receiver は cancel と無関係に常時 listen 状態、実装の単純化を優先
- LLM が cancel 後も継続生成する token は計算 cost が無駄になるが UX 影響なし(将来 metrics で「cancel ramp time」を観測予定、§13 Open Q 4)

## §9 error handling と observability

### §9.1 error category と対応

| 障害 | 検出 layer | 対応 |
|------|-----------|------|
| IBus disconnection(D-Bus session 切断) | `kotoha-engine-ibus` | `kotoha-bin` の event loop が D-Bus error を catch、process exit code 75(EX_TEMPFAIL)で systemd 再起動を期待 |
| Ranker worker thread panic | `KotohaEngine` | `mpsc::Receiver::recv` が `RecvError` を返したら panic 検出、`tracing::error` + worker thread 再生成、現 RankRequest は失敗扱いで `host.hide_candidate_window()` |
| Ranker individual backend error(LLM タイムアウト等)| Ranker 内部 | `tracing::warn` + partial 候補のみで continue、全 backend 全滅なら空 Replace を engine に送り `tracing::error` |
| Storage error(SQLite poison / disk full)| storage layer | `unwrap_or_else(PoisonError::into_inner)` 既設(PR #111)、`record_choice` の disk full は `Err` を返すが engine は **commit を成功させた上で** `tracing::warn`(学習失敗が変換失敗にならない、global feedback policy `silent_failure`)|
| Engine internal panic | top-level `catch_unwind` | `kotoha-bin` で thread panic catch → `IMEEngine::reset()` → process continue。session 全断は避ける |
| `RomajiConverter` invariant 違反 | `kotoha-core::romaji` | panic-free、不正入力は `KeyEventResult::Forwarded` で host に戻す |

### §9.2 observability(`tracing` 規約)

| level | 用途 | 例 |
|-------|------|----|
| `ERROR` | recoverable だが user 影響あり、または unrecoverable | LLM 全滅で空候補、IBus 切断 |
| `WARN` | 観測価値あり、user 影響なし | record_choice 失敗、LLM 個別失敗で partial 候補、cancel ramp time 超過 |
| `INFO` | lifecycle event | engine enable/disable、focus_in/out、process 起動/終了 |
| `DEBUG` | 状態遷移 | KotohaEngine 状態遷移、RankRequest 採番、coalescing window 経過 |
| `TRACE` | 高頻度 event | keypress 受信、RomajiConverter pending 更新、Ranker individual backend completion |

`tracing-subscriber` で env var `KOTOHA_LOG=info` で起動、開発時は `debug` / `trace` 切替可能とする。

### §9.3 silent failure 禁止

global `feedback_silent_failure.md` 規約に従い、以下を禁止する:

- catch して空候補 Vec 返却して終わる(必ず `tracing::warn` 以上を残す)
- `Result` を `unwrap_or_default()` して error を握り潰す(明示的な error path で `tracing` 観測)
- `panic!` を catch して continue する(top-level catch_unwind 以外は禁止)

例外:`record_choice` の Err は commit 成功優先のため WARN 残しで吸収する(global policy の例外、本 spec §9.1 表で明示)。

## §10 testing strategy

### §10.1 5-layer 構成

| Layer | Target | 手法 | 配置 |
|-------|--------|------|------|
| **L1 unit** | `ConversionContext` 構築、`CandidateUpdate` 差分処理、`CancellationToken` std::sync impl、`KotohaEngine` 状態遷移 table | `MockHostBridge` + `MockRanker` を inject、各遷移ごとに状態 snapshot を assert | `crates/kotoha-engine-core/src/**/*.rs` 内 `#[cfg(test)]` |
| **L2-core integration** | `KotohaEngine` + 実 `Ranker` impl(P2-D 完成後)+ `MockHostBridge`、typing → space → commit を end-to-end 実行 | `MockHostBridge` への呼び出し sequence を assert(操作ログ pattern) | `crates/kotoha-engine-core/tests/` |
| **L2-adapter integration** | `IBusHostBridge` を mock IBus daemon に接続、D-Bus call が正しく発行されるか | test 実行時に mock IBus daemon を spawn(`ibus-daemon --replace --xim` の test instance、またはより軽量に zbus mock service)、lefthook pre-push test 内で実行 | `crates/kotoha-engine-ibus/tests/` |
| **L3 manual smoke** | GNOME Wayland session で実際に Kotoha を起動、Firefox / GNOME Text Editor / VS Code で典型変換 10 件 | 手動実行ログ | `docs/wbs/<date>-phase3a-smoke.md` |
| **Regression** | Phase 1 Layer 3 smoke 14/15 を engine 経由でも維持 | 既存 fixture を engine adapter で wrap した integration test | `crates/kotoha-engine-core/tests/regression_phase1.rs` |

### §10.2 L1 unit test の coverage 要件

`KotohaEngine` 状態遷移 table(§5.2)の **全 row が test で網羅** されること。各 row につき以下を assert:

- 遷移後の engine 状態が期待値
- `MockHostBridge` への call sequence が期待 sequence と一致
- `MockRanker` への RankRequest 送信回数 / cancel 回数が期待値
- `commit_history` の長さが invariant(≤ 200)を保つ

### §10.3 L2-core integration の代表シナリオ

| シナリオ | 期待 sequence |
|---------|--------------|
| typing「kotoha」→ space → top 候補 Enter | update_preedit×7(逐次) → show_candidate_window → update_candidates(Replace) → commit_text("琴葉") → record_choice("ことは", "琴葉") → hide_candidate_window |
| typing「shi」→ backspace → typing「a」 | update_preedit "し" → update_preedit "" → update_preedit "あ"(Live RankRequest 各 1 回 + cancel 適切に発火) |
| space → 候補表示 → 別 word typing 再開 | show_candidate_window → update_candidates → hide_candidate_window → update_preedit(新 word kana)→ Live RankRequest 再起動 |
| focus_out 中の cancel | active_request.cancel が呼ばれ、commit_history が clear、hide_candidate_window |

### §10.4 testing baseline 要件

Phase 3-A 本番実装完了時、以下を baseline とする:

- 既存 baseline 340 PASS(`cargo test --workspace --features kotoha-storage/test-helpers`)を **0 regression**
- 新規追加 test 数: L1 で約 30 件(状態遷移 table row 数 × 1-2)、L2-core で約 10 件、Regression で 15 件(Phase 1 14/15 + 1 smoke)、合計 +55 件想定
- L3 manual smoke は merge 直前に手動実行、`docs/wbs/` にログ

### §10.5 mock 実装の責務

| Mock | 配置 | 責務 |
|------|------|------|
| `MockHostBridge` | `kotoha-engine-core/src/testing.rs`(or `tests/common/`) | 全 IMEHostBridge method を Vec<Operation> に記録、test から sequence 取得可能 |
| `MockRanker` | 同上 | 設定可能な fixed candidates を即時 sink.send、cancel 観測カウンタ保持 |
| Mock IBus daemon | adapter test | zbus 経由で IBus interface を fake する service、または `ibus-daemon --replace --xim` spawn |

## §11 implementation roadmap(高レベル sequence)

本 spec は spec-only PR として merge し、後続 milestone を以下の順で進める。詳細 plan は本 spec merge 後に `docs/superpowers/plans/` で別途起票する。

```text
[本 spec PR(small tier、code change 0)]
   ↓
[Phase 0 prerequisite tasks(§12)]
   ├─ Task A: RomajiConverter::reset_pending() 追加(別 ISSUE)
   └─ Task B: trie 3 方式並立確認 + 不足 entry 追加(別 ISSUE、A と統合可能)
   ↓
[P2-D Ranker 実装(本 spec の Ranker trait に依存)]
   ├─ kotoha-engine-core::Ranker / ConversionContext / CandidateUpdate trait 定義のみ先行追加(P2-D で実装) 
   ├─ SudachiDict + UserVocab + LearningCache + LLM の Hybrid Ranker
   └─ ranker module の test
   ↓
[Phase 3-A 本番実装]
   ├─ kotoha-engine-core crate 作成、KotohaEngine 状態機械 + RankerWorker + cancel 抽象
   ├─ kotoha-engine-ibus crate 作成、IBusHostBridge + IBusEventDispatcher
   ├─ kotoha-bin crate 作成、main + host hard-code 起動
   ├─ L1 / L2-core / L2-adapter test 整備
   └─ L3 manual smoke
```

各 milestone の estimated size:

| milestone | size 想定 | tier |
|-----------|----------|------|
| 本 spec PR | docs only、~30 KB | Small(spec-only) |
| Phase 0 prerequisite Task A | 1 file、~50 lines | Small |
| Phase 0 prerequisite Task B | 1-2 file、~100-300 lines | Small |
| P2-D Ranker | 5-10 file、~800-1500 lines | **Medium 想定、必要なら複数 PR split** |
| Phase 3-A 本番 | 10-20 file、~1500-2500 lines | **Large、複数 PR split 必須(crate 単位)** |

## §12 prerequisite tasks

本 spec が依存する Phase 0 RomajiConverter の追補事項。本 spec merge 後、以下を別 ISSUE で起票し P2-D 着手前に完了させる:

### §12.1 prerequisite Task A: `RomajiConverter::reset_pending()`

- **責務**: 内部 pending romaji buffer を clear する(kana buffer は engine 側責務、§6.2 backspace path 参照)
- **動機**: Phase 3-A backspace path で engine 側 preedit kana を `pop()` した直後、pending romaji を Idle 状態に揃えるため。Single Responsibility に従い、RomajiConverter は **romaji 状態のみ**、engine は **kana 状態のみ** を管理する
- **想定 API**:
  ```rust
  impl RomajiConverter {
      /// pending romaji buffer を空にする。
      /// 例: 「sh」を typing 後 backspace で「し」を削除する場合、engine が
      /// preedit kana から「し」を pop した後、本 method で「sh」も clear する。
      /// 既に空なら no-op。
      pub fn reset_pending(&mut self);
  }
  ```
- **test**: 「sh」pending 中の reset で空になること、空 buffer での reset が no-op、`reset_pending()` 後に新 char 入力が独立判定されること(「sha」期待が「a」=「あ」になる)の 3 ケース最低

### §12.2 prerequisite Task B: trie 3 方式並立確認

- **責務**: ADR 0005 trie に kunrei 式(`si` / `ti` / `tu`)/ Hepburn 式(`shi` / `chi` / `tsu`)/ waapuro 式(`sya` / `tya` 等)が全 entry 登録されているか empirical 確認
- **動機**: Phase 3-A typing path で user の romaji 入力慣行(kunrei 派 / Hepburn 派 / waapuro 派)を全方式並立で受容するため
- **検証方法**:
  1. `crates/kotoha-core/src/romaji/rules.rs` の RULES table を inspect
  2. golden fixture(`crates/kotoha-core/tests/fixtures/romaji_cases.tsv`)に 3 方式 entry が含まれることを確認
  3. 不足分は entry 追加 + golden fixture 拡張

Task A と Task B は実装規模が小さく(各 1 PR)、統合 PR にしても良い。

## §13 Open Questions

本 spec で確定せず、Phase 3-A 実装段階で empirical 確定する事項。実装中に決まり次第、別 ISSUE で記録する。

| # | 質問 | 暫定値 | 確定 method |
|---|------|--------|------------|
| 1 | `commit_history` の保持文字数 N | 200 chars(LLM context window と Phase 5 model spec から逆算)| Phase 3-A 本番実装で 100 / 200 / 400 で AB test、user 主観 + LLM prompt size measure |
| 2 | coalescing window の最適値 | typing 5-10ms、commit 30ms | Phase 3-A 本番実装で IBus host 描画 frame rate と実機 measure |
| 3 | IBus `update_lookup_table` 高頻度呼び出しが GNOME Mutter で flicker を起こすか | empirical 未測定 | Phase 3-A 実装段階の最初の verification task で 2-3 回連続 update を観察 |
| 4 | LLM cancel propagation の granularity | 10 token 毎 check | Phase 3-A 本番実装で cancel ramp time を metrics で観測、5 / 10 / 20 token で比較 |
| 5(prerequisite)| Phase 0 RomajiConverter に `reset_pending()` 追加 | §12.1 Task A | 別 ISSUE で起票し本 spec merge 後 immediate に対応 |
| 6(prerequisite)| Phase 0 RomajiConverter trie が kunrei/Hepburn/waapuro 3 方式並立か | §12.2 Task B | 別 ISSUE 起票 |
| 7 | typing 中 LLM invocation を投げるか / dict only にするか | 投げる(best-effort、cancel propagation 受容)| Phase 3-A 本番実装 + Phase 1 Gemma で empirical、Phase 5 custom model 来たら再評価 |
| 8 | `KeyModifiers` の IBus 完全 mapping | bitflags 暫定 4 種(Shift/Ctrl/Alt/Super)| IBus IBusModifierType 全列挙を実装段階で対応 |
| 9 | adapter 内 Mutex<Vec<Candidate>> internal buffer の同時編集競合 | Phase 3-B B0h-d で `Arc<Mutex<dyn IMEEngine>>` 化済(dispatcher + engine 両方が thread-safe)。adapter 側 buffer も `Mutex<Vec<Candidate>>` 保持で thread-safe | **closure**(B0h-d, B2):"best-effort 単一thread 想定" は撤回。multi-thread D-Bus signal listener(B3)に対応した lock 設計済 |

## §14 forward direction(Phase 4 / 5 / 6)

本 spec の trait 設計が以下の future direction で **trait 変更なし** で extend 可能であることを記録する。

### §14.1 Phase 4 fcitx5 adapter

- 新 crate `kotoha-engine-fcitx5` を追加し `IMEHostBridge` を impl
- `kotoha-bin::main` に host detection logic(env var / D-Bus name)を追加
- `kotoha-engine-core` は **無変更**

### §14.2 Phase 5 custom romaji-base model

- `Ranker` impl 内部で Phase 5 custom model を 1 backend として追加
- `ConversionContext` に `partial_input: Option<String>` / `typo_distance: u32` field を追加(non-breaking、defaults で既存 caller 影響なし)
- `CandidateUpdate::Remove(Range)` variant の本格活用(beam search で beam 削減 → 既存 candidate 範囲削除)
- Live 変換が partial-input + beam search で本格化、`ConversionMode::Live` の挙動が Phase 5 model 由来で大幅高度化

### §14.3 Phase 6 advanced features

- 再変換(reconvert): IBus `text-input-v3` SetSurroundingText 経由で application から周辺 text 取得、`IMEEngine` に新 method `reconvert(&mut self, surrounding: &str, cursor: usize)` を追加(non-breaking)
- per-application context separation: `ConversionContext` に `app_id: Option<String>` field を追加(non-breaking)
- 設定 UI: `kotoha-bin` に GTK / Adw 設定ダイアログを追加、または別 `kotoha-config` binary

## 参考 references

- handoff: `.claude/projects/-home-kohshiro-develops-student-kotoha-ime/memory/project_session_handoff_2026-04-28.md`
- ROADMAP: `docs/ROADMAP.md` Phase 3 entry
- Phase 2 design spec: `docs/superpowers/specs/2026-04-25-kotoha-phase-2-design.md` §3.3 / §11
- P2-C design spec: `docs/superpowers/specs/2026-04-26-p2-c-learning-cache-design.md`
- ADR 0005: `docs/adr/0005-romaji-trie-over-hashmap.md`
- ADR 0011: `docs/adr/0011-kanji-backend-trait-design.md`
- ADR 0014: `docs/adr/0014-phase-2-dictionary-layer-architecture.md`
- ADR 0015: `docs/adr/0015-kotoha-storage-sqlite-adoption.md`
- glossary: `docs/wiki/glossary.md`
- IBus protocol: https://github.com/ibus/ibus/wiki/IBusEngine
- fcitx5(future ref): https://github.com/fcitx/fcitx5/wiki

## 改訂履歴

| 日付 | revision | 内容 |
|------|----------|------|
| 2026-05-02 | r1 | 初版 draft、ISSUE #116 |
| 2026-05-04 | r2 | Phase 3-B B2 wire format 実装方針を §4.2 に追記、§13 Open Q 9 を B0h-d 結果で closure |
