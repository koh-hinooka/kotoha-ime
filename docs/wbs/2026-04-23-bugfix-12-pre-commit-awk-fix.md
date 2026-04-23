# 2026-04-23 bugfix/12 pre-commit awk fix — 実装ログ

## メタデータ

- **対象 ISSUE**: #12 — chore: fix awk logic bug in scripts/pre-commit-doc-naming.sh (CLAUDE.md progress-info guard)
- **対象 PR**: #43 — fix(hooks): doc-naming awk exit-code portability + TOC exemption (#12)
- **作業ブランチ**: `bugfix/12-pre-commit-awk-fix`
- **ベースブランチ**: `develop`
- **Merge commit SHA**: `9635608`
- **PR サイズ区分**: Small (3 files / 154 insertions / 3 deletions)

## 背景

`scripts/pre-commit-doc-naming.sh` の `check_claude_md_progress` 関数に埋め込まれていた awk が、mawk 1.3.4 (Ubuntu/Debian 標準の `/usr/bin/awk`) において期待どおりに動作していなかった。具体的には、マッチブロック内の `exit 0` の後でも `END` ブロックが実行され、その END が `exit 1` を返すことで「違反検出」の終了コードがサイレントに上書きされていた。結果、CLAUDE.md の進捗情報ガードは M1 以降ずっと no-op として放置されていた。

PR #11 のレビュー過程で偶発的に発覚した潜在バグであり、実害は未発生だった (Low priority として ISSUE 化)。

## 実施内容

### 1. awk ロジックの修正

`scripts/pre-commit-doc-naming.sh:111-118` の awk を `exit 0 ... END { exit 1 }` から `found=1 ... END { exit !found }` パターンに書き換えた。mawk / gawk 双方で一貫した終了コードを返す。

### 2. TOC 除外フィルタの追加

CLAUDE.md 冒頭の目次 (`- [ラベル](#アンカー)` 形式) が進捗語彙と字面一致してガードを誤発火させないよう、`^\+[[:space:]]*-?[[:space:]]*\[[^][]+\]\(#[^()]+\)[[:space:]]*$` を negative-match パターンに追加した。行全体が `[text](#anchor)` の形をとる場合に限定しているため、`[x]` / `[ ]` チェックボックスの検出は破壊しない。

### 3. 自己テストの追加

`scripts/tests/test-doc-naming.sh` を新規作成し、8 件のフィクスチャ (5 件初版 + レビュー指摘を受けた 3 件追加) で awk ロジックを直接検証する。`git diff` には依存せず、合成 diff を awk に流すことで再現性とテスト独立性を確保した。

- case1: `+TODO: foo` → 違反検出
- case2: `+ここでWBSを運用` (日本語進捗語) → 違反検出
- case3: `+- [WBS 直接 push の例外](#wbs-直接-push-の例外)` (TOC) → 除外、違反なし
- case4: 進捗語を含まない平文追加 → 違反なし
- case5: バッククォート内の `\`todo\`` → 違反なし (gsub 除去)
- case6: 削除のみの diff → 違反なし
- case7: 削除行の `-TODO` → 違反なし (`+` 行のみを対象とする)
- case8: `→` を含む `+` 行 → 違反なし (既存の除外パターン)

### 4. lefthook pre-commit への組み込み

`lefthook.yml` の `pre-commit` セクションに `doc-naming-self-test` エントリを追加。awk ロジックがサイレントに壊れた場合、commit 時点で必ず検出される。

## レビュー対応

`agent-teams:team-review` は `CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS` 未設定のためインライン fallback レビューを実施 (Security / Architecture / Testing の 3 次元)。

| 次元 | Severity 別 finding 数 | 判定 |
|------|-------------------------|------|
| Security | Info×3 / Low×1 | APPROVE |
| Architecture | Medium×1 (script と self-test の awk プログラム重複) / Low×1 / Info×1 | APPROVE (Small-tier 枠内では同期コメントで許容。extract は follow-up 候補) |
| Testing | Medium×1 (エッジケース追加推奨) / Low×2 / Info×2 | APPROVE |

`secrets-check`: 0 件 (手動パターンスキャンで API key / token / password / DB URL のいずれも未検出)。

Medium の Testing 指摘のうち、スコープ内で即対応可能な 3 件 (case6-8) を追加で commit し、同一 PR に積んだ。

## 検証結果

- `bash scripts/tests/test-doc-naming.sh` → PASS=8 / FAIL=0 (mawk 1.3.4)
- `shellcheck scripts/pre-commit-doc-naming.sh scripts/tests/test-doc-naming.sh` → exit 0
- `cargo fmt --all --check` → exit 0
- `cargo build --workspace` → exit 0
- `cargo test --workspace` → 100 + 3 + 6 + 2 + 5 + 2 = 118 tests all green
- `cargo clippy --workspace --all-targets -- -D warnings` → exit 0
- lefthook pre-commit / pre-push → 全ゲート PASS

## Follow-up 候補 (本 PR 外)

- script 本体と self-test 間の awk プログラム重複を `scripts/lib/claude-md-progress.awk` に抽出する案。現状は同期必須コメントで運用しており、将来 drift が観測された場合に別 ISSUE 化する。

## 参考リンク

- PR: https://github.com/std-koh-hinooka/kotoha-ime/pull/43
- ISSUE: https://github.com/std-koh-hinooka/kotoha-ime/issues/12
