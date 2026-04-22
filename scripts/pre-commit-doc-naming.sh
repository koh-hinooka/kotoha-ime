#!/usr/bin/env bash
# pre-commit-doc-naming.sh
# ドキュメント命名規則を検証するpre-commitフック
#
# 検証対象:
#   - docs/wbs/, docs/superpowers/plans/, docs/superpowers/specs/
#     → yyyy-MM-dd-<ブランチ名>.md 形式
#   - docs/adr/
#     → NNNN-<タイトル>.md 形式
#   - CLAUDE.md（ルート・サブディレクトリ）
#     → 進捗情報の直接記載がないこと
#
# 使用方法:
#   cp ~/.claude/templates/scripts/pre-commit-doc-naming.sh scripts/
#   ln -sf ../../scripts/pre-commit-doc-naming.sh .git/hooks/pre-commit
#   chmod +x scripts/pre-commit-doc-naming.sh

set -euo pipefail

ERRORS=()
STAGED_FILES=""

# grepのマッチなし(終了コード1)は許容し、エラー(終了コード2)は検出するヘルパー
# 使用法: safe_grep <args...>
safe_grep() {
  local output rc=0
  output=$(grep "$@") || rc=$?
  if [[ $rc -eq 0 || $rc -eq 1 ]]; then
    echo "$output"
    return 0
  fi
  echo "エラー: grep がコード $rc で失敗しました (引数: $*)" >&2
  exit 1
}

# 必須コマンドの存在確認
check_requirements() {
  local cmds=(git grep awk basename)
  for cmd in "${cmds[@]}"; do
    if ! command -v "$cmd" >/dev/null 2>&1; then
      echo "エラー: 必須コマンド '$cmd' が見つかりません" >&2
      exit 1
    fi
  done
}

# ステージされたファイル一覧を取得（追加・変更のみ）
get_staged_files() {
  STAGED_FILES=$(git diff --cached --name-only --diff-filter=ACM) || {
    echo "エラー: ステージされたファイル一覧の取得に失敗しました" >&2
    exit 1
  }
}

# 日付+ブランチ名の命名規則チェック (docs/wbs, docs/superpowers/plans, docs/superpowers/specs)
check_date_branch_naming() {
  [[ -z "$STAGED_FILES" ]] && return 0

  local date_branch_pattern='^[0-9]{4}-[0-9]{2}-[0-9]{2}-.+\.md$'
  local dirs=("docs/wbs" "docs/superpowers/plans" "docs/superpowers/specs")
  local filtered_files

  for dir in "${dirs[@]}"; do
    filtered_files=$(echo "$STAGED_FILES" | safe_grep "^${dir}/")
    while IFS= read -r file; do
      [ -z "$file" ] && continue
      local fname
      fname=$(basename "$file")
      [[ "$fname" == ".gitkeep" ]] && continue
      if [[ ! "$fname" =~ $date_branch_pattern ]]; then
        ERRORS+=("命名規則違反: $file (期待: yyyy-MM-dd-<ブランチ名>.md)")
      fi
    done <<< "$filtered_files"
  done
}

# ADR命名規則チェック (docs/adr)
check_adr_naming() {
  [[ -z "$STAGED_FILES" ]] && return 0

  local adr_pattern='^[0-9]{4}-.+\.md$'
  local filtered_files

  filtered_files=$(echo "$STAGED_FILES" | safe_grep '^docs/adr/')
  while IFS= read -r file; do
    [ -z "$file" ] && continue
    local fname
    fname=$(basename "$file")
    [[ "$fname" == ".gitkeep" ]] && continue
    if [[ ! "$fname" =~ $adr_pattern ]]; then
      ERRORS+=("ADR命名規則違反: $file (期待: NNNN-<タイトル>.md)")
    fi
  done <<< "$filtered_files"
}

# CLAUDE.mdに進捗情報が含まれていないかチェック
# 追加行のみを対象にし、パス参照（`docs/wbs/`等）は除外する
check_claude_md_progress() {
  [[ -z "$STAGED_FILES" ]] && return 0

  local filtered_files
  filtered_files=$(echo "$STAGED_FILES" | safe_grep 'CLAUDE\.md$')

  while IFS= read -r file; do
    [ -z "$file" ] && continue
    local diff_output
    diff_output=$(git diff --cached -- "$file") || {
      echo "エラー: git diff --cached -- $file が失敗しました" >&2
      exit 1
    }
    if echo "$diff_output" | LC_ALL=C.UTF-8 awk '
      /^\+/ && !/^\+\+\+/ && !/→/ {
        gsub(/`[^`]*`/, "")
        line = tolower($0)
        if (line ~ /(todo|wbs|進捗|タスク一覧|完了率|[0-9]+%|■|□|☑|☐|\[x\]|\[ \])/) exit 0
      }
      END { exit 1 }
    '; then
      ERRORS+=("CLAUDE.mdに進捗情報の疑い: $file (進捗管理はdocs/wbs/を使用してください)")
    fi
  done <<< "$filtered_files"
}

# 結果出力
report_errors() {
  if [ ${#ERRORS[@]} -gt 0 ]; then
    echo "========================================"
    echo " pre-commit: ドキュメント規約チェック失敗"
    echo "========================================"
    for err in "${ERRORS[@]}"; do
      echo "  - $err"
    done
    echo ""
    echo "規約の詳細は ~/.claude/CLAUDE.md を参照してください。"
    exit 1
  fi

  exit 0
}

main() {
  check_requirements
  get_staged_files
  check_date_branch_naming
  check_adr_naming
  check_claude_md_progress
  report_errors
}

main "$@"
