# P5-A data pipeline PoC (Kotoha Phase 5)

Kotoha IME の Phase 5(custom romaji-base model)における **P5-A data pipeline** の Proof of Concept 実装です。ADR 0010 で確定した方針に従い、Phase 4 完了後の Phase 5 kick-off 時に実証根拠として利用するための最小縦スライスを提供します。本ツールは hand-crafted 日本語文から以下を生成します:

- 通常 `(noisy_romaji, kanji, clean_romaji, typo_distance, romaji_style)` の学習対
- random truncation による partial-input 行(ISSUE #86 iter1 で追加)
- PUA による training-ready トークン列(ISSUE #86 iter1 で追加)
- mixed JP/EN 混合文行(ISSUE #86 iter1 で追加、segment 情報付き)

参照:

- `docs/adr/0010-kotoha-custom-romaji-base-model.md`(D4 PUA トークン初期値 / D8 mixed JP/EN 方針)
- `docs/superpowers/specs/2026-04-25-kotoha-phase-5-custom-model.md` §3.1 / §4.4 / §4.5 / §4.7
- ISSUE #79(親 PoC)/ #86(本 iter1 拡張)/ #88(mixed JP/EN 主目的)

## Install

```bash
cd tools/p5a-data-pipeline
uv sync
```

`uv sync` は `pyproject.toml` / `uv.lock` から `.venv/` を構築し、ランタイム依存(sudachipy / sudachidict-small)と dev 依存(pytest / ruff / mypy)をすべて導入します。

## 実行

最小実行(iter0 相当、既存 5 列):

```bash
uv run -m kotoha_p5a \
    --input fixtures/input_sentences.txt \
    --output fixtures/sample.tsv \
    --seed 42
```

拡張実行(iter1、10 列すべて活用):

```bash
uv run -m kotoha_p5a \
    --input fixtures/input_sentences.txt \
    --output fixtures/sample.tsv \
    --seed 42 \
    --with-partials 2 \
    --with-tokens \
    --with-mixed-jpen 5
```

実行後、`fixtures/sample.tsv` に header 付きの TSV が出力されます。`--seed` は typo 注入 / partial 生成 / mixed 合成すべての乱数シードで、同一シードなら入出力は bit 単位で再現します。

## CLI フラグ

| フラグ | 型 | 既定 | 説明 |
| --- | --- | --- | --- |
| `--input` | path | 必須 | UTF-8 入力ファイル(1 行 1 文) |
| `--output` | path | 必須 | TSV 出力先 |
| `--seed` | int | 42 | typo / partial / mixed 共通の base seed |
| `--with-partials` | int | 0 | 各 clean row に対して生成する partial(prefix truncation)数。0 で無効 |
| `--with-tokens` | flag | off | `tokenized` 列を PUA 包み文字列で埋める |
| `--with-mixed-jpen` | int | 0 | mixed JP/EN sentence 追加数。各 sentence は 3 styles × 3 distances = 9 rows に展開 |

## 出力 TSV schema(10 列)

| 列番号 | 列名 | 型 | 説明 |
| --- | --- | --- | --- |
| 1 | `noisy_romaji` | string | typo 注入後の入力文字列 |
| 2 | `kanji` | string | 正解の漢字表層(mixed 行は合成文原型) |
| 3 | `clean_romaji` | string | typo 注入前の romaji(正解) |
| 4 | `typo_distance` | int | `0` / `1` / `2` / `3` のいずれか |
| 5 | `romaji_style` | string | `hepburn` / `kunrei` / `waapuro` |
| 6 | `is_partial` | int | `1` のとき partial 行、`0` は full 行 |
| 7 | `partial_len` | int | partial の char 長(full 行は `0`) |
| 8 | `tokenized` | string | `--with-tokens` 有効時、`noisy_romaji` と `kanji` を PUA token (U+EE00..U+EE03) で包んだ training-ready 文字列。無効時は空文字列。`typo_distance=0` 行では `noisy_romaji == clean_romaji` のため挙動差はないが、`typo_distance >= 1` 行では typo 入り入力を包む(inference 時に model が受け取る noisy 入力と整合) |
| 9 | `language_segments` | string | mixed JP/EN 行のみ `jp:0-3;en:3-9;jp:9-14` 形式、他は空文字列 |
| 10 | `has_en_words` | int | mixed 行は `1`、他は `0` |

## PUA トークン(`--with-tokens`)

Karukan 踏襲の PUA code point を初期値として採用しています(ADR 0010 D4 / Phase 5 spec §4.5)。最終確定は Phase 5 kick-off のモデル学習検証後。

| 定数名 | Code point | 意味 |
| --- | --- | --- |
| `ROMAJI_TOKEN` | U+EE00 | `<romaji>` |
| `OUT_TOKEN` | U+EE01 | `<out>` |
| `CTX_TOKEN` | U+EE02 | `<ctx>`(空 context 時は省略) |
| `EOS_TOKEN` | U+EE03 | `<eos>` |

フォーマット:

```
[CTX_TOKEN ctx] ROMAJI_TOKEN romaji OUT_TOKEN kanji EOS_TOKEN
```

## mixed JP/EN(`--with-mixed-jpen`)

EN 単語は hand-crafted list(programming 文脈 + 一般 loanword)から、JP 部は `{en}` placeholder 付き hand-crafted template から、合成時にランダムに選択されます。将来、Wikipedia streaming corpus fetch で大規模化する計画(次 iteration)ですが、PoC では外部通信ゼロ・依存ゼロ追加で動的生成を完結させます。

romaji 化時は EN 部をそのまま保持、JP 部のみ 3 style で変換します。typo 注入は PoC では文字列全体に適用(厳密な segment-aware 注入は Phase 5 kick-off で実装)。

## 現 scope(iter1 時点)

- **入力**: `fixtures/input_sentences.txt` 33 件の hand-crafted 日本語文
- **形態素解析**: sudachipy (Apache-2.0) + sudachidict_small (Apache-2.0)、`SplitMode.C`
- **kana → romaji 変換**: 自前変換表 `src/kotoha_p5a/romaji.py`、Hepburn / Kunrei / waapuro の 3 方式、ASCII のみ
- **typo 注入**: QWERTY 隣接 substitute / transpose / delete / insert、編集距離 0-3、`random.Random(seed)` で再現性担保
- **partial-input**: random prefix truncation(`src/kotoha_p5a/partial.py`)
- **PUA 包み**: 4 種の PUA トークンで training-ready 列を生成(`src/kotoha_p5a/tokens.py`)
- **mixed JP/EN**: programming + 一般 loanword の hand-crafted EN 単語 × JP テンプレート(`src/kotoha_p5a/mixed_jp_en.py`)

出力行数の目安(iter1 の全フラグ ON): 既存フロー ~1260 行 + partial 拡張(~約 2500 行)+ mixed(5 × 9 = 45 行)≈ 3800 行。

## Phase 5 拡張計画

Phase 4 完了後の Phase 5 kick-off では、本 PoC を基盤として以下の拡張を段階的に加える予定です:

- [x] **partial-input 生成**(iter1: ISSUE #86 完了): random truncation による PoC 実装
- [x] **special token 挿入**(iter1: ISSUE #86 完了): PUA 包み(Karukan 踏襲)を初期候補として実装
- [x] **mixed JP/EN corpus**(iter1: ISSUE #86 完了): hand-crafted template + EN 単語 list から動的生成
- [ ] **Wikipedia corpus 拡張**: Wikipedia JP + LLM-JP + CC-100 JP の streaming fetch を `datasets` (Apache-2.0) で実装
- [ ] **bias sampling**: 常用漢字頻度や漢字・ひらがな比を考慮した重み付きサンプリング
- [ ] **train / val / test split**: 生成された対を 80 / 10 / 10 で分割
- [ ] **segment-aware typo 注入**: mixed 行で EN 単語境界を跨がない typo 注入
- [ ] **並列化**: sudachipy 呼び出しを `multiprocessing` で並列化し、1M+ pair 生成を実用時間内に収める
- [ ] **token id 最終確定**: Phase 5 kick-off での学習実験に基づき、PUA code point を学習済み tokenizer に適合

## テスト

```bash
uv run pytest
```

個別に品質ゲートを流す場合:

```bash
uv run ruff check .
uv run ruff format --check .
uv run mypy src tests
uv run pytest -v
```

iter1 時点のテスト数: 54(既存 extract 5 + romaji 18 + typo 7 = 30、新規 partial 8 + tokens 7 + mixed_jp_en 7 + main integration 2 = 24)。新しい integration tests は `tests/test_main.py` にあり、CLI を subprocess 起動して 10 列 schema / mixed 行生成 / flag なし fallback を検証する。

## License

本ツールは `Apache-2.0` で配布されます(Kotoha 全体ライセンス方針と整合)。

iter1 拡張は **依存を追加しません**(sudachipy + sudachidict-small のみ、すべて Apache-2.0)。

ランタイム依存:

| ライブラリ | ライセンス | 役割 |
| --- | --- | --- |
| sudachipy | Apache-2.0 | 形態素解析器本体 |
| sudachidict-small | Apache-2.0 | 軽量辞書 |

**採用しないライブラリ**(意図的に排除):

- `pykakasi`(GPL-3): kana→romaji 変換は自前の変換表で代替
- `fugashi`(GPL-3、MeCab wrapper): 形態素解析は sudachipy で代替

いずれも Kotoha IME の MIT/Apache-2.0 指向ライセンス方針との非互換性を避けるための決定です。
