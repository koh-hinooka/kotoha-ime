---
feature: docs-34-input-mode-adr
status: implemented
bounded_context: _uncategorized
related_issues: ["#34"]
related_prs: []
glossary_refs: []
last_reviewed: 2026-05-05
---

# M4a: ADR 0002 — input mode Transient vs Sticky + spec §8.5 ref fix

> **Migration note**: 本 spec は `docs/wbs/2026-04-23-docs-34-input-mode-adr.md` から spec 化 (Global CLAUDE.md §Documentation Structure 準拠の `docs/wbs/` 廃止対応、2026-05-05)。
> 元 WBS は実装ログ性質。14-section restructure は別 follow-up 起票予定 (各 project の既存 14-section follow-up issue と合流可能)。

## 元 WBS 内容 (実装ログ由来)


## 実施内容

- ADR 0002 `docs/adr/0002-input-mode-transient-vs-sticky.md` を新規作成 (Status: 承認、Karukan 非互換 / Mozc 互換の Transient-by-default 判断を記録、3 つの代替案を検討)。
- spec `docs/superpowers/specs/2026-04-22-kotoha-phase-0-design.md` §8.5 (line 406) と §13.3 (lines 645–647) の ADR 参照番号を修正。canonical numbering = 0001 retraction / 0002 input-mode / 0003 shift / 0004 cli-line。
- team-review (security + architecture + testing) で検出された Medium 1 件 (ADR line 21 の `(PR #)` プレースホルダ未埋め) を即時修正。
- follow-up ISSUE #36 を作成 (spec line 672 / 748 の "ADR 3 件" 総数 narrative を将来 "4 件" に更新するためのフォロー)。

## つまずき

- plan PR #33 の Task M4a-2 は spec §8.5 (line 406) のみを修正対象と想定していたが、実際には §13.3 checklist (lines 645–647) にも古い numbering が残っていた。ユーザ判断により Option #2 (M4a 内で全 numbering を一括で canonical scheme に揃える) を採用し、scope を line 406 単独 → line 406 + line 645–647 の 4 箇所へ拡張。Small tier (≤5 files / ≤100 lines) の範囲内で収まった。
- narrative references (line 672 / 748 の "ADR 3 件" 総数) は ADR 番号付けではなく Phase 0 計画の内容 (総 ADR 数 3→4) の更新なので M4a scope 外とし、follow-up ISSUE #36 で別途扱う。

## M4b への申し送り

- ADR 0002 で `ModeOrigin::Sticky / Transient` の 2 軸モデルが normative 化された。M4b の `input/mode.rs` 実装では Shift トリガ由来の Direct モード遷移は必ず `Transient` origin で記録し、commit / cancel 契機で `(Hiragana, Sticky)` へ自動復帰するロジックを state machine に組み込むこと。
- `ModeOrigin` は `pub(crate)` 可視性で、外部 API には露出しない (ADR 0002 決定事項)。
- 将来フラグ `allow_transient_to_sticky_promotion` の default は `false`。M4b では実装しなくてよいが、enum / struct 設計上 Phase 3 で容易に追加できる余地を残すこと。

## 成果物リンク

- ISSUE: #34
- PR: #35 (merge commit: `f40e1b6fddc5b710f01c0f6ca254a27a5ac1269d`)
- Follow-up ISSUE: #36
- ADR: `docs/adr/0002-input-mode-transient-vs-sticky.md`
- spec edit: `docs/superpowers/specs/2026-04-22-kotoha-phase-0-design.md` §8.5 / §13.3
