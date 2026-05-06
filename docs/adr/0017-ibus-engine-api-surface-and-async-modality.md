# ADR 0017: IBus engine API surface and async modality

## Status

Accepted (2026-05-02)、rev2 (2026-05-06):crossbeam-channel 採択を補足、「core は tokio 非依存」原則は維持

## Context

Phase 3-A IBus engine 実装で、core engine layer (`kotoha-engine-core`) が host 層
(IBus / fcitx5 / 将来) に対してどの粒度で API を出すべきか、また async runtime
(tokio) を core 側で要求するかを決める必要があった。

## Decision

- **2 trait port 構成**: driving port `IMEEngine`(host → engine)と
  driven port `IMEHostBridge`(engine → host)の 2 trait に分離する。
  `IMEEngine` は `Send` のみ要求(event loop 単一 thread 前提)、
  `IMEHostBridge` は `Send + Sync` 要求(engine 主 thread と `RankerWorker`
  thread の双方から呼ばれる)。
- **Async runtime 非依存**: core 層は tokio 等の async runtime に直接依存しない。
  channel-based plumbing は `std::sync::mpsc` + dedicated `std::thread` で構築する。
  `CancellationToken` は自作 trait + `StdCancellationToken` impl とし、Phase 5/6 で
  tokio runtime を導入する場合は別 crate で `TokioCancellationToken` adapter を
  提供する。
- **rev2 補足 (2026-05-06)**:Phase 3-B B0h-f + B3 一体化(ADR 0020)で multi-source
  event multiplex 用に `crossbeam-channel = "0.5"` を採択する。`std::sync::mpsc`
  上位互換であり「core は tokio 非依存」原則は維持される。`select!` macro は
  reactor 実装内部 detail として OS 依存 crate(`kotoha-engine-reactor-linux`)
  に閉じ、`kotoha-engine-core` の trait API には露出しない。

## Consequences

- IBus / fcitx5 / 将来の input-method protocol は adapter crate
  (`kotoha-engine-ibus` / `kotoha-engine-fcitx5`)の追加 / 差し替えだけで
  対応可能(boundary-first 原則)。
- core 単独 unit test が host adapter 不要(`MockHostBridge` 注入)で実現可能。
- async runtime を要求する Phase 5 機能(beam search 並列化等)は adapter 層 /
  別 crate で対応し、core は std 経路を維持する。
- 副作用として、blocking API 中心の zbus 5.x 経路を選び、tokio 依存を避けた
  (P3-A M5 の zbus dep `default features` 採用根拠)。

## References

- Phase 3-A spec `docs/specs/_uncategorized/p3-a-ibus-engine.md` §3 / §4
- ADR 0011: kanji backend trait design(trait ベース DI の前例)
- ADR 0020: Event-loop architecture for Phase 3-B(rev2 補足の根拠)
- Adaptive boundary-first 原則(`feedback_adaptive_boundary_first.md`)
