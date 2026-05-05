---
feature: refactor-60-assert-sh
status: deprecated
deprecated_reason: "Phase E migration で旧 docs/wbs/ から spec 化した実装ログ性質の文書。Global CLAUDE.md §Development Flow legacy spec 取扱いルール (実装ログ性質 → status: deprecated、本文 14-section restructure 不要) に基づき deprecated 扱い。git history は参照点として保持 (2026-05-06)。"
bounded_context: _uncategorized
related_issues: ["#60"]
related_prs: []
glossary_refs: ["lefthook","romaji","zenz"]
last_reviewed: 2026-05-06
---

# P1-0: scripts/lib/assert.sh 抽出 + Phase 0 smoke refactor

> **Migration note**: 本 spec は `docs/wbs/2026-04-24-refactor-60-assert-sh.md` から spec 化 (Global CLAUDE.md §Documentation Structure 準拠の `docs/wbs/` 廃止対応、2026-05-05)。
> 元 WBS は実装ログ性質。14-section restructure は別 follow-up 起票予定 (各 project の既存 14-section follow-up issue と合流可能)。

## 元 WBS 内容 (実装ログ由来)

milestone: P1-0
branch: refactor/60-assert-sh
pr: "#61"
issue: "#60"
status: done
started: 2026-04-24
finished: 2026-04-24
---

# P1-0: scripts/lib/assert.sh 抽出 + Phase 0 smoke refactor

## 実施内容

- `scripts/lib/assert.sh` を新規作成。`assert_equal` / `assert_contains` / `assert_summary` の 3 関数と、`ASSERT_PASS` / `ASSERT_FAIL` の 2 counter 変数を提供する。
- `scripts/phase0-smoke.sh` を `source lib/assert.sh` 経由に refactor。10 件の assertion は verbatim 維持 (引数順のみ `desc / expected / actual` → `desc / actual / expected` に rename)。
- 末尾の手書き summary echo + exit 判定を `assert_summary "phase0-smoke"` 呼び出しに統合。
- FAIL path の動作確認 (scratch test で assertion 1 の expected を `こんばんは` に故意破壊 → `9/10 PASS, 1/10 FAIL, exit 1` を確認) を実施し、restore 済み。

## つまずき

- 特になし。plan に従って Write / Edit / Bash の順に粛々と作業するだけで完了した。
- assert.sh 単体の scratch self-test で `|| echo "exit 1 as expected"` が発火しなかった点は、`assert_summary` 内部が `return` ではなく `exit` を呼ぶため subshell が即時終了する仕様上の挙動であり、plan の意図通り。

## Regression 検証

- `bash scripts/phase0-smoke.sh` (refactor 後、PR merge 前): `10/10 PASS, 0/10 FAIL`、exit 0
- `bash scripts/phase0-smoke.sh` (develop merge 後): `10/10 PASS, 0/10 FAIL`、exit 0
- FAIL case scratch test: `9/10 PASS, 1/10 FAIL`、exit 1 (期待通り動作)
- `cargo build --workspace`: `Finished dev profile in 0.03s`、exit 0
- `cargo test --workspace`: 129 tests 全 PASS (9 cli unit + 102 core unit + 3 mode_golden + 6 mode_property + 2 romaji_golden + 5 romaji_property + 2 doctests + 0 romaji binary doc)
- `cargo clippy --workspace --all-targets -- -D warnings`: `Finished dev profile in 0.07s`、warnings 0
- `cargo fmt --all --check`: no output、exit 0
- lefthook pre-commit (doc-naming-self-test 8/8 PASS) + pre-push (build / clippy / manifest-check / test 全 PASS)

## Review 結果

- Small tier review (security + architecture + testing 3 dimensions、experimental flag 未設定のため inline fallback で代替) + secrets-check を実施
- Critical / High / Medium findings: 0 件
- Low findings: 1 件 (`assert.sh` 自体の dedicated test harness (bats) なし) — 本 PR の scope 外として acknowledge。P1-3 の `phase1-smoke.sh` で `assert_contains` を間接的に exercise する。
- Info findings: 1 件 (library 側に `set -euo pipefail` を置かない — sourced library は caller の options を inherit する慣習に従う、file header に明記済み)
- secrets-check: CLEAN (API key / token / password / private key のいずれも検出なし)

## P1-1 への申し送り

- `assert_contains` は P1-3 の `scripts/phase1-smoke.sh` で stochastic な Zenz 出力検証に使用される想定。Phase 2 以降の smoke script も本 library を source する形で統一する。
- `scripts/lib/` directory は今後 `logging.sh` / `model-path.sh` など Phase 1+ 用の helper を置く場として活用可能。
- 引数順規約 (`desc / actual / expected`) は xUnit 系 / rspec 系と一致。Phase N smoke でも同順序を守ること。

## 成果物リンク

- ISSUE: <https://github.com/std-koh-hinooka/kotoha-ime/issues/60>
- PR: <https://github.com/std-koh-hinooka/kotoha-ime/pull/61> (squash merge commit: `ec829d51c7508ccb06967000caac040bbcfab194`)
- 新規: `scripts/lib/assert.sh` (71 lines)
- refactor: `scripts/phase0-smoke.sh` (net -22 lines: +18 insertions / -40 deletions)
