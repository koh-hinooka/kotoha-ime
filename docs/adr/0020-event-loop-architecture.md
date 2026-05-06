# ADR 0020: Event-loop architecture for Phase 3-B (B0h-f + B3 一体化)

## ステータス

承認 (2026-05-06)、rev2 (2026-05-07):実装フェーズで判明した B3 listener と SIGTERM hook の B6 deferral を §影響 に明記

## コンテキスト

Phase 3-B 残作業のうち以下 2 件は単一の architectural rework として一体化して扱う。

- **B0h-f** (#149 残): `KotohaEngine::dispatch_rank_request` の同期 blocking(`drain_events_blocking`)を撤去し、worker output を非同期 channel 経由で受け取る。spec §6.1 の「typing path < 10ms」budget 違反、および Commit 2nd window の LLM 結果が user に届かない silent drop を解消する。
- **B3** (#136 残): `kotoha-bin::run_ibus()` の `anyhow::bail!` を実 D-Bus signal listener loop に置換し、IBus Method Call(`ProcessKeyEvent` 他)を engine に届ける。

両者は engine の thread topology と event 受信機構を共に変更する一体的作業であり、分割すると thread モデルが PR 間で短期的に不整合になる。本 ADR は両者を 1 つの設計判断で記録する。

設計判断は 2026-05-06 の brainstorm session(`.superpowers/brainstorm/3008267-1777842100/`)で 4 つの sub-question に分解して評価した。

| Question | 採択 | 却下した代替案 |
|---|---|---|
| Q1: refactor scope | C: engine 独立 event-loop(radical restructure) | A: minimal patch / B: standard worker-only refactor |
| Q2: multiplex 機構 | A: crossbeam-channel `select!` macro | B: mio / C: tokio 全面 / D: std-only Condvar / 後発 kanal(pre-1.0) / flume(release 若い) |
| Q3: reactor の crate 境界 | β: 新 crate `kotoha-engine-reactor-linux` を切出し、`kotoha-engine-core` には `EventReactor` trait のみ置く | α: engine-core に colocate / γ: multi-OS 抽象を最初から導入 |
| Q4: thread topology | A: 4-thread lock-free(main / dbus-listener / engine-loop / ranker-worker、bridge channel + 単独所有 engine) | B: 3-thread 統合 / C: `Arc<Mutex<dyn IMEEngine>>` 共有 |

## 検討した選択肢

### 選択肢1: 既存構造での最小 patch(Q1=A、却下)

- 利点: 変更行数最小、既存 test が殆どそのまま
- 欠点: spec §6.1 latency budget 違反が解消しない、Commit 2nd window 問題が残る、B3 listener と engine 状態の lock 競合解消手段が無い

### 選択肢2: worker-only async 化(Q1=B、却下)

- 利点: B0h-f 単独としては成立、B3 は別途
- 欠点: B3 着手時に engine 主 thread と D-Bus listener thread が `Arc<Mutex<dyn IMEEngine>>`(B0h-d で導入済)で競合し、key event 処理中の `Mutex` 保持で他 event が遅延、テスト難易度高

### 選択肢3: engine 独立 event-loop(Q1=C、**採用**)

- 利点: lock 0 件で engine 状態を engine-loop thread が単独所有 / D-Bus signal / worker output / shutdown / 将来の notification source を `Event` sum 型に fan-in / spec §6.1 latency budget を `recv_timeout` で構造的に満たせる / Phase 4 fcitx5 / Phase 5 notification 拡張の経路が clean
- 欠点: 実装行数が多い(~600 lines 見込み)、shutdown ordering の設計が必要、ADR + spec 改訂が必要

### Q2 採択: crossbeam-channel `select!` macro

- 利点: ADR 0017「core は tokio 非依存」原則を維持、`std::sync::mpsc` から drop-in に近い差し替え、`select!` macro の表現力で多 source 待機が型安全、枯れた dependency(crossbeam-channel 0.5.15、master 2026-02 commit)
- 欠点: D-Bus connection fd を直接 poll する用途には bridge thread pattern が必要(本 ADR では十分許容)
- 却下した代替:
  - **mio**: source 数 3〜4 で表現力過剰、移植性が Linux/BSD 限定で fcitx5 macOS 対応時に再検討必要
  - **tokio**: ADR 0017 全面 revise + `std::thread` / `std::sync::mpsc` の大量書換、scope 膨張
  - **std-only Condvar**: spurious wakeup / fairness 自前で利点薄い
  - **kanal 0.2.0-beta1**: pre-1.0 で API 安定性懸念
  - **flume 0.12**: 活発だが crossbeam ほど枯れていない

### Q3 採択: 新 crate `kotoha-engine-reactor-linux`

- 利点: `kotoha-engine-core` は `EventReactor` trait と `Event` enum のみ置き、Linux/eventfd 依存を core から isolate(`feedback_adaptive_boundary_first.md` 準拠)/ Phase 4 fcitx5 で `kotoha-engine-reactor-fcitx5`(必要なら)を並列追加可能 / core unit test は `MockReactor` で完結
- 欠点: crate 数が 1 件増える(現行 8 → 9)
- 却下した代替:
  - **engine-core に colocate**: OS 依存(eventfd、`crossbeam_channel::select!`)が core に染み込み、test で OS 非依存性が失われる
  - **multi-OS 抽象を最初から**: Phase 3 は GNOME Wayland のみが対象、YAGNI

### Q4 採択: 4-thread lock-free topology

- 利点: engine 状態を engine-loop thread が `move` で単独所有 → lock 0 件 / D-Bus protocol 層と engine 論理層が thread 境界で分離 / zbus version update の影響が dbus-listener thread に局所化 / Phase 5 notification source を engine-loop の `select!` arm 追加だけで対応
- 欠点: shutdown ordering を main / engine-loop / listener / worker の 4 階層で設計する必要、per-message bridge channel コスト(unbounded crossbeam で ns オーダー、IME 用途では無視可)
- 却下した代替:
  - **3-thread 統合**(dbus-listener + engine-loop): zbus 内部 fd 構造に密結合、reactor wake が D-Bus message decode に阻害される
  - **Arc<Mutex<dyn IMEEngine>>**(B0h-d 既存形式の継続): Q1=C の event-loop 意図に矛盾、lock 競合 + poison cascade リスク

## 決定

Phase 3-B B0h-f + B3 を一体化し、以下の architecture を採用する。

### 構成要素

- **新 crate `kotoha-engine-reactor-linux`**: Linux 専用の `EventReactor` 実装(crossbeam-channel `select!` ベース)
- **`kotoha-engine-core::reactor` モジュール**: OS 非依存の `EventReactor` trait + `Event` sum 型(`#[non_exhaustive]`、variant: `IBusKey` / `IBusReset` / `WorkerOutput` / `Shutdown`)
- **4-thread topology**:
  - `main`: DI wiring、thread spawn、SIGTERM/SIGINT handler 設置、join 順制御
  - `kotoha-dbus-listener`: zbus `blocking::MessageStream` を while loop で受け、`Event::IBusKey` / `Event::IBusReset` に decode して bridge channel に送る(B3 本体)
  - `kotoha-engine-loop`: `KotohaEngine` を単独所有し、`reactor.recv()` で event multiplex、`apply_candidate_update` を engine-loop 内で実行(`drain_events_blocking` を撤去)
  - `kotoha-ranker-worker`: 既存 `RankerWorker` pattern を継承し、worker output を `Event::WorkerOutput` で送信
- **新規 dependency**: `crossbeam-channel = "0.5"` を workspace root に追加

### 削除する API / type

- `KotohaEngine::drain_events_blocking` (`crates/kotoha-engine-core/src/engine/mod.rs:276`)
- `KotohaEngine::dispatch_rank_request` 内の同期 blocking 待機部(method 自体は維持、send-only に変更)
- `IBusEventDispatcher` の `Arc<Mutex<dyn IMEEngine>>`(B0h-d で導入済)を engine-loop 単独所有に置換

### 影響範囲

- 新 crate: `crates/kotoha-engine-reactor-linux/` (1 crate、~250 lines 見込み)
- `kotoha-engine-core/src/reactor/`(新 module、~150 lines)
- `kotoha-engine-core/src/engine/mod.rs`(`drain_events_blocking` 削除、`dispatch_rank_request` 簡素化、~80 lines diff)
- `kotoha-engine-ibus/src/listener.rs`(B3 listener、新 module、~120 lines)
- `kotoha-engine-ibus/src/dispatcher.rs`(`Arc<Mutex>` 撤去、~40 lines diff)
- `kotoha-bin/src/main.rs`(thread spawn / join、`anyhow::bail!` 撤去、~80 lines diff)
- `kotoha-bin/src/engine_loop.rs`(新 module、~100 lines)
- 既存 test infrastructure(`MockReactor` 追加、~100 lines)
- 合計約 7-9 file、~900 lines 想定(global rule §PR Review Matrix の Large 上限 ≤20 files / ≤1000 lines 内)

## 影響

### 技術的影響

- `kotoha-engine-core` から OS 依存(eventfd / `select!`)を完全 isolate、core unit test の OS 中立性が向上
- engine 状態の lock 0 件化により、Mutex poison cascade(PR #111 で対処した類)のリスク自体が消滅
- B3 listener loop が独立 thread になり、key event decode と engine state 遷移が並列実行可能(latency budget 観測点が分かれる)
- spec §6.1 の「typing path < 10ms」budget は engine-loop の `recv_timeout(coalescing_window)` で構造的に保証
- spec §6.1 の「Commit 2nd window 150ms」LLM 結果が `Event::WorkerOutput` 経由で確実に user に届く(silent drop が消滅)
- `Arc<Mutex<dyn IMEEngine>>`(B0h-d 導入)を撤去するため、B0h-d は本 ADR で踏み台 architecture として位置付けられ、後続で superseded となる

### 組織 / 開発フロー影響

- ADR 0017 を rev2 に改訂し、crossbeam-channel 採択を明記(原則「core は tokio 非依存」は維持)
- spec `p3-a-ibus-engine.md` §6.1 / §7 / §13 を改訂
- ROADMAP v0.3.0 の B0h-f / B3 行を「進行中」へ更新
- 後続 Phase 4(fcitx5)で `kotoha-engine-reactor-fcitx5` を並列追加する選択肢が成立(現状は Linux で `kotoha-engine-reactor-linux` 1 crate のみ、必要時に分岐)

### 既存 ADR との関係

- **ADR 0017 (rev2 で更新)**: 「core は tokio 非依存」原則は維持。crossbeam-channel = "0.5" の採択を補足。
- **ADR 0018 (Ranker invocation contract)**: 維持。`Ranker` trait API は変更なし。
- **B0h-d (#157 / PR #158) で導入した `Arc<Mutex<dyn IMEEngine>>`**: 本 ADR で superseded。撤去理由は ADR 0020 §採択 Q4 を参照。

### rev2 (2026-05-07) 補足:B6 deferral 明示

実装フェーズ(PR #183)で以下 2 件は ADR 起案時の見積を超え、B6 manual smoke (#136 残)に deferral となった。本 deferral は本 ADR の §決定 / §採択 Q1-Q4 を変更しない(architectural intent は完成)が、実 production 動作までは B6 完了が必要である事実を残す。

1. **B3 listener の zbus message decode**:`kotoha-engine-ibus::listener::run` は本 PR で **architectural skeleton**(shutdown flag polling)として完成。実 zbus `MessageStream` decode + IBus engine factory 登録(`RequestName` + `interface!` macro impl)は B6 で完成させる。skeleton 起動は `KOTOHA_ALLOW_LISTENER_STUB=1` 必須(env var 未設定時は `ListenerStubRefused` で起動拒否、spec §9.3 fail-loud)。
2. **SIGTERM/SIGINT hook**:`kotoha-bin::run_ibus()` の `engine_loop_handle.join()` は signal 受信で抜ける経路が無い。本 PR は `tracing::warn!` 1 行で限界を明示。`ctrlc::set_handler` 等の最小依存追加は B6 で行う。

これらは ADR 0020 の architectural goals を変えるものではなく、実 IBus daemon との接続(B6 manual smoke)前段で完成させるべき残作業である。Issue #136 の acceptance に反映する。

## 参考

- spec `docs/specs/_uncategorized/p3-a-ibus-engine.md` §6.1 / §7
- ADR 0017: IBus engine API surface and async modality
- brainstorm session: `.superpowers/brainstorm/3008267-1777842100/` (Q1-Q4 decision logs)
- ROADMAP `docs/ROADMAP.md` Phase 3-B 残作業 table
- 関連 ISSUE: #149 (B0h umbrella) / #136 (Phase 3-B umbrella、本 ADR は B3 portion をカバー)
- feedback memory: `feedback_adaptive_boundary_first.md` / `feedback_clean_architecture_solid.md`
