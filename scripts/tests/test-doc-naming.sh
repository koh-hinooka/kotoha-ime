#!/usr/bin/env bash
# test-doc-naming.sh
# scripts/pre-commit-doc-naming.sh の CLAUDE.md 進捗情報検出ロジックの自己テスト
#
# 目的:
#   mawk (Ubuntu/Debian 標準) と gawk の双方で、awk パイプラインが
#   違反検出 (exit 0) / クリーン (exit 1) を一貫して返すことを保証する。
#
# テスト方針:
#   - 実際の git diff には依存せず、事前に用意した合成 diff を awk に直接流す。
#   - 本体 (pre-commit-doc-naming.sh) に埋め込まれている awk スクリプトと
#     同一のロジックを、テスト用にコピーして実行する。
#   - 期待値と実際の exit code を比較し、不一致があれば fail を 1 件加算する。
#
# 終了コード:
#   0 = 全ケース PASS
#   1 = 1 件以上 FAIL

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TARGET_SCRIPT="${SCRIPT_DIR}/../pre-commit-doc-naming.sh"

if [[ ! -f "$TARGET_SCRIPT" ]]; then
  echo "エラー: 対象スクリプトが見つかりません: $TARGET_SCRIPT" >&2
  exit 1
fi

# 本体スクリプトと同じ awk プログラム (pre-commit-doc-naming.sh と同期必須)。
# 本体が更新されたらこちらも更新する。
# shellcheck disable=SC2016  # awk プログラムリテラルのためシェル展開は不要
AWK_PROGRAM='
  /^\+/ && !/^\+\+\+/ && !/→/ && !/^\+[[:space:]]*-?[[:space:]]*\[[^][]+\]\(#[^()]+\)[[:space:]]*$/ {
    gsub(/`[^`]*`/, "")
    line = tolower($0)
    if (line ~ /(todo|wbs|進捗|タスク一覧|完了率|[0-9]+%|■|□|☑|☐|\[x\]|\[ \])/) found = 1
  }
  END { exit !found }
'

PASS=0
FAIL=0

# 合成 diff を awk に流し、期待 exit code と比較する。
#   expected_ec: 0 = 違反検出, 1 = クリーン
run_case() {
  local name=$1
  local expected_ec=$2
  local diff_input=$3
  local actual_ec=0

  echo "$diff_input" | LC_ALL=C.UTF-8 awk "$AWK_PROGRAM" || actual_ec=$?

  if [[ "$actual_ec" -eq "$expected_ec" ]]; then
    printf '  PASS  %-60s expected=%d actual=%d\n' "$name" "$expected_ec" "$actual_ec"
    PASS=$((PASS + 1))
  else
    printf '  FAIL  %-60s expected=%d actual=%d\n' "$name" "$expected_ec" "$actual_ec"
    FAIL=$((FAIL + 1))
  fi
}

echo "=========================================="
echo " doc-naming awk self-test"
echo "=========================================="
echo " awk impl: $(awk --version 2>&1 | head -1 || true)"
echo "------------------------------------------"

# Case 1: TODO を新規追加 → 違反検出 (exit 0)
run_case "case1: +TODO: foo (violation)" 0 \
  "--- a/CLAUDE.md
+++ b/CLAUDE.md
@@ -1,1 +1,2 @@
 existing
+TODO: foo"

# Case 2: 日本語の進捗語 (WBS) を新規追加 → 違反検出 (exit 0)
run_case "case2: +ここでWBSを運用 (violation)" 0 \
  "--- a/CLAUDE.md
+++ b/CLAUDE.md
@@ -1,1 +1,2 @@
 existing
+ここでWBSを運用"

# Case 3: TOC 形式のリンク行 → 除外、違反なし (exit 1)
run_case "case3: +- [WBS ...](#wbs-...) (TOC exempt)" 1 \
  "--- a/CLAUDE.md
+++ b/CLAUDE.md
@@ -1,1 +1,2 @@
 existing
+- [WBS 直接 push の例外](#wbs-直接-push-の例外)"

# Case 4: 進捗語を含まない平文追加 → 違反なし (exit 1)
run_case "case4: +plain text no markers (clean)" 1 \
  "--- a/CLAUDE.md
+++ b/CLAUDE.md
@@ -1,1 +1,2 @@
 existing
+The README has no progress markers."

# Case 5: バッククォート内の todo は gsub で除去される → 違反なし (exit 1)
run_case "case5: backtick-quoted \`todo\` (clean via gsub)" 1 \
  "--- a/CLAUDE.md
+++ b/CLAUDE.md
@@ -1,1 +1,2 @@
 existing
+Use the \`todo\` tool"

# Case 6: 追加行が無い diff (削除のみ) → 違反なし (exit 1)
run_case "case6: deletion-only diff (clean)" 1 \
  "--- a/CLAUDE.md
+++ b/CLAUDE.md
@@ -1,2 +1,1 @@
 existing
-old content"

# Case 7: 削除行に TODO があっても、追加行でなければ違反なし (exit 1)
run_case "case7: -TODO removed (clean; only + lines count)" 1 \
  "--- a/CLAUDE.md
+++ b/CLAUDE.md
@@ -1,2 +1,1 @@
 existing
-TODO: old task we removed"

# Case 8: 既存の '→' 除外パターン (矢印を含む行) → 違反なし (exit 1)
run_case "case8: arrow → line stays exempt" 1 \
  "--- a/CLAUDE.md
+++ b/CLAUDE.md
@@ -1,1 +1,2 @@
 existing
+flow step → TODO"

echo "------------------------------------------"
echo " PASS=$PASS  FAIL=$FAIL"
echo "=========================================="

if [[ "$FAIL" -gt 0 ]]; then
  exit 1
fi
exit 0
