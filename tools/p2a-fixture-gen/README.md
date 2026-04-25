# kotoha-p2a-fixture-gen

Phase 2 P2-A 用 golden fixture (530 cases) 生成 tool。

## 入力

- `../p5a-data-pipeline/fixtures/sample.tsv`(P5-A PoC 生成物、10 列 schema)

P5-A `sample.tsv` の schema(`tools/p5a-data-pipeline/README.md` §"出力 TSV schema"):

| 列番号 | 列名 | 用途 |
| --- | --- | --- |
| 1 | `noisy_romaji` | typo 入り入力(本 tool では未使用) |
| 2 | `kanji` | 正解漢字表層(本 tool で `surface` として採用) |
| 3 | `clean_romaji` | typo なし romaji(本 tool で `reading` として採用) |
| 4 | `typo_distance` | `0` のみ採用 |
| 5 | `romaji_style` | `hepburn` / `kunrei` / `waapuro` 全て採用 |
| 6 | `is_partial` | `0` のみ採用(full 行のみ) |
| 7-10 | partial / token / segment 関連 | 本 tool では未使用 |

P2-A 仕様 §6.3.4 は SudachiDict-core を入力源として明示しているが、Phase 1 完了直後の現状では P5-A `sample.tsv` を仮の入力源として採用する(B5 不足は plan §10 残論点に従い fallback concat で補う)。

## 出力

- `../../crates/kotoha-core/tests/fixtures/kanji_dictionary_golden.tsv`

## 内訳

- targeted 30: 敬称 / 人名 / 地名・組織名 / 外来語(manual curated)
- bulk 500: reading 文字数 5-bucket 各 100 件 stratified sampling

## 実行

```bash
cd tools/p2a-fixture-gen
uv sync
uv run python -m p2a_fixture_gen.main
```

seed 固定(42)で再現性あり。
