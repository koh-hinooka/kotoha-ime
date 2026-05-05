---
feature: docs-16-canonical-romaji-adr-tracking-hook
status: deprecated
deprecated_reason: "Phase E migration で旧 docs/wbs/ から spec 化した実装ログ性質の文書。Global CLAUDE.md §Development Flow legacy spec 取扱いルール (実装ログ性質 → status: deprecated、本文 14-section restructure 不要) に基づき deprecated 扱い。git history は参照点として保持 (2026-05-06)。"
bounded_context: _uncategorized
related_issues: ["#16"]
related_prs: []
glossary_refs: ["canonical-romaji","lefthook","romaji"]
last_reviewed: 2026-05-06
---

# 2026-04-23 docs/16-canonical-romaji-adr-tracking-hook 実装ログ

> **Migration note**: 本 spec は `docs/wbs/2026-04-23-docs-16-canonical-romaji-adr-tracking-hook.md` から spec 化 (Global CLAUDE.md §Documentation Structure 準拠の `docs/wbs/` 廃止対応、2026-05-05)。
> 元 WBS は実装ログ性質。14-section restructure は別 follow-up 起票予定 (各 project の既存 14-section follow-up issue と合流可能)。

## 元 WBS 内容 (実装ログ由来)


## 概要

ISSUE #16「canonical romaji ADR 候補」の tracking hook を 3 箇所に設置した。spec §11.3 revision 2 が Phase 1 送りとした論点を placeholder ADR として docs/adr/ に顕在化させ、ROADMAP.md と spec から相互リンクを貼ることで Phase 1 計画時の見落としを構造的に防止する。

## 実施内容

- ADR 0008 stub 作成 (`docs/adr/0008-canonical-romaji-and-partial-invertibility.md`, 提案 status, 32 行)。Phase 1 で正式評価する 3 選択肢 (現状維持 / canonical 定義 / configuration で選択) を列挙。
- ROADMAP.md に "Phase 1 への申し送り" subsection を新設 (Phase 0 マイルストーン の直後、注記 の直前)。ADR 0008 を carry-over 項目として明示。
- spec §11.3 の「ADR 候補」表現を `ADR 候補: docs/adr/0008-canonical-romaji-and-partial-invertibility.md — 提案ステータスの placeholder` に upgrade し、dangling commitment を concrete pointer へ変換。

## つまずき

- ISSUE #16 の body は stub 番号を `0005` と指定していたが、#46 で確定した canonical scheme で 0005 (Trie) / 0006 (non_exhaustive) / 0007 (rust-toolchain, M7 予約) が消費済みのため、次の空き番号 `0008` を採番。commit message と PR body で番号選択理由を明記。

## 成果物リンク

- ISSUE #16: https://github.com/std-koh-hinooka/kotoha-ime/issues/16
- PR #48: https://github.com/std-koh-hinooka/kotoha-ime/pull/48
- develop merge SHA: `4a63a61`
- ADR: `docs/adr/0008-canonical-romaji-and-partial-invertibility.md`
- ROADMAP anchor: `docs/ROADMAP.md` "Phase 1 への申し送り"
- spec cross-link: `docs/superpowers/specs/2026-04-22-kotoha-phase-0-design.md` §11.3

## 検証結果

- `cargo build --workspace` / `cargo test --workspace` / `cargo clippy --workspace --all-targets -- -D warnings` / `cargo fmt --all --check` すべて exit 0
- lefthook pre-commit (doc-naming) / pre-push (build + clippy + manifest-check + test) すべて通過
- Small tier inline review (security / architecture / testing / secrets-check) で Critical / High 0 件
