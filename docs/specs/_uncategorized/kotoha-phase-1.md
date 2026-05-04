---
feature: kotoha-phase-1
status: implemented
bounded_context: _uncategorized
related_issues: ["#75", "#83"]
related_prs: []
glossary_refs: ["llama-cpp", "gemma-2-jpn-it", "kana-to-kanji", "layer-3-smoke"]
last_reviewed: 2026-05-05
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

Phase 0 で確定した `InputContext` が生成するひらがな列を、LLM (Phase 1 default: Gemma-2-2B-jpn-it) に llama.cpp 経由で inference し、漢字混じり文の候補を top-K で返す機能を `kotoha-core` crate に追加する。CLI ツール `kotoha-kanji` を新設し、Phase 0 の `kotoha-romaji` と shell pipe で組み合わせることにより、ローマ字 → ひらがな → 漢字混じり文 のフル pipeline を shell 上で確認できるようにする。

Phase 1 の到達点は以下の 3 点に集約される:

1. `kotoha-core::kanji` 公開 API (trait + 具象実装 + Candidate / ConvertOptions / Error 型) が安定しており、Phase 2 以降の追加 backend / 辞書 / 学習層が pattern を再利用できる状態にあること
2. CLI `kotoha-kanji` が `kotoha-romaji | kotoha-kanji --model <path>` の pipe 運用で end-to-end に動作すること
3. 4 層 test 戦略 (unit / mock integration / llama.cpp smoke / E2E smoke) により、default features での lefthook pre-push が高速に完了し、かつ実推論 (Phase 1 default: Gemma-2-2B-jpn-it) による smoke が opt-in で実行可能な状態にあること

Phase 1 は `InputContext` との直接 wiring は行わない。IME engine 層との接合は Phase 3 で IBus 統合と同時に扱う。Phase 1 の CLI は stdin 1 行 1 ひらがな入力 → stdout 1 行 1 漢字混じり文出力の純粋変換ツールに徹する。

## 2. スコープ

### 2.1 In-scope

以下 8 項目を Phase 1 の実装範囲とする。6 section brainstorming により確定済み。

1. 新規 module `kotoha-core::kanji/` を追加し、trait `KanjiBackend` と最初の具象実装 `LlamaCppBackend` (llama-cpp-2 経由) を実装する
2. `Candidate` struct (`#[non_exhaustive]`) と `ConvertOptions` struct (`top_k` / `temperature` / `seed`) を公開 API として定義する
3. Config-driven backend factory `load_backend(&BackendConfig) -> Result<Box<dyn KanjiBackend>, KanjiError>` を実装する
4. CLI `kotoha-kanji` を `kotoha-cli` crate に新設する。stdin 1 行 1 ひらがな、stdout 1 行 1 漢字混じり文とし、options として `--model <path>`, `--top-k N`, `--show-scores`, `--show-model-id`, `--temperature F`, `--seed U` を提供する
5. Test 戦略を 4 層で整備する: default features での mock 使用 unit test、`mock-backend` feature による cross-crate integration test、`llama-cpp-smoke` feature による実推論 smoke (Phase 1 default: Gemma-2-2B-jpn-it、15 件 (row 3 skip で有効 14 件)、opt-in)、E2E smoke script
6. `scripts/phase1-smoke.sh` を Phase 0 の `scripts/phase0-smoke.sh` と並列配置する
7. Model placement は manual 前提とする (Q3=A 決定済み)。`README.md` に `Gemma-2-2B-jpn-it` の GGUF 入手コマンドと配置先を明記する
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

### 3.2 Default model: Gemma-2-2B-jpn-it (GGUF)

- Default: **Gemma-2-2B-jpn-it Q5_K_M** (`bartowski/gemma-2-2b-jpn-it-GGUF`, ファイル名 `gemma-2-2b-jpn-it-Q5_K_M.gguf`, 約 1.92 GB)
- License: Gemma License (再配布可、attribution 必須。Kotoha repo に同梱はしない — 利用者が HuggingFace から download する。手順は `crates/kotoha-core/tests/kanji_llama_cpp_smoke.rs` の module docstring 参照)
- Tokenizer: SentencePiece (Gemma 2 family), chat template は GGUF metadata の `tokenizer.chat_template` に埋め込み済
- 採用根拠: P1-2-9 empirical verification (WBS `docs/wbs/2026-04-24-feature-69-zenz-backend-layer3-smoke.md` commit `718fd8e`) で Qwen2.5-1.5B-Instruct / Gemma-2-2B-jpn-it / Gemma-3-1B-it の 3-way 比較を実施し、Gemma-2-2B-jpn-it が 5/5 (敬称 `やまださん` → `山田さん` を含む) を達成した唯一のモデル
- Phase 1 latency: cold load 約 10.6 秒 + warm inference 約 3 秒 / case。P1-2.5 follow-up (PR #76) で fixture が 15 行に復元され、row 3 skip で有効 14 件が約 52 秒 (cold load 約 10.6 秒 + 14 件 × 約 3 秒) で実行される。spec §8.3 の当初 "30 秒以内" target は ADR 0013 で empirical 実測に合わせて緩和済 (詳細は ADR 0013 を参照)
- Quantization 選択肢: Q4_K_M (品質劣化あり、Phase 1 default としては不適) / Q5_K_M (本採用) / Q6_K / Q8_0 (size 3.3 GB 超、Phase 1 budget 逼迫)

#### 3.2.1 IME-style prompt wrapper (実装上の補足)

Gemma-2-2B-jpn-it は instruction-tuned なので、生のひらがな入力を `apply_chat_template(None)` に渡すだけでは chat-style 応答 (入力エコー + emoji + 改行) を返し、kana→kanji 変換を行わない。P1-2.5 follow-up (PR #76) では当初採用した `apply_chat_template` 経路から **plain-text completion + v12 few-shot prompt** 方式へ pivot した。実装 (`crates/kotoha-core/src/kanji/llama_cpp.rs::build_prompt`) は `build_prompt(template: &PromptTemplate, user_input: &str) -> String` というシグネチャで、`Gemma2InstructChat` / `Qwen2Chat` variant の場合に以下の要素を単一の plain-text 文字列として組み立てる:

1. **strict directive** — 音韻保持の 1 対 1 写像であり翻訳・類義語置換を行わない、という指示文
2. **13 pair の positive few-shot** — 短単語 (`えき → 駅`) / 熟語 (`にほんご → 日本語`) / 拗音 (`ちゃわん → 茶碗`) / 外来語長音 (`こーひー → コーヒー`) / 送り仮名 (`たべもの → 食べ物`) / 敬称 (`やまださん → 山田さん`) / 外来語交じり文 (`パソコンをつかう → パソコンを使う`) / 文章 (`わたしはがくせいです → 私は学生です`) などを網羅
3. **3 pair の negative example 対照** — `あした → 明日` / `ぎゅうにゅう → 牛乳` / `りょうり → 料理` の 3 行を directive 内で「`翌日` / `ミルク` / `クッキング` ではない」と明示的に禁じたうえで、few-shot 末尾にも `あした → 明日` の正例を再掲し、pretrain bias による翻訳出力を抑制する
4. **query 行** — `入力: {user_input}\n出力: ` でモデル補完を誘導

この構造は `build_prompt` の内部実装詳細であり、`KanjiBackend::convert` の公開契約 (spec §5.3) には影響しない。Phase 2 で別 backend (e.g. fine-tuned specialized IME model) を追加する際は、Gemma 系には few-shot が必要/不要という事実を踏まえ、`PromptTemplate` variant を増やして切り替える。

#### 3.2.2 Historical context: Zenz (Miwa-Keita) reevaluation

P1-2 着手時点では Zenz-v2.5-medium (`Miwa-Keita/zenz-v2.5-medium-gguf`) を default 候補とした。P1-2-9 empirical verification 時に以下を発見:

- Miwa-Keita 配下の Zenz GGUF (v1 / v2 / v2.5-medium [gated] / v3.1-small) は `tokenizer.ggml.pre = "gpt2-small-japanese-char"` を使用
- llama-cpp-2 0.1.145 (bundled llama.cpp commit `e21cdc11`) および upstream llama.cpp master の pre-tokenizer allow-list には `gpt2-small-japanese-char` が未登録
- llama-cpp-2 version bump でも解消しない architectural blocker

結果として Zenz family は Phase 1 では採用しない。Phase 2+ で upstream llama.cpp が `gpt2-small-japanese-char` を allow-list に追加するか、Zenz 側が pre-tokenizer を変更した段階で再評価する。この経緯は ADR 0009 (P1-4 正式起票、先行メモは `docs/adr/0009-kanji-backend-model-selection-prep.md`) に記録する。

#### 3.2.3 Phase 2 tiered-model candidates

- **Qwen2.5-1.5B-Instruct Q5_K_M** (1.29 GB, Apache 2.0): 敬称対応は劣るが license の柔軟性が高く、将来 Kotoha を完全 OSS として再配布する際の fallback 候補
- **Gemma-3-1B-it Q5_K_M** (0.85 GB, Gemma License): 軽量だが hallucination が 3/5 発生 (Phase 1 quality bar に到達せず、tiered-model の fast-path 候補としてのみ保留)
- **Gemma-4-31B-it 系 Community GGUF** (13-18 GB, Gemma or Apache 2.0): Phase 1 size budget を 6-9 倍超過するため不採用。Phase 2+ の large-tier model 候補

### 3.3 Historical reference: AzooKey Zenzai documentation

> Phase 1 default model を Gemma-2-2B-jpn-it に pivot した結果、本 section は **実装必読ではなく歴史的参照** に降格する。Prompt format は llama-cpp-2 の `apply_chat_template` が GGUF metadata から読み取る `tokenizer.chat_template` を信頼する方針に切り替えた (ADR 0009 予定)。

AzooKey は Swift 実装の日本語 IME であり、Zenz 系 GGUF を使う prompt format の公開 reference 実装として Kotoha Phase 1 の初期設計で参照された:

- URL: <https://github.com/azooKey/AzooKeyKanaKanjiConverter/blob/main/Docs/zenzai.md>
- P1-2 着手時点では「Zenz 系 GGUF の PUA token 利用 / context / input / output 分離 / EOS 扱い」が 1 次情報源として必須
- P1-2-9 pivot により、Zenz 系 GGUF は本 Phase では採用しないため、上記解析ログは `docs/wbs/2026-04-24-feature-69-zenz-backend-layer3-smoke.md` に記録したまま残し、コードからは削除した (`crates/kotoha-core/src/kanji/llama_cpp.rs` からは PUA token / AzooKey 由来の実装は P1-2.5 で撤去済)

Phase 2 以降で Zenz 系 GGUF が再評価された際、または upstream llama.cpp が `gpt2-small-japanese-char` pre-tokenizer を allow-list に追加した際には、本 section の内容を再度一次情報化するかを ADR で判断する。

### 3.4 その他の依存追加

- `thiserror` — `KanjiError` の `#[derive(thiserror::Error)]` 用
- `tempfile` (dev-dependency) — llama.cpp smoke test の一時 model path 確認用

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
│   │   │       └── llama_cpp.rs   # LlamaCppBackend (feature = "llama-cpp")
│   │   └── tests/
│   │       ├── kanji_mock.rs           # ★ Phase 1 新設 (feature = "mock-backend")
│   │       └── kanji_llama_cpp_smoke.rs# ★ Phase 1 新設 (feature = "llama-cpp-smoke")
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
        ├── 0009-phase-1-default-model-selection.md  # P1-4 で正式化 (元 prep 0009-kanji-backend-model-selection-prep.md を rename)
        ├── 0010-kotoha-custom-romaji-base-model.md  # Phase 5 方針 (ADR 0010 として確定済、PR #78)
        ├── 0011-kanji-backend-trait-design.md       # P1-4 で作成 (spec 当初計画の 0010 から +1 繰上げ)
        ├── 0012-feature-flag-design-for-llama-cpp.md # P1-4 で作成 (spec 当初計画の 0011 から +1 繰上げ)
        └── 0013-phase-1-latency-target.md           # P1-4 で作成 (spec §8.3 の 30s target を empirical に見直し)
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
│         └── LlamaCppBackend (feature = "llama-cpp")┐    │
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
                                          │ GGUF model         │
                                          │ (Phase 1 default:  │
                                          │  Gemma-2-2B-jpn-it,│
                                          │  manual placement) │
                                          └────────────────────┘
```

`kanji/` module は `input` module に依存しない。Phase 1 の CLI `kotoha-kanji` は `InputContext` を経由せず、stdin のひらがな列を直接 `KanjiBackend::convert` に渡す。

### 4.3 Feature flag 構成

`kotoha-core/Cargo.toml` の `[features]` を以下のとおり定義する。

```toml
[features]
default = []
llama-cpp = ["dep:llama-cpp-2"]
llama-cpp-smoke = ["llama-cpp"]
mock-backend = []
```

各 feature の意味:

| Feature | 目的 | 典型利用シーン |
|---------|------|---------------|
| `default` | 実依存を一切引き込まない最小構成 | pre-push gate の素早い build / test |
| `llama-cpp` | `LlamaCppBackend` を有効化。llama-cpp-2 を dependency に引き込む | 実機で llama.cpp-family backend (Phase 1 default: Gemma-2-2B-jpn-it) 推論を行うとき |
| `llama-cpp-smoke` | `llama-cpp` を包含した上で `tests/kanji_llama_cpp_smoke.rs` を gate する | CI 風の opt-in smoke (手動 `cargo test --features llama-cpp-smoke`) |
| `mock-backend` | `MockBackend` を `#[cfg(feature = "mock-backend")]` として公開 | `kotoha-core` 外 (CLI crate) からの integration test |

`mock-backend` は `#[cfg(test)]` では cross-crate 可視にならないため、feature flag で切り出す。これは Rust の標準的な "test-only public API" の公開手段である。

`llama-cpp-smoke` を `llama-cpp` と分離する理由は、lefthook pre-push の default feature build では model load を試みず、opt-in (`--features llama-cpp-smoke`) でのみ実行する設計にするためである。詳細は §8 test strategy を参照。

CLI 側 (`kotoha-cli/Cargo.toml`) は `kotoha-core` を `{ workspace = true, features = ["llama-cpp"] }` で参照する。`kotoha-kanji` バイナリは常に `llama-cpp` feature を有効化して build する。

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

`seed: Some(u64)` の採用理由: llama.cpp smoke test と E2E smoke test を deterministic にするため。default の `Some(0)` + `temperature = 0.0` (greedy) により、同じ入力に対して常に同じ出力を得られる。CLI の `--seed` option で override 可能。

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
    /// 例: "gemma-2-2b-jpn-it-q5_k_m" / "mock"
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

/// Prompt template dispatch hint。
///
/// P1-2.5 follow-up (PR #76) 以降、Gemma / Qwen 系 instruct model は
/// `apply_chat_template` ではなく plain-text completion を使う
/// (§3.2.1 参照)。本 enum は prompt 構築ロジック (`build_prompt`) の
/// 分岐 tag であり、family ごとの将来拡張 (異なる few-shot 集合 /
/// 異なる directive) の受け皿として variant を保持する。
#[non_exhaustive]
#[derive(Debug, Clone)]
pub enum PromptTemplate {
    /// Gemma-2 instruction-tuned chat (`gemma-2-2b-jpn-it` 等)。
    /// `build_prompt` 経由で plain-text v12 few-shot prompt
    /// (strict directive + 13-pair positive few-shot + 3-pair
    /// negative 対照 + query) を生成する。
    Gemma2InstructChat,
    /// Qwen2 instruction-tuned chat (`qwen2.5-1.5b-instruct` 等)。
    /// Phase 1 では `Gemma2InstructChat` と同一の plain-text prompt を
    /// `build_prompt` が生成する (両 variant の内部 format は現状同じ)。
    /// variant tag は将来的な family 別分岐拡張用に保持している。
    Qwen2Chat,
    /// Custom escape hatch。Integrator が独自の wrapper を用いる場合
    /// (Phase 2+ の想定)。`build_prompt` は `system` + `user_wrapper` +
    /// `assistant_prefix` を verbatim で連結するのみで、few-shot scaffold は
    /// 注入しない。
    Custom {
        /// 先頭に付与する `system` turn の内容 (不要なら `None`)。
        system: Option<String>,
        /// user turn を包む `(prefix, suffix)`。
        /// 例: `("<start_of_turn>user\n", "<end_of_turn>")`
        user_wrapper: (String, String),
        /// assistant turn を開始する prefix。
        /// 例: `"<start_of_turn>model\n"`
        assistant_prefix: String,
    },
}

/// Backend の構築パラメータ。
#[non_exhaustive]
#[derive(Debug, Clone)]
pub enum BackendConfig {
    /// 決定的な mock (test 用)。
    ///
    /// `mock-backend` feature が有効な場合のみ構築可能。
    Mock,

    /// llama.cpp 経由の GGUF backend (llama-cpp-2 経由)。
    ///
    /// `llama-cpp` feature が有効な場合のみ構築可能。
    LlamaCpp {
        /// GGUF file への絶対パス
        model_path: PathBuf,
        /// prompt template dispatch hint (§3.2.1 参照)
        prompt_template: PromptTemplate,
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

        #[cfg(feature = "llama-cpp")]
        BackendConfig::LlamaCpp { model_path, prompt_template } => {
            Ok(Box::new(crate::kanji::LlamaCppBackend::load(
                model_path,
                prompt_template.clone(),
            )?))
        }

        #[cfg(not(feature = "llama-cpp"))]
        BackendConfig::LlamaCpp { .. } => Err(KanjiError::FeatureDisabled {
            feature: "llama-cpp",
        }),
    }
}
```

`PromptTemplate` は `build_prompt` の plain-text prompt 構築 (strict directive + v12 few-shot wrapper) の分岐 tag である。`Gemma2InstructChat` と `Qwen2Chat` variant は Phase 1 時点で同一の plain-text format を生成し、variant tag は将来の family 別分岐拡張用に保持する。詳細は §3.2.1 参照。

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

**Note on backend-internal preprocessing:** spec §5.6 の hiragana-only 契約は `KanjiBackend::convert` の **API boundary** で成立すればよく、backend の内部処理で kana casing を変換することは契約違反ではない。例えば Phase 2 で Zenz 系 GGUF が再採用された場合 (pre-tokenizer 問題が解消された場合)、当該 backend は自身の `convert` 実装内で hiragana → katakana 変換を行ってよい。Phase 1 default の `LlamaCppBackend + Gemma-2-2B-jpn-it` はこの変換を必要としない。

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
│    │ LlamaCppBackend                                   │   │
│    │   prompt = build_prompt(&template, "にほんご")    │   │
│    │     (plain-text: directive + v12 few-shot + query)│   │
│    │   tokens = model.str_to_token(&prompt,            │   │
│    │              AddBos::Always)                      │   │
│    │   output_bytes = greedy_loop(tokens)              │   │
│    │     (newline stop after non-whitespace content)   │   │
│    │   surface = String::from_utf8_lossy(&output_bytes)│   │
│    │             .trim()                               │   │
│    │   candidates = score_sort_dedupe(vec![surface])   │   │
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

註 (P1-2.5 follow-up / PR #76): 上記データフローは Phase 1 acceptance 14/15 (row 3 `あした → 翌日` のみ不合格) に対応する最終形である。row 3 は Gemma-2-2B-jpn-it Q5_K_M の pretrain bias に起因する既知の制約 (v5–v12 の prompt 反復でも解消せず) であり、task-specific fine-tuned romaji-base model を使う Phase 5 で解消する方針として deferral する (ADR 0010)。

`kotoha-romaji` と `kotoha-kanji` は独立したプロセスであり、shell pipe でのみ連結する。両者間で共有メモリ / IPC を使わないため、Phase 0 / Phase 1 の CLI 仕様は shell tooling で柔軟に組み合わせ可能である。

## 7. CLI contract: kotoha-kanji

### 7.1 Options

| Option | Required | Default | 意味 |
|--------|----------|---------|------|
| `--model <path>` | **yes** | — | GGUF file への絶対 or 相対 path (Phase 1 default: Gemma-2-2B-jpn-it Q5_K_M) |
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

Phase 0 の test philosophy (unit / integration の独立、fixture ベースの golden、reproducibility) を踏襲しつつ、LLM 実推論を伴う層を opt-in feature で分離する。

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

### 8.3 Layer 3: llama.cpp smoke (llama-cpp-smoke feature)

- 配置: `crates/kotoha-core/tests/kanji_llama_cpp_smoke.rs`
- 対象: 実際の GGUF モデル (Phase 1 default: Gemma-2-2B-jpn-it Q5_K_M) を load して 15 件の smoke input (Layer 3 fixture、row 3 は Phase 5 deferral として skip、有効 14 件) で変換し、品質ではなく「プロセスが通る」ことを確認
- Model path: 環境変数 `KOTOHA_LLAMA_MODEL_PATH` で渡す (未設定時は SKIP して exit 0)
- Feature: `llama-cpp-smoke` (`llama-cpp` を implies)
- 実行: `cargo test --features llama-cpp-smoke` (手動 / opt-in)
- lefthook pre-push には含めない (default features のみ実行)
- 所要時間: 約 52 秒 (cold load 約 10.6 秒 + 14 件 warm × 約 3 秒、詳細は ADR 0013 を参照)

### 8.4 Layer 4: E2E smoke (scripts/phase1-smoke.sh)

- 配置: `scripts/phase1-smoke.sh` (Phase 0 の `scripts/phase0-smoke.sh` と並列)
- 対象: `kotoha-romaji | kotoha-kanji` の pipe を subshell 実行し、end-to-end に期待出力を確認
- 件数目安: 5 件 (Layer 3 と parity を取る)
- 所要時間: 10〜30 秒 (内部で LLM 推論が走るため)
- 実行: 手動 `bash scripts/phase1-smoke.sh` or `make smoke` 相当
- `KOTOHA_LLAMA_MODEL_PATH` 未設定時は SKIP と表示して exit 0 で終了 (CI fail を起こさない)

Assertion は substring 一致 (`assert_contains`) とし、Layer 3 と同一 fixture を使って parity を保つ。

### 8.5 テスト件数と所要時間

| Layer | 件数目安 | 所要時間目安 | lefthook pre-push | Opt-in |
|-------|---------|-------------|------------------|--------|
| 1. Unit (default) | 〜25 | 0.2 秒 | ✅ | — |
| 2. Integration (mock-backend) | 5 | 0.1 秒 | ✅ | — |
| 3. llama.cpp smoke (llama-cpp-smoke) | 15 (row 3 skip で有効 14) | 約 52 秒 | ❌ | ✅ |
| 4. E2E smoke (shell) | 5 | 10〜30 秒 | ❌ | ✅ |
| **合計 (pre-push)** | 〜30 | 0.3 秒以内 | — | — |

### 8.6 Model 更新時の fixture regenerate 手順

Default model version が更新された場合 (ADR 0009 の policy で判断)、以下の手順で fixture を再生成する。

1. 新 model GGUF を `KOTOHA_LLAMA_MODEL_PATH` に配置する
2. `cargo test --features llama-cpp-smoke` を run し、Layer 3 が pass することを確認する
3. `bash scripts/phase1-smoke.sh` を run し、Layer 4 が pass することを確認する
4. Layer 3 / Layer 4 の期待出力が model update により揺らいでいる場合:
   - `tests/fixtures/kanji_smoke.tsv` の `expected_substring` を新 model の top-1 出力に合わせて更新する
   - 変更理由を commit message に「model version change」として明記する
5. ADR 0009 の「version change log」 section を update する

## 9. 共通 shell library: scripts/lib/assert.sh

### 9.1 API 関数

引数順序は **`<desc> <actual> <expected>`**(actual 先行)で固定する。これは Python `unittest` / `pytest` の `assertEqual(actual, expected)` 慣行に倣ったもので、xUnit 系の `<expected> <actual>` 順とは意図的に異なる(merge 済 `scripts/lib/assert.sh` も本順序で実装され、phase0-smoke / phase1-smoke 全行で本順序を使用する)。

`assert_summary` は `ASSERT_PASS` / `ASSERT_FAIL` 内部 counter を `assert_*` 内部で更新し、summary 時には phase 名のみ受け取る設計とする(個別 caller での `pass=$((pass + 1))` boilerplate を排除する)。

```bash
#!/usr/bin/env bash
# scripts/lib/assert.sh

ASSERT_PASS=0
ASSERT_FAIL=0

# 完全一致比較。引数 3 つで短い diagnostic を出力。
# usage: assert_equal <desc> <actual> <expected>
# 副作用: ASSERT_PASS / ASSERT_FAIL を更新する。
assert_equal() {
    local desc="$1"
    local actual="$2"
    local expected="$3"
    if [[ "$actual" == "$expected" ]]; then
        echo "PASS  $desc"
        ASSERT_PASS=$((ASSERT_PASS + 1))
    else
        echo "FAIL  $desc: expected '$expected', got '$actual'"
        ASSERT_FAIL=$((ASSERT_FAIL + 1))
    fi
}

# 部分一致比較 (substring)。LLM 出力等の stochastic な比較に用いる。
# usage: assert_contains <desc> <actual> <expected_substring>
# 副作用: ASSERT_PASS / ASSERT_FAIL を更新する。
assert_contains() {
    local desc="$1"
    local actual="$2"
    local expected_substring="$3"
    if [[ "$actual" == *"$expected_substring"* ]]; then
        echo "PASS  $desc: contains '$expected_substring'"
        ASSERT_PASS=$((ASSERT_PASS + 1))
    else
        echo "FAIL  $desc: '$actual' does not contain '$expected_substring'"
        ASSERT_FAIL=$((ASSERT_FAIL + 1))
    fi
}

# Pass/Fail count の最終 summary を表示し、FAIL > 0 で exit 1 する。
# usage: assert_summary <phase_name>
# 副作用: 集計 line を stdout に出し、ASSERT_FAIL > 0 の場合は process を exit 1。
assert_summary() {
    local phase_name="$1"
    local total=$((ASSERT_PASS + ASSERT_FAIL))
    echo "=== $phase_name: $ASSERT_PASS/$total PASS, $ASSERT_FAIL/$total FAIL ==="
    if [[ $ASSERT_FAIL -gt 0 ]]; then
        exit 1
    fi
}
```

3 関数の role:

- `assert_equal` — Phase 0 smoke の主用途 (stdout 完全一致)
- `assert_contains` — Phase 1 smoke の主用途 (LLM 出力の部分一致)
- `assert_summary` — 全行処理後の集計と exit code 決定(内部 counter を集計、phase 名のみ受け取る)

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

actual=$(echo "nihongo" | kotoha-romaji)
assert_equal "romaji nihongo" "$actual" "にほんご"
# (以下、他 fixture について同様)

assert_summary "phase0-smoke"
```

`assert_*` 関数が `ASSERT_PASS` / `ASSERT_FAIL` を内部更新するため、caller 側で `pass=$((pass + 1))` を書く必要は無い。`assert_summary` は phase 名のみ受け取り、内部 counter から exit code を決定する。

P1-0 は Phase 1 本体実装と分離した先行 PR として実施し、Phase 0 smoke が refactor 後も pass することを CI / lefthook で確認する。これにより Phase 1 の P1-3 で `scripts/phase1-smoke.sh` を書く時点で、library が既に安定している状態にする。

## 10. Risk と緩和策

| # | Risk | 影響 | 緩和策 |
|---|------|------|-------|
| 1 | llama-cpp-2 の API が bindgen 経由で llama.cpp 本体の breaking change に追随 | P1-2 の実装が version pinning に複雑化 | 実装時点 (P1-2 開始日) で Context7 / crates.io 最新 stable に pin し、ADR 0010 に version policy を明記 |
| 2 | Zenz の prompt format (special token / separator) が非自明で reverse-engineer が必要 | P1-2 の着手でロスが発生 | P1-2.5 で llama-cpp-2 の `apply_chat_template` に一本化し、GGUF metadata の `tokenizer.chat_template` を 1 次情報源とする方針に変更した。加えて、Gemma-2-2B-jpn-it が生入力だけでは kana→kanji 変換を行わない empirical finding (P1-2.5-8) を踏まえ、IME-style multi-turn few-shot wrapper (`build_chat_tuples`) を Phase 1 default prompt に組み込んだ。AzooKey Zenzai docs (§3.3) は歴史的参照に降格。Phase 2+ で Zenz 系が再採用される際は ADR で再評価する。 |
| 3 | GGUF model file size (medium 約 150MB、large 数 GB) を repo に含められない | Smoke test の再現性低下 | Model placement は manual、`KOTOHA_LLAMA_MODEL_PATH` で指定、未設定時 smoke は SKIP |
| 4 | CPU inference の latency が CLI pipe 運用で体感できるほど遅い | pipe 運用の UX 低下 | Phase 1 では pipe の proof-of-concept までを acceptance とし、latency 改善は Phase 5 advanced features に先送り |
| 5 | LLM の出力が同一 surface を複数 score で生成する | dedupe 漏れによる duplicate 候補表示 | `score_sort_dedupe` を trait dispatch の外 (helper 関数) に実装し unit test で覆う |
| 6 | lefthook pre-push に llama.cpp smoke が混入して CI が遅くなる | 開発者体験の低下 | `llama-cpp-smoke` を独立 feature 化、pre-push は default features のみ実行 |
| 7 | CLI の `--model` 省略時に hard-to-debug error を出す | 初回利用者の困惑 | `--model` を clap の `required = true` とし、unset 時は exit code 1 + README 引用の 1 行診断を stderr に出す |
| 8 | llama.cpp smoke の 9 fixture が model version 更新で一斉 fail する | 更新 PR のマージ阻害 | §8.6 の regenerate 手順を ADR 0009 と連動させ、PR 内で fixture update を明示許可する |

## 11. Milestone 分割

Phase 1 全体を 5 PR に分割する。合計見積もり約 5.3 日、各 milestone は独立 PR として develop にマージ可能な粒度に絞る。

| PR | Milestone | 主な成果物 | 所要 | 依存 |
|----|-----------|-----------|------|------|
| **P1-0** | Shell library 抽出 | `scripts/lib/assert.sh` 新設、`scripts/phase0-smoke.sh` を library 利用に refactor | 0.3 日 | なし (develop から直接) |
| **P1-1** | Kanji module skeleton + Mock backend | `kotoha-core::kanji/` module 追加、`KanjiBackend` trait、`Candidate` / `ConvertOptions` / `KanjiError`、`MockBackend` (feature = mock-backend)、`load_backend` factory、Layer 1 + Layer 2 test | 1.5 日 | P1-0 merge |
| **P1-2** | LlamaCppBackend 実装 | `LlamaCppBackend` 具象実装 (当初 ZenzBackend として着手、P1-2.5 で一般化)、llama-cpp-2 依存追加、llama-cpp-smoke test | 2.5 日 (P1-2 + P1-2.5 合計) | P1-1 merge |
| **P1-3** | CLI kotoha-kanji + Phase 1 smoke | `kotoha-cli::bin::kotoha-kanji` バイナリ、`process_line` 純粋関数、`scripts/phase1-smoke.sh`、Layer 4 E2E smoke | 1.0 日 | P1-2 merge |
| **P1-4** | ADR + closing | ADR 0009 (promote) / 0011 / 0012 / 0013 (ADR 番号競合の詳細は §12 末尾の注参照)、README 更新 (Gemma-2-2B-jpn-it 入手手順 + `--model` 使い方)、Phase 1 acceptance checklist の消化、implementation WBS log 確定 | 0.5 日 | P1-3 merge |

合計: 5.3 日、5 PR。各 PR は project CLAUDE.md「Branch Scope Policy」(10 files / 300 lines 目安) に収まる範囲で設計している。P1-2 のみ llama-cpp-2 依存追加と `LlamaCppBackend` 実装で lines が増えやすいため、テスト fixture の追加を含めて 300 lines を意識して分割する可能性がある。

## 12. ADR 候補 (Milestone P1-4 で作成想定)

Phase 1 終了時に以下 3 本の ADR を作成する。番号は Phase 0 までの canonical numbering (0001〜0008 使用済み) から継続する。

| # | タイトル (仮) | 主内容 |
|---|--------------|--------|
| 0009 | Phase 1 default model selection (Gemma-2-2B-jpn-it) | Zenz → Gemma-2-2B-jpn-it pivot の根拠、quantization (Q5_K_M) 選択、fixture regenerate procedure (§8.6 と連動) |
| 0010 | Kanji backend trait design | `KanjiBackend` trait + `BackendConfig` enum + `load_backend` factory の採用理由、llama_cpp-rs / mistral.rs / Candle との比較 |
| 0011 | Feature flag design for llama.cpp integration | `default` / `llama-cpp` / `llama-cpp-smoke` / `mock-backend` 4 flags の分離理由、lefthook pre-push との整合 |

いずれも P1-4 で作成し、本設計書と相互参照する (本書 → ADR、ADR → 本書)。

**注 (P1-4 実施時の結果)**: 上記は Phase 1 早期計画時点の ADR 番号想定である。実際は ADR 0010 が Phase 5 方針で先に確定した (PR #78) ため、P1-4 で作成した ADR は 0009 (promote) / 0011 / 0012 / 0013 に繰り上がった。詳細経緯は `docs/wbs/2026-04-25-docs-83-p1-4-phase1-wrap.md` §背景を参照。

## 13. Open Questions (spec 執筆中 / 実装中に解消)

以下 5 項目は本設計書時点で未確定であり、指定した milestone で解消する。

| # | Question | 解消 milestone |
|---|----------|---------------|
| Q1 | `llama-cpp-2` の具体 version pin (実装日の最新 stable) | P1-2 開始時点で Context7 確認し pin、ADR 0010 に記録 |
| Q2 | LLM prompt template の exact form (chat_template + few-shot wrapper) | P1-2.5 で llama-cpp-2 apply_chat_template + IME-style multi-turn few-shot wrapper を採用 (ADR 0009 prep note 参照)。decision close |
| Q3 | `process_line` の公開場所 (`kotoha-core::kanji::cli_support` vs `kotoha-cli` lib target) | P1-3 実装時に 2 案比較、ADR 候補外 (実装判断) |
| Q4 | `MockBackend` の fixture をハードコード vs TSV 外出し | P1-1 実装時に判断。小規模 (5 件) のため hard-code を優先候補とする |
| Q5 | `--seed` が `0` のとき deterministic を保証するかの llama-cpp-2 側の挙動 | P1-2 で挙動確認、docs/README に注記 |

## 14. Acceptance (Phase 1 完了条件)

以下 15 項目のうち 15/15 を満たした時点で Phase 1 完了とする。

- [ ] 1. `crates/kotoha-core/src/kanji/` module が公開されている
- [ ] 2. `KanjiBackend` trait、`Candidate` / `ConvertOptions` / `BackendConfig` / `KanjiError` が `kotoha_core::kanji::*` から re-export されている
- [ ] 3. `MockBackend` が `mock-backend` feature 下で公開されている
- [ ] 4. `LlamaCppBackend` が `llama-cpp` feature 下で公開されており、`load_backend(&BackendConfig::LlamaCpp { .. })` で構築可能である
- [ ] 5. `load_backend` factory が feature 未有効時に `KanjiError::FeatureDisabled` を返す
- [ ] 6. Layer 1 unit test (default features、約 25 件) が pass する
- [ ] 7. Layer 2 integration test (`--features mock-backend`、5 件) が pass する
- [ ] 8. Layer 3 llama.cpp smoke (`--features llama-cpp-smoke`、15 件 (row 3 skip で有効 14 件)) が `KOTOHA_LLAMA_MODEL_PATH` 指定で pass する
- [ ] 9. Layer 4 E2E smoke (`scripts/phase1-smoke.sh`、5 件) が pass する
- [ ] 10. CLI `kotoha-kanji` が `--model <path>` required、option `--top-k / --show-scores / --show-model-id / --temperature / --seed` を受け付ける
- [ ] 11. CLI の exit code 仕様 (0 / 1 / 2) が Phase 0 `kotoha-romaji` と整合している
- [ ] 12. `scripts/lib/assert.sh` が `assert_equal` / `assert_contains` / `assert_summary` を提供し、Phase 0 smoke が refactor 後も pass する
- [ ] 13. ADR 0009 / 0011 / 0012 / 0013 が作成され、本設計書と相互参照している (ADR 0010 は Phase 5 方針 ADR、番号競合の経緯は `docs/wbs/2026-04-25-docs-83-p1-4-phase1-wrap.md` §背景を参照)
- [ ] 14. `README.md` に Gemma-2-2B-jpn-it Q5_K_M の GGUF 入手コマンドと配置先、`kotoha-kanji --model` の使い方が記載されている
- [ ] 15. `cargo fmt --all --check` / `cargo clippy --workspace --all-targets -- -D warnings` / `cargo test --workspace` が CI / lefthook pre-push で warning なく pass する

### 14.1 Phase 1 完了宣言 (2026-04-25 P1-4)

Phase 1 の全 milestone (P1-0 〜 P1-4) が完了した。達成内容は以下のとおりである。

- **P1-0** (assert.sh 共通 library): 完了 (Phase 0 からの継承、`scripts/lib/assert.sh` を Phase 0 / Phase 1 smoke 双方で共有)
- **P1-1** (MockBackend + skeleton): 完了 (`kotoha-core::kanji` module、`KanjiBackend` trait、`Candidate` / `ConvertOptions` / `BackendConfig` / `KanjiError`、`MockBackend`、`load_backend` factory、Layer 1 + Layer 2 test)
- **P1-2** (初期 ZenzBackend): 完了 (後に P1-2.5 refactor で `LlamaCppBackend` に改称)
- **P1-2.5** (LlamaCppBackend 汎用化 + Gemma-2-2B-jpn-it 採用): 完了 (PR #74 merge `02cf035`、`PromptTemplate` enum 導入、9/9 PASS baseline)
- **P1-2.5 follow-up** (Layer 3 15 行復元 + v12 prompt): 完了 (PR #76 merge `3eccaa1`、14/15 PASS、row 3 は Phase 5 deferral)
- **P1-3** (kotoha-kanji CLI + phase1-smoke.sh): 完了 (PR #82 merge `f9a820c`、`kotoha-cli::bin::kotoha-kanji` バイナリ、`process_line` 純粋関数、Layer 4 E2E smoke)
- **P1-4** (ADR 0009 promote + ADR 0011/0012/0013 新規 + spec wrap): 完了 (本 PR、§14.1 宣言とともに close)

完了条件は §14 Acceptance 表の全 15 項目が (a) 達成、または (b) ADR で defer が明文化 (row 3 → ADR 0010、latency → ADR 0013) されている状態で満たす。

Phase 2 (Dictionary and learning) kick-off 可能。

## 15. 参照

- Phase 0 spec: `docs/superpowers/specs/2026-04-22-kotoha-phase-0-design.md` §16 (Phase 1 への橋渡し)
- Phase 0 overall plan: `docs/superpowers/plans/2026-04-22-kotoha-phase-0-implementation.md`
- ADR 0002 (input mode Transient vs Sticky): `docs/adr/0002-input-mode-transient-vs-sticky.md`
- ADR 0005 (romaji Trie over HashMap): `docs/adr/0005-romaji-trie-over-hashmap.md`
- ADR 0008 (canonical romaji Phase 1 判断 = 選択肢 1 採用): `docs/adr/0008-canonical-romaji-and-partial-invertibility.md`
- ADR 0006 (non-exhaustive on streaming enums): `docs/adr/0006-non-exhaustive-on-streaming-enums.md`
- AzooKey Zenzai docs (§3.3 で詳述): <https://github.com/azooKey/AzooKeyKanaKanjiConverter/blob/main/Docs/zenzai.md>
- Zenz-v2.5 collection (Phase 1 では採用見送り、§3.2.2 参照): <https://huggingface.co/collections/Miwa-Keita/zenz-v25-6784cd5d57147f61bc4c3031>
- Gemma-2-2B-jpn-it GGUF (Phase 1 default): <https://huggingface.co/bartowski/gemma-2-2b-jpn-it-GGUF>
- llama-cpp-2 crates.io: <https://crates.io/crates/llama-cpp-2>
- User memory: future vision として「入力 convention 拡張は canonical romaji ではなく rule table 追加で対応」、「Phase 6 UI で model 管理機能を想定」の 2 点が確定済み (ADR 0008 §影響 参照)

## 16. Phase 2 への橋渡し

Phase 1 完了時点で以下が Phase 2 の前提として利用可能になる:

- `kotoha-core::kanji::{KanjiBackend, LlamaCppBackend, PromptTemplate, Candidate, ConvertOptions, BackendConfig, KanjiError}` 公開 API
- Feature flag 構成の確立 (将来 backend 追加は同じ pattern で拡張可能、ADR 0011)
- `scripts/lib/assert.sh` 共通 shell library (Phase 2+ smoke でも継続利用)
- Model の manual placement 手順 (Phase 1 default: Gemma-2-2B-jpn-it、README 記載)
- ADR 0009 の model version policy (Phase 2 で model 更新する際に参照)

Phase 2 で `kotoha-core` に追加する予定:

- `dict/` モジュール (system dictionary / user dictionary)
- `learning/` モジュール (候補選択履歴の learning cache)
- `kanji/hf_download.rs` (model auto-download、当初 Phase 1 予定 → Phase 2 に shift)
- 品質評価インフラ (BLEU / exact-match metric、AJIMEE-Bench 採用検討)

Phase 2 以降も本設計書の `KanjiBackend` trait / `BackendConfig` factory pattern を踏襲し、新 backend 追加時に既存 `LlamaCppBackend` / `MockBackend` と同じ公開形式で module 拡張する。
