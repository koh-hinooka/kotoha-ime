---
feature: feature-26-property-strategy-broaden
status: implemented
bounded_context: _uncategorized
related_issues: ["#26"]
related_prs: []
glossary_refs: []
last_reviewed: 2026-05-05
---

# #26: romaji property strategy を plan-spec full alphabet に broaden

> **Migration note**: 本 spec は `docs/wbs/2026-04-23-feature-26-property-strategy-broaden.md` から spec 化 (Global CLAUDE.md §Documentation Structure 準拠の `docs/wbs/` 廃止対応、2026-05-05)。
> 元 WBS は実装ログ性質。14-section restructure は別 follow-up 起票予定 (各 project の既存 14-section follow-up issue と合流可能)。

## 元 WBS 内容 (実装ログ由来)

milestone: M3b-polish
branch: feature/26-property-strategy-broaden
pr: "#31"
merge_commit: "d2c0ff3"
issue: "#26"
status: done
started: 2026-04-23
finished: 2026-04-23
---

# #26: romaji property strategy を plan-spec full alphabet に broaden

## 背景

M3b (PR #24) 初版は property test strategy を `[aeioun]{0,12}` (母音 + n) に narrow していた。これは当時未解決だった M3a state machine の 3 つのバグクラス (後に #22 / #23 / #29 として追跡) を踏まないための暫定措置。各バグが順次解消されたため、plan 原型の `[a-z\\-'.,!?\\[\\]/]{0,12}` に復帰する。

## 前提 PR

本 PR は以下 3 つが merged であることを前提とする:

- #28 (#22 close): ADR 0001 で非 ASCII retraction policy を drop に pin。
- #27 (#23 close): `convert` EOF buffer normalization。
- #30 (#29 close): `convert` mid-stream buffer normalization。

## 実施内容

### 変更点 (1 file: `crates/kotoha-core/tests/romaji_property.rs`)

1. Strategy: `[aeioun]{0,12}` → `[a-z\\-'.,!?\\[\\]/]{0,12}` (plan-spec 原型)。
2. Iterations: `ProptestConfig::with_cases(256)` → `with_cases(1024)`。
3. `romaji_input` docstring の `# TODO(M3b / follow-up)` ブロック削除 (全 blocker 解消)。
4. モジュール `//!` doc を更新: ADR 0001 / spec §9.2 を citation、weakened idempotence property の根拠を明記。
5. `prop_idempotence_on_committed` は ADR 0001 の drop policy 確定により weakened 形 (`pending_second.is_empty()` のみ) で維持。strict committed equality は契約外として確定。
6. `#[test]` regression 3 件を追加:
   - `regression_byb_associative` (#23 関連 — partial-then-invalid 残渣)
   - `regression_j_comma_associative` (#23 / #22 関連 — punctuation 境界)
   - `regression_bracket_idempotent_pending` (#22 関連 — 非 ASCII 再入力)

### メトリクス

- workspace `#[test]` 総数: 81 → **84** (76 lib + 2 golden + 5 property (2 proptest + 3 regression) + 1 doc)。
- proptest 実行時間: 約 1.09s (2048 iterations 合計で 5s 以内)。
- clippy warnings: 0、fmt diff: 0、lefthook pre-push: 全通過。

### コミット

1 commit: `a986fbb` — strategy broaden + iterations up + 3 regression tests + docstring 整理。

## つまずき

- 最初の subagent dispatch 時、strategy を `[a-z\\-'.,!?\\[\\]/]{0,12}` にしたら `prop_associativity_via_pending` が `a="b!", b="a"` で FAIL。これが #29 を発見したきっかけ (当時未登録)。
- BLOCKED として subagent が停止、別 branch `bugfix/29-convert-mid-stream-normalize` で #29 を先行修正 → merge (PR #30)。
- その後本 branch を develop に rebase、unstash、再度 proptest 走行で 5/5 PASS を確認、commit + merge という順序で処理した。

## レビュー

Testing dimension review (1 file、test-only)。Critical / Important なし、Minor 3 件:
1. `regression_j_comma_associative` が split point 1 のみなので「2-char 入力は 1 split しかない」コメント追加が望ましい → deferred。
2. `prop_idempotence_on_committed` の failure message に `_committed_second` を含めるとデバッグ性向上 → deferred。
3. #29 に対応する named regression test 追加 (現状は property test で間接 cover) → M5 以降の clean-up で対応可能、deferred。

全 Minor deferred として merge。

## M3 全体の締め

本 PR merge により M3 (romaji モジュール) が完全体で締まった:

- M3a (PR #18): rules table + trie + state machine + RomajiConverter facade、33 unit tests。
- M3b (PR #24): golden fixture (207 rows) + narrow property test、M3b 時点で 75 tests。
- 後続 hotfix:
  - #23 (PR #27): EOF normalization。
  - #22 (PR #28): 非 ASCII retraction ADR 0001。
  - #29 (PR #30): mid-stream normalization。
  - #26 (PR #31、本 PR): property broaden to plan-spec + 1024 iterations + 3 regression tests。
- 最終: **84 tests** (76 lib + 2 golden + 2 proptest × 1024 iter + 3 regression + 1 doc)。

## 残 follow-up issue

- #19: perf — OnceLock Trie cache (低優先、M5 以降)。
- #20: docs — ADR for trie-vs-hashmap + #[non_exhaustive] on streaming enums (docs tier)。
- #25: test coverage — 55 rule keys 未カバーの cross-check test (M4 以降の hygiene)。
- #15: spec — §9.1 / §9.2 pending-buffer backtrack rule (M3a-3 契約の normative 化)。

## M4 への申し送り

- M3 は完全体で締まった。`RomajiConverter::convert` / `push` / `flush` の挙動が golden + property + 3 regression tests で多層的に固定化されている。
- property test が plan-spec full alphabet で回るようになったため、M4 の `input::InputContext` 実装時に romaji 層の契約を強い保証として依存できる。
- streaming path (`push` + `flush`) には依然として #29 類似のバグが潜在 (PR #30 WBS 参照)。M4 で `InputContext` 設計時に明示的対処が必要。

## 成果物リンク

- PR: https://github.com/std-koh-hinooka/kotoha-ime/pull/31
- ISSUE: https://github.com/std-koh-hinooka/kotoha-ime/issues/26 (CLOSED)
- 前提 PR: #27 (#23) / #28 (#22) / #30 (#29)
- Spec: `docs/superpowers/specs/2026-04-22-kotoha-phase-0-design.md` §11.3
- ADR: `docs/adr/0001-non-ascii-retraction-policy.md`
