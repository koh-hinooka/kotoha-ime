"""Tests for bulk stratified sampling."""

from pathlib import Path

import pytest

from p2a_fixture_gen.sample import (
    bucket_for_reading_length,
    bulk_as_tsv_rows,
    load_sample_pairs,
    stratified_sample,
)


def test_bucket_assignment_by_length() -> None:
    assert bucket_for_reading_length(1) == "B1"
    assert bucket_for_reading_length(3) == "B1"
    assert bucket_for_reading_length(4) == "B2"
    assert bucket_for_reading_length(7) == "B3"
    assert bucket_for_reading_length(11) == "B4"
    assert bucket_for_reading_length(21) == "B5"
    assert bucket_for_reading_length(0) is None


def test_stratified_sample_returns_five_buckets() -> None:
    # synthetic pairs covering buckets B1-B4 evenly; B5 via fallback
    pairs: list[tuple[str, str]] = []
    for n, _bucket in [(2, "B1"), (5, "B2"), (8, "B3"), (15, "B4")]:
        for i in range(150):
            pairs.append(("a" * n, "X" * n + str(i)))
    sampled = stratified_sample(pairs, per_bucket=100)
    assert set(sampled.keys()) == {"B1", "B2", "B3", "B4", "B5"}
    assert len(sampled["B1"]) == 100
    assert len(sampled["B2"]) == 100
    # B5 may be populated via fallback concat
    assert len(sampled["B5"]) == 100


def test_bulk_tsv_rows_have_four_fields() -> None:
    # Provide donor coverage in B3/B4 so that B5 fallback can concat to len>=21
    # (avoids spinning the scarcity-fallback loop).
    pairs: list[tuple[str, str]] = []
    for n in (2, 5, 9, 15):
        for i in range(10):
            pairs.append(("a" * n, "X" * n + str(i)))
    sampled = stratified_sample(pairs, per_bucket=3)
    for row in bulk_as_tsv_rows(sampled):
        assert len(row.split("\t")) == 4


def test_load_sample_pairs_raises_when_missing(tmp_path: Path) -> None:
    """load_sample_pairs must raise FileNotFoundError when the path is absent.

    Regression guard for P2-A hardening item 9: silent failure (returning empty
    list) hides upstream pipeline misconfiguration. The error message must
    contain "p5a sample not found" so users can identify the missing artefact
    without consulting the source.
    """
    missing = tmp_path / "nonexistent.tsv"
    with pytest.raises(FileNotFoundError, match="p5a sample not found"):
        load_sample_pairs(missing)
