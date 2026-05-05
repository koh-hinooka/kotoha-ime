---
feature: docs-56-adr-0008-approve-option-1
status: deprecated
deprecated_reason: "Phase E migration で旧 docs/wbs/ から spec 化した実装ログ性質の文書。Global CLAUDE.md §Development Flow legacy spec 取扱いルール (実装ログ性質 → status: deprecated、本文 14-section restructure 不要) に基づき deprecated 扱い。git history は参照点として保持 (2026-05-06)。"
bounded_context: _uncategorized
related_issues: ["#56"]
related_prs: []
glossary_refs: ["canonical-romaji","kana","romaji"]
last_reviewed: 2026-05-06
---

# 2026-04-24 ADR 0008 Status 昇格ログ

> **Migration note**: 本 spec は `docs/wbs/2026-04-24-docs-56-adr-0008-approve-option-1.md` から spec 化 (Global CLAUDE.md §Documentation Structure 準拠の `docs/wbs/` 廃止対応、2026-05-05)。
> 元 WBS は実装ログ性質。14-section restructure は別 follow-up 起票予定 (各 project の既存 14-section follow-up issue と合流可能)。

## 元 WBS 内容 (実装ログ由来)


- **ISSUE**: #56
- **PR**: #57 (merge SHA `516c490`)
- **Branch**: `docs/adr-0008-approve-option-1` (merge 後削除済み)

## 実施内容

- `docs/adr/0008-canonical-romaji-and-partial-invertibility.md` の Status を「提案 (2026-04-23) — Phase 1 で最終決定する placeholder」から「承認 (2026-04-24)」へ昇格。
- タイトル末尾を "(Phase 1 検討事項)" から "— Phase 1 判断: 現状維持" へ変更。
- 決定セクションを placeholder 文から実決定 (選択肢 1 採用: canonical romaji を定義しない) へ全文書き換え。Phase 1 Kana→Kanji 変換層とのレイヤー独立性を明記。
- 影響セクションを 4 項目 (Phase 0 rule table / Phase 1 以降 / property test / 将来の選択肢 2・3 への migration path) で充実化。
- 参照セクションに「Phase 1 kick-off review (2026-04-24)」の 1 行追記。
- `docs/ROADMAP.md` の「Phase 1 への申し送り」セクションの ADR 0008 項目を「judging-pending placeholder」表記から「判定済み」表記へ更新。

## 背景と判断理由

2026-04-24 の Phase 1 kick-off review で ADR 0008 記載の 3 選択肢 (現状維持 / canonical 定義 / configuration) を再評価し、選択肢 1 (現状維持) を採用。採用理由は以下の 4 点:

1. **Phase 1 レイヤー独立性**: Kana→Kanji 変換層は入力が既にひらがな (`じ` / `つ` / `し` / `ち` に正規化済み) であり、romaji 側の多対一は独立レイヤー。本決定は Phase 1 実装に影響しない。
2. **scope creep 回避**: Phase 0 完了直後に rule table 213 entries の正規化作業を入れると scope が膨らむ。
3. **future vision との両立**: 記号マクロ (例: `xl` → `→`) や convention 多様化は Phase 0 rule table に key を追加するだけで実装可能で、canonical romaji 経由は不要。選択肢 1 と両立する。
4. **migration path 確保**: 将来 canonical romaji が必要となる use case (例: kanji → reading 逆引き表示) が発生した場合は、(a) display layer で canonical 化、もしくは (b) 新 ADR で選択肢 2 / 3 採用、の 2 通りで対応可能。本 ADR 影響セクションに記録済み。

## 成果物リンク

- ISSUE: <https://github.com/std-koh-hinooka/kotoha-ime/issues/56>
- PR: <https://github.com/std-koh-hinooka/kotoha-ime/pull/57>
- merge commit: `516c490`
- ADR 本体: `docs/adr/0008-canonical-romaji-and-partial-invertibility.md`
- ROADMAP 更新箇所: `docs/ROADMAP.md` § Phase 1 への申し送り

## 次のステップ

- Phase 1 (Kana→Kanji conversion) の brainstorming を別セッションで開始する。本 ADR で canonical romaji が Phase 1 scope から外れたため、Phase 1 設計は romaji 層との interface を「ひらがな文字列」に固定した前提で進められる。
