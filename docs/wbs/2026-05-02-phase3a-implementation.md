# Phase 3-A IBus engine integration — implementation log

| 項目 | 値 |
|------|----|
| ISSUE | #128(自動 close)+ #136(P3-B tracking)+ #140(P3-B B0 必須前提) |
| 期間 | 2026-05-02 〜(機能完成は P3-B B0 完了時) |
| Status | **draft 完成 / 機能未完**(下記「再分類後の状態」参照) |
| 関連 spec | `docs/superpowers/specs/2026-05-02-p3-a-ibus-engine-design.md` |
| 関連 plan | `docs/superpowers/plans/2026-05-02-feature-128-p3a-engine-implementation.md` |

## 再分類後の状態(2026-05-03 包括レビュー後の更新)

P3-A の M1〜M6 PR は merge 済 / develop 上に着地済。ただし 2026-05-03 の **包括レビュー(architecture / security / testing / silent-failure 4 視点並列)** で以下の重大ギャップが発覚し、**機能完成宣言を取り下げ、`P3-B B0` を必須前提として後続化** する。

### Critical(機能完成 blocker、P3-B B0 で消化)

1. **spec §9.1 catch_unwind 未実装**:engine main thread の panic を receive する top-level catch_unwind が `kotoha-bin` に無い。worker thread 死亡時の `RecvError` も無音で `return` されており、1 回の panic で session が永久死亡する。
2. **kotoha-bin の degraded-mode 起動が production 検出不可**:`KOTOHA_SYSTEM_DICT_PATH` 未設定 / D-Bus 接続失敗で `StubRanker` + `StubHostBridge` に WARN log だけで起動継続。spec §9.3「空候補返却で終わる」を `StubRanker::rank` が直接体現している。
3. **spec §10.2 全 row 網羅未達**:状態遷移 14 row のうち 7 row が未テスト(Live+Esc / CommitConverting+output / CommitConverting+edit/Esc/focus_out / CandidatesShown+typing/backspace/focus_out)。`live_space_transitions_to_candidates_shown` は spec §5.2 が指定する `CommitConverting` を assert していない theater pattern。`regression_phase1.rs` は空 body の placeholder で silent test rot。L2-adapter `ibus_host_bridge.rs` も assertion ゼロ。
4. **spec §7 async モデルの実質崩壊**:`dispatch_rank_request` が同期 blocking で 1st window 候補のみ消費し、Commit mode の 2nd window LLM 候補が engine に届かない。ADR 0018「2 段 window 構造を凍結」と乖離。`CommitConverting` 状態が実用上到達不可能。

### Important(P3-B B0 で同時消化推奨)

5. cancel 5 trigger のうち Esc / reset / 連続 space / backspace 4 trigger が未テスト
6. worker `tx_event.send` 失敗時に `tracing` 観測なし(`engine/worker.rs:66/82/99` の `let _ =`)
7. `KotohaEngine::new` の worker spawn 失敗で panic(spec §9.1 recovery 不能)
8. worker 空 buffer 時に `Candidates` を送らないため、変換失敗と「該当候補なし」が区別不能 + 前回候補が画面に残留
9. `tx_request.send` 失敗時に `active_request` が stale state で残る
10. `RankerOutput.request_id` が dead field(B5 で worker 側 id 照合を削除した結果、ranker impl 側の値は誰にも読まれない契約)
11. `IBusEventDispatcher` 単体テスト 0、`MockEngine` で 6 method 簡単にテスト可能
12. L2-core scenario が host call 「順序」ではなく「存在」のみ assert(spec §10.3 違反)
13. `let _ = tx_event.send(...)` 等 `%e` で error chain 切捨て、`?e` で source chain 全表示すべき

### 詳細レビュー記録

包括レビューの全文は git log の `feature/p3a-wbs-downgrade-and-b0-issue` branch 起票時 PR description に保存。Critical 4 + Important 9 全て file:line citation 付き。

## P3-A milestone 結果(merge 済)

| # | Milestone | PR | branch |
|---|---|---|---|
| M1 | engine-core types + traits + Mock infra | #130 | `feature/128-p3a-m1-traits-and-mocks` |
| M2 | KotohaEngine 状態機械 (synchronous Ranker) | #131 | `feature/128-p3a-m2-state-machine` |
| M3 | RankerWorker + coalescing + cancel propagation | #132 | `feature/128-p3a-m3-ranker-worker` |
| M4 | kotoha-engine-ibus skeleton + IBusHostBridge | #133 | `feature/128-p3a-m4-ibus-host-bridge` |
| M5 | IBusEventDispatcher + zbus binding | #134 | `feature/128-p3a-m5-ibus-dispatcher` |
| M6 | kotoha-bin + L2 + glossary + ADRs | #135 | `feature/128-p3a-m6-bin-and-tests` |

## P3-B 着手済(merge 済)

| # | Sub-milestone | PR | branch |
|---|---|---|---|
| B1 | kotoha-bin 実 HybridRanker 配線 | #137 | `feature/136-p3b-b1-production-ranker-wiring` |
| B4 | KeyModifiers full IBus mapping + RELEASE handling | #138 | `feature/136-p3b-b4-keymodifiers-full-mapping` |
| B5 | KotohaEngine + 実 HybridRanker e2e test | #139 | `feature/136-p3b-b5-engine-wrap-integration` |

## Test count(実測)

| timing | full workspace |
|--------|----------------|
| P2-D 完了時(P3-A 開始前) | 416 |
| **P3-A M1〜M6 + P3-B B1/B4/B5 完了時(2026-05-03 現在)** | **454** |

註:過去の commit message で `446` / `448` と記載した数値は不正確。正規値は `cargo test --workspace --features kotoha-storage/test-helpers,kotoha-engine-core/test-helpers` で得た 454。

## L3 manual smoke 結果

GNOME Wayland session 上で `cargo run --bin kotoha` 起動 + 以下 application で典型変換を確認する。**P3-B B0 完了 + B2/B3 完了後** に実機検証する(現状の binary は B2/B3 未完で IBus 上で実用変換できない)。

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
| 8 | KeyModifiers full mapping | IBus IBusModifierType 全列挙 | **解決(P3-B B4 / PR #138)** |
| 9 | adapter Mutex<Vec<Candidate>> 競合 | multi-thread 化必要時に再評価 | TBD |
| 9 (D-Bus body) | `IBusText` / `IBusLookupTable` marshalling | L3 manual smoke で `connection.send_signal` body 確定 | TBD |

## P3-B 後続予定

| # | Sub-milestone | 状態 |
|---|---|---|
| **B0 (#140)** | **包括レビュー Critical 4 + Important 9 消化(下記 ISSUE 参照)** | **未着手 / 機能完成 blocker** |
| B2 | IBus signal body marshalling | B0 後着手 |
| B3 | IBus signal listener loop + event loop | B0 後着手 |
| B6 | L3 manual smoke | B2/B3 後着手 |
