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

    生成戦略:
        - ``max_unique <= count`` のとき: 全 prefix を exhaustive に列挙して
          返す (確定的)。random sampling の確率的失敗を回避する。
        - ``max_unique > count`` のとき: random sampling で distinct prefix
          を ``count`` 個取得する。

    Preconditions:
        - ``min_prefix_len >= 1``

    Postconditions:
        - 戻り値は重複のない prefix list
        - ``count <= 0`` のとき戻り値は空 list
        - 取り得る distinct な prefix 数が ``count`` 以下なら全列挙を返す
        - 取り得る distinct prefix が ``count`` より多い場合、
          random sampling で最大 ``count`` 個を返す

    Args:
        text: 元となる文字列。
        rng: 再現性のため外部から注入する乱数生成器。
        count: 生成する partial の最大数。``0`` 以下なら空 list を返す。
        min_prefix_len: prefix の最小長。既定は ``1``。

    Returns:
        distinct な prefix 文字列の list (入力から得られる順)。

    Raises:
        ValueError: ``min_prefix_len`` が ``1`` 未満のとき。
    """
    if count <= 0:
        return []
    if min_prefix_len < 1:
        raise ValueError(f"min_prefix_len must be >= 1, got {min_prefix_len}")
    # 取り得る distinct prefix 数 = len(text) - min_prefix_len + 1
    # (len(text) < min_prefix_len のとき max_unique=0 で target=0 になる)
    max_unique = max(0, len(text) - min_prefix_len + 1)
    target = min(count, max_unique)
    if target == 0:
        return []
    # max_unique が count 以下なら全 prefix を exhaustive に列挙 (確定的)
    # これにより短文入力での random sampling の確率的失敗を回避する
    if max_unique <= count:
        return [text[:i] for i in range(min_prefix_len, len(text) + 1)]
    # 十分長い text に対しては random sampling
    seen: set[str] = set()
    out: list[str] = []
    # 最大試行回数: count * 20。衝突が続いても無限 loop しないための保険
    attempts = 0
    max_attempts = count * 20
    while len(out) < target and attempts < max_attempts:
        prefix = truncate_random(text, rng, min_prefix_len)
        if prefix not in seen:
            seen.add(prefix)
            out.append(prefix)
        attempts += 1
    return out
