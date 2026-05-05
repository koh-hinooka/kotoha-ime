---
feature: bugfix-23-convert-buffer-normalization
status: implemented
bounded_context: _uncategorized
related_issues: ["#23"]
related_prs: []
glossary_refs: ["canonical-romaji","hatsuon","kana","lefthook","romaji","sokuon"]
last_reviewed: 2026-05-05
---

# #23 hotfix: convert() / flush() 終端 buffer 正規化

> **Migration note**: 本 spec は `docs/wbs/2026-04-23-bugfix-23-convert-buffer-normalization.md` から spec 化 (Global CLAUDE.md §Documentation Structure 準拠の `docs/wbs/` 廃止対応、2026-05-05)。
> 元 WBS は実装ログ性質。14-section restructure は別 follow-up 起票予定 (各 project の既存 14-section follow-up issue と合流可能)。

## 元 WBS 内容 (実装ログ由来)

milestone: hotfix
branch: bugfix/23-convert-buffer-normalization
pr: "#27"
merge_commit: "a42a8ce"
issue: "#23"
status: done
started: 2026-04-23
finished: 2026-04-23
---

# #23 hotfix: convert() / flush() 終端 buffer 正規化

## 背景

M3b (PR #24) の property test strategy 探索で発見した M3a state machine バグ。`RomajiConverter::convert()` は char-by-char for-loop 後に `take_buffer()` でバッファをそのまま返すため、partial-then-invalid 遷移 (例: "byb" → settle が leading 'b' のみ削除し buffer="yb" で終了) により、返却される pending が「不安定」な状態となる。同じ buffer を新たな `convert` 呼び出しに渡すと segment がさらに縮約されるため、whole 評価と split 評価で異なる結果となり、streaming IME として致命的な associativity 違反となる。

## 実施内容

### コミット順序

1. **4a3a93b** — `fix(kotoha-core): normalize convert() end-of-input buffer (#23)`
   - `crates/kotoha-core/src/romaji/state.rs` に `pub(crate) fn normalize(&mut self) -> String` を追加。settle の `Lookup::None` 経路 (sokuon / hatsuon / invalid-drop) を loop 化し、buffer が「empty / trie-partial / full-match」の安定状態に到達するまで削減。salvage 可能な kana (っ / ん / 途中で完成した rule) は返却 String に蓄積。
   - `crates/kotoha-core/src/romaji/mod.rs` の `convert()` を更新: for-loop 後に `normalize()` を呼び、salvage した kana を committed に追記してから `take_buffer()`。
   - 新規 unit tests: state.rs に 5 件 (normalize の挙動網羅)、mod.rs に 2 件 (byb class regression + 4 入力の pending 冪等性)。
   - Total: 63 → 70 unit tests。

2. **7889642** — `fix(kotoha-core): extend #23 fix to flush() and add salvage tests`
   - レビュー指摘 (Architecture Important 2 / Testing Minor 3) により、`flush()` が `normalize()` を呼んでいないため streaming path (`push('b'); push('y'); push('b'); flush()`) が依然 "yb" を返す #23 の variant が判明。`flush()` を修正: `normalize()` を先に呼び、salvage kana を prepend したうえで lone-n 特例を残す。
   - Testing Important 2 対応: misnamed test `normalize_salvages_sokuon_when_possible` (実際は normalize が trie-partial "k" に対し no-op であることを検証) を `normalize_is_noop_after_settle_already_emitted_sokuon` に改名。
   - Testing Important 1 対応: `normalize()` の `Lookup::Match` 分岐 (rustdoc で宣伝していたが未 test) を covering する 2 件を追加:
     - `convert_yba_salvages_ba_as_match_after_invalid_y_drop`: "yba" → "ば" (settle 経路内の Match)
     - `convert_byba_salvages_ba_via_normalize_match_branch`: "byba" → "ば" (normalize 経路内の Match)
   - 追加の flush regression 2 件: `flush_normalizes_streaming_byb_residue`、`flush_salvages_rule_match_then_lone_n`。
   - Testing Important 3 対応: `crates/kotoha-core/tests/romaji_property.rs` の `romaji_input` 戦略 docstring の `# TODO` から #23 を削除 (#22 は残置)。
   - Total: 70 → 74 unit tests。

### 最終メトリクス

- 変更ファイル: 3 (src/romaji/mod.rs / src/romaji/state.rs / tests/romaji_property.rs)。
- 追加行数: 264 insertions / 14 deletions (squash 後)。
- workspace `#[test]` 総数: 75 → **79** (74 unit + 2 golden + 2 property + 1 doc)。M3b 時点の 75 に 4 追加。
- clippy warnings: 0、fmt diff: 0、lefthook pre-push: 全通過。

## 設計判断

### normalize() を settle() と重複させた理由 (backtrack helper 抽出は follow-up)

レビュアー (Architecture Important 1) は settle / normalize 間の backtrack logic 重複を指摘。本 PR では「#23 hotfix」のスコープを守るため重複のままとし、`backtrack_once` helper 抽出は follow-up refactor で処理することとした。現時点で両者は同一 rule を実装しており、ISSUE #15 (spec §9.1 / §9.2 pending-buffer backtrack rule) 対応時に抽出すると効率的。

### flush() の lone-n 特例保持

`flush()` の "tail == n → ん" 特例は、`normalize()` 後も維持した。理由: "n" は trie Partial prefix (na/ni/...) のため `normalize()` は no-op で pass through する。ユーザーが「n で確定入力して ん を commit したい」streaming 意図は flush 固有の finalization 契約であり、normalize の責務 (stable state 化) とは直交する。

### convert("yba") の新 commit 挙動

レビュアー (Testing Important 1) の manual probe で発見: `convert("yba")` は修正前後で挙動が変化する。
- 修正前: settle が "yba" → "y" drop → "ba" で match → "ば" commit。この経路は変わらず、結果は ("ば", "")。
- 修正後も同じ。差分が出るのは `convert("byba")` 型: settle の経路内で "yba" → "ba" 残留後、末尾 'a' の settle で "ba" match できない変則経路 (実際は settle が match してしまうケースもあるが normalize が後拾いする場面もある)。両経路とも結果は ("ば", "") で仕様的に望ましい挙動。

## レビューで発見された項目 (対応状況)

| 指摘 | 分類 | 対応 |
|---|---|---|
| flush() が normalize 未呼び出し | Architecture Important | 本 PR 内で修正 (commit 7889642) |
| settle/normalize の backtrack 重複 | Architecture Important | follow-up 対応 (ISSUE #15 と合流予定) |
| normalize Lookup::Match 分岐の test 欠落 | Testing Important | 本 PR 内で 2 件追加 |
| misnamed sokuon test | Testing Important | 本 PR 内で改名 |
| property test TODO の #23 参照 stale | Testing Important | 本 PR 内で更新 |
| normalize rustdoc Preconditions 欠落 | Minor | follow-up |
| module header が settle のみ言及 | Minor | follow-up |
| golden fixture に partial-then-invalid 行なし | Minor | 既存 issue #25 に合流 |

## M3b 属性更新

PR #27 merge 後、M3b の property test の `romaji_input` 戦略 (`[aeioun]{0,12}`) は #22 のみを残した状態で broaden 可能となった。follow-up として別 PR で `[a-z]{0,12}` への拡大 + `with_cases` 引き上げ (ISSUE #26) を計画。

## M4 への申し送り

- `convert()` / `flush()` ともに返却 pending が「empty / trie-partial / full-match」の安定形に正規化されるようになった。`input::InputContext` が将来 `convert` / `flush` を直接または間接に呼んだ際、返却 pending の再入力が idempotent であることに依存した設計が可能。
- `normalize()` は `pub(crate)` で、input モジュールからも (同一 crate 内であれば) 直接利用可能。ただし通常は `convert` / `flush` 経由で十分。
- 残る M3a gap は #22 (非 ASCII retraction policy) のみ。ADR (#16 canonical-romaji) での正規化を待つ。

## 成果物リンク

- PR: https://github.com/std-koh-hinooka/kotoha-ime/pull/27
- ISSUE: https://github.com/std-koh-hinooka/kotoha-ime/issues/23 (CLOSED)
- Related: #22 (open, non-ASCII retraction policy ADR 待ち)
- Spec: `docs/superpowers/specs/2026-04-22-kotoha-phase-0-design.md`
- Related PR: #24 (M3b — #23 を発見した original PR)
