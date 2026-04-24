"""Tests for PUA-based special-token wrapping."""

import pytest

from kotoha_p5a.tokens import (
    CTX_TOKEN,
    EOS_TOKEN,
    OUT_TOKEN,
    ROMAJI_TOKEN,
    unwrap,
    wrap_with_tokens,
)


def test_pua_code_points_match_spec() -> None:
    """PUA トークンの code point は ADR 0010 D4 / Phase 5 spec §4.5 の初期値と一致する。"""
    assert ord(ROMAJI_TOKEN) == 0xEE00
    assert ord(OUT_TOKEN) == 0xEE01
    assert ord(CTX_TOKEN) == 0xEE02
    assert ord(EOS_TOKEN) == 0xEE03


def test_wrap_without_context_omits_ctx_token() -> None:
    """``context`` が空文字列のとき、戻り値に ``CTX_TOKEN`` は現れない。"""
    wrapped = wrap_with_tokens("koohii", "コーヒー")
    assert CTX_TOKEN not in wrapped
    assert ROMAJI_TOKEN in wrapped
    assert OUT_TOKEN in wrapped
    assert wrapped.endswith(EOS_TOKEN)


def test_wrap_with_context_includes_ctx_token() -> None:
    """``context`` が非空のとき、戻り値は ``CTX_TOKEN`` 節を含む。"""
    wrapped = wrap_with_tokens("kyou", "今日", context="senko")
    assert CTX_TOKEN in wrapped
    assert wrapped.index(CTX_TOKEN) < wrapped.index(ROMAJI_TOKEN)


def test_wrap_unwrap_round_trip_without_context() -> None:
    """context なしの wrap / unwrap は元の値を復元する。"""
    wrapped = wrap_with_tokens("koohii", "コーヒー")
    result = unwrap(wrapped)
    assert result == {"context": "", "romaji": "koohii", "output": "コーヒー"}


def test_wrap_unwrap_round_trip_with_context() -> None:
    """context ありの wrap / unwrap は元の値を復元する。"""
    wrapped = wrap_with_tokens("kyou", "今日", context="先行文脈")
    result = unwrap(wrapped)
    assert result == {"context": "先行文脈", "romaji": "kyou", "output": "今日"}


def test_wrap_accepts_empty_romaji_and_kanji() -> None:
    """空文字列 romaji / kanji も許容し、round-trip も保たれる。"""
    wrapped = wrap_with_tokens("", "")
    result = unwrap(wrapped)
    assert result == {"context": "", "romaji": "", "output": ""}


def test_unwrap_raises_on_missing_required_tokens() -> None:
    """必須トークン (``ROMAJI_TOKEN`` / ``OUT_TOKEN``) 欠落で ValueError。"""
    # ROMAJI_TOKEN なし
    with pytest.raises(ValueError, match="missing required tokens"):
        unwrap(f"{OUT_TOKEN}kanji{EOS_TOKEN}")
    # OUT_TOKEN なし
    with pytest.raises(ValueError, match="missing required tokens"):
        unwrap(f"{ROMAJI_TOKEN}romaji{EOS_TOKEN}")
