---
title: Kotoha Phase 1 設計書 — かな→漢字変換 (Zenz + llama.cpp)
date: 2026-04-24
status: draft
phase: 1
revision: 1
---

# Kotoha Phase 1 (Kana → Kanji Conversion) 設計書

## 目次

- [1. 概要](#1-概要)
- [2. スコープ](#2-スコープ)
- [3. 外部依存](#3-外部依存)
- [4. Architecture](#4-architecture)
- [5. Core API](#5-core-api)
- [6. Data flow (end-to-end)](#6-data-flow-end-to-end)
- [7. CLI contract: kotoha-kanji](#7-cli-contract-kotoha-kanji)
- [8. Test strategy (4 layer)](#8-test-strategy-4-layer)
- [9. 共通 shell library: scripts/lib/assert.sh](#9-共通-shell-library-scriptslibassertsh)
- [10. Risk と緩和策](#10-risk-と緩和策)
- [11. Milestone 分割](#11-milestone-分割)
- [12. ADR 候補](#12-adr-候補-milestone-p1-4-で作成想定)
- [13. Open Questions](#13-open-questions-spec-執筆中--実装中に解消)
- [14. Acceptance (Phase 1 完了条件)](#14-acceptance-phase-1-完了条件)
- [15. 参照](#15-参照)
- [16. Phase 2 への橋渡し](#16-phase-2-への橋渡し)

## 1. 概要

Phase 0 で確定した `InputContext` が生成するひらがな列を、Zenz モデル (GPT-2 系) に llama.cpp 経由で inference し、漢字混じり文の候補を top-K で返す機能を `kotoha-core` crate に追加する。CLI ツール `kotoha-kanji` を新設し、Phase 0 の `kotoha-romaji` と shell pipe で組み合わせることにより、ローマ字 → ひらがな → 漢字混じり文 のフル pipeline を shell 上で確認できるようにする。

Phase 1 の到達点は以下の 3 点に集約される:

1. `kotoha-core::kanji` 公開 API (trait + 具象実装 + Candidate / ConvertOptions / Error 型) が安定しており、Phase 2 以降の追加 backend / 辞書 / 学習層が pattern を再利用できる状態にあること
2. CLI `kotoha-kanji` が `kotoha-romaji | kotoha-kanji --model <path>` の pipe 運用で end-to-end に動作すること
3. 4 層 test 戦略 (unit / mock integration / Zenz smoke / E2E smoke) により、default features での lefthook pre-push が高速に完了し、かつ Zenz 実推論による smoke が opt-in で実行可能な状態にあること

Phase 1 は `InputContext` との直接 wiring は行わない。IME engine 層との接合は Phase 3 で IBus 統合と同時に扱う。Phase 1 の CLI は stdin 1 行 1 ひらがな入力 → stdout 1 行 1 漢字混じり文出力の純粋変換ツールに徹する。

## 2. スコープ

### 2.1 In-scope

以下 8 項目を Phase 1 の実装範囲とする。6 section brainstorming により確定済み。

1. 新規 module `kotoha-core::kanji/` を追加し、trait `KanjiBackend` と最初の具象実装 `ZenzBackend` (llama-cpp-2 経由) を実装する
2. `Candidate` struct (`#[non_exhaustive]`) と `ConvertOptions` struct (`top_k` / `temperature` / `seed`) を公開 API として定義する
3. Config-driven backend factory `load_backend(&BackendConfig) -> Result<Box<dyn KanjiBackend>, KanjiError>` を実装する
4. CLI `kotoha-kanji` を `kotoha-cli` crate に新設する。stdin 1 行 1 ひらがな、stdout 1 行 1 漢字混じり文とし、options として `--model <path>`, `--top-k N`, `--show-scores`, `--show-model-id`, `--temperature F`, `--seed U` を提供する
5. Test 戦略を 4 層で整備する: default features での mock 使用 unit test、`mock-backend` feature による cross-crate integration test、`zenz-smoke` feature による Zenz 実推論 smoke (5-10 件、opt-in)、E2E smoke script
6. `scripts/phase1-smoke.sh` を Phase 0 の `scripts/phase0-smoke.sh` と並列配置する
7. Model placement は manual 前提とする (Q3=A 決定済み)。`README.md` に `Zenz-v2.5-medium` の GGUF 入手コマンドと配置先を明記する
8. 共通 shell library `scripts/lib/assert.sh` を抽出する。Phase 0 smoke script も本 library を使うよう refactor する先行 PR (P1-0) を設ける

### 2.2 Out-of-scope (Phase 2+ 延期)

以下 8 項目は Phase 1 では扱わず、後続 Phase に送る。

1. System dictionary / user dictionary: Phase 2
2. Learning cache (候補選択履歴学習): Phase 2
3. HuggingFace 自動 model download (`hf_download.rs` 実装): Phase 2 または独立 issue として切り出す
4. `InputContext` と kanji backend の直接 wiring: Phase 3 (IBus engine 層で合流)
5. Interactive candidate UI (候補リスト表示 / 選択操作): Phase 3 以降
6. BLEU / exact-match 等の accuracy metric 評価基盤: Phase 2 で品質評価基盤を整備する際に合流
7. Multi-sentence / paragraph 単位の一括変換: Phase 1 は 1 行 1 変換に限定する
8. Benchmark / performance 測定: Phase 5 の advanced features に含める

### 2.3 境界の明文化

Phase 1 の変換単位は「単文 (1 行のひらがな列)」とする。Phase 0 で既に `kotoha-romaji` が line-based commit を採用しているため、Phase 1 の `kotoha-kanji` も line-based の純粋変換ツールとして整合する。段落を越えた文脈考慮や読点 / 句点を区切り文字とする文分割は Phase 2 以降の責務とする。

## 3. 外部依存

### 3.1 llama-cpp-2 (Rust crate)

- crate 名: `llama-cpp-2`
- maintainer: `utilityai`
- version: 実装時 (P1-2 開始時点) に Context7 または crates.io で最新 stable を確認して pin する
- 性質: llama.cpp に bindgen 経由で密結合しているため、llama.cpp 本体の API 変更が Rust 側に波及しうる
- 代替候補の不採用理由 (brainstorming で調査済み):
  - `llama_cpp-rs` — 更新頻度が低く、P1-2 開始時点で最新の llama.cpp API に追随していない
  - `mistral.rs` — GPT-2 系 (Zenz) の GGUF ロード例が少なく、採用リスクが高い
  - `Candle` — pure Rust 推論 framework だが、Zenz の GGUF tokenizer (character-level + byte-level BPE) の対応が Phase 1 時点では未成熟

Phase 1 では `llama-cpp-2` を第一候補として採用し、採用事由と代替比較を ADR 0010 (後述) に記録する。

### 3.2 Zenz model (HuggingFace GGUF)

- Default: **Zenz-v2.5-medium** (`Miwa-Keita/zenz-v2.5-medium-gguf`)
- 選択可能な 3 サイズ: `small` / `medium` / `large`
- 学習データ: 190M JSONL dataset + AJIMEE-Bench (かな→漢字変換 benchmark)
- Release: 2025-01
- Tokenizer: character-level + byte-level BPE。GGUF file に埋め込み済みのため、Rust 側で tokenizer 実装を別途用意する必要はない

Zenz-v2.5-medium を default とする理由:

- `small` は品質が不足し、default として Phase 1 acceptance (smoke 5 件の目視品質判定) を通らない可能性がある
- `large` は推論コストが高く、Phase 1 smoke の 30 秒以内完了 target を超過するリスクがある
- `medium` は品質とコストのバランスが取れており、CPU 推論でも実用的 (brainstorming で確認済み)

### 3.3 【必読】AzooKey Zenzai documentation

> **本 Phase 1 の実装者は、P1-2 (ZenzBackend 実装) の着手前に必ず本ドキュメントを通読すること。**
>
> - URL: <https://github.com/azooKey/AzooKeyKanaKanjiConverter/blob/main/Docs/zenzai.md>
> - AzooKey IME (Swift 実装) の Zenzai system 設計を詳解する唯一の公開 reference
> - Zenz モデルの prompt format / special token handling / decoding strategy は本 docs が 1 次情報源
> - Phase 1 リスク #2 (prompt format 未確定) の 1 次緩和策として、参照は **mandatory**
> - 解析結果は WBS の「prompt format 解析ログ」として必ず記録する

AzooKey は Swift 実装の日本語 IME であり、Kotoha とは別プロジェクトであるが、Zenz model を使った kana→kanji 変換の公開 reference 実装として参考価値が極めて高い。Phase 1 実装者は本 docs を最初に読み、その上で llama-cpp-2 経由の Rust 実装に落とし込む作業を進める。

解析対象は以下:

1. Prompt template の特殊 token 配置 (BOS / EOS / separator 等)
2. 入力ひらがな列の tokenize 方針 (character-level 単位での区切り)
3. Decoding 時の stop condition (EOS token の検出 / max new tokens)
4. Top-K / top-P / temperature の Zenz 文脈での推奨範囲
5. Score (log-probability の aggregation) の扱い方

解析結果は本 Phase 1 の WBS log (P1-2 実装 PR の WBS) に「prompt format 解析ログ」セクションを設けて記録する。記録は Kotoha 実装が AzooKey と挙動差分を生んだ際に参照できる形で残す。

### 3.4 その他の依存追加

- `thiserror` — `KanjiError` の `#[derive(thiserror::Error)]` 用
- `tempfile` (dev-dependency) — Zenz smoke test の一時 model path 確認用

新規依存追加の都度、目的と代替案の比較を commit message または PR body に記載する (project CLAUDE.md の依存管理規約に従う)。

## 4. Architecture

### 4.1 Crate / module layout

Phase 1 完了時点の workspace 構成:

```
kotoha-ime/
├── Cargo.toml                     # workspace
├── crates/
│   ├── kotoha-core/
│   │   ├── Cargo.toml
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── error.rs
│   │   │   ├── kana/              # Phase 0 既存
│   │   │   ├── romaji/            # Phase 0 既存
│   │   │   ├── input/             # Phase 0 既存
│   │   │   └── kanji/             # ★ Phase 1 新設
│   │   │       ├── mod.rs         # 公開 API の再 export
│   │   │       ├── candidate.rs   # Candidate, ConvertOptions
│   │   │       ├── backend.rs     # KanjiBackend trait, BackendConfig, load_backend
│   │   │       ├── error.rs       # KanjiError
│   │   │       ├── mock.rs        # MockBackend (feature = "mock-backend")
│   │   │       └── zenz.rs        # ZenzBackend (feature = "zenz")
│   │   └── tests/
│   │       ├── kanji_mock.rs      # ★ Phase 1 新設 (feature = "mock-backend")
│   │       └── kanji_zenz_smoke.rs# ★ Phase 1 新設 (feature = "zenz-smoke")
│   └── kotoha-cli/
│       ├── Cargo.toml
│       └── src/
│           └── bin/
│               ├── kotoha-romaji.rs  # Phase 0 既存
│               └── kotoha-kanji.rs   # ★ Phase 1 新設
├── scripts/
│   ├── lib/
│   │   └── assert.sh              # ★ Phase 1 先行 PR (P1-0) で新設
│   ├── phase0-smoke.sh            # ★ P1-0 で assert.sh 利用に refactor
│   └── phase1-smoke.sh            # ★ Phase 1 新設
└── docs/
    ├── superpowers/
    │   ├── specs/
    │   │   └── 2026-04-24-kotoha-phase-1-design.md  # 本書
    │   └── plans/
    │       └── 2026-04-24-kotoha-phase-1-implementation.md  # 後続 PR で作成
    └── adr/
        ├── 0009-zenz-model-version-policy.md        # ★ P1-4 で作成
        ├── 0010-kanji-backend-trait-design.md       # ★ P1-4 で作成
        └── 0011-feature-flag-design-for-zenz.md     # ★ P1-4 で作成
```

### 4.2 依存方向

layer 構造は Phase 0 の方針を踏襲する。`kotoha-cli` → `kotoha-core` の単方向依存を維持し、`kanji` module は `kana` / `romaji` / `input` と同列の `kotoha-core` sub-module として配置する。

```
┌─────────────────────────────────────────────────────────┐
│  kotoha-cli (bins: kotoha-romaji, kotoha-kanji)        │
└───────────────┬─────────────────────────────────────────┘
                │ 依存 (single direction)
                ▼
┌─────────────────────────────────────────────────────────┐
│  kotoha-core                                            │
│    ├── kana/    (Phase 0)                               │
│    ├── romaji/  (Phase 0)                               │
│    ├── input/   (Phase 0)                               │
│    └── kanji/   ← Phase 1                               │
│         ├── Candidate / ConvertOptions                  │
│         ├── KanjiBackend trait                          │
│         ├── BackendConfig / load_backend factory        │
│         ├── MockBackend   (feature = "mock-backend")    │
│         └── ZenzBackend   (feature = "zenz") ─────┐     │
└───────────────────────────────────────────────────┼─────┘
                                                    │
                                                    ▼
                                          ┌────────────────────┐
                                          │ llama-cpp-2        │
                                          │   (+ llama.cpp)    │
                                          └────────────────────┘
                                                    │
                                                    ▼
                                          ┌────────────────────┐
                                          │ Zenz GGUF model    │
                                          │ (manual placement) │
                                          └────────────────────┘
```

`kanji/` module は `input` module に依存しない。Phase 1 の CLI `kotoha-kanji` は `InputContext` を経由せず、stdin のひらがな列を直接 `KanjiBackend::convert` に渡す。

### 4.3 Feature flag 構成

`kotoha-core/Cargo.toml` の `[features]` を以下のとおり定義する。

```toml
[features]
default = []
zenz = ["dep:llama-cpp-2"]
zenz-smoke = ["zenz"]
mock-backend = []
```

各 feature の意味:

| Feature | 目的 | 典型利用シーン |
|---------|------|---------------|
| `default` | 実依存を一切引き込まない最小構成 | pre-push gate の素早い build / test |
| `zenz` | `ZenzBackend` を有効化。llama-cpp-2 を dependency に引き込む | 実機で Zenz 推論を行うとき |
| `zenz-smoke` | `zenz` を包含した上で `tests/kanji_zenz_smoke.rs` を gate する | CI 風の opt-in smoke (手動 `cargo test --features zenz-smoke`) |
| `mock-backend` | `MockBackend` を `#[cfg(feature = "mock-backend")]` として公開 | `kotoha-core` 外 (CLI crate) からの integration test |

`mock-backend` は `#[cfg(test)]` では cross-crate 可視にならないため、feature flag で切り出す。これは Rust の標準的な "test-only public API" の公開手段である。

`zenz-smoke` を `zenz` と分離する理由は、lefthook pre-push の default feature build では model load を試みず、opt-in (`--features zenz-smoke`) でのみ実行する設計にするためである。詳細は §8 test strategy を参照。

CLI 側 (`kotoha-cli/Cargo.toml`) は `kotoha-core` を `{ workspace = true, features = ["zenz"] }` で参照する。`kotoha-kanji` バイナリは常に `zenz` feature を有効化して build する。

## 5. Core API

### 5.1 Candidate

```rust
/// 変換結果の 1 候補。
///
/// Phase 2 以降で fields が追加される可能性があるため `#[non_exhaustive]` とする。
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct Candidate {
    /// 漢字混じり文 (UTF-8)
    pub surface: String,
    /// 候補 score (log-probability の aggregation、降順で大きいほど良い)
    pub score: f32,
}

impl Candidate {
    pub fn new(surface: impl Into<String>, score: f32) -> Self {
        Self {
            surface: surface.into(),
            score,
        }
    }
}
```

### 5.2 ConvertOptions

```rust
/// 変換動作の調整 parameter。
///
/// Phase 2 以降で fields が追加される可能性があるため `#[non_exhaustive]` とする。
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct ConvertOptions {
    /// 返す候補の最大数 (1..=N)
    pub top_k: usize,
    /// sampling 温度 (0.0..=2.0 程度。0.0 は greedy)
    pub temperature: f32,
    /// sampling 乱数 seed。`Some(s)` で deterministic、`None` で非 deterministic
    pub seed: Option<u64>,
}

impl Default for ConvertOptions {
    fn default() -> Self {
        Self {
            top_k: 5,
            temperature: 0.0,
            seed: Some(0),
        }
    }
}
```

`seed: Some(u64)` の採用理由: Zenz smoke test と E2E smoke test を deterministic にするため。default の `Some(0)` + `temperature = 0.0` (greedy) により、同じ入力に対して常に同じ出力を得られる。CLI の `--seed` option で override 可能。

### 5.3 KanjiBackend trait

```rust
use crate::kanji::{Candidate, ConvertOptions, KanjiError};

/// ひらがな → 漢字混じり文 の変換 backend 抽象。
///
/// 実装は thread-safety 不要 (Phase 1 は single-thread CLI のみ)。
/// Phase 3 以降で IBus engine 層から呼び出す際に `Send + Sync` が必要になる
/// 可能性があるが、その時点で Phase 2 までの実装を再評価する。
pub trait KanjiBackend {
    /// 実装固有の model identifier (debug 表示 / log 用)。
    /// 例: "zenz-v2.5-medium" / "mock"
    fn model_id(&self) -> &str;

    /// ひらがな列 `input` を top-`options.top_k` 件の漢字候補に変換する。
    ///
    /// # 契約
    ///
    /// - `input` は hiragana のみを含む文字列 (空白 / 記号 / latin 不可)
    /// - `input.chars().count()` は 128 以下
    /// - 返り値の順序は score 降順
    /// - 返り値は `surface` に対して dedupe 済み (最高 score を保持)
    /// - `top_k == 0` のときは空 Vec を返す (error にしない)
    ///
    /// # Errors
    ///
    /// - `KanjiError::InvalidInput { .. }` — 契約違反 (hiragana 以外を含む / 128 超)
    /// - `KanjiError::Backend { .. }` — 実装固有の失敗 (model load / inference 中断)
    fn convert(
        &self,
        input: &str,
        options: &ConvertOptions,
    ) -> Result<Vec<Candidate>, KanjiError>;
}
```

### 5.4 BackendConfig + load_backend factory

```rust
use std::path::PathBuf;

/// Backend の構築パラメータ。
#[non_exhaustive]
#[derive(Debug, Clone)]
pub enum BackendConfig {
    /// 決定的な mock (test 用)。
    ///
    /// `mock-backend` feature が有効な場合のみ構築可能。
    Mock,

    /// Zenz model (llama-cpp-2 経由)。
    ///
    /// `zenz` feature が有効な場合のみ構築可能。
    Zenz {
        /// GGUF file への絶対パス
        model_path: PathBuf,
    },
}

/// Config-driven backend factory。
///
/// 指定された `config` に応じた `KanjiBackend` 実装を返す。
pub fn load_backend(
    config: &BackendConfig,
) -> Result<Box<dyn KanjiBackend>, KanjiError> {
    match config {
        #[cfg(feature = "mock-backend")]
        BackendConfig::Mock => Ok(Box::new(crate::kanji::MockBackend::new())),

        #[cfg(not(feature = "mock-backend"))]
        BackendConfig::Mock => Err(KanjiError::FeatureDisabled {
            feature: "mock-backend",
        }),

        #[cfg(feature = "zenz")]
        BackendConfig::Zenz { model_path } => {
            Ok(Box::new(crate::kanji::ZenzBackend::load(model_path)?))
        }

        #[cfg(not(feature = "zenz"))]
        BackendConfig::Zenz { .. } => Err(KanjiError::FeatureDisabled {
            feature: "zenz",
        }),
    }
}
```

### 5.5 KanjiError

```rust
use std::path::PathBuf;
use thiserror::Error;

/// `kanji` module の error 型。
///
/// streaming enum として扱うため `#[non_exhaustive]` (ADR 0006 準拠)。
#[non_exhaustive]
#[derive(Debug, Error)]
pub enum KanjiError {
    #[error("input violates contract: {reason}")]
    InvalidInput { reason: String },

    #[error("model file not found: {path}")]
    ModelNotFound { path: PathBuf },

    #[error("model load failed: {source}")]
    ModelLoadFailed {
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },

    #[error("backend inference failed: {reason}")]
    Backend { reason: String },

    #[error("required feature not enabled at build time: {feature}")]
    FeatureDisabled { feature: &'static str },
}
```

Error message は英語で記述する (global CLAUDE.md の「backend-facing error messages は英語」に従う)。

### 5.6 Input 制約 (hiragana only / max 128 chars / 単文)

`KanjiBackend::convert` は以下の契約を呼び出し側に課す。

1. **hiragana only**: `input` は Unicode の U+3040..=U+309F (Hiragana block) に加え、長音符 U+30FC ("ー") のみを含む。latin / 漢字 / 記号 / 数字 / 空白 (全角含む) は不可。
2. **max 128 chars**: `input.chars().count() <= 128`。これを超える場合は `KanjiError::InvalidInput` を返す。
3. **単文**: `input` は 1 文 (句点 / 読点を含まない) を想定する。句点を含んでも technical には変換されるが、品質保証の対象外とする。

CLI 側 (`kotoha-kanji`) は入力 line ごとに契約検査を行い、違反時は stderr へ診断を出力し exit code 2 で終了する (§7 参照)。

Phase 0 の `kotoha-romaji` が出力するひらがな列は契約を自動的に満たす (romaji 変換の image が hiragana block に限定されるため)。ただし `kotoha-romaji | kotoha-kanji` の pipe において romaji 側が空行を出力した場合 (例: input が空行)、`kotoha-kanji` は空行入力として扱い空の候補 list を出力する。

### 5.7 Output 順序保証 (score 降順) + dedupe 仕様

`KanjiBackend::convert` の返り値 `Vec<Candidate>` は以下の性質を満たす。

1. **score 降順**: `result[i].score >= result[i+1].score` が任意の隣接 index について成立
2. **surface dedupe**: 返り値内に `surface` が重複する要素は存在しない。同一 surface が複数 score で生成された場合は最高 score を保持する
3. **top_k 尊重**: `result.len() <= options.top_k`。model が top_k 以上の候補を生成しても切り詰める
4. **empty 許容**: `top_k == 0` または `input == ""` のとき、空 Vec を返す (error ではない)

Dedupe は score 順 sort の後、先頭から見て既出 surface を skip することで実装する。これにより「同 surface の最高 score のみが残り、かつ順序は元の score 順に従う」が保証される。

## 6. Data flow (end-to-end)

`kotoha-romaji | kotoha-kanji` pipe の 1 行分の data flow を以下に示す。

```
┌──────────────────┐
│  user input      │  例: "nihongo"
│  (stdin to       │
│  kotoha-romaji)  │
└────────┬─────────┘
         │
         ▼
┌────────────────────────────────────────────────────────────┐
│  kotoha-romaji                                             │
│    RomajiConverter::convert("nihongo")                     │
│       → "にほんご"                                         │
│    stdout に 1 行出力                                       │
└────────┬───────────────────────────────────────────────────┘
         │ shell pipe
         ▼
┌────────────────────────────────────────────────────────────┐
│  kotoha-kanji                                              │
│    1. stdin から 1 行読む                                   │
│    2. Input 契約検査 (hiragana only / 128 chars)            │
│    3. KanjiBackend::convert("にほんご", &options)           │
│       ─┐                                                   │
│        │ trait dispatch                                    │
│        ▼                                                   │
│    ┌───────────────────────────────────────────────────┐   │
│    │ ZenzBackend                                       │   │
│    │   prompt = format_prompt("にほんご")              │   │
│    │   tokens = llama_cpp_2.tokenize(prompt)           │   │
│    │   outputs = llama_cpp_2.generate(tokens, top_k=5) │   │
│    │   candidates = parse_outputs(outputs)             │   │
│    │   candidates = score_sort_dedupe(candidates)      │   │
│    │   → Vec<Candidate>                                │   │
│    └───────────────────────────────────────────────────┘   │
│       │                                                    │
│       ▼                                                    │
│    4. top-1 の surface を stdout に 1 行出力                │
│       (--show-scores 時は score も付与)                     │
└────────────────────────────────────────────────────────────┘
         │
         ▼
┌──────────────────┐
│  user stdout     │  例: "日本語"
└──────────────────┘
```

`kotoha-romaji` と `kotoha-kanji` は独立したプロセスであり、shell pipe でのみ連結する。両者間で共有メモリ / IPC を使わないため、Phase 0 / Phase 1 の CLI 仕様は shell tooling で柔軟に組み合わせ可能である。

## 7. CLI contract: kotoha-kanji

### 7.1 Options

| Option | Required | Default | 意味 |
|--------|----------|---------|------|
| `--model <path>` | **yes** | — | Zenz GGUF file への絶対 or 相対 path |
| `--top-k <N>` | no | 5 | 表示する候補数 (1..=20) |
| `--show-scores` | no | false | 出力行に score を tab 区切りで付与する |
| `--show-model-id` | no | false | 起動時に stderr へ model id を 1 行出力 |
| `--temperature <F>` | no | 0.0 | sampling 温度 (0.0..=2.0) |
| `--seed <U>` | no | 0 | sampling seed (0 で deterministic) |

`--model` を required とするのは、Phase 1 で HuggingFace auto-download を扱わないためである (Out-of-scope §2.2 item 3)。ユーザは README 記載の手順で GGUF を配置し、path を明示する。

### 7.2 STDIN / STDOUT format

- **stdin**: 1 行 1 ひらがな列 (UTF-8)。末尾 newline あり。空行は空行として扱い、空行を stdout に出力する
- **stdout**: 1 行 1 漢字混じり文 (UTF-8)。
  - `--show-scores=false` (default): top-1 の surface のみ
  - `--show-scores=true`: `surface\tscore\tsurface2\tscore2\t...` (top-K 分、tab 区切り)
- **stderr**: 診断情報のみ。`--show-model-id` 指定時は起動時に `model: <id>` を 1 行出力

Line-oriented 設計は Phase 0 の `kotoha-romaji` と一貫させ、shell tooling (grep / awk / head 等) で組み合わせやすくする。

### 7.3 Exit code

Phase 0 convention (`kotoha-romaji`) を踏襲する。

| Code | 意味 |
|------|------|
| 0 | 正常終了 (全行 variant 成功、または stdin が空) |
| 1 | 起動時 fatal error (model load 失敗 / option parse 失敗 / 権限不足) |
| 2 | 実行中に 1 行以上で変換失敗 (契約違反等)、処理は継続するが最終 exit code は 2 |

### 7.4 Line-level error handling

変換途中で 1 行が失敗しても process 全体を停止させず、以下のポリシーで継続する。

1. 失敗した行については stdout に **空行** を出力する (行番号対応を保つ)
2. stderr に `line <N>: <error message>` 形式で 1 行診断を出力する
3. 全行処理完了後、失敗行が 1 以上あれば exit code 2 で終了する

この方針は Phase 0 の `kotoha-romaji` と整合しており、shell 上で `diff <(kotoha-romaji ... | kotoha-kanji ...) expected.txt` 比較が行単位で成立する。

### 7.5 process_line 純粋関数

CLI 内部では I/O を切り離した純粋関数 `process_line` を公開し、unit test 可能にする。

```rust
/// 1 行分の変換結果。
pub struct LineOutput {
    /// stdout に書く本文 (末尾 newline なし)
    pub stdout: String,
    /// 失敗時に stderr に書く 1 行診断 (None = 正常)
    pub stderr_diag: Option<String>,
    /// true = この行で失敗した (exit code 2 候補)
    pub failed: bool,
}

/// 1 行分を変換して LineOutput にする純粋関数。
///
/// I/O を一切行わず、backend だけを依存 (dependency injection)。
pub fn process_line(
    backend: &dyn KanjiBackend,
    options: &ConvertOptions,
    show_scores: bool,
    line_number: usize,
    line: &str,
) -> LineOutput {
    // 1. 空行 pass-through
    // 2. 契約検査 (hiragana only / 128 chars)
    // 3. backend.convert
    // 4. show_scores に応じて整形
}
```

`process_line` は `kotoha-core::kanji::cli_support` 等の sub-module として公開するか、`kotoha-cli` crate 内の lib target として公開するかを実装時に決定する (Open Question Q3 参照)。

## 8. Test strategy (4 layer)

Phase 0 の test philosophy (unit / integration の独立、fixture ベースの golden、reproducibility) を踏襲しつつ、Zenz 実推論を伴う層を opt-in feature で分離する。

### 8.1 Layer 1: Unit (default features)

- 配置: `crates/kotoha-core/src/kanji/*.rs` 内 `#[cfg(test)] mod tests`
- 対象: pure 関数 (`score_sort_dedupe`, `validate_input`, `ConvertOptions::default` 等)
- 件数目安: 約 25 件
- 所要時間: 0.2 秒以内
- Feature: default (= flag なし)
- lefthook pre-push で実行される最速層

### 8.2 Layer 2: Integration (mock-backend feature)

- 配置: `crates/kotoha-core/tests/kanji_mock.rs`
- 対象: `load_backend(BackendConfig::Mock)` / `KanjiBackend` trait の contract (order / dedupe / top_k 尊重) を mock で確認
- 件数目安: 5 件 (trait contract の 5 プロパティに 1:1 対応)
- 所要時間: 0.1 秒以内
- Feature: `mock-backend` (cross-crate 可視にするため)
- 実行: `cargo test --features mock-backend`

`MockBackend` は deterministic な候補を返す実装とする。例: 入力 "にほんご" に対して常に `[Candidate { surface: "日本語", score: 0.9 }, Candidate { surface: "二本後", score: 0.3 }]` を返す。これにより contract test が golden 的に機能する。

### 8.3 Layer 3: Zenz smoke (zenz-smoke feature)

- 配置: `crates/kotoha-core/tests/kanji_zenz_smoke.rs`
- 対象: 実際の Zenz-v2.5-medium GGUF を load して 5 件の smoke input で変換し、品質ではなく「プロセスが通る」ことを確認
- 件数目安: 5 件 (Phase 1 smoke fixture と同一の 5 fixture)
- 所要時間: 10〜30 秒 (CPU 推論)
- Feature: `zenz-smoke`
- 実行: `cargo test --features zenz-smoke` (手動 / opt-in)
- lefthook pre-push には **含めない** (所要時間と model 依存のため)
- model path は環境変数 `KOTOHA_ZENZ_MODEL_PATH` で指定。未設定時は `#[ignore]` 相当で skip する

Smoke 5 件 fixture は `crates/kotoha-core/tests/fixtures/kanji_smoke.tsv` に格納する (`input_hiragana<TAB>expected_substring` 形式)。「top-1 が `expected_substring` を含む」を assertion とする (厳密一致ではなく部分一致。model 更新による些細な揺らぎを許容するため)。

### 8.4 Layer 4: E2E smoke (scripts/phase1-smoke.sh)

- 配置: `scripts/phase1-smoke.sh` (Phase 0 の `scripts/phase0-smoke.sh` と並列)
- 対象: `kotoha-romaji | kotoha-kanji` の pipe を subshell 実行し、end-to-end に期待出力を確認
- 件数目安: 5 件 (Layer 3 と parity を取る)
- 所要時間: 10〜30 秒 (内部で Zenz 推論が走るため)
- 実行: 手動 `bash scripts/phase1-smoke.sh` or `make smoke` 相当
- `KOTOHA_ZENZ_MODEL_PATH` 未設定時は SKIP と表示して exit 0 で終了 (CI fail を起こさない)

Assertion は substring 一致 (`assert_contains`) とし、Layer 3 と同一 fixture を使って parity を保つ。

### 8.5 テスト件数と所要時間

| Layer | 件数目安 | 所要時間目安 | lefthook pre-push | Opt-in |
|-------|---------|-------------|------------------|--------|
| 1. Unit (default) | 〜25 | 0.2 秒 | ✅ | — |
| 2. Integration (mock-backend) | 5 | 0.1 秒 | ✅ | — |
| 3. Zenz smoke (zenz-smoke) | 5 | 10〜30 秒 | ❌ | ✅ |
| 4. E2E smoke (shell) | 5 | 10〜30 秒 | ❌ | ✅ |
| **合計 (pre-push)** | 〜30 | 0.3 秒以内 | — | — |

### 8.6 Model 更新時の fixture regenerate 手順

Zenz model version が更新された場合 (ADR 0009 の policy で判断)、以下の手順で fixture を再生成する。

1. 新 model GGUF を `KOTOHA_ZENZ_MODEL_PATH` に配置する
2. `cargo test --features zenz-smoke` を run し、Layer 3 が pass することを確認する
3. `bash scripts/phase1-smoke.sh` を run し、Layer 4 が pass することを確認する
4. Layer 3 / Layer 4 の期待出力が model update により揺らいでいる場合:
   - `tests/fixtures/kanji_smoke.tsv` の `expected_substring` を新 model の top-1 出力に合わせて更新する
   - 変更理由を commit message に「model version change」として明記する
5. ADR 0009 の「version change log」 section を update する

## 9. 共通 shell library: scripts/lib/assert.sh

### 9.1 API 関数

```bash
# scripts/lib/assert.sh

# 完全一致比較。引数 3 つで短い diagnostic を出力。
# usage: assert_equal <label> <expected> <actual>
# exit: 0 = pass, 1 = fail
assert_equal() {
  local label="$1"
  local expected="$2"
  local actual="$3"
  if [ "$expected" = "$actual" ]; then
    echo "PASS: $label"
    return 0
  else
    echo "FAIL: $label"
    echo "  expected: $expected"
    echo "  actual:   $actual"
    return 1
  fi
}

# 部分一致比較 (substring)。
# usage: assert_contains <label> <needle> <haystack>
# exit: 0 = pass, 1 = fail
assert_contains() {
  local label="$1"
  local needle="$2"
  local haystack="$3"
  case "$haystack" in
    *"$needle"*)
      echo "PASS: $label"
      return 0
      ;;
    *)
      echo "FAIL: $label"
      echo "  expected to contain: $needle"
      echo "  actual:              $haystack"
      return 1
      ;;
  esac
}

# Pass/Fail count の最終 summary。
# usage: assert_summary <pass_count> <fail_count>
# exit: 0 if fail_count == 0, 1 otherwise
assert_summary() {
  local pass="$1"
  local fail="$2"
  echo "---"
  echo "passed: $pass"
  echo "failed: $fail"
  if [ "$fail" -eq 0 ]; then
    return 0
  else
    return 1
  fi
}
```

3 関数の role:

- `assert_equal` — Phase 0 smoke の主用途 (stdout 完全一致)
- `assert_contains` — Phase 1 smoke の主用途 (Zenz 出力の部分一致)
- `assert_summary` — 全行処理後の集計と exit code 決定

### 9.2 Phase 0 smoke の refactor (先行 PR P1-0 で実施)

`scripts/phase0-smoke.sh` を以下のように refactor する。

変更前 (Phase 0 M6 時点):

```bash
# Phase 0 smoke の既存構造 (抜粋・擬似)
actual=$(echo "nihongo" | kotoha-romaji)
if [ "$actual" = "にほんご" ]; then
  echo "PASS"
else
  echo "FAIL: expected にほんご, got $actual"
  fail=$((fail + 1))
fi
```

変更後:

```bash
#!/usr/bin/env bash
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
# shellcheck source=lib/assert.sh
. "$HERE/lib/assert.sh"

pass=0
fail=0

actual=$(echo "nihongo" | kotoha-romaji)
if assert_equal "romaji nihongo" "にほんご" "$actual"; then
  pass=$((pass + 1))
else
  fail=$((fail + 1))
fi
# (以下、他 fixture について同様)

assert_summary "$pass" "$fail"
```

P1-0 は Phase 1 本体実装と分離した先行 PR として実施し、Phase 0 smoke が refactor 後も pass することを CI / lefthook で確認する。これにより Phase 1 の P1-3 で `scripts/phase1-smoke.sh` を書く時点で、library が既に安定している状態にする。

## 10. Risk と緩和策

| # | Risk | 影響 | 緩和策 |
|---|------|------|-------|
| 1 | llama-cpp-2 の API が bindgen 経由で llama.cpp 本体の breaking change に追随 | P1-2 の実装が version pinning に複雑化 | 実装時点 (P1-2 開始日) で Context7 / crates.io 最新 stable に pin し、ADR 0010 に version policy を明記 |
| 2 | Zenz の prompt format (special token / separator) が非自明で reverse-engineer が必要 | P1-2 の着手でロスが発生 | **§3.3 の AzooKey Zenzai docs を mandatory 参照**、WBS に「prompt format 解析ログ」を記録、差分測定は Phase 2 以降 |
| 3 | GGUF model file size (medium 約 150MB、large 数 GB) を repo に含められない | Smoke test の再現性低下 | Model placement は manual、`KOTOHA_ZENZ_MODEL_PATH` で指定、未設定時 smoke は SKIP |
| 4 | CPU inference の latency が CLI pipe 運用で体感できるほど遅い | pipe 運用の UX 低下 | Phase 1 では pipe の proof-of-concept までを acceptance とし、latency 改善は Phase 5 advanced features に先送り |
| 5 | Zenz の出力が同一 surface を複数 score で生成する | dedupe 漏れによる duplicate 候補表示 | `score_sort_dedupe` を trait dispatch の外 (helper 関数) に実装し unit test で覆う |
| 6 | lefthook pre-push に Zenz smoke が混入して CI が遅くなる | 開発者体験の低下 | `zenz-smoke` を独立 feature 化、pre-push は default features のみ実行 |
| 7 | CLI の `--model` 省略時に hard-to-debug error を出す | 初回利用者の困惑 | `--model` を clap の `required = true` とし、unset 時は exit code 1 + README 引用の 1 行診断を stderr に出す |
| 8 | Zenz smoke の 5 fixture が model version 更新で一斉 fail する | 更新 PR のマージ阻害 | §8.6 の regenerate 手順を ADR 0009 と連動させ、PR 内で fixture update を明示許可する |

## 11. Milestone 分割

Phase 1 全体を 5 PR に分割する。合計見積もり約 5.3 日、各 milestone は独立 PR として develop にマージ可能な粒度に絞る。

| PR | Milestone | 主な成果物 | 所要 | 依存 |
|----|-----------|-----------|------|------|
| **P1-0** | Shell library 抽出 | `scripts/lib/assert.sh` 新設、`scripts/phase0-smoke.sh` を library 利用に refactor | 0.3 日 | なし (develop から直接) |
| **P1-1** | Kanji module skeleton + Mock backend | `kotoha-core::kanji/` module 追加、`KanjiBackend` trait、`Candidate` / `ConvertOptions` / `KanjiError`、`MockBackend` (feature = mock-backend)、`load_backend` factory、Layer 1 + Layer 2 test | 1.5 日 | P1-0 merge |
| **P1-2** | ZenzBackend 実装 | `ZenzBackend` 具象実装、llama-cpp-2 依存追加、prompt format 解析 (AzooKey docs 参照)、Layer 3 zenz-smoke test | 2.0 日 | P1-1 merge |
| **P1-3** | CLI kotoha-kanji + Phase 1 smoke | `kotoha-cli::bin::kotoha-kanji` バイナリ、`process_line` 純粋関数、`scripts/phase1-smoke.sh`、Layer 4 E2E smoke | 1.0 日 | P1-2 merge |
| **P1-4** | ADR + closing | ADR 0009 / 0010 / 0011、README 更新 (Zenz 入手手順 + `--model` 使い方)、Phase 1 acceptance checklist の消化、implementation WBS log 確定 | 0.5 日 | P1-3 merge |

合計: 5.3 日、5 PR。各 PR は project CLAUDE.md「Branch Scope Policy」(10 files / 300 lines 目安) に収まる範囲で設計している。P1-2 のみ llama-cpp-2 依存追加と `ZenzBackend` 実装で lines が増えやすいため、テスト fixture の追加を含めて 300 lines を意識して分割する可能性がある。

## 12. ADR 候補 (Milestone P1-4 で作成想定)

Phase 1 終了時に以下 3 本の ADR を作成する。番号は Phase 0 までの canonical numbering (0001〜0008 使用済み) から継続する。

| # | タイトル (仮) | 主内容 |
|---|--------------|--------|
| 0009 | Zenz model version policy | default を v2.5-medium にする理由、更新判断基準、fixture regenerate procedure (§8.6 と連動) |
| 0010 | Kanji backend trait design | `KanjiBackend` trait + `BackendConfig` enum + `load_backend` factory の採用理由、llama_cpp-rs / mistral.rs / Candle との比較 |
| 0011 | Feature flag design for Zenz | `default` / `zenz` / `zenz-smoke` / `mock-backend` 4 flags の分離理由、lefthook pre-push との整合 |

いずれも P1-4 で作成し、本設計書と相互参照する (本書 → ADR、ADR → 本書)。

## 13. Open Questions (spec 執筆中 / 実装中に解消)

以下 5 項目は本設計書時点で未確定であり、指定した milestone で解消する。

| # | Question | 解消 milestone |
|---|----------|---------------|
| Q1 | `llama-cpp-2` の具体 version pin (実装日の最新 stable) | P1-2 開始時点で Context7 確認し pin、ADR 0010 に記録 |
| Q2 | Zenz prompt template の exact form (special token sequence) | P1-2 で AzooKey docs 参照 + 実機検証、WBS に解析ログ記載 |
| Q3 | `process_line` の公開場所 (`kotoha-core::kanji::cli_support` vs `kotoha-cli` lib target) | P1-3 実装時に 2 案比較、ADR 候補外 (実装判断) |
| Q4 | `MockBackend` の fixture をハードコード vs TSV 外出し | P1-1 実装時に判断。小規模 (5 件) のため hard-code を優先候補とする |
| Q5 | `--seed` が `0` のとき deterministic を保証するかの llama-cpp-2 側の挙動 | P1-2 で挙動確認、docs/README に注記 |

## 14. Acceptance (Phase 1 完了条件)

以下 15 項目のうち 15/15 を満たした時点で Phase 1 完了とする。

- [ ] 1. `crates/kotoha-core/src/kanji/` module が公開されている
- [ ] 2. `KanjiBackend` trait、`Candidate` / `ConvertOptions` / `BackendConfig` / `KanjiError` が `kotoha_core::kanji::*` から re-export されている
- [ ] 3. `MockBackend` が `mock-backend` feature 下で公開されている
- [ ] 4. `ZenzBackend` が `zenz` feature 下で公開されており、`load_backend(&BackendConfig::Zenz { .. })` で構築可能である
- [ ] 5. `load_backend` factory が feature 未有効時に `KanjiError::FeatureDisabled` を返す
- [ ] 6. Layer 1 unit test (default features、約 25 件) が pass する
- [ ] 7. Layer 2 integration test (`--features mock-backend`、5 件) が pass する
- [ ] 8. Layer 3 Zenz smoke (`--features zenz-smoke`、5 件) が `KOTOHA_ZENZ_MODEL_PATH` 指定で pass する
- [ ] 9. Layer 4 E2E smoke (`scripts/phase1-smoke.sh`、5 件) が pass する
- [ ] 10. CLI `kotoha-kanji` が `--model <path>` required、option `--top-k / --show-scores / --show-model-id / --temperature / --seed` を受け付ける
- [ ] 11. CLI の exit code 仕様 (0 / 1 / 2) が Phase 0 `kotoha-romaji` と整合している
- [ ] 12. `scripts/lib/assert.sh` が `assert_equal` / `assert_contains` / `assert_summary` を提供し、Phase 0 smoke が refactor 後も pass する
- [ ] 13. ADR 0009 / 0010 / 0011 が作成され、本設計書と相互参照している
- [ ] 14. `README.md` に Zenz-v2.5-medium の GGUF 入手コマンドと配置先、`kotoha-kanji --model` の使い方が記載されている
- [ ] 15. `cargo fmt --all --check` / `cargo clippy --workspace --all-targets -- -D warnings` / `cargo test --workspace` が CI / lefthook pre-push で warning なく pass する

## 15. 参照

- Phase 0 spec: `docs/superpowers/specs/2026-04-22-kotoha-phase-0-design.md` §16 (Phase 1 への橋渡し)
- Phase 0 overall plan: `docs/superpowers/plans/2026-04-22-kotoha-phase-0-implementation.md`
- ADR 0002 (input mode Transient vs Sticky): `docs/adr/0002-input-mode-transient-vs-sticky.md`
- ADR 0005 (romaji Trie over HashMap): `docs/adr/0005-romaji-trie-over-hashmap.md`
- ADR 0008 (canonical romaji Phase 1 判断 = 選択肢 1 採用): `docs/adr/0008-canonical-romaji-and-partial-invertibility.md`
- ADR 0006 (non-exhaustive on streaming enums): `docs/adr/0006-non-exhaustive-on-streaming-enums.md`
- AzooKey Zenzai docs (§3.3 で詳述): <https://github.com/azooKey/AzooKeyKanaKanjiConverter/blob/main/Docs/zenzai.md>
- Zenz-v2.5 collection: <https://huggingface.co/collections/Miwa-Keita/zenz-v25-6784cd5d57147f61bc4c3031>
- llama-cpp-2 crates.io: <https://crates.io/crates/llama-cpp-2>
- User memory: future vision として「入力 convention 拡張は canonical romaji ではなく rule table 追加で対応」、「Phase 6 UI で model 管理機能を想定」の 2 点が確定済み (ADR 0008 §影響 参照)

## 16. Phase 2 への橋渡し

Phase 1 完了時点で以下が Phase 2 の前提として利用可能になる:

- `kotoha-core::kanji::{KanjiBackend, ZenzBackend, Candidate, ConvertOptions, BackendConfig, KanjiError}` 公開 API
- Feature flag 構成の確立 (将来 backend 追加は同じ pattern で拡張可能、ADR 0011)
- `scripts/lib/assert.sh` 共通 shell library (Phase 2+ smoke でも継続利用)
- Zenz model の manual placement 手順 (README 記載)
- ADR 0009 の model version policy (Phase 2 で model 更新する際に参照)

Phase 2 で `kotoha-core` に追加する予定:

- `dict/` モジュール (system dictionary / user dictionary)
- `learning/` モジュール (候補選択履歴の learning cache)
- `kanji/hf_download.rs` (model auto-download、当初 Phase 1 予定 → Phase 2 に shift)
- 品質評価インフラ (BLEU / exact-match metric、AJIMEE-Bench 採用検討)

Phase 2 以降も本設計書の `KanjiBackend` trait / `BackendConfig` factory pattern を踏襲し、新 backend 追加時に既存 `ZenzBackend` / `MockBackend` と同じ公開形式で module 拡張する。
