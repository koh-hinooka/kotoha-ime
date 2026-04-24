"""Tests for targeted 30-case list."""

from p2a_fixture_gen.targeted import TARGETED_CASES, targeted_as_tsv_rows


def test_targeted_has_30_cases() -> None:
    assert len(TARGETED_CASES) == 30


def test_targeted_categories_cover_four_buckets() -> None:
    categories = {c.category for c in TARGETED_CASES}
    assert categories == {"honorific", "personal_name", "place_org", "loanword"}


def test_targeted_tsv_rows_have_four_fields() -> None:
    for row in targeted_as_tsv_rows():
        assert len(row.split("\t")) == 4
