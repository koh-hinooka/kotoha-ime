## 概要

<!-- PRの目的を簡潔に説明 -->

Closes #[ISSUE番号]

## 変更内容

-

## 動作確認

- [ ] ユニットテスト通過
- [ ] 既存テストに影響なし
- [ ] ローカルで動作確認済み
- [ ] (UI/frontend 変更時) ブラウザで golden path + edge case 動作確認済み

## レビュー観点

<!-- レビュアーに特に確認してほしい点 -->

## スクリーンショット

<!-- UI変更がある場合のみ -->

---

## レビュー実施記録 (PR Review Matrix)

**PR サイズ判定**: [ ] Small (≤7files/200lines) / [ ] Medium (≤15/600) / [ ] Large (≤20/1000)

### 必須レビュー (CLAUDE.md §PR Review Matrix 参照)

- [ ] `agent-teams:team-review` (Small: security+arch+testing / Medium・Large: 全 5 dims)
- [ ] `secrets-check`
- [ ] (Medium 以上) `owasp-security`
- [ ] (Medium 以上) `security-scanning:security-sast`
- [ ] (Large) `pr-review-toolkit:review-pr`
- [ ] (Large・pre-release) `audit`

### Trigger-based add-ons (該当時のみ実行)

- [ ] (UI/frontend) `ui-design:accessibility-audit` + `web-design-guidelines`
- [ ] (Auth/crypto/input validation) `security-scanning:security-hardening`
- [ ] (DB schema/migration) `database-migrations:sql-migrations` (review-only)
- [ ] (LLM/AI integration) `owasp-security` (Agentic AI section)
- [ ] (Critical path / 大規模 refactor) `/ultrareview`
- [ ] **(新規 spec 追加 / 依存変更 / 構造変更 / 開発手順変更) `claude-md-management:claude-md-improver`**

### Finding Resolve 宣言

- [ ] 全 review skill の出した **finding を同 PR 内で resolve 完了** (severity 問わず)
- [ ] follow-up ISSUE への先送りは **以下の例外条件のみ** 許容、該当時は下記に記載:
  - 別 bounded context への影響で本 PR scope 逸脱
  - 技術調査必要で即修正不能
  - 仕様判断必要で日野岡さん確認待ち

  follow-up: #<ISSUE番号> / 理由: <記載>

---

## CLAUDE.md / README.md 更新確認 (自己申告)

- [ ] (trigger 該当 PR) `claude-md-improver` 実行済み
- [ ] CLAUDE.md / README.md / 関連 docs の更新要否を確認した (不要なら本 PR では更新なし)

---

## post-merge follow-up (merge 後に PR コメントで実施結果を宣言)

CLAUDE.md §post-merge follow-up checklist を merge 直後に実施。各項目を「実施」「該当なし」で PR コメントに記載:

- [ ] Spec status 更新 (`status: implemented` 等)
- [ ] Glossary 同期 (`~/Obsidian/<scope>/glossary/<concept>.md`)
- [ ] CLAUDE.md / README.md 更新
- [ ] ROADMAP 更新 (機能完了 `[x]` / 仕様変更反映 / マイルストーン完了判定)
- [ ] Vault 同期 (Scope Lifecycle 該当時)

---

## 参照

- 仕様書: `docs/specs/{_uncategorized,<bounded-context>}/<feature-slug>.md`
- 計画書: `docs/plans/<yyyy-MM-dd>-<branch>.md`
- ROADMAP: `docs/ROADMAP.md` (Active: vX.Y.Z)
