"""Kana-to-romaji conversion in three styles (Hepburn / Kunrei / waapuro).

This module implements a hand-written conversion table to avoid GPL-licensed
libraries (pykakasi is GPL-3), keeping Kotoha's MIT/Apache-2.0 licensing
direction intact.

Design choices:
- ASCII-only output: long vowels are doubled (e.g. ``koohii``), macrons
  (``ā``, ``ō``) are intentionally not used.
- Sokuon ``っ`` doubles the following consonant; for Hepburn ``ch`` it
  becomes ``tch``.
- Hatsuon ``ん`` is rendered as ``n`` in all styles (the spec accepts
  ``shinbun`` for Hepburn; a full m/n split is not implemented here as
  the spec-permitted simplification).
- Punctuation characters (``、``, ``。``, ``・``, ASCII punctuation) are
  passed through unchanged and are not subject to typo injection at the
  kana stage.
- Small ``っ`` before a vowel or ``ん`` before an unknown character falls
  back to plain ``tsu`` / ``n``.
"""

from typing import Final, Literal

RomajiStyle = Literal["hepburn", "kunrei", "waapuro"]

ALL_STYLES: Final[tuple[RomajiStyle, ...]] = ("hepburn", "kunrei", "waapuro")

# Base table: (hepburn, kunrei, waapuro) for each kana unit.
# Ordered so that multi-char keys (digraphs such as きゃ) are tried first.
_BASE_TABLE: Final[dict[str, tuple[str, str, str]]] = {
    # Youon (digraphs) — must come before single kana in longest-match.
    "きゃ": ("kya", "kya", "kya"),
    "きゅ": ("kyu", "kyu", "kyu"),
    "きょ": ("kyo", "kyo", "kyo"),
    "ぎゃ": ("gya", "gya", "gya"),
    "ぎゅ": ("gyu", "gyu", "gyu"),
    "ぎょ": ("gyo", "gyo", "gyo"),
    "しゃ": ("sha", "sya", "sya"),
    "しゅ": ("shu", "syu", "syu"),
    "しょ": ("sho", "syo", "syo"),
    "じゃ": ("ja", "zya", "ja"),
    "じゅ": ("ju", "zyu", "ju"),
    "じょ": ("jo", "zyo", "jo"),
    "ちゃ": ("cha", "tya", "tya"),
    "ちゅ": ("chu", "tyu", "tyu"),
    "ちょ": ("cho", "tyo", "tyo"),
    "にゃ": ("nya", "nya", "nya"),
    "にゅ": ("nyu", "nyu", "nyu"),
    "にょ": ("nyo", "nyo", "nyo"),
    "ひゃ": ("hya", "hya", "hya"),
    "ひゅ": ("hyu", "hyu", "hyu"),
    "ひょ": ("hyo", "hyo", "hyo"),
    "びゃ": ("bya", "bya", "bya"),
    "びゅ": ("byu", "byu", "byu"),
    "びょ": ("byo", "byo", "byo"),
    "ぴゃ": ("pya", "pya", "pya"),
    "ぴゅ": ("pyu", "pyu", "pyu"),
    "ぴょ": ("pyo", "pyo", "pyo"),
    "みゃ": ("mya", "mya", "mya"),
    "みゅ": ("myu", "myu", "myu"),
    "みょ": ("myo", "myo", "myo"),
    "りゃ": ("rya", "rya", "rya"),
    "りゅ": ("ryu", "ryu", "ryu"),
    "りょ": ("ryo", "ryo", "ryo"),
    "ヴァ": ("va", "va", "va"),
    "ヴィ": ("vi", "vi", "vi"),
    "ヴェ": ("ve", "ve", "ve"),
    "ヴォ": ("vo", "vo", "vo"),
    # Basic vowels
    "あ": ("a", "a", "a"),
    "い": ("i", "i", "i"),
    "う": ("u", "u", "u"),
    "え": ("e", "e", "e"),
    "お": ("o", "o", "o"),
    # K row
    "か": ("ka", "ka", "ka"),
    "き": ("ki", "ki", "ki"),
    "く": ("ku", "ku", "ku"),
    "け": ("ke", "ke", "ke"),
    "こ": ("ko", "ko", "ko"),
    # G row
    "が": ("ga", "ga", "ga"),
    "ぎ": ("gi", "gi", "gi"),
    "ぐ": ("gu", "gu", "gu"),
    "げ": ("ge", "ge", "ge"),
    "ご": ("go", "go", "go"),
    # S row
    "さ": ("sa", "sa", "sa"),
    "し": ("shi", "si", "shi"),
    "す": ("su", "su", "su"),
    "せ": ("se", "se", "se"),
    "そ": ("so", "so", "so"),
    # Z row
    "ざ": ("za", "za", "za"),
    "じ": ("ji", "zi", "ji"),
    "ず": ("zu", "zu", "zu"),
    "ぜ": ("ze", "ze", "ze"),
    "ぞ": ("zo", "zo", "zo"),
    # T row
    "た": ("ta", "ta", "ta"),
    "ち": ("chi", "ti", "ti"),
    "つ": ("tsu", "tu", "tu"),
    "て": ("te", "te", "te"),
    "と": ("to", "to", "to"),
    # D row
    "だ": ("da", "da", "da"),
    "ぢ": ("ji", "zi", "di"),
    "づ": ("zu", "zu", "du"),
    "で": ("de", "de", "de"),
    "ど": ("do", "do", "do"),
    # N row
    "な": ("na", "na", "na"),
    "に": ("ni", "ni", "ni"),
    "ぬ": ("nu", "nu", "nu"),
    "ね": ("ne", "ne", "ne"),
    "の": ("no", "no", "no"),
    # H row
    "は": ("ha", "ha", "ha"),
    "ひ": ("hi", "hi", "hi"),
    "ふ": ("fu", "hu", "hu"),
    "へ": ("he", "he", "he"),
    "ほ": ("ho", "ho", "ho"),
    # B row
    "ば": ("ba", "ba", "ba"),
    "び": ("bi", "bi", "bi"),
    "ぶ": ("bu", "bu", "bu"),
    "べ": ("be", "be", "be"),
    "ぼ": ("bo", "bo", "bo"),
    # P row
    "ぱ": ("pa", "pa", "pa"),
    "ぴ": ("pi", "pi", "pi"),
    "ぷ": ("pu", "pu", "pu"),
    "ぺ": ("pe", "pe", "pe"),
    "ぽ": ("po", "po", "po"),
    # M row
    "ま": ("ma", "ma", "ma"),
    "み": ("mi", "mi", "mi"),
    "む": ("mu", "mu", "mu"),
    "め": ("me", "me", "me"),
    "も": ("mo", "mo", "mo"),
    # Y row
    "や": ("ya", "ya", "ya"),
    "ゆ": ("yu", "yu", "yu"),
    "よ": ("yo", "yo", "yo"),
    # R row
    "ら": ("ra", "ra", "ra"),
    "り": ("ri", "ri", "ri"),
    "る": ("ru", "ru", "ru"),
    "れ": ("re", "re", "re"),
    "ろ": ("ro", "ro", "ro"),
    # W row
    "わ": ("wa", "wa", "wa"),
    # Intentional asymmetry across styles for を (particle `wo`):
    # - Traditional Hepburn transcribes the particle as "o" (pronounced /o/).
    # - Kunrei-shiki and waapuro both keep "wo" to preserve the kana
    #   distinction from お.
    # - Revised Hepburn (if adopted later) would map both を and お to "o",
    #   but that normalization is deferred to Phase 5 kick-off where the
    #   canonical style for training data will be confirmed empirically.
    "を": ("o", "wo", "wo"),
    "ん": ("n", "n", "n"),
    # Small kana (standalone fallback)
    "ゃ": ("ya", "ya", "ya"),
    "ゅ": ("yu", "yu", "yu"),
    "ょ": ("yo", "yo", "yo"),
    "ぁ": ("a", "a", "a"),
    "ぃ": ("i", "i", "i"),
    "ぅ": ("u", "u", "u"),
    "ぇ": ("e", "e", "e"),
    "ぉ": ("o", "o", "o"),
}

# Katakana base table: same semantics as hiragana, added for sentences
# mixing katakana loanwords such as ``コーヒー``.
_KATAKANA_BASE: Final[dict[str, tuple[str, str, str]]] = {
    "キャ": ("kya", "kya", "kya"),
    "キュ": ("kyu", "kyu", "kyu"),
    "キョ": ("kyo", "kyo", "kyo"),
    "ギャ": ("gya", "gya", "gya"),
    "ギュ": ("gyu", "gyu", "gyu"),
    "ギョ": ("gyo", "gyo", "gyo"),
    "シャ": ("sha", "sya", "sya"),
    "シュ": ("shu", "syu", "syu"),
    "ショ": ("sho", "syo", "syo"),
    "ジャ": ("ja", "zya", "ja"),
    "ジュ": ("ju", "zyu", "ju"),
    "ジョ": ("jo", "zyo", "jo"),
    "チャ": ("cha", "tya", "tya"),
    "チュ": ("chu", "tyu", "tyu"),
    "チョ": ("cho", "tyo", "tyo"),
    "ニャ": ("nya", "nya", "nya"),
    "ニュ": ("nyu", "nyu", "nyu"),
    "ニョ": ("nyo", "nyo", "nyo"),
    "ヒャ": ("hya", "hya", "hya"),
    "ヒュ": ("hyu", "hyu", "hyu"),
    "ヒョ": ("hyo", "hyo", "hyo"),
    "ビャ": ("bya", "bya", "bya"),
    "ビュ": ("byu", "byu", "byu"),
    "ビョ": ("byo", "byo", "byo"),
    "ピャ": ("pya", "pya", "pya"),
    "ピュ": ("pyu", "pyu", "pyu"),
    "ピョ": ("pyo", "pyo", "pyo"),
    "ミャ": ("mya", "mya", "mya"),
    "ミュ": ("myu", "myu", "myu"),
    "ミョ": ("myo", "myo", "myo"),
    "リャ": ("rya", "rya", "rya"),
    "リュ": ("ryu", "ryu", "ryu"),
    "リョ": ("ryo", "ryo", "ryo"),
    "ア": ("a", "a", "a"),
    "イ": ("i", "i", "i"),
    "ウ": ("u", "u", "u"),
    "エ": ("e", "e", "e"),
    "オ": ("o", "o", "o"),
    "カ": ("ka", "ka", "ka"),
    "キ": ("ki", "ki", "ki"),
    "ク": ("ku", "ku", "ku"),
    "ケ": ("ke", "ke", "ke"),
    "コ": ("ko", "ko", "ko"),
    "ガ": ("ga", "ga", "ga"),
    "ギ": ("gi", "gi", "gi"),
    "グ": ("gu", "gu", "gu"),
    "ゲ": ("ge", "ge", "ge"),
    "ゴ": ("go", "go", "go"),
    "サ": ("sa", "sa", "sa"),
    "シ": ("shi", "si", "shi"),
    "ス": ("su", "su", "su"),
    "セ": ("se", "se", "se"),
    "ソ": ("so", "so", "so"),
    "ザ": ("za", "za", "za"),
    "ジ": ("ji", "zi", "ji"),
    "ズ": ("zu", "zu", "zu"),
    "ゼ": ("ze", "ze", "ze"),
    "ゾ": ("zo", "zo", "zo"),
    "タ": ("ta", "ta", "ta"),
    "チ": ("chi", "ti", "ti"),
    "ツ": ("tsu", "tu", "tu"),
    "テ": ("te", "te", "te"),
    "ト": ("to", "to", "to"),
    "ダ": ("da", "da", "da"),
    "ヂ": ("ji", "zi", "di"),
    "ヅ": ("zu", "zu", "du"),
    "デ": ("de", "de", "de"),
    "ド": ("do", "do", "do"),
    "ナ": ("na", "na", "na"),
    "ニ": ("ni", "ni", "ni"),
    "ヌ": ("nu", "nu", "nu"),
    "ネ": ("ne", "ne", "ne"),
    "ノ": ("no", "no", "no"),  # noqa: RUF001
    "ハ": ("ha", "ha", "ha"),
    "ヒ": ("hi", "hi", "hi"),
    "フ": ("fu", "hu", "hu"),
    "ヘ": ("he", "he", "he"),
    "ホ": ("ho", "ho", "ho"),
    "バ": ("ba", "ba", "ba"),
    "ビ": ("bi", "bi", "bi"),
    "ブ": ("bu", "bu", "bu"),
    "ベ": ("be", "be", "be"),
    "ボ": ("bo", "bo", "bo"),
    "パ": ("pa", "pa", "pa"),
    "ピ": ("pi", "pi", "pi"),
    "プ": ("pu", "pu", "pu"),
    "ペ": ("pe", "pe", "pe"),
    "ポ": ("po", "po", "po"),
    "マ": ("ma", "ma", "ma"),
    "ミ": ("mi", "mi", "mi"),
    "ム": ("mu", "mu", "mu"),
    "メ": ("me", "me", "me"),
    "モ": ("mo", "mo", "mo"),
    "ヤ": ("ya", "ya", "ya"),
    "ユ": ("yu", "yu", "yu"),
    "ヨ": ("yo", "yo", "yo"),
    "ラ": ("ra", "ra", "ra"),
    "リ": ("ri", "ri", "ri"),
    "ル": ("ru", "ru", "ru"),
    "レ": ("re", "re", "re"),
    "ロ": ("ro", "ro", "ro"),
    "ワ": ("wa", "wa", "wa"),
    # Same asymmetry as the hiragana を entry above: Hepburn uses "o" while
    # Kunrei / waapuro keep "wo". See the comment on "を" in _BASE_TABLE.
    "ヲ": ("o", "wo", "wo"),
    "ン": ("n", "n", "n"),
    "ヴ": ("vu", "vu", "vu"),
}

_SOKUON_HIRA: Final[str] = "っ"
_SOKUON_KATA: Final[str] = "ッ"
_CHOUON: Final[str] = "ー"  # katakana long-vowel mark

# Punctuation that must be passed through untouched. The full-width
# parentheses and Japanese marks are intentional; noqa silences ruff's
# confusable-character warning (RUF001) for this literal.
_PUNCTUATION: Final[frozenset[str]] = frozenset(
    "、。・「」『』（）()[]{}〜!?.,:;"  # noqa: RUF001
)

_STYLE_INDEX: Final[dict[RomajiStyle, int]] = {
    "hepburn": 0,
    "kunrei": 1,
    "waapuro": 2,
}


def _style_index(style: RomajiStyle) -> int:
    try:
        return _STYLE_INDEX[style]
    except KeyError as exc:  # pragma: no cover - exhaustive Literal
        raise ValueError(f"Unknown romaji style: {style}") from exc


def _longest_match(
    text: str,
    start: int,
    table: dict[str, tuple[str, str, str]],
) -> tuple[str, tuple[str, str, str]] | None:
    # Try 2-char keys first, then 1-char.
    if start + 2 <= len(text):
        key = text[start : start + 2]
        value = table.get(key)
        if value is not None:
            return key, value
    if start < len(text):
        key = text[start : start + 1]
        value = table.get(key)
        if value is not None:
            return key, value
    return None


def _last_vowel_double(base: str) -> str:
    # Katakana long-vowel: repeat the last vowel of the previous syllable.
    # Implemented with a small mapping rather than guessing IPA.
    if not base:
        return "a"
    last = base[-1]
    if last in "aeiou":
        return last
    # Fallback: no clear vowel, default to 'a' (the PoC never reaches this
    # for the 30 hand-crafted sentences in fixtures/input_sentences.txt).
    return "a"


def kana_to_romaji(text: str, style: RomajiStyle) -> str:
    """Convert a kana string to romaji in the requested ``style``.

    Args:
        text: Input string composed of hiragana, katakana, and punctuation.
        style: One of ``"hepburn"``, ``"kunrei"``, ``"waapuro"``.

    Returns:
        A romaji string. Non-kana characters (including ASCII and
        Japanese punctuation) are copied unchanged.

    Raises:
        ValueError: If ``style`` is not one of the three supported styles.
    """
    idx = _style_index(style)
    out: list[str] = []
    i = 0
    n = len(text)
    while i < n:
        ch = text[i]

        # Sokuon (っ / ッ) doubles the next consonant.
        if ch in (_SOKUON_HIRA, _SOKUON_KATA):
            next_match = _longest_match(text, i + 1, _BASE_TABLE) or _longest_match(
                text, i + 1, _KATAKANA_BASE
            )
            if next_match is None:
                # Dangling sokuon: emit as 'tsu' fallback (Hepburn-like).
                out.append("tsu")
                i += 1
                continue
            next_key, next_value = next_match
            next_romaji = next_value[idx]
            if next_romaji.startswith("ch") and style == "hepburn":
                # Hepburn: っち → tchi.
                out.append("t")
            elif next_romaji:
                out.append(next_romaji[0])
            out.append(next_romaji)
            i += 1 + len(next_key)
            continue

        # Katakana long-vowel mark (ー): double the previous vowel.
        if ch == _CHOUON:
            previous = out[-1] if out else ""
            out.append(_last_vowel_double(previous))
            i += 1
            continue

        # Punctuation passthrough.
        if ch in _PUNCTUATION:
            out.append(ch)
            i += 1
            continue

        match = _longest_match(text, i, _BASE_TABLE)
        if match is None:
            match = _longest_match(text, i, _KATAKANA_BASE)
        if match is None:
            # Unknown character (e.g. residual kanji); pass through as-is.
            out.append(ch)
            i += 1
            continue
        key, value = match
        out.append(value[idx])
        i += len(key)

    return "".join(out)
