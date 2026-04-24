"""CLI orchestrator for the Phase 5 P5-A data pipeline PoC.

Reads Japanese sentences from ``--input``, extracts (kanji, reading)
pairs via :mod:`kotoha_p5a.extract`, converts each reading to three
romaji styles via :mod:`kotoha_p5a.romaji`, and emits a 10-column TSV
covering clean / noisy romaji, partial truncations, PUA-wrapped
training-ready strings, and mixed JP/EN rows.

Example:
    uv run -m kotoha_p5a \\
        --input fixtures/input_sentences.txt \\
        --output fixtures/sample.tsv \\
        --seed 42 \\
        --with-partials 2 --with-tokens --with-mixed-jpen 5
"""

import argparse
import csv
import random
import sys
from pathlib import Path
from typing import Any, Final

from kotoha_p5a.extract import extract_pairs, iter_sentences, make_tokenizer
from kotoha_p5a.mixed_jp_en import (
    compose_mixed_sentence,
    expand_romaji_mixed,
    inject_typos_mixed,
)
from kotoha_p5a.partial import generate_partials
from kotoha_p5a.romaji import ALL_STYLES, kana_to_romaji
from kotoha_p5a.tokens import wrap_with_tokens
from kotoha_p5a.typo import inject_typos

TSV_HEADER: Final[tuple[str, ...]] = (
    "noisy_romaji",
    "kanji",
    "clean_romaji",
    "typo_distance",
    "romaji_style",
    "is_partial",
    "partial_len",
    "tokenized",
    "language_segments",
    "has_en_words",
)

_DISTANCES: Final[tuple[int, ...]] = (0, 1, 2, 3)

# Mixed JP/EN 拡張で採用する typo distance (scope 制限のため 0/1/2 のみ)。
# 各 mixed sentence は 3 styles x 3 distances = 9 rows を生成する。
_MIXED_DISTANCES: Final[tuple[int, ...]] = (0, 1, 2)


def _parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        prog="kotoha_p5a",
        description=(
            "Phase 5 P5-A data pipeline PoC: "
            "extract / romaji / typo / partial / tokens / mixed JP-EN."
        ),
    )
    parser.add_argument(
        "--input",
        type=Path,
        required=True,
        help="Path to a UTF-8 text file with one Japanese sentence per line.",
    )
    parser.add_argument(
        "--output",
        type=Path,
        required=True,
        help="Path to write the TSV output.",
    )
    parser.add_argument(
        "--seed",
        type=int,
        default=42,
        help="Base seed used to derive per-row typo seeds (default: 42).",
    )
    parser.add_argument(
        "--with-partials",
        type=int,
        default=0,
        metavar="N",
        help=(
            "Generate N partial (truncated) prefixes per clean romaji. "
            "0 disables partial generation (default)."
        ),
    )
    parser.add_argument(
        "--with-tokens",
        action="store_true",
        help="Populate the `tokenized` column with the PUA-wrapped training-ready format.",
    )
    parser.add_argument(
        "--with-mixed-jpen",
        type=int,
        default=0,
        metavar="N",
        help=(
            "Append N mixed JP/EN sentences (3 styles x 3 distances each). "
            "0 disables mixed generation (default)."
        ),
    )
    return parser.parse_args(argv)


def _row_seed(base_seed: int, row_index: int) -> int:
    """入力 base_seed と row_index から決定論的な per-row seed を導出する。

    同じ ``(input, base_seed)`` の組に対して常に同一の出力 TSV を生成するため、
    row_index を mix-in する。
    """
    return (base_seed * 1_000_003 + row_index) & 0xFFFFFFFF


def _write_row(
    writer: Any,
    *,
    noisy_romaji: str,
    kanji: str,
    clean_romaji: str,
    typo_distance: int,
    romaji_style: str,
    is_partial: int = 0,
    partial_len: int = 0,
    with_tokens: bool,
    language_segments: str = "",
    has_en_words: int = 0,
) -> None:
    """10 列 schema に従って 1 行を TSV に書き出すヘルパ。

    ``--with-tokens`` が有効なときは ``tokenized`` 列を
    :func:`wrap_with_tokens` で生成、無効なときは空文字列で埋める。
    """
    tokenized = wrap_with_tokens(romaji=clean_romaji, kanji=kanji) if with_tokens else ""
    writer.writerow(
        (
            noisy_romaji,
            kanji,
            clean_romaji,
            str(typo_distance),
            romaji_style,
            str(is_partial),
            str(partial_len),
            tokenized,
            language_segments,
            str(has_en_words),
        )
    )


def _format_language_segments(segments: list[dict[str, object]]) -> str:
    """Segment list を ``"jp:0-3;en:3-9;jp:9-14"`` 形式の文字列に整形する。

    Args:
        segments: :func:`compose_mixed_sentence` の ``segments`` を想定
            (``kind`` / ``start`` / ``end`` を持つ dict の list)。

    Returns:
        セミコロン区切りの segment 記述文字列。
    """
    out: list[str] = []
    for seg in segments:
        out.append(f"{seg['kind']}:{seg['start']}-{seg['end']}")
    return ";".join(out)


def run(
    input_path: Path,
    output_path: Path,
    base_seed: int,
    *,
    with_partials: int = 0,
    with_tokens: bool = False,
    with_mixed_jpen: int = 0,
) -> int:
    """パイプラインを実行し、書き出した data 行数を返す。

    Args:
        input_path: UTF-8 入力ファイル (1 行 1 文)。
        output_path: TSV 出力先パス (存在時は上書き)。
        base_seed: 決定論的 typo 生成のためのベースシード。
        with_partials: 各 clean row に追加する partial (prefix) 数。
            ``0`` のとき無効。
        with_tokens: ``tokenized`` 列を PUA 包み文字列で埋めるかどうか。
        with_mixed_jpen: 追加する mixed JP/EN sentence 数。
            各 sentence は 3 styles x 3 distances = 9 rows を生成する。

    Returns:
        書き出した data row 数 (header 行を除く)。
    """
    sentences = iter_sentences(input_path)
    tokenizer = make_tokenizer()

    output_path.parent.mkdir(parents=True, exist_ok=True)
    row_index = 0
    with output_path.open("w", encoding="utf-8", newline="") as fp:
        writer = csv.writer(fp, delimiter="\t", lineterminator="\n")
        writer.writerow(TSV_HEADER)

        # 1) 既存フロー: extract / romaji / typo
        for sentence in sentences:
            for pair in extract_pairs(sentence, tokenizer):
                for style in ALL_STYLES:
                    clean = kana_to_romaji(pair.reading, style)
                    for distance in _DISTANCES:
                        seed = _row_seed(base_seed, row_index)
                        noisy = inject_typos(clean, distance, seed)
                        _write_row(
                            writer,
                            noisy_romaji=noisy,
                            kanji=pair.kanji,
                            clean_romaji=clean,
                            typo_distance=distance,
                            romaji_style=style,
                            is_partial=0,
                            partial_len=0,
                            with_tokens=with_tokens,
                            language_segments="",
                            has_en_words=0,
                        )
                        row_index += 1

                    # 2) partial 拡張: clean (distance=0) に対して N 個の
                    # partial を生成し、各 partial に 4 distance を適用する。
                    if with_partials > 0 and clean:
                        partial_rng = random.Random(_row_seed(base_seed, row_index))
                        partials = generate_partials(clean, partial_rng, count=with_partials)
                        for partial in partials:
                            for p_distance in _DISTANCES:
                                p_seed = _row_seed(base_seed, row_index)
                                p_noisy = inject_typos(partial, p_distance, p_seed)
                                _write_row(
                                    writer,
                                    noisy_romaji=p_noisy,
                                    kanji=pair.kanji,
                                    clean_romaji=partial,
                                    typo_distance=p_distance,
                                    romaji_style=style,
                                    is_partial=1,
                                    partial_len=len(partial),
                                    with_tokens=with_tokens,
                                    language_segments="",
                                    has_en_words=0,
                                )
                                row_index += 1

        # 3) mixed JP/EN 拡張: sentence 単位で決定論的に合成
        if with_mixed_jpen > 0:
            mixed_rng = random.Random(_row_seed(base_seed, 0xDEADBEEF))
            for _ in range(with_mixed_jpen):
                sentence_obj = compose_mixed_sentence(mixed_rng)
                composed = sentence_obj["composed"]
                seg_str = _format_language_segments(
                    # TypedDict を dict[str, object] 互換へ緩める
                    [dict(seg) for seg in sentence_obj["segments"]]
                )
                for style in ALL_STYLES:
                    clean_romaji = expand_romaji_mixed(sentence_obj, style)
                    for distance in _MIXED_DISTANCES:
                        seed = _row_seed(base_seed, row_index)
                        noisy = inject_typos_mixed(clean_romaji, distance, seed)
                        # mixed 行の kanji 列には ``composed`` (合成文原型) を格納:
                        # 将来 tokenizer が EN 部をそのまま扱うか確認できるよう、
                        # placeholder ではなく元文を残す方針 (ADR 0010 D8 の
                        # 方向性と整合)。
                        _write_row(
                            writer,
                            noisy_romaji=noisy,
                            kanji=composed,
                            clean_romaji=clean_romaji,
                            typo_distance=distance,
                            romaji_style=style,
                            is_partial=0,
                            partial_len=0,
                            with_tokens=with_tokens,
                            language_segments=seg_str,
                            has_en_words=1,
                        )
                        row_index += 1

    return row_index


def main(argv: list[str] | None = None) -> int:
    args = _parse_args(sys.argv[1:] if argv is None else argv)
    count = run(
        args.input,
        args.output,
        args.seed,
        with_partials=args.with_partials,
        with_tokens=args.with_tokens,
        with_mixed_jpen=args.with_mixed_jpen,
    )
    print(f"wrote {count} rows to {args.output}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
