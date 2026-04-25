"""Bulk 500-case stratified sampling (5 buckets × 100) from sample.tsv.

Schema mapping for P5-A `tools/p5a-data-pipeline/fixtures/sample.tsv` (10 cols, header row):

| col idx | name           | usage in this tool                                  |
| ------- | -------------- | --------------------------------------------------- |
| 0       | noisy_romaji   | unused                                              |
| 1       | kanji          | adopted as `surface` (output column 2)              |
| 2       | clean_romaji   | adopted as `reading` (output column 1)              |
| 3       | typo_distance  | filter: only `0` rows accepted                      |
| 4       | romaji_style   | all 3 styles accepted                               |
| 5       | is_partial     | filter: only `0` rows accepted                      |
| 6-9     | partial / token / segment | unused                                   |

Note: the plan originally indexed `parts[0]` / `parts[1]` assuming a 2-col
`reading\\tsurface` schema. The actual P5-A output is a 10-col TSV with header,
so this module reads `clean_romaji` (idx 2) as reading and `kanji` (idx 1) as
surface, filters to `typo_distance=0` and `is_partial=0`, and dedupes pairs.
"""

import random
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[4]
P5A_SAMPLE = REPO_ROOT / "tools" / "p5a-data-pipeline" / "fixtures" / "sample.tsv"


def bucket_for_reading_length(n: int) -> str | None:
    if 1 <= n <= 3:
        return "B1"
    if 4 <= n <= 6:
        return "B2"
    if 7 <= n <= 10:
        return "B3"
    if 11 <= n <= 20:
        return "B4"
    if n >= 21:
        return "B5"
    return None


def load_sample_pairs(path: Path = P5A_SAMPLE) -> list[tuple[str, str]]:
    """Load (reading, surface) pairs from p5a sample.tsv.

    File format: 10-col TSV with header. Columns used:
      - col idx 2 = clean_romaji (treated as reading)
      - col idx 1 = kanji (treated as surface)
    Filter: typo_distance == 0 AND is_partial == 0. Dedupe identical pairs.
    Empty / comment lines and the header row are skipped.
    """
    pairs: list[tuple[str, str]] = []
    seen: set[tuple[str, str]] = set()
    if not path.exists():
        raise FileNotFoundError(
            f"p5a sample not found: {path}. Run "
            "`cd tools/p5a-data-pipeline && uv run -m kotoha_p5a "
            "--input fixtures/input_sentences.txt --output fixtures/sample.tsv "
            "--seed 42 --with-partials 2 --with-tokens --with-mixed-jpen 5` first."
        )
    for raw in path.read_text(encoding="utf-8").splitlines():
        line = raw.strip()
        if not line or line.startswith("#"):
            continue
        parts = line.split("\t")
        if len(parts) < 6:
            continue
        # Skip header row (first column literal value `noisy_romaji`).
        if parts[0] == "noisy_romaji":
            continue
        # Filter: typo_distance must be 0, is_partial must be 0.
        if parts[3] != "0" or parts[5] != "0":
            continue
        reading = parts[2]
        surface = parts[1]
        if not reading or not surface:
            continue
        key = (reading, surface)
        if key in seen:
            continue
        seen.add(key)
        pairs.append(key)
    return pairs


def stratified_sample(
    pairs: list[tuple[str, str]], per_bucket: int = 100, seed: int = 42
) -> dict[str, list[tuple[str, str]]]:
    """Group pairs by bucket and sample per_bucket from each. B5 fallback handles scarcity."""
    rng = random.Random(seed)
    buckets: dict[str, list[tuple[str, str]]] = {
        "B1": [],
        "B2": [],
        "B3": [],
        "B4": [],
        "B5": [],
    }
    for reading, surface in pairs:
        b = bucket_for_reading_length(len(reading))
        if b is not None:
            buckets[b].append((reading, surface))

    sampled: dict[str, list[tuple[str, str]]] = {}
    # Iteration cap on the fallback loop. Prevents infinite spinning when the
    # donor pool cannot, in principle, synthesize a target-bucket pair.
    max_attempts = per_bucket * 200
    for b, rows in buckets.items():
        if len(rows) >= per_bucket:
            sampled[b] = rng.sample(rows, per_bucket)
            continue

        sampled[b] = list(rows)
        # B5 scarcity fallback (plan §10): concatenate two B3/B4 entries to
        # reach len>=21 until per_bucket items are reached. For B1-B4, no
        # fallback is performed (concatenation cannot synthesize shorter
        # strings); the bucket simply takes whatever natural data exists.
        if b != "B5":
            continue
        donor = buckets["B3"] + buckets["B4"]
        attempts = 0
        while len(sampled[b]) < per_bucket and donor and attempts < max_attempts:
            attempts += 1
            a = rng.choice(donor)
            c = rng.choice(donor)
            combined = (a[0] + c[0], a[1] + c[1])
            if len(combined[0]) >= 21:
                sampled[b].append(combined)
    return sampled


def bulk_as_tsv_rows(sampled: dict[str, list[tuple[str, str]]]) -> list[str]:
    """Convert sampled dict into TSV rows with category=bulk_<bucket>."""
    rows: list[str] = []
    for bucket, pairs in sampled.items():
        for reading, surface in pairs:
            rows.append(f"{reading}\t{surface}\tbulk_{bucket}\tstratified")
    return rows
