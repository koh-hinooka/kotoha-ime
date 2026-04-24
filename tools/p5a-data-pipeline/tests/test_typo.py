"""Tests for QWERTY-adjacent typo injection."""

import string

import pytest

from kotoha_p5a.typo import QWERTY_ADJACENCY, inject_typos


def test_distance_zero_returns_input_unchanged() -> None:
    assert inject_typos("koohii", 0, seed=42) == "koohii"


def test_distance_one_produces_exactly_one_diff_or_length_delta() -> None:
    # For a single operation, the Levenshtein distance to the original is
    # at most 1. Here we sanity-check the simpler property that the output
    # differs from the input and is within +/-1 character of the original.
    original = "kippu"
    noisy = inject_typos(original, 1, seed=123)
    assert noisy != original
    assert abs(len(noisy) - len(original)) <= 1


def test_seed_is_reproducible() -> None:
    a = inject_typos("konnichiha", 3, seed=7)
    b = inject_typos("konnichiha", 3, seed=7)
    assert a == b


def test_different_seeds_can_diverge() -> None:
    # Two different seeds usually produce different corruptions; when they
    # happen to collide, the test asserts at least one of several seeds
    # diverges to avoid flakiness on a single unlucky pair.
    base = "koohii"
    outputs = {inject_typos(base, 2, seed=s) for s in range(1, 20)}
    assert len(outputs) > 1


def test_substitute_stays_within_qwerty_adjacency() -> None:
    # After a single substitution, every character in the output must
    # either equal the original position's character or be one of that
    # original character's QWERTY neighbours. Also, inserted/deleted
    # characters obviously shift positions; to keep this strict we force
    # a length-preserving operation by re-running until we hit one.
    rng_seed = 0
    original = "abcdef"
    for seed in range(200):
        noisy = inject_typos(original, 1, seed=seed)
        if len(noisy) == len(original) and noisy != original:
            # Exactly one position differs.
            diffs = [
                (i, original[i], noisy[i]) for i in range(len(original)) if original[i] != noisy[i]
            ]
            if len(diffs) != 1:
                continue
            _, orig_ch, new_ch = diffs[0]
            assert new_ch in QWERTY_ADJACENCY[orig_ch]
            rng_seed = seed
            break
    else:
        pytest.fail("no length-preserving substitution found in 200 seeds")
    assert rng_seed >= 0


def test_negative_distance_raises() -> None:
    with pytest.raises(ValueError, match="non-negative"):
        inject_typos("abc", -1, seed=1)


def test_adjacency_table_is_lowercase_ascii_plus_known_punctuation() -> None:
    # Regression guard: if someone adds digits or uppercase entries by
    # mistake, the PoC's lowercase-only romaji assumption would break.
    allowed = set(string.ascii_lowercase) | {" ", ",", "."}
    assert set(QWERTY_ADJACENCY.keys()) == allowed
