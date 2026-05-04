---
feature: p2-a-dictionary-layer
status: implemented
bounded_context: _uncategorized
related_issues: ["#92", "#94", "#100"]
related_prs: []
glossary_refs: ["sudachi-dict", "backend-trait"]
last_reviewed: 2026-05-05
---

# Kotoha Phase 2 P2-A (Dictionary layer) 設計書

本設計書は Phase 2「Dictionary and learning」の最初の milestone P2-A を詳細化する子 spec である。Phase 2 spec(parent-spec)が示す Phase 2 全体スコープの中で、P2-A は Dictionary backend を新設し、`KanjiBackend` trait の 2 つ目の実装(LlamaCpp に続く)として位置づける。Phase 2 spec / ADR 0014 と本設計書の記述が矛盾する箇所は、本設計書が新しい(P2-A brainstorming で確定した最新方針)とする。

## 目次

- [1. 概要](#1-概要)
- [2. スコープ](#2-スコープ)
- [3. 設計判断(brainstorming で確定した 11 項目)](#3-設計判断brainstorming-で確定した-11-項目)
- [4. アーキテクチャ](#4-アーキテクチャ)
- [5. Dictionary 設計](#5-dictionary-設計)
- [6. Test 戦略](#6-test-戦略)
- [7. Dependencies と Feature flags](#7-dependencies-と-feature-flags)
- [8. 付随更新(本 P2-A PR に同梱する別 file 変更)](#8-付随更新本-p2-a-pr-に同梱する別-file-変更)
- [9. 規模見積り](#9-規模見積り)
- [10. 残論点(P2-A 実装中に確定 or 後続 milestone)](#10-残論点p2-a-実装中に確定-or-後続-milestone)
- [11. 参照](#11-参照)

## 1. 概要

P2-A は Phase 2「Dictionary and learning」の最初の milestone であり、Kotoha プロジェクトは P2-A で SudachiDict-core を Rust から runtime load する Dictionary backend を新設する。Kotoha は本 milestone において、Phase 1 で確定した `KanjiBackend` trait の 2 つ目の実装(`LlamaCppBackend` に続く)として `DictionaryBackend` 構造体を導入する。

P2-A は parent-spec(Phase 2 spec)§3 アーキテクチャ / §4 Dictionary 設計 / §6 API と Trait 拡張 / §7 テスト戦略 を詳細化する子 spec として位置づく。Phase 2 spec / ADR 0014 と矛盾する記述は、P2-A brainstorming で empirical に確定した最新方針として本設計書が優先する。具体的には、parent-spec §3.4 で「候補 1 / 候補 2 のいずれかを P2-A で選択」と stub 化していた `BackendConfig` 拡張は、本設計書 §3.3 で「P2-A は集約型 1 variant を追加し、Hybrid 再帰 wrap 型は P2-D で追加する」と確定する。

P2-A の到達目標は以下 4 点である。

- Phase 2 spec G2(敬称 / 固有名詞の System dict 補完)を 530 case golden fixture で 90% pass rate 達成
- Phase 1 14/15 baseline を退行させない(ADR 0014 D1 の hybrid architecture 前提を維持)
- Phase 2 spec §6.3 案 1(kotoha-core 内配置 + feature flag)を採用
- ADR 0014 D4 を「Phase 2 で 2 variants 追加(集約型 P2-A + Hybrid 再帰 wrap 型 P2-D)」に改訂する付随 commit を P2-A PR に含める

## 2. スコープ

### 2.1 In-scope

P2-A は以下 13 項目を実装範囲とする。

1. `crates/kotoha-core/src/dict/` 新規モジュール群(`mod.rs` / `backend.rs` / `engine.rs` / `vocab.rs` / `sudachi_adapter.rs` / `custom_vocab.rs`)
2. `MorphologicalEngine` trait(SudachiAdapter / 将来の Vibrato / Lindera 等を抽象化する境界 trait)
3. `VocabularyLookup` trait(CustomVocab / 将来の UserVocab を抽象化する境界 trait)
4. `DictionaryBackend` 構造体(`KanjiBackend` を実装、`MorphologicalEngine` と `VocabularyLookup` を field に保持)
5. `SudachiAdapter`(`MorphologicalEngine` を実装、sudachi.rs runtime ラッパー)
6. `CustomVocab`(`VocabularyLookup` を実装、`kotoha-dict.tsv` reader)
7. `BackendConfig::Dictionary { config: DictionaryConfig }` variant 追加と `load_backend` factory arm 追加
8. `DictionaryConfig` 構造体(`system_dict_path: PathBuf` / `custom_vocab_path: Option<PathBuf>` 等の engine 非依存 field)
9. `KOTOHA_SYSTEM_DICT_PATH` 環境変数読込ユーティリティ
10. `crates/kotoha-core/resources/kotoha-dict.tsv` 空 fixture(header + curation policy comments のみ、entry なし)
11. `crates/kotoha-core/tests/kanji_dictionary_unit.rs`(Layer 2 integration test)
12. `crates/kotoha-core/tests/kanji_dictionary_golden.rs` + `crates/kotoha-core/tests/fixtures/kanji_dictionary_golden.tsv`(Layer 3 golden、530 cases)
13. `scripts/phase2-smoke.sh` と `tools/p2a-fixture-gen/`(Phase 1 `phase1-smoke.sh` と同 pattern)

### 2.2 Out-of-scope(後続 milestone / Phase に送る)

P2-A は以下を扱わない。

- **User dictionary 永続化(P2-B)**: `add` / `remove` / `list` の CLI サブコマンドと永続化 format(TSV / JSONL / TOML)選定は P2-B の範囲。P2-A は `VocabularyLookup` trait を先出しすることで P2-B の extension path のみ確保する
- **Learning cache(P2-C)**: in-memory LRU + 起動時 load + shutdown save の実装、`(kana_input, chosen_kanji, frequency, last_used_at)` 永続化 format 確定、eviction 閾値 tuning は P2-C の範囲
- **Ranker と Hybrid backend(P2-D)**: Dictionary 候補と LLM 候補を merge / dedupe / rerank する Ranker の実装、`BackendConfig::Hybrid { llm: Box<BackendConfig>, dict, learning }` の追加、初期重み(dict 0.95 / LLM 1.0)の empirical tuning は P2-D の範囲
- **SudachiDict-full(500MB)variant 採用検討**: P2-A default は SudachiDict-core(70MB、76 万 entries 規模)で固定。full への変更は P2-D の golden fixture で recall 不足が判明した場合のみ検討する
- **streaming IME 統合 / personalization ML / cross-device sync / GUI dict editor**: parent-spec §8 と整合し、Phase 3+ / Phase 5+ / Phase 6+ に送る
- **mixed JP/EN 入力(Phase 5)**: Phase 5 spec の P5-A〜P5-D で扱う。P2-A は hiragana 入力のみ対象とする(Phase 1 と同等の入力契約を維持)

## 3. 設計判断(brainstorming で確定した 11 項目)

P2-A brainstorming は以下 11 項目を empirical に確定した。各項目は本 P2-A 実装の根拠として機能し、後続 milestone でも継承する。

### 3.1 Q1: 形態素解析エンジンの選定

P2-A は `sudachi.rs`(WorksApplications, Apache-2.0、git rev `90fd6068c80c` = v0.6.11)を採用する。

- **採用理由**: sudachi.rs は SudachiDict-core を native Rust で読込み可能であり、Phase 5 P5-A PoC が同辞書を採用済(語彙整合性が取れる)。Apache-2.0 licence は OSS 互換性を保つ
- **lindera 棄却理由**: lindera は MeCab 互換 API を Rust で再実装したライブラリであり、辞書として MeCab-IPADIC または UniDic を要求する。UniDic は商用利用制約があるため採用不可(ADR 0014 C4 と整合)
- **vibrato 棄却理由**: vibrato は MeCab 互換の高速形態素解析器だが、SudachiDict をネイティブにサポートしない。SudachiDict の merge 形態(short / middle / long unit)は sudachi.rs のみが正しく扱える
- **SudachiDict→MeCab 変換棄却理由**: SudachiDict を MeCab 形式に変換する script は WorksApplications が公式提供しておらず、変換結果の精度保証が無い。Phase 5 P5-A の data pipeline 整合性も損なわれる

### 3.2 Q2: PR 分割

P2-A は単一 PR として提出する。global CLAUDE.md「Branch Scope Policy」は 20 files / 1000 lines を目安とし、P2-A の総規模見積り(§9)はこの範囲内に収まる(理由: Phase 2 は trait 2 本 + struct 4 本 + test 2 本 + ADR 改訂の structural な追加が密結合しており、機能境界で PR 分割すると review コンテキストが分断される)。

### 3.3 Q3: BackendConfig 拡張順序

P2-A は集約型 1 variant のみを追加する。

- **P2-A で追加**: `BackendConfig::Dictionary { config: DictionaryConfig }`(LLM を含まない pure Dictionary backend)
- **P2-D で追加**: `BackendConfig::Hybrid { llm: Box<BackendConfig>, dict: DictionaryConfig, learning: LearningConfig }`(Phase 5 `KotohaNative` 加入時に再帰 Hybrid で自動対応)
- **ADR 0014 D4 改訂**: 現行 D4 は「P2-A kick-off で候補 1 / 候補 2 のいずれかを選択」と記述しているが、本 P2-A PR で「Phase 2 で 2 variants 追加(集約型 P2-A + Hybrid 再帰 wrap 型 P2-D)」に改訂する付随 commit を含める

### 3.4 Q4: 公開 API の命名(engine 非依存)

P2-A は engine 実装を抽象化するため、公開 API 名は engine 非依存の形で命名する。

| 公開要素 | 命名 | 注記 |
|---|---|---|
| `DictionaryConfig` field | `system_dict_path: PathBuf` | `sudachi_dict_path` ではない |
| 環境変数 | `KOTOHA_SYSTEM_DICT_PATH` | `KOTOHA_SUDACHI_DICT_PATH` ではない |
| 内部 file 名 | `crates/kotoha-core/src/dict/sudachi_adapter.rs` | Phase 1 の `llama_cpp.rs` と同 pattern で内部実装名は engine 名を含む |
| 内部 struct | `SudachiAdapter` | 外部公開しない `pub(crate)` または非公開 |

公開 API を engine 非依存にすることで、将来 vibrato / 別 engine への置換時に config 互換性を維持できる。一方、内部実装ファイル / 構造体名は engine 名を含めることで、Phase 1 `llama_cpp.rs` と同 pattern を保つ。

### 3.5 Q5: MorphologicalEngine trait の先出し

P2-A は `MorphologicalEngine` trait を先出しし、`DictionaryBackend` は `Box<dyn MorphologicalEngine>` を field に保持する。

```rust
/// 形態素解析 engine の抽象境界。
///
/// # Preconditions
/// - 入力は hiragana 文字列(`KanjiBackend` 契約と同じ U+3040..=U+309F + U+30FC)
///
/// # Postconditions
/// - 返値は入力の形態素分割候補列。同一 reading に対し複数 surface があり得る
pub trait MorphologicalEngine {
    fn tokenize(&self, reading: &str) -> Result<Vec<EngineCandidate>, KanjiError>;
    fn engine_id(&self) -> &str;
}
```

trait 先出しの理由は以下 2 点である。

- **unit test 注入容易性**: `MockEngine` を `DictionaryBackend` に注入することで、SudachiDict 不在環境でも Layer 1 unit test が実行可能となる
- **将来の engine 切替**: vibrato / lindera 等への置換時に `DictionaryBackend` の変更を最小化する(Clean Architecture「Interface 依存」/ SOLID DIP)

### 3.6 Q6: VocabularyLookup trait の先出し

P2-A は `VocabularyLookup` trait を先出しし、P2-A では `CustomVocab` のみが本 trait を実装する。P2-B で `UserVocab` が同 trait を実装する extension path を確保する。

```rust
/// User dictionary / Custom vocabulary の lookup 抽象境界。
///
/// # Postconditions
/// - 同一 reading に対し 0 件以上の `VocabEntry` を返す
/// - 返値順序は score 降順(score 同値時の順序は実装依存)
pub trait VocabularyLookup {
    fn lookup(&self, reading: &str) -> Vec<VocabEntry>;
    fn vocab_id(&self) -> &str;
}
```

`DictionaryBackend` は `Vec<Box<dyn VocabularyLookup>>` を field に保持し、複数 vocab source(P2-A: CustomVocab のみ、P2-B 以降: CustomVocab + UserVocab)を統合可能とする。

### 3.7 Q7: kotoha-dict.tsv の初期内容

P2-A は `crates/kotoha-core/resources/kotoha-dict.tsv` を **空で start** する。

- **理由**: SudachiDict-core 76 万 entries の coverage を baseline として trust する。事前に「補完が必要そうな語彙」を speculative に追加すると、SudachiDict と重複したり、文法的多義性を含むことで誤変換を生む可能性がある(`feedback_vocab_grammatical_collision.md` 参照)
- **運用方針**: golden fixture(§6.3)で観察される gap を後続 milestone(P2-B / P2-D)で証拠ベースに埋める。P2-A 時点では header + curation policy comments のみを記載する

### 3.8 Q8: Golden fixture 規模

P2-A は 530 cases の golden fixture を整備する。

| カテゴリ | 件数 | 目的 |
|---|---|---|
| Targeted cases | 30 | parent-spec §7.3 の 4 カテゴリ(敬称 / 人名 / 地名・組織名 / 外来語)を最低担保 |
| Bulk cases | 500 | 5-bucket stratified sampling(reading 文字数: 1-3 / 4-6 / 7-10 / 11-20 / 20+ で各 100 件) |

bulk 500 cases は `tools/p2a-fixture-gen/` で SudachiDict-core からサンプリングして生成する。詳細は §6.3 で記述する。

### 3.9 Q9: Pass rate threshold(段階的締込み)

P2-A は pass rate threshold を以下のとおり段階的に締込む。

| Milestone | Pass rate threshold | 根拠 |
|---|---|---|
| P2-A | 90% | SudachiDict-core baseline + 空 custom vocab で達成可能な水準 |
| P2-D | 95% | Ranker tuning と user vocab 整備で +5% 改善 |
| Phase 5 | 98% | KotohaNative custom model 統合で残 3% を解消 |

pass rate は `category breakdown` + `length bucket breakdown` の 2 軸で集計し、特定 bucket のみ低下するパターンを検出可能にする。

### 3.10 Q10: sudachi.rs version pin

P2-A は sudachi.rs を `rev = "90fd6068c80c"`(v0.6.11)に pin する。

- **pin 方法**: `Cargo.toml` の `[workspace.dependencies]` で git rev pin(`{ git = "https://github.com/WorksApplications/sudachi.rs", rev = "90fd6068c80c" }`)
- **pin 理由**: sudachi.rs は crates.io 公開 version(0.6.x)が古く、最新の SudachiDict v20260116 と互換性に懸念がある。git rev pin で再現性を担保する。Phase 2 closure までは固定し、bump 必要時は別 ADR / ISSUE を起票する

### 3.11 Q11: SudachiDict-core version pin と配布方針

P2-A は SudachiDict-core v20260116(2026-01-16 release)を採用し、manual placement で運用する。

- **配布方針**: parent-spec §4.3 と整合し、bundling せず manual placement とする(Phase 1 GGUF と同 pattern)
- **path 指定**: 環境変数 `KOTOHA_SYSTEM_DICT_PATH` で指定。未設定時は `KanjiError::ModelNotFound` 相当のエラーを返す
- **upstream update 方針**: parent-spec §4.4 の「固定版」方針を継承。Phase 2 closure までは v20260116 を維持し、bump は別 ADR で承認する

## 4. アーキテクチャ

### 4.1 Module 構造

P2-A は以下のディレクトリ / ファイル構造で実装する。

```text
crates/kotoha-core/src/dict/
├── mod.rs              (pub use re-export、DictionaryConfig 定義)
├── backend.rs          (DictionaryBackend struct + KanjiBackend impl)
├── engine.rs           (MorphologicalEngine trait + EngineCandidate)
├── vocab.rs            (VocabularyLookup trait + VocabEntry)
├── sudachi_adapter.rs  (SudachiAdapter: impl MorphologicalEngine)
└── custom_vocab.rs     (CustomVocab: impl VocabularyLookup)

crates/kotoha-core/resources/
└── kotoha-dict.tsv     (空、header + curation policy comments のみ)

crates/kotoha-core/tests/
├── kanji_dictionary_unit.rs       (Layer 2 integration test)
├── kanji_dictionary_golden.rs     (Layer 3 golden、dict-smoke gate)
└── fixtures/
    └── kanji_dictionary_golden.tsv  (530 cases)

scripts/
└── phase2-smoke.sh     (新設、phase1-smoke.sh と同 pattern)

tools/
└── p2a-fixture-gen/    (新設、500-case stratified sampling script)
```

ディレクトリ命名は Phase 1 `crates/kotoha-core/src/kanji/` と同 pattern を踏襲し、`backend.rs` / `mock.rs` / `llama_cpp.rs` の構造を `dict/` 配下に展開する。

### 4.2 Trait 設計(MorphologicalEngine + VocabularyLookup)

P2-A は 2 本の trait を導入する。両 trait は Clean Architecture「Interface 依存」/ SOLID DIP に整合し、`DictionaryBackend` の concrete 実装 lock-in を避ける。

#### 4.2.1 MorphologicalEngine

```rust
/// 形態素解析 engine の抽象境界。
///
/// # Preconditions
/// - `reading` は `KanjiBackend` 契約 §5.6 と同じ hiragana 文字列(U+3040..=U+309F + U+30FC)
/// - `reading.chars().count() <= 128`
///
/// # Postconditions
/// - 返値は `reading` を tokenize した候補列
/// - 同一 reading に対し複数 surface があり得る(homophone)
/// - score は engine 実装が付与(SudachiAdapter は cost を score に変換)
///
/// # Errors
/// - [`KanjiError::Backend`] when tokenization fails inside the engine
pub trait MorphologicalEngine {
    fn tokenize(&self, reading: &str) -> Result<Vec<EngineCandidate>, KanjiError>;
    fn engine_id(&self) -> &str;
}

#[derive(Debug, Clone)]
pub struct EngineCandidate {
    pub surface: String,
    pub reading: String,
    pub score: f32,
}
```

#### 4.2.2 VocabularyLookup

```rust
/// User dictionary / Custom vocabulary の lookup 抽象境界。
///
/// # Preconditions
/// - `reading` は hiragana 文字列(`KanjiBackend` 契約と同じ)
///
/// # Postconditions
/// - 同一 reading に対し 0 件以上の `VocabEntry` を返す
/// - 返値順序は score 降順(score 同値時の順序は実装依存)
pub trait VocabularyLookup {
    fn lookup(&self, reading: &str) -> Vec<VocabEntry>;
    fn vocab_id(&self) -> &str;
}

#[derive(Debug, Clone)]
pub struct VocabEntry {
    pub surface: String,
    pub reading: String,
    pub pos: String,
    pub score: f32,
}
```

両 trait の設計理由は以下のとおりである。

- **Clean Architecture 整合**: `DictionaryBackend` は trait のみに依存し、SudachiAdapter / CustomVocab の concrete 実装に直接依存しない
- **SOLID DIP 整合**: 高レベルモジュール(`DictionaryBackend`)は低レベル詳細(SudachiAdapter)に依存せず、両者は abstraction(`MorphologicalEngine`)に依存する
- **テスト容易性**: `MockEngine` / `MockVocab` を unit test で注入することで、SudachiDict 不在環境でも Layer 1 test が実行可能

### 4.3 DictionaryBackend の構造

P2-A は `DictionaryBackend` を以下のとおり定義する。

```rust
/// SudachiDict ベースの Dictionary backend。
///
/// `KanjiBackend` を実装し、`MorphologicalEngine` と `VocabularyLookup` を
/// field に保持する。Phase 1 の `LlamaCppBackend` に続く 2 つ目の `KanjiBackend`
/// 実装である。
pub struct DictionaryBackend {
    engine: Box<dyn MorphologicalEngine>,
    vocabs: Vec<Box<dyn VocabularyLookup>>,
    model_id: String,
}

impl DictionaryBackend {
    pub fn load(config: &DictionaryConfig) -> Result<Self, KanjiError> {
        let engine = Box::new(SudachiAdapter::load(&config.system_dict_path)?);
        let mut vocabs: Vec<Box<dyn VocabularyLookup>> = Vec::new();
        if let Some(path) = &config.custom_vocab_path {
            vocabs.push(Box::new(CustomVocab::load(path)?));
        }
        let model_id = format!("dictionary({})", engine.engine_id());
        Ok(Self { engine, vocabs, model_id })
    }
}

impl KanjiBackend for DictionaryBackend {
    fn model_id(&self) -> &str { &self.model_id }
    fn convert(&self, input: &str, options: &ConvertOptions) -> Result<Vec<Candidate>, KanjiError> {
        validate_input(input)?;
        let mut raw: Vec<Candidate> = Vec::new();
        for ec in self.engine.tokenize(input)? {
            raw.push(Candidate { surface: ec.surface, score: ec.score });
        }
        for vocab in &self.vocabs {
            for ve in vocab.lookup(input) {
                raw.push(Candidate { surface: ve.surface, score: ve.score });
            }
        }
        Ok(score_sort_dedupe(raw, options.top_k))
    }
}
```

field 構成の理由は以下のとおりである。

- **`Box<dyn MorphologicalEngine>`**: engine は単一(SudachiDict 1 個)を前提とし、Phase 5 で別 engine への切替時にも単一 box で完結する
- **`Vec<Box<dyn VocabularyLookup>>`**: vocab source は複数(P2-A: CustomVocab、P2-B 以降: CustomVocab + UserVocab)を前提とし、Vec で複数注入を許容する
- **`model_id`**: `engine_id()` を含む文字列(例: `"dictionary(sudachi-0.6.11)"`)を初期化時に生成し、`KanjiBackend::model_id()` で返す

### 4.4 BackendConfig 拡張

P2-A は `BackendConfig` enum に 1 variant を追加する。

```rust
#[non_exhaustive]
#[derive(Debug, Clone)]
pub enum BackendConfig {
    Mock,
    LlamaCpp { model_path: PathBuf, prompt_template: PromptTemplate },
    Dictionary { config: DictionaryConfig },  // P2-A 追加
}

#[derive(Debug, Clone)]
pub struct DictionaryConfig {
    pub system_dict_path: PathBuf,
    pub custom_vocab_path: Option<PathBuf>,
}
```

`load_backend` factory に新 arm を追加する。

```rust
pub fn load_backend(config: &BackendConfig) -> Result<Box<dyn KanjiBackend>, KanjiError> {
    match config {
        // ... 既存 Mock / LlamaCpp arm ...

        #[cfg(feature = "dict")]
        BackendConfig::Dictionary { config } => {
            Ok(Box::new(DictionaryBackend::load(config)?))
        }

        #[cfg(not(feature = "dict"))]
        BackendConfig::Dictionary { .. } => Err(KanjiError::FeatureDisabled {
            feature: "dict",
        }),
    }
}
```

`#[non_exhaustive]` enum 拡張は ADR 0011 D2 の方針に整合する。Phase 1 の `BackendConfig::LlamaCpp` 追加と同 pattern で、外部 match site を破壊せず追加可能である。

## 5. Dictionary 設計

### 5.1 SudachiDict-core 配布

P2-A は SudachiDict-core(v20260116、約 70MB、76 万 entries 規模)を採用し、manual placement で配布する。

- **placement 方針**: parent-spec §4.3 と整合し、bundling せず手動配置(Phase 1 GGUF と同 pattern)
- **path 指定**: 環境変数 `KOTOHA_SYSTEM_DICT_PATH` で指定する。未設定時は `KanjiError::ModelNotFound { feature: "system-dict" }` を返す
- **将来の AutoDownload**: MEMORY.md「Model management future vision」の Phase 6 UX で実装予定。Phase 2 範囲では manual placement のみ
- **README 追記**: 本 P2-A PR に「SudachiDict-core 取得手順 + `KOTOHA_SYSTEM_DICT_PATH` 設定例」を README.md へ追記する付随 commit を含める(§8 参照)

### 5.2 Custom vocabulary

P2-A は `crates/kotoha-core/resources/kotoha-dict.tsv` を空で start する(§3.7 Q7 参照)。

- **schema**: SudachiDict と同一の `(surface, reading, pos, score)` を採用する。TSV header は `# surface<TAB>reading<TAB>pos<TAB>score`
- **curation policy comments**: ファイル冒頭に「kotoha-dict.tsv は SudachiDict baseline では補完できない語彙のみを追加する。speculative な追加は文法的多義性を生むため避ける。golden fixture で観察される gap を証拠として追加する」旨を記載する
- **merge 戦略**: `DictionaryBackend::convert` 内で `engine.tokenize` 結果と `vocab.lookup` 結果を 1 本の `Vec<Candidate>` に連結し、`score_sort_dedupe` で同一 surface を排除する(Phase 1 と同一 helper を流用)
- **文法的多義性チェック**: `feedback_vocab_grammatical_collision.md` の知見を踏まえ、新規 entry 追加時は「同 reading で SudachiDict が既に出している surface を上書きしないか」を P2-B 以降で機械的にチェックする(P2-A は空 start のため適用機会無し)

```tsv
# kotoha-dict.tsv — Kotoha custom vocabulary
# Schema: surface<TAB>reading<TAB>pos<TAB>score
# Curation policy:
#   1. SudachiDict-core baseline で recall 可能な語彙は追加しない
#   2. golden fixture で観察される gap のみを証拠ベースで追加する
#   3. 同 reading で SudachiDict が出す surface を上書きする entry は P2-B 以降で機械的多義性チェックを通すこと
# (P2-A: 空 start)
```

## 6. Test 戦略

### 6.1 Layer 1: pure unit test

P2-A は Layer 1 として `MockEngine` + `MockVocab` を注入した pure unit test を 33〜42 件程度実装する。

| 対象 | 件数目安 | 内容 |
|---|---|---|
| `MorphologicalEngine` trait contract | 6〜8 | tokenize 結果が score 降順、empty 入力で empty 返却、invalid 入力で `KanjiError::Backend` |
| `VocabularyLookup` trait contract | 5〜7 | lookup 結果が score 降順、未 hit で empty Vec、複数 entry 返却 |
| `DictionaryBackend::convert` 統合動作 | 12〜15 | engine + vocab の merge、score_sort_dedupe 適用、top_k truncate、空入力 |
| `DictionaryConfig` 構築 | 4〜5 | path 指定、custom_vocab None / Some、env var 読込 |
| `CustomVocab::load` TSV parse | 6〜7 | header skip、空ファイル、不正 schema、UTF-8 BOM |

すべて `#[cfg(test)]` で同ファイル内に配置する(Phase 1 `backend.rs` 末尾の `mod tests` と同 pattern)。

### 6.2 Layer 2: integration / factory

P2-A は Layer 2 として `crates/kotoha-core/tests/kanji_dictionary_unit.rs` に integration test を 6〜10 件実装する。

| 対象 | 内容 |
|---|---|
| `load_backend` dispatch | `BackendConfig::Dictionary { ... }` が `DictionaryBackend` を返す |
| feature flag 無効時 | `dict` feature off で `KanjiError::FeatureDisabled` 返却 |
| MockEngine 統合 | MockEngine + 空 vocab で end-to-end |
| MockEngine + MockVocab | 両者 hit 時の merge 動作 |
| 同一 surface 衝突 | engine と vocab で同じ surface を返した場合の dedupe |
| top_k = 0 / 大値 | edge case の境界動作 |

Layer 2 は SudachiDict 不在環境でも実行可能とし、`dict` feature は MockEngine 経由で部分的に検証する。

### 6.3 Layer 3: golden fixture(statistical evaluation)

P2-A は 530 cases の golden fixture を `crates/kotoha-core/tests/fixtures/kanji_dictionary_golden.tsv` に配置し、`crates/kotoha-core/tests/kanji_dictionary_golden.rs` で実行する。

#### 6.3.1 Fixture 構成

| カテゴリ | 件数 | 内訳 |
|---|---|---|
| Targeted | 30 | 敬称 10 + 人名 10 + 地名・組織名 5 + 外来語 5(parent-spec §7.3 を踏襲) |
| Bulk(stratified) | 500 | 5-bucket × 100 件 |

bulk の 5-bucket stratified sampling は reading 文字数を bucket 軸とする。

| Bucket | reading 文字数 | 件数 |
|---|---|---|
| B1 | 1〜3 | 100 |
| B2 | 4〜6 | 100 |
| B3 | 7〜10 | 100 |
| B4 | 11〜20 | 100 |
| B5 | 20+ | 100 |

B5(20+ 文字)は自然データの scarcity が懸念されるため、不足時は manual concat / curated list で fallback する(§10 残論点参照)。

#### 6.3.2 Pass rate threshold(段階的締込み)

P2-A の dict-smoke gate は pass rate 90% を hard threshold とする(§3.9 Q9 参照)。

#### 6.3.3 Test runner の出力形式

`kanji_dictionary_golden.rs` は実行結果を以下の 2 軸で集計し、stdout に出力する。

```text
=== Category breakdown ===
Honorific:     10/10  (100.0%)
Personal name:  9/10  ( 90.0%)
Place/Org:      4/5   ( 80.0%)
Loanword:       5/5   (100.0%)
Targeted:      28/30  ( 93.3%)

=== Length bucket breakdown ===
B1 (1-3):    95/100 (95.0%)
B2 (4-6):    92/100 (92.0%)
B3 (7-10):   88/100 (88.0%)
B4 (11-20):  85/100 (85.0%)
B5 (20+):    80/100 (80.0%)
Bulk:       440/500 (88.0%)

=== Total ===
PASS: 468/530 (88.3%)  -- threshold 90% NOT MET
```

特定 bucket のみ pass rate が低下するパターンを検出可能とすることで、後続 milestone での改善対象を可視化する。

#### 6.3.4 Fixture 生成手順

`tools/p2a-fixture-gen/` 配下に Python(uv プロジェクト)で 500-case bulk fixture 生成 script を配置する。Phase 5 P5-A `tools/p5a-data-pipeline/`(MEMORY.md 参照)と同 pattern を踏襲する。

- input: SudachiDict-core(v20260116)
- output: `crates/kotoha-core/tests/fixtures/kanji_dictionary_golden.tsv` の bulk 500 行
- sampling: 各 bucket から `random.sample` で 100 件抽出、再現性のため seed 固定

### 6.4 Phase 1 14/15 regression 防止

P2-A は Phase 1 の Layer 3 smoke fixture 14/15 baseline を退行させない。

- **構造的根拠**: P2-A の変更箇所は `BackendConfig` enum への variant 追加と `load_backend` factory の arm 追加のみであり、既存の `Mock` / `LlamaCpp` arm は不変
- **gate 方針**: pre-push lefthook gate で workspace 全 test を実行し、Phase 1 の `crates/kotoha-core/tests/kanji_llama_cpp_smoke.rs`(14/15 baseline)を継続 PASS させる
- **`#[non_exhaustive]` 整合**: ADR 0011 D2 の拡張点を使用するため、外部 match site の破壊は構造的に発生しない

## 7. Dependencies と Feature flags

### 7.1 sudachi.rs git dep

P2-A は workspace root の `[workspace.dependencies]` に sudachi.rs を git rev pin で追加する。

```toml
[workspace.dependencies]
sudachi = { git = "https://github.com/WorksApplications/sudachi.rs", rev = "90fd6068c80c" }
```

- **licence**: Apache-2.0(Kotoha OSS 互換)
- **alternatives 棄却理由**: §3.1 Q1 参照(lindera / vibrato / SudachiDict→MeCab 変換のいずれも採用不可)

`crates/kotoha-core/Cargo.toml` は以下のとおり参照する。

```toml
[dependencies]
sudachi = { workspace = true, optional = true }
```

### 7.2 Cargo features

P2-A は `crates/kotoha-core/Cargo.toml` に 2 個の feature を追加する。

```toml
[features]
default = []
dict = ["dep:sudachi"]
dict-smoke = ["dict"]
```

- **`dict`**: SudachiDict 連携を有効化する optional dependency 隔離 feature。default = [] を維持し、ADR 0012 D5「default = []」厳守方針に整合する
- **`dict-smoke`**: Layer 3 golden fixture を gate する feature。`dict` を含む。CI / lefthook では opt-in で有効化する

`scripts/phase2-smoke.sh` は `dict-smoke` feature を有効化して `cargo test --features dict-smoke` を実行する(Phase 1 `phase1-smoke.sh` の `llama-cpp-smoke` と同 pattern)。

### 7.3 lefthook 影響

sudachi.rs git dep を追加することで、初回 `cargo build` の dep fetch + compile が 30〜60 秒程度発生する。incremental build 時は約 5 秒以内に収束する見込み。

Phase 1 の llama-cpp-2 が C++ build chain を含む(初回 2〜5 分、incremental 30 秒)のと比較すると、sudachi.rs は pure Rust で軽量である。pre-push gate のレスポンスは Phase 1 と同等以下に保てる見込み。

## 8. 付随更新(本 P2-A PR に同梱する別 file 変更)

P2-A 実装と同一 PR に以下 4 件の docs 更新を含める。

| 対象 | 内容 | 行数目安 |
|---|---|---|
| `docs/adr/0014-phase-2-dictionary-layer-architecture.md` D4 改訂 | 「P2-A kick-off で候補 1 / 候補 2 を選択」→「Phase 2 で 2 variants 追加(集約型 P2-A + Hybrid 再帰 wrap 型 P2-D)」 | +30 |
| `docs/specs/_uncategorized/kotoha-phase-2.md` 同期更新 | §3.4 / §4.2 / §6.3 / §7.3 を P2-A 確定方針に同期 | +95 |
| `README.md` SudachiDict-core 取得手順 | manual placement 手順 + `KOTOHA_SYSTEM_DICT_PATH` 設定例 | +30 |
| `$OBSIDIAN_VAULT_DIR/glossary/` 用語追加 | MorphologicalEngine / VocabularyLookup / SudachiDict / 形態素解析 | +10 |

ADR 0014 と Phase 2 spec の改訂は本 P2-A PR の最後の commit にまとめ、設計判断の根拠として spec / ADR と実装が同一 PR で同期する構造とする。

## 9. 規模見積り

P2-A の総規模は概ね 950 行 / 17 files に収まる見込みである。Branch Scope Policy(20 files / 1000 lines、§3.2 Q2 参照)の範囲内に収まる。

| 区分 | 行数目安 | files |
|---|---|---|
| `dict/` 6 modules | 540 | 6 |
| `tests/kanji_dictionary_unit.rs` + `kanji_dictionary_golden.rs` | 180 | 2 |
| `kanji_dictionary_golden.tsv` (530 cases) | (data) | 1 |
| `kotoha-dict.tsv`(空 + comments) | 15 | 1 |
| `tools/p2a-fixture-gen/`(Python) | 80 | 3 |
| `scripts/phase2-smoke.sh` | 25 | 1 |
| `Cargo.toml` 修正(workspace + kotoha-core) | 15 | 2 |
| ADR 0014 D4 改訂 | 30 | 1 |
| Phase 2 spec 同期更新 | 95 | 1 |
| README.md 追記 | 30 | 1 |
| `glossary.md` 追記 | 10 | 1 |
| **合計** | **約 1020(うち実装は約 940)** | **17** |

docs 改訂(ADR 0014 / Phase 2 spec / README / glossary)を別 commit にすれば実装行数は約 940 行となり、PR Review Matrix(global CLAUDE.md)の Medium tier(≤15 files AND ≤600 lines)を超過する。Large tier(Medium 超過 + Branch Scope Policy 上限内)の team-review(全 5 次元)+ owasp-security + secrets-check + security-scanning:security-sast + pr-review-toolkit:review-pr を適用する。

## 10. 残論点(P2-A 実装中に確定 or 後続 milestone)

P2-A 着手時点で未確定の論点を以下に列挙する。

- **Learning cache 永続化 format(P2-C 範囲)**: parent-spec §5.2 の TOML / JSONL / TSV 候補。P2-A では決定不要だが、`VocabularyLookup` trait の interface は将来 Learning cache の hit bonus 加算が可能な形を想定する
- **Ranker 重み tuning(P2-D 範囲)**: parent-spec §3.3 の dict 0.95 / LLM 1.0 初期重み。P2-D で 530 case golden fixture の pass rate を KPI に empirical tuning する
- **Phase 5 KotohaNative integration plan(Phase 5 kick-off 範囲)**: ADR 0014 D6 / parent-spec §9 Q6 と相互参照。P2-A の trait 設計(MorphologicalEngine / VocabularyLookup)は Phase 5 でも継承可能な形で先出ししている
- **B5(20+ 文字)bucket の自然データ scarcity 対応**: P2-A 実装中に判断する。SudachiDict-core から 100 件サンプリング不可な場合、`tools/p2a-fixture-gen/` で manual concat(短語連結)/ curated list で fallback する。fallback 採用時は fixture file の冒頭コメントに記録する

## 11. 参照

### 11.1 上位 spec / ADR

- 上位 spec(parent-spec): [`docs/specs/_uncategorized/kotoha-phase-2.md`](./2026-04-25-kotoha-phase-2-design.md)
- ADR 0014(Phase 2 dictionary layer architecture、本 P2-A で D4 改訂): [`docs/adr/0014-phase-2-dictionary-layer-architecture.md`](../../adr/0014-phase-2-dictionary-layer-architecture.md)
- ADR 0011(`#[non_exhaustive]` enum 拡張の根拠): [`docs/adr/0011-kanji-backend-trait-design.md`](../../adr/0011-kanji-backend-trait-design.md)
- ADR 0012(`default = []` feature flag 方針): [`docs/adr/0012-feature-flag-design-for-llama-cpp.md`](../../adr/0012-feature-flag-design-for-llama-cpp.md)
- ADR 0010(Phase 5 KotohaNative integration の継承先): [`docs/adr/0010-kotoha-custom-romaji-base-model.md`](../../adr/0010-kotoha-custom-romaji-base-model.md)

### 11.2 関連 memory

- Clean Architecture / SOLID 整合の feedback: `~/.claude/projects/-home-kohshiro-develops-student-kotoha-ime/memory/feedback_clean_architecture_solid.md`
- Vocab 文法的多義性 feedback: `~/.claude/projects/-home-kohshiro-develops-student-kotoha-ime/memory/feedback_vocab_grammatical_collision.md`

### 11.3 外部参照

- sudachi.rs: <https://github.com/WorksApplications/sudachi.rs>(Apache-2.0、v0.6.11)
- SudachiDict-core: <https://github.com/WorksApplications/SudachiDict>(Apache-2.0、v20260116)
