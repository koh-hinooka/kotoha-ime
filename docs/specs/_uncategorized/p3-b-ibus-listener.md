---
feature: p3-b-ibus-listener
status: draft
bounded_context: _uncategorized
related_issues: ["#136", "#195"]
related_prs: []
glossary_refs: ["coalescing-window", "event-loop", "event-reactor", "ime-engine", "ime-host-bridge", "phase3-ibus-engine-terms", "preedit"]
last_reviewed: 2026-05-08
---

# Phase 3-B B6-b: IBus engine method dispatch listener

| 項目 | 値 |
|------|----|
| Phase | Phase 3 (IBus integration) — milestone P3-B、sub-task B6-b |
| ISSUE | [#195](https://github.com/std-koh-hinooka/kotoha-ime/issues/195) (parent: [#136](https://github.com/std-koh-hinooka/kotoha-ime/issues/136)) |
| 起票日 | 2026-05-08 |
| Status | Draft |
| 関連 ADR(候補) | 0021(zbus integration architecture for IBus engine listener) |
| 前提 spec | `docs/specs/_uncategorized/p3-a-ibus-engine.md`、`docs/adr/0020-event-loop-architecture.md` |
| 前提 PR | #183 (B0h-f + B3 event-loop)、#187 (ShutdownObserver)、#193 (RequestId)、#198 (SIGTERM hook) |

## 目次

1. [§1 概要](#1-概要)
2. [§2 スコープと範囲外](#2-スコープと範囲外)
3. [§3 architecture](#3-architecture)
4. [§4 `KotohaEngineService` API contract](#4-kotohaengineservice-api-contract)
5. [§5 data flow(`process_key_event` 経路)](#5-data-flowprocess_key_event-経路)
6. [§6 respond channel と 100ms timeout 規約](#6-respond-channel-と-100ms-timeout-規約)
7. [§7 IBus daemon integration](#7-ibus-daemon-integration)
8. [§8 削除コード](#8-削除コード)
9. [§9 error handling と observability](#9-error-handling-と-observability)
10. [§10 testing strategy](#10-testing-strategy)
11. [§11 implementation roadmap](#11-implementation-roadmap)
12. [§12 spec / ADR の更新箇所](#12-spec--adr-の更新箇所)
13. [§13 forward direction](#13-forward-direction)

---

## §1 概要

本 spec は Phase 3-B の sub-milestone **B6-b**(zbus `MessageStream` decode + IBus engine factory registration)の設計書である。

Phase 3-B B0h-f + B3 (PR #183, ADR 0020) で 4-thread topology の architectural skeleton が完成したが、`kotoha-engine-ibus::listener::run` は zbus message stream の decode を実装せず、`KOTOHA_ALLOW_LISTENER_STUB=1` で起動を許可する stub に留まっていた。本 spec は listener thread を **IBus daemon からの method 呼び出しを実 receive + decode する production 実装** に置き換える設計を定める。

本 spec の workflow 上の位置:

```text
[#198 SIGTERM hook merge 済] → [本 spec 起票] → [implementation plan 起票 / writing-plans skill] → [実装 PR (#195)]
                                                                                                   ↓
                                                               [#196 L3 manual smoke (実機検証)]
                                                                                                   ↓
                                                                              [v0.3.0 milestone close]
```

本 spec scope 完了後、Phase 3-B の残課題は **B6-c (#196 L3 manual smoke 実機検証)** のみとなる。

---

## §2 スコープと範囲外

### §2.1 本 spec scope

- zbus 5 `#[interface]` macro による IBus 1.5.x engine method dispatcher 実装
- 以下 6 method の dispatch:
  - `ProcessKeyEvent(u keyval, u keycode, u state) -> b`(主要、戻り値 b は Kotoha が消費したか)
  - `Reset()` / `FocusOut()` / `Disable()`(reset 系 3 種)
  - `FocusIn()` / `Enable()`(no-op stub、daemon 側 introspection 互換のため)
- `org.freedesktop.IBus.Engine.Kotoha` bus name 取得(`RequestName` via `blocking::connection::Builder::name`)
- `/org/freedesktop/IBus/Engine/Kotoha` object path での service 登録
- `Event::IBusKey { event, respond }` enum 拡張(respond channel)
- engine_loop の `Event::IBusKey` arm に `respond.send(KeyEventResult)` 経路追加
- `KOTOHA_ALLOW_LISTENER_STUB_ENV` / `allow_listener_stub` / `ListenerStubRefused` 削除
- ADR 0021 起票(zbus integration architecture for IBus engine listener)

### §2.2 本 spec 範囲外

- **L3 manual smoke 実機検証**(別 ISSUE [#196](https://github.com/std-koh-hinooka/kotoha-ime/issues/196))
- **PageUp / PageDown / CursorUp / CursorDown / CandidateClicked 等の高度 method**(Phase 5 / 6 で UI 機能拡張時に追加)
- **`/usr/share/ibus/component/kotoha.xml` の component 記述ファイル**(installer / packaging 段階で扱う、現状は `cargo run --bin kotoha` で開発時 ibus daemon と直接対話する想定)
- **Phase 4 fcitx5 adapter**(別 Phase)
- **Phase 5 partial-input + beam search**(別 Phase)

---

## §3 architecture

### §3.1 thread topology は ADR 0020 を継承

ADR 0020 §採択 Q4 で確定した 4-thread topology を維持する:

| thread | 主責務 | 本 spec での変更 |
|---|---|---|
| `main` | DI wiring、SIGTERM/SIGINT、join | (変更なし、#198 で SIGTERM hook 完成済) |
| `kotoha-dbus-listener` | IBus method 受信 + decode + bridge channel 送信 | **本 spec scope**。stub から production 実装へ |
| `kotoha-engine-loop` | `EventReactor::recv()` で multiplex、apply_candidate_update | `Event::IBusKey` arm の respond.send 経路追加 |
| `kotoha-ranker-worker` | `Event::WorkerOutput` 生成 | (変更なし) |

### §3.2 listener thread の内部構造

```text
[IBus daemon] --DBus method calls--> [zbus::blocking::Connection]
                                        ↓ (内部 smol executor、zbus が管理)
                                        ↓ (#[interface] dispatcher)
                                   [KotohaEngineService struct]
                                     - bridge_tx: Sender<Event>
                                        ↓ bridge_tx.send(Event::IBusKey { event, respond })
                                   [crossbeam unbounded channel: bridge]
                                        ↓
                                   [kotoha-engine-loop thread]
                                        ↓ engine.process_key_event(event)
                                        ↓ respond.send(KeyEventResult)
                                   [crossbeam bounded(1) channel: response]
                                        ↓
                                   [KotohaEngineService::process_key_event]
                                        ↓ bool 返却
                                   [zbus が daemon に反映]
```

`blocking::Connection` は zbus 内部の smol executor を背景 thread で走らせる。`#[interface]` impl の各 method は smol executor 経由で dispatch される(本 crate のコード自身は std::sync 同期 primitive のみ使用)。

### §3.3 Connection ownership

listener thread は `blocking::Connection` を **単独所有** する(`let conn = ...build()?` で local に bind)。Connection drop 時に zbus の内部 executor が exit し、bus name が release される。

`ShutdownObserver` を polling し、shutdown 観測時に Connection を drop して thread を抜ける。

---

## §4 `KotohaEngineService` API contract

### §4.1 配置と visibility

- 配置: `crates/kotoha-engine-ibus/src/service.rs`(新規 module)
- visibility: `pub(crate) struct KotohaEngineService`(crate 外部に export しない)
- `listener::run` 内のみで構築・使用される

### §4.2 struct 定義

```rust
pub(crate) struct KotohaEngineService {
    /// engine-loop thread に Event を送る単方向 channel。
    bridge_tx: crossbeam_channel::Sender<Event>,
}
```

`bridge_tx` 以外の field は持たない:

- engine state を direct に読まない(ADR 0020 §採択 Q4 lock-free 原則)
- `ShutdownObserver` は `listener::run` の poll loop 専用、service struct は知らない
- KeyEvent 変換 helper は既存 free function(`crate::keysym::from_ibus(keysym, keycode, state) -> KeyEvent`、Phase 3-B B4 実装済)を使う

### §4.3 `#[interface]` impl

`#[interface(name = "org.freedesktop.IBus.Engine")]` で 6 method を実装する:

| method | signature | 内部動作 |
|---|---|---|
| `process_key_event` | `(u32, u32, u32) -> bool` | `Event::IBusKey { event, respond }` 送信 + 100ms blocking recv → bool |
| `focus_out` | `() -> ()` | `Event::IBusReset(IBusResetKind::FocusOut)` 送信(応答不要)|
| `reset` | `() -> ()` | `Event::IBusReset(IBusResetKind::Reset)` 送信 |
| `disable` | `() -> ()` | `Event::IBusReset(IBusResetKind::Disable)` 送信 |
| `focus_in` | `() -> ()` | `tracing::debug!`(daemon の introspection 互換用 stub)|
| `enable` | `() -> ()` | `tracing::debug!`(同上)|

`#[interface]` macro が生成する method は同期(blocking)で書く。zbus 5 の smol executor が background thread で dispatch を回すため、本 crate コード自身は async/await を使わない(ADR 0017 / ADR 0020 の "core は tokio 非依存" 原則維持)。

### §4.4 IBus 1.5.x 仕様準拠点

- interface 名: `org.freedesktop.IBus.Engine`(`crates/kotoha-engine-ibus/src/proxy.rs::IBUS_ENGINE_INTERFACE` と共有)
- `ProcessKeyEvent` の signature `uuu` は IBus 1.5.x `bus/inputcontext.c` 仕様に準拠
- 戻り値 `b`(true = 消費した、false = アプリに forward する)も同仕様

---

## §5 data flow(`process_key_event` 経路)

### §5.1 listener 側 dispatch

```rust
fn process_key_event(&self, keyval: u32, keycode: u32, state: u32) -> bool {
    // 既存 helper: crates/kotoha-engine-ibus/src/keysym.rs::from_ibus
    // (Phase 3-B B4 で IBusModifierType → KeyModifiers full mapping 実装済)
    let event = crate::keysym::from_ibus(keyval, keycode, state);
    let (resp_tx, resp_rx) = crossbeam_channel::bounded::<KeyEventResult>(1);
    if self
        .bridge_tx
        .send(Event::IBusKey { event, respond: resp_tx })
        .is_err()
    {
        // engine-loop 側 receiver drop = engine 死亡。app へ forward (= keystroke 失わない)
        tracing::warn!(
            error_id = "listener.process_key_event.bridge_disconnected",
            "engine-loop disconnected; forwarding key to app"
        );
        return false;
    }
    match resp_rx.recv_timeout(Duration::from_millis(100)) {
        Ok(KeyEventResult::Consumed) => true,
        Ok(KeyEventResult::Forwarded) => false,
        Err(_) => {
            tracing::warn!(
                error_id = "listener.process_key_event.timeout",
                "engine response timeout (>100ms); forwarding key to app"
            );
            false
        }
    }
}
```

### §5.2 engine_loop 側 dispatch

```rust
Ok(Event::IBusKey { event, respond }) => {
    let result = engine.process_key_event(event);
    // listener が timeout した場合 send は Err、ignore (= key は app に forward 済)
    let _ = respond.send(result);
}
```

`KeyEventResult` は engine 状態機械の出力(spec §5.2)で、`Consumed` / `Forwarded` の 2 variant を持つ既存型(`crates/kotoha-engine-core/src/key_event.rs`)。

---

## §6 respond channel と 100ms timeout 規約

### §6.1 timeout 値の根拠

100ms timeout は以下を考慮した値:

| ケース | engine_loop 処理時間 | 100ms timeout 内に収まるか |
|---|---|---|
| 通常 | ~1ms(state machine + worker channel send + respond.send) | ✓ |
| heavy(30 候補 Replace + filter) | ~5-10ms | ✓ |
| recovery(worker 死亡 → 再 spawn) | ~30-50ms | ✓ |
| bug(>100ms) | timeout fire → app へ forward + tracing::warn | (fallback 経路) |

10 keys/sec ピーク typing(inter-keystroke 100ms)に対し、通常 1ms / heavy 10ms / recovery 50ms 全てを 100ms 内で吸収する。fast typist の DX を損なわない。

### §6.2 timeout fire 時の double-input 懸念

timeout 後に engine が eventual に event を処理して `Consumed` 判定した場合、preedit が表示されつつ app も raw key を受信する double-input 状態が原理的にありうる:

- 発生条件: engine_loop が 100ms 以上 hang(= bug 状態)
- 観測経路: `error_id = "listener.process_key_event.timeout"` を log filter で監視
- mitigation: bug 状態を fix する。本 spec scope では `respond.send` Err を engine 側で観測したら処理を abort する deadline check は **入れない**(複雑性 vs benefit が見合わない)
- Phase 5 / 6 で engine が重くなる場合は再評価し、必要なら `Event::IBusKey { event, respond, deadline: Instant }` への拡張を検討する

### §6.3 `bounded(1)` の選択理由

response channel は `bounded(1)`(1 element capacity):

- engine 側は必ず 1 度だけ `respond.send` を呼ぶ → capacity 1 で十分
- listener は必ず 1 度だけ `recv_timeout` を呼ぶ
- unbounded だと send が blocking しない代わりに backpressure 検出機会を失う。bounded(1) は send / recv 1:1 を構造的に enforce する

---

## §7 IBus daemon integration

### §7.1 bus name と object path

```rust
pub(crate) const IBUS_ENGINE_BUS_NAME: &str = "org.freedesktop.IBus.Engine.Kotoha";
pub(crate) const IBUS_ENGINE_OBJECT_PATH: &str = "/org/freedesktop/IBus/Engine/Kotoha";
```

新規定数として `crates/kotoha-engine-ibus/src/proxy.rs` に追加(既存 `IBUS_ENGINE_INTERFACE` と同じ module)。

### §7.2 connection 構築

```rust
let connection = blocking::connection::Builder::session()?
    .serve_at(IBUS_ENGINE_OBJECT_PATH, KotohaEngineService { bridge_tx })?
    .name(IBUS_ENGINE_BUS_NAME)?
    .build()?;
```

`build()` 完了時点で:

- session bus への connection 確立
- service struct が指定 path で publish
- bus name `RequestName` で daemon に登録(他 process が同名で publish していたら `Err` で listener 起動失敗)

### §7.3 `org.freedesktop.IBus.IBus.RegisterComponent` の取扱

IBus 1.5.x では engine 登録の正規経路は `/usr/share/ibus/component/<name>.xml` ファイル配置 + ibus daemon 再起動である。`RegisterComponent` DBus method 経由の dynamic 登録もあるが、daemon 側が file ベースの registry を優先するため一般的でない。

本 spec scope では:

- DBus call 経路の `RegisterComponent` は **使用しない**
- `kotoha.xml` 配置経路は **packaging task として後続 ISSUE で扱う**(本 spec 範囲外、§2.2)
- 開発時は手動で `~/.config/ibus/component/kotoha.xml` を配置するか、ibus daemon を直接 reset して name 取得状態を観測する(L3 manual smoke #196 で検証)

---

## §8 削除コード

本 spec で listener が production 実装になるため、以下の stub safety net を完全削除する:

| 削除対象 | 配置 | 理由 |
|---|---|---|
| `KOTOHA_ALLOW_LISTENER_STUB_ENV` 定数 | `listener.rs` | listener が機能するため env var 不要 |
| `allow_listener_stub()` 関数 | 同上 | 同上 |
| `ListenerStubRefused` struct + `thiserror::Error` impl | 同上 | 同上 |
| stub 起動時 `tracing::error!`(STUB mode 警告)| 同上 | listener が機能するため警告不要 |
| 既存 stub poll loop(`while !shutdown.load() { sleep(50ms) }`)| 同上 | service 経由 dispatch に置換 |
| 既存 unit test 2 件(`run_refuses_to_start_without_env_var` / `run_exits_within_200ms_after_shutdown_request`)| 同上 | stub 自体が消えるため obsolete |

`ShutdownObserver` 関連 unit test 3 件(`observer_reflects_trigger_state` / `cloned_observer_shares_state` / `cloned_trigger_shares_state`)は **維持**(SIGTERM hook の正当性を pin する)。

---

## §9 error handling と observability

### §9.1 listener 経路の error 表

| 状況 | listener 動作 | observability |
|---|---|---|
| zbus connection 確立失敗 | `run()` Err で main thread に伝播 → process exit | `tracing::error!` + `?` propagation |
| `RequestName` 失敗(他 process が同名占有)| 同上 | 同上 |
| service method 内 panic | `&self` impl で field なし、panic 起こりにくい。万一発生時は zbus が catch して daemon に Err 返却、connection は維持 | zbus 標準 |
| bridge_tx.send Err(engine_loop drop)| `false` 返却 + `error_id = "listener.process_key_event.bridge_disconnected"` | tracing::warn |
| respond.recv timeout(>100ms)| `false` 返却 + `error_id = "listener.process_key_event.timeout"` | tracing::warn |
| ShutdownObserver `is_shutting_down() == true` | poll loop 抜ける、Connection drop | `tracing::info!` |

### §9.2 spec §9.3「silent failure 禁止」整合

- timeout は warn level + `error_id` 付きで観測経路を残す(bug 早期検出)
- bridge_tx disconnect は warn level(IME を黙って disable しない)
- 通常経路の successful dispatch は `tracing::trace!` のみ(hot path、log volume 抑制)

---

## §10 testing strategy

### §10.1 layer 別 test

| Layer | テスト対象 | 配置 | gating |
|---|---|---|---|
| L1 unit | `KotohaEngineService::process_key_event` の正常 / timeout / disconnect 経路 | `crates/kotoha-engine-ibus/src/service.rs` `#[cfg(test)]` | 無条件(mock bridge_tx) |
| L1 unit | `KotohaEngineService::{focus_out, reset, disable, focus_in, enable}` の Event 送信 | 同上 | 同上 |
| L1 unit | `Event::IBusKey { event, respond }` の destructure と engine_loop 経路 | `crates/kotoha-bin/src/engine_loop.rs` `#[cfg(test)]` または既存 integration test | mock reactor + mock host |
| L2 integration | listener thread + engine_loop + dummy bridge channel 経路で `process_key_event` が `true`(= Consumed)を返す | `crates/kotoha-engine-ibus/tests/integration.rs`(新規) | `#[ignore]`(dbus session bus 必要)|
| L3 manual | 実機 IBus daemon に register、Firefox / GNOME Editor / VS Code で型変換 10 件 | `docs/wbs/<date>-phase3b-b6-l3-smoke.md` | 別 PR(#196)|

### §10.2 既存 test の preservation

- `crates/kotoha-engine-reactor-linux/tests/integration.rs`(shutdown / fan-in / bursty / explicit_shutdown)は **無修正で pass** すべき
- `crates/kotoha-engine-ibus/src/listener.rs::tests` の `ShutdownObserver` 系 3 件は維持
- engine_loop 経路の既存 test(`engine_wrap_hybrid` 等)は `Event::IBusKey` の variant 形式変更に追随した update が必要

### §10.3 mock 戦略

- `KotohaEngineService` の test では bridge_tx を `crossbeam_channel::unbounded()` の `Sender` で構築し、test 内で `Receiver` を直接 drain して送信内容を assert
- L2 integration の dbus session bus は CI 環境差異の温床なので `#[ignore]` で opt-in
- production-like test は L3 manual smoke(#196)で実施

---

## §11 implementation roadmap

実装は以下の順序で進める(各 step は同 PR 内、commit 単位):

1. **Event::IBusKey 変更**: `crates/kotoha-engine-core/src/reactor/event.rs` の `Event::IBusKey(KeyEvent)` を `Event::IBusKey { event: KeyEvent, respond: Sender<KeyEventResult> }` に変更。callers(engine_loop + tests)を update
2. **proxy.rs 定数追加**: `IBUS_ENGINE_BUS_NAME` / `IBUS_ENGINE_OBJECT_PATH`
3. **service.rs 新規**: `KotohaEngineService` struct + `#[interface]` impl(6 method)+ unit tests
4. **listener.rs rewrite**: `KOTOHA_ALLOW_LISTENER_STUB` 関連削除、`run()` を `blocking::connection::Builder` 経路に書き換え
5. **engine_loop.rs 更新**: `Event::IBusKey { event, respond }` の destructure + `respond.send(result)` 追加(`crates/kotoha-bin/src/engine_loop.rs:43-46` の `let _result = engine.process_key_event(key);` を `let result = ...; let _ = respond.send(result);` に置換)
6. **kotoha-bin/src/main.rs**: 本 spec scope では追加変更不要(SIGTERM hook は #197 で配線済、`KOTOHA_ALLOW_LISTENER_STUB=1` 起動 hint のような stub 関連表現も既に main.rs に残っていない)
7. **L1 / L2 test 追加**
8. **ADR 0021 起票**
9. **`docs/specs/_uncategorized/p3-a-ibus-engine.md` の §6.1 step [1] / §11 acceptance criteria 更新**
10. **ROADMAP の Phase 3-B sub-milestone table 更新**(B6-b 完了マーク、B6-c 残)

PR size 想定: ~400-600 lines(spec + source + test + ADR)。Large tier 上限 ≤1000 lines に収まる。

---

## §12 spec / ADR の更新箇所

### §12.1 ADR 0021(新規)

`docs/adr/0021-zbus-integration-architecture-for-ibus-listener.md` を起票:

- 採択: zbus 5 `#[interface]` macro + `blocking::connection::Builder::serve_at + name` + bounded response channel + 100ms timeout
- rejected:
  - `MessageStream` 手動 dispatch(zbus 5 では async API 主導、blocking 系で MessageStream 直接受信は限定的)
  - `Arc<AtomicBool>` snapshot による listener-side enabled cache(spec §5.2 `Forwarded` 精度を犠牲)
  - `ForwardKeyEvent` signal 二相 pattern(API surface + race + integration test 負荷増)

### §12.2 spec p3-a-ibus-engine.md の更新

- §6.1 step [1] の listener stub 表現を実装済表現に更新
- §11 acceptance criteria の listener 関連項目を満了マーク
- §13 Open Q 2(coalescing window 値)は #196 manual smoke で確定 → status: open のまま維持

### §12.3 ROADMAP

`docs/ROADMAP.md` の Active milestone v0.3.0 table:

- ISSUE #195 entry(B6-b)を `[x]`
- Phase 3-B sub-milestone table の B6 entry を「進行中(B6-c #196 残)」と更新

---

## §13 forward direction

### §13.1 Phase 5 / 6 への影響

本 spec で確立する zbus dispatch pattern は Phase 5 / 6 で以下の追加 method 実装に拡張する:

- `PageUp` / `PageDown` / `CursorUp` / `CursorDown`(候補 navigation、Phase 6 UX polish)
- `CandidateClicked(u32 index)`(マウス操作対応、Phase 6)
- `PropertyActivate(s name, u state)`(設定 UI 連携、Phase 6)
- `SetSurroundingText(v text, u cursor, u anchor)`(文脈 reranking、Phase 6 / Phase 7)

これらは `KotohaEngineService` に method 追加、`Event` enum に variant 追加、engine_loop dispatch 拡張で対応する。本 spec の design pattern を踏襲できる。

### §13.2 Phase 4 fcitx5 adapter との関係

Phase 4 で fcitx5 用 adapter を追加する際、`kotoha-engine-fcitx5` crate を別途新設する。`kotoha-engine-core::reactor::Event` enum の `IBusKey` / `IBusReset` variant は `IBusKey` のまま rename せず、fcitx5 側は同 variant を「key event(protocol 由来 unified)」として再解釈するか、`Fcitx5Key` variant を追加する。判断は Phase 4 kick-off 時。

### §13.3 timeout 値の再評価

100ms timeout は「engine が pathological 状態でない限り発火しない」値として spec §6.1 で根拠を示している。Phase 5(custom romaji-base model)で engine_loop の処理時間が増える場合、以下を再評価する:

- timeout 値の引き上げ(e.g., 200ms)
- `Event::IBusKey { ..., deadline: Instant }` 拡張で engine 側 abort 経路追加
- service method を async fn 化して smol::unblock で response 待機(ADR 0017 再評価必要)

Phase 5 spec kick-off 時に決定する。
