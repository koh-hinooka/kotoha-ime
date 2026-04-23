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

cd "$(dirname "$0")/.."

FAIL_COUNT=0
PASS_COUNT=0
TOTAL=10

# Pre-build once so per-assertion timings are uniform (the first
# `cargo run` otherwise dominates the wall-clock of assertion #1).
echo "=== phase0-smoke: pre-building kotoha-cli (debug) ==="
cargo build -p kotoha-cli --quiet

RUN=(cargo run --quiet -p kotoha-cli --bin kotoha-romaji --)

assert_eq() {
    local name="$1"
    local expected="$2"
    local actual="$3"
    if [ "$actual" = "$expected" ]; then
        PASS_COUNT=$((PASS_COUNT + 1))
        printf 'PASS  %s\n' "$name"
    else
        FAIL_COUNT=$((FAIL_COUNT + 1))
        printf 'FAIL  %s\n' "$name"
        printf '      expected: %q\n' "$expected"
        printf '      actual:   %q\n' "$actual"
    fi
}

echo "=== phase0-smoke: running ${TOTAL} assertions ==="

# --- Romaji → kana (4 assertions, spec §13.2) -------------------------

A1=$(printf 'konnnichiha\n' | "${RUN[@]}")
assert_eq "1. konnnichiha → こんにちは"        "こんにちは" "$A1"

A2=$(printf 'tsumugi\n'     | "${RUN[@]}")
assert_eq "2. tsumugi → つむぎ"                  "つむぎ"     "$A2"

A3=$(printf "n'ya\n"        | "${RUN[@]}")
assert_eq "3. n'ya → んや"                       "んや"       "$A3"

A4=$(printf 'nya\n'         | "${RUN[@]}")
assert_eq "4. nya → にゃ"                        "にゃ"       "$A4"

# --- Mode management (6 assertions, spec §13.2) -----------------------

A5=$(printf 'HELLO\n'       | "${RUN[@]}")
assert_eq "5. HELLO (Shift trigger) → HELLO"     "HELLO"      "$A5"

EXPECTED6=$'Ko\nこんにちは'
A6=$(printf 'Ko\nkonnnichiha\n' | "${RUN[@]}")
assert_eq "6. Ko\\nkonnnichiha (Transient auto-return) → Ko\\nこんにちは" \
          "$EXPECTED6" "$A6"

EXPECTED7=$'hello\nworld'
A7=$(printf 'hello\nworld\n' | "${RUN[@]}" --mode direct)
assert_eq "7. hello\\nworld --mode direct (Sticky) → hello\\nworld" \
          "$EXPECTED7" "$A7"

EXPECTED8=$'Konnichiwa\nこんにちは'
A8=$(printf 'Konnichiwa\nkonnnichiha\n' | "${RUN[@]}")
assert_eq "8. Konnichiwa\\nkonnnichiha (mixed) → Konnichiwa\\nこんにちは" \
          "$EXPECTED8" "$A8"

A9=$(printf 'Hi\n'          | "${RUN[@]}" --show-mode)
assert_eq "9. Hi --show-mode → Hi [H]"           "Hi [H]"     "$A9"

A10=$(printf 'hi\n'         | "${RUN[@]}" --mode direct --show-mode)
assert_eq "10. hi --mode direct --show-mode → hi [D]" "hi [D]" "$A10"

# --- Summary ----------------------------------------------------------

echo "=== phase0-smoke: ${PASS_COUNT}/${TOTAL} PASS, ${FAIL_COUNT}/${TOTAL} FAIL ==="

if [ "$FAIL_COUNT" -gt 0 ]; then
    exit 1
fi

exit 0
