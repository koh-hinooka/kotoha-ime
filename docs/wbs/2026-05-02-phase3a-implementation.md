# Phase 3-A IBus engine integration — implementation log

| 項目 | 値 |
|------|----|
| ISSUE | #128 |
| 期間 | 2026-05-02 〜(L3 manual smoke 完了時に更新) |
| 関連 spec | `docs/superpowers/specs/2026-05-02-p3-a-ibus-engine-design.md` |
| 関連 plan | `docs/superpowers/plans/2026-05-02-feature-128-p3a-engine-implementation.md` |

## Milestone 結果

| # | Milestone | PR | branch |
|---|---|---|---|
| M1 | engine-core types + traits + Mock infra | #130 | `feature/128-p3a-m1-traits-and-mocks` |
| M2 | KotohaEngine 状態機械 (synchronous Ranker) | #131 | `feature/128-p3a-m2-state-machine` |
| M3 | RankerWorker + coalescing + cancel propagation | #132 | `feature/128-p3a-m3-ranker-worker` |
| M4 | kotoha-engine-ibus skeleton + IBusHostBridge | #133 | `feature/128-p3a-m4-ibus-host-bridge` |
| M5 | IBusEventDispatcher + zbus binding | #134 | `feature/128-p3a-m5-ibus-dispatcher` |
| M6 | kotoha-bin + L2 + glossary + ADRs | (本 PR) | `feature/128-p3a-m6-bin-and-tests` |

## Test count

| timing | engine-core unit | engine-core integration | engine-ibus | full workspace |
|--------|------------------|-------------------------|-------------|----------------|
| baseline (P2-D 直後) | 17 | 27 | — | 416 |
| M1 後 | 21 | 27 | — | 420 |
| M2 後 | 24 | 35 | — | 431 |
| M3 後 | 24 | 37 | — | 433 |
| M4 後 | 24 | 37 | 5 | 438 |
| M5 後 | 24 | 37 | 9 | 442 |
| M6 後(本 PR) | 24 | 41 (+ L2-core 4) | 9 | 446 |

## L3 manual smoke 結果

GNOME Wayland session 上で `cargo run --bin kotoha` 起動 + 以下 application で典型変換を確認する。本 PR では skeleton として枠を確保し、実機検証は Phase 3-A 実装段階で詳細化する。

- [ ] Firefox URL bar / textarea で 5 件
- [ ] GNOME Text Editor で 3 件
- [ ] VS Code editor で 2 件
- [ ] flicker observation(spec §13 Open Q 3)

## 残 Open Questions(spec §13 由来)

| # | 暫定値 | 実装段階での確定 method | 結果 |
|---|--------|------------------------|------|
| 1 | commit_history 200 chars | 100 / 200 / 400 で AB test | TBD |
| 2 | coalescing Live 7ms / Commit 30ms | IBus host 描画 frame rate と実機 measure | TBD |
| 3 | IBus update_lookup_table flicker | 連続 update 観察 | TBD |
| 4 | LLM cancel 10 token check | metrics 観測 | TBD |
| 7 | typing 中 LLM invocation | empirical 観測 | TBD |
| 8 | KeyModifiers full mapping | IBus IBusModifierType 全列挙 | TBD |
| 9 | adapter Mutex<Vec<Candidate>> 競合 | multi-thread 化必要時に再評価 | TBD |
| 9 (D-Bus body) | `IBusText` / `IBusLookupTable` marshalling | L3 manual smoke で `connection.send_signal` body 確定 | TBD |

## 主要 deferral 事項

本 P3-A 実装で確立されたが Phase 3-A 実装段階の L3 manual smoke で詳細化する事項:

- `IBusEngineSignals` の signal body 完全 marshalling(`IBusText` struct
  D-Bus type の正確 serialize)
- `IBusEventDispatcher` の D-Bus signal listener loop(`zbus::blocking::MessageStream`
  経由の signal receive)
- `kotoha-bin` の production `MorphologicalEngine` / LLM backend 統合
  (本 PR は StubRanker で wiring のみ確立)
- Phase 1 14/15 regression engine wrap test の本格実装
