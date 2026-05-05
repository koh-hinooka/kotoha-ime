---
feature: feature-1-project-setup
status: implemented
bounded_context: _uncategorized
related_issues: ["#1"]
related_prs: []
glossary_refs: ["lefthook"]
last_reviewed: 2026-05-05
---

# M1: Project setup

> **Migration note**: 本 spec は `docs/wbs/2026-04-22-feature-1-project-setup.md` から spec 化 (Global CLAUDE.md §Documentation Structure 準拠の `docs/wbs/` 廃止対応、2026-05-05)。
> 元 WBS は実装ログ性質。14-section restructure は別 follow-up 起票予定 (各 project の既存 14-section follow-up issue と合流可能)。

## 元 WBS 内容 (実装ログ由来)

milestone: M1
branch: feature/1-project-setup
pr: "#2"
merge_commit: "8873e7722fc6d6cb4da6de8a20ce00873224f425"
issue: "#1"
status: done
started: 2026-04-22
finished: 2026-04-22
---

# M1: Project setup

## 実施内容

- git init + main / develop / feature/1-project-setup branch 作成
- GitHub repo `std-koh-hinooka/kotoha-ime` を private で作成、default branch を develop に変更
- M1 用 ISSUE #1 作成
- Cargo workspace scaffold (`Cargo.toml`、空 workspace members)
- docs/ 骨組み配置 (`docs/ROADMAP.md`、`docs/adr/0000-template.md`、`docs/wbs/template.md`)
- .github/ テンプレート配置 (`ISSUE_TEMPLATE.md`、`PULL_REQUEST_TEMPLATE.md`)
- Project-specific `CLAUDE.md` 配置(英語 commit message 例外 + WBS 直接 push 例外を明記)
- Dual MIT/Apache-2.0 ライセンスファイル配置
- lefthook 設定 + `scripts/pre-commit-doc-naming.sh` 配置 + `lefthook install`
- `.gitignore` に `.claude/` 追加
- `README.md` のライセンスセクション更新

## つまずき

1. Cargo 1.94+ が空の virtual workspace をハードエラーにする仕様変更に当初プランが未対応 → `cargo build --workspace` 検証を `cargo metadata --no-deps --format-version=1` に差し替え、M1-4 Step 2 / M1-10 Step 3 / M1 完了条件 / M2 受入条件を plan 改訂 (`aea2134`)
2. lefthook pre-push が同じ理由で cargo build/clippy/test を呼ぶと M1 期間中は落ちる → `if [ -d crates ]; then ... else echo "lefthook: skip ..."; fi` の shell ガードで M1 中は skip、M2 で crates/ が現れた時点で自動有効化する設計に改訂 (`27f8e14`)
3. GitHub Free plan の private repo は branch protection API が 403 を返すため、M1-2 Step 4 は graceful skip

## M2 への申し送り

- Cargo workspace の `[workspace.dependencies]` は M2 で `thiserror`, `anyhow`, `tracing`, `tracing-subscriber` を追加する
- lefthook の fmt-check の実効性は M2 で kotoha-core が追加されて初めて検証できる(M1 では `.rs` ファイル 0 件のため発火しない)
- M2 で最初の crate 追加時、`crates/kotoha-core/` 作成と `Cargo.toml` の workspace `members` 更新を同一 commit で行う(中間状態では lefthook pre-push の `[ -d crates ]` ガードが crate 空状態でエラー起こす可能性がある点に注意)
- ADR 0001-0003 は M7 で作成。M1 PR レビューで追加の ADR 候補 (MSRV 選定、M1 cargo build 例外、LICENSE holder、公開メール埋め込み) が挙がっているので M7 で集約

## PR review 指摘の follow-up

すべて Critical/High のブロッカーなし。以下は後続タスクで対応予定:

- M7 ADR 対象拡張: MSRV/edition 選定理由、M1 cargo build 例外、LICENSE copyright holder、Cargo.toml 公開メール埋め込み方針
- M2 kickoff 前の lefthook 小改善 PR: `[ -d crates ]` → member 数検査に強化、無害化方式統一、doc-naming に glob 付与、pre-push test timeout、`.gitignore` にシークレットパターン追加
- ~~Phase 1 前: GCP Cloud Build trigger 構築を ROADMAP に追加~~ → 廃止 (PR #4 で spec §12.2 改訂、Kotoha はローカル実行の日本語 IME で GCP 不要と判断)
- .github テンプレートの英訳
- CLAUDE.md 改善(TOC、API doc 明確化、proptest 注記、glossary stub)

## 成果物リンク

- PR: #2 (squash merge)
- ISSUE: #1
- Spec: `docs/superpowers/specs/2026-04-22-kotoha-phase-0-design.md`
- Plan: `docs/superpowers/plans/2026-04-22-kotoha-phase-0-implementation.md` (M1 section)
