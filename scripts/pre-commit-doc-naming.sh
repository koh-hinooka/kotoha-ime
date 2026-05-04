#!/usr/bin/env bash
# pre-commit-doc-naming.sh
# ドキュメント命名・配置・frontmatter 規約を検証する pre-commit フック
#
# 検証対象 (本 hook の責務):
#   - docs/plans/                  : <yyyy-MM-dd>-<branch>.md (現行維持)
#   - docs/specs/                  : 配置・命名 + frontmatter 必須 7 キー存在
#                                  + feature/bounded_context のファイル名/dir 名一致
#                                  + status enum + last_reviewed 形式
#   - docs/adr/                    : NNNN-<title>.md (現行維持)
#   - CLAUDE.md (ルート/サブ)      : 進捗情報の直接記載がないこと (現行維持)
#   - docs/superpowers/ 配下       : ファイル残存禁止 (廃止検証)
#
# 検証範囲外 (pre-push hook `pre-push-obsidian-sync.sh` の責務):
#   - related_issues / related_prs 形式 (#NNN または owner/repo#NNN)
#   - glossary_refs と Obsidian vault 内 concept の突合
#   - vault → spec symlink の解決可能性
#   - ADR 新規追加時の vault adr-index.md 反映
#   - YAML 厳密構文検証 (yq parse)
#
# 詳細仕様: ~/.claude/CLAUDE.md §Documentation Structure
#
# Invocation:
#   - lefthook pre-commit から自動実行 (lefthook.yml で配線)
#   - 手動実行: bash scripts/pre-commit-doc-naming.sh
#
# 配布:
#   cp ~/.claude/templates/scripts/pre-commit-doc-naming.sh scripts/
#   lefthook install

set -euo pipefail

ERRORS=()
STAGED_FILES=""

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

check_requirements() {
  local cmds=(git grep awk basename)
  for cmd in "${cmds[@]}"; do
    if ! command -v "$cmd" >/dev/null 2>&1; then
      echo "エラー: 必須コマンド '$cmd' が見つかりません" >&2
      exit 1
    fi
  done
}

get_staged_files() {
  STAGED_FILES=$(git diff --cached --name-only --diff-filter=ACM) || {
    echo "エラー: ステージされたファイル一覧の取得に失敗しました" >&2
    exit 1
  }
}

check_plan_naming() {
  [[ -z "$STAGED_FILES" ]] && return 0
  local pattern='^[0-9]{4}-[0-9]{2}-[0-9]{2}-.+\.md$'
  local filtered_files
  filtered_files=$(echo "$STAGED_FILES" | safe_grep '^docs/plans/')
  while IFS= read -r file; do
    [ -z "$file" ] && continue
    local fname
    fname=$(basename "$file")
    [[ "$fname" == ".gitkeep" ]] && continue
    if [[ ! "$fname" =~ $pattern ]]; then
      ERRORS+=("Plan 命名違反: $file (期待: yyyy-MM-dd-<branch>.md)")
    fi
  done <<< "$filtered_files"
}

_iter_spec_files() {
  echo "$STAGED_FILES" | safe_grep '^docs/specs/.*\.md$'
}

_skip_special_spec_file() {
  local fname="$1" dir_path="$2"
  [[ "$fname" == ".gitkeep" ]] && return 0
  [[ "$fname" == "README.md" && "$dir_path" == "docs/specs" ]] && return 0
  return 1
}

check_spec_placement() {
  [[ -z "$STAGED_FILES" ]] && return 0
  local spec_files
  spec_files=$(_iter_spec_files)
  while IFS= read -r file; do
    [ -z "$file" ] && continue
    local fname dir_path depth
    fname=$(basename "$file")
    dir_path=$(dirname "$file")
    _skip_special_spec_file "$fname" "$dir_path" && continue
    # 配置検証 (path depth ベース):
    #   - depth=3 (docs/specs/foo.md): 直下配置 → 禁止
    #   - depth=4 (docs/specs/<bc>/foo.md): 正常 (_uncategorized/ 含む)
    #   - depth>4 (docs/specs/<bc>/sub/foo.md): bounded-context 配下の subdirectory → 禁止
    depth=$(awk -F'/' '{print NF}' <<< "$file")
    if [[ "$dir_path" == "docs/specs" ]]; then
      ERRORS+=("Spec 配置違反: $file (期待: docs/specs/{_uncategorized,<bounded-context>}/<feature-slug>.md, 直下配置禁止)")
      continue
    fi
    if [[ $depth -gt 4 ]]; then
      ERRORS+=("Spec 配置違反: $file (bounded-context dir 配下の subdirectory 禁止, 期待 depth=4)")
    fi
  done <<< "$spec_files"
}

check_spec_dir_name() {
  [[ -z "$STAGED_FILES" ]] && return 0
  local spec_files
  spec_files=$(_iter_spec_files)
  while IFS= read -r file; do
    [ -z "$file" ] && continue
    local fname dir_path bc_dir
    fname=$(basename "$file")
    dir_path=$(dirname "$file")
    _skip_special_spec_file "$fname" "$dir_path" && continue
    [[ "$dir_path" == "docs/specs" ]] && continue
    bc_dir=$(basename "$dir_path")
    if [[ "$bc_dir" != "_uncategorized" ]] && [[ ! "$bc_dir" =~ ^[a-z][a-z0-9-]*$ ]]; then
      ERRORS+=("Bounded context dir 命名違反: $dir_path (期待: kebab-case ^[a-z][a-z0-9-]*\$ または _uncategorized)")
    fi
  done <<< "$spec_files"
}

check_spec_filename() {
  [[ -z "$STAGED_FILES" ]] && return 0
  local spec_files
  spec_files=$(_iter_spec_files)
  while IFS= read -r file; do
    [ -z "$file" ] && continue
    local fname dir_path
    fname=$(basename "$file")
    dir_path=$(dirname "$file")
    _skip_special_spec_file "$fname" "$dir_path" && continue
    [[ "$dir_path" == "docs/specs" ]] && continue
    if [[ ! "$fname" =~ ^[a-z][a-z0-9-]*\.md$ ]]; then
      ERRORS+=("Spec 命名違反: $file (期待: kebab-case ^[a-z][a-z0-9-]*\\.md\$, アンダースコア prefix は予約)")
    fi
  done <<< "$spec_files"
}

_extract_frontmatter() {
  local file="$1"
  awk '/^---$/{n++; if(n==2)exit; next} n==1' "$file"
}

_validate_required_keys() {
  local file="$1" fm="$2"
  local required_keys=("feature" "status" "bounded_context" "related_issues" "related_prs" "glossary_refs" "last_reviewed")
  local missing=()
  local key
  for key in "${required_keys[@]}"; do
    if ! echo "$fm" | grep -qF "${key}:"; then
      missing+=("$key")
    fi
  done
  if [ ${#missing[@]} -gt 0 ]; then
    ERRORS+=("Frontmatter 必須項目欠落: $file (欠落キー: ${missing[*]})")
    return 1
  fi
  return 0
}

_validate_field_consistency() {
  local file="$1" fm="$2" slug="$3" bc_dir="$4"
  local valid_status=("draft" "reviewed" "implemented" "deprecated")
  local feature_value bc_value status_value last_reviewed_value
  feature_value=$(echo "$fm" | awk -F': *' '/^feature:/{print $2; exit}')
  bc_value=$(echo "$fm" | awk -F': *' '/^bounded_context:/{print $2; exit}')
  status_value=$(echo "$fm" | awk -F': *' '/^status:/{print $2; exit}')
  last_reviewed_value=$(echo "$fm" | awk -F': *' '/^last_reviewed:/{print $2; exit}')

  if [[ "$feature_value" != "$slug" ]]; then
    ERRORS+=("Frontmatter 値不整合: $file (feature='$feature_value' ≠ ファイル名 '$slug')")
  fi
  if [[ "$bc_value" != "$bc_dir" ]]; then
    ERRORS+=("Frontmatter 値不整合: $file (bounded_context='$bc_value' ≠ ディレクトリ名 '$bc_dir')")
  fi
  local is_valid_status=0 v
  for v in "${valid_status[@]}"; do
    [[ "$status_value" == "$v" ]] && is_valid_status=1 && break
  done
  if [[ $is_valid_status -eq 0 ]]; then
    ERRORS+=("Frontmatter status 値違反: $file (status='$status_value', 期待: draft|reviewed|implemented|deprecated)")
  fi
  if [[ ! "$last_reviewed_value" =~ ^[0-9]{4}-[0-9]{2}-[0-9]{2}$ ]]; then
    ERRORS+=("Frontmatter last_reviewed 形式違反: $file (期待: yyyy-MM-dd)")
  fi
}

check_spec_frontmatter() {
  [[ -z "$STAGED_FILES" ]] && return 0
  local spec_files
  spec_files=$(echo "$STAGED_FILES" | safe_grep '^docs/specs/.*/.*\.md$')
  while IFS= read -r file; do
    [ -z "$file" ] && continue
    local fname dir_path bc_dir slug
    fname=$(basename "$file")
    dir_path=$(dirname "$file")
    [[ "$fname" == ".gitkeep" ]] && continue
    [[ "$fname" == "README.md" ]] && continue
    bc_dir=$(basename "$dir_path")
    slug="${fname%.md}"
    [ -f "$file" ] || continue

    local fm
    fm=$(_extract_frontmatter "$file")
    [ -z "$fm" ] && { ERRORS+=("Frontmatter 不在: $file"); continue; }

    _validate_required_keys "$file" "$fm" || continue
    _validate_field_consistency "$file" "$fm" "$slug" "$bc_dir"
  done <<< "$spec_files"
}

check_no_superpowers_dir() {
  [[ -z "$STAGED_FILES" ]] && return 0
  local filtered_files
  filtered_files=$(echo "$STAGED_FILES" | safe_grep '^docs/superpowers/')
  while IFS= read -r file; do
    [ -z "$file" ] && continue
    ERRORS+=("Spec/plan 配置違反: $file (docs/superpowers/ ディレクトリは廃止されました。docs/specs/ または docs/plans/ へ移動してください)")
  done <<< "$filtered_files"
}

check_adr_naming() {
  [[ -z "$STAGED_FILES" ]] && return 0
  local pattern='^[0-9]{4}-.+\.md$'
  local filtered_files
  filtered_files=$(echo "$STAGED_FILES" | safe_grep '^docs/adr/')
  while IFS= read -r file; do
    [ -z "$file" ] && continue
    local fname
    fname=$(basename "$file")
    [[ "$fname" == ".gitkeep" ]] && continue
    if [[ ! "$fname" =~ $pattern ]]; then
      ERRORS+=("ADR 命名違反: $file (期待: NNNN-<title>.md)")
    fi
  done <<< "$filtered_files"
}

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
        if (line ~ /(todo|進捗|タスク一覧|完了率|[0-9]+%|■|□|☑|☐|\[x\]|\[ \])/) { found = 1; exit }
      }
      END { if (!found) exit 1 }
    '; then
      ERRORS+=("CLAUDE.md に進捗情報の疑い: $file (進捗管理は ROADMAP.md または TaskCreate を使用してください)")
    fi
  done <<< "$filtered_files"
}

report_errors() {
  if [ ${#ERRORS[@]} -gt 0 ]; then
    echo "========================================"
    echo " pre-commit: ドキュメント規約チェック失敗"
    echo "========================================"
    for err in "${ERRORS[@]}"; do
      echo "  - $err"
    done
    echo ""
    echo "規約の詳細は ~/.claude/CLAUDE.md §Documentation Structure / §Spec Frontmatter を参照してください。"
    exit 1
  fi
  # 成功時 hint: pre-push で別途厳密検証が走ることを明示する。
  # pre-commit は軽量検証のため、push 時に YAML 厳密構文 / glossary 突合 / symlink 解決等が
  # 追加実施される (scripts/pre-push-obsidian-sync.sh)。pre-commit PASS = "push まで安全" ではない。
  echo "[doc-naming] OK (pre-push で YAML 厳密検証 + glossary 突合 + symlink 解決を実施)"
  exit 0
}

main() {
  check_requirements
  get_staged_files
  check_plan_naming
  check_spec_placement
  check_spec_dir_name
  check_spec_filename
  check_spec_frontmatter
  check_no_superpowers_dir
  check_adr_naming
  check_claude_md_progress
  report_errors
}

main "$@"
