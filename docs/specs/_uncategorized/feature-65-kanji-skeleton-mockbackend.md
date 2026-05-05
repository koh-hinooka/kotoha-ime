---
feature: feature-65-kanji-skeleton-mockbackend
status: implemented
bounded_context: _uncategorized
related_issues: ["#65"]
related_prs: []
glossary_refs: []
last_reviewed: 2026-05-05
---

# P1-1: kanji skeleton + MockBackend

> **Migration note**: 本 spec は `docs/wbs/2026-04-24-feature-65-kanji-skeleton-mockbackend.md` から spec 化 (Global CLAUDE.md §Documentation Structure 準拠の `docs/wbs/` 廃止対応、2026-05-05)。
> 元 WBS は実装ログ性質。14-section restructure は別 follow-up 起票予定 (各 project の既存 14-section follow-up issue と合流可能)。

## 元 WBS 内容 (実装ログ由来)

milestone: P1-1
branch: feature/65-kanji-skeleton-mockbackend
pr: "#66"
merge_commit: "cb2a458"
issue: "#65"
status: done
started: 2026-04-24
finished: 2026-04-24
---

# P1-1: kanji skeleton + MockBackend

## 実施内容

- `crates/kotoha-core/Cargo.toml` に feature flag 4 種 (default / mock-backend / zenz / zenz-smoke) を追加した。`zenz` は P1-1 では空 flag とし、llama-cpp-2 依存追加は P1-2 に繰り延べた (Open Question Q1 準拠)。
- `crates/kotoha-core/src/kanji/` module を新設した:
  - `candidate.rs` (111 lines): `Candidate` (surface + score) と `ConvertOptions` (top_k / temperature / seed、Default top_k=5 / temperature=0.0 / seed=Some(0)) を spec §5.1 / §5.2 に従って実装。いずれも `#[non_exhaustive]`。
  - `error.rs` (113 lines): `KanjiError` enum を spec §5.5 に従い 5 variants (`InvalidInput` / `ModelNotFound` / `ModelLoadFailed` / `Backend` / `FeatureDisabled`) で実装。`thiserror` 由来、`#[non_exhaustive]`、error message は英語。
  - `backend.rs` (409 lines): `KanjiBackend` trait + `pub(crate) fn validate_input` (spec §5.6: hiragana U+3040..=U+309F + U+30FC + max 128 chars) + `pub(crate) fn score_sort_dedupe` (spec §5.7: score 降順 + surface dedupe 最高 score 保持 + top_k truncate) + `BackendConfig` enum + `load_backend` factory を実装。feature gate 経由で `FeatureDisabled` を返す。
  - `mock.rs` (140 lines): `MockBackend` を `feature = "mock-backend"` gate 下で実装。4 known inputs (にほんご / かんじ / あした / にほん) のハードコード fixture (spec §13 Q4 準拠)。にほん は意図的に `日本` surface を重複させた 3 candidate 構成で、`score_sort_dedupe` の dedupe path を Layer 2 統合テストで end-to-end に発火させる。
  - `zenz.rs` (83 lines): `ZenzBackend` skeleton を `feature = "zenz"` gate 下で実装。`load()` は P1-1 では `KanjiError::Backend` を返す (review round 1 ARCH-2 / SEC-1 の修正により `todo!()` panic から切替)。`model_id` / `convert` は P1-2 実装予定のため `todo!()` を維持。
  - `mod.rs` (24 lines): sub-module 宣言 + `pub use` で `kanji::{load_backend, BackendConfig, KanjiBackend, Candidate, ConvertOptions, KanjiError, MockBackend (mock-backend feature), ZenzBackend (zenz feature)}` を公開。
- `crates/kotoha-core/src/lib.rs` を更新: `pub mod kanji;` + `pub use kanji::{load_backend, BackendConfig, Candidate, ConvertOptions, KanjiBackend, KanjiError};`。
- `crates/kotoha-core/tests/kanji_mock.rs` (114 lines) を新設: Layer 2 integration 5 件 (model_id / score 降順 / top_k 尊重 / surface dedupe / top_k=0 空)。`#![cfg(feature = "mock-backend")]` で file-level gate。`#[non_exhaustive]` の制約で struct-literal 構築できない (E0639) ため `ConvertOptions::default() + field mutation` idiom を採用。

## テスト件数 (P1-0 baseline 129 からの差分)

- `cargo test --workspace` (default features): 160 PASS (+31 = candidate 5 + error 5 + backend.validate_input 10 + backend.score_sort_dedupe 7 + backend.BackendConfig 2 + backend.load_backend 2 (cfg により活性化))
- `cargo test --workspace --features mock-backend`: 171 PASS (+11 = MockBackend unit 6 + Layer 2 integration 5)
- `cargo test --workspace --features zenz`: ZenzBackend skeleton 用の `zenz_load_returns_backend_error_in_p1_1_skeleton` (+1) が cfg 有効化で追加

## つまずき

1. **`.expect_err()` の Debug 制約**: Task P1-1-8 で plan が `load_backend(...).expect_err(...)` を使っていたが、`load_backend` は `Result<Box<dyn KanjiBackend>, KanjiError>` を返し、`Box<dyn KanjiBackend>` に Debug が無いため compile error。implementer が `match` に切り替えて解消した。plan の該当箇所は P1-4 bundle の修正対象。
2. **`#[non_exhaustive]` の cross-crate struct-literal 禁止 (E0639)**: Task P1-1-10 で plan が `ConvertOptions { top_k: ..., temperature: ..., seed: ... }` を integration test (別 crate から import) で使っていたが、`#[non_exhaustive]` 付き型は定義 crate 外からは struct-literal 構築できない。`ConvertOptions::default() + field mutation` idiom に切替 (forward-compatible pattern)。plan の該当箇所も P1-4 bundle の修正対象。
3. **Review round 1: ZenzBackend::load の todo!() panic 回避 (ARCH-2 / SEC-1)**: architecture + security reviewers が、`zenz` feature を有効化したまま `load_backend(&BackendConfig::Zenz {..})` を呼ぶと process panic になる点を指摘。`Err(KanjiError::Backend { reason: "... P1-2" })` を返すよう修正し、`zenz_load_returns_backend_error_in_p1_1_skeleton` unit test を追加して P1-1 期待挙動を固定した。P1-2 でこの test を新仕様に書き換える。
4. **`#[allow(dead_code)]` on pub(crate) helpers**: `validate_input` / `score_sort_dedupe` が P1-1-4 時点では unused (MockBackend は P1-1-6 でようやく呼ぶ) のため clippy dead_code 警告を回避する `#[allow(dead_code)]` を付与。P1-1-6 以降で実コールパスが生じても、`--no-default-features` build では引き続き unused のため attribute は維持する。

## Regression 検証

- `cargo test --workspace` (default features): 160 PASS (P1-0 129 baseline + 31 kanji 新規)、既存 kana / romaji / input / cli の test 件数は不変。
- `cargo test --workspace --features mock-backend`: 171 PASS (Layer 1 160 + MockBackend unit 6 + Layer 2 integration 5)。
- `cargo check -p kotoha-core` を 6 通りの feature 組合せ (`--no-default-features` / default / `--features mock-backend` / `--features zenz` / `--features zenz-smoke` / `--all-features`) で走らせ、すべて PASS を確認。
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` warnings ゼロ。
- `cargo fmt --all --check` diff ゼロ。
- lefthook pre-push (manifest-check / build / clippy / test) PASS。

## Review 結果

- `secrets-check`: CLEAN (AWS / GH / GL tokens / private keys / DB conn strings / .env 参照すべてゼロ)。
- `agent-teams:team-reviewer` (architecture): PASS with Medium 2 (ARCH-2 を round 1 で解消、ARCH-3 lib.rs 再 export スコープは spec §14 #2 解釈の問題で P1-4 に defer) + Low 5。
- `agent-teams:team-reviewer` (testing): PASS with Medium 2 (TEST-1 score 降順 test が事前ソート fixture を使用、TEST-2 stable_for_equal_scores test が 2 要素のみ、いずれも test 強度向上のため P1-4 bundle で対応) + Low 6。
- `agent-teams:team-reviewer` (security): PASS with Low 2 (SEC-1 は ARCH-2 と同一 root cause のため round 1 で解消、SEC-2 validate_input error message に glyph を含む点は defense-in-depth suggestion で defer)。

## P1-2 への申し送り

- `crates/kotoha-core/Cargo.toml` の `zenz = []` を `zenz = ["dep:llama-cpp-2"]` に書き換えること。同時に `[dependencies]` に `llama-cpp-2 = { version = "X.Y", optional = true }` を追加する。正確な version は P1-2 開始時点で Context7 / crates.io 最新 stable を確認して pin する (Open Question Q1)。
- `crates/kotoha-core/src/kanji/zenz.rs` の `load` を `Err(KanjiError::Backend {..})` 返却から本実装に置換する。同時に `model_id` / `convert` の `todo!()` を実装に置き換える。
- 実装前に spec §3.3 の AzooKey Zenzai docs (<https://github.com/azooKey/AzooKeyKanaKanjiConverter/blob/main/Docs/zenzai.md>) を通読し、prompt format 解析ログを P1-2 WBS に残すこと (spec §10 Risk #2 の 1 次緩和策)。
- `crates/kotoha-core/src/kanji/zenz.rs` の `#[allow(unused_variables)]` / `#[allow(dead_code)]` / `#[derive(Debug)]` を、llama-cpp-2 による実装完了時に最適化すること (`_placeholder: ()` field は P1-2 で real field と差し替え)。
- `validate_input` / `score_sort_dedupe` は backend.rs に `pub(crate)` helper として配置済み。ZenzBackend::convert 実装時にはこれら helper をそのまま利用して契約を一元化すること。
- Layer 3 (`zenz-smoke` feature、`crates/kotoha-core/tests/kanji_zenz_smoke.rs`) は P1-2 の scope。
- `zenz_load_returns_backend_error_in_p1_1_skeleton` 単体テストは P1-1 skeleton の振る舞いを固定するためのものなので、P1-2 で ZenzBackend::load を本実装に置換する際に削除または書き換えること。
- Open Question Q2 (prompt template exact form) / Q5 (`--seed 0` deterministic 挙動) は P1-2 開始時に解消する。

## P1-4 への申し送り (plan refinement bundle)

Review で surface した plan-level bug と Medium/Low findings を P1-4 docs バンドルで解消する:

- Plan Task P1-1-8 の `.expect_err()` 記述を `match` pattern に更新 (`Box<dyn KanjiBackend>` が Debug を持たないため)。
- Plan Task P1-1-10 の struct-literal 例を `ConvertOptions::default() + field mutation` idiom に更新 (`#[non_exhaustive]` cross-crate 制約)。
- ARCH-3 (lib.rs の `pub use kanji::{...}` を落とし、spec §14 #2 の「kotoha_core::kanji::* から re-export」のみに絞る案) の判断。Phase 0 の慣例 (error / input / romaji も crate root から re-export) との整合を踏まえた上で最終判断する。
- TEST-1 / TEST-2 (score 降順 test を にほん fixture で強化、stable_for_equal_scores test を 3+ 要素に拡張) の test 強度向上。
- SEC-2 (`validate_input` の error message に U+コードポイント表記併用) の defense-in-depth。
- ARCH-5 / ARCH-6 / ARCH-7 / TEST-3 など Low findings の任意消化。

## 成果物リンク

- PR: https://github.com/std-koh-hinooka/kotoha-ime/pull/66
- ISSUE: https://github.com/std-koh-hinooka/kotoha-ime/issues/65
- Merge commit: cb2a458
- Plan PR (本 plan 本体): https://github.com/std-koh-hinooka/kotoha-ime/pull/64 (#63)
- Spec: `docs/superpowers/specs/2026-04-24-kotoha-phase-1-design.md`
- Overall plan: `docs/superpowers/plans/2026-04-24-kotoha-phase-1-implementation.md`
- Detailed plan: `docs/superpowers/plans/2026-04-24-kotoha-phase-1-p1-1.md`
