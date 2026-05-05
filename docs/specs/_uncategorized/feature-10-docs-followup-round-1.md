---
feature: feature-10-docs-followup-round-1
status: deprecated
deprecated_reason: "Phase E migration で旧 docs/wbs/ から spec 化した実装ログ性質の文書。Global CLAUDE.md §Development Flow legacy spec 取扱いルール (実装ログ性質 → status: deprecated、本文 14-section restructure 不要) に基づき deprecated 扱い。git history は参照点として保持 (2026-05-06)。"
bounded_context: _uncategorized
related_issues: ["#10"]
related_prs: []
glossary_refs: ["romaji"]
last_reviewed: 2026-05-06
---

# docs follow-up round 1: English templates + CLAUDE.md refinements

> **Migration note**: 本 spec は `docs/wbs/2026-04-23-feature-10-docs-followup-round-1.md` から spec 化 (Global CLAUDE.md §Documentation Structure 準拠の `docs/wbs/` 廃止対応、2026-05-05)。
> 元 WBS は実装ログ性質。14-section restructure は別 follow-up 起票予定 (各 project の既存 14-section follow-up issue と合流可能)。

## 元 WBS 内容 (実装ログ由来)

milestone: docs-followup
branch: feature/10-docs-followup-round-1
pr: "#11"
merge_commit: "6a431fc"
issue: "#10, #5"
status: done
started: 2026-04-23
finished: 2026-04-23
---

# docs follow-up round 1: English templates + CLAUDE.md refinements

## 実施内容

- `.github/ISSUE_TEMPLATE.md` / `.github/PULL_REQUEST_TEMPLATE.md` を日本語から英語へ全訳し、プレースホルダ名も `[ISSUE番号]` → `[ISSUE_NUMBER]`、`[ブランチ名]` → `[BRANCH_NAME]` へ統一(CLAUDE.md の英語 policy と整合)
- `CLAUDE.md` に目次(8 項目)を追加して navigation を改善
- `CLAUDE.md` 内で `rustdoc / API doc` の方針記述を「日本語を維持する対象」リストから「英語で記述する対象」リストへ移動し、両リストを相互排他にした。rationale(OSS 貢献者との互換性)は従来どおり保持
- `CLAUDE.md` の proptest 記述を改訂し、M3(dev-dep 追加 + romaji property test)と M5(input mode property test)の 2 milestone を明示。review 指摘(plan 本体との不整合)を修正
- `docs/wiki/glossary.md` を新規作成(stub)。今後のドメイン用語エントリ形式 legend を記載し、`CLAUDE.md` に Glossary セクションを追加して参照リンクを張った
- `.github/PULL_REQUEST_TEMPLATE.md` の `Closes #[ISSUE_NUMBER]` 直上に placeholder 置換ヒントの HTML コメントを追加(testing review 指摘の silent-failure 防止)
- 3 commits を branch に積み、squash merge で develop に統合(base `863b6b3` → merge `6a431fc`、4 files changed、+81 / -40)
- 既存テスト 25/25 が PASS のまま(コード変更なし、pre-push gate で検証済み)

### Branch 上の 3 commits

1. `379be9d` — docs: translate .github templates to English
2. `dc20e6e` — docs: refine CLAUDE.md and add glossary stub
3. `2cb8f78` — docs: address review findings (proptest milestones, PR template placeholder hint)

## つまずき

1. Sub-agent が `rustdoc / API doc` の bullet を「日本語を維持する対象」リスト内に配置した。主エージェントの委任指示で配置先リストを明示しなかったのが原因。主エージェントの Read で矛盾を検出し、再度 sub-agent 委任で「英語で記述する対象」リストへ移動して修正
2. 当初 plan では proptest の初出を「M3–M4」と記載していたが、architecture review と testing review の両方で plan 本体(`docs/superpowers/plans/2026-04-22-kotoha-phase-0-implementation.md` および M2 plan)と不整合と指摘され、修正 commit を追加。実際は M3(dev-dep + romaji property test)と M5(input mode property test)の 2 milestone
3. Testing review が pre-existing の awk バグを検出(`scripts/pre-commit-doc-naming.sh` の `check_claude_md_progress` 関数)。PR #11 範囲外のため follow-up ISSUE として切り出して対応(ISSUE #12 起票済み)

## Review

Small tier(≤5 files / ≤100 lines)に従い、`agent-teams:team-review` の 3 dimension(security / architecture / testing)を並列 dispatch。gitleaks 未インストールのため security agent が defense-in-depth の grep による代替検査を実施(zero match)。

- Security: **APPROVED**(findings 無し)
- Architecture: **APPROVED_WITH_OBSERVATIONS**(Low: proptest milestone 記述が plan 不整合 → commit `2cb8f78` で修正済み)
- Testing: **APPROVED_WITH_OBSERVATIONS**(Low 3 件: proptest 記述の同件 / PR template `Closes #` placeholder の silent-failure / 既存 awk バグ)
- 指摘対応: proptest 記述修正 + PR template HTML コメント追加の 2 件を commit `2cb8f78` に集約。awk バグは別 ISSUE #12 に切り出し

## M3 への申し送り

1. proptest の dev-dep 追加は M3 の最初のタスク(plan 通り)。本 WBS で `CLAUDE.md` を「M3 と M5」と明記したので、M3 plan 実行時に romaji property test を、M5 plan 実行時に input mode property test を wire up する
2. `docs/wiki/glossary.md` は現時点で stub(legend のみ)。M3 の romaji 変換モジュール実装時にドメイン用語(ローマ字、かな、長音、撥音、拗音、促音、テーブルドリブン、trie など)が定義されるので、M3 WBS で `glossary.md` にも追記する
3. follow-up ISSUE #12(awk バグ修正)は Low priority。M3 着手前のタイミング、または M3 着手中の小休止タイミングで Small PR として処理する
4. `.github/` templates が英語化されたので、M3 以降の ISSUE 起票・PR 作成ではこの英語テンプレートが自動適用される

## 成果物リンク

- PR: #11 (squash merge, merge commit `6a431fc`)
- ISSUE: #10 (Closed), #5 (Closed)
- Follow-up ISSUE: #12(awk バグ修正、Open、Low priority)
- 影響ファイル: `.github/ISSUE_TEMPLATE.md`, `.github/PULL_REQUEST_TEMPLATE.md`, `CLAUDE.md`, `docs/wiki/glossary.md`(新規)
