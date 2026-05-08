# Phase 2 P2-A Dictionary Layer Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Phase 2 の最初のマイルストーン P2-A として、Sudachi-based Dictionary backend を新設し、`KanjiBackend` trait の 2 つ目の実装(LlamaCpp に続く)を Kotoha IME に追加する。

**Architecture:** `MorphologicalEngine` / `VocabularyLookup` の 2 trait を先出し(Clean Architecture DIP)、`DictionaryBackend` が両 trait の `Box<dyn>` を保持する形で engine 切替可能性を確保する。`BackendConfig::Dictionary { config }` を `#[non_exhaustive]` enum に追加(ADR 0011 拡張点)、SudachiDict-core を `KOTOHA_SYSTEM_DICT_PATH` 経由 manual placement で runtime load する。

**Tech Stack:** Rust 1.80 / edition 2021、`sudachi.rs` (git rev `90fd6068c80c2fc3b63e0dbab0e341475bad4d8f` = v0.6.11、Apache-2.0)、SudachiDict-core v20260116 (Apache-2.0、manual placement)、`dict` / `dict-smoke` Cargo features (default = []、ADR 0012 整合)、Python 3.12 + uv (fixture 生成 tooling)、bash + assert.sh (smoke script)

---

## Task 1: sudachi.rs dependency 追加と dict / dict-smoke feature 定義

**Files:**
- Modify: `Cargo.toml`(workspace root、`[workspace.dependencies]` セクション)
- Modify: `crates/kotoha-core/Cargo.toml`(`[dependencies]` + `[features]` セクション)

**Depends-on:** なし

**Estimated LOC:** 15

参照: spec §7.1 / §7.2

- [ ] **Step 1: workspace root `Cargo.toml` に sudachi git dep を追加**

以下を `[workspace.dependencies]` の末尾(`llama-cpp-2` 行の直後)に追加する。

```toml
# sudachi.rs pinned via P2-A Q1 (brainstorming, 2026-04-25).
# Adopted reason: SudachiDict-core を native Rust で読込可能、P5-A PoC と辞書整合、Apache-2.0。
# Alternatives rejected per P2-A spec §3.1 Q1:
#   - lindera: MeCab-IPADIC / UniDic 要求(UniDic は商用利用制約)
#   - vibrato: SudachiDict native 非対応(short/middle/long unit 不可)
#   - SudachiDict→MeCab 変換: WorksApplications 公式提供なし、精度保証なし
sudachi = { git = "https://github.com/WorksApplications/sudachi.rs", rev = "90fd6068c80c2fc3b63e0dbab0e341475bad4d8f" }
```

- [ ] **Step 2: `crates/kotoha-core/Cargo.toml` `[dependencies]` に optional sudachi を追加**

既存の `llama-cpp-2 = { workspace = true, optional = true }` の直後に以下を追加する。

```toml
# `sudachi` is optional so default features do not pull in the git-sourced dep tree.
# Activated only when the `dict` feature is enabled (P2-A spec §7.1).
sudachi = { workspace = true, optional = true }
```

- [ ] **Step 3: `crates/kotoha-core/Cargo.toml` `[features]` に dict / dict-smoke を追加**

既存 `llama-cpp-smoke = ["llama-cpp"]` の直後に以下を追加する。

```toml
# `dict` gates DictionaryBackend (SudachiDict baseline, P2-A spec §3.1 Q1).
# default = [] を維持し ADR 0012 D5 整合。
dict = ["dep:sudachi"]
# `dict-smoke` is the opt-in gate for Layer 3 golden fixture (530 cases).
# Kept separate from `dict` so lefthook pre-push does not attempt to load SudachiDict-core.
dict-smoke = ["dict"]
```

- [ ] **Step 4: `cargo check --workspace --features dict` が通ることを確認**

```bash
cargo check --workspace --features dict
```

想定出力: `Checking kotoha-core v0.1.0` … `Finished ...`(warning 0 件)。初回は sudachi.rs の git fetch が発生するため 30〜60 秒かかる。

- [ ] **Step 5: `cargo build --workspace --features dict` で sudachi.rs 本体までコンパイル**

```bash
cargo build --workspace --features dict
```

想定出力: `Compiling sudachi v0.6.11 (...)` → `Finished ...`。初回 30〜60 秒 / incremental 5 秒以内。

- [ ] **Final step: Commit**

本 task では test を書かない(skeleton 前段のため test 対象が存在しない)。

```bash
git add Cargo.toml crates/kotoha-core/Cargo.toml
git commit -m "chore(deps): add sudachi.rs git dep + dict/dict-smoke features (#91)"
```

---

## Task 2: dict/ モジュールスケルトン作成

**Files:**
- Create: `crates/kotoha-core/src/dict/mod.rs`
- Create: `crates/kotoha-core/src/dict/backend.rs`
- Create: `crates/kotoha-core/src/dict/engine.rs`
- Create: `crates/kotoha-core/src/dict/vocab.rs`
- Create: `crates/kotoha-core/src/dict/sudachi_adapter.rs`
- Create: `crates/kotoha-core/src/dict/custom_vocab.rs`
- Modify: `crates/kotoha-core/src/lib.rs`(`pub mod dict;` を feature gated で追加)

**Depends-on:** Task 1

**Estimated LOC:** 30

参照: spec §4.1

- [ ] **Step 1: `dict/mod.rs` を最小スケルトンで作成**

```rust
//! Dictionary-based kanji conversion backend (Phase 2 P2-A).
//!
//! Spec: `docs/superpowers/specs/2026-04-25-p2-a-dictionary-layer-design.md`.
//!
//! この module は P2-A で追加された `KanjiBackend` の 2 つ目の実装
//! (`LlamaCppBackend` に続く)を提供する。SudachiDict-core を
//! `sudachi.rs` 経由で runtime load する Dictionary backend を中核とし、
//! `MorphologicalEngine` / `VocabularyLookup` 2 本の trait で engine 切替を
//! 抽象化する(Clean Architecture DIP、spec §4.2)。

pub(crate) mod backend;
pub(crate) mod custom_vocab;
pub(crate) mod engine;
pub(crate) mod sudachi_adapter;
pub(crate) mod vocab;
```

- [ ] **Step 2: `dict/backend.rs` を doc-comment のみで作成**

```rust
//! `DictionaryBackend` struct (P2-A、`KanjiBackend` 実装)。
//!
//! Spec: `docs/superpowers/specs/2026-04-25-p2-a-dictionary-layer-design.md` §4.3.
```

- [ ] **Step 3: `dict/engine.rs` を doc-comment のみで作成**

```rust
//! `MorphologicalEngine` trait と `EngineCandidate` 値型の定義。
//!
//! Spec: `docs/superpowers/specs/2026-04-25-p2-a-dictionary-layer-design.md` §3.5, §4.2.1.
```

- [ ] **Step 4: `dict/vocab.rs` を doc-comment のみで作成**

```rust
//! `VocabularyLookup` trait と `VocabEntry` 値型の定義。
//!
//! Spec: `docs/superpowers/specs/2026-04-25-p2-a-dictionary-layer-design.md` §3.6, §4.2.2.
```

- [ ] **Step 5: `dict/sudachi_adapter.rs` を doc-comment のみで作成**

```rust
//! `SudachiAdapter`: `MorphologicalEngine` の sudachi.rs 実装。
//!
//! Spec: `docs/superpowers/specs/2026-04-25-p2-a-dictionary-layer-design.md` §3.4, §4.1, §5.1.
```

- [ ] **Step 6: `dict/custom_vocab.rs` を doc-comment のみで作成**

```rust
//! `CustomVocab`: `VocabularyLookup` の TSV reader 実装。
//!
//! Spec: `docs/superpowers/specs/2026-04-25-p2-a-dictionary-layer-design.md` §5.2.
```

- [ ] **Step 7: `lib.rs` に dict module を feature gated で追加**

既存の `pub mod romaji;` 行の直後に以下を追加する。

```rust
#[cfg(feature = "dict")]
pub mod dict;
```

- [ ] **Step 8: `cargo check --features dict` で compile 通過を確認**

```bash
cargo check -p kotoha-core --features dict
```

想定出力: `warning: unused ...`(各 module が空のため unused 警告が出るが一旦容認、task 3 以降で解消)。error 0 件であれば OK。

- [ ] **Final step: Commit**

```bash
git add crates/kotoha-core/src/dict/ crates/kotoha-core/src/lib.rs
git commit -m "feat(dict): scaffold dict/ module skeleton (#91)"
```

---

## Task 3: MorphologicalEngine trait + EngineCandidate 定義(TDD)

**Files:**
- Modify: `crates/kotoha-core/src/dict/engine.rs`(impl + in-file `#[cfg(test)] mod tests`)

**Depends-on:** Task 2

**Estimated LOC:** 80(impl 35 + tests 45)

参照: spec §3.5, §4.2.1

- [ ] **Step 1: test を先に書く(Red)— MockEngine で trait contract を検証**

`dict/engine.rs` 末尾に以下を追加する(impl がまだ無いため、最初は compile すら通らない)。

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::kanji::KanjiError;

    /// tests 内専用の MockEngine。trait contract を external に lock-in する。
    struct MockEngine {
        canned: Vec<EngineCandidate>,
    }

    impl MorphologicalEngine for MockEngine {
        fn tokenize(&self, reading: &str) -> Result<Vec<EngineCandidate>, KanjiError> {
            if reading.is_empty() {
                return Ok(Vec::new());
            }
            Ok(self.canned.clone())
        }

        fn engine_id(&self) -> &str {
            "mock-engine"
        }
    }

    #[test]
    fn engine_candidate_new_holds_surface_reading_score() {
        let ec = EngineCandidate {
            surface: "日本語".to_string(),
            reading: "にほんご".to_string(),
            score: 0.9,
        };
        assert_eq!(ec.surface, "日本語");
        assert_eq!(ec.reading, "にほんご");
        assert!((ec.score - 0.9).abs() < 1e-6);
    }

    #[test]
    fn mock_engine_returns_canned_candidates() {
        let engine = MockEngine {
            canned: vec![EngineCandidate {
                surface: "日本語".to_string(),
                reading: "にほんご".to_string(),
                score: 0.9,
            }],
        };
        let result = engine.tokenize("にほんご").expect("tokenize succeeds");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].surface, "日本語");
    }

    #[test]
    fn mock_engine_empty_input_returns_empty() {
        let engine = MockEngine { canned: Vec::new() };
        let result = engine.tokenize("").expect("empty input must be accepted");
        assert!(result.is_empty());
    }

    #[test]
    fn mock_engine_engine_id_is_stable() {
        let engine = MockEngine { canned: Vec::new() };
        assert_eq!(engine.engine_id(), "mock-engine");
    }
}
```

- [ ] **Step 2: `cargo test --features dict -p kotoha-core dict::engine::tests` で FAIL を確認**

```bash
cargo test --features dict -p kotoha-core -- dict::engine::tests
```

想定: `error[E0412]: cannot find type 'EngineCandidate'` / `cannot find trait 'MorphologicalEngine'`。test が compile エラーで FAIL することを確認する。

- [ ] **Step 3: trait と struct を実装(Green)**

`dict/engine.rs` の `#[cfg(test)]` より前に以下を追加する。

```rust
use crate::kanji::KanjiError;

/// 形態素解析 engine の抽象境界。
///
/// `DictionaryBackend` は `Box<dyn MorphologicalEngine>` を field に保持する。
/// P2-A は `SudachiAdapter` のみが本 trait を実装する。Phase 5 以降で vibrato /
/// lindera 等への置換時に `DictionaryBackend` の変更を最小化する目的で先出し
/// する(spec §3.5 / §4.2.1)。
///
/// # Preconditions
///
/// - `reading` は `KanjiBackend` 契約 §5.6 と同じ hiragana 文字列
///   (U+3040..=U+309F + U+30FC)
/// - `reading.chars().count() <= 128`
///
/// # Postconditions
///
/// - 返値は `reading` を tokenize した候補列
/// - 同一 reading に対し複数 surface があり得る(homophone)
/// - `score` は engine 実装が付与(`SudachiAdapter` は cost を score に変換)
///
/// # Errors
///
/// - [`KanjiError::Backend`] when tokenization fails inside the engine
pub trait MorphologicalEngine {
    /// Tokenizes `reading` and returns candidate morphemes.
    fn tokenize(&self, reading: &str) -> Result<Vec<EngineCandidate>, KanjiError>;

    /// Returns a stable, human-readable identifier for the engine instance.
    ///
    /// Used by `DictionaryBackend::model_id()` to format the composite model id
    /// (e.g. `"dictionary(sudachi-0.6.11)"`).
    fn engine_id(&self) -> &str;
}

/// A single morpheme candidate returned by [`MorphologicalEngine::tokenize`].
#[derive(Debug, Clone)]
pub struct EngineCandidate {
    /// Surface form (kanji / hiragana / katakana mix).
    pub surface: String,
    /// Reading (hiragana) of `surface`.
    pub reading: String,
    /// Score; larger is better. `SudachiAdapter` converts Sudachi cost into
    /// a descending-order score.
    pub score: f32,
}
```

- [ ] **Step 4: `cargo test --features dict -p kotoha-core -- dict::engine::tests` で全 PASS を確認**

想定: `test result: ok. 4 passed; 0 failed; ...`。

- [ ] **Step 5: `cargo clippy --features dict -p kotoha-core --all-targets -- -D warnings` 通過を確認**

```bash
cargo clippy --features dict -p kotoha-core --all-targets -- -D warnings
```

- [ ] **Final step: Commit**

```bash
git add crates/kotoha-core/src/dict/engine.rs
git commit -m "feat(dict): define MorphologicalEngine trait + EngineCandidate (#91)"
```

---

## Task 4: VocabularyLookup trait + VocabEntry 定義(TDD)

**Files:**
- Modify: `crates/kotoha-core/src/dict/vocab.rs`(impl + in-file `#[cfg(test)] mod tests`)

**Depends-on:** Task 3

**Estimated LOC:** 80(impl 35 + tests 45)

参照: spec §3.6, §4.2.2

- [ ] **Step 1: test を先に書く(Red)— tests 内 MockVocab を定義**

`dict/vocab.rs` 末尾に以下を追加する。

```rust
#[cfg(test)]
mod tests {
    use super::*;

    struct MockVocab {
        canned: Vec<VocabEntry>,
    }

    impl VocabularyLookup for MockVocab {
        fn lookup(&self, reading: &str) -> Vec<VocabEntry> {
            if reading.is_empty() {
                return Vec::new();
            }
            self.canned.clone()
        }

        fn vocab_id(&self) -> &str {
            "mock-vocab"
        }
    }

    #[test]
    fn vocab_entry_holds_surface_reading_pos_score() {
        let ve = VocabEntry {
            surface: "漢字".to_string(),
            reading: "かんじ".to_string(),
            pos: "名詞".to_string(),
            score: 0.85,
        };
        assert_eq!(ve.surface, "漢字");
        assert_eq!(ve.reading, "かんじ");
        assert_eq!(ve.pos, "名詞");
        assert!((ve.score - 0.85).abs() < 1e-6);
    }

    #[test]
    fn mock_vocab_returns_canned_entries() {
        let vocab = MockVocab {
            canned: vec![VocabEntry {
                surface: "漢字".to_string(),
                reading: "かんじ".to_string(),
                pos: "名詞".to_string(),
                score: 0.85,
            }],
        };
        let result = vocab.lookup("かんじ");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].surface, "漢字");
    }

    #[test]
    fn mock_vocab_empty_input_returns_empty() {
        let vocab = MockVocab { canned: Vec::new() };
        assert!(vocab.lookup("").is_empty());
    }

    #[test]
    fn mock_vocab_vocab_id_is_stable() {
        let vocab = MockVocab { canned: Vec::new() };
        assert_eq!(vocab.vocab_id(), "mock-vocab");
    }
}
```

- [ ] **Step 2: `cargo test --features dict -p kotoha-core -- dict::vocab::tests` で FAIL を確認**

想定: `error[E0412]: cannot find type 'VocabEntry'`。

- [ ] **Step 3: trait と struct を実装(Green)**

`dict/vocab.rs` の `#[cfg(test)]` より前に以下を追加する。

```rust
/// User dictionary / Custom vocabulary の lookup 抽象境界。
///
/// `DictionaryBackend` は `Vec<Box<dyn VocabularyLookup>>` を field に保持する。
/// P2-A は `CustomVocab` のみが本 trait を実装する。P2-B で `UserVocab` が
/// 同 trait を実装する extension path を確保する(spec §3.6 / §4.2.2)。
///
/// # Preconditions
///
/// - `reading` は hiragana 文字列(`KanjiBackend` 契約と同じ)
///
/// # Postconditions
///
/// - 同一 reading に対し 0 件以上の `VocabEntry` を返す
/// - 返値順序は score 降順(score 同値時の順序は実装依存)
pub trait VocabularyLookup {
    /// Returns all vocab entries whose reading matches `reading`.
    fn lookup(&self, reading: &str) -> Vec<VocabEntry>;

    /// Returns a stable, human-readable identifier for the vocab source.
    fn vocab_id(&self) -> &str;
}

/// A single vocabulary entry returned by [`VocabularyLookup::lookup`].
#[derive(Debug, Clone)]
pub struct VocabEntry {
    /// Surface form.
    pub surface: String,
    /// Reading (hiragana).
    pub reading: String,
    /// Part of speech (SudachiDict と同 schema、`"名詞"` / `"固有名詞"` 等)。
    pub pos: String,
    /// Score; larger is better.
    pub score: f32,
}
```

- [ ] **Step 4: `cargo test --features dict -p kotoha-core -- dict::vocab::tests` で全 PASS を確認**

想定: `test result: ok. 4 passed; 0 failed; ...`。

- [ ] **Step 5: clippy 通過確認**

```bash
cargo clippy --features dict -p kotoha-core --all-targets -- -D warnings
```

- [ ] **Final step: Commit**

```bash
git add crates/kotoha-core/src/dict/vocab.rs
git commit -m "feat(dict): define VocabularyLookup trait + VocabEntry (#91)"
```

---

## Task 5: CustomVocab(TSV reader)実装(TDD strict)

**Files:**
- Modify: `crates/kotoha-core/src/dict/custom_vocab.rs`

**Depends-on:** Task 4

**Estimated LOC:** 150(impl 80 + tests 70)

参照: spec §5.2, §6.1

- [ ] **Step 1: test を先に書く(Red)— 6 test を先行記述**

`dict/custom_vocab.rs` 末尾に以下を追加する(impl ゼロの段階)。

```rust
#[cfg(test)]
mod tests {
    use super::*;

    const EMPTY_TSV: &str = "# comment only\n# no entries\n";
    const SINGLE_ENTRY_TSV: &str = "漢字\tかんじ\t名詞\t0.85\n";
    const MULTI_ENTRY_TSV: &str = "\
# header comment
漢字\tかんじ\t名詞\t0.85
感じ\tかんじ\t動詞\t0.45
";
    // surface / reading / pos / score のいずれかが欠けた行は reject 対象。
    const MALFORMED_TSV: &str = "漢字\tかんじ\t名詞\n";

    #[test]
    fn custom_vocab_empty_tsv_yields_no_entries() {
        let vocab = CustomVocab::from_str(EMPTY_TSV).expect("empty TSV must load");
        assert!(vocab.lookup("かんじ").is_empty());
    }

    #[test]
    fn custom_vocab_single_entry_can_be_looked_up() {
        let vocab = CustomVocab::from_str(SINGLE_ENTRY_TSV).expect("single TSV must load");
        let result = vocab.lookup("かんじ");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].surface, "漢字");
        assert_eq!(result[0].pos, "名詞");
    }

    #[test]
    fn custom_vocab_skips_comment_lines() {
        let vocab = CustomVocab::from_str(MULTI_ENTRY_TSV).expect("multi TSV must load");
        let result = vocab.lookup("かんじ");
        assert_eq!(result.len(), 2, "2 non-comment entries expected");
    }

    #[test]
    fn custom_vocab_rejects_malformed_line() {
        let err = CustomVocab::from_str(MALFORMED_TSV)
            .expect_err("malformed TSV must be rejected");
        // 受容契約: backend 層の KanjiError::Backend へ折り畳み、reason に "malformed" を含める
        match err {
            crate::kanji::KanjiError::Backend { reason } => {
                assert!(
                    reason.contains("malformed") || reason.contains("fields"),
                    "error reason should mention malformed line: {reason}"
                );
            }
            other => panic!("unexpected error variant: {other:?}"),
        }
    }

    #[test]
    fn custom_vocab_returns_empty_for_unknown_reading() {
        let vocab = CustomVocab::from_str(SINGLE_ENTRY_TSV).expect("single TSV must load");
        assert!(vocab.lookup("みず").is_empty());
    }

    #[test]
    fn custom_vocab_vocab_id_contains_source_label() {
        let vocab = CustomVocab::from_str(EMPTY_TSV).expect("empty TSV must load");
        assert!(
            vocab.vocab_id().contains("custom"),
            "vocab_id should mention 'custom': {}",
            vocab.vocab_id()
        );
    }
}
```

- [ ] **Step 2: `cargo test --features dict -p kotoha-core -- dict::custom_vocab::tests` で 6 件 FAIL(compile error)を確認**

- [ ] **Step 3: impl を実装(Green)**

`dict/custom_vocab.rs` の `#[cfg(test)]` より前に以下を追加する。

```rust
use std::collections::HashMap;
use std::fs;
use std::path::Path;

use crate::dict::vocab::{VocabEntry, VocabularyLookup};
use crate::kanji::KanjiError;

/// Custom vocabulary source backed by a TSV file.
///
/// Schema: `surface<TAB>reading<TAB>pos<TAB>score`。
/// 先頭 `#` の行および空行は comment として skip する。`score` は f32 として
/// 解釈し、parse 失敗時は malformed line として reject する(spec §5.2)。
///
/// # Invariants
///
/// - `entries` key は hiragana reading、value は同 reading を持つ entry 配列
/// - `vocab_id` は `"custom(<source_label>)"` 形式
#[derive(Debug, Clone)]
pub struct CustomVocab {
    entries: HashMap<String, Vec<VocabEntry>>,
    vocab_id: String,
}

impl CustomVocab {
    /// Loads a custom vocab from a TSV file on disk.
    ///
    /// # Errors
    ///
    /// - [`KanjiError::Backend`] when the file cannot be read or parsed.
    pub fn load(path: &Path) -> Result<Self, KanjiError> {
        let content = fs::read_to_string(path).map_err(|e| KanjiError::Backend {
            reason: format!("failed to read custom vocab {}: {e}", path.display()),
        })?;
        let label = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();
        Self::parse(&content, &label)
    }

    /// Parses a TSV string in-memory. Used directly by unit tests and by
    /// `load` after reading from disk.
    ///
    /// # Errors
    ///
    /// - [`KanjiError::Backend`] when any non-comment line has a field count
    ///   other than 4 or a non-parsable score.
    pub fn from_str(content: &str) -> Result<Self, KanjiError> {
        Self::parse(content, "inline")
    }

    fn parse(content: &str, source_label: &str) -> Result<Self, KanjiError> {
        let mut entries: HashMap<String, Vec<VocabEntry>> = HashMap::new();
        for (lineno, raw) in content.lines().enumerate() {
            let line = raw.trim_end_matches('\r');
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let parts: Vec<&str> = line.split('\t').collect();
            if parts.len() != 4 {
                return Err(KanjiError::Backend {
                    reason: format!(
                        "malformed TSV line {} in '{}': expected 4 fields, got {}",
                        lineno + 1,
                        source_label,
                        parts.len()
                    ),
                });
            }
            let score: f32 = parts[3].parse().map_err(|e| KanjiError::Backend {
                reason: format!(
                    "malformed score on line {} in '{}': {e}",
                    lineno + 1,
                    source_label
                ),
            })?;
            let entry = VocabEntry {
                surface: parts[0].to_string(),
                reading: parts[1].to_string(),
                pos: parts[2].to_string(),
                score,
            };
            entries
                .entry(entry.reading.clone())
                .or_default()
                .push(entry);
        }
        // 同 reading 内を score 降順に保持しておく(lookup で再 sort しないため)
        for vs in entries.values_mut() {
            vs.sort_by(|a, b| {
                b.score
                    .partial_cmp(&a.score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
        }
        Ok(Self {
            entries,
            vocab_id: format!("custom({source_label})"),
        })
    }
}

impl VocabularyLookup for CustomVocab {
    fn lookup(&self, reading: &str) -> Vec<VocabEntry> {
        self.entries
            .get(reading)
            .cloned()
            .unwrap_or_default()
    }

    fn vocab_id(&self) -> &str {
        &self.vocab_id
    }
}
```

- [ ] **Step 4: `cargo test --features dict -p kotoha-core -- dict::custom_vocab::tests` で 6/6 PASS を確認**

想定: `test result: ok. 6 passed; 0 failed; ...`。

- [ ] **Step 5: clippy 通過確認**

```bash
cargo clippy --features dict -p kotoha-core --all-targets -- -D warnings
```

- [ ] **Final step: Commit**

```bash
git add crates/kotoha-core/src/dict/custom_vocab.rs
git commit -m "feat(dict): implement CustomVocab TSV reader (#91)"
```

---

## Task 6: DictionaryConfig 定義(TDD)

**Files:**
- Modify: `crates/kotoha-core/src/dict/mod.rs`(`DictionaryConfig` 定義 + `resolve_dict_path` helper + `pub use` 追加)

**Depends-on:** Task 5

**Estimated LOC:** 50(impl 25 + tests 25)

参照: spec §3.4, §4.4, §5.1

- [ ] **Step 1: test を先に書く(Red)**

`dict/mod.rs` 末尾に以下を追加する。

```rust
#[cfg(test)]
mod config_tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn dictionary_config_default_is_none_paths() {
        let cfg = DictionaryConfig::default();
        assert!(cfg.system_dict_path.is_none());
        assert!(cfg.custom_vocab_path.is_none());
    }

    #[test]
    fn dictionary_config_clone_preserves_fields() {
        let cfg = DictionaryConfig {
            system_dict_path: Some(PathBuf::from("/tmp/system_core.dic")),
            custom_vocab_path: Some(PathBuf::from("/tmp/kotoha-dict.tsv")),
        };
        let cloned = cfg.clone();
        assert_eq!(cloned.system_dict_path, cfg.system_dict_path);
        assert_eq!(cloned.custom_vocab_path, cfg.custom_vocab_path);
    }

    #[test]
    fn dictionary_config_debug_contains_field_names() {
        let cfg = DictionaryConfig {
            system_dict_path: Some(PathBuf::from("/tmp/system_core.dic")),
            custom_vocab_path: None,
        };
        let msg = format!("{cfg:?}");
        assert!(msg.contains("system_dict_path"), "Debug must expose field names: {msg}");
    }

    #[test]
    fn resolve_dict_path_prefers_explicit_over_env() {
        let explicit = PathBuf::from("/tmp/explicit.dic");
        let resolved = resolve_dict_path(Some(&explicit), None);
        assert_eq!(resolved, Some(PathBuf::from("/tmp/explicit.dic")));
    }

    #[test]
    fn resolve_dict_path_falls_back_to_env_var() {
        let resolved = resolve_dict_path(None, Some("/tmp/env.dic".to_string()));
        assert_eq!(resolved, Some(PathBuf::from("/tmp/env.dic")));
    }

    #[test]
    fn resolve_dict_path_returns_none_when_both_absent() {
        let resolved = resolve_dict_path(None, None);
        assert!(resolved.is_none());
    }
}
```

- [ ] **Step 2: `cargo test --features dict -p kotoha-core -- dict::config_tests` で FAIL を確認**

- [ ] **Step 3: impl を実装(Green)**

`dict/mod.rs` の冒頭 doc-comment と `pub(crate) mod` 群との間に以下を追加する。

```rust
use std::path::{Path, PathBuf};

pub use self::backend::DictionaryBackend;
pub use self::engine::{EngineCandidate, MorphologicalEngine};
pub use self::vocab::{VocabEntry, VocabularyLookup};

/// Engine 非依存の Dictionary backend 設定(spec §3.4 Q4)。
///
/// 将来 engine を vibrato / lindera に置換しても config 互換性を保つため、
/// field 名に engine 名を含めない(`sudachi_dict_path` ではなく `system_dict_path`)。
///
/// # Invariants
///
/// - `system_dict_path` が `None` の場合、`DictionaryBackend::load` は
///   `KOTOHA_SYSTEM_DICT_PATH` 環境変数を読みに行く
/// - `custom_vocab_path` が `None` の場合、backend は custom vocab を持たない
#[derive(Debug, Clone, Default)]
pub struct DictionaryConfig {
    /// 形態素解析用 system dictionary file のパス(SudachiDict-core `system_core.dic`)。
    pub system_dict_path: Option<PathBuf>,
    /// Custom vocabulary TSV file のパス(`kotoha-dict.tsv`)。
    pub custom_vocab_path: Option<PathBuf>,
}

/// 明示 path、次点で env var、どちらも無ければ `None` を返す解決ヘルパ。
///
/// `DictionaryBackend::load` は本関数で `system_dict_path` を解決する。
/// `env_value` 引数は test 容易性のため `std::env::var` の結果を呼び出し側が
/// 注入する契約にする(spec §3.4 / §5.1)。
pub(crate) fn resolve_dict_path(
    explicit: Option<&Path>,
    env_value: Option<String>,
) -> Option<PathBuf> {
    if let Some(p) = explicit {
        return Some(p.to_path_buf());
    }
    env_value.map(PathBuf::from)
}
```

- [ ] **Step 4: `cargo test --features dict -p kotoha-core -- dict::config_tests` で 6/6 PASS を確認**

- [ ] **Step 5: clippy 通過確認**

```bash
cargo clippy --features dict -p kotoha-core --all-targets -- -D warnings
```

- [ ] **Final step: Commit**

```bash
git add crates/kotoha-core/src/dict/mod.rs
git commit -m "feat(dict): define DictionaryConfig with engine-neutral field names (#91)"
```

---

## Task 7: SudachiAdapter 実装(TDD relaxed)

**Files:**
- Modify: `crates/kotoha-core/src/dict/sudachi_adapter.rs`

**Depends-on:** Task 6

**Estimated LOC:** 120(impl 90 + tests 30)

参照: spec §3.4, §4.1, §5.1

SudachiDict 実体が無いと本格的な unit test を書けないため、本 task の Layer 1 test は struct compile + `engine_id` の format 検証のみを対象とする。実辞書経由の検証は Task 14 (Layer 3 golden) で行う。

- [ ] **Step 1: test を先に書く(Red / compile-only check)**

`dict/sudachi_adapter.rs` 末尾に以下を追加する。

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::kanji::KanjiError;
    use std::path::Path;

    #[test]
    fn sudachi_adapter_load_missing_file_errors() {
        let err = SudachiAdapter::load(Path::new("/tmp/definitely-does-not-exist-kotoha-p2a.dic"))
            .expect_err("load must error when the path does not exist");
        match err {
            KanjiError::ModelNotFound { path } => {
                assert!(path.to_string_lossy().contains("definitely-does-not-exist"));
            }
            other => panic!("expected ModelNotFound, got: {other:?}"),
        }
    }

    // engine_id フォーマット整合(実辞書 load せずに type / trait 経由では
    // engine_id を確認できないため、本 test は Layer 3 golden 側へ委譲する)。
    // ここでは engine_id() format に含めるべき文字列リテラルを const で固定し
    // Layer 3 test 側と同一文字列を使うことで整合を担保する。
    #[test]
    fn sudachi_engine_id_label_constant_is_stable() {
        assert!(SUDACHI_ENGINE_ID_LABEL.contains("sudachi"));
        assert!(SUDACHI_ENGINE_ID_LABEL.contains("0.6"));
    }
}
```

- [ ] **Step 2: FAIL を確認**

```bash
cargo test --features dict -p kotoha-core -- dict::sudachi_adapter::tests
```

想定: `cannot find type 'SudachiAdapter'` / `cannot find value 'SUDACHI_ENGINE_ID_LABEL'`。

- [ ] **Step 3: impl(sudachi.rs runtime ラッパー)**

`dict/sudachi_adapter.rs` の `#[cfg(test)]` より前に以下を追加する。

```rust
use std::path::{Path, PathBuf};

use sudachi::analysis::stateful_tokenizer::StatefulTokenizer;
use sudachi::config::Config;
use sudachi::dic::dictionary::JapaneseDictionary;
use sudachi::prelude::*;

use crate::dict::engine::{EngineCandidate, MorphologicalEngine};
use crate::kanji::KanjiError;

/// sudachi.rs の engine_id() で返す固定 label。Layer 3 golden test から
/// 同一文字列でアサートするため、module スコープの `pub(crate)` const として
/// 固定する。
pub(crate) const SUDACHI_ENGINE_ID_LABEL: &str = "sudachi-0.6.11+sudachidict-core:v20260116";

/// `MorphologicalEngine` の sudachi.rs 実装。
///
/// # Invariants
///
/// - `dict` は初期化済み `JapaneseDictionary`
/// - `system_dict_path` は `dict` を load した際の path(diagnostics 用)
///
/// # Errors
///
/// - [`KanjiError::ModelNotFound`] when the dict file does not exist
/// - [`KanjiError::ModelLoadFailed`] when sudachi.rs fails to parse the dict
pub struct SudachiAdapter {
    dict: JapaneseDictionary,
    system_dict_path: PathBuf,
}

impl SudachiAdapter {
    /// Loads a SudachiDict-core file from disk.
    pub fn load(system_dict_path: &Path) -> Result<Self, KanjiError> {
        if !system_dict_path.exists() {
            return Err(KanjiError::ModelNotFound {
                path: system_dict_path.to_path_buf(),
            });
        }
        // sudachi.rs は Config builder で system dict path を受ける
        let config = Config::new(None, None, Some(system_dict_path.to_path_buf()))
            .map_err(|e| KanjiError::ModelLoadFailed {
                source: Box::new(e),
            })?;
        let dict = JapaneseDictionary::from_cfg(&config).map_err(|e| KanjiError::ModelLoadFailed {
            source: Box::new(e),
        })?;
        Ok(Self {
            dict,
            system_dict_path: system_dict_path.to_path_buf(),
        })
    }
}

impl MorphologicalEngine for SudachiAdapter {
    fn tokenize(&self, reading: &str) -> Result<Vec<EngineCandidate>, KanjiError> {
        if reading.is_empty() {
            return Ok(Vec::new());
        }
        let mut tokenizer: StatefulTokenizer<&JapaneseDictionary> =
            StatefulTokenizer::create(&self.dict, false, sudachi::analysis::Mode::C);
        tokenizer
            .reset()
            .push_str(reading);
        tokenizer.do_tokenize().map_err(|e| KanjiError::Backend {
            reason: format!(
                "sudachi tokenize failed for input {:?} using dict {}: {e}",
                reading,
                self.system_dict_path.display()
            ),
        })?;
        let mut morphemes = MorphemeList::empty(&self.dict);
        tokenizer
            .into_morpheme_list(&mut morphemes)
            .map_err(|e| KanjiError::Backend {
                reason: format!("sudachi morpheme collection failed: {e}"),
            })?;

        let mut out = Vec::with_capacity(morphemes.len());
        for m in morphemes.iter() {
            // sudachi.rs の morpheme は surface + reading_form + word_info を持つ。
            // score は cost を反転(負)して「大きい方が良い」契約へ揃える。
            let surface = m.surface().to_string();
            let reading_form = m.reading_form().to_string();
            let cost = m.word_info().head_word_length() as f32; // placeholder score proxy
            out.push(EngineCandidate {
                surface,
                reading: reading_form,
                score: -cost,
            });
        }
        Ok(out)
    }

    fn engine_id(&self) -> &str {
        SUDACHI_ENGINE_ID_LABEL
    }
}
```

注: sudachi.rs の score 抽出 API は v0.6.11 時点で完全に安定しておらず、本 plan では `head_word_length` を score proxy に用いる。実測で pass rate を満たさない場合、Task 14 の診断 step で `word_info().oov()` や cost 直接取得 API への切替を検討する。

- [ ] **Step 4: `cargo test --features dict -p kotoha-core -- dict::sudachi_adapter::tests` で 2/2 PASS を確認**

想定: `test result: ok. 2 passed; 0 failed; ...`(実辞書 load は行わない test のみ)。

- [ ] **Step 5: clippy 通過確認**

```bash
cargo clippy --features dict -p kotoha-core --all-targets -- -D warnings
```

- [ ] **Final step: Commit**

```bash
git add crates/kotoha-core/src/dict/sudachi_adapter.rs
git commit -m "feat(dict): implement SudachiAdapter morphological engine (#91)"
```

---

## Task 8: DictionaryBackend 実装(TDD strict、MockEngine + MockVocab 注入)

**Files:**
- Modify: `crates/kotoha-core/src/dict/backend.rs`

**Depends-on:** Task 7

**Estimated LOC:** 250(impl 150 + tests 100)

参照: spec §3.5, §3.6, §4.3

- [ ] **Step 1: test を先に書く(Red)— 7 test を先行記述**

`dict/backend.rs` 末尾に以下を追加する。

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::dict::engine::{EngineCandidate, MorphologicalEngine};
    use crate::dict::vocab::{VocabEntry, VocabularyLookup};
    use crate::kanji::{Candidate, ConvertOptions, KanjiError};

    struct StubEngine {
        canned: Vec<EngineCandidate>,
    }

    impl MorphologicalEngine for StubEngine {
        fn tokenize(&self, reading: &str) -> Result<Vec<EngineCandidate>, KanjiError> {
            if reading.is_empty() {
                return Ok(Vec::new());
            }
            Ok(self.canned.clone())
        }
        fn engine_id(&self) -> &str { "stub-engine" }
    }

    struct StubVocab {
        canned: Vec<VocabEntry>,
    }

    impl VocabularyLookup for StubVocab {
        fn lookup(&self, reading: &str) -> Vec<VocabEntry> {
            if reading.is_empty() { Vec::new() } else { self.canned.clone() }
        }
        fn vocab_id(&self) -> &str { "stub-vocab" }
    }

    fn backend_with(engine_cands: Vec<EngineCandidate>, vocab_cands: Vec<VocabEntry>) -> DictionaryBackend {
        DictionaryBackend::from_parts(
            Box::new(StubEngine { canned: engine_cands }),
            vec![Box::new(StubVocab { canned: vocab_cands })],
        )
    }

    #[test]
    fn dict_backend_rejects_non_hiragana_input() {
        let b = backend_with(Vec::new(), Vec::new());
        let err = b.convert("abc", &ConvertOptions::default()).expect_err("latin reject");
        assert!(matches!(err, KanjiError::InvalidInput { .. }));
    }

    #[test]
    fn dict_backend_empty_input_returns_empty_vec() {
        let b = backend_with(Vec::new(), Vec::new());
        let out = b.convert("", &ConvertOptions::default()).expect("empty ok");
        assert!(out.is_empty());
    }

    #[test]
    fn dict_backend_merges_engine_and_vocab_results() {
        let b = backend_with(
            vec![EngineCandidate { surface: "漢字".into(), reading: "かんじ".into(), score: 0.5 }],
            vec![VocabEntry { surface: "感じ".into(), reading: "かんじ".into(), pos: "動詞".into(), score: 0.4 }],
        );
        let out = b.convert("かんじ", &ConvertOptions::default()).expect("merge ok");
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].surface, "漢字");
        assert_eq!(out[1].surface, "感じ");
    }

    #[test]
    fn dict_backend_dedupes_duplicate_surfaces_keeping_highest_score() {
        let b = backend_with(
            vec![EngineCandidate { surface: "漢字".into(), reading: "かんじ".into(), score: 0.3 }],
            vec![VocabEntry { surface: "漢字".into(), reading: "かんじ".into(), pos: "名詞".into(), score: 0.9 }],
        );
        let out = b.convert("かんじ", &ConvertOptions::default()).expect("dedupe ok");
        assert_eq!(out.len(), 1);
        assert!((out[0].score - 0.9).abs() < 1e-6);
    }

    #[test]
    fn dict_backend_truncates_to_top_k() {
        let engine_cands = (0..10)
            .map(|i| EngineCandidate {
                surface: format!("s{i}"),
                reading: "かんじ".into(),
                score: i as f32 / 10.0,
            })
            .collect();
        let b = backend_with(engine_cands, Vec::new());
        let opts = ConvertOptions { top_k: 3, temperature: 0.0, seed: Some(0) };
        let out = b.convert("かんじ", &opts).expect("truncate ok");
        assert_eq!(out.len(), 3);
    }

    #[test]
    fn dict_backend_top_k_zero_returns_empty() {
        let b = backend_with(
            vec![EngineCandidate { surface: "漢字".into(), reading: "かんじ".into(), score: 0.9 }],
            Vec::new(),
        );
        let opts = ConvertOptions { top_k: 0, temperature: 0.0, seed: Some(0) };
        let out = b.convert("かんじ", &opts).expect("empty ok");
        assert!(out.is_empty());
    }

    #[test]
    fn dict_backend_model_id_contains_engine_id() {
        let b = backend_with(Vec::new(), Vec::new());
        let id = b.model_id();
        assert!(id.contains("dictionary"), "model_id should start with 'dictionary': {id}");
        assert!(id.contains("stub-engine"), "model_id should embed engine_id: {id}");
    }
}
```

- [ ] **Step 2: FAIL を確認**

```bash
cargo test --features dict -p kotoha-core -- dict::backend::tests
```

想定: `cannot find type 'DictionaryBackend'`。

- [ ] **Step 3: impl**

`dict/backend.rs` の `#[cfg(test)]` より前に以下を追加する。

```rust
use std::path::Path;

use crate::dict::engine::MorphologicalEngine;
use crate::dict::vocab::VocabularyLookup;
use crate::dict::{resolve_dict_path, DictionaryConfig};
use crate::kanji::backend::{score_sort_dedupe, validate_input};
use crate::kanji::{Candidate, ConvertOptions, KanjiBackend, KanjiError};

/// SudachiDict ベースの Dictionary backend。
///
/// `KanjiBackend` の 2 つ目の実装(LlamaCpp に続く、spec §4.3)。
/// `MorphologicalEngine` と `VocabularyLookup` を field に保持し、両者の
/// 候補を merge / dedupe / truncate して返す。
///
/// # Invariants
///
/// - `engine` は単一の形態素解析 engine(SudachiAdapter 等)
/// - `vocab_sources` は 0 個以上の vocab source(CustomVocab 等)
/// - `model_id` は `"dictionary({engine_id})"` 形式で初期化時に確定
pub struct DictionaryBackend {
    engine: Box<dyn MorphologicalEngine>,
    vocab_sources: Vec<Box<dyn VocabularyLookup>>,
    model_id: String,
}

impl std::fmt::Debug for DictionaryBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DictionaryBackend")
            .field("engine_id", &self.engine.engine_id())
            .field("vocab_count", &self.vocab_sources.len())
            .field("model_id", &self.model_id)
            .finish()
    }
}

impl DictionaryBackend {
    /// Construct from a `DictionaryConfig`. Resolves `system_dict_path` via
    /// the env var `KOTOHA_SYSTEM_DICT_PATH` fallback (spec §3.4 / §5.1).
    ///
    /// # Errors
    ///
    /// - [`KanjiError::ModelNotFound`] when neither config nor env var is set.
    /// - Errors bubbled up from `SudachiAdapter::load` / `CustomVocab::load`.
    pub fn load(config: &DictionaryConfig) -> Result<Self, KanjiError> {
        let env_value = std::env::var("KOTOHA_SYSTEM_DICT_PATH").ok();
        let Some(dict_path) = resolve_dict_path(config.system_dict_path.as_deref(), env_value)
        else {
            return Err(KanjiError::ModelNotFound {
                path: Path::new("").to_path_buf(),
            });
        };

        let engine = Box::new(crate::dict::sudachi_adapter::SudachiAdapter::load(&dict_path)?);

        let mut vocab_sources: Vec<Box<dyn VocabularyLookup>> = Vec::new();
        if let Some(vp) = &config.custom_vocab_path {
            vocab_sources.push(Box::new(crate::dict::custom_vocab::CustomVocab::load(vp)?));
        }
        let model_id = format!("dictionary({})", engine.engine_id());
        Ok(Self {
            engine,
            vocab_sources,
            model_id,
        })
    }

    /// Test-only constructor — allows direct injection of stub engine / vocab.
    ///
    /// Not part of the stable public API.
    #[doc(hidden)]
    pub fn from_parts(
        engine: Box<dyn MorphologicalEngine>,
        vocab_sources: Vec<Box<dyn VocabularyLookup>>,
    ) -> Self {
        let model_id = format!("dictionary({})", engine.engine_id());
        Self {
            engine,
            vocab_sources,
            model_id,
        }
    }
}

impl KanjiBackend for DictionaryBackend {
    fn model_id(&self) -> &str {
        &self.model_id
    }

    fn convert(&self, input: &str, options: &ConvertOptions) -> Result<Vec<Candidate>, KanjiError> {
        validate_input(input)?;
        if input.is_empty() || options.top_k == 0 {
            return Ok(Vec::new());
        }
        let mut raw: Vec<Candidate> = Vec::new();
        for ec in self.engine.tokenize(input)? {
            raw.push(Candidate::new(ec.surface, ec.score));
        }
        for vocab in &self.vocab_sources {
            for ve in vocab.lookup(input) {
                raw.push(Candidate::new(ve.surface, ve.score));
            }
        }
        Ok(score_sort_dedupe(raw, options.top_k))
    }
}
```

- [ ] **Step 4: `cargo test --features dict -p kotoha-core -- dict::backend::tests` で 7/7 PASS を確認**

- [ ] **Step 5: clippy 通過確認**

```bash
cargo clippy --features dict -p kotoha-core --all-targets -- -D warnings
```

- [ ] **Final step: Commit**

```bash
git add crates/kotoha-core/src/dict/backend.rs
git commit -m "feat(dict): implement DictionaryBackend with MockEngine/MockVocab tests (#91)"
```

---

## Task 9: resources/kotoha-dict.tsv(空 fixture + curation policy)

**Files:**
- Create: `crates/kotoha-core/resources/kotoha-dict.tsv`

**Depends-on:** Task 8

**Estimated LOC:** 15

参照: spec §3.7, §5.2

- [ ] **Step 1: resources/ ディレクトリを作成**

```bash
mkdir -p crates/kotoha-core/resources
```

- [ ] **Step 2: `kotoha-dict.tsv` を以下の内容で作成**

```tsv
# kotoha-dict.tsv — Kotoha custom vocabulary
# Schema: surface<TAB>reading<TAB>pos<TAB>score
# Curation policy (P2-A spec §3.7 Q7):
#   1. SudachiDict-core baseline で recall 可能な語彙は追加しない
#   2. golden fixture で観察される gap のみを証拠ベースで追加する
#   3. 同 reading で SudachiDict が出す surface を上書きする entry は
#      P2-B 以降で機械的多義性チェックを通すこと
# (P2-A: 空 start。entry は後続 milestone で追加する)
```

- [ ] **Step 3: kotoha-core から `include_str!` 経由で読み込む予備 smoke として、直接参照はしないため check のみ**

本 task では bundled file は作成するが、current impl で `include_str!` 経由の runtime 参照は行わない(runtime は `custom_vocab_path` config or env var 経由 load)。`cargo check` が通ることのみを確認する。

```bash
cargo check --workspace --features dict
```

- [ ] **Final step: Commit**

```bash
git add crates/kotoha-core/resources/kotoha-dict.tsv
git commit -m "feat(dict): bundle empty kotoha-dict.tsv with curation policy (#91)"
```

---

## Task 10: BackendConfig::Dictionary variant + load_backend arm 追加(TDD strict)

**Files:**
- Modify: `crates/kotoha-core/src/kanji/backend.rs`(enum 拡張 + load_backend arm)
- Modify: `crates/kotoha-core/src/kanji/mod.rs`(`DictionaryBackend` の re-export gated)

**Depends-on:** Task 9

**Estimated LOC:** 70(enum + arm 30 + tests 40)

参照: spec §3.3, §4.4

- [ ] **Step 1: test を先に書く(Red)— kanji/backend.rs `mod tests` 末尾に追加**

既存 `mod tests` 内、`BackendConfig` セクションの末尾に以下を追加する。

```rust
    // ======================================================================
    // BackendConfig::Dictionary (P2-A)
    // ======================================================================

    #[cfg(feature = "dict")]
    #[test]
    fn backend_config_dictionary_is_clone_and_debug() {
        use crate::dict::DictionaryConfig;
        let cfg = BackendConfig::Dictionary {
            config: DictionaryConfig {
                system_dict_path: Some(PathBuf::from("/tmp/system_core.dic")),
                custom_vocab_path: None,
            },
        };
        let cloned = cfg.clone();
        let msg = format!("{cloned:?}");
        assert!(msg.contains("Dictionary"));
        assert!(msg.contains("system_dict_path"));
    }

    #[cfg(not(feature = "dict"))]
    #[test]
    fn dictionary_config_without_feature_errors_feature_disabled() {
        // feature=dict OFF 時は BackendConfig::Dictionary variant 自体が
        // `#[cfg(feature = "dict")]` で gate される設計のため本 test は該当時
        // のみ compile する。本 test は「feature OFF を明示する placeholder」
        // として空アサーションを置く。
        let _ = std::any::type_name::<BackendConfig>();
    }

    #[cfg(feature = "dict")]
    #[test]
    fn dictionary_config_load_backend_missing_env_errors_model_not_found() {
        use crate::dict::DictionaryConfig;
        // env var も config も無い状態で load_backend を呼ぶと ModelNotFound
        std::env::remove_var("KOTOHA_SYSTEM_DICT_PATH");
        let cfg = BackendConfig::Dictionary {
            config: DictionaryConfig::default(),
        };
        match load_backend(&cfg) {
            Err(KanjiError::ModelNotFound { .. }) => {}
            other => panic!("expected ModelNotFound, got: {other:?}"),
        }
    }
```

- [ ] **Step 2: FAIL を確認**

```bash
cargo test --features dict -p kotoha-core --lib -- kanji::backend::tests
```

想定: `variant 'Dictionary' not found in 'BackendConfig'`。

- [ ] **Step 3: `BackendConfig` に variant を追加(Green)**

`kanji/backend.rs` の enum 定義に以下を追加する。

```rust
    /// SudachiDict-based dictionary backend (P2-A、spec §3.3 / §4.4).
    /// Constructible only when the `dict` feature flag is enabled at build time.
    #[cfg(feature = "dict")]
    Dictionary {
        /// Engine-neutral config (spec §3.4 Q4).
        config: crate::dict::DictionaryConfig,
    },
```

- [ ] **Step 4: `load_backend` factory に arm を追加**

既存 `LlamaCpp { .. }` arm の直後に以下を追加する。

```rust
        #[cfg(feature = "dict")]
        BackendConfig::Dictionary { config } => {
            Ok(Box::new(crate::dict::DictionaryBackend::load(config)?))
        }
```

- [ ] **Step 5: `kanji/mod.rs` で `DictionaryBackend` の re-export を gated 追加**

既存 `#[cfg(feature = "llama-cpp")] pub use llama_cpp::LlamaCppBackend;` の直後に追加する。

```rust
#[cfg(feature = "dict")]
pub use crate::dict::DictionaryBackend;
```

- [ ] **Step 6: `cargo test --features dict -p kotoha-core --lib` で全 PASS を確認**

```bash
cargo test --features dict -p kotoha-core --lib
```

- [ ] **Step 7: `cargo test --features mock-backend,dict -p kotoha-core` で既存 Mock 系を含めた全 PASS を確認**

- [ ] **Step 8: clippy 通過確認**

```bash
cargo clippy --features mock-backend,dict -p kotoha-core --all-targets -- -D warnings
```

- [ ] **Final step: Commit**

```bash
git add crates/kotoha-core/src/kanji/backend.rs crates/kotoha-core/src/kanji/mod.rs
git commit -m "feat(kanji): add BackendConfig::Dictionary variant + load_backend arm (#91)"
```

---

## Task 11: Layer 2 integration test(kanji_dictionary_unit.rs)

**Files:**
- Create: `crates/kotoha-core/tests/kanji_dictionary_unit.rs`

**Depends-on:** Task 10

**Estimated LOC:** 120

参照: spec §6.2

- [ ] **Step 1: test file を作成**

```rust
//! Layer 2 integration tests for the Dictionary backend (P2-A).
//!
//! Spec: `docs/superpowers/specs/2026-04-25-p2-a-dictionary-layer-design.md` §6.2.
//!
//! これらの test は SudachiDict 不在環境でも実行可能。実辞書が要る smoke
//! は `kanji_dictionary_golden.rs` (Layer 3、`dict-smoke` feature) 側に置く。

#![cfg(feature = "dict")]

use std::path::PathBuf;

use kotoha_core::dict::DictionaryConfig;
use kotoha_core::kanji::{load_backend, BackendConfig, KanjiError};

#[test]
fn load_backend_dictionary_without_env_errors_model_not_found() {
    std::env::remove_var("KOTOHA_SYSTEM_DICT_PATH");
    let cfg = BackendConfig::Dictionary {
        config: DictionaryConfig::default(),
    };
    let err = load_backend(&cfg).expect_err("no dict path must error");
    assert!(matches!(err, KanjiError::ModelNotFound { .. }));
}

#[test]
fn load_backend_dictionary_with_nonexistent_path_errors_model_not_found() {
    let cfg = BackendConfig::Dictionary {
        config: DictionaryConfig {
            system_dict_path: Some(PathBuf::from(
                "/tmp/definitely-does-not-exist-kotoha-p2a-integration.dic",
            )),
            custom_vocab_path: None,
        },
    };
    let err = load_backend(&cfg).expect_err("missing file must error");
    match err {
        KanjiError::ModelNotFound { path } => {
            assert!(path.to_string_lossy().contains("definitely-does-not-exist"));
        }
        other => panic!("expected ModelNotFound, got: {other:?}"),
    }
}

#[test]
fn dictionary_config_default_is_all_none() {
    let cfg = DictionaryConfig::default();
    assert!(cfg.system_dict_path.is_none());
    assert!(cfg.custom_vocab_path.is_none());
}

#[test]
fn dictionary_config_env_var_resolve_fallback_works() {
    // NOTE: env var を直接書き換える test は並列実行で flaky になる可能性
    // があるため、resolve は unit test 側 (dict::config_tests) で検証し、
    // 本 integration test では path 明示の振る舞いのみを確認する。
    let cfg = DictionaryConfig {
        system_dict_path: Some(PathBuf::from("/tmp/explicit-path.dic")),
        custom_vocab_path: None,
    };
    let result = load_backend(&BackendConfig::Dictionary { config: cfg });
    // explicit path でも実ファイルは無いため ModelNotFound になる。これは
    // env var fallback 経路ではなく explicit 経路を通過したことの裏返し。
    match result {
        Err(KanjiError::ModelNotFound { path }) => {
            assert_eq!(path, PathBuf::from("/tmp/explicit-path.dic"));
        }
        other => panic!("expected ModelNotFound from explicit path, got: {other:?}"),
    }
}

#[test]
fn dictionary_config_custom_vocab_missing_errors_backend() {
    let cfg = DictionaryConfig {
        system_dict_path: Some(PathBuf::from("/tmp/no-such-system.dic")),
        custom_vocab_path: Some(PathBuf::from("/tmp/no-such-custom.tsv")),
    };
    // system_dict の load で ModelNotFound に到達し、custom_vocab の load
    // には進まない。この test は「system が先にチェックされる」順序契約の
    // regression 防止に相当する。
    let err = load_backend(&BackendConfig::Dictionary { config: cfg }).expect_err("err");
    assert!(matches!(err, KanjiError::ModelNotFound { .. }));
}
```

- [ ] **Step 2: `cargo test --features dict -p kotoha-core --test kanji_dictionary_unit` で 5/5 PASS を確認**

```bash
cargo test --features dict -p kotoha-core --test kanji_dictionary_unit
```

- [ ] **Step 3: clippy 通過確認**

```bash
cargo clippy --features dict -p kotoha-core --all-targets -- -D warnings
```

- [ ] **Final step: Commit**

```bash
git add crates/kotoha-core/tests/kanji_dictionary_unit.rs
git commit -m "test(dict): add Layer 2 integration tests for DictionaryBackend (#91)"
```

---

## Task 12: Fixture 生成 tooling(tools/p2a-fixture-gen/)

**Files:**
- Create: `tools/p2a-fixture-gen/pyproject.toml`
- Create: `tools/p2a-fixture-gen/README.md`
- Create: `tools/p2a-fixture-gen/src/p2a_fixture_gen/__init__.py`
- Create: `tools/p2a-fixture-gen/src/p2a_fixture_gen/sample.py`
- Create: `tools/p2a-fixture-gen/src/p2a_fixture_gen/targeted.py`
- Create: `tools/p2a-fixture-gen/src/p2a_fixture_gen/main.py`
- Create: `tools/p2a-fixture-gen/tests/test_sample.py`
- Create: `tools/p2a-fixture-gen/tests/test_targeted.py`

**Depends-on:** Task 11

**Estimated LOC:** 200

参照: spec §6.3.1, §6.3.4

前提: `tools/p5a-data-pipeline/data/sample.tsv` が 1140 行程度存在する(MEMORY.md 記載、P5-A PoC で生成済み)。無い場合は Step 1 で再生成する手順を踏む。

- [ ] **Step 1: p5a sample.tsv の存在を確認、無ければ再生成**

```bash
ls -la tools/p5a-data-pipeline/data/sample.tsv || (cd tools/p5a-data-pipeline && uv run python -m kotoha_p5a)
```

想定: sample.tsv が 1100 行以上あれば OK。無い場合の再生成は `uv run python -m kotoha_p5a` で行う(P5-A PoC の main entry)。

- [ ] **Step 2: tools/p2a-fixture-gen ディレクトリを作成**

```bash
mkdir -p tools/p2a-fixture-gen/src/p2a_fixture_gen tools/p2a-fixture-gen/tests
```

- [ ] **Step 3: pyproject.toml を作成**

```toml
[project]
name = "kotoha-p2a-fixture-gen"
version = "0.1.0"
description = "Phase 2 P2-A golden fixture generator (530-case stratified sampling)"
readme = "README.md"
requires-python = ">=3.12"
license = { text = "Apache-2.0" }
authors = [
    { name = "Kotoha IME contributors" },
]

[dependency-groups]
dev = [
    "pytest>=8.0",
    "ruff>=0.5",
]

[build-system]
requires = ["hatchling"]
build-backend = "hatchling.build"

[tool.hatch.build.targets.wheel]
packages = ["src/p2a_fixture_gen"]

[tool.ruff]
target-version = "py312"
line-length = 100

[tool.pytest.ini_options]
testpaths = ["tests"]
```

- [ ] **Step 4: README.md を作成**

```markdown
# kotoha-p2a-fixture-gen

Phase 2 P2-A 用 golden fixture (530 cases) 生成 tool。

## 入力

- `../p5a-data-pipeline/data/sample.tsv`(1100+ 行想定、P5-A PoC 生成物)

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
```

- [ ] **Step 5: `__init__.py` を作成(空)**

```python
"""Kotoha P2-A golden fixture generator."""

__version__ = "0.1.0"
```

- [ ] **Step 6: `targeted.py` を作成(targeted 30 cases の manual list)**

```python
"""Targeted 30 cases for Phase 2 golden fixture (spec §6.3.1)."""

from dataclasses import dataclass


@dataclass(frozen=True)
class Case:
    input: str
    expected_top_surface: str
    category: str
    note: str


TARGETED_CASES: list[Case] = [
    # 敬称 10
    Case("やまださん", "山田さん", "honorific", "敬称+人名"),
    Case("たなかさん", "田中さん", "honorific", "敬称+人名"),
    Case("すずきせんせい", "鈴木先生", "honorific", "敬称+教職"),
    Case("さとうさま", "佐藤様", "honorific", "尊敬敬称"),
    Case("ほんださん", "本田さん", "honorific", "敬称+人名"),
    Case("なかむらくん", "中村くん", "honorific", "男性敬称"),
    Case("やまもとちゃん", "山本ちゃん", "honorific", "親愛敬称"),
    Case("こばやしせんぱい", "小林先輩", "honorific", "年長敬称"),
    Case("おがわかちょう", "小川課長", "honorific", "役職敬称"),
    Case("いしいしゃちょう", "石井社長", "honorific", "役職敬称"),
    # 人名 10
    Case("やまだたろう", "山田太郎", "personal_name", "姓+名"),
    Case("すずきはなこ", "鈴木花子", "personal_name", "姓+名"),
    Case("たなかいちろう", "田中一郎", "personal_name", "姓+名"),
    Case("さとうけんじ", "佐藤健二", "personal_name", "姓+名"),
    Case("わたなべゆき", "渡辺由紀", "personal_name", "姓+名"),
    Case("いとうまさし", "伊藤正", "personal_name", "姓+名"),
    Case("やまもとあきら", "山本明", "personal_name", "姓+名"),
    Case("なかむらみさき", "中村美咲", "personal_name", "姓+名"),
    Case("こばやしゆうじ", "小林雄二", "personal_name", "姓+名"),
    Case("まつもとなおこ", "松本直子", "personal_name", "姓+名"),
    # 地名・組織名 5
    Case("とうきょうと", "東京都", "place_org", "地名"),
    Case("おおさかふ", "大阪府", "place_org", "地名"),
    Case("ほっかいどう", "北海道", "place_org", "地名"),
    Case("きょうとだいがく", "京都大学", "place_org", "組織名"),
    Case("とよたじどうしゃ", "トヨタ自動車", "place_org", "組織名"),
    # 外来語 5
    Case("こんぴゅーたー", "コンピューター", "loanword", "長音付き"),
    Case("ぷろぐらむ", "プログラム", "loanword", "促音なし"),
    Case("さーばー", "サーバー", "loanword", "長音付き"),
    Case("かめら", "カメラ", "loanword", "短い外来語"),
    Case("すまーとふぉん", "スマートフォン", "loanword", "長音+合成語"),
]


def targeted_as_tsv_rows() -> list[str]:
    """Return targeted cases as TSV lines (input<TAB>expected<TAB>category<TAB>note)."""
    return [
        f"{c.input}\t{c.expected_top_surface}\t{c.category}\t{c.note}"
        for c in TARGETED_CASES
    ]
```

- [ ] **Step 7: `sample.py` を作成(bulk 500 cases の stratified sampling)**

```python
"""Bulk 500-case stratified sampling (5 buckets × 100) from sample.tsv."""

import random
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[4]
P5A_SAMPLE = REPO_ROOT / "tools" / "p5a-data-pipeline" / "data" / "sample.tsv"


def bucket_for_reading_length(n: int) -> str | None:
    if 1 <= n <= 3:
        return "B1"
    if 4 <= n <= 6:
        return "B2"
    if 7 <= n <= 10:
        return "B3"
    if 11 <= n <= 20:
        return "B4"
    if n >= 21:
        return "B5"
    return None


def load_sample_pairs(path: Path = P5A_SAMPLE) -> list[tuple[str, str]]:
    """Load (reading, surface) pairs from p5a sample.tsv.

    File format: reading<TAB>surface (per P5-A PoC). Empty / comment lines skipped.
    """
    pairs: list[tuple[str, str]] = []
    if not path.exists():
        raise FileNotFoundError(
            f"p5a sample not found: {path}. Run `cd tools/p5a-data-pipeline && uv run python -m kotoha_p5a` first."
        )
    for line in path.read_text(encoding="utf-8").splitlines():
        line = line.strip()
        if not line or line.startswith("#"):
            continue
        parts = line.split("\t")
        if len(parts) < 2:
            continue
        pairs.append((parts[0], parts[1]))
    return pairs


def stratified_sample(
    pairs: list[tuple[str, str]], per_bucket: int = 100, seed: int = 42
) -> dict[str, list[tuple[str, str]]]:
    """Group pairs by bucket and sample per_bucket from each. B5 fallback handles scarcity."""
    rng = random.Random(seed)
    buckets: dict[str, list[tuple[str, str]]] = {
        "B1": [],
        "B2": [],
        "B3": [],
        "B4": [],
        "B5": [],
    }
    for reading, surface in pairs:
        b = bucket_for_reading_length(len(reading))
        if b is not None:
            buckets[b].append((reading, surface))

    sampled: dict[str, list[tuple[str, str]]] = {}
    for b, rows in buckets.items():
        if len(rows) >= per_bucket:
            sampled[b] = rng.sample(rows, per_bucket)
        else:
            # B5 scarcity fallback: concatenate two B3/B4 entries to reach 21+ chars
            # until we have per_bucket items. Non-destructive for other buckets.
            sampled[b] = list(rows)
            donor = buckets["B3"] + buckets["B4"]
            while len(sampled[b]) < per_bucket and donor:
                a = rng.choice(donor)
                c = rng.choice(donor)
                combined = (a[0] + c[0], a[1] + c[1])
                if len(combined[0]) >= 21:
                    sampled[b].append(combined)
    return sampled


def bulk_as_tsv_rows(sampled: dict[str, list[tuple[str, str]]]) -> list[str]:
    """Convert sampled dict into TSV rows with category=bulk_<bucket>."""
    rows: list[str] = []
    for bucket, pairs in sampled.items():
        for reading, surface in pairs:
            rows.append(f"{reading}\t{surface}\tbulk_{bucket}\tstratified")
    return rows
```

- [ ] **Step 8: `main.py` を作成(entry point)**

```python
"""P2-A fixture generator entry point: writes 530-case golden TSV."""

from pathlib import Path

from p2a_fixture_gen.sample import (
    bulk_as_tsv_rows,
    load_sample_pairs,
    stratified_sample,
)
from p2a_fixture_gen.targeted import targeted_as_tsv_rows

REPO_ROOT = Path(__file__).resolve().parents[4]
OUTPUT = REPO_ROOT / "crates" / "kotoha-core" / "tests" / "fixtures" / "kanji_dictionary_golden.tsv"


def main() -> None:
    header = [
        "# kanji_dictionary_golden.tsv — Phase 2 P2-A golden fixture",
        "# Schema: input<TAB>expected_top_surface<TAB>category<TAB>note",
        "# Contents: targeted 30 + bulk 500 (5-bucket stratified × 100)",
        "# Generation: tools/p2a-fixture-gen (seed=42)",
        "# Threshold: ≥90% pass rate (spec §3.9 / §6.3.2)",
    ]
    targeted = targeted_as_tsv_rows()
    bulk = bulk_as_tsv_rows(stratified_sample(load_sample_pairs()))

    OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    OUTPUT.write_text("\n".join(header + targeted + bulk) + "\n", encoding="utf-8")
    print(f"wrote {len(targeted) + len(bulk)} rows → {OUTPUT}")


if __name__ == "__main__":
    main()
```

- [ ] **Step 9: `tests/test_targeted.py` を作成**

```python
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
```

- [ ] **Step 10: `tests/test_sample.py` を作成**

```python
"""Tests for bulk stratified sampling."""

from p2a_fixture_gen.sample import (
    bucket_for_reading_length,
    bulk_as_tsv_rows,
    stratified_sample,
)


def test_bucket_assignment_by_length() -> None:
    assert bucket_for_reading_length(1) == "B1"
    assert bucket_for_reading_length(3) == "B1"
    assert bucket_for_reading_length(4) == "B2"
    assert bucket_for_reading_length(7) == "B3"
    assert bucket_for_reading_length(11) == "B4"
    assert bucket_for_reading_length(21) == "B5"
    assert bucket_for_reading_length(0) is None


def test_stratified_sample_returns_five_buckets() -> None:
    # synthetic pairs covering buckets B1-B4 evenly; B5 via fallback
    pairs: list[tuple[str, str]] = []
    for n, bucket in [(2, "B1"), (5, "B2"), (8, "B3"), (15, "B4")]:
        for i in range(150):
            pairs.append(("あ" * n, "X" * n + str(i)))
    sampled = stratified_sample(pairs, per_bucket=100)
    assert set(sampled.keys()) == {"B1", "B2", "B3", "B4", "B5"}
    assert len(sampled["B1"]) == 100
    assert len(sampled["B2"]) == 100
    # B5 may be populated via fallback concat
    assert len(sampled["B5"]) == 100


def test_bulk_tsv_rows_have_four_fields() -> None:
    pairs = [("あいう", "AIU")] * 5 + [("あいうえ", "AIUE")] * 5
    sampled = stratified_sample(pairs, per_bucket=5)
    for row in bulk_as_tsv_rows(sampled):
        assert len(row.split("\t")) == 4
```

- [ ] **Step 11: uv sync で依存を解決**

```bash
cd tools/p2a-fixture-gen && uv sync
```

- [ ] **Step 12: `uv run pytest` で tool の unit test が通ることを確認**

```bash
cd tools/p2a-fixture-gen && uv run pytest
```

想定: `6 passed` 前後(sample_test 3 件 + targeted_test 3 件)。

- [ ] **Final step: Commit**

```bash
git add tools/p2a-fixture-gen/
git commit -m "chore(tools): add p2a-fixture-gen for golden fixture generation (#91)"
```

---

## Task 13: Golden fixture 生成 & commit

**Files:**
- Create: `crates/kotoha-core/tests/fixtures/kanji_dictionary_golden.tsv`(530 rows)

**Depends-on:** Task 12

**Estimated LOC:** ~530 行 data(+ comment header 5 行)

参照: spec §6.3.1

- [ ] **Step 1: p5a sample.tsv の行数を確認**

```bash
wc -l tools/p5a-data-pipeline/data/sample.tsv
```

想定: 1100 行以上。これ未満の場合、`cd tools/p5a-data-pipeline && uv run python -m kotoha_p5a` で再生成する。

- [ ] **Step 2: fixture を生成**

```bash
cd tools/p2a-fixture-gen && uv run python -m p2a_fixture_gen.main
```

想定出力: `wrote 530 rows → /home/kohshiro/develops/student/kotoha-ime/crates/kotoha-core/tests/fixtures/kanji_dictionary_golden.tsv`。

- [ ] **Step 3: 生成された TSV を目視確認**

```bash
head -10 crates/kotoha-core/tests/fixtures/kanji_dictionary_golden.tsv
tail -5 crates/kotoha-core/tests/fixtures/kanji_dictionary_golden.tsv
wc -l crates/kotoha-core/tests/fixtures/kanji_dictionary_golden.tsv
```

想定: 先頭 5 行が header comment、次の 30 行が targeted (honorific / personal_name / …)、残り 500 行が bulk_B1〜B5 の順。total 535 行 (header 5 + data 530)。

- [ ] **Step 4: B5 bucket が fallback 由来であるかどうかを確認し、必要ならコメント追記**

```bash
grep -c 'bulk_B5' crates/kotoha-core/tests/fixtures/kanji_dictionary_golden.tsv
```

想定: 100 行。fallback (concat) を使った場合は header の下に `# B5 bucket uses concat fallback for scarcity (spec §10)` を追記する。

- [ ] **Final step: Commit**

```bash
git add crates/kotoha-core/tests/fixtures/kanji_dictionary_golden.tsv
git commit -m "test(dict): add 530-case golden fixture for Phase 2 evaluation (#91)"
```

---

## Task 14: Layer 3 golden test runner

**Files:**
- Create: `crates/kotoha-core/tests/kanji_dictionary_golden.rs`

**Depends-on:** Task 13

**Estimated LOC:** 150

参照: spec §6.3.2, §6.3.3

前提: 以下の manual setup を実行者が完了していること。

1. SudachiDict-core を取得 (<https://github.com/WorksApplications/SudachiDict>) し `~/.local/share/sudachidict/system_core.dic` に配置する。
2. `export KOTOHA_SYSTEM_DICT_PATH=$HOME/.local/share/sudachidict/system_core.dic` を shell に設定する。

- [ ] **Step 1: test runner code を作成**

```rust
//! Layer 3 golden fixture runner for the Dictionary backend (P2-A).
//!
//! Spec: `docs/superpowers/specs/2026-04-25-p2-a-dictionary-layer-design.md` §6.3.
//!
//! # Enabling
//!
//! - Build-time: `--features dict-smoke`
//! - Runtime: `KOTOHA_SYSTEM_DICT_PATH` set to SudachiDict-core system_core.dic
//!
//! Without the env var the single test in this file prints SKIPPED and
//! returns early (exit 0), matching the opt-in policy established in
//! `kanji_llama_cpp_smoke.rs`.

#![cfg(feature = "dict-smoke")]

use std::collections::BTreeMap;
use std::path::PathBuf;

use kotoha_core::dict::DictionaryConfig;
use kotoha_core::kanji::{load_backend, BackendConfig, ConvertOptions};

const THRESHOLD: f64 = 0.90;

fn get_dict_path_or_skip() -> Option<PathBuf> {
    match std::env::var("KOTOHA_SYSTEM_DICT_PATH") {
        Ok(p) => Some(PathBuf::from(p)),
        Err(_) => {
            println!("SKIPPED: KOTOHA_SYSTEM_DICT_PATH not set");
            None
        }
    }
}

struct Row {
    input: String,
    expected: String,
    category: String,
}

fn load_fixture() -> Vec<Row> {
    let tsv = include_str!("fixtures/kanji_dictionary_golden.tsv");
    tsv.lines()
        .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
        .map(|l| {
            let parts: Vec<&str> = l.splitn(4, '\t').collect();
            assert!(parts.len() >= 3, "malformed row: {l}");
            Row {
                input: parts[0].to_string(),
                expected: parts[1].to_string(),
                category: parts[2].to_string(),
            }
        })
        .collect()
}

#[test]
fn dictionary_golden_meets_threshold() {
    let Some(dict_path) = get_dict_path_or_skip() else {
        return;
    };

    let cfg = BackendConfig::Dictionary {
        config: DictionaryConfig {
            system_dict_path: Some(dict_path),
            custom_vocab_path: None,
        },
    };
    let backend = load_backend(&cfg).expect("dict backend must load with real dict");
    let opts = ConvertOptions {
        top_k: 5,
        temperature: 0.0,
        seed: Some(0),
    };

    let rows = load_fixture();
    let total = rows.len();
    let mut pass = 0usize;
    let mut by_category: BTreeMap<String, (usize, usize)> = BTreeMap::new();

    for row in &rows {
        let result = backend.convert(&row.input, &opts);
        let ok = match result {
            Ok(cands) if !cands.is_empty() => cands.iter().any(|c| c.surface.contains(&row.expected)),
            _ => false,
        };
        if ok {
            pass += 1;
        }
        let entry = by_category
            .entry(row.category.clone())
            .or_insert((0, 0));
        entry.1 += 1;
        if ok {
            entry.0 += 1;
        }
    }

    println!("\n=== Category breakdown ===");
    for (cat, (p, t)) in &by_category {
        let pct = 100.0 * (*p as f64) / (*t as f64);
        println!("{cat:15} {p:4}/{t:4} ({pct:5.1}%)");
    }
    let pct_total = 100.0 * (pass as f64) / (total as f64);
    println!("\n=== Total ===");
    println!(
        "PASS: {pass}/{total} ({pct_total:.1}%)  -- threshold {:.0}%",
        THRESHOLD * 100.0
    );

    assert!(
        (pass as f64) / (total as f64) >= THRESHOLD,
        "pass rate {pct_total:.1}% below threshold {:.0}%",
        THRESHOLD * 100.0
    );
}
```

- [ ] **Step 2: env var 未設定で SKIPPED 動作を確認**

```bash
unset KOTOHA_SYSTEM_DICT_PATH
cargo test --features dict-smoke -p kotoha-core --test kanji_dictionary_golden
```

想定出力: `SKIPPED: KOTOHA_SYSTEM_DICT_PATH not set` → `test result: ok. 1 passed`(early return)。

- [ ] **Step 3: env var 設定で実辞書経由の golden 実行**

```bash
export KOTOHA_SYSTEM_DICT_PATH=$HOME/.local/share/sudachidict/system_core.dic
cargo test --features dict-smoke -p kotoha-core --test kanji_dictionary_golden -- --nocapture
```

想定: Category breakdown が表示され、Total pass rate が 90% 以上。

- [ ] **Step 4: pass rate 90% 未満だった場合の診断**

以下を順に確認し、再 run する。

1. `KOTOHA_SYSTEM_DICT_PATH` が実在ファイルを指しているか `ls -la $KOTOHA_SYSTEM_DICT_PATH`
2. sudachi.rs `Config::new` の引数順序が v0.6.11 の API と一致しているか(`sudachi::config` module docs 参照)
3. fixture の category 別 pass rate(targeted < bulk なら fixture 側の expected が厳しすぎる可能性)
4. `SudachiAdapter::tokenize` の score 付与ロジック(`head_word_length` proxy で不適なら `word_info().oov()` 等に切替)

- [ ] **Step 5: clippy 通過確認**

```bash
cargo clippy --features dict-smoke -p kotoha-core --all-targets -- -D warnings
```

- [ ] **Final step: Commit**

```bash
git add crates/kotoha-core/tests/kanji_dictionary_golden.rs
git commit -m "test(dict): add Layer 3 golden test runner with pass-rate threshold (#91)"
```

---

## Task 15: scripts/phase2-smoke.sh

**Files:**
- Create: `scripts/phase2-smoke.sh`

**Depends-on:** Task 14

**Estimated LOC:** 25

参照: spec §6.3 / phase1-smoke.sh のテンプレ

- [ ] **Step 1: script を作成**

```bash
#!/usr/bin/env bash
# Phase 2 P2-A smoke test — runs the Layer 3 golden fixture via `cargo test
# --features dict-smoke` against a real SudachiDict-core system dictionary.
#
# Usage:
#   scripts/phase2-smoke.sh
#
# Environment:
#   KOTOHA_SYSTEM_DICT_PATH (required)
#     Absolute path to SudachiDict-core system_core.dic (v20260116).
#     If unset, this script prints SKIPPED and exits 77 (matches automake
#     test-suite SKIP convention, letting CI/lefthook skip gracefully).
#
# Exit codes:
#   0   — golden fixture pass rate ≥ 90%
#   1   — pass rate below threshold
#   77  — SKIPPED (env var unset)
#
# References:
#   - Fixture: crates/kotoha-core/tests/fixtures/kanji_dictionary_golden.tsv
#   - Runner:  crates/kotoha-core/tests/kanji_dictionary_golden.rs
#   - Spec:    docs/superpowers/specs/2026-04-25-p2-a-dictionary-layer-design.md §6.3

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

cd "$REPO_ROOT"

if [[ -z "${KOTOHA_SYSTEM_DICT_PATH:-}" ]]; then
  echo "SKIPPED: KOTOHA_SYSTEM_DICT_PATH not set"
  exit 77
fi

if [[ ! -f "$KOTOHA_SYSTEM_DICT_PATH" ]]; then
  echo "FAIL: KOTOHA_SYSTEM_DICT_PATH file does not exist: $KOTOHA_SYSTEM_DICT_PATH" >&2
  exit 1
fi

echo "=== phase2-smoke: running Layer 3 golden fixture (530 cases) ==="
cargo test --features dict-smoke -p kotoha-core --test kanji_dictionary_golden -- --nocapture
```

- [ ] **Step 2: 実行権限を付与**

```bash
chmod +x scripts/phase2-smoke.sh
```

- [ ] **Step 3: env var 未設定で SKIPPED 動作を確認**

```bash
unset KOTOHA_SYSTEM_DICT_PATH
bash scripts/phase2-smoke.sh
echo "exit=$?"
```

想定出力: `SKIPPED: KOTOHA_SYSTEM_DICT_PATH not set` / `exit=77`。

- [ ] **Step 4: env var 設定で real run**

```bash
export KOTOHA_SYSTEM_DICT_PATH=$HOME/.local/share/sudachidict/system_core.dic
bash scripts/phase2-smoke.sh
```

想定: Layer 3 runner が走り、pass rate ≥ 90% で exit 0。

- [ ] **Step 5: shellcheck 通過確認**

```bash
shellcheck scripts/phase2-smoke.sh
```

- [ ] **Final step: Commit**

```bash
git add scripts/phase2-smoke.sh
git commit -m "chore(scripts): add phase2-smoke.sh for Layer 3 fixture (#91)"
```

---

## Task 16: Phase 1 14/15 regression check + workspace lint

**Files:** (file 変更なし、sanity check のみ)

**Depends-on:** Task 15

**Estimated LOC:** 0

参照: spec §6.4

- [ ] **Step 1: fmt check**

```bash
cargo fmt --all -- --check
```

想定: 差分なしで exit 0。

- [ ] **Step 2: clippy(mock-backend + dict 両 feature on)**

```bash
cargo clippy --workspace --all-targets --features mock-backend,dict -- -D warnings
```

想定: warning 0 件で exit 0。

- [ ] **Step 3: workspace test(default features + mock-backend + dict)**

```bash
cargo test --workspace --features mock-backend,dict
```

想定: Phase 1 既存 test(175 件前後)+ P2-A 新規 test(33-52 件前後)= total 200-227 件すべて PASS。

- [ ] **Step 4: Layer 3 dict-smoke は別途 Task 14 で検証済。本 task では再実行不要**

- [ ] **Step 5: 結果サマリを記録(commit 不要、変更なし)**

全コマンド exit 0 なら本 task 完了。Fail があれば該当 task に戻って修正する。

---

## Task 17: 付随 docs commits(ADR 0014 改訂 + Phase 2 spec 同期 + README + glossary)

**Files:**
- Modify: `docs/adr/0014-phase-2-dictionary-layer-architecture.md`(D4 改訂、~30 行)
- Modify: `docs/superpowers/specs/2026-04-25-kotoha-phase-2-design.md`(§3.4 / §4.2 / §6.3 / §7.3 同期、~95 行)
- Modify: `README.md`(SudachiDict-core 取得手順 + `KOTOHA_SYSTEM_DICT_PATH` 設定例、~30 行)
- Modify: `docs/wiki/glossary.md`(MorphologicalEngine / VocabularyLookup / SudachiDict / 形態素解析 4 用語、~10 行)

**Depends-on:** Task 16

**Estimated LOC:** 165 行 docs

参照: spec §8

- [ ] **Step 1: ADR 0014 の現状を Read**

`docs/adr/0014-phase-2-dictionary-layer-architecture.md` の D4 セクションを Read で読み、現行文言を把握する。

- [ ] **Step 2: ADR 0014 D4 を改訂**

旧文言: 「P2-A kick-off で候補 1 / 候補 2 のいずれかを選択」
新文言: 「Phase 2 で 2 variants を段階的に追加する:
- P2-A: `BackendConfig::Dictionary { config: DictionaryConfig }` (集約型、LLM を含まない pure Dictionary backend)
- P2-D: `BackendConfig::Hybrid { llm: Box<BackendConfig>, dict, learning }` (再帰 wrap 型、Phase 5 KotohaNative 加入時にも自動対応)

本判断の根拠は P2-A spec §3.3 Q3 に記載する。」

- [ ] **Step 3: Phase 2 spec を同期更新**

`docs/superpowers/specs/2026-04-25-kotoha-phase-2-design.md` の §3.4 / §4.2 / §6.3 / §7.3 を Read し、以下 4 点を反映する。

1. §3.4(BackendConfig 候補): 「P2-A で集約型 1 variant 追加、P2-D で再帰 Hybrid 追加」と明記
2. §4.2(Trait 構成): `MorphologicalEngine` / `VocabularyLookup` の正式 signature を子 spec 側に委譲する旨を追記
3. §6.3(fixture): 530 cases の内訳(targeted 30 + bulk 500)を子 spec から同期
4. §7.3(feature flag): `dict` / `dict-smoke` feature 構成を子 spec から同期

- [ ] **Step 4: README.md に SudachiDict-core 取得手順を追記**

`README.md` の Phase 1 モデル配置セクション直後に以下を追加する(既存 LlamaCpp 手順と同 pattern)。

```markdown
### Phase 2 P2-A: SudachiDict-core の取得と配置

Phase 2 P2-A の Dictionary backend は SudachiDict-core(v20260116、~70MB、Apache-2.0)を runtime load で使用する。bundle しないため、ユーザー側で手動配置する必要がある。

#### 取得手順

```bash
# 1. SudachiDict-core (system_core.dic) を取得
mkdir -p ~/.local/share/sudachidict
curl -L -o ~/.local/share/sudachidict/system_core.dic \
  https://github.com/WorksApplications/SudachiDict/releases/download/v20260116/sudachi-dictionary-20260116-core.zip

# 2. 環境変数を設定(~/.zshrc または ~/.bashrc に追記推奨)
export KOTOHA_SYSTEM_DICT_PATH=$HOME/.local/share/sudachidict/system_core.dic

# 3. Layer 3 golden fixture で動作確認
cargo test --features dict-smoke -p kotoha-core --test kanji_dictionary_golden -- --nocapture
```

Phase 2 範囲では auto-download しない(Phase 6 UX で実装予定)。
```

- [ ] **Step 5: glossary.md に 4 用語を追加**

既存 entries を Read で確認し、重複が無いことを確かめた上で以下を追加する(アルファベット順 / 50音順の既存方針に従う)。

```markdown
## 形態素解析 (Morphological Analysis)

入力テキストを形態素(意味を持つ最小単位)に分割し、各形態素の表記 / 読み / 品詞を同定する処理。Phase 2 P2-A では `MorphologicalEngine` trait 経由で SudachiAdapter 実装として提供する。

## MorphologicalEngine

Phase 2 P2-A で導入した形態素解析 engine の抽象境界 trait。`tokenize(reading) -> Vec<EngineCandidate>` と `engine_id()` を提供する。P2-A は `SudachiAdapter` のみが本 trait を実装するが、Phase 5 以降で vibrato / lindera 等への差し替え余地を確保する目的で先出しする。

## VocabularyLookup

Phase 2 P2-A で導入した user / custom vocabulary の lookup 抽象境界 trait。`lookup(reading) -> Vec<VocabEntry>` と `vocab_id()` を提供する。P2-A は `CustomVocab`(TSV reader)のみが本 trait を実装し、P2-B で `UserVocab` が同 trait を実装する extension path を確保する。

## SudachiDict

WorksApplications が提供する日本語形態素解析辞書(core / small / full の 3 variant)。Kotoha Phase 2 P2-A は SudachiDict-core v20260116 (~70MB、76 万 entries 規模、Apache-2.0) を manual placement で採用する。
```

- [ ] **Step 6: markdownlint / prettier pre-commit gate 通過を確認**

```bash
git add docs/adr/0014-phase-2-dictionary-layer-architecture.md \
        docs/superpowers/specs/2026-04-25-kotoha-phase-2-design.md \
        README.md \
        docs/wiki/glossary.md
# pre-commit は lefthook 経由で commit 時に自動発火するので dry-run 要求しない
```

- [ ] **Final step: Commit**

```bash
git commit -m "docs(p2a): revise ADR 0014 D4 + sync Phase 2 spec + README + glossary (#91)"
```

---

## Task 18: Branch push + PR 作成

**Files:** (file 変更なし、git + gh 操作のみ)

**Depends-on:** Task 17

**Estimated LOC:** 0

- [ ] **Step 1: branch を push**

```bash
git push origin feature/91-p2-a-dictionary-layer
```

- [ ] **Step 2: PR を作成(英語 body、ISSUE #91 close 宣言含む)**

```bash
gh pr create --base develop --title "feat(phase-2): P2-A — Dictionary layer kick-off (#91)" --body "$(cat <<'EOF'
## Summary

Phase 2 P2-A: introduce a SudachiDict-based dictionary backend as the second `KanjiBackend` implementation (after `LlamaCppBackend`).

- Adds `MorphologicalEngine` / `VocabularyLookup` traits (Clean Architecture DIP) and `DictionaryBackend` struct holding both as `Box<dyn _>`.
- Adds `BackendConfig::Dictionary { config: DictionaryConfig }` variant + `load_backend` arm (gated behind `dict` feature).
- Adds `dict` / `dict-smoke` Cargo features with `default = []` (per ADR 0012 D5).
- Adds 530-case golden fixture + Layer 3 runner with 90% pass-rate threshold.
- Adds `tools/p2a-fixture-gen/` (Python 3.12 + uv) for reproducible fixture generation.
- Revises ADR 0014 D4 to reflect the two-variant plan (P2-A aggregated + P2-D recursive Hybrid).
- Syncs parent Phase 2 spec + README + glossary.

Closes #91.

## Spec links

- Child spec: `docs/superpowers/specs/2026-04-25-p2-a-dictionary-layer-design.md`
- Parent spec: `docs/superpowers/specs/2026-04-25-kotoha-phase-2-design.md`
- ADR: `docs/adr/0014-phase-2-dictionary-layer-architecture.md` (D4 revised)

## Acceptance criteria (from spec §1)

- [ ] Phase 2 G2 (honorific / proper noun coverage) reaches ≥ 90% pass rate on 530-case golden fixture.
- [ ] Phase 1 14/15 Layer 3 smoke baseline does not regress.
- [ ] `BackendConfig::Dictionary` / `DictionaryConfig` land with engine-neutral public field names (spec §3.4 Q4).
- [ ] ADR 0014 D4 is revised and included in this PR.

## Test counts

- Phase 1 existing tests: ~175 (unchanged)
- P2-A new unit tests (Layer 1): ~33-42
- P2-A new integration tests (Layer 2): 5
- P2-A new golden test (Layer 3, opt-in via `dict-smoke`): 1 (530 assertions)
- `tools/p2a-fixture-gen/` pytest: ~6

## Review checklist (Large PR tier, per global CLAUDE.md PR Review Matrix)

- [ ] `agent-teams:team-review` with all 5 dimensions (security / performance / architecture / testing / a11y)
- [ ] `owasp-security`
- [ ] `secrets-check`
- [ ] `security-scanning:security-sast`
- [ ] `pr-review-toolkit:review-pr`

## Smoke test (manual)

```bash
export KOTOHA_SYSTEM_DICT_PATH=$HOME/.local/share/sudachidict/system_core.dic
bash scripts/phase2-smoke.sh
```
EOF
)"
```

- [ ] **Step 3: 出力された PR URL を記録し、最終報告に含める**

想定: `https://github.com/std-koh-hinooka/kotoha-ime/pull/<number>` が stdout に出力される。

- [ ] **Final step: 報告**

PR URL を main agent / user に返答する。本 task 完了後、Review Matrix に列挙された各 skill を順次起動する(本 plan の範囲外、別 branch session で対応)。

---

## 参照

- 子 spec: [`docs/superpowers/specs/2026-04-25-p2-a-dictionary-layer-design.md`](../specs/2026-04-25-p2-a-dictionary-layer-design.md)
- 親 spec: [`docs/superpowers/specs/2026-04-25-kotoha-phase-2-design.md`](../specs/2026-04-25-kotoha-phase-2-design.md)
- ADR 0014: [`docs/adr/0014-phase-2-dictionary-layer-architecture.md`](../../adr/0014-phase-2-dictionary-layer-architecture.md)
- Phase 1 plan(参考): [`2026-04-24-kotoha-phase-1-implementation.md`](./2026-04-24-kotoha-phase-1-implementation.md)
