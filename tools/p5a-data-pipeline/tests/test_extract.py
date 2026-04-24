"""Tests for sudachipy-backed kanji/reading extraction."""

from pathlib import Path

import pytest
from sudachipy import Tokenizer

from kotoha_p5a.extract import (
    KanjiPair,
    extract_pairs,
    iter_sentences,
    make_tokenizer,
)


@pytest.fixture(scope="module")
def tokenizer() -> Tokenizer:
    # Sudachi dictionary init is expensive (~100ms); share across tests.
    return make_tokenizer()


def test_extract_kanji_bearing_tokens(tokenizer: Tokenizer) -> None:
    # "今日は晴れです" should expose 今日 and 晴れ as kanji-bearing pairs
    # (surface contains kanji); "は" and "です" must be skipped.
    pairs = extract_pairs("今日は晴れです", tokenizer)
    surfaces = [p.kanji for p in pairs]
    assert "今日" in surfaces
    # 晴れ or 晴 may appear depending on Sudachi split mode; assert the
    # presence of a kanji containing 晴 rather than an exact form.
    assert any("晴" in s for s in surfaces)
    # No kana-only token should have leaked through.
    assert all(any("一" <= c <= "鿿" for c in p.kanji) for p in pairs)


def test_extract_reading_is_hiragana(tokenizer: Tokenizer) -> None:
    pairs = extract_pairs("日本語を学ぶ", tokenizer)
    assert pairs, "at least one kanji-bearing pair expected"
    for p in pairs:
        for ch in p.reading:
            # Hiragana range: U+3041..U+309F.
            assert "ぁ" <= ch <= "ゟ", f"non-hiragana char {ch!r} in reading"


def test_sentence_without_kanji_returns_empty(tokenizer: Tokenizer) -> None:
    pairs = extract_pairs("ひらがなだけのぶんしょう", tokenizer)
    assert pairs == []


def test_iter_sentences_skips_blank_lines(tmp_path: Path) -> None:
    path = tmp_path / "sample.txt"
    path.write_text("今日は晴れです\n\n  \n日本語を学ぶ\n", encoding="utf-8")
    assert iter_sentences(path) == ["今日は晴れです", "日本語を学ぶ"]


def test_kanji_pair_is_frozen() -> None:
    pair = KanjiPair(kanji="今日", reading="きょう")
    with pytest.raises(AttributeError):
        pair.kanji = "明日"  # type: ignore[misc]
