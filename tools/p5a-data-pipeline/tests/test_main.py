"""Integration tests for the orchestrator CLI.

`uv run -m kotoha_p5a` を subprocess で起動し、新 CLI フラグ (partial /
tokens / mixed JP-EN) の組み合わせで出力 TSV が 10 列 schema を守るか、
mixed 行が実際に生成されるか、flag なしでも schema 幅が保たれるかを
verify する E2E レベルの統合テスト。
"""

from __future__ import annotations

import subprocess
import sys
from pathlib import Path


def _repo_root() -> Path:
    """本 test file を起点に ``tools/p5a-data-pipeline`` directory を返す。

    Returns:
        ``tools/p5a-data-pipeline`` の絶対パス。
    """
    return Path(__file__).resolve().parent.parent


def test_run_with_all_new_flags_produces_expected_schema(tmp_path: Path) -> None:
    """全新 flag 有効時、header と schema と mixed 行の存在を verify する。

    ``--with-partials 2 --with-tokens --with-mixed-jpen 3`` の組み合わせで
    CLI が exit 0 で完了し、出力 TSV の header が expected 10 列と一致し、
    ``has_en_words == 1`` の mixed 行が最低 1 行含まれることを検証する。
    """
    input_path = _repo_root() / "fixtures" / "input_sentences.txt"
    output_path = tmp_path / "out.tsv"
    result = subprocess.run(
        [
            sys.executable,
            "-m",
            "kotoha_p5a",
            "--input",
            str(input_path),
            "--output",
            str(output_path),
            "--seed",
            "42",
            "--with-partials",
            "2",
            "--with-tokens",
            "--with-mixed-jpen",
            "3",
        ],
        check=True,
        capture_output=True,
        text=True,
        cwd=_repo_root(),
    )
    assert result.returncode == 0
    lines = output_path.read_text(encoding="utf-8").splitlines()
    assert len(lines) > 1, "expected header + at least one data row"
    header = lines[0].split("\t")
    expected_cols = [
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
    ]
    assert header == expected_cols
    # has_en_words=1 行 (mixed JP/EN) が最低 1 行存在すること
    mixed_rows = [line for line in lines[1:] if line.split("\t")[-1] == "1"]
    assert len(mixed_rows) >= 1, "expected at least one mixed JP/EN row with has_en_words=1"


def test_run_without_new_flags_keeps_original_schema_width(tmp_path: Path) -> None:
    """flag を省略しても全 line が 10 列 schema (9 tab) を保つ。

    ``--with-partials`` / ``--with-tokens`` / ``--with-mixed-jpen`` を全て
    省略しても schema 幅が変わらず、新 5 列は空文字列 / 0 で埋められる
    ことを tab 数で間接的に検証する。
    """
    input_path = _repo_root() / "fixtures" / "input_sentences.txt"
    output_path = tmp_path / "out.tsv"
    subprocess.run(
        [
            sys.executable,
            "-m",
            "kotoha_p5a",
            "--input",
            str(input_path),
            "--output",
            str(output_path),
            "--seed",
            "42",
        ],
        check=True,
        cwd=_repo_root(),
    )
    lines = output_path.read_text(encoding="utf-8").splitlines()
    assert len(lines) > 1, "expected header + at least one data row"
    for line in lines:
        # 10 columns -> 9 tabs per line
        assert line.count("\t") == 9, f"expected 10 columns (9 tabs) per line, got: {line!r}"
