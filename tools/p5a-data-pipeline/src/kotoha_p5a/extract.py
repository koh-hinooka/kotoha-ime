"""Morphological extraction of (kanji_surface, kana_reading) pairs.

This module wraps :mod:`sudachipy` (Apache-2.0) and ``sudachidict_small``
(Apache-2.0) to avoid GPL dependencies such as :mod:`fugashi`. The
resulting pairs feed the romaji/typo pipeline.

A token is considered a kanji-bearing segment when its surface contains
at least one CJK unified ideograph in the range U+4E00..U+9FFF. Tokens
made purely of hiragana, katakana, ASCII, or punctuation are skipped:
the PoC model targets kanji conversion, so kana-only spans add no
training signal here.

The reading is taken from Sudachi's ``reading_form()``, which returns
katakana. This module converts the reading to hiragana before returning
it, so the downstream romaji module can use its hiragana-first table.
"""

from dataclasses import dataclass
from pathlib import Path
from typing import Final

from sudachipy import Dictionary, SplitMode, Tokenizer

_KANJI_START: Final[int] = 0x4E00
_KANJI_END: Final[int] = 0x9FFF
_KATAKANA_START: Final[int] = 0x30A1
_KATAKANA_END: Final[int] = 0x30FA  # includes ヴ etc.
_HIRAGANA_OFFSET: Final[int] = _KATAKANA_START - 0x3041


@dataclass(frozen=True, slots=True)
class KanjiPair:
    """A surface/reading pair extracted from a single token.

    Attributes:
        kanji: The original surface form containing at least one kanji.
        reading: The hiragana reading of ``kanji``.
    """

    kanji: str
    reading: str


def _contains_kanji(text: str) -> bool:
    return any(_KANJI_START <= ord(c) <= _KANJI_END for c in text)


def _katakana_to_hiragana(text: str) -> str:
    out: list[str] = []
    for ch in text:
        code = ord(ch)
        if _KATAKANA_START <= code <= _KATAKANA_END:
            out.append(chr(code - _HIRAGANA_OFFSET))
        else:
            out.append(ch)
    return "".join(out)


def make_tokenizer() -> Tokenizer:
    """Create a Sudachi tokenizer backed by ``sudachidict_small``.

    Returns:
        A ready-to-use :class:`sudachipy.Tokenizer` instance. Callers
        should reuse the returned tokenizer across many sentences because
        dictionary initialisation is expensive.
    """
    return Dictionary(dict="small").create()


def extract_pairs(sentence: str, tokenizer: Tokenizer) -> list[KanjiPair]:
    """Extract (kanji, hiragana-reading) pairs from a single sentence.

    Args:
        sentence: A single line of Japanese text (kanji / kana / ASCII).
        tokenizer: A Sudachi tokenizer obtained from
            :func:`make_tokenizer`. The tokenizer is called with
            :class:`sudachipy.SplitMode.C` (longest split) because the
            PoC prefers whole-word surfaces over aggressive sub-word
            splitting.

    Returns:
        A list of :class:`KanjiPair`. Tokens whose surface contains no
        kanji are skipped. Tokens whose reading is empty (rare Sudachi
        edge cases) are also skipped.
    """
    pairs: list[KanjiPair] = []
    for morpheme in tokenizer.tokenize(sentence, SplitMode.C):
        surface = morpheme.surface()
        if not _contains_kanji(surface):
            continue
        reading_kata = morpheme.reading_form()
        if not reading_kata:
            continue
        reading_hira = _katakana_to_hiragana(reading_kata)
        pairs.append(KanjiPair(kanji=surface, reading=reading_hira))
    return pairs


def iter_sentences(path: Path) -> list[str]:
    """Read non-blank lines from ``path`` as individual sentences.

    Args:
        path: File path to read (UTF-8). Blank lines and lines that
            consist solely of whitespace are dropped.

    Returns:
        A list of cleaned sentence strings in file order.
    """
    lines = path.read_text(encoding="utf-8").splitlines()
    return [line.strip() for line in lines if line.strip()]
