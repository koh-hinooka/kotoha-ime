# Phase 3-A IBus engine integration — implementation log

| 項目 | 値 |
|------|----|
| ISSUE | #128(自動 close)+ #136(P3-B tracking)+ #140(P3-B B0 / 完了)+ #146(P3-B B0f / 機能完成宣言取り下げ) |
| 期間 | 2026-05-02 〜(機能完成は P3-B B2 + B3 + L3 manual smoke 完了時に再判定) |
| Status | **draft 完成 / 機能未完(2026-05-03 再レビューで取り下げ)**(下記「再分類後の状態 v2」参照) |
| 関連 spec | `docs/superpowers/specs/2026-05-02-p3-a-ibus-engine-design.md` |
| 関連 plan | `docs/superpowers/plans/2026-05-02-feature-128-p3a-engine-implementation.md` |

## 再分類後の状態 v2(2026-05-03 第 2 回包括レビュー後)

ISSUE #140(P3-B B0)で第 1 回レビューの Critical 4 + Important 9 を消化(PR #141-#145、test 416 → 473)。直後に **第 2 回包括レビュー(architecture / security / testing / silent-failure 4 dimension 並列再走)** を実施し、**前回見逃した新規 Critical 3 件 + Important 17 件** を検出。最大の問題は以下:

1. **`crates/kotoha-engine-ibus/src/proxy.rs` の 5 method がすべて no-op stub**(`tracing::trace!` + `Ok(())` のみで実 D-Bus signal 送信無し)
2. **`crates/kotoha-bin/src/main.rs:188` の `let _dispatcher = ...; Ok(())`** で event loop 無く即 exit。INFO log は「kotoha engine wired up」を出すが、IBus daemon に signal は届かず process は exit 0 で正常終了する silent failure(spec §9.3 違反)

上記 2 件は **B0e 段階で「機能完成」と宣言した直後の review で初めて表面化**。`cargo run --bin kotoha` は exit 0 で成功するが、L3 manual smoke を実行した瞬間に「kotoha が 1 keystroke も処理しない」現実が露呈する構造だった。

このため **P3-A 機能完成宣言を取り下げ**(#146 / B0f)、本 WBS の Status を「draft 完成 / 機能未完」に再戻し。機能完成は **B0f + B2 + B3 + L3 manual smoke** 完了時に再判定。

### 第 2 回レビュー検出 Critical(B0f / B2 / B3 で消化)

- **C1 (新)**: `proxy.rs` 5 method の no-op stub(B0f で `Err(zbus::Error::Failure)` 化、B3 で実 signal emit に置換)
- **C2 (新)**: `main.rs:188` `let _dispatcher; Ok(())` の silent exit(B0f で `anyhow::bail!` に変更、B3 で event loop 実装)
- **C3 (新)**: hexagonal port 違反 — `kotoha-engine-core` が `kotoha-storage` の trait に直接依存し domain core が adapter 層を import(後続 B0h で driven port 反転)
- **C4 (新, silent-failure)**: `panic_message` の `Box<dyn Any>` downcast が `&'static str` / `String` のみ対応で `panic_any(anyhow::Error)` 等の payload を消失(後続 B0g で payload + backtrace 拡充)
- **C5 (新, silent-failure)**: `lookup_table.rs:52` の `_ =>` arm が `non_exhaustive` trap で future variant を WARN 1 行 + no-op fallback に偽装(後続 B0g で exhaustive match 化)

### 第 2 回レビュー検出 Important(後続 B0g / B0h で順次消化)

- I1: `KotohaEngine` SRP 過負荷(5 責務 / `pub(crate)` 14 field 露出)
- I2: `IBusEventDispatcher<E: IMEEngine>` が engine を own、B3 で `Arc<Mutex<dyn IMEEngine>>` 必須化で破壊的変更不可避
- I3: `dispatch_rank_request` 同期 blocking が spec §6.1「< 10ms」凍結に違反、加えて Commit 第 2 window (LLM 結果 150ms) が次 keystroke までユーザーに到達しない
- I4: `HybridRanker` が engine-core 内で dict/kanji backend を巻き込む(crate 分離推奨)
- I5: Stub fallback が release binary に常時 link、`cfg(debug_assertions)` gate 推奨
- I6: log credential leak — `hybrid.rs` `kana = %kana_owned` が WARN レベルで `KOTOHA_LOG=info` default 常時出力(B0f / B0g で redact)
- I7: `HybridRanker` 内 `thread::spawn` の child thread panic が worker.rs catch_unwind 範囲外
- I8: Engine → IBus 出力に control char / ANSI escape / RTL override / NUL byte sanitization 無し
- I9: `worker_loop` 自体に top-level catch_unwind が無く永続 IME-disabled risk
- I10: `silent_ranker_clears_engine_candidates_via_empty_replace` が tautology(theater pattern 再発)
- I11: spec §5.2 row 8(CommitConverting + Esc/backspace/focus_out)が unit test 0
- I12: thread::sleep 依存 timing assertion で flaky 温床(`commit_converting_..._on_late_arrival` は 30ms margin only)
- I13: `worker_recovers_after_ranker_panic` が `second_calls >= 1` のみで half-dead engine を pass 判定
- I14: MockRanker/MockHostBridge の引数 verification 不在(cursor 値 / kana 値が catch されない)
- I15: hybrid.rs 全 backend 全滅時 dict-only path で **空 Vec を tracing::error 無しで送信**(spec §9.1 row 3 違反)
- I16: worker 連続 panic に circuit breaker 無し(error log flood)
- I17: `dispatcher.rs:41-47` `dispatch_key` が `engine.process_key_event` の panic を catch せず spec §9.1 row 5 違反

### B0f scope(本 ISSUE #146)

機能完成宣言の取り下げ + fail-loud gating の最小範囲:

- proxy.rs 5 method を `Err(zbus::Error::Failure(NOT_YET_IMPLEMENTED))` 化
- proxy.rs `tracing::trace!(text, ...)` を `text_len = text.chars().count()` に redact(I6 部分対応)
- main.rs `run_ibus()` を `anyhow::bail!` 化(exit code 1 で起動失敗)
- 本 WBS の status / 機能完成記述を取り下げ
- B0g / B0h は別 ISSUE で順次対応

## 再分類後の状態 v1(2026-05-03 第 1 回包括レビュー、ISSUE #140 で消化済)

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
| ~~B0 (#140)~~ | 第 1 回包括レビュー Critical 4 + Important 9 消化 | **完了**(PR #141-#145、test 416 → 473) |
| **B0f (#146)** | 機能完成宣言取り下げ + proxy / event loop fail-loud 化 | **進行中**(本 PR) |
| B0g (TBD ISSUE) | 第 2 回 review C4 / C5 / I7-I17 系の中期消化 | B0f 後着手 |
| B0h (TBD ISSUE) | hexagonal port 反転(C3) + SRP 分割(I1) + dispatcher Arc<Mutex>(I2) | OSS 公開前必修 |
| B2 | IBus signal body marshalling | B0f 後着手 |
| B3 | IBus signal listener loop + event loop | B0f / B2 後着手 |
| B6 | L3 manual smoke | B2 / B3 後着手 |
