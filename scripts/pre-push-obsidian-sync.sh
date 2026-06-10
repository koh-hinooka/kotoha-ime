#!/usr/bin/env bash
# pre-push-obsidian-sync.sh
# Obsidian vault 同期検証 pre-push フック
#
# 検証内容:
#   1. spec frontmatter YAML 厳密構文 (yq -e .)
#   2. related_issues / related_prs 形式 (#NNN または owner/repo#NNN)
#   3. last_reviewed 妥当日付 (yq -e で「キー不在」と「parse error」を区別)
#   4. glossary_refs と vault 内 concept 突合
#   5. spec → vault symlink 解決可能性 (空文字検出含む)
#   6. ADR 新規追加時の vault adr-index.md 反映 (index 不在も検出)
#   7. orphan glossary (WARN)
#
# 対象外:
#   - <feature>.bugs.md (/spec-reverse ADR 0001 生成物、frontmatter を持たない設計) は
#     spec frontmatter / glossary 検証から除外する。pre-commit-doc-naming.sh と整合
#   - <feature>.trace.md (trace-matrix.ts 生成物、frontmatter を持たない設計) も同様に除外。
#     pre-commit-doc-naming.sh の _skip_frontmatter_check と整合
#
# 動作:
#   - $OBSIDIAN_VAULT_DIR 未設定: silent skip exit 0
#   - yq 未 install: error exit 1
#   - yq 抽出失敗: silent ではなく ERROR
#   - vault 構造異常 (glossary/ や README.md 不在): WARN
#   - origin/develop/main/HEAD~1 解決不能 (初回 commit): WARN として可視化
#
# 一時ファイル:
#   /tmp/spec-fm.* を mktemp で作成、chmod 600。EXIT trap で全削除。
#
# 詳細: ~/.claude/CLAUDE.md §Obsidian (Scope-Shared Vaults)

set -euo pipefail

ERRORS=()
WARNINGS=()

# 本 script が mktemp で作成した一時ファイル一覧。EXIT trap で個別削除し、
# 他 process / 他 user が同 prefix で作成したファイルへの干渉を避ける。
TMPFILES=()

# trap は EXIT 時のみ間接呼び出し、shellcheck SC2329/SC2317 は invocation を検知できないため抑制
# shellcheck disable=SC2329,SC2317
_cleanup() {
  local f
  for f in "${TMPFILES[@]:-}"; do
    [ -n "$f" ] && rm -f "$f" 2>/dev/null || true
  done
}
trap _cleanup EXIT

check_requirements() {
  if [ -z "${OBSIDIAN_VAULT_DIR:-}" ]; then
    exit 0
  fi
  for cmd in yq git find awk basename date realpath; do
    if ! command -v "$cmd" >/dev/null 2>&1; then
      echo "エラー: 必須コマンド '$cmd' が見つかりません" >&2
      [[ "$cmd" == "yq" ]] && echo "  install: mise use -g yq@latest" >&2
      exit 1
    fi
  done
  # OBSIDIAN_VAULT_DIR を絶対パスに正規化、存在しないか解決不能ならエラー
  local resolved
  resolved=$(realpath -e "$OBSIDIAN_VAULT_DIR" 2>/dev/null) || {
    echo "エラー: OBSIDIAN_VAULT_DIR '$OBSIDIAN_VAULT_DIR' が存在しないか解決できません" >&2
    exit 1
  }
  OBSIDIAN_VAULT_DIR="$resolved"
}

# yq の抽出を実行し、失敗時は ERROR を立てて非 0 を返す
# usage: _yq_extract <file> <expr> <out_var_name>
_yq_extract() {
  local file="$1" expr="$2" out_var="$3"
  local result
  if ! result=$(yq -r "$expr" "$file" 2>&1); then
    ERRORS+=("yq 抽出失敗: $file ($expr) — $result")
    return 1
  fi
  printf -v "$out_var" '%s' "$result"
}

validate_spec_yaml_and_concepts() {
  [ ! -d docs/specs ] && return 0
  local spec_files
  # <feature>.bugs.md (/spec-reverse ADR 0001 生成物) と <feature>.trace.md (trace-matrix.ts 生成物) は
  # frontmatter を持たない設計のため除外。pre-commit-doc-naming.sh の _skip_frontmatter_check と整合。
  spec_files=$(find docs/specs -name "*.md" -type f | grep -vE '(README\.md|\.gitkeep|\.bugs\.md|\.trace\.md)$' || true)
  while IFS= read -r file; do
    [ -z "$file" ] && continue
    local fm_file
    fm_file=$(mktemp -t spec-fm.XXXXXXXXXX)
    TMPFILES+=("$fm_file")
    if ! chmod 600 "$fm_file"; then
      ERRORS+=("一時ファイル chmod 600 失敗: $fm_file (tmpfs/SELinux 制限の可能性)")
      rm -f "$fm_file"
      continue
    fi
    if ! awk '/^---$/{n++; if(n==2)exit; next} n==1' "$file" > "$fm_file"; then
      ERRORS+=("awk frontmatter 抽出失敗: $file")
      rm -f "$fm_file"
      continue
    fi

    if ! yq -e . "$fm_file" >/dev/null 2>&1; then
      ERRORS+=("Spec frontmatter YAML parse 失敗: $file")
      rm -f "$fm_file"
      continue
    fi

    # last_reviewed: yq -e で「キー不在」と「parse error」を区別
    # キー不在は pre-commit-doc-naming.sh で検出済みなので skip
    local last_reviewed
    if last_reviewed=$(yq -e -r '.last_reviewed' "$fm_file" 2>/dev/null); then
      if ! date -d "$last_reviewed" >/dev/null 2>&1; then
        ERRORS+=("last_reviewed 妥当日付違反: $file (値: '$last_reviewed')")
      fi
    fi

    # test_plan_status: ADR 0036 (2026-05-12) で導入された 8 番目の必須 field。
    # enum 検証は pre-commit-doc-naming.sh で実施済みだが、yq parse 整合 + キー存在を
    # ここでも確認 (pre-commit を bypass した manual push 経路の防御)。
    local test_plan_status
    if test_plan_status=$(yq -e -r '.test_plan_status' "$fm_file" 2>/dev/null); then
      case "$test_plan_status" in
        draft|designed|executing|evaluated) ;;
        *) ERRORS+=("test_plan_status 値違反: $file (値: '$test_plan_status', 期待: draft|designed|executing|evaluated)") ;;
      esac
    else
      ERRORS+=("test_plan_status キー不在: $file (ADR 0036 8 fields 必須)")
    fi

    # related_issues / related_prs / glossary_refs: yq 抽出は明示的に rc を確認
    local issues_out prs_out refs_out
    local issues=() prs=() refs=()
    if _yq_extract "$fm_file" '.related_issues[]?' issues_out; then
      [ -n "$issues_out" ] && mapfile -t issues <<< "$issues_out"
    fi
    if _yq_extract "$fm_file" '.related_prs[]?' prs_out; then
      [ -n "$prs_out" ] && mapfile -t prs <<< "$prs_out"
    fi
    if _yq_extract "$fm_file" '.glossary_refs[]?' refs_out; then
      [ -n "$refs_out" ] && mapfile -t refs <<< "$refs_out"
    fi

    local issue pr ref
    for issue in "${issues[@]}"; do
      [ -z "$issue" ] && continue
      [ "$issue" = "null" ] && continue
      if ! [[ "$issue" =~ ^#[0-9]+$ || "$issue" =~ ^[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+#[0-9]+$ ]]; then
        ERRORS+=("related_issues 形式違反: $file 要素 '$issue' (期待: #NNN または owner/repo#NNN)")
      fi
    done
    for pr in "${prs[@]}"; do
      [ -z "$pr" ] && continue
      [ "$pr" = "null" ] && continue
      if ! [[ "$pr" =~ ^#[0-9]+$ || "$pr" =~ ^[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+#[0-9]+$ ]]; then
        ERRORS+=("related_prs 形式違反: $file 要素 '$pr' (期待: #NNN または owner/repo#NNN)")
      fi
    done
    for ref in "${refs[@]}"; do
      [ -z "$ref" ] && continue
      [ "$ref" = "null" ] && continue
      if [ ! -f "$OBSIDIAN_VAULT_DIR/glossary/$ref.md" ]; then
        ERRORS+=("Glossary 不在: $file の glossary_refs に '$ref' が指定されていますが、$OBSIDIAN_VAULT_DIR/glossary/$ref.md が存在しません")
      fi
    done

    rm -f "$fm_file"
  done <<< "$spec_files"
}

validate_spec_symlink() {
  [ ! -d docs/specs ] && return 0
  local project_name symlink target expected
  project_name=$(basename "$(pwd)")
  symlink="$OBSIDIAN_VAULT_DIR/specs/$project_name"
  if [ -L "$symlink" ]; then
    target=$(realpath "$symlink" 2>/dev/null || echo "")
    expected=$(realpath "docs/specs" 2>/dev/null || echo "")
    if [ -z "$target" ] || [ -z "$expected" ]; then
      ERRORS+=("Vault symlink 解決不能: $symlink (target='$target', expected='$expected')")
    elif [ "$target" != "$expected" ]; then
      ERRORS+=("Vault symlink 不整合: $symlink が docs/specs を指していません (target: $target, expected: $expected)")
    fi
  fi
}

validate_adr_index_entry() {
  # base: feature branch の divergence point (origin/develop または origin/main との
  # merge-base) を採用する。@{u} (upstream tracking branch) は push 後の feature
  # branch では同 branch を指すため、新規 ADR を含む commit が "既 push" 状態に
  # なった瞬間に diff が空集合となり検証が false-PASS する (re-push でも同じ)。
  # よって PR scope 全体を base から見て新規追加 ADR を検出する方針に変更。
  # 解決順: origin/develop → origin/main → HEAD~1 → skip
  local base=""
  if base=$(git merge-base HEAD origin/develop 2>/dev/null); then
    :
  elif base=$(git merge-base HEAD origin/main 2>/dev/null); then
    :
  elif git rev-parse HEAD~1 >/dev/null 2>&1; then
    base="HEAD~1"
  else
    WARNINGS+=("ADR index 検証 skip: base ref 解決不能 (origin/develop/main/HEAD~1 不在)")
    return 0
  fi
  local idx="$OBSIDIAN_VAULT_DIR/adr-index.md"
  local new_adrs
  new_adrs=$(git diff --diff-filter=A --name-only "${base}...HEAD" 2>/dev/null | grep '^docs/adr/.*\.md$' || true)
  while IFS= read -r adr; do
    [ -z "$adr" ] && continue
    local fname
    fname=$(basename "$adr")
    [[ "$fname" == ".gitkeep" ]] && continue
    # README.md は ADR 索引そのものであり ADR 本体ではないため vault adr-index 反映検証から除外する。
    [[ "$fname" == "README.md" ]] && continue
    if [ ! -f "$idx" ]; then
      ERRORS+=("ADR index ファイル不在: $idx (vault が壊れている可能性)")
      return 1
    fi
    if ! grep -qF "$fname" "$idx"; then
      ERRORS+=("ADR index 未反映: 新規 ADR $adr が $idx に未追加です")
    fi
  done <<< "$new_adrs"
}

detect_orphan_glossary() {
  if [ ! -d "$OBSIDIAN_VAULT_DIR/glossary" ]; then
    WARNINGS+=("vault 構造異常: $OBSIDIAN_VAULT_DIR/glossary/ 不在")
    return 0
  fi
  local readme="$OBSIDIAN_VAULT_DIR/README.md"
  if [ ! -f "$readme" ]; then
    WARNINGS+=("vault 構造異常: $OBSIDIAN_VAULT_DIR/README.md 不在")
    return 0
  fi
  if [ ! -d docs/specs ]; then
    return 0
  fi
  local new_glossary
  new_glossary=$(find "$OBSIDIAN_VAULT_DIR/glossary" -name "*.md" -newer "$readme" 2>/dev/null | grep -v '\.gitkeep$' || true)
  while IFS= read -r gloss; do
    [ -z "$gloss" ] && continue
    local concept
    concept=$(basename "$gloss" .md)
    # $concept は外部 vault のファイル名由来 (regex メタ文字を含む可能性あり)。
    # grep -F で固定文字列マッチに固定し、regex injection を回避。
    if ! grep -rqF "glossary_refs" docs/specs/ 2>/dev/null; then
      # docs/specs に glossary_refs 参照行が一切ない場合は orphan 判定 skip
      WARNINGS+=("Orphan glossary: $gloss はいずれの spec の glossary_refs からも参照されていません")
    # set -euo pipefail 環境下では `grep -r ... | grep -qF ...` の右側が早期 exit で
    # stdin を close すると左側 grep が SIGPIPE で 141 を返し、pipefail で pipe 全体が
    # non-zero と判定されて false orphan を誤検出する (ISSUE #497)。
    # 左側を `{ ... || true; }` で wrap して SIGPIPE / 非ゼロ exit を吸収する。
    elif ! { grep -r "glossary_refs:" docs/specs/ 2>/dev/null || true; } | grep -qF "$concept" && \
         ! { grep -rE "^- |^  - " docs/specs/ 2>/dev/null || true; } | grep -qF "$concept"; then
      WARNINGS+=("Orphan glossary: $gloss はいずれの spec の glossary_refs からも参照されていません")
    fi
  done <<< "$new_glossary"
}

report_errors_and_warnings() {
  if [ ${#ERRORS[@]} -gt 0 ] || [ ${#WARNINGS[@]} -gt 0 ]; then
    echo "========================================"
    echo " pre-push: Obsidian 同期チェック"
    echo "========================================"
    for err in "${ERRORS[@]}"; do
      echo "[ERROR]   $err"
    done
    for warn in "${WARNINGS[@]}"; do
      echo "[WARN]    $warn"
    done
    echo ""
    echo "規約の詳細は ~/.claude/CLAUDE.md §Obsidian (Scope-Shared Vaults) を参照してください。"
  fi
  if [ ${#ERRORS[@]} -gt 0 ]; then
    exit 1
  fi
  exit 0
}

main() {
  check_requirements
  validate_spec_yaml_and_concepts
  validate_spec_symlink
  validate_adr_index_entry
  detect_orphan_glossary
  report_errors_and_warnings
}

main "$@"
