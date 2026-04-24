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
