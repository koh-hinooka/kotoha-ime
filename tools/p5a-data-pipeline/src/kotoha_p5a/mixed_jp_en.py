"""Mixed JP/EN sentence generation (Phase 5 spec §4.7 PoC).

Phase 5 spec §4.7 および ADR 0010 D8 で規定された「日本語中に英単語を
含む混合文」の学習 corpus を PoC レベルで生成するモジュール。

設計方針:
    - EN 単語は programming context (例: ``commit``, ``pull request``) と
      一般 loanword (例: ``coffee``, ``meeting``) を hand-crafted list で
      保持する。
    - JP テンプレートには placeholder ``{en}`` と ``{en2}`` を 1 つずつ
      含め、EN 単語を 2 個挿入して混合文を合成する。
    - romaji 拡張時は EN 部をそのまま保持し、JP 部のみ Hepburn / Kunrei /
      waapuro で変換する。
    - typo 注入は PoC では文字列全体に対して適用するが、厳密な
      segment-aware 注入は Phase 5 kick-off で別途実装する。

Public API:
    - :func:`compose_mixed_sentence`: template + EN 単語 2 個から混合文を
      合成し segment 情報を返す。
    - :func:`expand_romaji_mixed`: 混合文を指定 style で romaji 化する。
    - :func:`inject_typos_mixed`: 混合 romaji 文字列に typo を注入する。
"""

import random
from typing import Final, Literal, TypedDict

from .romaji import RomajiStyle, kana_to_romaji
from .typo import inject_typos

# Programming 文脈の EN 単語
EN_WORDS_PROGRAMMING: Final[tuple[str, ...]] = (
    "commit",
    "pull request",
    "merge",
    "branch",
    "function",
    "cache",
    "debug",
    "deploy",
    "fork",
    "issue",
    "repository",
    "review",
    "test",
    "build",
    "release",
    "CI",
    "API",
    "database",
)

# 一般 loanword の EN 単語
EN_WORDS_GENERAL: Final[tuple[str, ...]] = (
    "coffee",
    "table",
    "restaurant",
    "computer",
    "internet",
    "email",
    "meeting",
    "schedule",
    "project",
    "member",
    "team",
    "update",
)

# JP テンプレート: かなのみで記述し、kanji は含めない
# (PoC の romaji 化を簡潔に保つため)。{en} / {en2} の各 placeholder は
# それぞれ 1 回だけ現れ、EN 単語の挿入位置を一意に定める。
JP_TEMPLATES: Final[tuple[str, ...]] = (
    "{en}をしゅうせいして{en2}をだして",
    "{en}のもどりちを{en2}にほぞんする",
    "{en}をかくにんしてから{en2}する",
    "つぎの{en}は{en2}たんとうです",
    "{en}のけっかを{en2}にきょうゆうする",
    "{en}はじゅうような{en2}である",
)


class Segment(TypedDict):
    """混合文中の 1 セグメントを表す typed dict。

    Attributes:
        kind: セグメント種別。``"jp"`` または ``"en"``。
        text: セグメントの文字列内容。
        start: ``composed`` 内での char 開始 offset (inclusive)。
        end: ``composed`` 内での char 終了 offset (exclusive)。
    """

    kind: Literal["jp", "en"]
    text: str
    start: int
    end: int


class MixedSentence(TypedDict):
    """合成された混合文と segment 情報を保持する typed dict。

    Attributes:
        composed: 合成済の混合文文字列。
        segments: JP / EN セグメントの list。
            ``composed`` 内を左から右に走査した順で格納される。
    """

    composed: str
    segments: list[Segment]


def compose_mixed_sentence(rng: random.Random) -> MixedSentence:
    """ランダムに template 1 個と EN 単語 2 個を選び混合文を合成する。

    Preconditions:
        - ``rng`` は :class:`random.Random` インスタンス

    Postconditions:
        - 戻り値の ``composed`` は非空文字列
        - 戻り値の ``segments`` 内の全セグメントは、
          連結すると ``composed`` に等しい
        - 各セグメントの ``start`` / ``end`` は
          ``composed`` の char offset と一致する

    Args:
        rng: 再現性のため外部から注入する乱数生成器。

    Returns:
        合成された :class:`MixedSentence`。EN 単語は 2 個 (重複可) 挿入される。
    """
    template = rng.choice(JP_TEMPLATES)
    pool = EN_WORDS_PROGRAMMING + EN_WORDS_GENERAL
    en_word_1 = rng.choice(pool)
    en_word_2 = rng.choice(pool)
    # {en} を en_word_1 で 1 回だけ、{en2} を en_word_2 で 1 回だけ置換する
    composed = template.replace("{en}", en_word_1, 1).replace("{en2}", en_word_2, 1)

    segments: list[Segment] = []
    cursor = 0
    for en_word in (en_word_1, en_word_2):
        # cursor 以降での最初の出現位置を取得 (同じ EN 単語が複数回含まれる
        # 場合でも、cursor 更新により左から右へ順序通り処理する)
        idx = composed.find(en_word, cursor)
        if idx < 0:
            # template 内の {en} / {en2} 個数が設計と一致しない異常系。
            # silent skip すると「segments を連結すると composed に等しい」
            # という postcondition が崩れるため、template 設計の誤りを
            # 早期に検出できるよう明示的に raise する。
            raise RuntimeError(
                f"EN word {en_word!r} not found in composed sentence "
                f"{composed!r} (cursor={cursor}). "
                "Template or EN word list design error."
            )
        if idx > cursor:
            segments.append(
                {
                    "kind": "jp",
                    "text": composed[cursor:idx],
                    "start": cursor,
                    "end": idx,
                }
            )
        segments.append(
            {
                "kind": "en",
                "text": en_word,
                "start": idx,
                "end": idx + len(en_word),
            }
        )
        cursor = idx + len(en_word)
    if cursor < len(composed):
        segments.append(
            {
                "kind": "jp",
                "text": composed[cursor:],
                "start": cursor,
                "end": len(composed),
            }
        )
    return {"composed": composed, "segments": segments}


def expand_romaji_mixed(sentence: MixedSentence, style: RomajiStyle) -> str:
    """混合文を指定 style で romaji 化する (EN 部はそのまま保持)。

    Preconditions:
        - ``sentence["segments"]`` の JP セグメントは hiragana / katakana
          のみで構成される (``JP_TEMPLATES`` の設計により kanji は含まれない)
        - ``style`` は :data:`~kotoha_p5a.romaji.ALL_STYLES` のいずれか

    Postconditions:
        - EN セグメントの text はそのまま出力に含まれる
        - JP セグメントの text は対応する romaji に変換される

    Args:
        sentence: :func:`compose_mixed_sentence` の戻り値。
        style: romaji 変換スタイル (``"hepburn"`` / ``"kunrei"`` /
            ``"waapuro"``)。

    Returns:
        romaji 化済の混合文字列。
    """
    parts: list[str] = []
    for seg in sentence["segments"]:
        if seg["kind"] == "jp":
            parts.append(kana_to_romaji(seg["text"], style))
        else:
            # EN セグメントは romaji 化せずそのまま出力
            parts.append(seg["text"])
    return "".join(parts)


def inject_typos_mixed(romaji: str, distance: int, seed: int) -> str:
    """混合 romaji 文字列全体に typo を注入する (PoC 簡略版)。

    厳密な segment 境界保持 (EN 単語を壊さない) は Phase 5 kick-off で
    実装する。PoC では :func:`~kotoha_p5a.typo.inject_typos` をそのまま
    呼び出し、segment awareness なく全体に注入する。

    Args:
        romaji: :func:`expand_romaji_mixed` が返す romaji 化済混合文字列。
        distance: 編集操作数 (0-3)。
        seed: 乱数シード。

    Returns:
        typo 注入後の文字列。

    Raises:
        ValueError: ``distance`` が負のとき (``inject_typos`` が raise)。
    """
    return inject_typos(romaji, distance, seed)
