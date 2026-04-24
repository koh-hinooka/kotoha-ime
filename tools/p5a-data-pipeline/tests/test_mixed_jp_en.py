"""Tests for mixed JP/EN sentence generation."""

import random
import string

from kotoha_p5a.mixed_jp_en import (
    EN_WORDS_GENERAL,
    EN_WORDS_PROGRAMMING,
    compose_mixed_sentence,
    expand_romaji_mixed,
    inject_typos_mixed,
)


def test_compose_is_reproducible_with_same_seed() -> None:
    """同一シードで構築した RNG から合成した文は bit 単位で一致する。"""
    rng_a = random.Random(42)
    rng_b = random.Random(42)
    a = compose_mixed_sentence(rng_a)
    b = compose_mixed_sentence(rng_b)
    assert a == b


def test_segments_cover_composed_without_gaps() -> None:
    """segments を連結すると composed に完全一致する。"""
    rng = random.Random(7)
    sentence = compose_mixed_sentence(rng)
    reconstructed = "".join(seg["text"] for seg in sentence["segments"])
    assert reconstructed == sentence["composed"]


def test_segment_offsets_match_composed_indices() -> None:
    """各 segment の ``start`` / ``end`` offset が ``composed`` の索引と一致する。"""
    rng = random.Random(13)
    sentence = compose_mixed_sentence(rng)
    for seg in sentence["segments"]:
        assert sentence["composed"][seg["start"] : seg["end"]] == seg["text"]


def test_en_segments_contain_known_en_words() -> None:
    """``kind == 'en'`` のセグメントの text は EN 単語 list に含まれる。"""
    known = set(EN_WORDS_PROGRAMMING) | set(EN_WORDS_GENERAL)
    for seed in range(10):
        rng = random.Random(seed)
        sentence = compose_mixed_sentence(rng)
        en_segments = [s for s in sentence["segments"] if s["kind"] == "en"]
        assert len(en_segments) >= 1
        for seg in en_segments:
            assert seg["text"] in known


def test_expand_romaji_preserves_en_segments_and_lowercases_jp() -> None:
    """romaji 化で EN 部は不変、JP 部は ASCII 小文字のみに変換される。"""
    rng = random.Random(3)
    sentence = compose_mixed_sentence(rng)
    romaji = expand_romaji_mixed(sentence, "hepburn")
    # EN 部の文字列が output にそのまま含まれる
    for seg in sentence["segments"]:
        if seg["kind"] == "en":
            assert seg["text"] in romaji
    # JP 部は romaji 化すると ASCII 小文字のみ (記号や空白を除く)
    allowed_jp_chars = set(string.ascii_lowercase)
    for seg in sentence["segments"]:
        if seg["kind"] == "jp":
            jp_romaji = "".join(c for c in romaji if c in allowed_jp_chars)
            # JP 部が romaji 化された結果として、非 ASCII 文字が残らない
            # (JP_TEMPLATES はかなのみを含む設計)
            for ch in seg["text"]:
                # seg["text"] は kana なので、romaji 出力には含まれない
                assert ch not in romaji
            # sanity: jp_romaji は非空
            assert len(jp_romaji) > 0
            break


def test_inject_typos_mixed_is_reproducible() -> None:
    """``inject_typos_mixed`` は同一 seed で再現する。"""
    romaji = "commitwoshuuseishitepullrequestwodashite"
    a = inject_typos_mixed(romaji, distance=2, seed=42)
    b = inject_typos_mixed(romaji, distance=2, seed=42)
    assert a == b


def test_inject_typos_mixed_distance_zero_returns_input() -> None:
    """distance=0 では文字列が変わらない。"""
    romaji = "commitwoshuuseishite"
    assert inject_typos_mixed(romaji, distance=0, seed=1) == romaji
