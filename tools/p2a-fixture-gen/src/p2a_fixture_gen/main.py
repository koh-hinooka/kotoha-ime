"""P2-A fixture generator entry point: writes 530-case golden TSV."""

from pathlib import Path

from p2a_fixture_gen.sample import (
    bulk_as_tsv_rows,
    load_sample_pairs,
    stratified_sample,
)
from p2a_fixture_gen.targeted import targeted_as_tsv_rows

REPO_ROOT = Path(__file__).resolve().parents[4]
OUTPUT = REPO_ROOT / "crates" / "kotoha-core" / "tests" / "fixtures" / "kanji_dictionary_golden.tsv"


def main() -> None:
    header = [
        "# kanji_dictionary_golden.tsv — Phase 2 P2-A golden fixture",
        "# Schema: input<TAB>expected_top_surface<TAB>category<TAB>note",
        "# Contents: targeted 30 + bulk 500 (5-bucket stratified × 100)",
        "# Generation: tools/p2a-fixture-gen (seed=42)",
        "# Threshold: ≥90% pass rate (spec §3.9 / §6.3.2)",
    ]
    targeted = targeted_as_tsv_rows()
    bulk = bulk_as_tsv_rows(stratified_sample(load_sample_pairs()))

    OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    OUTPUT.write_text("\n".join(header + targeted + bulk) + "\n", encoding="utf-8")
    print(f"wrote {len(targeted) + len(bulk)} rows -> {OUTPUT}")


if __name__ == "__main__":
    main()
