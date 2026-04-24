"""Tests for partial-input generation (random prefix truncation)."""

import random

import pytest

from kotoha_p5a.partial import generate_partials, truncate_random


def test_truncate_random_is_reproducible_with_same_seed() -> None:
    """同じシードで構築した RNG なら :func:`truncate_random` は同一結果を返す。"""
    text = "konnichiha"
    rng_a = random.Random(42)
    rng_b = random.Random(42)
    assert truncate_random(text, rng_a) == truncate_random(text, rng_b)


def test_truncate_random_rejects_min_prefix_len_below_one() -> None:
    """``min_prefix_len`` が 1 未満のとき :class:`ValueError` を送出する。"""
    rng = random.Random(0)
    with pytest.raises(ValueError, match="min_prefix_len must be >= 1"):
        truncate_random("abc", rng, min_prefix_len=0)


def test_truncate_random_returns_text_when_min_prefix_len_covers_text() -> None:
    """``len(text) <= min_prefix_len`` のとき入力の ``text`` をそのまま返す。"""
    rng = random.Random(0)
    # len(text) == min_prefix_len のケース
    assert truncate_random("abc", rng, min_prefix_len=3) == "abc"
    # len(text) < min_prefix_len のケース
    assert truncate_random("ab", rng, min_prefix_len=5) == "ab"


def test_truncate_random_result_is_prefix_of_input() -> None:
    """戻り値は常に入力の prefix である。"""
    text = "kotohaime"
    for seed in range(20):
        rng = random.Random(seed)
        result = truncate_random(text, rng, min_prefix_len=2)
        assert text.startswith(result)
        assert len(result) >= 2


def test_generate_partials_returns_empty_when_count_is_zero_or_negative() -> None:
    """``count <= 0`` のとき戻り値は空 list。"""
    rng = random.Random(0)
    assert generate_partials("abcdef", rng, count=0) == []
    assert generate_partials("abcdef", rng, count=-3) == []


def test_generate_partials_returns_distinct_prefixes() -> None:
    """生成された partial 間には重複がない。"""
    rng = random.Random(42)
    partials = generate_partials("kotohaimeproject", rng, count=4, min_prefix_len=1)
    assert len(partials) == len(set(partials))
    assert all("kotohaimeproject".startswith(p) for p in partials)


def test_generate_partials_capped_by_available_unique_count() -> None:
    """distinct prefix の理論最大数を超えるとき、取得できた数だけ返す。"""
    rng = random.Random(0)
    # len("abc") = 3, min_prefix_len = 1 -> distinct prefix は 3 個 ("a", "ab", "abc")
    partials = generate_partials("abc", rng, count=10, min_prefix_len=1)
    assert len(partials) == 3
    assert set(partials) == {"a", "ab", "abc"}
