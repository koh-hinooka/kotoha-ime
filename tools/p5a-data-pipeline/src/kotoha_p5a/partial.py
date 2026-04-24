"""Partial-input generation (random prefix truncation).

Phase 5 spec §4.4 の partial-input 生成を PoC レベルで実装するモジュール。
random truncation による部分入力の再現のみを対象とし、モデル評価用途の
厳密な bias sampling や token-aware truncation は Phase 5 kick-off での
詳細化に委ねる。

Public API:
    - :func:`truncate_random`: 単一 prefix を切り出すヘルパ
    - :func:`generate_partials`: 複数の distinct prefix を生成するヘルパ

いずれも :class:`random.Random` を明示的に引数で受け取り、シード固定で
出力の再現性を保証する。
"""

import random


def truncate_random(text: str, rng: random.Random, min_prefix_len: int = 1) -> str:
    """``text`` の先頭から長さ ``>= min_prefix_len`` のランダム prefix を返す。

    Preconditions:
        - ``min_prefix_len >= 1``

    Postconditions:
        - 戻り値は ``text`` の prefix であり、長さは ``min_prefix_len`` 以上
          ``len(text)`` 以下
        - ``len(text) <= min_prefix_len`` のとき、戻り値は ``text`` そのもの

    Args:
        text: 元となる文字列。空文字列は ``min_prefix_len >= 1`` と矛盾する
            (長さ 0 は ``min_prefix_len=1`` より小さい) が、関数は
            ``len(text) <= min_prefix_len`` 判定により ``text`` をそのまま返す。
        rng: 再現性のため外部から注入する乱数生成器。
        min_prefix_len: prefix の最小長。既定は ``1``。

    Returns:
        ``text`` の先頭からの prefix 文字列。

    Raises:
        ValueError: ``min_prefix_len`` が ``1`` 未満のとき。
    """
    if min_prefix_len < 1:
        raise ValueError(f"min_prefix_len must be >= 1, got {min_prefix_len}")
    if len(text) <= min_prefix_len:
        return text
    cut = rng.randint(min_prefix_len, len(text))
    return text[:cut]


def generate_partials(
    text: str,
    rng: random.Random,
    count: int = 3,
    min_prefix_len: int = 1,
) -> list[str]:
    """``text`` から distinct な prefix を最大 ``count`` 個生成する。

    Preconditions:
        - ``min_prefix_len >= 1`` (``truncate_random`` 経由で検証)

    Postconditions:
        - 戻り値は重複のない prefix list
        - ``count <= 0`` のとき戻り値は空 list
        - 取り得る distinct な prefix 数が ``count`` より少ない場合、
          取得できた数だけを返す (sampling 失敗でブロックしない)

    Args:
        text: 元となる文字列。
        rng: 再現性のため外部から注入する乱数生成器。
        count: 生成する partial の最大数。``0`` 以下なら空 list を返す。
        min_prefix_len: prefix の最小長。既定は ``1``。

    Returns:
        distinct な prefix 文字列の list (入力から得られる順)。

    Raises:
        ValueError: ``min_prefix_len`` が ``1`` 未満のとき
            (``truncate_random`` が raise)。
    """
    if count <= 0:
        return []
    # 取り得る distinct prefix 数 = len(text) - min_prefix_len + 1
    # (len(text) < min_prefix_len のとき max_unique=0 で target=0 になる)
    max_unique = max(0, len(text) - min_prefix_len + 1)
    target = min(count, max_unique)
    seen: set[str] = set()
    out: list[str] = []
    # 最大試行回数: count * 10。確率的に衝突が続いても無限 loop しないため
    attempts = 0
    max_attempts = count * 10
    while len(out) < target and attempts < max_attempts:
        prefix = truncate_random(text, rng, min_prefix_len)
        if prefix not in seen:
            seen.add(prefix)
            out.append(prefix)
        attempts += 1
    return out
