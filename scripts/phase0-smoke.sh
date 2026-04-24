#!/usr/bin/env bash
# Phase 0 smoke test — runs spec §13.2's 10 CLI acceptance assertions.
#
# Usage:
#   scripts/phase0-smoke.sh
#
# Exit codes:
#   0   — all 10 assertions PASS
#   1   — at least one assertion FAILED (or build error)
#
# Invocation model: uses `cargo run -p kotoha-cli --bin kotoha-romaji --`
# (debug build) per plan M6 §『本 M6 plan 内で解決する既知の懸念』. The
# first invocation triggers compilation; subsequent invocations re-use
# the cache and start in < 200 ms.
#
# Reference:
#   - Spec §13.2 (docs/superpowers/specs/2026-04-22-kotoha-phase-0-design.md)
#   - Plan  M6   (docs/superpowers/plans/2026-04-23-kotoha-phase-0-m6.md)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/lib/assert.sh"

cd "$(dirname "$0")/.."

# Pre-build once so per-assertion timings are uniform (the first
# `cargo run` otherwise dominates the wall-clock of assertion #1).
echo "=== phase0-smoke: pre-building kotoha-cli (debug) ==="
cargo build -p kotoha-cli --quiet

RUN=(cargo run --quiet -p kotoha-cli --bin kotoha-romaji --)

echo "=== phase0-smoke: running 10 assertions ==="

# --- Romaji → kana (4 assertions, spec §13.2) -------------------------

A1=$(printf 'konnnichiha\n' | "${RUN[@]}")
assert_equal "1. konnnichiha → こんにちは"        "$A1" "こんにちは"

A2=$(printf 'tsumugi\n'     | "${RUN[@]}")
assert_equal "2. tsumugi → つむぎ"                  "$A2" "つむぎ"

A3=$(printf "n'ya\n"        | "${RUN[@]}")
assert_equal "3. n'ya → んや"                       "$A3" "んや"

A4=$(printf 'nya\n'         | "${RUN[@]}")
assert_equal "4. nya → にゃ"                        "$A4" "にゃ"

# --- Mode management (6 assertions, spec §13.2) -----------------------

A5=$(printf 'HELLO\n'       | "${RUN[@]}")
assert_equal "5. HELLO (Shift trigger) → HELLO"     "$A5" "HELLO"

EXPECTED6=$'Ko\nこんにちは'
A6=$(printf 'Ko\nkonnnichiha\n' | "${RUN[@]}")
assert_equal "6. Ko\\nkonnnichiha (Transient auto-return) → Ko\\nこんにちは" \
          "$A6" "$EXPECTED6"

EXPECTED7=$'hello\nworld'
A7=$(printf 'hello\nworld\n' | "${RUN[@]}" --mode direct)
assert_equal "7. hello\\nworld --mode direct (Sticky) → hello\\nworld" \
          "$A7" "$EXPECTED7"

EXPECTED8=$'Konnichiwa\nこんにちは'
A8=$(printf 'Konnichiwa\nkonnnichiha\n' | "${RUN[@]}")
assert_equal "8. Konnichiwa\\nkonnnichiha (mixed) → Konnichiwa\\nこんにちは" \
          "$A8" "$EXPECTED8"

A9=$(printf 'Hi\n'          | "${RUN[@]}" --show-mode)
assert_equal "9. Hi --show-mode → Hi [H]"           "$A9" "Hi [H]"

A10=$(printf 'hi\n'         | "${RUN[@]}" --mode direct --show-mode)
assert_equal "10. hi --mode direct --show-mode → hi [D]" "$A10" "hi [D]"

# --- Summary ----------------------------------------------------------

assert_summary "phase0-smoke"
