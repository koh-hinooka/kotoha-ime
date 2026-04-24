"""Tests for the self-written kana-to-romaji conversion table."""

import pytest

from kotoha_p5a.romaji import kana_to_romaji


@pytest.mark.parametrize(
    ("kana", "hepburn", "kunrei", "waapuro"),
    [
        ("し", "shi", "si", "shi"),
        ("つ", "tsu", "tu", "tu"),
        ("ふ", "fu", "hu", "hu"),
        ("ち", "chi", "ti", "ti"),
        ("じ", "ji", "zi", "ji"),
    ],
)
def test_single_kana_three_styles(kana: str, hepburn: str, kunrei: str, waapuro: str) -> None:
    assert kana_to_romaji(kana, "hepburn") == hepburn
    assert kana_to_romaji(kana, "kunrei") == kunrei
    assert kana_to_romaji(kana, "waapuro") == waapuro


@pytest.mark.parametrize(
    ("kana", "hepburn", "kunrei", "waapuro"),
    [
        ("しゃ", "sha", "sya", "sya"),
        ("ちゃ", "cha", "tya", "tya"),
        ("じゃ", "ja", "zya", "ja"),
        ("きゃ", "kya", "kya", "kya"),
    ],
)
def test_youon_digraphs(kana: str, hepburn: str, kunrei: str, waapuro: str) -> None:
    assert kana_to_romaji(kana, "hepburn") == hepburn
    assert kana_to_romaji(kana, "kunrei") == kunrei
    assert kana_to_romaji(kana, "waapuro") == waapuro


def test_sokuon_doubles_next_consonant() -> None:
    # きっぷ: sokuon doubles the 'p' in ぷ across all three styles.
    assert kana_to_romaji("きっぷ", "hepburn") == "kippu"
    assert kana_to_romaji("きっぷ", "kunrei") == "kippu"
    assert kana_to_romaji("きっぷ", "waapuro") == "kippu"


def test_sokuon_before_chi_hepburn_uses_tch() -> None:
    # Hepburn-specific rule: っち → tchi (not cchi).
    assert kana_to_romaji("まっちゃ", "hepburn") == "matcha"
    # Kunrei / waapuro do not use 'ch', so they double the 't'.
    assert kana_to_romaji("まっちゃ", "kunrei") == "mattya"
    assert kana_to_romaji("まっちゃ", "waapuro") == "mattya"


def test_hatsuon_and_particle_ha() -> None:
    # こんにちは: ん→n (all styles), は→ha (particle rendered as-written).
    result = kana_to_romaji("こんにちは", "waapuro")
    assert result == "konnitiha"
    hepburn_result = kana_to_romaji("こんにちは", "hepburn")
    assert hepburn_result == "konnichiha"


def test_katakana_long_vowel_doubled() -> None:
    # ASCII-only simplification: ー → repeat previous vowel.
    assert kana_to_romaji("コーヒー", "hepburn") == "koohii"
    assert kana_to_romaji("コーヒー", "waapuro") == "koohii"


def test_vu_row_katakana() -> None:
    assert kana_to_romaji("ヴァ", "hepburn") == "va"
    assert kana_to_romaji("ヴァイオリン", "hepburn") == "vaiorin"


def test_punctuation_is_passed_through() -> None:
    assert kana_to_romaji("あ、い。", "hepburn") == "a、i。"


def test_invalid_style_raises() -> None:
    with pytest.raises(ValueError, match="Unknown romaji style"):
        kana_to_romaji("あ", "invalid")  # type: ignore[arg-type]


def test_dangling_sokuon_at_end() -> None:
    """Sokuon at end-of-string falls back to ``tsu`` (Hepburn-like).

    The implementation cannot double a non-existent following consonant,
    so ``romaji.py`` emits the literal ``"tsu"`` for a dangling ``っ``.
    This test locks in that documented fallback behaviour.
    """
    # Sanity: normal ki stays ki.
    assert kana_to_romaji("き", "hepburn") == "ki"
    # Dangling trailing っ: "ki" + fallback "tsu" = "kitsu".
    assert kana_to_romaji("きっ", "hepburn") == "kitsu"
    assert kana_to_romaji("きっ", "kunrei") == "kitsu"
    assert kana_to_romaji("きっ", "waapuro") == "kitsu"


def test_sokuon_before_vowel() -> None:
    """Sokuon directly followed by a bare vowel emits a doubled vowel.

    ``っあ`` / ``っい`` etc. are not standard Japanese, but typos can
    produce them. The implementation takes the first character of the
    following romaji (here the vowel itself) and prepends it, yielding
    a deterministic doubled vowel without raising.
    """
    # っあ → "a" doubled = "aa".
    assert kana_to_romaji("っあ", "hepburn") == "aa"
    # っい → "i" doubled = "ii".
    assert kana_to_romaji("っい", "hepburn") == "ii"
    # Chain: きっあ → "ki" + "aa" = "kiaa".
    assert kana_to_romaji("きっあ", "hepburn") == "kiaa"
