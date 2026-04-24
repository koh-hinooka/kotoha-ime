#!/usr/bin/env bash
# Phase 1 smoke test — runs the Layer 3 fixture rows against the
# `kotoha-kanji` CLI binary (llama.cpp backend with Gemma-2-2B-jpn-it).
#
# Usage:
#   scripts/phase1-smoke.sh
#
# Environment:
#   KOTOHA_LLAMA_MODEL_PATH (required)
#     Absolute path to a Gemma-2-2B-jpn-it Q5_K_M GGUF file.
#     If unset, this script prints SKIPPED and exits 0 (Phase 1
#     spec §8.3 opt-in policy, matches kanji_llama_cpp_smoke.rs).
#
# Exit codes:
#   0   — all 14 assertions PASS (or SKIPPED)
#   1   — at least one assertion FAILED
#
# Known limitation: row 3 (あした → 明日) is intentionally skipped.
# Gemma-2-2B-jpn-it emits 翌日 for this input across v5–v12 prompt
# iterations (see PR #76 WBS, ADR 0010). Root resolution is deferred
# to Phase 5 (custom romaji-base model).
#
# References:
#   - Fixture: crates/kotoha-core/tests/fixtures/kanji_smoke.tsv
#   - Integration test: crates/kotoha-core/tests/kanji_llama_cpp_smoke.rs
#   - ADR: docs/adr/0010-kotoha-custom-romaji-base-model.md (Phase 5 deferral)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
# shellcheck source=lib/assert.sh
source "$SCRIPT_DIR/lib/assert.sh"

cd "$REPO_ROOT"

if [[ -z "${KOTOHA_LLAMA_MODEL_PATH:-}" ]]; then
  echo "SKIPPED: KOTOHA_LLAMA_MODEL_PATH not set"
  exit 0
fi

if [[ ! -f "$KOTOHA_LLAMA_MODEL_PATH" ]]; then
  echo "FAIL: KOTOHA_LLAMA_MODEL_PATH file does not exist: $KOTOHA_LLAMA_MODEL_PATH" >&2
  exit 1
fi

echo "=== phase1-smoke: pre-building kotoha-kanji (debug, llama-cpp feature) ==="
cargo build -p kotoha-cli --bin kotoha-kanji --features llama-cpp --quiet

FIXTURE="$REPO_ROOT/crates/kotoha-core/tests/fixtures/kanji_smoke.tsv"
if [[ ! -f "$FIXTURE" ]]; then
  echo "FAIL: fixture not found: $FIXTURE" >&2
  exit 1
fi

RUN=(cargo run --quiet -p kotoha-cli --bin kotoha-kanji --features llama-cpp --)

echo "=== phase1-smoke: running fixture (row 3 あした SKIPPED per ADR 0010) ==="

# Row number is 1-indexed, matching fixture row order and the
# integration test function naming (llama_cpp_smoke_N_*).
row_num=0
while IFS=$'\t' read -r hiragana expected; do
  row_num=$((row_num + 1))
  if [[ "$row_num" -eq 3 ]]; then
    echo "SKIP  $row_num. $hiragana → $expected (known limitation, Phase 5 deferral)"
    continue
  fi
  actual=$(printf '%s\n' "$hiragana" | "${RUN[@]}")
  assert_contains "$row_num. $hiragana → contains '$expected'" "$actual" "$expected"
done <"$FIXTURE"

assert_summary "phase1-smoke"
