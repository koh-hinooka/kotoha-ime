---
feature: docs-46-plan-adr-canonical-renumber
status: implemented
bounded_context: _uncategorized
related_issues: ["#46"]
related_prs: []
glossary_refs: ["romaji","romaji-trie"]
last_reviewed: 2026-05-05
---

# #46: Phase 0 plan — M7 ADR canonical renumber

> **Migration note**: 本 spec は `docs/wbs/2026-04-23-docs-46-plan-adr-canonical-renumber.md` から spec 化 (Global CLAUDE.md §Documentation Structure 準拠の `docs/wbs/` 廃止対応、2026-05-05)。
> 元 WBS は実装ログ性質。14-section restructure は別 follow-up 起票予定 (各 project の既存 14-section follow-up issue と合流可能)。

## 元 WBS 内容 (実装ログ由来)


## 実施内容

- plan `docs/superpowers/plans/2026-04-22-kotoha-phase-0-implementation.md` の M7 関連 ADR 参照 5 箇所を canonical scheme (0001〜0007) に揃えた。canonical scheme は 0001 non-ASCII retraction (PR #28 shipped) / 0002 input-mode Transient vs Sticky (PR #35 shipped) / 0003 shift-via-uppercase-char (reserved) / 0004 cli-line-based-commit (reserved) / 0005 romaji Trie over HashMap (PR #45 shipped) / 0006 non_exhaustive on streaming enums (PR #45 shipped) / 0007 rust-toolchain-and-publishing-policy (reserved、旧番号 0004 から renumber)。
- 編集箇所: line 29 工数テーブル M7 行 (`ADR 3 件` → `ADR 7 件`)、line 873 narrative (`0001 / 0002 / 0003` → `0003 / 0004 / 0007`、前倒し作成の経緯注記を併記)、lines 1055-1057 "Files to create" list (M7 で新規作成する 3 件のみに縮約、ファイル名を canonical スキームに合わせて更新)、line 1060 narrative (`ADR 0004` → `ADR 0007`)、line 1074 Acceptance (`ADR 3 件` → `ADR 7 件`、前倒し作成の経緯注記を併記)。
- 0004 → 0007 renumber の根拠: canonical スキームで 0004 = cli-line-based-commit が確定したため、旧 plan の `0004-rust-toolchain` は 0007 に繰り下げる必要があった (ISSUE body で事前承認済み)。
- 工数 `1 日` は据え置き: 4 件 shipped (0001 / 0002 / 0005 / 0006)、M7 で残り 3 件作成 (0003 / 0004 / 0007) なので元の `ADR 3 件 / 1 日` と同じ稼働量に収まる。
- 事前 / 事後 grep で stale ref ゼロ、cross-repo grep でも plan 外 orphan ゼロを確認。
- team-review (security + architecture + testing、inline fallback) + secrets-check は blocking finding ゼロ。
- cargo baseline (build / test / clippy / fmt) は docs-only のため不変、全 PASS (pre-push hook でも同一結果を確認)。

## つまずき

- 特になし。ISSUE body に 5 箇所の target location と 0004→0007 renumber の明示的承認が含まれていたため scope blurring なく完了。

## 成果物リンク

- ISSUE: #46
- PR: #47 (merge commit: `1b44627`)
- plan edit: `docs/superpowers/plans/2026-04-22-kotoha-phase-0-implementation.md` line 29 / 873 / 1055-1057 / 1060 / 1074
