# Kotoha Phase 1 (kana→kanji) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Phase 0 の `InputContext` が確定させたひらがな列を、Zenz (GPT-2 系) + llama.cpp で漢字混じり文候補に変換する `kotoha-core::kanji` module と CLI `kotoha-kanji` を実装し、shell pipe で `romaji → かな → 漢字` の full pipeline を shell 上で確認できる状態にする。

**Architecture:** 新規 module `kotoha-core::kanji/` に `KanjiBackend` trait + 初の具象 `ZenzBackend` (llama-cpp-2 経由) + `MockBackend` (test 専用) を実装する。`kotoha-cli` に新規 binary `kotoha-kanji` を追加し、`kotoha-romaji` との pipe 組合せで shell-composable な動作確認パイプラインを構築する。Spec `docs/superpowers/specs/2026-04-24-kotoha-phase-1-design.md` §1-16 を normative reference とする。

**Tech Stack:** Rust 2021 / MSRV 1.80 / kotoha-core (既存) / llama-cpp-2 (新規、P1-2 で導入) / thiserror (既存) / clap 4.5 (既存、P1-3 で `kotoha-kanji` binary を追加) / Zenz-v2.5-medium GGUF (manual placement)

---

## Spec 準拠

本 plan は `docs/superpowers/specs/2026-04-24-kotoha-phase-1-design.md` (863 行、PR #58 で merge 済み) を normative reference として作成する。plan と spec の記述に齟齬が生じた場合は spec を優先し、plan 側を訂正する。本 plan は spec §11 (Milestone 分割) の 5 milestone (P1-0〜P1-4) 構成を 1:1 で継承する。

## Milestone 一覧

| M | 名称 | scope 要約 | 工数 | tier | plan |
|---|---|---|---|---|---|
| P1-0 | 先行 refactor: assert.sh 抽出 | `scripts/lib/assert.sh` 新設 + Phase 0 smoke を共通 library 経由に移植 | 0.3 日 | Small | **詳細化済み (本 plan §P1-0)** |
| P1-1 | kanji skeleton + MockBackend | `kotoha-core::kanji` module 新設、trait/struct/error 型定義、`MockBackend` 実装、Layer 1+2 test 全件 | 1.5 日 | Medium | 着手直前に別 plan PR |
| P1-2 | ZenzBackend + Layer 3 smoke | llama-cpp-2 依存追加、実 model 経由 inference、feature gate 完成 | 2 日 | Medium | 着手直前に別 plan PR、AzooKey docs mandatory 参照 |
| P1-3 | kotoha-kanji CLI + phase1-smoke.sh | CLI binary、`process_line` 純粋関数、E2E smoke 5 件 | 1 日 | Medium | 着手直前に別 plan PR |
| P1-4 | ADR + Phase 1 closing docs | ADR 0009/0010/0011 + ROADMAP "完了" 更新 + README 追記 | 0.5 日 | Small | 着手直前に別 plan PR |

**合計: 約 5.3 日、5 PR** (各 milestone で implementation PR を 1 本、P1-1〜P1-4 は事前に別 plan PR も追加するため plan PR 含めて最大 9 PR)。

## Milestone 間の依存関係

```
P1-0 (assert.sh 抽出)
  ↓ (Phase 0 smoke が lib/assert.sh 経由で動く状態)
P1-1 (kanji skeleton + Mock)
  ↓ (公開 API + MockBackend 完備)
P1-2 (ZenzBackend + smoke)
  ↓ (real model inference 動作確認済み)
P1-3 (kotoha-kanji CLI + E2E smoke)
  ↓ (CLI + smoke script 完備)
P1-4 (ADR + closing docs) → Phase 1 完了
```

直列実行を前提とする。5 milestone が linear dependency chain を構成しているため、並行化のメリットは薄い。

## 共通規約

Phase 0 から継承する規約を本 Phase 1 でも適用する。

- Branch 命名: `feature/<ISSUE>-*` / `test/<ISSUE>-*` / `docs/<ISSUE>-*` / `bugfix/<ISSUE>-*` / `perf/<ISSUE>-*` / `refactor/<ISSUE>-*`
- Commit / PR / ISSUE 言語: 英語 (Kotoha exception)
- 文書言語: 日本語 (plan / spec / ADR / WBS)
- コード内コメント: 日本語可 (ドメイン説明が日本語で自然な箇所)、rustdoc 公開 API は英語推奨
- TDD: Red → Green → Refactor 原則 (writing-plans skill 水準)
- Commit 粒度: 小さく、頻繁に行う
- lefthook pre-commit / pre-push: 絶対 bypass しない (`--no-verify` 禁止)
- File 操作: Read / Edit / Write のみ、`cat` / `sed` / `awk` / `echo >` は使用しない
- Cargo: sequential 実行 (memory 7GB / swap 48% 想定、並列禁止)
- 各 milestone 完了後に `docs/wbs/2026-04-XX-{feature,docs,test,perf,bugfix,refactor}-<ISSUE>-*.md` を develop に直接 push する
- PR review は CLAUDE.md PR Review Matrix に従う (Small = 3 dim + secrets-check、Medium = 5 dim + owasp-security + secrets-check)

## 共通リスクと緩和策 (spec §10 引用)

Spec §10 の 8 項目 risk を本 plan でも normative に継承する。各 milestone の詳細 plan でも同 table を参照し続ける。

1. **llama-cpp-2 の C FFI build 失敗** — CPU-only で構築、README に apt deps (例: `libclang-dev`, `cmake`, `build-essential`) を明記する。P1-2 着手時に Linux / GNOME Wayland 環境で build 確認する。
2. **Zenz-v2.5 prompt format 未確定** — AzooKey Zenzai docs (spec §3.3) を P1-2 の最初の Task として mandatory に参照し、WBS に「prompt format 解析ログ」を記録する。
3. **cold start latency** — Phase 1 は初回 model load を許容範囲とし、Phase 3 で prewarm / daemon 化を検討する。
4. **feature flag compile matrix (default / zenz / zenz-smoke / mock-backend の 4 組合せ)** — `cargo check --all-features` を P1-1 以降の pre-push に追加し、matrix hell を CI 的に防止する。
5. **llama-cpp-2 unsafe FFI** — safe wrapper のみ使用し、新規 `unsafe` block の導入は ADR を必須とする。
6. **proptest / fuzzing 不在** — Phase 1 では seed 固定 greedy 推論のみを扱い、ランダム入力 test は Phase 2 の品質評価基盤に先送りする。
7. **Send + Sync 将来変更** — Phase 1 は single-thread CLI のみなので bound を追加しない。Phase 3 IBus engine 層で再評価する。
8. **model license redistribution** — Phase 1 は manual placement に限定し、auto-download (HuggingFace API) は Phase 2 以降に切り出す。

## PR #1 — P1-0: scripts/lib/assert.sh 抽出 + Phase 0 smoke refactor

**Goal:** Phase 0 smoke script の assert 関数を共通 library `scripts/lib/assert.sh` に抽出し、Phase 0 smoke を共通 library 経由で動かす形に refactor する。本 PR は Phase 1 の stepping stone として機能する infrastructure 改善 PR。コード変更なし (Rust code は不変)、shell script のみを触る。

### P1-0 完了条件

- [ ] GitHub ISSUE (P1-0) が作成され、merge 済み PR で close される
- [ ] `scripts/lib/assert.sh` が develop に存在し、`assert_equal` / `assert_contains` / `assert_summary` の 3 関数を提供する
- [ ] `scripts/phase0-smoke.sh` が `source lib/assert.sh` 経由で refactor され、**10/10 PASS regression なし**
- [ ] `cargo build --workspace` / `cargo test --workspace` / `cargo clippy --workspace --all-targets -- -D warnings` / `cargo fmt --all --check` 全て既存 baseline と同一
- [ ] lefthook pre-commit + pre-push 全 PASS
- [ ] WBS ログ `docs/wbs/2026-04-XX-refactor-<ISSUE>-assert-sh.md` が develop に push 済み

### ファイル構成 (P1-0)

新規作成:

- `scripts/lib/assert.sh`

変更:

- `scripts/phase0-smoke.sh` (既存 `assert_eq` 関数を削除、`source lib/assert.sh` に置換)

---

### Task P1-0-1: ISSUE 作成 + branch 作成

**Files:** (GitHub 操作と branch 作成のみ)

- [ ] **Step 1: 作業ディレクトリと develop の最新化**

Run:

```bash
cd /home/kohshiro/develops/student/kotoha-ime
git checkout develop
git pull
git status
git log --oneline -3
```

Expected: `develop` が `origin/develop` と同期、working tree clean、最新 commit は P1 spec merge の `8afa5f4 docs(spec): add Phase 1 (kana→kanji) design specification (#58)` 以降。

- [ ] **Step 2: P1-0 用 ISSUE を作成**

Run:

```bash
gh issue create \
  --title "P1-0 (refactor): extract scripts/lib/assert.sh + refactor phase0-smoke.sh" \
  --body "$(cat <<'EOF'
Phase 1 の stepping stone PR。Phase 0 smoke script に埋め込まれている assert 関数を共通 shell library `scripts/lib/assert.sh` に抽出し、将来の Phase 1 smoke / Phase N smoke で再利用可能にする。本 PR は Phase 1 の前提 infrastructure 整備であり、Rust コードは変更しない。

## Scope

- New: `scripts/lib/assert.sh` — provides `assert_equal` / `assert_contains` / `assert_summary`
- Refactor: `scripts/phase0-smoke.sh` — remove inline `assert_eq`, source `lib/assert.sh`, preserve all 10 assertions verbatim

## Out of scope

- Phase 1 の kanji module / CLI / smoke (別 milestone)
- 新規 script の追加
- Rust コード変更

## Acceptance

- scripts/lib/assert.sh provides `assert_equal` / `assert_contains` / `assert_summary`
- scripts/phase0-smoke.sh is refactored to source lib/assert.sh
- bash scripts/phase0-smoke.sh exits 0 with 10/10 PASS (regression-free)
- cargo build / test / clippy / fmt all match pre-refactor baseline
- lefthook pre-commit + pre-push all green

## Reference

- Phase 1 spec: docs/superpowers/specs/2026-04-24-kotoha-phase-1-design.md §9
- Phase 1 overall plan: docs/superpowers/plans/2026-04-24-kotoha-phase-1-implementation.md §P1-0
EOF
)"
```

Expected: ISSUE が作成され URL と番号が表示される (以降、本 Task 内では ISSUE 番号を `N` と呼ぶ)。

- [ ] **Step 3: branch を作成する**

Run (`N` は Step 2 の ISSUE 番号):

```bash
git checkout -b refactor/N-assert-sh develop
git branch --show-current
```

Expected: `refactor/N-assert-sh` に切り替わる。

---

### Task P1-0-2: scripts/lib/assert.sh を Write で作成

**Files:**

- Create: `scripts/lib/assert.sh`

**方針:** Phase 0 smoke script の既存 `assert_eq` 関数を抽出し、spec §9.1 で定義した 3 関数 (`assert_equal` / `assert_contains` / `assert_summary`) + counter 変数 (`ASSERT_PASS` / `ASSERT_FAIL`) を公開する。本 Task の assert.sh 実装は spec §9.1 の疑似 API を忠実に具象化したものである。

- [ ] **Step 1: scripts/lib/ ディレクトリを作成する**

Run:

```bash
mkdir -p scripts/lib
ls -la scripts/lib/
```

Expected: `scripts/lib/` ディレクトリが存在する (空)。

- [ ] **Step 2: scripts/lib/assert.sh を Write で新規作成する**

Write tool で `scripts/lib/assert.sh` を新規作成する。内容は verbatim で以下のとおり。

```bash
#!/usr/bin/env bash
# Common shell assertion library for Kotoha smoke scripts.
#
# Source this file from a smoke script and use:
#   assert_equal    <desc> <actual> <expected>           # exact string match
#   assert_contains <desc> <actual> <expected_substring> # substring match
#   assert_summary  <phase_name>                         # print summary, exit 1 on any FAIL
#
# Example:
#   #!/usr/bin/env bash
#   set -euo pipefail
#   SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
#   source "$SCRIPT_DIR/lib/assert.sh"
#   assert_equal "1. hello" "$(echo hello)" "hello"
#   assert_summary "my-smoke"
#
# Counter state is held in ASSERT_PASS / ASSERT_FAIL. Both are initialized
# to 0 on source and updated by assert_* functions. assert_summary inspects
# them and exits non-zero if ASSERT_FAIL > 0.

ASSERT_PASS=0
ASSERT_FAIL=0

# Exact string match. Prints PASS on success, FAIL with diff on failure.
# Args:
#   $1: description (free text, e.g., "1. konnnichiha -> こんにちは")
#   $2: actual value (string)
#   $3: expected value (string)
assert_equal() {
    local desc="$1"
    local actual="$2"
    local expected="$3"
    if [[ "$actual" == "$expected" ]]; then
        echo "PASS  $desc"
        ASSERT_PASS=$((ASSERT_PASS + 1))
    else
        echo "FAIL  $desc: expected '$expected', got '$actual'"
        ASSERT_FAIL=$((ASSERT_FAIL + 1))
    fi
}

# Substring containment match. Prints PASS on success, FAIL with detail on failure.
# Used for stochastic model outputs where exact match is too brittle.
# Args:
#   $1: description
#   $2: actual value (string)
#   $3: expected substring
assert_contains() {
    local desc="$1"
    local actual="$2"
    local expected_substring="$3"
    if [[ "$actual" == *"$expected_substring"* ]]; then
        echo "PASS  $desc: contains '$expected_substring'"
        ASSERT_PASS=$((ASSERT_PASS + 1))
    else
        echo "FAIL  $desc: '$actual' does not contain '$expected_substring'"
        ASSERT_FAIL=$((ASSERT_FAIL + 1))
    fi
}

# Print summary line and exit non-zero if any assertion failed.
# Args:
#   $1: phase name (free text, e.g., "phase0-smoke")
assert_summary() {
    local phase_name="$1"
    local total=$((ASSERT_PASS + ASSERT_FAIL))
    echo "=== $phase_name: $ASSERT_PASS/$total PASS, $ASSERT_FAIL/$total FAIL ==="
    if [[ $ASSERT_FAIL -gt 0 ]]; then
        exit 1
    fi
}
```

- [ ] **Step 3: assert.sh の permission を確認する**

Run:

```bash
ls -la scripts/lib/assert.sh
```

Expected: `-rw-r--r--` (644) 相当。本ファイルは `source` されるだけで直接実行されないため、executable bit は不要である。

- [ ] **Step 4: assert.sh を scratch で動作確認する**

Run (一時的な動作検証、本 Step の後 shell を閉じれば痕跡は残らない):

```bash
bash -c '
    source scripts/lib/assert.sh
    assert_equal "test eq pass" "a" "a"
    assert_equal "test eq fail" "a" "b"
    assert_contains "test cn pass" "hello world" "world"
    assert_contains "test cn fail" "hello world" "xyz"
    assert_summary "self-test" || echo "exit 1 as expected"
'
```

Expected output (順不同ではなく固定順で出力される):

```
PASS  test eq pass
FAIL  test eq fail: expected 'b', got 'a'
PASS  test cn pass: contains 'world'
FAIL  test cn fail: 'hello world' does not contain 'xyz'
=== self-test: 2/4 PASS, 2/4 FAIL ===
exit 1 as expected
```

---

### Task P1-0-3: scripts/phase0-smoke.sh を refactor する

**Files:**

- Modify: `scripts/phase0-smoke.sh`

**方針:** 既存 `assert_eq` 関数を削除し、`source lib/assert.sh` に置換する。既存 10 件の assertion は verbatim で残し、関数名を `assert_eq` → `assert_equal` に rename する (引数順は `desc / expected / actual` から `desc / actual / expected` に変わる点に注意)。末尾の手書き summary echo と exit 判定を `assert_summary` 呼び出しに統合する。

- [ ] **Step 1: 現状の phase0-smoke.sh を Read で確認する**

Read tool で `/home/kohshiro/develops/student/kotoha-ime/scripts/phase0-smoke.sh` 全体を読み、以下を確定する。

1. 既存の `assert_eq()` 関数定義の開始/終了行番号
2. 10 件の `assert_eq "desc" "expected" "actual"` 呼び出しの開始/終了行番号
3. 末尾の summary echo (`=== phase0-smoke: ...`) と `exit` 判定の行番号

Expected: 既存 `assert_eq` は 35-48 行付近に存在、10 件の呼び出しは 55-90 行付近、summary/exit は 92-100 行付近。本 Task は上記 3 箇所をまとめて書き換える。

- [ ] **Step 2: phase0-smoke.sh の assert_eq 定義を削除し、source directive を追加する**

Edit tool で、既存 `assert_eq()` 関数定義ブロック (`assert_eq() { ... }` の全体) と直前の `PASS_COUNT`/`FAIL_COUNT`/`TOTAL` 変数宣言を削除し、代わりに `set -euo pipefail` の直後に以下 2 行を挿入する。

```bash
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/lib/assert.sh"
```

注意: 既存の `cd "$(dirname "$0")/.."` (repository root への cd) はそのまま残す。`SCRIPT_DIR` は `cd` 前に evaluate するため、`$(dirname "${BASH_SOURCE[0]}")` が scripts ディレクトリを指す。

- [ ] **Step 3: 10 件の assert_eq 呼び出しを assert_equal に rename する**

Edit tool の `replace_all` は `assert_eq` が summary echo 文中の "PASS" 等と衝突しないよう、行頭の `assert_eq "` だけを対象に個別置換する。引数順の変更 (expected と actual の入れ替え) にも注意する:

- **変更前**: `assert_eq "desc" "$expected_value" "$actual_value"`
- **変更後**: `assert_equal "desc" "$actual_value" "$expected_value"`

各 10 件について、Edit tool で 1 件ずつ置換する。個別置換の際は、`desc` 文字列で unique になるため `old_string` を行頭から 1 行分含めれば uniqueness は保証される。

- [ ] **Step 4: 末尾の summary 手書き echo と exit 判定を assert_summary 呼び出しに置換する**

Edit tool で以下のブロック (phase0-smoke.sh 末尾) を削除する:

```bash
# --- Summary ----------------------------------------------------------

echo "=== phase0-smoke: ${PASS_COUNT}/${TOTAL} PASS, ${FAIL_COUNT}/${TOTAL} FAIL ==="

if [ "$FAIL_COUNT" -gt 0 ]; then
    exit 1
fi

exit 0
```

代わりに以下 1 行を追加する:

```bash
assert_summary "phase0-smoke"
```

`assert_summary` 内部で FAIL > 0 のとき `exit 1` に到達するため、末尾の `exit 0` は不要である (`set -e` + fall-through で exit 0 扱いとなる)。

- [ ] **Step 5: refactor 後の差分を diff で確認する**

Run:

```bash
git diff scripts/phase0-smoke.sh
```

Expected:

- `-` 行: 既存 `assert_eq()` 関数定義 (約 14 行)、`PASS_COUNT`/`FAIL_COUNT`/`TOTAL` 変数宣言 (3 行)、末尾 summary/exit ブロック (約 7 行)、10 件の `assert_eq` 呼び出し
- `+` 行: `SCRIPT_DIR=...` + `source ...` (2 行)、10 件の `assert_equal` 呼び出し、末尾 `assert_summary "phase0-smoke"` (1 行)
- ネット diff: 合計 -15 〜 -20 行 (共通化による簡素化)

---

### Task P1-0-4: 手動実行 smoke で regression 検証を行う

**方針:** Phase 0 の 10 件 smoke が既存と同一の PASS/FAIL 結果を返すか確認する。本 Task が P1-0 の最重要検証点である。

- [ ] **Step 1: refactor 後の Phase 0 smoke を実行する**

Run:

```bash
bash scripts/phase0-smoke.sh
```

Expected:

- 10 件の `PASS  N. ...` 行が順に出力される (N は 1-10)
- 末尾に `=== phase0-smoke: 10/10 PASS, 0/10 FAIL ===` が出力される
- exit code 0

もし 1 件でも FAIL が出る場合、Task P1-0-3 の refactor に誤りがあるため、Edit tool で修正して本 Step を再実行する。

- [ ] **Step 2: FAIL 時の動作確認 (scratch) を行う**

方針: 故意に expected を破損して 1 件 FAIL を発生させ、exit code 1 が出ることを確認する。後続 Step で必ず restore する。

Run:

```bash
git diff scripts/phase0-smoke.sh
```

Expected: この時点で本 branch の diff は「refactor 後の状態」。

Edit tool で 1 件 assertion を故意に壊す (例: 1 件目 `"こんにちは"` を `"こんばんは"` に変更する)。

Run:

```bash
bash scripts/phase0-smoke.sh; echo "exit code: $?"
```

Expected:

- 1 件 `FAIL` 行が出力される
- 末尾 `=== phase0-smoke: 9/10 PASS, 1/10 FAIL ===`
- `exit code: 1`

- [ ] **Step 3: Restore する**

Edit tool で Step 2 で故意に壊した行を元に戻す。

Run:

```bash
bash scripts/phase0-smoke.sh
```

Expected: 再度 10/10 PASS、exit 0。

---

### Task P1-0-5: workspace 全体検証 + lefthook pre-push を通す

**方針:** Rust コードは変更していないが、baseline が壊れていないか念のため確認する。Cargo sequential (memory 7GB / swap 48% 考慮)。

- [ ] **Step 1: cargo build --workspace**

Run:

```bash
cargo build --workspace
```

Expected: `Finished ... in 0.0Xs` (cached、即完了)、exit 0。

- [ ] **Step 2: cargo test --workspace**

Run:

```bash
cargo test --workspace
```

Expected: Phase 0 既存 118 件 + α が全 PASS、`test result: ok. N passed` で終わる。

- [ ] **Step 3: cargo clippy --workspace --all-targets -- -D warnings**

Run:

```bash
cargo clippy --workspace --all-targets -- -D warnings
```

Expected: `Finished ... in 0.0Xs`、warnings 0。

- [ ] **Step 4: cargo fmt --all --check**

Run:

```bash
cargo fmt --all --check
```

Expected: no output、exit 0。

---

### Task P1-0-6: commit + push + PR 作成

- [ ] **Step 1: git status / diff の最終確認を行う**

Run:

```bash
git status
git diff --stat
```

Expected:

- `scripts/lib/assert.sh` が新規ファイル (`??` または `A`)、約 60 行
- `scripts/phase0-smoke.sh` が modified、net -15 〜 -20 行
- 他のファイルに変更なし

- [ ] **Step 2: commit する**

Run (`N` は ISSUE 番号):

```bash
git add scripts/lib/assert.sh scripts/phase0-smoke.sh
git commit -m "refactor(scripts): extract assert.sh and refactor phase0-smoke (#N)

Extract the inline assert function from scripts/phase0-smoke.sh into a
shared shell library scripts/lib/assert.sh. The library provides three
functions:

- assert_equal(desc, actual, expected): exact string match
- assert_contains(desc, actual, expected_substring): substring match
- assert_summary(phase_name): print summary and exit 1 on any FAIL

Refactor scripts/phase0-smoke.sh to source lib/assert.sh and use
assert_equal + assert_summary. The 10 existing assertions are preserved
verbatim; regression is prevented by running the full smoke suite post
refactor (10/10 PASS confirmed).

This is Phase 1 milestone P1-0, a stepping-stone refactor PR: the new
library will be consumed by the upcoming scripts/phase1-smoke.sh (P1-3)
and any future Phase N smoke scripts. No Rust code changes.

Reference:
- Phase 1 spec: docs/superpowers/specs/2026-04-24-kotoha-phase-1-design.md §9
- Phase 1 plan: docs/superpowers/plans/2026-04-24-kotoha-phase-1-implementation.md §P1-0

Closes #N
"
```

- [ ] **Step 3: push する**

Run:

```bash
git push -u origin refactor/N-assert-sh
```

Expected: lefthook pre-push (build + clippy + manifest-check + test) が全 green で push 成功。

- [ ] **Step 4: PR を作成する**

Run:

```bash
gh pr create --base develop \
  --title "refactor(scripts): extract assert.sh and refactor phase0-smoke (#N)" \
  --body "$(cat <<'EOF'
## Summary

Extract the inline assert function from `scripts/phase0-smoke.sh` into a shared shell library `scripts/lib/assert.sh` for reuse by Phase 1+ smoke scripts.

## Changes

- **New**: `scripts/lib/assert.sh` with 3 public functions:
  - `assert_equal(desc, actual, expected)` — exact string match
  - `assert_contains(desc, actual, expected_substring)` — substring match (for stochastic outputs)
  - `assert_summary(phase_name)` — print summary and exit 1 on any FAIL
- **Refactored**: `scripts/phase0-smoke.sh` now sources `lib/assert.sh` instead of defining assert inline. All 10 assertions preserved verbatim (argument order updated from `desc/expected/actual` to `desc/actual/expected` to match the shared library's convention).

## Verification

- `bash scripts/phase0-smoke.sh` → 10/10 PASS (regression-free confirmed)
- Intentional-failure scratch test: `9/10 PASS, 1/10 FAIL, exit 1` (FAIL path works)
- `cargo build / test / clippy -D warnings / fmt --all --check` all green (unchanged baseline)
- lefthook pre-commit + pre-push all green

## Context

Phase 1 milestone **P1-0**: stepping-stone refactor PR before implementing Phase 1 kanji module. The new `assert.sh` will be consumed by the upcoming `scripts/phase1-smoke.sh` (P1-3 milestone) and by any future Phase N smoke scripts.

## Test plan

- [x] `bash scripts/phase0-smoke.sh` exits 0 with 10/10 PASS
- [x] cargo build/test/clippy/fmt all green
- [x] lefthook pre-commit + pre-push green

Closes #N
EOF
)"
```

Step 4 が完了したら、PR 番号を `P#` として記録する (以後の Task で使用)。

---

### Task P1-0-7: review + findings 解消 + merge

- [ ] **Step 1: Small tier review を行う**

CLAUDE.md PR Review Matrix に従い、Small tier (`≤5 files AND ≤100 lines`、本 PR は 2 files / net 約 +40 行で該当) のレビューを実施する:

- `agent-teams:team-review` (dimensions: security + architecture + testing) — 並列起動する。experimental flag 未設定の場合は inline fallback で代替する
- `secrets-check`

レビュー focus:

- **security**: shell injection risk — `assert.sh` は `[[ ]]` 内で bash quote 展開を使用。argument が user input でなければ safe である点を確認する
- **architecture**: directory 構成 (`scripts/lib/`) が既存 `scripts/tests/` と整合するか、命名 `assert.sh` が拡張性を確保しているかを確認する
- **testing**: 10/10 regression PASS と scratch FAIL case (9/10 PASS 1/10 FAIL exit 1) の両方を evidence として受理する

- [ ] **Step 2: findings を resolve する**

Critical / High の findings: 新規 commit で fix する (never amend)。Medium: 5 分以内で fix するか follow-up ISSUE に切り出す。Low / Info: acknowledge し、次 Step に進む。

- [ ] **Step 3: merge する**

Run (`P#` は Task P1-0-6 Step 4 の PR 番号):

```bash
gh pr merge P# --squash --delete-branch
```

Expected: `"state":"MERGED"`。merge commit SHA を `<SHA>` として記録する。

---

### Task P1-0-8: WBS ログ作成 + develop 直接 push

- [ ] **Step 1: checkout develop + pull する**

Run:

```bash
git checkout develop
git pull
git log --oneline -3
```

Expected: 最新 commit が P1-0 の squash merge commit (`<SHA>`) である。

- [ ] **Step 2: WBS ログを Write で作成する**

Write tool で `docs/wbs/2026-04-XX-refactor-N-assert-sh.md` を作成する (`XX` = 実施日、`N` = ISSUE 番号)。

内容 (日本語、約 40 行):

```markdown
---
milestone: P1-0
branch: refactor/N-assert-sh
pr: "#P#"
issue: "#N"
status: done
started: 2026-04-XX
finished: 2026-04-XX
---

# P1-0: scripts/lib/assert.sh 抽出 + Phase 0 smoke refactor

## 実施内容

- `scripts/lib/assert.sh` を新規作成。`assert_equal` / `assert_contains` / `assert_summary` の 3 関数を提供する。
- `scripts/phase0-smoke.sh` を `source lib/assert.sh` 経由に refactor。10 件の assertion は verbatim 維持 (引数順のみ `desc/expected/actual` → `desc/actual/expected` に rename)。
- FAIL path の動作確認 (scratch test) を実施して restore 済み。

## つまずき

(実施時の気付きを記録する。初回実行時は "特になし" 等でも可。)

## Regression 検証

- `bash scripts/phase0-smoke.sh`: 10/10 PASS、exit 0 (refactor 前と同一結果)
- FAIL case scratch test: 9/10 PASS, 1/10 FAIL, exit 1 (期待通り動作)
- `cargo build / test / clippy -D warnings / fmt --all --check`: 全 green (baseline 不変)

## P1-1 への申し送り

- `assert_contains` は P1-3 の `scripts/phase1-smoke.sh` で使用される想定。Phase 2 以降の smoke script も本 library を source する形で統一する。
- `scripts/lib/` directory は今後 `logging.sh` / `model-path.sh` など Phase 1+ 用の helper を置く場として活用可能。

## 成果物リンク

- ISSUE: #N
- PR: #P# (merge commit: `<SHA>`)
- 新規: scripts/lib/assert.sh
- refactor: scripts/phase0-smoke.sh
```

`N` / `P#` / `<SHA>` / `XX` は実値で埋める。

- [ ] **Step 3: commit + push する**

Run:

```bash
git add docs/wbs/2026-04-XX-refactor-N-assert-sh.md
git commit -m "docs(wbs): P1-0 assert.sh refactor — implementation log (#N, PR #P#)"
git push origin develop
```

Expected: lefthook pre-commit (doc-naming script) + pre-push が全 PASS すること。特に doc-naming script が `docs/wbs/2026-04-XX-refactor-N-assert-sh.md` の naming convention を通すことを確認する。

---

### P1-0 完了条件チェックリスト

以下すべてが ✅ になったら P1-1 (別 plan PR で詳細化) に進む:

- [ ] GitHub ISSUE #N が close されている
- [ ] PR #P# が develop に merge され、refactor branch が削除されている
- [ ] `scripts/lib/assert.sh` が develop に存在する
- [ ] `scripts/phase0-smoke.sh` が `source lib/assert.sh` 経由で refactor されている
- [ ] `bash scripts/phase0-smoke.sh` が exit 0 + 10/10 PASS を返す
- [ ] `cargo build --workspace` / `cargo test --workspace` / `cargo clippy --workspace --all-targets -- -D warnings` / `cargo fmt --all --check` が baseline と同一
- [ ] WBS ログ `docs/wbs/2026-04-XX-refactor-N-assert-sh.md` が develop に push 済み

---

## PR #2 以降 — P1-1 〜 P1-4 (outline only)

以下各 milestone は、完了条件 / ファイル構成 / 想定 PR スコープの outline のみを記述する。TDD 詳細は各 milestone 着手直前に別 plan PR として writing-plans skill を再 invoke し、`docs/superpowers/plans/2026-04-XX-kotoha-phase-1-p1-M.md` 形式で起票する (Phase 0 M3/M4/M6 と同 pattern)。

### PR #2 — P1-1: kanji skeleton + MockBackend

**Goal:** `kotoha-core::kanji` module skeleton を新設し、公開 API 型 (`KanjiBackend` trait / `Candidate` / `ConvertOptions` / `BackendConfig` / `KanjiError`) と `MockBackend` 実装を配備する。`ZenzBackend` は空の skeleton のみ (実装は P1-2)。Layer 1 (unit) + Layer 2 (mock integration) テスト全件をこの PR 内で緑にする。

**完了条件 (outline):**

- `kotoha_core::kanji::{Candidate, ConvertOptions, KanjiBackend, MockBackend, BackendConfig, load_backend, KanjiError}` が `lib.rs` から re-export されている
- `cargo test --workspace` (default features) 全 PASS、Layer 1 約 25 件 + Layer 2 5 件が新規追加されている
- `cargo test --workspace --features mock-backend` 全 PASS
- `cargo check --all-features` PASS (feature flag matrix hell 回避)
- feature flag `default` / `mock-backend` / `zenz` / `zenz-smoke` の 4 構成が各 `cargo check` で compile 通る
- WBS ログが develop に push 済み

**ファイル構成 (outline):**

- Create: `crates/kotoha-core/src/kanji/mod.rs` (re-export)
- Create: `crates/kotoha-core/src/kanji/candidate.rs` (`Candidate` + `ConvertOptions`)
- Create: `crates/kotoha-core/src/kanji/error.rs` (`KanjiError` enum)
- Create: `crates/kotoha-core/src/kanji/backend.rs` (`KanjiBackend` trait + `BackendConfig` + `load_backend`)
- Create: `crates/kotoha-core/src/kanji/mock.rs` (`MockBackend`、`#[cfg(feature = "mock-backend")]`)
- Create: `crates/kotoha-core/src/kanji/zenz.rs` (skeleton のみ、実装は `todo!()`)
- Modify: `crates/kotoha-core/Cargo.toml` (feature flag 4 種類追加、llama-cpp-2 は optional 依存 placeholder)
- Modify: `crates/kotoha-core/src/lib.rs` (`pub mod kanji;` + re-export)
- Create: `crates/kotoha-core/tests/kanji_mock.rs` (Layer 2 integration、5 件)

**scope / tier:** Medium (production + tests 合計 約 500 LOC 想定、10 files 以内)

**依存:** P1-0 merged

**詳細 plan:** P1-1 着手直前に `docs/superpowers/plans/2026-04-XX-kotoha-phase-1-p1-1.md` として起票する。Open Question Q4 (Mock fixture hard-code vs TSV 外出し) は P1-1 plan 内で判断する (spec §13 Q4 に従い、5 件規模では hard-code 優先)。

---

### PR #3 — P1-2: ZenzBackend 実装 + Layer 3 smoke

**Goal:** P1-1 の `ZenzBackend` skeleton に llama-cpp-2 経由で実装を追加する。Layer 3 (`zenz-smoke` feature) の integration test 5 件を追加する。AzooKey Zenzai docs を実装着手前に必ず参照し、prompt format を reverse-engineer した結果を WBS に「prompt format 解析ログ」として記録する。

**完了条件 (outline):**

- `crates/kotoha-core/Cargo.toml` に `llama-cpp-2 = "X.Y"` 依存が追加されている (`optional = true`、feature `zenz` で enable)
- `ZenzBackend::load(&Path)` / `convert(&str, &ConvertOptions)` が実装済み
- `cargo test --workspace --features zenz-smoke` で Layer 3 5 件 PASS (`KOTOHA_ZENZ_MODEL_PATH` 環境変数 or spec §3.2 記載の default path に real model 配置済み想定)
- `cargo test --workspace` (default features) で Layer 3 は skip される (feature gate で compile skip)
- `cargo check --all-features` PASS (matrix 検証)
- WBS に prompt format 解析ログ (AzooKey docs 通読結果 + 実装で判明した token format) が記録されている
- WBS に cold start latency 実測値 (1 回目 load、2 回目 warm cache) が記録されている

**ファイル構成 (outline):**

- Modify: `crates/kotoha-core/Cargo.toml` (llama-cpp-2 dep を実 version に pin、`dep:llama-cpp-2` を features に配線)
- Modify: `crates/kotoha-core/src/kanji/zenz.rs` (P1-1 の skeleton を full 実装に展開)
- Create: `crates/kotoha-core/tests/kanji_zenz_smoke.rs` (Layer 3、5 件)
- Create: `crates/kotoha-core/tests/fixtures/kanji_smoke.tsv` (Layer 3 fixture 5 行、`input_hiragana<TAB>expected_substring` 形式、spec §8.3 引用)

**scope / tier:** Medium (llama-cpp-2 FFI bridge + prompt 整形 + candidate decode 合計 約 600 LOC 想定、AzooKey docs 解析 / prompt format 実装 / llama.cpp API 習熟の比重が大)

**依存:** P1-1 merged

**詳細 plan:** P1-2 着手直前に `docs/superpowers/plans/2026-04-XX-kotoha-phase-1-p1-2.md` として起票する。**AzooKey Zenzai docs (<https://github.com/azooKey/AzooKeyKanaKanjiConverter/blob/main/Docs/zenzai.md>) の参照を plan の最初の Task に組み込み、解析ログを WBS に残すこと。** Open Question Q1 (llama-cpp-2 exact version) と Q2 (Zenz prompt exact form) と Q5 (`--seed 0` deterministic 挙動) も P1-2 plan 内で解消する。

---

### PR #4 — P1-3: kotoha-kanji CLI + phase1-smoke.sh

**Goal:** CLI binary `kotoha-kanji` を `kotoha-cli` crate に追加し、`scripts/phase1-smoke.sh` で spec §8.4 相当の E2E 検証 5 件を実現する。`process_line` 純粋関数を `kotoha-cli/src/kanji_process.rs` に抽出して unit test 可能にする。

**完了条件 (outline):**

- `kotoha-kanji` binary が `cargo build -p kotoha-cli` で生成される
- `kotoha-kanji --help` / `--version` が動作する
- CLI options (`--model` required / `--top-k` / `--show-scores` / `--show-model-id` / `--temperature` / `--seed`) が spec §7.1 の table と一致する
- `process_line` 純粋関数の unit test 5 件以上が PASS
- `scripts/phase1-smoke.sh` が model 配置済み環境で 5/5 PASS、未配置環境で SKIP かつ exit 0 を返す
- `scripts/phase1-smoke.sh` が `source lib/assert.sh` を使用 (P1-0 の成果物を継承)
- WBS ログが develop に push 済み

**ファイル構成 (outline):**

- Create: `crates/kotoha-cli/src/bin/kanji.rs` (CLI main、clap derive)
- Create: `crates/kotoha-cli/src/kanji_process.rs` (`process_line` 純粋関数 + in-file unit test)
- Modify: `crates/kotoha-cli/src/lib.rs` (`pub mod kanji_process;`) — lib target を新設する必要がある場合
- Modify: `crates/kotoha-cli/Cargo.toml` (binary 定義追加、`kotoha-core` feature `zenz` enable)
- Create: `scripts/phase1-smoke.sh` (5 件、`source lib/assert.sh`、`assert_contains` 使用)

**scope / tier:** Medium (CLI + pure function + unit/integration test 合計 約 400 LOC 想定)

**依存:** P1-2 merged (`ZenzBackend` 動作必須)、P1-0 merged (`assert.sh` 必須)

**詳細 plan:** P1-3 着手直前に `docs/superpowers/plans/2026-04-XX-kotoha-phase-1-p1-3.md` として起票する。Open Question Q3 (`process_line` を `kotoha-core::kanji::cli_support` で公開するか `kotoha-cli` lib target で公開するか) は P1-3 plan 内で判断する。

---

### PR #5 — P1-4: ADR + Phase 1 closing docs

**Goal:** Phase 1 で確定した設計判断を ADR 0009 / 0010 / 0011 として記録し、ROADMAP を "完了" に更新し、README に Phase 1 内容 + `kotoha-kanji` 使用例を追記する。

**完了条件 (outline):**

- `docs/adr/0009-zenz-model-version-policy.md` が作成されている (Zenz-v2.5-medium default、version upgrade 時の対応手順、spec §8.6 と連動)
- `docs/adr/0010-kanji-backend-trait-design.md` が作成されている (`KanjiBackend` trait + `BackendConfig` enum + `load_backend` factory の採用理由、llama_cpp-rs / mistral.rs / Candle との比較 — spec §3.1 と連動)
- `docs/adr/0011-feature-flag-design-for-zenz.md` が作成されている (default / zenz / zenz-smoke / mock-backend の 4 flags 分離理由、lefthook pre-push との整合 — spec §4.3 と連動)
- 各 ADR に Status "承認 (2026-04-XX)" / 検討した選択肢 / 決定 / 影響 の sections が記述されている
- `docs/ROADMAP.md`: Phase 1 行が "完了" に更新されている
- `README.md`: Phase 1 実装内容追記 + `kotoha-kanji` 使用例 5 件以上 (spec §6 Data flow に沿った pipe 例を含む)
- WBS ログが develop に push 済み
- Phase 1 overall plan (本 plan) と spec が normative reference として完全に整合している

**ファイル構成 (outline):**

- Create: `docs/adr/0009-zenz-model-version-policy.md`
- Create: `docs/adr/0010-kanji-backend-trait-design.md`
- Create: `docs/adr/0011-feature-flag-design-for-zenz.md`
- Modify: `docs/ROADMAP.md` (Phase 1 行の状態を "未着手" → "完了")
- Modify: `README.md` (Phase 1 usage examples section を追加、Zenz GGUF 入手手順を記載)

**scope / tier:** Small (ADR 3 本 × 約 60 行 + README 約 100 行 + ROADMAP 数行、合計 約 400 LOC)

**依存:** P1-3 merged

**詳細 plan:** P1-4 着手直前に `docs/superpowers/plans/2026-04-XX-kotoha-phase-1-p1-4.md` として起票する。Phase 0 M7 (`docs/superpowers/plans/2026-04-23-kotoha-phase-0-m4.md` ベースの PR #55 工程) が参考実装になる。

---

## Phase 1 完了の総合 acceptance

spec §14 の 15 項目 checklist に準拠する (本 plan の各 milestone 完了条件の union がこれに相当する)。

- [ ] `crates/kotoha-core/src/kanji/` module が公開されている
- [ ] `KanjiBackend` trait、`Candidate` / `ConvertOptions` / `BackendConfig` / `KanjiError` が `kotoha_core::kanji::*` から re-export されている
- [ ] `MockBackend` が `mock-backend` feature 下で公開されている
- [ ] `ZenzBackend` が `zenz` feature 下で公開されており、`load_backend(&BackendConfig::Zenz { .. })` で構築可能である
- [ ] `load_backend` factory が feature 未有効時に `KanjiError::FeatureDisabled` を返す
- [ ] Layer 1 unit test (default features、約 25 件) が pass する
- [ ] Layer 2 integration test (`--features mock-backend`、5 件) が pass する
- [ ] Layer 3 Zenz smoke (`--features zenz-smoke`、5 件) が `KOTOHA_ZENZ_MODEL_PATH` 指定で pass する
- [ ] Layer 4 E2E smoke (`scripts/phase1-smoke.sh`、5 件) が pass する
- [ ] CLI `kotoha-kanji` が `--model <path>` required、option `--top-k / --show-scores / --show-model-id / --temperature / --seed` を受け付ける
- [ ] CLI の exit code 仕様 (0 / 1 / 2) が Phase 0 `kotoha-romaji` と整合している
- [ ] `scripts/lib/assert.sh` が `assert_equal` / `assert_contains` / `assert_summary` を提供し、Phase 0 smoke が refactor 後も pass する
- [ ] ADR 0009 / 0010 / 0011 が作成され、本 plan および spec と相互参照している
- [ ] `README.md` に Zenz-v2.5-medium の GGUF 入手コマンドと配置先、`kotoha-kanji --model` の使い方が記載されている
- [ ] `cargo fmt --all --check` / `cargo clippy --workspace --all-targets --all-features -- -D warnings` / `cargo test --workspace` が lefthook pre-push で warning なく pass する

## Spec Coverage 確認

Spec §1-16 の主要要件と本 plan milestone の対応を以下に示す。

| Spec 要件 | Plan のカバー位置 |
|---|---|
| §1 概要 | 本 plan Goal / Milestone 一覧 |
| §2.1 In-scope 8 項目 | P1-1 (#1-#3) / P1-2 (#1 実装) / P1-3 (#4, #6) / P1-0 (#8) / 全 milestone (#5, #7) |
| §2.2 Out-of-scope | 本 plan scope 外 (Phase 2+ 明示) |
| §2.3 境界 (単文 1 行 1 変換) | P1-3 CLI 仕様として継承 |
| §3.1 llama-cpp-2 | P1-2 依存追加 + ADR 0010 |
| §3.2 Zenz model | P1-2 default 選定 + P1-4 ADR 0009 |
| §3.3 AzooKey docs mandatory | P1-2 最初の Task に組み込み、WBS ログに解析記録 |
| §3.4 thiserror / tempfile | P1-1 (thiserror) / P1-2 (tempfile dev-dep) |
| §4.1 crate/module layout | P1-1 skeleton + P1-2 実装 + P1-3 CLI で段階的構築 |
| §4.2 依存方向 (single direction) | P1-1 で確立 (kotoha-cli → kotoha-core) |
| §4.3 Feature flag | P1-1 で 4 flags 定義 + P1-4 ADR 0011 |
| §5.1 Candidate | P1-1 |
| §5.2 ConvertOptions | P1-1 |
| §5.3 KanjiBackend trait | P1-1 |
| §5.4 BackendConfig + load_backend | P1-1 |
| §5.5 KanjiError | P1-1 |
| §5.6 Input 制約 | P1-1 (validate_input) + P1-3 (CLI 側検査) |
| §5.7 Output 順序 + dedupe | P1-1 (score_sort_dedupe helper) |
| §6 Data flow | P1-3 で CLI 組上げ |
| §7.1-7.5 CLI contract | P1-3 |
| §8.1 Layer 1 Unit | P1-1 |
| §8.2 Layer 2 Integration (mock-backend) | P1-1 |
| §8.3 Layer 3 Zenz smoke (zenz-smoke) | P1-2 |
| §8.4 Layer 4 E2E smoke (shell) | P1-3 |
| §8.5 テスト件数・所要時間 table | P1-1 〜 P1-3 で段階的に達成 |
| §8.6 model 更新時 fixture regenerate 手順 | P1-4 ADR 0009 に policy として記録 |
| §9.1 assert.sh API 関数 | P1-0 で実装 |
| §9.2 Phase 0 smoke refactor | P1-0 で実施 |
| §10 Risk 8 項目 | 本 plan §共通リスクと緩和策 + 各 milestone 詳細 plan |
| §11 Milestone 分割 | 本 plan Milestone 一覧 (1:1 継承) |
| §12 ADR 候補 | P1-4 で 0009/0010/0011 作成 |
| §13 Open Questions Q1-Q5 | Q1/Q2/Q5 → P1-2、Q3 → P1-3、Q4 → P1-1 |
| §14 Acceptance 15 項目 | 本 plan §Phase 1 完了の総合 acceptance に 1:1 引用 |
| §15 参照 | 各 milestone plan の preamble で随時引用 |
| §16 Phase 2 への橋渡し | 本 plan §Phase 2 への橋渡し |

## Phase 2 への橋渡し

Phase 1 完了時点で Phase 2 の前提として利用可能になる asset:

- `kotoha-core::kanji::{KanjiBackend, ZenzBackend, Candidate, ConvertOptions}` 公開 API (安定)
- feature flag 構成の確立 (将来 backend 追加は同じ pattern で拡張可能、ADR 0011 準拠)
- `scripts/lib/assert.sh` 共通 shell library (Phase 2+ smoke で継続利用)
- Zenz model の manual placement 手順 (README 記載)
- ADR 0009 の model version policy (Phase 2 で model 更新する際の参照点)

Phase 2 で `kotoha-core` に追加予定 (本 plan の scope 外):

- `dict/` モジュール (システム辞書 / ユーザ辞書)
- `learning/` モジュール (候補選択履歴の学習キャッシュ)
- `kanji/hf_download.rs` (model auto-download、当初 Phase 1 予定 → Phase 2 に shift 済み)
- 品質評価インフラ (BLEU / exact-match metric、AJIMEE-Bench 採用検討)

Phase 2 以降も本 plan の `KanjiBackend` trait / `BackendConfig` factory pattern を踏襲し、新 backend 追加時に既存 `ZenzBackend` / `MockBackend` と同じ公開形式で module を拡張する。

---

## Self-Review 済み事項

本 plan 書き起こし後のセルフレビューで確認した項目:

1. **Spec coverage**: spec §1-16 の全章節を本 plan のいずれかの milestone に割り当て済み (上表参照)。gap 0。
2. **Placeholder scan**: grep 相当で "TBD" / "TODO" / "FIXME" / "implement later" / "add ... handling" を走査。残プレースホルダは `2026-04-XX` (実施日)、`N` (ISSUE 番号)、`P#` (PR 番号)、`<SHA>` (merge commit SHA)、`X.Y` (llama-cpp-2 version、P1-2 plan 起票時に確定) のみで、いずれも着手時に実値で確定する運用上の変数である。
3. **型・API 一貫性**: Rust identifiers (`KanjiBackend` / `Candidate` / `ConvertOptions` / `BackendConfig` / `load_backend` / `KanjiError` / `ZenzBackend` / `MockBackend`) が spec と本 plan のすべての言及箇所で一致している。
4. **Cross-reference validity**: spec 参照 (`docs/superpowers/specs/2026-04-24-kotoha-phase-1-design.md`) が存在する。ADR 0009/0010/0011 は P1-4 で新規作成予定であり、本 plan は「P1-4 で作成」として参照している。Phase 0 の ADR 0001-0008 は既存 (docs/adr/ 配下に実在を確認済み)。Phase 0 plan (`docs/superpowers/plans/2026-04-22-kotoha-phase-0-implementation.md`) および M4 plan (`docs/superpowers/plans/2026-04-23-kotoha-phase-0-m4.md`) は既存。File path はいずれも absolute 位置で存在を確認済み。
5. **Branch Scope Policy**: 各 milestone が 10 files / 300 lines / 2 日の guideline 内に収まる見積もり。P1-2 のみ LOC が増えやすいため (llama-cpp-2 FFI + prompt format 実装)、詳細 plan 起票時に分割要否を再評価する。
