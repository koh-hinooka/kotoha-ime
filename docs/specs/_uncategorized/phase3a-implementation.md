---
feature: phase3a-implementation
status: deprecated
deprecated_reason: "Phase E migration で旧 docs/wbs/ から spec 化した実装ログ性質の文書。Global CLAUDE.md §Development Flow legacy spec 取扱いルール (実装ログ性質 → status: deprecated、本文 14-section restructure 不要) に基づき deprecated 扱い。git history は参照点として保持 (2026-05-06)。"
bounded_context: _uncategorized
related_issues: []
related_prs: []
glossary_refs: ["candidate","kana","kotoha-engine","kotoha-storage","lefthook","preedit","romaji","row-3"]
last_reviewed: 2026-05-06
---

# Phase 3-A IBus engine integration — implementation log

> **Migration note**: 本 spec は `docs/wbs/2026-05-02-phase3a-implementation.md` から spec 化 (Global CLAUDE.md §Documentation Structure 準拠の `docs/wbs/` 廃止対応、2026-05-05)。
> 元 WBS は実装ログ性質。14-section restructure は別 follow-up 起票予定 (各 project の既存 14-section follow-up issue と合流可能)。

## 元 WBS 内容 (実装ログ由来)


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
| B0h-a | C3 hexagonal driven port 反転 (`learning_port` + `kotoha-engine-adapter`) | #154 | `feature/153-b0h-a-...` |
| B0h-b | I4 `HybridRanker` を `kotoha-ranker-hybrid` 別 crate へ切り出し | #156 | `feature/155-b0h-b-...` |
| B0h-d | I2 `IBusEventDispatcher` を `Arc<Mutex<dyn IMEEngine>>` 化 | #158 | `feature/157-b0h-d-...` |
| B0h-e | I5 stub fallback `dev-stubs` feature gate | #160 | `feature/159-b0h-e-...` |
| B0h-c-i | I1 SRP 分割 sub-PR 1/3:`PreeditBuffer` + `CandidateBuffer` 抽出 | #162 | `feature/161-b0h-c-i-...` |
| B0h-c-ii | I1 SRP 分割 sub-PR 2/3:`WorkerChannel` 抽出 + Drop 移管 | #164 | `feature/163-b0h-c-ii-...` |

## Test count(実測)

| timing | default features | `--features test-helpers` |
|--------|------------------|---------------------------|
| P2-D 完了時(P3-A 開始前) | n/a | 416 |
| P3-A M1〜M6 + P3-B B1/B4/B5 完了時(B0e merge 直後) | n/a | 459 |
| **P3-B B0e 完了時(2026-05-03)** | 432 | **473** |
| P3-B B0f 完了時(2026-05-03、PR #147) | 432 | 473(test 改変ゼロ) |
| P3-B B0g-a 完了時(2026-05-03、PR #150) | 436 | 478(+5: panic_message 3 + worker circuit breaker 1 + dispatcher panic catch 1) |
| P3-B B0g-b 完了時(2026-05-03、PR #151) | 447 | 491(+13: sanitize unit 7 + filter 4 + boundary_notify regression 2) |
| P3-B B0g-c 完了時(2026-05-03、PR #152) | 447 | 498(+7: row 8 4 件 + I14 demo 1 + boundary skip-when-empty 1 + silent_ranker e2e 1) |
| P3-B B0h-a 完了時(2026-05-03、PR #154) | 464 | 515(+17 / +17: hexagonal port 反転で domain port test 群が test-helpers なしでも回るようになり default 集合に組込) |
| P3-B B0h-b 完了時(2026-05-03、PR #156) | 504 | 515(default +40: workspace feature unification で engine_wrap_hybrid.rs が default にも参加、test-helpers は変動なし) |
| P3-B B0h-d 完了時(2026-05-03、PR #158) | 504 | 515(test 改変ゼロ) |
| P3-B B0h-e 完了時(2026-05-04、PR #160) | 504 | 515(test 改変ゼロ) |
| P3-B B0h-c-i 完了時(2026-05-04、PR #162) | 504 | 515(test 改変ゼロ、SRP 内部 refactor のみ) |
| P3-B B0h-c-ii 完了時(2026-05-04、PR #164) | 504 | 515(test 改変ゼロ、`WorkerChannel` 抽出 + Drop 移管のみ) |
| **P3-B B0h-c-iii 完了時(2026-05-04、本 PR)** | **504** | **515**(test 改変ゼロ、`LearningSink` 抽出のみ、SRP 4 sub-struct 完成) |

註:`cargo test --workspace` (default features) と `cargo test --workspace --features kotoha-storage/test-helpers,kotoha-engine-core/test-helpers` で結果が異なる。lefthook pre-push は default features を回す。第 1 回包括 review (B0a-B0e) では test-helpers feature 経由の合計値 (416 → 473) を baseline として参照する。過去の commit message で `446` / `448` / `454` と記載した数値はいずれも不正確で、上表が正規値。

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
| 9 | adapter Mutex<Vec<Candidate>> 競合 | B0h-d で `Arc<Mutex<dyn IMEEngine>>` 化済 | **解決(B0h-d / B2、spec §13 r2 で closure)** |
| 9 (D-Bus body) | `IBusText` / `IBusLookupTable` marshalling | IBus 1.5.x signature 準拠で実装、`Value::Structure(...)` 経由(spec §4.2 r3) | **進行中(B2 / #170)** |
| - (B0h-f 前提) | `update_preedit` blocking `connection.send` が typing path で 10ms budget 直撃 | Phase 3-B B0h-f(I3 async dispatch)で `WorkerChannel` 投入経路に切替、coalescing 5ms typing / 30ms commit を組合せ + `host_bridge::tracing::warn!` emit 検証(`tracing-test` crate を dev-dependency 追加し L2 で 5 method 個別検証) | TBD(B0h-f scope) |
| - (Phase 5 前提) | `IBusLookupTable::from_candidates` が候補数 unbounded(現状 dict + LLM 最大 ~30 件、Phase 5 で custom model 導入時に再評価) | `from_candidates` で `candidates.truncate(MAX_LOOKUP_TABLE_CANDIDATES)` 等の defense-in-depth を導入、debug log で truncation 観測 | TBD(Phase 5 scope、別 ISSUE 起票) |

## P3-B 後続予定

| # | Sub-milestone | 状態 |
|---|---|---|
| ~~B0 (#140)~~ | 第 1 回包括レビュー Critical 4 + Important 9 消化 | **完了**(PR #141-#145、test 416 → 473) |
| ~~B0f (#146)~~ | 機能完成宣言取り下げ + proxy / event loop fail-loud 化 | **完了**(PR #147 squash `9602dff`) |
| ~~B0g-a (#148)~~ | C4 panic_message + C5 lookup_table + I9/I16 worker circuit breaker + I15 hybrid empty-fallback + I17 dispatcher panic catch | **完了**(PR #150 squash `fdaafe3`、test 432→436 / 473→478) |
| ~~B0g-b (#148)~~ | I6 hybrid+stub kana/text redact + I7 HybridRanker child thread catch_unwind + I8 engine 境界 candidate sanitize + commit_text safety | **完了**(PR #151 squash `4b010c8`、test 436→447 / 478→491) |
| ~~B0g-c (#148)~~ | I10 theater fix + I11 spec §5.2 row 8 + I12 polling helper + I13 half-dead engine + I14 mock arg verify | **完了**(PR #152 squash `833c7d9`、test 447 / 498) |
| ~~B0h-a (#153)~~ | C3 hexagonal driven port 反転(`learning_port` を engine-core に新設、`kotoha-engine-adapter` crate 切出し、engine-core が storage を直接 import しない構造へ) | **完了**(PR #154 squash `5c37d65`、test 464 / 515) |
| ~~B0h-b (#155)~~ | I4 `HybridRanker` を `kotoha-ranker-hybrid` 別 crate へ切り出し(engine-core から concrete adapter を分離、LLM features を ranker-hybrid 側に移送) | **完了**(PR #156 squash `3cc36a9`、test 504 / 515) |
| ~~B0h-d (#157)~~ | I2 `IBusEventDispatcher` を `Arc<Mutex<dyn IMEEngine>>` 化(B3 event loop の前提整備) | **完了**(PR #158 squash `ba7dc6d`、test 504 / 515) |
| ~~B0h-e (#159)~~ | I5 stub fallback feature gate(`StubRanker` / `StubHostBridge` を `dev-stubs` feature で gate、release default で stub symbol 0 link) | **完了**(PR #160 squash `7b58cbc`、test 504 / 515) |
| ~~B0h-c-i (#161)~~ | I1 SRP 分割 sub-PR 1/3:`PreeditBuffer` + `CandidateBuffer` 抽出(`current_preedit` + `romaji` / `candidates` + `highlight_idx` を 2 sub-struct に集約) | **完了**(PR #162 squash `9c3bd0e`、test 504 / 515) |
| ~~B0h-c-ii (#163)~~ | I1 SRP 分割 sub-PR 2/3:`WorkerChannel` 抽出(`tx_request` / `rx_event` / `worker_handle` を 1 sub-struct に集約、`KotohaEngine::Drop` を `WorkerChannel::Drop` に移管。`request_id_seed` は engine 側に残置) | **完了**(PR #164 squash `7dd4b70`、test 504 / 515) |
| **B0h-c-iii (#165)** | I1 SRP 分割 sub-PR 3/3:`LearningSink` 抽出(`learning_writer` + `commit_history` + `last_commit_at` を 1 sub-struct に集約)。transitions.rs の free-function method 化は API ergonomics 課題で SRP とは独立、本 PR 範囲外(B0h-c-iv で別途扱うか判断) | **進行中**(本 PR、test 504 / 515) |
| B0g 後追加検討(B0h 候補) | self-review#1〜#9: `non_exhaustive` trade-off ADR、flaky panic sliding-window metrics ADR、`IMEEngine::enable` Result 化、F4-F9 系 ADR | OSS 公開前 |
| B0h-f (#149) | I3 async dispatch(`drain_events_blocking` 撤去 + wakeup channel) | OSS 公開前必修 |
| ~~B2 (#170)~~ | IBus signal body marshalling(`proxy.rs` 5 method を実 D-Bus signal emit に置換、`IBusText` / `IBusLookupTable` wire format 確定:`(sa{sv}sv)` / `(sa{sv}uubbiavav)`、IBus 1.5.x source 準拠、PR review 5 dim 統合反映で types を `attrs: IBusAttrList` / `candidates: Vec<IBusText>` に refactor、structural assertion test、`pub(crate)` visibility、threat model rustdoc) | **完了**(PR #171 squash `4731c50`、test **536 / 547**(+32)、release stub symbol 0、`KOTOHA_ALLOW_STUB=1` exit 1) |
| B3 | IBus signal listener loop + event loop | B0f / B2 後着手 |
| B6 | L3 manual smoke | B2 / B3 後着手 |
