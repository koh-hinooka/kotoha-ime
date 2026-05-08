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
3. **threading**: listener thread は単独で `blocking::Connection` を保持し、内部 smol executor が dispatch を回す。本 crate コードは `std::sync` 同期 primitive のみ使用(ADR 0017「core は tokio 非依存」原則維持)。
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
- 欠点: API surface 拡大(`IMEHostBridge` trait に method 追加 + `IBusEngineSignals` 追加)、二相セマンティクスの複雑性、daemon round-trip を含む race condition の debug 経路、integration test が daemon 必須となり regression 検出網が薄くなる
- 結論: maintenance vs ミリ秒単位の理論的応答性差を秤にかけ、Phase 3-B 段階では (i)+(A) を優先

## Consequences

### Positive

- 4 thread topology(ADR 0020 §採択 Q4)を維持しつつ listener thread が production 実装になる
- `Event::IBusKey { event, respond }` 拡張は engine_loop の既存 dispatch 経路に最小変更で統合される
- ADR 0017 の "core は tokio 非依存" 原則を維持(zbus internal smol executor は本 crate コードに propagate しない)

### Negative

- `process_key_event` 経路で listener thread が最大 100ms blocking する。fast typist (10 keys/sec) でも通常 ~1ms / heavy ~10ms / recovery ~50ms で吸収できる範囲(spec §6.1)。
- timeout fire(>100ms)時に engine が eventual に `Consumed` 判定すると double-input が原理的に起こりうる(spec §6.2 で受容、Phase 5/6 で deadline check 拡張余地)

### Neutral

- IBus engine factory registration の正規経路(`/usr/share/ibus/component/<name>.xml`)は本 ADR scope 外。packaging task として別 ISSUE で扱う。

## Future revisit triggers

- Phase 5 custom romaji-base model の inference latency が増えた場合、100ms timeout 値の引き上げ / `Event::IBusKey` への deadline 拡張 / `async fn` method 化 + smol::unblock を再評価する
- Phase 4 fcitx5 adapter で異なる protocol への generalization が必要な場合、`KotohaEngineService` を trait 化する余地
