# WBS: feature/86-p5a-expansion-iter1

**ISSUE:** #86
**ブランチ:** feature/86-p5a-expansion-iter1
**作成日:** 2026-04-25
**完了日:** 2026-04-25
**ステータス:** 完了

## 概要

P5-A data pipeline PoC(親 PoC PR #80)を iter1 として拡張し、以下の 3 機能を追加する:

1. **partial-input 生成**(Phase 5 spec §4.4 対応、PoC レベル)
2. **special token 挿入**(Phase 5 spec §4.5 / ADR 0010 D4 対応、PUA 包み)
3. **mixed JP/EN corpus 動的生成**(Phase 5 spec §4.7 / ADR 0010 D8 対応、ISSUE #88 主目的)

PR #89(ADR 0010 D8 / Phase 5 spec §3.5 / §4.7 確定)を前提として、依存追加ゼロ(sudachipy + sudachidict-small のみ、GPL ゼロ維持)で拡張する。

## タスク分解

### 1. 3 新 module の追加(`src/kotoha_p5a/`)

- [x] `partial.py`(98 行): `truncate_random` / `generate_partials` を実装。`random.Random` を外部注入で受け取り決定論的動作。PR #90 review で `max_unique <= count` の短文に対して exhaustive enumeration path を追加(確定的動作に)
- [x] `tokens.py`(115 行): Karukan 踏襲 PUA 4 種(U+EE00..U+EE03、ADR 0010 D4 / Phase 5 spec §4.5 初期候補と整合)を `Final[str]` 定数として定義、`wrap_with_tokens` / `unwrap` を実装
- [x] `mixed_jp_en.py`(225 行): EN 単語 list 2 種(programming 18 語 + 一般 12 語)、hand-crafted JP template 6 種、`compose_mixed_sentence` / `expand_romaji_mixed` / `inject_typos_mixed` を実装。PR #90 review で EN 単語 find 失敗時の silent skip を早期 raise に変更

### 2. 対応する test file の追加(`tests/`)

- [x] `test_partial.py`(8 tests): seed 再現 / guard / prefix 性質 / distinct count capped / 短文 exhaustive enumeration(PR #90 review で追加)
- [x] `test_tokens.py`(7 tests): PUA code point(U+EE00..U+EE03)/ wrap+unwrap 往復(context あり / なし / 空列)/ malformed での ValueError
- [x] `test_mixed_jp_en.py`(7 tests): 再現 / segment 連結性 / offset 整合 / EN 単語 list 包含 / JP ASCII 変換 / typo 再現
- [x] `test_main.py`(2 tests、PR #90 review で追加): 全新 flag 有効時の 10 列 schema + mixed 行存在 / flag 省略時の tab 数保持 を subprocess 起動で verify

### 3. `__main__.py` 更新

- [x] 新 CLI フラグ 3 種を追加(`--with-partials N`、`--with-tokens`、`--with-mixed-jpen N`)
- [x] TSV schema を 5 列から 10 列に拡張(末尾に `is_partial` / `partial_len` / `tokenized` / `language_segments` / `has_en_words` を追加)
- [x] 処理フローを (a) 既存 extract→romaji→typo、(b) partial 拡張、(c) mixed JP/EN 拡張 の 3 層に分割
- [x] `_write_row` ヘルパで書き出しを集約

### 4. 検証

- [x] `uv run ruff check .` exit 0
- [x] `uv run ruff format --check .` exit 0
- [x] `uv run mypy src tests` exit 0
- [x] `uv run pytest -v` exit 0、54 tests PASS(既存 extract 5 + romaji 18 + typo 7 = 30、新規 partial 8 + tokens 7 + mixed_jp_en 7 + main integration 2 = 24)。最終数は PR #90 review 対応後の pytest 実測結果に基づく

### 5. fixtures / README / WBS

- [x] `fixtures/sample.tsv` を再生成(1261 行 → 3826 行)
- [x] `README.md` を iter1 向けに全面改訂(新 CLI フラグ / 10 列 schema / PUA トークン表 / mixed JP/EN 解説 / 拡張計画の ✓ 更新 / テスト数 51 記載)
- [x] 本 WBS を新規作成

## schema 拡張(before / after)

| 項目 | iter0(PR #80) | iter1(本 PR) |
| --- | --- | --- |
| 列数 | 5 | 10 |
| 新列 | — | `is_partial` / `partial_len` / `tokenized` / `language_segments` / `has_en_words` |
| サンプル行数(header 込) | 1261 | 3826 |
| テスト数 | 30(extract 5 + romaji 18 + typo 7) | 54(+ partial 8 + tokens 7 + mixed_jp_en 7 + main integration 2) |
| 新 module 数 | 3(extract / romaji / typo) | 6(+ partial / tokens / mixed_jp_en) |
| 依存追加 | — | **なし**(GPL ゼロ維持) |

## Phase 5 spec への対応付け

| spec §節 | 対応 module | 状態 |
| --- | --- | --- |
| §3.5(mixed JP/EN 方針) | `mixed_jp_en.py` | iter1 で PoC 実装完了 |
| §4.4(partial-input 生成) | `partial.py` | iter1 で PoC 実装完了(random truncation のみ) |
| §4.5(special token 設計) | `tokens.py` | iter1 で PUA 初期候補を実装(最終確定は Phase 5 kick-off) |
| §4.7(mixed JP/EN corpus) | `mixed_jp_en.py` | iter1 で hand-crafted 版を実装(Wikipedia streaming は次 iter) |

ADR 0010 の D4(PUA 初期候補)/ D8(mixed JP/EN 方針)を実装レベルで反映済。

## 次 iteration への申し送り

Phase 5 kick-off(Phase 4 完了後)で対応する残項目:

- **Wikipedia JP / LLM-JP / CC-100 JP の streaming fetch**: `datasets` (Apache-2.0) 採用、1M-10M 規模で本番データ化
- **bias sampling**: 常用漢字頻度 + 漢字/ひらがな比で均衡調整
- **train / val / test split**: 80 / 10 / 10 で `train.tsv` / `val.tsv` / `test.tsv` を別出力
- **segment-aware typo 注入**: mixed 行で EN 単語境界を跨がないよう注入を segment 内に限定
- **並列化**: sudachipy 呼び出しを `multiprocessing` で並列化、1M+ pair 生成を実用時間内に
- **PUA 最終確定**: 学習実験の結果を見て、Karukan 踏襲 PUA を維持するか、本番 tokenizer 向け specialized token id に差し替えるかを決定(ADR 0010 D4 フォローアップ)
- **romaji 混合 typo**: 現 PoC では PUA 文字が typo 注入対象に含まれない(tokenized 列は注入後に生成)。将来 training ready 列を直接注入対象にする場合、PUA 保護の adjacency 除外を追加

## 依存関係

- PR #80(親 PoC)**merge 済み前提**
- PR #89(ADR 0010 D8 / Phase 5 spec §3.5 / §4.7)**merge 済み前提**
- 後続: Phase 5 kick-off 時に iter2 として Wikipedia streaming 対応

## 参照

- 親 PoC: PR #80(`fa4ea5d`)
- 前提 PR: PR #89(`3631935` + mixed JP/EN 追記)
- ADR: `docs/adr/0010-kotoha-custom-romaji-base-model.md`(D4 / D8)
- spec: `docs/superpowers/specs/2026-04-25-kotoha-phase-5-custom-model.md` §3.5 / §4.4 / §4.5 / §4.7
