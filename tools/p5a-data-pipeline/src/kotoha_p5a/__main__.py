"""CLI orchestrator for the Phase 5 P5-A data pipeline PoC.

Reads Japanese sentences from ``--input``, extracts (kanji, reading)
pairs via :mod:`kotoha_p5a.extract`, converts each reading to three
romaji styles via :mod:`kotoha_p5a.romaji`, and emits (noisy_romaji,
kanji, clean_romaji, typo_distance, romaji_style) tuples at four
typo distances (0..3) via :mod:`kotoha_p5a.typo`.

Example:
    uv run -m kotoha_p5a \\
        --input fixtures/input_sentences.txt \\
        --output fixtures/sample.tsv \\
        --seed 42
"""

import argparse
import csv
import sys
from pathlib import Path
from typing import Final

from kotoha_p5a.extract import extract_pairs, iter_sentences, make_tokenizer
from kotoha_p5a.romaji import ALL_STYLES, kana_to_romaji
from kotoha_p5a.typo import inject_typos

TSV_HEADER: Final[tuple[str, ...]] = (
    "noisy_romaji",
    "kanji",
    "clean_romaji",
    "typo_distance",
    "romaji_style",
)

_DISTANCES: Final[tuple[int, ...]] = (0, 1, 2, 3)


def _parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        prog="kotoha_p5a",
        description="Phase 5 P5-A data pipeline PoC: extract → romaji → typo → TSV.",
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
    return parser.parse_args(argv)


def _row_seed(base_seed: int, row_index: int) -> int:
    # Derive a per-row seed deterministically from the base seed and the
    # row index so that (input, base_seed) uniquely determines every
    # output row while individual rows still diverge from one another.
    return (base_seed * 1_000_003 + row_index) & 0xFFFFFFFF


def run(input_path: Path, output_path: Path, base_seed: int) -> int:
    """Execute the pipeline and return the number of data rows written.

    Args:
        input_path: UTF-8 text file, one sentence per line.
        output_path: Destination TSV path (overwritten if it exists).
        base_seed: Base seed for deterministic typo generation.

    Returns:
        The count of data rows emitted (header excluded).
    """
    sentences = iter_sentences(input_path)
    tokenizer = make_tokenizer()

    output_path.parent.mkdir(parents=True, exist_ok=True)
    row_index = 0
    with output_path.open("w", encoding="utf-8", newline="") as fp:
        writer = csv.writer(fp, delimiter="\t", lineterminator="\n")
        writer.writerow(TSV_HEADER)
        for sentence in sentences:
            for pair in extract_pairs(sentence, tokenizer):
                for style in ALL_STYLES:
                    clean = kana_to_romaji(pair.reading, style)
                    for distance in _DISTANCES:
                        seed = _row_seed(base_seed, row_index)
                        noisy = inject_typos(clean, distance, seed)
                        writer.writerow((noisy, pair.kanji, clean, str(distance), style))
                        row_index += 1
    return row_index


def main(argv: list[str] | None = None) -> int:
    args = _parse_args(sys.argv[1:] if argv is None else argv)
    count = run(args.input, args.output, args.seed)
    print(f"wrote {count} rows to {args.output}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
