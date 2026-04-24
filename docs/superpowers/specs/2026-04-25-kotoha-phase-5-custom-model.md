---
title: Kotoha Phase 5 設計書 (stub) — Kotoha 専用 romaji-base かな→漢字モデル
date: 2026-04-25
status: stub
phase: 5
revision: 1
---

# Kotoha Phase 5 (Kotoha custom romaji-base model) 設計書 — stub

本書は Phase 5 の設計書 **stub** である。詳細パラメータ (data pipeline の exact script、model hyperparameter、training schedule、量子化比較の empirical 数値) は Phase 4 完了時の Phase 5 kick-off で確定する。本書の役割は ISSUE #77 時点で Phase 5 のスコープ / 方針 / open question を確定し、Phase 2〜4 実装中に Phase 5 への影響を考慮可能にすることである。

## 目次

- [1. 背景と動機](#1-背景と動機)
- [2. スコープ](#2-スコープ)
- [3. アーキテクチャ](#3-アーキテクチャ)
- [4. データ設計](#4-データ設計)
- [5. 学習戦略](#5-学習戦略)
- [6. 評価](#6-評価)
- [7. 非スコープ (将来 phase)](#7-非スコープ-将来-phase)
- [8. Open questions](#8-open-questions)
- [9. 参照](#9-参照)

## 1. 背景と動機

### 1.1 Phase 1 baseline の ICL 限界 (定量)

Phase 1 P1-2.5 follow-up (PR #76 / ISSUE #75 / merge commit `3eccaa1`) で、Gemma-2-2B-jpn-it Q5_K_M をバックエンドに Layer 3 smoke fixture 15 行を empirical 実測した。結果は `14/15 PASS` であった。

唯一 FAIL した row 3 は「あした → 明日」(期待 substring `明日`) であり、モデルは `翌日` を生成した。v5〜v12 の 8 世代 prompt iteration の概要は以下のとおり。

| version | pass rate | 主要変更 | row 3 状態 |
|---|---|---|---|
| v5 | 8/15 | `apply_chat_template` + 3 件 IME-style few-shot turn | 翌日 |
| v6 | 9/15 | plain-text completion へ pivot、7 pair few-shot | 翌日 |
| v7 | 10/15 | few-shot 11 pair (拗音・外来語・敬称網羅) | 翌日 |
| v8 | 11/15 | few-shot 12 pair (助詞付き文例追加) | 翌日 |
| v9 | 12/15 | few-shot 16 pair (情報量過多で特定 row が regress) | 翌日 |
| v10 | 14/15 | few-shot 13 pair + directive を IME 業務ドメインに具体化 | 翌日 |
| v11 | 14/15 | 「あした → 明日」を query 直前 (隣接位置) に配置 | 翌日 |
| v12 | 14/15 | directive 内に「あした は 明日 であり 翌日 ではない」等の negative example 3 件を明示埋込 | 翌日 |

同種の negative example「ぎゅうにゅう → 牛乳 (not ミルク)」「りょうり → 料理 (not クッキング)」は v12 で PASS した。row 3 のみ解消しない原因は Gemma-2-2B-jpn-it 事前学習分布における「あした ↔ 翌日」の語彙意味的距離が極めて小さく、2B parameter instruction-tuning の表層的な few-shot / directive 指示の信号強度が、pretrain 語彙バイアスを上書きできないためと推定する。

### 1.2 Karukan 調査の要約

Karukan (Linux 向け日本語 IME) は `jinen-v1-small` (90M parameter、GPT-2 base、Q5_K_M 量子化、約 80MB) を採用し、row 3 相当の問題を起こさない。観察した成功要因は以下 4 点である。

1. **タスク専用 fine-tune モデル**: kana→kanji 変換専用に fine-tune された GPT-2 系 decoder-only モデルであり、汎用 instruction LLM ではない。「あした → 明日」は学習分布内に位置し、ICL に依存しない。
2. **学習済み PUA special tokens による構造指示**: `\u{ee02}` (CONTEXT) / `\u{ee00}` (INPUT_START) / `\u{ee01}` (OUTPUT_START) を fine-tuning 時に学習させており、prompt は `\u{ee02}今日は\u{ee00}コンニチハ\u{ee01}` の構造で渡される。Prompt injection 耐性と構造安定性の両方に寄与する。
3. **外部 HuggingFace tokenizer**: Rust の `tokenizers` crate で `tokenizer.json` を直接 load し、llama.cpp 内蔵 tokenizer を完全バイパスする。Phase 1 P1-2-9 で Zenz GGUF が `gpt2-small-japanese-char` pre-tokenizer 不対応により llama.cpp load 失敗した問題を構造的に回避できる。
4. **context 情報の受理**: 推論関数 `build_jinen_prompt(katakana, context)` は周辺テキストを CONTEXT token 後に注入する。

### 1.3 Karukan にも残る前処理層起因の不便

以下の現象はモデルを大きくしても解消しない。モデル到達前の決定論的 romaji→katakana 変換層の限界である。

- 「s」単打: fcitx5 前処理層が「未確定 prefix」として保留し、モデルに届かない
- 「si」入力: Hepburn 規則前処理層が「し」に変換せず、`si` のまま残留する
- 「s<backspace>」: 前処理層の状態機械が中間状態で破綻する
- 「まうｓ」(「ます」の typo): 前処理層は typo を解釈できないため、モデルが「ます」に復元する機会を奪う

### 1.4 Phase 5 で解決する具体目標

Phase 5 は以下 4 点を同時解決する。

- **G1**: row 3 類の synonym bias を task-specific fine-tune で根本解消し、Phase 1 の 14/15 を 15/15 以上に引き上げる
- **G2**: typo tolerance を native に実現する (「まうｓ → ます」「maus → ます」の意図的 typo 注入データで fine-tune)
- **G3**: partial-input 対応 (`s` / `sh` / `si` 等の prefix から top-k 候補を即時算出)
- **G4**: context-aware 変換 (周辺テキストを special token で注入し同音異義の曖昧性を解決)

詳細は Phase 5 kick-off で確定する。

## 2. スコープ

### 2.1 In-scope

Phase 5 の実装範囲は以下 5 項目とする。

1. **モデル本体**: romaji-base 90〜180M parameter decoder-only transformer (GPT-2-like) の Kotoha 固有構築
2. **学習パイプライン**: data 抽出 + MeCab 形態素解析 + romaji 拡張 + typo 注入 + partial-input 生成 + special tokens 設計 + training loop + evaluation harness
3. **推論統合**: GGUF Q5_K_M 量子化 + 外部 HuggingFace tokenizer + `BackendConfig::KotohaNative` variant + partial-input beam search + typo top-k + context 注入 + Sudachi 辞書 fallback
4. **評価 set**: smoke 15-row / 200-row regression / 10k-row golden / typo robustness 100 件 / partial-input set / latency SLA
5. **CLI 統合**: `kotoha-kanji` への `--backend kotoha-native` option 追加 (Phase 1 の `llama-cpp` backend と並立)

### 2.2 Out-of-scope (Phase 6 以降)

以下 3 項目は Phase 5 では扱わず、後続 Phase に送る。

1. **Training infrastructure 自動化** (CI-driven training pipeline、data 差分再学習): Phase 6 以降
2. **Personalization / user adapter** (ユーザごとの微小 fine-tune layer): Phase 6 以降
3. **Multilingual extension** (英語 / 中国語 / 韓国語 入力対応): Phase 8 以降の別議論

詳細は Phase 5 kick-off で再確認する。

## 3. アーキテクチャ

### 3.1 データパイプライン (概観)

データパイプラインは以下の段階を直列に接続する。詳細スクリプト仕様は Phase 5 kick-off で確定する。

```
[コーパス source]
  LLM-JP Corpus / CC-100 JP / Wikipedia JP
        │ ライセンス確認 + 抽出
        ▼
[形態素解析 + kana→kanji ペア生成]
  MeCab + 独自 filter (信頼性の低い読み / 多義語を排除)
        │
        ▼
[kana → romaji 拡張]
  Hepburn / Kunrei / waapuro 3 方式を並立展開
        │
        ▼
[augmentation]
  typo 注入 (edit distance 1〜3)
  partial-input 生成 (ランダム truncate)
        │
        ▼
[special tokens wrap]
  <ctx> / <romaji> / <out> / <eos> + 拡張候補
        │
        ▼
[tokenize]
  HuggingFace tokenizers (tokenizer.json)
        │
        ▼
[training 入力]
```

### 3.2 学習 (概観)

学習戦略候補を 3 本洗い出し、default を B3 distillation とする。詳細は §5 学習戦略を参照。empirical 比較と最終確定は Phase 5 kick-off で行う。

### 3.3 推論統合 (概観)

推論経路は Phase 1 の `LlamaCppBackend` と並立させる。

- Phase 1 の `BackendConfig::LlamaCpp { model_path, prompt_template }` に加え、Phase 5 で `BackendConfig::KotohaNative { model_path, tokenizer_path }` variant を追加する
- 外部 HuggingFace tokenizer を `tokenizers` crate で load する
- 推論本体は llama-cpp-2 を使用する (Phase 5 kick-off で他 inference backend との比較を行い確定、stub 段階では default candidate として記述)。ただし tokenize / detokenize の I/O boundary は `tokenizers` に委譲する
- beam search で partial-input 候補 top-k を算出する
- Sudachi 辞書 fallback を OOV (Out-Of-Vocabulary) 検知時の後段として併用する

詳細 variant 定義と CLI option は Phase 5 kick-off で確定する。

### 3.4 Sudachi 辞書 fallback

モデル単独では人名 / 地名 / 新語など OOV 語彙の recall が不足するため、Sudachi 辞書 (`Sudachi-core-dict`) を後段 fallback として併用する。統合方式は以下 3 候補から empirical 比較 (Phase 5 kick-off) で確定する。

- 候補 a: `kanji` module 側で convert 前に Sudachi lookup し、hit 時は model をスキップ
- 候補 b: model の top-k 候補に Sudachi lookup 結果を後段 reranker として merge
- 候補 c: model と Sudachi を並列実行し、score fusion で最終候補を決定

詳細は Phase 5 kick-off で確定する。

## 4. データ設計

### 4.1 コーパス source とライセンス

以下 3 source を候補とする。最終採用は Phase 5 kick-off で確定する。

| Source | 想定 size | ライセンス | 特徴 |
|---|---|---|---|
| LLM-JP Corpus | 数百 GB 級 | CC BY 4.0 (一部) | 日本語 LLM 学習向けに整備済み、形態素解析済み版も存在 |
| CC-100 Japanese | 数十 GB | Common Crawl 由来 | Web text 主体、noise fill が必要 |
| Wikipedia JP | 数 GB | CC BY-SA 3.0 | 高品質だが size が不足、augmentation source として利用 |

Kotoha は OSS として再配布予定のため、学習済みモデルの再配布ライセンスと source ライセンスの両立を Phase 5 kick-off 時点で再確認する。

### 4.2 kana → romaji 拡張規則

kana 1 つに対して複数の romaji 候補を並立展開する。以下は代表例であり、完全表は Phase 5 kick-off で確定する。

| かな | Hepburn | Kunrei | waapuro |
|---|---|---|---|
| し | shi | si | si / shi |
| ち | chi | ti | ti / chi |
| つ | tsu | tu | tu / tsu |
| ふ | fu | hu | hu / fu |
| ぢ | ji | zi | di |
| づ | zu | zu | du |
| ん | n (母音前 n'), m (b/p 前) | n | nn / n |
| 促音 っ + X | 子音重ね | 子音重ね | 子音重ね |
| 長音符 ー | 直前母音重ね / `-` | 直前母音重ね | `-` |

目的は「実ユーザが入力しうる全ての romaji 綴りを学習分布内に含める」ことである。

### 4.3 typo 注入規則

edit distance 1〜3 の範囲で以下 4 種類の typo を確率的に注入する。

| 種類 | 例 | 注入確率 (目安) |
|---|---|---|
| 隣接キー置換 | `arigato` → `afigato` (r → f) | 0.3 |
| 文字転倒 | `arigato` → `airgato` | 0.2 |
| 文字欠落 | `arigato` → `arigto` | 0.3 |
| 文字余剰 | `arigato` → `arigaato` | 0.2 |

キーボードレイアウトは US 109 JIS として隣接 map を定義する。確率と edit distance 分布の最終値は Phase 5 kick-off で確定する。

### 4.4 partial-input 生成規則

完全な romaji 列 `arigato` から先頭切り出しで `a`, `ar`, `ari`, `arig`, `ariga`, `arigat`, `arigato` の 7 通り prefix を生成する。全 prefix に対して正解 kanji を教師信号として与えると「入力完了前に候補を出す」IME 挙動が native に学習される。

prefix 切り出しの分布 (どの長さを何割選ぶか) は Phase 5 kick-off で empirical 確定する。

### 4.5 special tokens

PUA (Private Use Area) 領域 `\u{ee00}-\u{ee0F}` を Karukan 踏襲で採用する案と、新領域 / 新 id で設計する案を Phase 5 kick-off で empirical 判断する。最低限学習させる token の役割は以下 4 種。

| Token 役割 | 例 PUA id | 意味 |
|---|---|---|
| `<ctx>` | `\u{ee02}` | 周辺テキスト (context) の開始 |
| `<romaji>` | `\u{ee00}` | romaji 入力の開始 |
| `<out>` | `\u{ee01}` | モデル出力の開始 |
| `<eos>` | `\u{ee03}` | 生成終了 |

Phase 5 の拡張 special tokens 候補は以下 3 種を検討する。

- `<edit>`: `backspace` / 再変換の編集履歴を表現
- `<partial>`: 未確定 prefix を明示
- `<typo-hint>`: typo tolerance を強調する hint

詳細 id 割り当ては Phase 5 kick-off で確定する。

### 4.6 データ scale

目標は 1M〜10M kana→kanji ペアである。B1 scratch training では 10M+ ペアが必要と推定し、B3 distillation では 1M〜3M ペアで teacher の soft label を引き継げる想定である。最終 scale は Phase 5 kick-off 時の empirical pilot で確定する。

## 5. 学習戦略

学習戦略候補 3 本を empirical 比較し、default を B3 distillation とする。

### 5.1 B1 scratch

GPT-2 Small (90M parameter) アーキテクチャを random 初期化から学習する。

- 必要 data size: 10M+ kana→kanji ペア
- 必要 compute: RTX 4090 で 1〜3 GPU-day
- 利点: teacher に依存しない、Kotoha 固有の語彙傾向を直接学習できる
- 欠点: data size が膨らむ、初期 loss が高く収束までの時間が長い

### 5.2 B2 LoRA

Gemma-2-2B-jpn-it の LoRA adapter を学習する。

- 必要 data size: 100k〜1M ペア
- 必要 compute: 数 GPU-hour
- 利点: training cost が最小
- 欠点: **inference 時に base model (2.6B / 1.92GB) の load が必要であり、IME 常駐 size budget (≤ 200MB) を満たさない**
- 扱い: **候補から除外推奨**。ただし Phase 5 kick-off 時点で「offline batch 用途」の別 target として再検討する余地は残す

### 5.3 B3 distillation (default 推奨)

Gemma-4-31B-it 等の大規模 teacher モデルで soft label (top-k 確率分布) を生成し、90〜180M parameter student モデルに transfer する。

- 必要 data size: 1M〜3M ペア
- 必要 compute: teacher 推論 (数 GPU-day) + student 学習 (1〜2 GPU-day)
- 利点: 高品質な soft label により、scratch より少ない data で収束する。teacher の Japanese 語彙知識を student に圧縮できる
- 欠点: teacher の Japanese quality に下限が制約される。teacher が row 3 類の synonym bias を持つ場合、student にも波及する可能性があり、evaluation で確認が必要

teacher 候補は以下 3 つを Phase 5 kick-off で empirical 比較する。

- `Gemma-4-31B-it` (31B、Japanese 性能 high、size 18GB、inference に H100 級が必要)
- `Llama-3.1-Nemotron-70B-Instruct` (70B、英語中心だが日本語も可、size 40GB)
- `Qwen2.5-72B-Instruct` (72B、多言語、Apache 2.0、size 45GB)

### 5.4 比較計画

Phase 5 kick-off 初週に以下を実施する。

1. 共通 evaluation fixture を §6 の smoke / typo / partial-input set で事前確定する
2. B1 と B3 の両方で pilot training を走らせる (B1 は data 1M で short run、B3 は teacher soft label 100k で short run)
3. 上記 pilot の結果で row 3 解消 / typo pass rate / latency / size の 4 軸を比較する
4. default を最終確定する (想定: B3、B1 を fallback として残す)

## 6. 評価

### 6.1 Smoke / regression / golden の 3 層

| 層 | 件数 | 用途 |
|---|---|---|
| Layer 3 smoke (Phase 1 と共通) | 15 | Phase 1 baseline との regression 確認 |
| 200-row regression | 200 | 敬称 / 拗音 / 外来語 / 文章など Phase 5 固有に拡張 |
| 10k-row golden | 10000 | BLEU / exact-match metric での統計的品質評価 |

10k-row golden の作成方針は Phase 5 kick-off で確定する (人手校正 vs 自動抽出の比率、license 互換性)。

### 6.2 typo robustness set

意図的 typo 100 件を edit distance 1〜3 で作成し、Phase 5 モデルで pass rate を測定する。例: `arigat` (欠落) / `afigato` (隣接キー置換) / `airgato` (転倒) / `arigaato` (余剰) / `まうｓ` (半角/全角混在 + 欠落) を含む。

### 6.3 partial-input set

完全 romaji 列 100 件から各 prefix を抽出し、top-k 候補に正解 kanji が含まれる率を測定する。IME の逐次候補表示の品質指標とする。

### 6.4 latency SLA

- p50 ≤ 30ms (CPU 推論、RTX 不要 host)
- p99 ≤ 100ms
- 測定環境: Phase 5 kick-off で確定 (現状想定: Intel i7-12700 相当 + 16GB RAM)

### 6.5 model size target

- GGUF Q5_K_M で **≤ 200MB**
- 量子化精度の empirical 比較候補: Q5_K_M default、Q4_K_M (size 縮小但し精度劣化確認)、Q8_0 (精度維持但し size 膨張)

詳細な benchmark スクリプトは Phase 5 kick-off で確定する。

## 7. 非スコープ (将来 phase)

Phase 5 のスコープから以下を明示的に除外する。

- **user adapter / personalization**: ユーザごとの微小 fine-tune layer は Phase 6 以降に延期する。Phase 5 は汎用モデル 1 本で完結させる
- **multilingual extension**: 英語 / 中国語 / 韓国語 入力対応は Phase 8 以降の別議論とする
- **streaming inference**: 現行 Phase 5 は single-shot 推論のみを扱う。streaming が必要となった場合は Phase 6 着手時点で別 ADR を起こして判断する
- **training infrastructure の CI 化**: Phase 5 は手動 training を前提とする。CI-driven 差分再学習は Phase 6 以降

## 8. Open questions

Phase 5 kick-off 時点で解消する 6 点を以下に列挙する。

| # | Question | 解消 milestone |
|---|---|---|
| Q1 | B1 scratch と B3 distillation のどちらを default とするか | Phase 5 kick-off 初週 (pilot 結果で判断) |
| Q2 | special tokens を PUA 踏襲 (`\u{ee00}-\u{ee0F}`) とするか新設とするか | tokenizer 学習時に empirical 確定 |
| Q3 | context window size を 256 / 512 / 1024 のいずれにするか | Phase 5 kick-off 初週 (Karukan は 256、Phase 5 は 512 / 1024 を検討) |
| Q4 | 量子化精度を Q5_K_M default として Q4_K_M / Q8_0 をどう扱うか | Phase 5 P5-C で empirical 比較 |
| Q5 | Sudachi 辞書 fallback の integration layer を §3.4 の候補 a / b / c のいずれにするか | Phase 5 P5-C で empirical 比較 |
| Q6 | user learning cache (Karukan の `learning.tsv` 相当) を Phase 5 に含めるか Phase 6 に延期するか | Phase 5 kick-off 初週で scope 判断 |

詳細は Phase 5 kick-off で確定する。

## 9. 参照

- ADR 0010 (Phase 5 方針決定): `docs/adr/0010-kotoha-custom-romaji-base-model.md`
- ROADMAP restructure 反映先: `docs/ROADMAP.md` Phase 一覧 + Phase 5 マイルストーン分割節
- Phase 1 設計書 (14/15 baseline の根拠): `docs/superpowers/specs/2026-04-24-kotoha-phase-1-design.md`
- Phase 1 P1-2.5 follow-up 実装ログ (row 3 ICL 限界の empirical 記録): `docs/wbs/2026-04-24-feature-75-prompt-optimization-15-row-fixture.md`
- ADR 0009 prep note (Gemma-2-2B-jpn-it pivot): `docs/adr/0009-kanji-backend-model-selection-prep.md`
- Karukan (参照設計): <https://github.com/akaza-im/karukan> (jinen-v1-small の PUA special tokens + 外部 HuggingFace tokenizer 実装)
- LLM-JP Corpus: <https://llm-jp.nii.ac.jp/>
- CC-100 Japanese: <https://data.statmt.org/cc-100/>
- Sudachi dictionary: <https://github.com/WorksApplications/Sudachi>

本 stub の詳細化は Phase 4 完了時の Phase 5 kick-off で実施する。
