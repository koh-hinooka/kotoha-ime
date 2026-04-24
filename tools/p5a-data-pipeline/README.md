# P5-A data pipeline PoC (Kotoha Phase 5)

Kotoha IME の Phase 5(custom romaji-base model)における **P5-A data pipeline** の Proof of Concept 実装です。ADR 0010 で確定した方針に従い、Phase 4 完了後の Phase 5 kick-off 時に実証根拠として利用するための最小縦スライスを提供します。本ツールは 30 件の hand-crafted 日本語文から `(noisy_romaji, kanji, clean_romaji, typo_distance, romaji_style)` の学習対を生成します。

参照:

- `docs/adr/0010-kotoha-custom-romaji-base-model.md`
- `docs/superpowers/specs/2026-04-25-kotoha-phase-5-custom-model.md` §3.1 / §4
- ISSUE #79

## Install

```bash
cd tools/p5a-data-pipeline
uv sync
```

`uv sync` は `pyproject.toml` / `uv.lock` から `.venv/` を構築し、ランタイム依存(sudachipy / sudachidict-small)と dev 依存(pytest / ruff / mypy)をすべて導入します。

## 実行

```bash
uv run -m kotoha_p5a \
    --input fixtures/input_sentences.txt \
    --output fixtures/sample.tsv \
    --seed 42
```

実行後、`fixtures/sample.tsv` に header 付きの TSV が出力されます。`--seed` は typo 注入の乱数シードで、同一シードなら入出力は bit 単位で再現します。

## 出力 TSV schema

| 列番号 | 列名 | 型 | 説明 |
| --- | --- | --- | --- |
| 1 | `noisy_romaji` | string | typo 注入後の入力文字列(編集距離 = `typo_distance`) |
| 2 | `kanji` | string | 正解の漢字表層 |
| 3 | `clean_romaji` | string | typo 注入前の romaji(正解) |
| 4 | `typo_distance` | int | `0` / `1` / `2` / `3` のいずれか。`0` は clean pair |
| 5 | `romaji_style` | string | `hepburn` / `kunrei` / `waapuro` のいずれか |

## 現 scope

- **入力**: `fixtures/input_sentences.txt` 30 件の hand-crafted 日本語文(kanji / hiragana / katakana 混在、短文中心)
- **形態素解析**: sudachipy (Apache-2.0) + sudachidict_small (Apache-2.0)。`SplitMode.C`(最長分割)で kanji を含む文節のみ抽出
- **kana → romaji 変換**: 自前の変換表 `src/kotoha_p5a/romaji.py`。Hepburn / Kunrei / waapuro の 3 方式、ASCII のみ(macron は使わず母音重複で長音を表現)
- **typo 注入**: QWERTY 隣接キーに基づく substitute / transpose / delete / insert、編集距離 0-3、`random.Random(seed)` で再現性を担保

出力行数の目安: 30 sentences × kanji 含む文節(変動)× 3 styles × 4 distances ≈ 300-500 行。

## 非 scope(Phase 5 kick-off 時に対応)

本 PoC は以下を **意図的に含みません**:

- Wikipedia JP / LLM-JP / CC-100 JP からの実データ取得
- 1M〜10M 対の本番規模データ生成
- モデル学習 / 推論
- partial-input generation(ユーザ部分入力の再現)
- special token 挿入
- train / val / test 分割
- lefthook / CI 統合

## Phase 5 kick-off 時の拡張計画

Phase 4(local LLM 統合)完了後の Phase 5 kick-off では、本 PoC を基盤として以下の拡張を段階的に加える予定です:

- **corpus 拡張**: Wikipedia JP + LLM-JP + CC-100 JP の streaming fetch を `datasets` (Apache-2.0) で実装。本 PoC の 30 sentences 固定から、数 M 行の streaming 消費に切り替える
- **partial-input 生成**: 各 clean romaji に対し、先頭 N 文字で切り詰めた部分入力を生成し、実際の IME 入力時の「打鍵途中」状態を模倣する
- **special token 挿入**: 学習時に `<ctx>`、`<romaji>`、`<out>`、`<eos>` 等のセパレータを TSV 出力に埋め込む(現 PoC は raw pair のみ)
- **bias sampling**: 常用漢字頻度や漢字・ひらがな比を考慮した重み付きサンプリングで、低頻度漢字を過学習させないよう均衡を取る
- **validation / test split**: 生成された対を 80 / 10 / 10 で分割し、`train.tsv` / `val.tsv` / `test.tsv` を出力する
- **並列化**: sudachipy 呼び出しを `multiprocessing` で並列化し、1M+ pair 生成を実用時間内(数十分)に収める

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

## License

本ツールは `Apache-2.0` で配布されます(Kotoha 全体ライセンス方針と整合)。

ランタイム依存:

| ライブラリ | ライセンス | 役割 |
| --- | --- | --- |
| sudachipy | Apache-2.0 | 形態素解析器本体 |
| sudachidict-small | Apache-2.0 | 軽量辞書 |

**採用しないライブラリ**(意図的に排除):

- `pykakasi`(GPL-3): kana→romaji 変換は自前の変換表で代替
- `fugashi`(GPL-3、MeCab wrapper): 形態素解析は sudachipy で代替

いずれも Kotoha IME の MIT/Apache-2.0 指向ライセンス方針との非互換性を避けるための決定です。
