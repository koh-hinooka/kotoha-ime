"""Special tokens (PUA) for training-ready TSV format.

Karukan 踏襲の Private Use Area (PUA) id を初期候補として採用する
(ADR 0010 D4)。最終確定は Phase 5 kick-off のモデル学習検証後に行う。

採用する PUA code point (初期値):

- ``CTX_TOKEN``    : U+E002 (``<ctx>``)
- ``ROMAJI_TOKEN`` : U+E000 (``<romaji>``)
- ``OUT_TOKEN``    : U+E001 (``<out>``)
- ``EOS_TOKEN``    : U+E003 (``<eos>``)

PUA 領域 (U+E000..U+F8FF) は Unicode 標準が用途を定めない範囲であり、
通常のテキストに混入する可能性が低いため、学習データの tokenizer が
これらの文字を正しく学習用トークンとして分離しやすい。

Public API:
    - :func:`wrap_with_tokens`: romaji + kanji を PUA 包んで 1 つの
      training-ready 文字列を生成
    - :func:`unwrap`: 学習データ debug 用途の逆変換 (round-trip)
"""

from typing import Final, TypedDict

CTX_TOKEN: Final[str] = ""
ROMAJI_TOKEN: Final[str] = ""
OUT_TOKEN: Final[str] = ""
EOS_TOKEN: Final[str] = ""


class Unwrapped(TypedDict):
    """:func:`unwrap` の戻り値の typed dict 表現。

    Attributes:
        context: ``<ctx>`` 節の内容 (context が存在しないとき空文字列)。
        romaji: ``<romaji>`` 節の内容。
        output: ``<out>`` 節の内容 (末尾の ``<eos>`` は除いた本体のみ)。
    """

    context: str
    romaji: str
    output: str


def wrap_with_tokens(romaji: str, kanji: str, context: str = "") -> str:
    """romaji / kanji / context を PUA トークンで包んだ学習用文字列を返す。

    Format:
        ``[CTX_TOKEN ctx] ROMAJI_TOKEN romaji OUT_TOKEN kanji EOS_TOKEN``

    ``context`` が空文字列のときは CTX_TOKEN 節を省略する。

    Postconditions:
        - 戻り値には必ず ``ROMAJI_TOKEN``, ``OUT_TOKEN``, ``EOS_TOKEN`` が
          この順で含まれる。
        - 戻り値は ``EOS_TOKEN`` で終わる。
        - ``context`` が空文字列なら戻り値に ``CTX_TOKEN`` は現れない。

    Args:
        romaji: 入力側 romaji 文字列 (clean または noisy)。空文字列も許容。
        kanji: 出力側 kanji (正解) 文字列。空文字列も許容。
        context: 先行文脈などの補助情報。省略時は CTX 節なし。

    Returns:
        PUA トークンで包まれた単一文字列。
    """
    parts: list[str] = []
    if context:
        parts.append(f"{CTX_TOKEN}{context}")
    parts.append(f"{ROMAJI_TOKEN}{romaji}")
    parts.append(f"{OUT_TOKEN}{kanji}")
    parts.append(EOS_TOKEN)
    return "".join(parts)


def unwrap(wrapped: str) -> Unwrapped:
    """:func:`wrap_with_tokens` の戻り値を分解する (debug / test 用)。

    Preconditions:
        - ``wrapped`` に ``ROMAJI_TOKEN`` と ``OUT_TOKEN`` の両方を含む

    Postconditions:
        - 戻り値の ``romaji`` / ``output`` は空文字列になり得る
        - ``CTX_TOKEN`` が含まれないとき ``context`` は空文字列
        - ``EOS_TOKEN`` が含まれないとき ``output`` は OUT_TOKEN 以降の全体

    Args:
        wrapped: :func:`wrap_with_tokens` が生成した文字列。

    Returns:
        :class:`Unwrapped` (``context`` / ``romaji`` / ``output`` を持つ
        TypedDict)。

    Raises:
        ValueError: 必須トークン (``ROMAJI_TOKEN`` / ``OUT_TOKEN``) が
            欠落しているとき。
    """
    if ROMAJI_TOKEN not in wrapped or OUT_TOKEN not in wrapped:
        raise ValueError(f"wrapped missing required tokens: {wrapped!r}")
    # context 節を取得: CTX_TOKEN と ROMAJI_TOKEN の間
    ctx_body = ""
    if CTX_TOKEN in wrapped:
        after_ctx = wrapped.split(CTX_TOKEN, 1)[1]
        if ROMAJI_TOKEN in after_ctx:
            ctx_body = after_ctx.split(ROMAJI_TOKEN, 1)[0]
    # romaji 節を取得: ROMAJI_TOKEN と OUT_TOKEN の間
    after_romaji = wrapped.split(ROMAJI_TOKEN, 1)[1]
    romaji_body = after_romaji.split(OUT_TOKEN, 1)[0]
    # output 節を取得: OUT_TOKEN と EOS_TOKEN の間 (EOS_TOKEN が無ければ末尾まで)
    after_out = wrapped.split(OUT_TOKEN, 1)[1]
    if EOS_TOKEN in after_out:
        output_body = after_out.split(EOS_TOKEN, 1)[0]
    else:
        output_body = after_out
    return {"context": ctx_body, "romaji": romaji_body, "output": output_body}
