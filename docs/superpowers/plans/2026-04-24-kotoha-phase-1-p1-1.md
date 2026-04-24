# Kotoha Phase 1 — P1-1 Implementation Plan (kanji skeleton + MockBackend)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** kotoha-core::kanji module skeleton (公開 API 型: KanjiBackend trait / Candidate / ConvertOptions / BackendConfig / KanjiError / load_backend factory) と MockBackend を TDD で配備し、ZenzBackend は空 skeleton (todo!()) のみとする。Layer 1 (unit、新規 ≥ 20 件) + Layer 2 (mock integration、5 件) を PASS させる。

**Architecture:** kanji module は kotoha-core の top-level sub-module として kana / romaji / input と同列に配置する。KanjiBackend trait を central abstraction とし、実装 (MockBackend / ZenzBackend) は feature flag で切り分ける。validate_input / score_sort_dedupe pure fn を backend.rs に配置して全 backend で共有し、contract enforcement (§5.6 hiragana-only / ≤128 chars、§5.7 score-desc sort + surface dedupe + top_k truncate) を一元化する。

**Tech Stack:** Rust 2021 / MSRV 1.80 / thiserror 1 / tracing 0.1。llama-cpp-2 は P1-1 では [dependencies] に追加しない。zenz feature は P1-1 では空 flag (`zenz = []`) として定義し、zenz.rs は todo!() スケルトン。P1-2 で Cargo.toml と zenz.rs を書き換えて llama-cpp-2 を実装に配線する。

**Spec:** docs/superpowers/specs/2026-04-24-kotoha-phase-1-design.md §4.1 / §4.3 / §5 / §8.1 / §8.2

**Phase 1 overall plan:** docs/superpowers/plans/2026-04-24-kotoha-phase-1-implementation.md §"PR #2 — P1-1" (lines 721-752)

---

## 目次

- [マイルストーン位置付け](#マイルストーン位置付け)
- [ファイル構成](#ファイル構成)
- [P1-1 完了条件](#p1-1-完了条件)
- [共通規約](#共通規約)
- [Task P1-1-0: 実装 ISSUE + branch 作成 + baseline 確認](#task-p1-1-0-実装-issue--branch-作成--baseline-確認)
- [Task P1-1-1: Cargo.toml feature flag 追加 + baseline build](#task-p1-1-1-cargotoml-feature-flag-追加--baseline-build)
- [Task P1-1-2: candidate.rs TDD (Candidate + ConvertOptions)](#task-p1-1-2-candidaters-tdd-candidate--convertoptions)
- [Task P1-1-3: error.rs TDD (KanjiError)](#task-p1-1-3-errorrs-tdd-kanjierror)
- [Task P1-1-4: backend.rs Phase A — KanjiBackend trait + validate_input + score_sort_dedupe TDD](#task-p1-1-4-backendrs-phase-a--kanjibackend-trait--validate_input--score_sort_dedupe-tdd)
- [Task P1-1-5: backend.rs Phase B — BackendConfig enum](#task-p1-1-5-backendrs-phase-b--backendconfig-enum)
- [Task P1-1-6: mock.rs TDD (MockBackend)](#task-p1-1-6-mockrs-tdd-mockbackend)
- [Task P1-1-7: zenz.rs skeleton (ZenzBackend todo!() stub)](#task-p1-1-7-zenzrs-skeleton-zenzbackend-todo-stub)
- [Task P1-1-8: backend.rs Phase C — load_backend factory TDD](#task-p1-1-8-backendrs-phase-c--load_backend-factory-tdd)
- [Task P1-1-9: kanji/mod.rs + lib.rs re-export](#task-p1-1-9-kanjimodrs--librs-re-export)
- [Task P1-1-10: tests/kanji_mock.rs — Layer 2 integration tests (5 件)](#task-p1-1-10-testskanji_mockrs--layer-2-integration-tests-5-件)
- [Task P1-1-11: Multi-feature cargo check + lefthook pre-push](#task-p1-1-11-multi-feature-cargo-check--lefthook-pre-push)
- [Task P1-1-12: commit + push + PR 作成](#task-p1-1-12-commit--push--pr-作成)
- [Task P1-1-13: review + findings + merge](#task-p1-1-13-review--findings--merge)
- [Task P1-1-14: WBS ログ作成 + develop 直接 push](#task-p1-1-14-wbs-ログ作成--develop-直接-push)
- [P1-1 完了条件チェックリスト](#p1-1-完了条件チェックリスト)
- [Spec Coverage 確認](#spec-coverage-確認)
- [Self-Review 済み事項](#self-review-済み事項)

---

## マイルストーン位置付け

| 観点 | 内容 |
|---|---|
| Phase | 1 / Kana → Kanji |
| マイルストーン | P1-1 of 5 (P1-0 ... P1-4) |
| 前提 | P1-0 (assert.sh 抽出) が develop に merge 済み |
| 後続 | P1-2 (ZenzBackend 実装 + Layer 3 smoke) |
| 工数見積 | 1.5 日 |
| Scope tier | Medium (7 new + 2 modified files、約 500 LOC、Spec §11 の milestone table に準拠) |

---

## ファイル構成

### 新規作成 (7 ファイル)

```
crates/
└── kotoha-core/
    ├── src/
    │   └── kanji/
    │       ├── mod.rs              # 公開 API の re-export
    │       ├── candidate.rs        # Candidate + ConvertOptions
    │       ├── error.rs            # KanjiError enum
    │       ├── backend.rs          # KanjiBackend trait + validate_input + score_sort_dedupe + BackendConfig + load_backend
    │       ├── mock.rs             # MockBackend (feature = "mock-backend")
    │       └── zenz.rs             # ZenzBackend skeleton (feature = "zenz"、todo!())
    └── tests/
        └── kanji_mock.rs           # Layer 2 integration、5 件 (file-level #![cfg(feature = "mock-backend")])
```

### 変更 (2 ファイル)

- `crates/kotoha-core/Cargo.toml`: `[features]` ブロックを新設 (default / mock-backend / zenz / zenz-smoke の 4 flag)
- `crates/kotoha-core/src/lib.rs`: `pub mod kanji;` を追加し、`pub use` ブロックに `kanji::*` を追加

### 各ファイルの責務

| ファイル | 責務 |
|---|---|
| `crates/kotoha-core/Cargo.toml` | P1-1 feature flag 群を定義 (llama-cpp-2 は P1-2 で追加、本 PR では zenz は empty flag) |
| `crates/kotoha-core/src/lib.rs` | kanji module を workspace から可視化し、公開型 5 種 (+ load_backend fn) を re-export |
| `crates/kotoha-core/src/kanji/mod.rs` | kanji sub-module tree の entry point。backend / candidate / error を常時 `pub use`、mock / zenz は feature-gated `pub use` |
| `crates/kotoha-core/src/kanji/candidate.rs` | 値型 Candidate (`#[non_exhaustive]`, Debug+Clone+PartialEq) と ConvertOptions (`#[non_exhaustive]`, Default=top_k 5/temp 0/seed Some(0)) |
| `crates/kotoha-core/src/kanji/error.rs` | KanjiError (`#[non_exhaustive]`, thiserror 由来 5 variant)、spec §5.5 の定義を英語エラーメッセージで実装 |
| `crates/kotoha-core/src/kanji/backend.rs` | KanjiBackend trait + pub(crate) fn validate_input + pub(crate) fn score_sort_dedupe + BackendConfig enum + load_backend factory fn。本ファイルで contract (§5.6/§5.7) を一元化 |
| `crates/kotoha-core/src/kanji/mock.rs` | 決定論的 MockBackend 実装。`#[cfg(feature = "mock-backend")]` gate。spec §8.2 の hard-coded fixture 3 件 (にほんご / かんじ / あした) と未知入力 → 空 Vec を実装 |
| `crates/kotoha-core/src/kanji/zenz.rs` | ZenzBackend skeleton。P1-1 では todo!() stub のみ。`#[cfg(feature = "zenz")]` gate。P1-2 で llama-cpp-2 依存と実装を追加する |
| `crates/kotoha-core/tests/kanji_mock.rs` | Layer 2 integration test (spec §8.2、5 件)。file-level `#![cfg(feature = "mock-backend")]` で gate |

---

## P1-1 完了条件

以下すべてが満たされたら P1-1 完了とする。

- [ ] 実装 ISSUE (本 plan PR #<PLAN_PR> 由来ではなく別途起票) が作成され、merge 済みの PR で close されている
- [ ] `cargo check -p kotoha-core --no-default-features` PASS
- [ ] `cargo check -p kotoha-core` (default features) PASS
- [ ] `cargo check -p kotoha-core --features mock-backend` PASS
- [ ] `cargo check -p kotoha-core --features zenz` PASS (zenz は空 flag なので llama-cpp-2 依存なしに通る)
- [ ] `cargo check -p kotoha-core --features zenz-smoke` PASS
- [ ] `cargo check -p kotoha-core --all-features` PASS
- [ ] `cargo test --workspace` (default features) が baseline (P1-0 完了時点) + Layer 1 新規 ≥ 20 件で PASS
- [ ] `cargo test --workspace --features mock-backend` が Layer 1 + Layer 2 (5 件) 合計で PASS
- [ ] `cargo clippy --workspace --all-targets --all-features -- -D warnings` warnings ゼロ
- [ ] `cargo fmt --all --check` diff ゼロ
- [ ] lefthook pre-push が全コマンド PASS (build / clippy / test が default feature で走り green)
- [ ] PR が develop に squash-merge され、`feature/<IMPL_ISSUE>-kanji-skeleton-mockbackend` branch が削除済み
- [ ] WBS ログ `docs/wbs/2026-04-24-feature-<IMPL_ISSUE>-kanji-skeleton-mockbackend.md` が develop に push 済み

---

## 共通規約

本 plan を実装するサブエージェントは以下の規約を遵守する。

- **TDD 遵守**: Task P1-1-2 〜 P1-1-10 の各実装ステップは `Red (失敗するテスト)` → `Green (最小実装)` の順を守る。Candidate / ConvertOptions / KanjiError の一部だけは仕様が一意なので簡略化可だが、backend.rs 内の validate_input / score_sort_dedupe と MockBackend の convert 実装については明示的に Red → Green を踏む。
- **commit 頻度**: 各 Task (P1-1-1 〜 P1-1-10) 完了時点で 1 commit。検証のみの Task (P1-1-11) は commit しない。P1-1-0 は ISSUE + branch 操作のみで commit なし。
- **言語**: commit message / PR body / ISSUE body / rustdoc は英語。本 plan 本文と対話応答は日本語。
- **可視性**: module 内部 helper (validate_input / score_sort_dedupe など) は `pub(crate)` を基本とする。公開 API (KanjiBackend / Candidate / ConvertOptions / BackendConfig / KanjiError / load_backend / MockBackend / ZenzBackend) のみ `pub` とする。
- **feature flag 規律**: mock.rs は `#[cfg(feature = "mock-backend")]`、zenz.rs は `#[cfg(feature = "zenz")]`、tests/kanji_mock.rs は file-level `#![cfg(feature = "mock-backend")]`。テスト module も同じ gate を継承する。
- **`#[non_exhaustive]` 必須**: Candidate / ConvertOptions / BackendConfig / KanjiError の 4 型すべてに付与 (ADR 0006 準拠)。
- **rustdoc**: 全 pub 項目に `///` を付け、spec § 番号への参照を含める。trait / fn は `# Contract` / `# Errors` / `# Panics` (該当時) を明記する。
- **verbatim output**: 各 Task の verification step で `cargo test` / `cargo check` / `cargo clippy` を走らせた場合、サブエージェントは final report に stdout の head (先頭 5 行) + tail (末尾 15 行) を verbatim で含める (CLAUDE.md「Sub-agent Self-Report is Untrusted」節準拠)。
- **`--no-verify` 禁止**: push 時に lefthook pre-push を `--no-verify` で skip することは禁止。hook が fail したら原因を直す。
- **placeholder 置換**: 本 plan 中の `<IMPL_ISSUE>` は Task P1-1-0 で作成した実装 ISSUE 番号に、`<PR_NUMBER>` は Task P1-1-12 で作成した PR 番号に、`<MERGE_COMMIT>` は P1-1-13 で取得する merge commit SHA に置換する。

---

## Task P1-1-0: 実装 ISSUE + branch 作成 + baseline 確認

**Files:** (ローカル変更なし、GitHub + git 操作のみ)

注: plan PR (#63) は本 plan 文書を書くための PR である。本 Task で作成する「実装 ISSUE」はそれとは別に、実際のコード実装を追跡するための新 ISSUE である。

- [ ] **Step 1: 作業ディレクトリと baseline 確認**

Run:

```bash
cd /home/kohshiro/develops/student/kotoha-ime
git checkout develop
git pull
git status
git log --oneline -3
```

Expected: `develop` が `origin/develop` と同期、working tree clean。`git log` の先頭に P1-0 の merge commit (`refactor(scripts): extract assert.sh ...`) があるはず。

- [ ] **Step 2: baseline `cargo test` の件数を記録**

Run:

```bash
cargo test --workspace 2>&1 | tail -20
```

Expected: すべて PASS。`test result: ok. N passed` の N 値 (P1-0 完了時点の test 件数) を記録する (Task P1-1-11 で regression 確認に使う)。

- [ ] **Step 3: 実装 ISSUE を作成**

Run:

```bash
gh issue create \
  --title "P1-1: kanji skeleton + MockBackend (Layer 1 + Layer 2)" \
  --body "Phase 1 milestone P1-1: add the kotoha-core::kanji module skeleton with public API types, MockBackend, and Layer 1/2 tests.

## Scope

- New module \`crates/kotoha-core/src/kanji/\` with 6 files
  - \`mod.rs\` (re-exports)
  - \`candidate.rs\` (Candidate + ConvertOptions)
  - \`error.rs\` (KanjiError)
  - \`backend.rs\` (KanjiBackend trait + validate_input + score_sort_dedupe + BackendConfig + load_backend)
  - \`mock.rs\` (MockBackend, feature = mock-backend)
  - \`zenz.rs\` (skeleton only, todo!() stub, feature = zenz)
- Add feature flags to \`crates/kotoha-core/Cargo.toml\` (default / mock-backend / zenz / zenz-smoke)
- Update \`crates/kotoha-core/src/lib.rs\` re-exports
- New integration test file \`crates/kotoha-core/tests/kanji_mock.rs\` (Layer 2, 5 tests, feature-gated)

## Out of Scope

- ZenzBackend real implementation (P1-2)
- llama-cpp-2 dependency wiring (P1-2)
- Layer 3 Zenz smoke (P1-2)
- CLI kotoha-kanji (P1-3)
- ADR 0009 / 0010 / 0011 (P1-4)

## Reference

- Spec: \`docs/superpowers/specs/2026-04-24-kotoha-phase-1-design.md\` §4.1 / §4.3 / §5 / §8.1 / §8.2
- Phase 1 overall plan: \`docs/superpowers/plans/2026-04-24-kotoha-phase-1-implementation.md\` §\"PR #2 — P1-1\"
- Detailed plan: \`docs/superpowers/plans/2026-04-24-kotoha-phase-1-p1-1.md\`"
```

Expected: ISSUE が作成され、URL と番号が表示される。番号を `<IMPL_ISSUE>` として記録する。

- [ ] **Step 4: 実装 branch 作成**

Run (`<IMPL_ISSUE>` は Step 3 で取得した値):

```bash
git checkout -b feature/<IMPL_ISSUE>-kanji-skeleton-mockbackend develop
git branch --show-current
```

Expected: `feature/<IMPL_ISSUE>-kanji-skeleton-mockbackend` branch が develop から作成され、checkout される。

---

## Task P1-1-1: Cargo.toml feature flag 追加 + baseline build

**Files:**
- Modify: `crates/kotoha-core/Cargo.toml`

- [ ] **Step 1: 現状 Cargo.toml を確認**

Read file: `crates/kotoha-core/Cargo.toml`

Expected: `[features]` ブロックが存在しないことを確認する。

- [ ] **Step 2: `[features]` ブロックを追加**

Edit `crates/kotoha-core/Cargo.toml`:

old_string:

```toml
[dev-dependencies]
proptest = { workspace = true }
```

new_string:

```toml
[dev-dependencies]
proptest = { workspace = true }

[features]
default = []
# `mock-backend` exposes the deterministic MockBackend to cross-crate integration tests.
# Cannot be reduced to `#[cfg(test)]` because downstream test files live in a separate
# crate (the `tests/` directory) and need the public surface.
mock-backend = []
# `zenz` gates ZenzBackend. In P1-1 this is an empty flag (no `dep:llama-cpp-2`) because
# the real backend skeleton only contains todo!() stubs. P1-2 will replace this with
# `zenz = ["dep:llama-cpp-2"]` once the llama-cpp-2 dependency is wired up.
zenz = []
# `zenz-smoke` is the opt-in gate for Layer 3 live-inference smoke tests (added in P1-2).
# Kept separate from `zenz` so that lefthook pre-push, which only runs default features,
# does not attempt to load a real GGUF model.
zenz-smoke = ["zenz"]
```

- [ ] **Step 3: 5 通りの feature combination で check が通ることを確認**

Run:

```bash
cargo check -p kotoha-core --no-default-features
cargo check -p kotoha-core
cargo check -p kotoha-core --features mock-backend
cargo check -p kotoha-core --features zenz
cargo check -p kotoha-core --features zenz-smoke
cargo check -p kotoha-core --all-features
```

Expected: 6 コマンド全てが `Finished dev profile` で終わる (P1-1 本体コードを追加する前なので、features を定義しただけでは compile 挙動は変わらない)。

- [ ] **Step 4: commit**

Run:

```bash
git add crates/kotoha-core/Cargo.toml
git commit -m "chore(kotoha-core): add feature flags for Phase 1 (mock-backend / zenz / zenz-smoke)"
```

Expected: `1 file changed`。

---

## Task P1-1-2: candidate.rs TDD (Candidate + ConvertOptions)

**Files:**
- Create: `crates/kotoha-core/src/kanji/mod.rs` (初回作成、本 Task では `mod candidate; pub use ...` のみ、他 module は後続 Task で追記)
- Create: `crates/kotoha-core/src/kanji/candidate.rs`
- Modify: `crates/kotoha-core/src/lib.rs` (`pub mod kanji;` を一時的に追加する。Task P1-1-9 で re-export を拡充する)

spec §5.1 (Candidate) と §5.2 (ConvertOptions) をそのまま実装する。

- [ ] **Step 1: kanji ディレクトリ作成**

Run:

```bash
mkdir -p crates/kotoha-core/src/kanji
```

Expected: `crates/kotoha-core/src/kanji/` が作成される。

- [ ] **Step 2: `crates/kotoha-core/src/kanji/mod.rs` を Write で作成 (初回版)**

Content:

```rust
//! Kana-to-kanji conversion subsystem (Phase 1).
//!
//! This module is being populated incrementally in P1-1. Task P1-1-9 finalizes
//! the `pub use` surface. Until then, only `candidate` is declared.
//!
//! Spec: `docs/superpowers/specs/2026-04-24-kotoha-phase-1-design.md` §4-§8.

mod candidate;

pub use candidate::{Candidate, ConvertOptions};
```

- [ ] **Step 3: `crates/kotoha-core/src/lib.rs` に `pub mod kanji;` を一時追加**

Edit `crates/kotoha-core/src/lib.rs`:

old_string:

```rust
pub mod error;
pub mod input;
pub mod kana;
pub mod romaji;

pub use error::{Error, Result};
pub use input::{InputContext, InputMode, InputStep};
pub use romaji::{ConvertStep, RomajiConverter};
```

new_string:

```rust
pub mod error;
pub mod input;
pub mod kana;
pub mod kanji;
pub mod romaji;

pub use error::{Error, Result};
pub use input::{InputContext, InputMode, InputStep};
pub use kanji::{Candidate, ConvertOptions};
pub use romaji::{ConvertStep, RomajiConverter};
```

Task P1-1-9 で kanji re-export を拡充する。

- [ ] **Step 4: Write `crates/kotoha-core/src/kanji/candidate.rs` — 最初は stub + 失敗する tests**

Content:

```rust
//! Value types for kanji conversion results and options.
//!
//! Spec: `docs/superpowers/specs/2026-04-24-kotoha-phase-1-design.md` §5.1, §5.2.

/// A single kanji conversion candidate.
///
/// Phase 2+ may add fields (for example confidence sources or tokenization metadata),
/// so this type is marked `#[non_exhaustive]` per ADR 0006.
///
/// # Invariants
///
/// - `surface` is a UTF-8 string, typically containing kanji, hiragana, and/or katakana.
/// - `score` is the aggregated log-probability of the candidate; larger is better.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct Candidate {
    /// Kanji-mixed surface form (UTF-8).
    pub surface: String,
    /// Candidate score (aggregated log-probability; larger is better).
    pub score: f32,
}

impl Candidate {
    /// Constructs a new [`Candidate`].
    ///
    /// # Postconditions
    ///
    /// - The returned value's `surface` equals `surface.into()`.
    /// - The returned value's `score` equals the supplied `score` bit-for-bit.
    pub fn new(surface: impl Into<String>, score: f32) -> Self {
        Self {
            surface: surface.into(),
            score,
        }
    }
}

/// Tunable parameters for a single kanji conversion call.
///
/// Phase 2+ may add fields (for example sampling strategy or penalty knobs),
/// so this type is marked `#[non_exhaustive]` per ADR 0006.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct ConvertOptions {
    /// Maximum number of candidates to return. `0` yields an empty `Vec`.
    pub top_k: usize,
    /// Sampling temperature. `0.0` means greedy decoding.
    pub temperature: f32,
    /// Sampling seed. `Some(s)` pins deterministic output; `None` is non-deterministic.
    pub seed: Option<u64>,
}

impl Default for ConvertOptions {
    /// Default options: `top_k = 5`, `temperature = 0.0`, `seed = Some(0)`.
    ///
    /// Combined defaults yield deterministic greedy decoding, which is the
    /// baseline Layer 3 smoke test and E2E smoke test use.
    fn default() -> Self {
        Self {
            top_k: 5,
            temperature: 0.0,
            seed: Some(0),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn candidate_new_constructs_with_surface_and_score() {
        let c = Candidate::new("日本語", 0.9);
        assert_eq!(c.surface, "日本語");
        assert_eq!(c.score, 0.9);
    }

    #[test]
    fn candidate_is_clone_and_eq() {
        let a = Candidate::new("漢字", 0.5);
        let b = a.clone();
        assert_eq!(a, b);
    }

    #[test]
    fn convert_options_default_is_top_k_5_temp_0_seed_0() {
        let opt = ConvertOptions::default();
        assert_eq!(opt.top_k, 5);
        assert_eq!(opt.temperature, 0.0);
        assert_eq!(opt.seed, Some(0));
    }

    #[test]
    fn convert_options_is_clone_and_eq() {
        let a = ConvertOptions::default();
        let b = a.clone();
        assert_eq!(a, b);
    }

    #[test]
    fn convert_options_custom_fields_round_trip() {
        let opt = ConvertOptions {
            top_k: 10,
            temperature: 0.7,
            seed: None,
        };
        assert_eq!(opt.top_k, 10);
        assert_eq!(opt.temperature, 0.7);
        assert!(opt.seed.is_none());
    }
}
```

- [ ] **Step 5: Red — テストを走らせて compile/PASS を確認**

Run:

```bash
cargo test -p kotoha-core --lib kanji::candidate::tests
```

Expected: 5 tests passed (`candidate_new_constructs_with_surface_and_score`、`candidate_is_clone_and_eq`、`convert_options_default_is_top_k_5_temp_0_seed_0`、`convert_options_is_clone_and_eq`、`convert_options_custom_fields_round_trip`)。

> Candidate / ConvertOptions は spec §5.1 / §5.2 で仕様が一意に確定しているため、実装と test を同じ Step でまとめて書き、Red → Green の厳密な分離は省略する (Phase 0 M2 の error module と同様の運用)。

- [ ] **Step 6: clippy + fmt**

Run:

```bash
cargo clippy -p kotoha-core --all-targets --all-features -- -D warnings
cargo fmt --all --check
```

Expected: warnings ゼロ、fmt diff ゼロ。

- [ ] **Step 7: commit**

Run:

```bash
git add crates/kotoha-core/src/lib.rs crates/kotoha-core/src/kanji/mod.rs crates/kotoha-core/src/kanji/candidate.rs
git commit -m "feat(kanji): add Candidate and ConvertOptions types with Default"
```

Expected: `3 files changed`。

---

## Task P1-1-3: error.rs TDD (KanjiError)

**Files:**
- Create: `crates/kotoha-core/src/kanji/error.rs`
- Modify: `crates/kotoha-core/src/kanji/mod.rs` (add `mod error; pub use error::KanjiError;`)

spec §5.5 の KanjiError を実装する。5 variant、`#[non_exhaustive]`、`thiserror::Error` derive。

- [ ] **Step 1: mod.rs に error 宣言を追加**

Edit `crates/kotoha-core/src/kanji/mod.rs`:

old_string:

```rust
mod candidate;

pub use candidate::{Candidate, ConvertOptions};
```

new_string:

```rust
mod candidate;
mod error;

pub use candidate::{Candidate, ConvertOptions};
pub use error::KanjiError;
```

- [ ] **Step 2: Write `crates/kotoha-core/src/kanji/error.rs`**

Content:

```rust
//! Error type for the kanji subsystem.
//!
//! Spec: `docs/superpowers/specs/2026-04-24-kotoha-phase-1-design.md` §5.5.
//!
//! All error messages are in English per the project convention for backend-facing
//! diagnostics.

use std::path::PathBuf;

use thiserror::Error;

/// Errors returned from the kanji subsystem.
///
/// Marked `#[non_exhaustive]` per ADR 0006 so that Phase 2+ can add variants
/// (for example cache / learning failures) without breaking external match
/// sites.
#[non_exhaustive]
#[derive(Debug, Error)]
pub enum KanjiError {
    /// The caller supplied input that violates the contract (hiragana-only, max 128 chars).
    #[error("input violates contract: {reason}")]
    InvalidInput {
        /// Human-readable reason for the rejection.
        reason: String,
    },

    /// The requested model file does not exist at the given path.
    #[error("model file not found: {}", path.display())]
    ModelNotFound {
        /// Absolute path that was probed.
        path: PathBuf,
    },

    /// Loading the model file succeeded at the filesystem level but failed at
    /// parse / initialization time (wrapped by the underlying backend library).
    #[error("model load failed: {source}")]
    ModelLoadFailed {
        /// Underlying error reported by the backend library.
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },

    /// Inference failed inside the backend.
    #[error("backend inference failed: {reason}")]
    Backend {
        /// Human-readable reason reported by the backend.
        reason: String,
    },

    /// The caller asked for a backend whose feature flag is not enabled at build time.
    #[error("required feature not enabled at build time: {feature}")]
    FeatureDisabled {
        /// Name of the Cargo feature that must be enabled.
        feature: &'static str,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_input_display_contains_reason() {
        let err = KanjiError::InvalidInput {
            reason: "non-hiragana character".to_string(),
        };
        let msg = format!("{err}");
        assert!(
            msg.contains("non-hiragana character"),
            "display should include the reason: {msg}"
        );
    }

    #[test]
    fn model_not_found_display_contains_path() {
        let err = KanjiError::ModelNotFound {
            path: PathBuf::from("/tmp/zenz.gguf"),
        };
        let msg = format!("{err}");
        assert!(
            msg.contains("/tmp/zenz.gguf"),
            "display should include the path: {msg}"
        );
    }

    #[test]
    fn feature_disabled_display_contains_feature_name() {
        let err = KanjiError::FeatureDisabled { feature: "zenz" };
        let msg = format!("{err}");
        assert!(
            msg.contains("zenz"),
            "display should include the feature name: {msg}"
        );
    }

    #[test]
    fn backend_display_contains_reason() {
        let err = KanjiError::Backend {
            reason: "token decode failed".to_string(),
        };
        let msg = format!("{err}");
        assert!(
            msg.contains("token decode failed"),
            "display should include the reason: {msg}"
        );
    }

    #[test]
    fn kanji_error_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<KanjiError>();
    }
}
```

- [ ] **Step 3: テスト実行**

Run:

```bash
cargo test -p kotoha-core --lib kanji::error::tests
```

Expected: 5 tests passed。

- [ ] **Step 4: clippy + fmt**

Run:

```bash
cargo clippy -p kotoha-core --all-targets --all-features -- -D warnings
cargo fmt --all --check
```

Expected: warnings ゼロ、fmt diff ゼロ。

- [ ] **Step 5: commit**

Run:

```bash
git add crates/kotoha-core/src/kanji/mod.rs crates/kotoha-core/src/kanji/error.rs
git commit -m "feat(kanji): add KanjiError with 5 variants per spec §5.5"
```

Expected: `2 files changed`。

---

## Task P1-1-4: backend.rs Phase A — KanjiBackend trait + validate_input + score_sort_dedupe TDD

**Files:**
- Create: `crates/kotoha-core/src/kanji/backend.rs` (Phase A は trait + 2 helper、BackendConfig は P1-1-5、load_backend は P1-1-8)
- Modify: `crates/kotoha-core/src/kanji/mod.rs` (add `mod backend; pub use backend::KanjiBackend;`)

本 Task は backend.rs を 3 段階に分けて成長させるうちの Phase A。以下 3 つを同時に実装する。

1. `KanjiBackend` trait (spec §5.3)
2. `pub(crate) fn validate_input` (spec §5.6 の契約検査: hiragana only + ≤128 chars)
3. `pub(crate) fn score_sort_dedupe` (spec §5.7 の出力保証: score 降順 + surface dedupe + top_k truncate)

TDD は 2 helper 関数に対して厳密に Red → Green を踏む。trait 定義自体は仕様一意なので簡略化する。

- [ ] **Step 1: mod.rs に backend 宣言を追加**

Edit `crates/kotoha-core/src/kanji/mod.rs`:

old_string:

```rust
mod candidate;
mod error;

pub use candidate::{Candidate, ConvertOptions};
pub use error::KanjiError;
```

new_string:

```rust
mod backend;
mod candidate;
mod error;

pub use backend::KanjiBackend;
pub use candidate::{Candidate, ConvertOptions};
pub use error::KanjiError;
```

- [ ] **Step 2: Write `crates/kotoha-core/src/kanji/backend.rs` 初版 (Red: helper が未実装 + tests が失敗)**

Content:

```rust
//! Backend abstraction and shared helpers for the kanji subsystem.
//!
//! Spec: `docs/superpowers/specs/2026-04-24-kotoha-phase-1-design.md` §5.3, §5.6, §5.7.
//!
//! The [`KanjiBackend`] trait defines the central abstraction for pluggable
//! conversion backends (Mock / Zenz / future). The `pub(crate)` helpers
//! [`validate_input`] and [`score_sort_dedupe`] live here so that every
//! backend implementation enforces the same input contract and output
//! guarantees without reimplementing the logic.

use crate::kanji::{Candidate, ConvertOptions, KanjiError};

/// Abstraction for a kana-to-kanji conversion backend.
///
/// Implementations are not required to be `Send + Sync` in Phase 1
/// (the CLI is single-threaded). Phase 3 may revisit this when wiring
/// the backend into an IBus engine running on a separate thread.
///
/// # Contract (spec §5.3)
///
/// Implementations MUST:
///
/// - Reject non-hiragana input (hiragana block `U+3041..=U+309F` plus the
///   prolonged-sound mark `ー U+30FC` is the only accepted alphabet) by
///   returning [`KanjiError::InvalidInput`]. Use the shared
///   [`validate_input`] helper to ensure a uniform error shape.
/// - Reject input longer than 128 characters (counted by
///   `str::chars().count()`) with [`KanjiError::InvalidInput`].
/// - Return candidates sorted in descending `score` order.
/// - Return candidates with unique `surface` (a surface that the model
///   produces with multiple scores collapses to the one with the highest
///   score).
/// - Return at most `options.top_k` candidates.
/// - Treat `options.top_k == 0` as a request for an empty `Vec`, not an
///   error.
///
/// Use [`score_sort_dedupe`] to enforce the last four properties in a single
/// call on the raw model output.
///
/// # Errors
///
/// - [`KanjiError::InvalidInput`] when input violates §5.6.
/// - [`KanjiError::Backend`] when inference fails inside the backend.
/// - [`KanjiError::ModelLoadFailed`] / [`KanjiError::ModelNotFound`] —
///   typically surfaced only from the constructor, not from `convert`.
pub trait KanjiBackend {
    /// Returns a human-readable identifier for the active model.
    ///
    /// Examples: `"mock"`, `"zenz-v2.5-medium"`. Used for logging and
    /// CLI diagnostics.
    fn model_id(&self) -> &str;

    /// Converts a hiragana string into top-`options.top_k` kanji candidates.
    ///
    /// # Contract
    ///
    /// See the trait-level `# Contract` section.
    ///
    /// # Errors
    ///
    /// See the trait-level `# Errors` section.
    fn convert(
        &self,
        input: &str,
        options: &ConvertOptions,
    ) -> Result<Vec<Candidate>, KanjiError>;
}

/// Validates that `input` satisfies the backend input contract (spec §5.6).
///
/// # Accepted characters
///
/// - Hiragana block: `U+3041..=U+309F`
/// - Prolonged sound mark: `U+30FC` (ー)
///
/// Empty input is accepted (the backend returns an empty `Vec`).
///
/// # Errors
///
/// - [`KanjiError::InvalidInput`] if `input` contains any character outside
///   the accepted ranges or if `input.chars().count() > 128`.
pub(crate) fn validate_input(input: &str) -> Result<(), KanjiError> {
    let count = input.chars().count();
    if count > 128 {
        return Err(KanjiError::InvalidInput {
            reason: format!(
                "input length {count} exceeds the 128-character limit"
            ),
        });
    }
    for ch in input.chars() {
        let ok = matches!(ch, '\u{3041}'..='\u{309F}' | '\u{30FC}');
        if !ok {
            return Err(KanjiError::InvalidInput {
                reason: format!(
                    "character {ch:?} is outside the hiragana block (U+3041..=U+309F) and is not the prolonged sound mark (U+30FC)"
                ),
            });
        }
    }
    Ok(())
}

/// Enforces the output ordering and deduplication contract from spec §5.7.
///
/// Steps:
///
/// 1. Sort `candidates` in descending `score` order (stable, NaN treated as equal).
/// 2. Deduplicate by `surface`, keeping the first occurrence (which is the
///    highest-score entry thanks to step 1).
/// 3. Truncate to `top_k` entries. If `top_k == 0`, the returned `Vec` is empty.
///
/// # Postconditions
///
/// - `result.len() <= top_k`.
/// - For every adjacent pair `(result[i], result[i+1])`, `result[i].score >= result[i+1].score`.
/// - No two entries in `result` share the same `surface`.
pub(crate) fn score_sort_dedupe(
    mut candidates: Vec<Candidate>,
    top_k: usize,
) -> Vec<Candidate> {
    if top_k == 0 {
        return Vec::new();
    }
    candidates.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let mut seen: Vec<String> = Vec::with_capacity(candidates.len());
    let mut out: Vec<Candidate> = Vec::with_capacity(top_k.min(candidates.len()));
    for c in candidates {
        if seen.iter().any(|s| s == &c.surface) {
            continue;
        }
        seen.push(c.surface.clone());
        out.push(c);
        if out.len() >= top_k {
            break;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    // ======================================================================
    // validate_input
    // ======================================================================

    #[test]
    fn validate_input_accepts_empty() {
        assert!(validate_input("").is_ok());
    }

    #[test]
    fn validate_input_accepts_hiragana() {
        assert!(validate_input("にほんご").is_ok());
    }

    #[test]
    fn validate_input_accepts_choonpu() {
        assert!(validate_input("ばー").is_ok());
    }

    #[test]
    fn validate_input_rejects_latin() {
        let err = validate_input("abc").expect_err("latin input must be rejected");
        assert!(matches!(err, KanjiError::InvalidInput { .. }));
    }

    #[test]
    fn validate_input_rejects_kanji() {
        let err = validate_input("日本").expect_err("kanji input must be rejected");
        assert!(matches!(err, KanjiError::InvalidInput { .. }));
    }

    #[test]
    fn validate_input_rejects_katakana() {
        let err =
            validate_input("カタカナ").expect_err("katakana input must be rejected");
        assert!(matches!(err, KanjiError::InvalidInput { .. }));
    }

    #[test]
    fn validate_input_rejects_space() {
        let err = validate_input(" ").expect_err("space input must be rejected");
        assert!(matches!(err, KanjiError::InvalidInput { .. }));
    }

    #[test]
    fn validate_input_rejects_digit() {
        let err = validate_input("1").expect_err("digit input must be rejected");
        assert!(matches!(err, KanjiError::InvalidInput { .. }));
    }

    #[test]
    fn validate_input_rejects_over_128_chars() {
        let s = "あ".repeat(129);
        let err =
            validate_input(&s).expect_err("input of 129 chars must be rejected");
        assert!(matches!(err, KanjiError::InvalidInput { .. }));
    }

    #[test]
    fn validate_input_accepts_exactly_128_chars() {
        let s = "あ".repeat(128);
        assert!(validate_input(&s).is_ok());
    }

    // ======================================================================
    // score_sort_dedupe
    // ======================================================================

    #[test]
    fn score_sort_dedupe_empty_input_returns_empty() {
        let out = score_sort_dedupe(Vec::new(), 5);
        assert!(out.is_empty());
    }

    #[test]
    fn score_sort_dedupe_top_k_zero_returns_empty() {
        let input = vec![Candidate::new("日本語", 0.9)];
        let out = score_sort_dedupe(input, 0);
        assert!(out.is_empty());
    }

    #[test]
    fn score_sort_dedupe_sorts_descending_by_score() {
        let input = vec![
            Candidate::new("二本後", 0.3),
            Candidate::new("日本語", 0.9),
            Candidate::new("日本後", 0.5),
        ];
        let out = score_sort_dedupe(input, 5);
        assert_eq!(out.len(), 3);
        assert_eq!(out[0].surface, "日本語");
        assert_eq!(out[1].surface, "日本後");
        assert_eq!(out[2].surface, "二本後");
    }

    #[test]
    fn score_sort_dedupe_dedupes_surface_keeping_highest_score() {
        let input = vec![
            Candidate::new("日本語", 0.3),
            Candidate::new("日本語", 0.9),
            Candidate::new("二本後", 0.5),
        ];
        let out = score_sort_dedupe(input, 5);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].surface, "日本語");
        assert_eq!(out[0].score, 0.9);
        assert_eq!(out[1].surface, "二本後");
    }

    #[test]
    fn score_sort_dedupe_truncates_to_top_k() {
        let input = vec![
            Candidate::new("a", 0.9),
            Candidate::new("b", 0.8),
            Candidate::new("c", 0.7),
            Candidate::new("d", 0.6),
        ];
        let out = score_sort_dedupe(input, 2);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].surface, "a");
        assert_eq!(out[1].surface, "b");
    }

    #[test]
    fn score_sort_dedupe_handles_single_candidate() {
        let input = vec![Candidate::new("日本語", 0.9)];
        let out = score_sort_dedupe(input, 5);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].surface, "日本語");
    }

    #[test]
    fn score_sort_dedupe_stable_for_equal_scores() {
        // Two candidates with identical score must retain input order.
        let input = vec![
            Candidate::new("first", 0.5),
            Candidate::new("second", 0.5),
        ];
        let out = score_sort_dedupe(input, 5);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].surface, "first");
        assert_eq!(out[1].surface, "second");
    }
}
```

- [ ] **Step 3: テスト実行**

Run:

```bash
cargo test -p kotoha-core --lib kanji::backend::tests
```

Expected: 17 tests passed (10 validate_input + 7 score_sort_dedupe)。

> 本 Task で一度に実装 + テストを書いているため、厳密な Red フェーズは省略している。ただし `validate_input` / `score_sort_dedupe` 各関数は仕様が複数の edge case (空 / top_k=0 / dedupe 優先順 / 128 境界) を含むので、test で挙動を個別に 固定する。実装担当のサブエージェントが TDD を厳密に守りたい場合は、Step 2 で helper fn の body を `todo!()` にしてから Step 3 で Red を確認し、改めて実装して Green にする運用でも良い。

- [ ] **Step 4: clippy + fmt**

Run:

```bash
cargo clippy -p kotoha-core --all-targets --all-features -- -D warnings
cargo fmt --all --check
```

Expected: warnings ゼロ、fmt diff ゼロ。

- [ ] **Step 5: commit**

Run:

```bash
git add crates/kotoha-core/src/kanji/mod.rs crates/kotoha-core/src/kanji/backend.rs
git commit -m "feat(kanji): add KanjiBackend trait, validate_input, score_sort_dedupe with tests"
```

Expected: `2 files changed`。

---

## Task P1-1-5: backend.rs Phase B — BackendConfig enum

**Files:**
- Modify: `crates/kotoha-core/src/kanji/backend.rs` (append BackendConfig definition + tests)
- Modify: `crates/kotoha-core/src/kanji/mod.rs` (append `pub use backend::BackendConfig;`)

- [ ] **Step 1: mod.rs の re-export を更新**

Edit `crates/kotoha-core/src/kanji/mod.rs`:

old_string:

```rust
pub use backend::KanjiBackend;
pub use candidate::{Candidate, ConvertOptions};
pub use error::KanjiError;
```

new_string:

```rust
pub use backend::{BackendConfig, KanjiBackend};
pub use candidate::{Candidate, ConvertOptions};
pub use error::KanjiError;
```

- [ ] **Step 2: backend.rs に BackendConfig を追加**

Edit `crates/kotoha-core/src/kanji/backend.rs`:

old_string (先頭 `use` ブロック):

```rust
use crate::kanji::{Candidate, ConvertOptions, KanjiError};
```

new_string:

```rust
use std::path::PathBuf;

use crate::kanji::{Candidate, ConvertOptions, KanjiError};
```

次に、`pub trait KanjiBackend` の直前に BackendConfig を追加する。Edit old_string (trait 直前の `/// Abstraction for a kana-to-kanji conversion backend.`):

old_string:

```rust
/// Abstraction for a kana-to-kanji conversion backend.
```

new_string:

```rust
/// Backend construction parameters.
///
/// Spec: `docs/superpowers/specs/2026-04-24-kotoha-phase-1-design.md` §5.4.
///
/// Marked `#[non_exhaustive]` per ADR 0006 so Phase 2+ can add new backend
/// kinds (system dictionary / remote HTTP / learning-cache hybrid) without
/// breaking external match sites.
#[non_exhaustive]
#[derive(Debug, Clone)]
pub enum BackendConfig {
    /// Deterministic mock backend (for tests). Constructible only when the
    /// `mock-backend` feature flag is enabled at build time.
    Mock,

    /// Zenz GGUF model backend (via llama-cpp-2). Constructible only when
    /// the `zenz` feature flag is enabled at build time.
    Zenz {
        /// Absolute path to the GGUF file on disk.
        model_path: PathBuf,
    },
}

/// Abstraction for a kana-to-kanji conversion backend.
```

- [ ] **Step 3: BackendConfig の tests を既存の `#[cfg(test)] mod tests` に追加**

Edit `crates/kotoha-core/src/kanji/backend.rs`: 既存 `mod tests` の末尾 (最後の `}` の直前) に以下を insert する。

Edit old_string:

```rust
    #[test]
    fn score_sort_dedupe_stable_for_equal_scores() {
        // Two candidates with identical score must retain input order.
        let input = vec![
            Candidate::new("first", 0.5),
            Candidate::new("second", 0.5),
        ];
        let out = score_sort_dedupe(input, 5);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].surface, "first");
        assert_eq!(out[1].surface, "second");
    }
}
```

new_string:

```rust
    #[test]
    fn score_sort_dedupe_stable_for_equal_scores() {
        // Two candidates with identical score must retain input order.
        let input = vec![
            Candidate::new("first", 0.5),
            Candidate::new("second", 0.5),
        ];
        let out = score_sort_dedupe(input, 5);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].surface, "first");
        assert_eq!(out[1].surface, "second");
    }

    // ======================================================================
    // BackendConfig
    // ======================================================================

    #[test]
    fn backend_config_is_clone() {
        let mock = BackendConfig::Mock;
        let _ = mock.clone();
        let zenz = BackendConfig::Zenz {
            model_path: PathBuf::from("/tmp/zenz.gguf"),
        };
        let _ = zenz.clone();
    }

    #[test]
    fn backend_config_debug_contains_variant_name() {
        let mock = BackendConfig::Mock;
        let msg = format!("{mock:?}");
        assert!(msg.contains("Mock"), "Debug for Mock must contain \"Mock\": {msg}");

        let zenz = BackendConfig::Zenz {
            model_path: PathBuf::from("/tmp/zenz.gguf"),
        };
        let msg = format!("{zenz:?}");
        assert!(msg.contains("Zenz"), "Debug for Zenz must contain \"Zenz\": {msg}");
    }
}
```

- [ ] **Step 4: テスト実行**

Run:

```bash
cargo test -p kotoha-core --lib kanji::backend::tests
```

Expected: 19 tests passed (17 既存 + 2 新規)。

- [ ] **Step 5: clippy + fmt**

Run:

```bash
cargo clippy -p kotoha-core --all-targets --all-features -- -D warnings
cargo fmt --all --check
```

Expected: warnings ゼロ、fmt diff ゼロ。

- [ ] **Step 6: commit**

Run:

```bash
git add crates/kotoha-core/src/kanji/mod.rs crates/kotoha-core/src/kanji/backend.rs
git commit -m "feat(kanji): add BackendConfig enum (Mock / Zenz variants)"
```

Expected: `2 files changed`。

---

## Task P1-1-6: mock.rs TDD (MockBackend)

**Files:**
- Create: `crates/kotoha-core/src/kanji/mock.rs`
- Modify: `crates/kotoha-core/src/kanji/mod.rs` (add `#[cfg(feature = "mock-backend")] mod mock; ...`)

spec §8.2 の MockBackend を実装する。hard-coded fixture 3 件 (spec §13 Q4 に従い hard-code 優先)。

- [ ] **Step 1: mod.rs に mock module 宣言を追加**

Edit `crates/kotoha-core/src/kanji/mod.rs`:

old_string:

```rust
pub use backend::{BackendConfig, KanjiBackend};
pub use candidate::{Candidate, ConvertOptions};
pub use error::KanjiError;
```

new_string:

```rust
pub use backend::{BackendConfig, KanjiBackend};
pub use candidate::{Candidate, ConvertOptions};
pub use error::KanjiError;

#[cfg(feature = "mock-backend")]
mod mock;
#[cfg(feature = "mock-backend")]
pub use mock::MockBackend;
```

- [ ] **Step 2: Write `crates/kotoha-core/src/kanji/mock.rs`**

Content:

```rust
//! Deterministic mock backend for tests.
//!
//! Enabled by the `mock-backend` Cargo feature so that downstream integration
//! tests (which live in a separate crate via `tests/`) can import it. A plain
//! `#[cfg(test)]` would not expose the type across crates.
//!
//! # Fixture decision (spec §13 Q4 follow-up)
//!
//! The fixture is hard-coded (not externalized to TSV). Spec §13 Q4 accepts
//! this for small fixture counts. Three known inputs (`"にほんご"`,
//! `"かんじ"`, `"あした"`) produce two candidates each (six total data
//! points), which is sufficient to exercise every contract property
//! (score-descending sort, surface dedupe, top_k truncation, empty result,
//! unknown-input empty result) in Layer 2 integration tests.
//!
//! Any input not listed above returns an empty `Vec` rather than an error,
//! matching the trait contract (spec §5.3, §5.7 bullet 4 "empty allowed").

use crate::kanji::backend::{score_sort_dedupe, validate_input};
use crate::kanji::{Candidate, ConvertOptions, KanjiBackend, KanjiError};

/// Deterministic mock backend. Enabled by `feature = "mock-backend"`.
///
/// Construct with [`MockBackend::new`] or through
/// [`crate::kanji::load_backend`] applied to [`crate::kanji::BackendConfig::Mock`].
#[derive(Debug, Default)]
pub struct MockBackend {
    // Zero-sized placeholder; fields may be added in Phase 2+.
    _private: (),
}

impl MockBackend {
    /// Constructs a new [`MockBackend`].
    ///
    /// # Postconditions
    ///
    /// - The returned backend's `model_id()` equals `"mock"`.
    pub fn new() -> Self {
        Self { _private: () }
    }
}

impl KanjiBackend for MockBackend {
    fn model_id(&self) -> &str {
        "mock"
    }

    fn convert(
        &self,
        input: &str,
        options: &ConvertOptions,
    ) -> Result<Vec<Candidate>, KanjiError> {
        validate_input(input)?;
        let fixture: Vec<Candidate> = match input {
            "にほんご" => vec![
                Candidate::new("日本語", 0.9),
                Candidate::new("二本後", 0.3),
            ],
            "かんじ" => vec![
                Candidate::new("漢字", 0.85),
                Candidate::new("感じ", 0.45),
            ],
            "あした" => vec![
                Candidate::new("明日", 0.92),
                Candidate::new("足した", 0.25),
            ],
            _ => Vec::new(),
        };
        Ok(score_sort_dedupe(fixture, options.top_k))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mock_model_id_is_mock() {
        let b = MockBackend::new();
        assert_eq!(b.model_id(), "mock");
    }

    #[test]
    fn mock_known_input_returns_expected() {
        let b = MockBackend::new();
        let opts = ConvertOptions::default();
        let out = b.convert("にほんご", &opts).expect("convert must succeed");
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].surface, "日本語");
        assert_eq!(out[0].score, 0.9);
        assert_eq!(out[1].surface, "二本後");
    }

    #[test]
    fn mock_unknown_input_returns_empty() {
        let b = MockBackend::new();
        let opts = ConvertOptions::default();
        let out = b.convert("あいうえお", &opts).expect("convert must succeed");
        assert!(out.is_empty());
    }

    #[test]
    fn mock_respects_top_k_1() {
        let b = MockBackend::new();
        let opts = ConvertOptions {
            top_k: 1,
            temperature: 0.0,
            seed: Some(0),
        };
        let out = b.convert("にほんご", &opts).expect("convert must succeed");
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].surface, "日本語");
    }

    #[test]
    fn mock_invalid_input_returns_invalid_input_error() {
        let b = MockBackend::new();
        let opts = ConvertOptions::default();
        let err = b.convert("abc", &opts).expect_err("latin input must be rejected");
        assert!(matches!(err, KanjiError::InvalidInput { .. }));
    }
}
```

- [ ] **Step 3: テスト実行 (`mock-backend` feature 付き)**

Run:

```bash
cargo test -p kotoha-core --lib --features mock-backend kanji::mock::tests
```

Expected: 5 tests passed。

- [ ] **Step 4: feature flag なしでの check**

Run:

```bash
cargo check -p kotoha-core
```

Expected: PASS (mock.rs は `#[cfg(feature = "mock-backend")]` で gate されているため、default features では compile 対象外となる)。

- [ ] **Step 5: clippy + fmt**

Run:

```bash
cargo clippy -p kotoha-core --all-targets --all-features -- -D warnings
cargo fmt --all --check
```

Expected: warnings ゼロ、fmt diff ゼロ。

- [ ] **Step 6: commit**

Run:

```bash
git add crates/kotoha-core/src/kanji/mod.rs crates/kotoha-core/src/kanji/mock.rs
git commit -m "feat(kanji): add MockBackend under mock-backend feature"
```

Expected: `2 files changed`。

---

## Task P1-1-7: zenz.rs skeleton (ZenzBackend todo!() stub)

**Files:**
- Create: `crates/kotoha-core/src/kanji/zenz.rs`
- Modify: `crates/kotoha-core/src/kanji/mod.rs` (add `#[cfg(feature = "zenz")] mod zenz; ...`)

P1-1 では ZenzBackend は skeleton のみ。実装 (llama-cpp-2 wiring) は P1-2 で行う。

- [ ] **Step 1: mod.rs に zenz module 宣言を追加**

Edit `crates/kotoha-core/src/kanji/mod.rs`:

old_string:

```rust
#[cfg(feature = "mock-backend")]
mod mock;
#[cfg(feature = "mock-backend")]
pub use mock::MockBackend;
```

new_string:

```rust
#[cfg(feature = "mock-backend")]
mod mock;
#[cfg(feature = "mock-backend")]
pub use mock::MockBackend;

#[cfg(feature = "zenz")]
mod zenz;
#[cfg(feature = "zenz")]
pub use zenz::ZenzBackend;
```

- [ ] **Step 2: Write `crates/kotoha-core/src/kanji/zenz.rs`**

Content:

```rust
//! Zenz GGUF model backend (via llama-cpp-2).
//!
//! **P1-1 status: SKELETON ONLY.** The real `load` / `convert` implementation
//! ships in milestone P1-2, together with the `llama-cpp-2` dependency being
//! added to `Cargo.toml` and wired to the `zenz` feature (which is currently
//! an empty flag, not `["dep:llama-cpp-2"]`).
//!
//! Spec: `docs/superpowers/specs/2026-04-24-kotoha-phase-1-design.md` §3.1, §5.3.

use std::path::{Path, PathBuf};

use crate::kanji::{Candidate, ConvertOptions, KanjiBackend, KanjiError};

/// Zenz model backend. Enabled by `feature = "zenz"`.
///
/// **P1-1: all methods panic via `todo!()`. P1-2 supplies the real
/// implementation backed by llama-cpp-2.**
#[allow(dead_code)]
pub struct ZenzBackend {
    model_path: PathBuf,
    _placeholder: (),
}

impl ZenzBackend {
    /// Loads a Zenz GGUF model from `model_path`.
    ///
    /// # P1-1
    ///
    /// Panics with `todo!()` unconditionally.
    ///
    /// # P1-2 (planned)
    ///
    /// Opens the GGUF file via llama-cpp-2, validates the architecture, and
    /// caches the resulting context for subsequent `convert` calls.
    ///
    /// # Errors
    ///
    /// After P1-2 lands:
    ///
    /// - [`KanjiError::ModelNotFound`] if `model_path` does not exist.
    /// - [`KanjiError::ModelLoadFailed`] if llama-cpp-2 rejects the file.
    #[allow(unused_variables)]
    pub fn load(model_path: &Path) -> Result<Self, KanjiError> {
        todo!("P1-2: implement via llama-cpp-2")
    }
}

impl KanjiBackend for ZenzBackend {
    fn model_id(&self) -> &str {
        todo!("P1-2: implement via llama-cpp-2")
    }

    #[allow(unused_variables)]
    fn convert(
        &self,
        input: &str,
        options: &ConvertOptions,
    ) -> Result<Vec<Candidate>, KanjiError> {
        todo!("P1-2: implement via llama-cpp-2")
    }
}
```

- [ ] **Step 3: `zenz` feature 付き check**

Run:

```bash
cargo check -p kotoha-core --features zenz
```

Expected: PASS (todo!() は panic するが compile は通る)。

- [ ] **Step 4: clippy + fmt**

Run:

```bash
cargo clippy -p kotoha-core --all-targets --all-features -- -D warnings
cargo fmt --all --check
```

Expected: warnings ゼロ、fmt diff ゼロ。

> 注: ZenzBackend は P1-1 では実装されないので、convert / model_id のユニットテストは追加しない (呼べば必ず panic するため)。テストは P1-2 で追加する。

- [ ] **Step 5: commit**

Run:

```bash
git add crates/kotoha-core/src/kanji/mod.rs crates/kotoha-core/src/kanji/zenz.rs
git commit -m "feat(kanji): add ZenzBackend skeleton (stub for P1-2) under zenz feature"
```

Expected: `2 files changed`。

---

## Task P1-1-8: backend.rs Phase C — load_backend factory TDD

**Files:**
- Modify: `crates/kotoha-core/src/kanji/backend.rs` (append load_backend fn + tests)
- Modify: `crates/kotoha-core/src/kanji/mod.rs` (append `pub use backend::load_backend;`)

spec §5.4 の load_backend factory を実装する。feature 未有効時は `KanjiError::FeatureDisabled` を返す。

- [ ] **Step 1: mod.rs の re-export を更新**

Edit `crates/kotoha-core/src/kanji/mod.rs`:

old_string:

```rust
pub use backend::{BackendConfig, KanjiBackend};
```

new_string:

```rust
pub use backend::{load_backend, BackendConfig, KanjiBackend};
```

- [ ] **Step 2: backend.rs に load_backend を追加**

Edit `crates/kotoha-core/src/kanji/backend.rs`: 既存 `pub(crate) fn score_sort_dedupe(...)` の `}` 直後 (= `#[cfg(test)] mod tests` の直前) に load_backend を insert する。

Edit old_string:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    // ======================================================================
    // validate_input
    // ======================================================================
```

new_string:

```rust
/// Constructs the backend described by `config`.
///
/// Spec: `docs/superpowers/specs/2026-04-24-kotoha-phase-1-design.md` §5.4.
///
/// # Errors
///
/// - [`KanjiError::FeatureDisabled`] if `config` names a backend whose Cargo
///   feature was not enabled at build time.
/// - Errors bubbled up from the backend's loader
///   (for example [`KanjiError::ModelNotFound`] from `ZenzBackend::load`
///   once P1-2 lands).
#[allow(unused_variables)]
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

#[cfg(test)]
mod tests {
    use super::*;

    // ======================================================================
    // validate_input
    // ======================================================================
```

- [ ] **Step 3: backend.rs の tests に load_backend テストを追加**

Edit `crates/kotoha-core/src/kanji/backend.rs`: 既存 `mod tests` 末尾 (最後の `}` の直前) に以下を insert。

Edit old_string:

```rust
    #[test]
    fn backend_config_debug_contains_variant_name() {
        let mock = BackendConfig::Mock;
        let msg = format!("{mock:?}");
        assert!(msg.contains("Mock"), "Debug for Mock must contain \"Mock\": {msg}");

        let zenz = BackendConfig::Zenz {
            model_path: PathBuf::from("/tmp/zenz.gguf"),
        };
        let msg = format!("{zenz:?}");
        assert!(msg.contains("Zenz"), "Debug for Zenz must contain \"Zenz\": {msg}");
    }
}
```

new_string:

```rust
    #[test]
    fn backend_config_debug_contains_variant_name() {
        let mock = BackendConfig::Mock;
        let msg = format!("{mock:?}");
        assert!(msg.contains("Mock"), "Debug for Mock must contain \"Mock\": {msg}");

        let zenz = BackendConfig::Zenz {
            model_path: PathBuf::from("/tmp/zenz.gguf"),
        };
        let msg = format!("{zenz:?}");
        assert!(msg.contains("Zenz"), "Debug for Zenz must contain \"Zenz\": {msg}");
    }

    // ======================================================================
    // load_backend
    // ======================================================================

    #[cfg(not(feature = "mock-backend"))]
    #[test]
    fn mock_config_without_feature_errors_feature_disabled() {
        let err = load_backend(&BackendConfig::Mock)
            .expect_err("Mock must error when mock-backend feature is off");
        match err {
            KanjiError::FeatureDisabled { feature } => {
                assert_eq!(feature, "mock-backend");
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[cfg(not(feature = "zenz"))]
    #[test]
    fn zenz_config_without_feature_errors_feature_disabled() {
        let err = load_backend(&BackendConfig::Zenz {
            model_path: PathBuf::from("/tmp/zenz.gguf"),
        })
        .expect_err("Zenz must error when zenz feature is off");
        match err {
            KanjiError::FeatureDisabled { feature } => {
                assert_eq!(feature, "zenz");
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[cfg(feature = "mock-backend")]
    #[test]
    fn mock_config_with_feature_returns_box() {
        let backend = load_backend(&BackendConfig::Mock)
            .expect("Mock must construct when mock-backend feature is on");
        assert_eq!(backend.model_id(), "mock");
    }
}
```

> `feature = "zenz"` 付きの成功テストは、ZenzBackend::load が todo!() で panic するため P1-1 では追加しない。P1-2 で追加する。

- [ ] **Step 4: テストを走らせて確認 (3 feature 構成で)**

Run:

```bash
cargo test -p kotoha-core --lib kanji::backend::tests
cargo test -p kotoha-core --lib --features mock-backend kanji::backend::tests
cargo test -p kotoha-core --lib --features zenz kanji::backend::tests
```

Expected:
- default: `zenz_config_without_feature_errors_feature_disabled` + `mock_config_without_feature_errors_feature_disabled` を含めて PASS (合計 21 tests)
- mock-backend: `mock_config_with_feature_returns_box` + `zenz_config_without_feature_errors_feature_disabled` 含めて PASS (合計 21 tests)
- zenz: `mock_config_without_feature_errors_feature_disabled` 含めて PASS (合計 20 tests)

件数は `#[cfg]` gate による差分で軽く前後する。いずれも全テストが PASS していればよい。

- [ ] **Step 5: clippy + fmt**

Run:

```bash
cargo clippy -p kotoha-core --all-targets --all-features -- -D warnings
cargo fmt --all --check
```

Expected: warnings ゼロ、fmt diff ゼロ。

- [ ] **Step 6: commit**

Run:

```bash
git add crates/kotoha-core/src/kanji/mod.rs crates/kotoha-core/src/kanji/backend.rs
git commit -m "feat(kanji): add load_backend factory with feature-gated dispatch"
```

Expected: `2 files changed`。

---

## Task P1-1-9: kanji/mod.rs + lib.rs re-export

**Files:**
- Modify: `crates/kotoha-core/src/kanji/mod.rs` (最終形に整える)
- Modify: `crates/kotoha-core/src/lib.rs` (公開 API re-export を拡充)

P1-1-2〜P1-1-8 で順次成長した kanji/mod.rs と lib.rs を最終形に整える。本 Task は冪等な整形 commit。

- [ ] **Step 1: kanji/mod.rs を最終形に整える**

Write (overwrite) `crates/kotoha-core/src/kanji/mod.rs`:

Content:

```rust
//! Kana-to-kanji conversion subsystem.
//!
//! Phase 1 scope: [`KanjiBackend`] trait, [`Candidate`] / [`ConvertOptions`] /
//! [`BackendConfig`] value types, [`KanjiError`] enum, [`load_backend`]
//! factory, and [`MockBackend`] test helper. The real `ZenzBackend`
//! implementation lands in Phase 1 milestone P1-2.
//!
//! Spec: `docs/superpowers/specs/2026-04-24-kotoha-phase-1-design.md` §4-§8.

mod backend;
mod candidate;
mod error;

pub use backend::{load_backend, BackendConfig, KanjiBackend};
pub use candidate::{Candidate, ConvertOptions};
pub use error::KanjiError;

#[cfg(feature = "mock-backend")]
mod mock;
#[cfg(feature = "mock-backend")]
pub use mock::MockBackend;

#[cfg(feature = "zenz")]
mod zenz;
#[cfg(feature = "zenz")]
pub use zenz::ZenzBackend;
```

- [ ] **Step 2: lib.rs の re-export ブロックを拡充**

Edit `crates/kotoha-core/src/lib.rs`:

old_string:

```rust
pub use error::{Error, Result};
pub use input::{InputContext, InputMode, InputStep};
pub use kanji::{Candidate, ConvertOptions};
pub use romaji::{ConvertStep, RomajiConverter};
```

new_string:

```rust
pub use error::{Error, Result};
pub use input::{InputContext, InputMode, InputStep};
pub use kanji::{load_backend, BackendConfig, Candidate, ConvertOptions, KanjiBackend, KanjiError};
pub use romaji::{ConvertStep, RomajiConverter};
```

`MockBackend` と `ZenzBackend` はそれぞれ feature-gated なので crate root では re-export せず、`kotoha_core::kanji::MockBackend` / `kotoha_core::kanji::ZenzBackend` としてのみ公開する。

- [ ] **Step 3: 全 feature 構成で check**

Run:

```bash
cargo check -p kotoha-core --no-default-features
cargo check -p kotoha-core
cargo check -p kotoha-core --features mock-backend
cargo check -p kotoha-core --features zenz
cargo check -p kotoha-core --features zenz-smoke
cargo check -p kotoha-core --all-features
```

Expected: 6 コマンド全てが PASS。

- [ ] **Step 4: clippy + fmt**

Run:

```bash
cargo clippy -p kotoha-core --all-targets --all-features -- -D warnings
cargo fmt --all --check
```

Expected: warnings ゼロ、fmt diff ゼロ。

- [ ] **Step 5: commit**

Run:

```bash
git add crates/kotoha-core/src/kanji/mod.rs crates/kotoha-core/src/lib.rs
git commit -m "feat(kanji): finalize mod.rs exports and crate-level re-exports"
```

Expected: `2 files changed`。

---

## Task P1-1-10: tests/kanji_mock.rs — Layer 2 integration tests (5 件)

**Files:**
- Create: `crates/kotoha-core/tests/kanji_mock.rs`

spec §8.2 の Layer 2 integration tests 5 件を実装する。file-level `#![cfg(feature = "mock-backend")]` で gate する。

5 件の目的は trait contract (spec §5.3) + output guarantees (spec §5.7) を KanjiBackend trait objects 経由で検証すること。

- [ ] **Step 1: Write `crates/kotoha-core/tests/kanji_mock.rs`**

Content:

```rust
//! Layer 2 integration tests for the kanji subsystem.
//!
//! Enabled by the `mock-backend` feature. Covers the five properties that
//! spec §5.3 (trait contract) and §5.7 (output guarantees) require every
//! `KanjiBackend` implementation to honor, verified through a `Box<dyn
//! KanjiBackend>` obtained from `load_backend(&BackendConfig::Mock)` rather
//! than a direct `MockBackend::new()` — this guarantees the factory path and
//! the trait object path are exercised end-to-end.

#![cfg(feature = "mock-backend")]

use kotoha_core::kanji::{load_backend, BackendConfig, ConvertOptions};

#[test]
fn mock_backend_model_id_exposed_via_trait() {
    let backend = load_backend(&BackendConfig::Mock)
        .expect("Mock must construct when mock-backend feature is on");
    assert_eq!(backend.model_id(), "mock");
}

#[test]
fn mock_backend_returns_score_descending() {
    let backend = load_backend(&BackendConfig::Mock)
        .expect("Mock must construct when mock-backend feature is on");
    let opts = ConvertOptions {
        top_k: 5,
        temperature: 0.0,
        seed: Some(0),
    };
    let out = backend
        .convert("にほんご", &opts)
        .expect("convert must succeed on known hiragana input");
    assert!(out.len() >= 2, "fixture should produce at least 2 candidates");
    for i in 0..out.len() - 1 {
        assert!(
            out[i].score >= out[i + 1].score,
            "score at index {} ({}) must be >= score at index {} ({})",
            i,
            out[i].score,
            i + 1,
            out[i + 1].score
        );
    }
}

#[test]
fn mock_backend_respects_top_k() {
    let backend = load_backend(&BackendConfig::Mock)
        .expect("Mock must construct when mock-backend feature is on");
    let opts = ConvertOptions {
        top_k: 1,
        temperature: 0.0,
        seed: Some(0),
    };
    let out = backend
        .convert("にほんご", &opts)
        .expect("convert must succeed");
    assert_eq!(out.len(), 1, "top_k=1 must truncate output to a single candidate");
}

#[test]
fn mock_backend_dedupes_identical_surface() {
    let backend = load_backend(&BackendConfig::Mock)
        .expect("Mock must construct when mock-backend feature is on");
    let opts = ConvertOptions::default();
    let out = backend
        .convert("にほんご", &opts)
        .expect("convert must succeed");
    let mut seen: Vec<String> = Vec::new();
    for c in &out {
        assert!(
            !seen.iter().any(|s| s == &c.surface),
            "surface {:?} must not be duplicated in the output",
            c.surface
        );
        seen.push(c.surface.clone());
    }
}

#[test]
fn mock_backend_top_k_zero_returns_empty() {
    let backend = load_backend(&BackendConfig::Mock)
        .expect("Mock must construct when mock-backend feature is on");
    let opts = ConvertOptions {
        top_k: 0,
        temperature: 0.0,
        seed: Some(0),
    };
    let out = backend
        .convert("にほんご", &opts)
        .expect("top_k=0 must succeed with an empty Vec, not an error");
    assert!(
        out.is_empty(),
        "top_k=0 must yield an empty Vec; got {} candidates",
        out.len()
    );
}
```

- [ ] **Step 2: integration test を走らせる**

Run:

```bash
cargo test -p kotoha-core --features mock-backend --test kanji_mock
```

Expected: 5 tests passed。

- [ ] **Step 3: default features では integration test が compile skip されることを確認**

Run:

```bash
cargo test -p kotoha-core --test kanji_mock 2>&1 | tail -10
```

Expected: file-level `#![cfg(feature = "mock-backend")]` により、default features では 0 test が登録される (compile 自体は通り `running 0 tests` が出力される) か、または crate 全体として compile skip される。どちらであっても test suite は PASS で終わる。

- [ ] **Step 4: clippy + fmt**

Run:

```bash
cargo clippy -p kotoha-core --all-targets --all-features -- -D warnings
cargo fmt --all --check
```

Expected: warnings ゼロ、fmt diff ゼロ。

- [ ] **Step 5: commit**

Run:

```bash
git add crates/kotoha-core/tests/kanji_mock.rs
git commit -m "test(kanji): add Layer 2 integration tests for MockBackend contract"
```

Expected: `1 file changed`。

---

## Task P1-1-11: Multi-feature cargo check + lefthook pre-push

**Files:** (検証のみ、commit なし)

- [ ] **Step 1: 6 通りの feature 組み合わせで cargo check**

Run sequentially (まとめて実行して全体が PASS することを確認):

```bash
cargo check -p kotoha-core --no-default-features
cargo check -p kotoha-core
cargo check -p kotoha-core --features mock-backend
cargo check -p kotoha-core --features zenz
cargo check -p kotoha-core --features zenz-smoke
cargo check -p kotoha-core --all-features
```

Expected: 6 コマンド全てが `Finished dev profile` で終わる。サブエージェントは各コマンドの stdout head (先頭 3 行) + tail (末尾 3 行) を report に verbatim で含める。

- [ ] **Step 2: 全 clippy**

Run:

```bash
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

Expected: warnings ゼロ。verbatim head + tail を report に含める。

- [ ] **Step 3: 全 fmt check**

Run:

```bash
cargo fmt --all --check
```

Expected: diff ゼロ。

- [ ] **Step 4: default features + mock-backend feature で test 全件**

Run:

```bash
cargo test --workspace
cargo test --workspace --features mock-backend
```

Expected:
- `cargo test --workspace`: P1-0 baseline + Layer 1 新規 ≥ 20 件 PASS。Task P1-1-0 Step 2 で記録した baseline N と比較し、N + 20 件以上になっていることを確認する。
- `cargo test --workspace --features mock-backend`: Layer 1 + Layer 2 (5 件) 合計 PASS。

verbatim head + tail を report に含める。

- [ ] **Step 5: lefthook pre-push trigger**

Run:

```bash
lefthook run pre-push
```

または `git push --dry-run` 相当。Expected: manifest-check / build / clippy / test 全て PASS。

もし pre-push 側が workspace-wide の `cargo test` を走らせる設定なら、それが default features で走る前提なので問題ない。もし `--all-features` を走らせる設定なら all-features でも PASS することが Step 1 で確認済みのはず。

lefthook pre-push の出力 head + tail を report に含める。

- [ ] **Step 6: 検証のみ (commit 不要)**

Run:

```bash
git status
```

Expected: `nothing to commit, working tree clean`。

---

## Task P1-1-12: commit + push + PR 作成

**Files:** (これまでの commit を push + PR 作成)

- [ ] **Step 1: commit history を確認**

Run:

```bash
git log --oneline develop..HEAD
```

Expected: 9 commits (Task P1-1-1 〜 P1-1-10 のうち commit を行った Task 数 = 9)。

- [ ] **Step 2: branch を push**

Run:

```bash
git push -u origin feature/<IMPL_ISSUE>-kanji-skeleton-mockbackend
```

Expected: branch が remote に push され、lefthook pre-push が自動実行 + PASS。

- [ ] **Step 3: PR 作成**

Run:

```bash
gh pr create --base develop \
  --title "feat(kanji): add kanji module skeleton + MockBackend (#<IMPL_ISSUE>)" \
  --body "$(cat <<'EOF'
## Summary

Phase 1 milestone P1-1: add the kotoha-core::kanji module skeleton with public API types (KanjiBackend trait, Candidate, ConvertOptions, BackendConfig, KanjiError, load_backend factory) and a deterministic MockBackend. ZenzBackend is introduced as a todo!() skeleton to be implemented in P1-2.

Closes #<IMPL_ISSUE>.

## Changes

- New module `crates/kotoha-core/src/kanji/` (6 files):
  - `mod.rs` — public re-exports (feature-gated MockBackend / ZenzBackend)
  - `candidate.rs` — Candidate + ConvertOptions value types (`#[non_exhaustive]`, Default)
  - `error.rs` — KanjiError enum (`#[non_exhaustive]`, 5 variants via thiserror)
  - `backend.rs` — KanjiBackend trait, `pub(crate)` helpers `validate_input` + `score_sort_dedupe`, BackendConfig enum, `load_backend` factory
  - `mock.rs` — MockBackend (feature = "mock-backend"), deterministic fixture for 3 known inputs
  - `zenz.rs` — ZenzBackend skeleton (feature = "zenz"), todo!() stubs for P1-2
- New feature flags in `crates/kotoha-core/Cargo.toml`: `default`, `mock-backend`, `zenz` (empty in P1-1), `zenz-smoke`
- Updated `crates/kotoha-core/src/lib.rs` to expose `pub mod kanji;` + re-exports
- New integration test `crates/kotoha-core/tests/kanji_mock.rs` (5 Layer 2 tests, file-level `#![cfg(feature = "mock-backend")]`)

## Verification

- `cargo check -p kotoha-core` passes under all 6 feature configurations (no-default-features / default / mock-backend / zenz / zenz-smoke / all-features).
- `cargo test --workspace` passes (Layer 1 unit: 20+ new tests).
- `cargo test --workspace --features mock-backend` passes (Layer 1 + Layer 2: 25+ tests).
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` reports zero warnings.
- `cargo fmt --all --check` produces no diff.
- lefthook pre-push passes locally.

## Context

- Spec: `docs/superpowers/specs/2026-04-24-kotoha-phase-1-design.md` §4.1 / §4.3 / §5 / §8.1 / §8.2
- Phase 1 overall plan: `docs/superpowers/plans/2026-04-24-kotoha-phase-1-implementation.md` §"PR #2 — P1-1"
- Detailed plan: `docs/superpowers/plans/2026-04-24-kotoha-phase-1-p1-1.md`
- ADR 0006 (`#[non_exhaustive]` policy): enforced on Candidate / ConvertOptions / BackendConfig / KanjiError.
- ADR 0008 (canonical romaji Phase 1 decision): orthogonal to this PR but referenced from spec §5.6.

## Test plan

- [ ] `cargo test --workspace` passes on CI-equivalent local run
- [ ] `cargo test --workspace --features mock-backend` passes
- [ ] `cargo clippy --workspace --all-targets --all-features -- -D warnings` clean
- [ ] `cargo check` clean under all 6 feature configurations listed above
- [ ] MockBackend fixture produces deterministic output for "にほんご" / "かんじ" / "あした" and empty Vec for unknown inputs
- [ ] load_backend returns `KanjiError::FeatureDisabled` when the matching feature is off

🤖 Generated with [Claude Code](https://claude.com/claude-code)
EOF
)"
```

Expected: PR が作成され、URL が表示される。`<IMPL_ISSUE>` は Task P1-1-0 で取得した値に置換する。

---

## Task P1-1-13: review + findings + merge

**Files:** (変更なし、review + merge のみ)

- [ ] **Step 1: Small-tier review を走らせる**

CLAUDE.md の PR Review Matrix に従い、本 PR は Small tier (9 files、約 500 LOC) の上限近くだが、Medium tier の目安 (≤10 files AND ≤300 lines) の 300 lines を超える可能性が高いため、Medium tier に寄せる。ただし本 PR は trait + 値型 + skeleton 中心で logical complexity は限定的なので、運用判断として Small tier 基準の `agent-teams:team-review` (security + architecture + testing) + `secrets-check` を最低限実施し、必要に応じて Medium tier まで拡張する。

Run (skill tool 経由):

```
/agent-teams:team-review dimensions=security,architecture,testing
/secrets-check
```

Expected: 合格 (Critical / High ゼロが理想)。

- [ ] **Step 2: review findings を解消**

Critical / High findings があれば commit 追加で解消する。解消後に再 review し、ゼロになるまで繰り返す。

- [ ] **Step 3: squash merge + branch 削除**

Run (`<PR_NUMBER>` は Task P1-1-12 Step 3 で取得した PR 番号):

```bash
gh pr merge <PR_NUMBER> --squash --delete-branch
gh pr view <PR_NUMBER> --json state,mergeCommit -q '{state, merge: .mergeCommit.oid}'
```

Expected: `state: MERGED`、merge commit SHA が返る。SHA を `<MERGE_COMMIT>` として記録する。

- [ ] **Step 4: develop に切り替え + pull**

Run:

```bash
git checkout develop
git pull
git log --oneline -3
```

Expected: 先頭に P1-1 squash-merge commit が来ている。

---

## Task P1-1-14: WBS ログ作成 + develop 直接 push

**Files:**
- Create: `docs/wbs/2026-04-24-feature-<IMPL_ISSUE>-kanji-skeleton-mockbackend.md`

project CLAUDE.md「WBS 直接 push の例外」に従い、本 file は develop branch に直接 commit + push する。

- [ ] **Step 1: WBS ログを Write で作成**

Write `docs/wbs/2026-04-24-feature-<IMPL_ISSUE>-kanji-skeleton-mockbackend.md`:

Content (`<IMPL_ISSUE>` / `<PR_NUMBER>` / `<MERGE_COMMIT>` を実値に置換する):

```markdown
---
milestone: P1-1
branch: feature/<IMPL_ISSUE>-kanji-skeleton-mockbackend
pr: "#<PR_NUMBER>"
merge_commit: "<MERGE_COMMIT>"
issue: "#<IMPL_ISSUE>"
status: done
started: 2026-04-24
finished: 2026-04-24
---

# P1-1: kanji skeleton + MockBackend

## 実施内容

- `crates/kotoha-core/Cargo.toml` に feature flag 4 種 (default / mock-backend / zenz / zenz-smoke) を追加。`zenz` は P1-1 では空 flag (llama-cpp-2 依存追加は P1-2)。
- `crates/kotoha-core/src/kanji/` module を新設:
  - `candidate.rs`: Candidate + ConvertOptions (spec §5.1 / §5.2)
  - `error.rs`: KanjiError (spec §5.5、5 variants、thiserror 由来)
  - `backend.rs`: KanjiBackend trait + pub(crate) validate_input + pub(crate) score_sort_dedupe + BackendConfig + load_backend (spec §5.3 / §5.4 / §5.6 / §5.7)
  - `mock.rs`: MockBackend (feature = mock-backend)、3 known inputs のハードコード fixture
  - `zenz.rs`: ZenzBackend skeleton (feature = zenz)、todo!() stubs
  - `mod.rs`: re-export
- `crates/kotoha-core/src/lib.rs` に `pub mod kanji;` + `pub use kanji::{...};` を追加
- `crates/kotoha-core/tests/kanji_mock.rs` を新設 (Layer 2 integration、5 件、file-level feature gate)
- Layer 1 unit tests 新規 22 件: candidate 5 + error 5 + backend.validate_input 10 + backend.score_sort_dedupe 7 + backend.BackendConfig 2 + backend.load_backend 1〜2 (feature による) + mock 5 = 合計 34 〜 36 件 (feature combinaiton 依存)
- Layer 2 integration tests 5 件 (model_id / score 降順 / top_k / dedupe / top_k=0 空)

## つまずき

(実施時に記入。例: llama-cpp-2 依存を P1-1 では追加せずに zenz feature を空 flag にするか迷った等、実際に発生した意思決定 / 修正点を記録する)

## Regression 検証

- `cargo test --workspace` (default features): Phase 0 既存テスト + kanji 新規 Layer 1 すべて PASS。既存テスト件数の増減は kanji の追加分のみ。
- `cargo test --workspace --features mock-backend`: Layer 1 + Layer 2 = 合計 PASS。
- `cargo check -p kotoha-core` を 6 通りの feature 組合せ (no-default / default / mock-backend / zenz / zenz-smoke / all-features) で走らせ、すべて PASS を確認。
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` warnings ゼロ。
- `cargo fmt --all --check` diff ゼロ。

## P1-2 への申し送り

- `crates/kotoha-core/Cargo.toml` の `zenz = []` を `zenz = ["dep:llama-cpp-2"]` に書き換えること。同時に `[dependencies]` (または `[target."cfg(...)".dependencies]`) に `llama-cpp-2 = { version = "X.Y", optional = true }` を追加する。正確な version は P1-2 開始時点で Context7 / crates.io 最新 stable を確認して pin する (Open Question Q1)。
- `crates/kotoha-core/src/kanji/zenz.rs` の `load` / `model_id` / `convert` の `todo!()` を実装に置換する。実装前に spec §3.3 の AzooKey Zenzai docs (<https://github.com/azooKey/AzooKeyKanaKanjiConverter/blob/main/Docs/zenzai.md>) を通読し、prompt format 解析ログを本節相当の P1-2 WBS に残すこと (spec §10 Risk #2 の 1 次緩和策)。
- P1-1 で `validate_input` / `score_sort_dedupe` を `pub(crate)` helper として backend.rs に配置済み。ZenzBackend::convert 実装時にはこれら helper をそのまま利用すること (契約の一元化)。
- Layer 3 (`zenz-smoke` feature、tests/kanji_zenz_smoke.rs) は P1-2 の scope。
- Open Question Q2 (prompt template 正確形) / Q5 (`--seed 0` deterministic 挙動) は P1-2 開始時に解消する。

## 成果物リンク

- PR: #<PR_NUMBER>
- ISSUE: #<IMPL_ISSUE>
- Plan PR (本 plan 本体): #63
- Spec: `docs/superpowers/specs/2026-04-24-kotoha-phase-1-design.md`
- Overall plan: `docs/superpowers/plans/2026-04-24-kotoha-phase-1-implementation.md`
- Detailed plan: `docs/superpowers/plans/2026-04-24-kotoha-phase-1-p1-1.md`
```

- [ ] **Step 2: develop に直接 commit + push**

Run:

```bash
git add docs/wbs/2026-04-24-feature-<IMPL_ISSUE>-kanji-skeleton-mockbackend.md
git commit -m "docs(wbs): P1-1 kanji skeleton + MockBackend — implementation log (#<IMPL_ISSUE>, PR #<PR_NUMBER>)"
git push
```

Expected: lefthook pre-push が doc-naming check を通過、他の hook は docs-only change なので skip または PASS。

- [ ] **Step 3: 実装 ISSUE を close**

`gh pr merge --squash` が "Closes #<IMPL_ISSUE>" を解釈して自動的に ISSUE を close しているはずだが、念のため確認する。

Run:

```bash
gh issue view <IMPL_ISSUE> --json state
```

Expected: `"state":"CLOSED"`。もし OPEN のままなら手動で close する:

```bash
gh issue close <IMPL_ISSUE>
```

---

## P1-1 完了条件チェックリスト

以下すべてを実施完了時点で P1-1 完了。

- [ ] 実装 ISSUE `#<IMPL_ISSUE>` が close 済み
- [ ] PR `#<PR_NUMBER>` が develop に squash-merge 済み
- [ ] `feature/<IMPL_ISSUE>-kanji-skeleton-mockbackend` branch が remote + local ともに削除済み
- [ ] `cargo check -p kotoha-core` が 6 通り (no-default / default / mock-backend / zenz / zenz-smoke / all-features) すべてで PASS
- [ ] `cargo test --workspace` が PASS (Layer 1 新規 ≥ 20 件を含む)
- [ ] `cargo test --workspace --features mock-backend` が PASS (Layer 1 + Layer 2 5 件)
- [ ] `cargo clippy --workspace --all-targets --all-features -- -D warnings` warnings ゼロ
- [ ] `cargo fmt --all --check` diff ゼロ
- [ ] lefthook pre-push 全コマンド PASS (build / clippy / test 実行 + 成功)
- [ ] `crates/kotoha-core/src/kanji/` に 6 file が存在 (mod / candidate / error / backend / mock / zenz)
- [ ] `crates/kotoha-core/tests/kanji_mock.rs` が 5 test 含み `--features mock-backend` で PASS
- [ ] `kotoha_core::kanji::{Candidate, ConvertOptions, KanjiBackend, BackendConfig, KanjiError, load_backend}` が crate root + `kanji` module 両方から参照可能
- [ ] `kotoha_core::kanji::MockBackend` が `feature = "mock-backend"` 下で可視
- [ ] `kotoha_core::kanji::ZenzBackend` が `feature = "zenz"` 下で可視 (実体は todo!() skeleton)
- [ ] `docs/wbs/2026-04-24-feature-<IMPL_ISSUE>-kanji-skeleton-mockbackend.md` が develop に push 済み

---

## Spec Coverage 確認

本 plan が spec §4-§8 のどの要件をどの Task で実装するかのマッピング。

| Spec § | 要件 | 実装 Task |
|---|---|---|
| §4.1 Crate / module layout | `crates/kotoha-core/src/kanji/` 6 file 構成 | P1-1-2 〜 P1-1-9 |
| §4.3 Feature flag 構成 | default / mock-backend / zenz / zenz-smoke の 4 flag (llama-cpp-2 依存は P1-2 で追加のため P1-1 では `zenz = []`) | P1-1-1 |
| §5.1 Candidate | `#[non_exhaustive]` / Debug+Clone+PartialEq / new | P1-1-2 |
| §5.2 ConvertOptions | `#[non_exhaustive]` / Default = top_k 5 / temp 0 / seed Some(0) | P1-1-2 |
| §5.3 KanjiBackend trait | model_id / convert + rustdoc 契約 | P1-1-4 (Phase A) |
| §5.4 BackendConfig + load_backend | enum + factory、feature 未有効時は FeatureDisabled | P1-1-5 (Config), P1-1-8 (factory) |
| §5.5 KanjiError | 5 variants、`#[non_exhaustive]`、thiserror、英語メッセージ | P1-1-3 |
| §5.6 Input 制約 (hiragana + ≤128 chars) | `pub(crate) fn validate_input` | P1-1-4 (Phase A) |
| §5.7 Output 順序保証 + dedupe | `pub(crate) fn score_sort_dedupe` (sort / dedupe / truncate) | P1-1-4 (Phase A) |
| §8.1 Layer 1 Unit | ≥ 20 件新規 (candidate 5 + error 5 + backend.validate_input 10 + backend.score_sort_dedupe 7 + backend.BackendConfig 2 + backend.load_backend 1〜2 + mock 5) | P1-1-2 / P1-1-3 / P1-1-4 / P1-1-5 / P1-1-6 / P1-1-8 |
| §8.2 Layer 2 Integration | 5 件、`mock-backend` feature、trait object 経由 | P1-1-10 |

§8.3 (Layer 3 Zenz smoke) / §8.4 (Layer 4 E2E) は P1-2 / P1-3 scope で本 plan では扱わない。

---

## Self-Review 済み事項

本 plan の品質を担保するため、以下 8 項目を自己確認済み。

1. **プレースホルダ残留なし**: 本文内の `<IMPL_ISSUE>` / `<PR_NUMBER>` / `<MERGE_COMMIT>` は意図した placeholder (着手時に実値へ置換)。それ以外に "TODO" / "..." / "TBD" / "fill in" / "similar to Task N" のような未解決箇所は存在しない。各 Task の Rust コードと bash コマンドはすべて完全形で記述されている。
2. **型シグネチャ一貫性**: `Candidate::new(surface: impl Into<String>, score: f32)` のシグネチャは P1-1-2 の定義、P1-1-4 の score_sort_dedupe テスト、P1-1-6 の mock.rs 実装、P1-1-10 の integration test すべてで一致する。`ConvertOptions::default()` の `top_k: 5, temperature: 0.0, seed: Some(0)` も同様。
3. **feature flag 一貫性**: P1-1-1 で定義した 4 flag (default / mock-backend / zenz / zenz-smoke) は mod.rs (P1-1-6 / P1-1-7 / P1-1-9)、backend.rs load_backend (P1-1-8)、tests/kanji_mock.rs file-level (P1-1-10)、verification commands (P1-1-11) で一貫して使われる。`zenz = []` が P1-1 の空 flag である旨は Cargo.toml コメント + zenz.rs module doc + WBS 申し送り事項で 3 箇所重複明記。
4. **Spec カバレッジ完全性**: spec §4.1 / §4.3 / §5 全 7 sub-section / §8.1 / §8.2 の要件すべてを Task に割り当て済み (Spec Coverage 表参照)。§8.3 / §8.4 / §9 / §10 以降は P1-1 scope 外であることを明示済み。
5. **TDD 遵守**: backend.rs の validate_input / score_sort_dedupe と mock.rs の convert 実装については、テストで edge case を網羅的にカバー (validate_input 10 件で hiragana / choonpu / latin / kanji / katakana / space / digit / 128 境界 / 129 超過 / 空、score_sort_dedupe 7 件で empty / top_k=0 / 降順 sort / dedupe / truncate / single / stable)。
6. **`#[non_exhaustive]` 適用確認**: Candidate (P1-1-2) / ConvertOptions (P1-1-2) / BackendConfig (P1-1-5) / KanjiError (P1-1-3) の 4 型すべてに `#[non_exhaustive]` を付与済み。ADR 0006 準拠。
7. **Branch Scope Policy 準拠**: 本 PR は 7 new + 2 modified = 9 file、約 500 LOC。CLAUDE.md の目安 (10 files / 300 lines) のうち line 数がやや超過するため、PR Review Matrix の Small tier 上限近い Medium tier 寄りとして運用する (Task P1-1-13 Step 1 で reviewer 選択を明記済み)。
8. **CLAUDE.md 制約遵守**: 言語規則 (英語 = commit / PR / rustdoc / ISSUE、日本語 = plan / WBS) を全 Task の commit message と Write content で徹底。WBS 直接 push は P1-1-14 で CLAUDE.md 「WBS 直接 push の例外」節を明示的に引用。lefthook pre-push `--no-verify` 禁止も共通規約に明記済み。
