# Kotoha Phase 1 — P1-2 Implementation Plan (ZenzBackend + Layer 3 smoke)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `crates/kotoha-core/src/kanji/zenz.rs` の P1-1 skeleton を llama-cpp-2 経由の実装に置き換え、Zenz-v2.5-medium GGUF model で ひらがな→漢字 変換を行えるようにする。`crates/kotoha-core/Cargo.toml` の `zenz` feature を `["dep:llama-cpp-2"]` に書き換え、`llama-cpp-2` を optional dependency として追加する。Layer 3 (`zenz-smoke` feature) integration test 5 件を `crates/kotoha-core/tests/kanji_zenz_smoke.rs` に追加する。AzooKey Zenzai docs (<https://github.com/azooKey/AzooKeyKanaKanjiConverter/blob/main/Docs/zenzai.md>) を最初の Task で通読し、prompt format 解析ログを WBS に残す。

**Architecture:** ZenzBackend は llama-cpp-2 の `LlamaModel` を所有する。`convert` 呼び出しで毎回 prompt を組み立てて tokenize → generate → decode → `score_sort_dedupe` で contract enforcement、という流れ。prompt format と tokenizer 挙動は AzooKey Zenzai docs と実機検証で確定させる (Open Question Q2)。決定論的出力は `temperature = 0.0` (greedy) + `seed = Some(0)` で担保する (Open Question Q5)。`KOTOHA_ZENZ_MODEL_PATH` 環境変数が未設定の場合、Layer 3 smoke test は `#[ignore]` 相当で skip し、cargo test 全体が fail しないようにする (spec §8.3)。

**Tech Stack:** Rust 2021 / MSRV 1.80 / `llama-cpp-2` (version pin: P1-2 開始時点の Context7 / crates.io 最新 stable — Q1 解決)、`thiserror`、`tracing`。Zenz-v2.5-medium GGUF (`Miwa-Keita/zenz-v2.5-medium-gguf`) を手動配置する想定。

**Spec:** docs/superpowers/specs/2026-04-24-kotoha-phase-1-design.md §3.1 / §3.2 / §3.3 / §5.3 / §6 / §8.3 / §8.6 / §10 / §13

**Phase 1 overall plan:** docs/superpowers/plans/2026-04-24-kotoha-phase-1-implementation.md §"PR #3 — P1-2" (lines 754-780)

**P1-1 carry-over:** docs/wbs/2026-04-24-feature-65-kanji-skeleton-mockbackend.md — "P1-2 への申し送り" section

---

## 目次

- [マイルストーン位置付け](#マイルストーン位置付け)
- [ファイル構成](#ファイル構成)
- [P1-2 完了条件](#p1-2-完了条件)
- [共通規約](#共通規約)
- [Task P1-2-0: 実装 ISSUE + branch 作成 + baseline 確認](#task-p1-2-0-実装-issue--branch-作成--baseline-確認)
- [Task P1-2-1: AzooKey Zenzai docs 通読 + prompt format 解析メモ作成](#task-p1-2-1-azookey-zenzai-docs-通読--prompt-format-解析メモ作成)
- [Task P1-2-2: llama-cpp-2 version pin research (Q1 解決)](#task-p1-2-2-llama-cpp-2-version-pin-research-q1-解決)
- [Task P1-2-3: Cargo.toml に llama-cpp-2 optional dep 追加 + zenz feature 書き換え](#task-p1-2-3-cargotoml-に-llama-cpp-2-optional-dep-追加--zenz-feature-書き換え)
- [Task P1-2-4: ZenzBackend struct rewrite](#task-p1-2-4-zenzbackend-struct-rewrite)
- [Task P1-2-5: ZenzBackend::load 実装](#task-p1-2-5-zenzbackendload-実装)
- [Task P1-2-6: ZenzBackend::model_id 実装](#task-p1-2-6-zenzbackendmodel_id-実装)
- [Task P1-2-7: ZenzBackend::convert 実装](#task-p1-2-7-zenzbackendconvert-実装)
- [Task P1-2-8: fixture TSV + Layer 3 test file skeleton](#task-p1-2-8-fixture-tsv--layer-3-test-file-skeleton)
- [Task P1-2-9: Layer 3 empirical verification (model 必要)](#task-p1-2-9-layer-3-empirical-verification-model-必要)
- [Task P1-2-10: Multi-feature verification gate](#task-p1-2-10-multi-feature-verification-gate)
- [Task P1-2-11: commit history 確認 + push + PR 作成](#task-p1-2-11-commit-history-確認--push--pr-作成)
- [Task P1-2-12: Medium-tier review + findings 解消 + squash-merge](#task-p1-2-12-medium-tier-review--findings-解消--squash-merge)
- [Task P1-2-13: WBS ログ作成 + develop 直接 push](#task-p1-2-13-wbs-ログ作成--develop-直接-push)
- [P1-2 完了条件チェックリスト](#p1-2-完了条件チェックリスト)
- [Spec Coverage 確認](#spec-coverage-確認)
- [Self-Review 済み事項](#self-review-済み事項)

---

## マイルストーン位置付け

| 観点 | 内容 |
|---|---|
| Phase | 1 / Kana → Kanji |
| マイルストーン | P1-2 of 5 (P1-0 ... P1-4) |
| 前提 | P1-1 (kanji skeleton + MockBackend) が develop に merge 済み (#66、merge commit `cb2a458`) |
| 後続 | P1-3 (CLI `kotoha-kanji` + `scripts/phase1-smoke.sh`) |
| 工数見積 | 2.0 日 |
| Scope tier | Medium (3 modified + 2 new files、約 600 LOC 想定、Spec §11 の milestone table に準拠) |

---

## ファイル構成

### 変更 (3 ファイル)

| ファイル | 変更内容 |
|---|---|
| `Cargo.toml` (workspace root) | `[workspace.dependencies]` に `llama-cpp-2 = "<PINNED_VERSION>"` を追加 (version は Task P1-2-2 で確定) |
| `crates/kotoha-core/Cargo.toml` | `[dependencies]` に `llama-cpp-2 = { workspace = true, optional = true }` を追加、`[features]` の `zenz = []` を `zenz = ["dep:llama-cpp-2"]` に書き換える |
| `crates/kotoha-core/src/kanji/zenz.rs` | P1-1 skeleton (83 LOC) を llama-cpp-2 ベース実装 (約 300〜400 LOC) に置換。struct field (`_placeholder: ()`) を実 `LlamaModel` 相当に、`load` / `model_id` / `convert` を本実装に書き換える。`#[allow(dead_code)]` / `#[allow(unused_variables)]` / `#[derive(Debug)]` も再評価する |

### 新規作成 (2 ファイル)

| ファイル | 責務 |
|---|---|
| `crates/kotoha-core/tests/kanji_zenz_smoke.rs` | Layer 3 integration test (spec §8.3、5 件、約 150 LOC)。file-level `#![cfg(feature = "zenz-smoke")]` で gate。`KOTOHA_ZENZ_MODEL_PATH` 環境変数が未設定なら SKIP (stdout に `SKIPPED: ...` を出して `return`)、設定済みなら 5 fixture (にほんご / かんじ / あした / やまださん / ことば) を読み込んで substring 部分一致を assertion する |
| `crates/kotoha-core/tests/fixtures/kanji_smoke.tsv` | Layer 3 / Layer 4 共通 fixture (5 行、`input_hiragana<TAB>expected_substring` 形式、spec §8.3 の規定)。Layer 4 (P1-3 で追加の `scripts/phase1-smoke.sh`) と parity を保つため、本 PR で完成させる |

### 削除 (0 ファイル)

なし。`zenz_load_returns_backend_error_in_p1_1_skeleton` test は zenz.rs の中で書き換え (Task P1-2-5) であり、file 単位の削除はない。

---

## P1-2 完了条件

以下すべてが満たされたら P1-2 完了とする。

- [ ] 実装 ISSUE が close 済み
- [ ] `llama-cpp-2` version が pinned 済み、かつ pin 判断の根拠 (Context7 / crates.io 確認日、alternative 比較) が commit message または WBS に明記されている
- [ ] `ZenzBackend::load` / `model_id` / `convert` すべて実装済み (`todo!()` / P1-1 の `Err(KanjiError::Backend { reason: "... P1-2" })` skeleton がどこにも残っていない)
- [ ] `cargo check -p kotoha-core --no-default-features` PASS
- [ ] `cargo check -p kotoha-core` (default) PASS
- [ ] `cargo check -p kotoha-core --features mock-backend` PASS
- [ ] `cargo check -p kotoha-core --features zenz` PASS (llama-cpp-2 を実際に引き込む compile)
- [ ] `cargo check -p kotoha-core --features zenz-smoke` PASS
- [ ] `cargo check -p kotoha-core --all-features` PASS
- [ ] `cargo test --workspace` (default) PASS — Layer 3 は feature gate で compile skip されるので既存件数と同一か、Layer 2 削減のみ
- [ ] `cargo test --workspace --features mock-backend` PASS — Layer 1 + Layer 2 合計件数は P1-1 baseline (171) に ZenzBackend 追加 unit test 分 (約 +2) を加算した値
- [ ] `KOTOHA_ZENZ_MODEL_PATH` 設定済みの環境で `cargo test --workspace --features zenz-smoke` で Layer 3 5 件 PASS
- [ ] `KOTOHA_ZENZ_MODEL_PATH` 未設定の環境で `cargo test --workspace --features zenz-smoke` が SKIP メッセージを出しつつ exit 0 で終わる (CI/lefthook を fail させない)
- [ ] `cargo clippy --workspace --all-targets --all-features -- -D warnings` warnings ゼロ
- [ ] `cargo fmt --all --check` diff ゼロ
- [ ] WBS に「prompt format 解析ログ」section (AzooKey docs 通読結果 + 実装で判明した prompt format) が記載されている
- [ ] WBS に「cold start latency 測定」section (1 回目 load + 2 回目 warm 相当の計測値) が記載されている
- [ ] P1-1 の `zenz_load_returns_backend_error_in_p1_1_skeleton` unit test が新 spec に置換または削除されている (新 spec = `load` が実 GGUF を開けるか、または `ModelNotFound` を返すかのどちらか)
- [ ] zenz.rs の `#[allow(dead_code)]` / `#[allow(unused_variables)]` が削除されている (field / 引数が実際に使われるため不要)
- [ ] PR が develop に squash-merge 済み、branch 削除済み
- [ ] WBS ログが develop に push 済み

---

## 共通規約

本 plan を実装するサブエージェントは以下の規約を遵守する。共通規約は P1-1 plan §「共通規約」(<2026-04-24-kotoha-phase-1-p1-1.md>) を基底としつつ、P1-2 固有の追加規約を以下に記す。

### P1-1 から継承する共通規約

- **TDD 遵守**: 実装 Task (P1-2-5 / P1-2-6) は Red → Green の順を守る。P1-2-7 `convert` 実装は model inference を伴うため unit test では Red → Green が不完全になる (model 実機が無いと Green 側が書けない)。このため `convert` の検証は Layer 3 smoke (Task P1-2-8 + P1-2-9) に委ねる。
- **commit 頻度**: 各実装 Task (P1-2-3 / P1-2-4 / P1-2-5 / P1-2-6 / P1-2-7 / P1-2-8) 完了時点で 1 commit。research task (P1-2-1 / P1-2-2) は commit しない (scratch file は WBS に統合する時点で commit される)。verification task (P1-2-9 / P1-2-10) は fixture 調整が発生した場合のみ commit。P1-2-0 / P1-2-11 / P1-2-12 は commit なし (git / gh 操作のみ)。
- **言語**: commit message / PR body / ISSUE body / rustdoc は英語。本 plan 本文と対話応答と WBS 本文は日本語。Scratch file (`docs/wbs/p1-2-prompt-format-analysis.md`) も日本語可。
- **可視性**: module 内部 helper は `pub(crate)`。公開 API (`ZenzBackend` とその pub method) のみ `pub`。llama-cpp-2 の具体 type (LlamaModel 等) は ZenzBackend の private field に閉じ込め、外部には漏らさない。
- **feature flag 規律**: zenz.rs は引き続き `#[cfg(feature = "zenz")]`、tests/kanji_zenz_smoke.rs は file-level `#![cfg(feature = "zenz-smoke")]`。
- **rustdoc**: 全 pub 項目に `///` を付け、spec § 番号への参照を含める。`convert` は `# Prompt format` section を設け AzooKey docs 参照と実機で確定した prompt 形を明記する。
- **verbatim output**: 各 Task の verification step で `cargo test` / `cargo check` / `cargo clippy` を走らせた場合、サブエージェントは final report に stdout の head (先頭 5 行) + tail (末尾 15 行) を verbatim で含める (CLAUDE.md「Sub-agent Self-Report is Untrusted」節準拠)。
- **`--no-verify` 禁止**: push 時に lefthook pre-push を `--no-verify` で skip することは禁止。hook が fail したら原因を直す。
- **placeholder 置換**: 本 plan 中の `<IMPL_ISSUE>` は Task P1-2-0 で作成した実装 ISSUE 番号に、`<PR_NUMBER>` は Task P1-2-11 で作成した PR 番号に、`<MERGE_COMMIT>` は P1-2-12 で取得する merge commit SHA に、`<PINNED_VERSION>` は P1-2-2 で確定する llama-cpp-2 version に置換する。
- **`#[non_exhaustive]` 適用**: 新規型を追加する場合は ADR 0006 準拠で `#[non_exhaustive]` を付与する (P1-2 では新規型追加は想定していないが、inference helper struct を作る場合は適用)。

### P1-2 固有の追加規約

- **Research task の evidence 必須**: Task P1-2-1 (AzooKey docs 通読) / Task P1-2-2 (llama-cpp-2 version pin) は「コード変更なし」だが、両 Task とも `docs/wbs/p1-2-prompt-format-analysis.md` (scratch file) に結果を記録する。scratch file は最終的に WBS 本体の「prompt format 解析ログ」section に統合する。scratch file 単体は git に commit しない (WBS 統合時に初めて commit される)。
- **llama-cpp-2 version pin の記録**: Task P1-2-2 で確定した version 番号、Context7 / crates.io 確認日付、alternative 候補 (`llama_cpp-rs` / `mistral.rs` / `Candle`) 不採用の再確認結果を、Task P1-2-3 の commit message に明記する。ADR 0010 (P1-4 で作成予定) で同じ内容を再利用できるよう、記録の文字列形を spec §3.1 との整合を保って書く。
- **Zenz model setup の文書化**: 実装者が Zenz-v2.5-medium GGUF を手元に用意するための手順を、`crates/kotoha-core/tests/kanji_zenz_smoke.rs` の file-level doc comment (`//!`) または README の「Model setup」 section に明記する (Task P1-2-8)。HuggingFace の `Miwa-Keita/zenz-v2.5-medium-gguf` を参照し、`huggingface-cli download` または直接 URL `curl` の 2 方法を示す。
- **SKIP on missing env var idiom**: Layer 3 test 各関数の先頭で `let model_path = match std::env::var("KOTOHA_ZENZ_MODEL_PATH") { Ok(p) => PathBuf::from(p), Err(_) => { println!("SKIPPED: KOTOHA_ZENZ_MODEL_PATH not set"); return; } };` 型の早期 return pattern を採用する。`#[ignore]` attribute は使わない (spec §8.3 の「opt-in」設計は env-var 式を採用している)。
- **prompt format と tokenizer は AzooKey docs 準拠**: P1-2-1 の解析結果を prompt 組み立てコードにそのまま反映する。AzooKey docs と実装が乖離する場合は、実装側を AzooKey に寄せ、乖離理由を rustdoc と WBS の両方に明記する。
- **決定論性**: `options.temperature == 0.0 && options.seed == Some(0)` のとき同一入力が同一出力を返すことを Layer 3 smoke で間接的に検証する。llama-cpp-2 の API が temperature / seed を直接受け付けない場合、sampler 構築時に該当 parameter を明示的に設定する (Q5 解決)。

---

## Task P1-2-0: 実装 ISSUE + branch 作成 + baseline 確認

**Files:** (ローカル変更なし、GitHub + git 操作のみ)

注: 本 plan PR (#<PLAN_PR>、plan Issue #67) は plan 文書を書くための PR である。本 Task で作成する「実装 ISSUE」はそれとは別に、ZenzBackend 実装を追跡する新 ISSUE である。

- [ ] **Step 1: 作業ディレクトリと baseline 確認**

Run:

```bash
cd /home/kohshiro/develops/student/kotoha-ime
git checkout develop
git pull
git status
git log --oneline -3
```

Expected: `develop` が `origin/develop` と同期、working tree clean。`git log` の先頭付近に P1-1 の WBS merge commit (`docs(wbs): P1-1 kanji skeleton + MockBackend ...`、SHA `7f54c10`) と P1-1 本体 merge commit (`feat(kanji): add kanji module skeleton + MockBackend (#65) (#66)`、SHA `cb2a458`) があるはず。

- [ ] **Step 2: baseline `cargo test` の件数を記録**

Run:

```bash
cargo test --workspace 2>&1 | tail -20
```

Expected: すべて PASS。`test result: ok. 160 passed` (P1-1 完了時点の件数) が出力される。件数を Task P1-2-10 の regression 確認に使う。

- [ ] **Step 3: 実装 ISSUE を作成**

Run:

```bash
gh issue create \
  --title "P1-2: ZenzBackend via llama-cpp-2 + Layer 3 smoke" \
  --body "Phase 1 milestone P1-2: replace the ZenzBackend P1-1 skeleton with a llama-cpp-2 backed implementation and add Layer 3 (zenz-smoke) integration tests.

## Scope

- Add \`llama-cpp-2\` as an optional dependency (version pinned against Context7 / crates.io latest stable at implementation time)
- Rewrite \`crates/kotoha-core/Cargo.toml\` \`zenz\` feature from \`[]\` to \`[\"dep:llama-cpp-2\"]\`
- Rewrite \`crates/kotoha-core/src/kanji/zenz.rs\` from the P1-1 skeleton to a real implementation (ZenzBackend holding LlamaModel; load / model_id / convert implemented)
- Add Layer 3 integration test file \`crates/kotoha-core/tests/kanji_zenz_smoke.rs\` (5 tests, file-level \`#![cfg(feature = \\\"zenz-smoke\\\")]\`, KOTOHA_ZENZ_MODEL_PATH env-var gated)
- Add fixture \`crates/kotoha-core/tests/fixtures/kanji_smoke.tsv\` (5 rows, input_hiragana<TAB>expected_substring)
- Read AzooKey Zenzai docs (<https://github.com/azooKey/AzooKeyKanaKanjiConverter/blob/main/Docs/zenzai.md>) before implementation and record prompt format analysis in WBS

## Out of Scope

- CLI \`kotoha-kanji\` (P1-3)
- \`scripts/phase1-smoke.sh\` (P1-3)
- ADR 0009 / 0010 / 0011 (P1-4)
- HuggingFace auto-download (Phase 2+)

## Reference

- Spec: \`docs/superpowers/specs/2026-04-24-kotoha-phase-1-design.md\` §3.1 / §3.2 / §3.3 / §5.3 / §6 / §8.3 / §8.6 / §10 / §13
- Phase 1 overall plan: \`docs/superpowers/plans/2026-04-24-kotoha-phase-1-implementation.md\` §\"PR #3 — P1-2\"
- Detailed plan: \`docs/superpowers/plans/2026-04-24-kotoha-phase-1-p1-2.md\`
- P1-1 carry-over: \`docs/wbs/2026-04-24-feature-65-kanji-skeleton-mockbackend.md\` \"P1-2 への申し送り\""
```

Expected: ISSUE が作成され、URL と番号が表示される。番号を `<IMPL_ISSUE>` として記録する。

- [ ] **Step 4: 実装 branch 作成**

Run (`<IMPL_ISSUE>` は Step 3 で取得した値):

```bash
git checkout -b feature/<IMPL_ISSUE>-zenz-backend-layer3-smoke develop
git branch --show-current
```

Expected: `feature/<IMPL_ISSUE>-zenz-backend-layer3-smoke` branch が develop から作成され、checkout される。

---

## Task P1-2-1: AzooKey Zenzai docs 通読 + prompt format 解析メモ作成

**Files:**
- Create: `docs/wbs/p1-2-prompt-format-analysis.md` (scratch file、git commit しない)

`[research]` 本 Task は実装に着手する前の必読調査。spec §3.3 の指示「P1-2 着手前に必ず通読」に基づく。結果を scratch file として残し、最終的には Task P1-2-13 の WBS 本体に統合する。

- [ ] **Step 1: AzooKey Zenzai docs を WebFetch で取得**

Run: WebFetch tool (または `curl`) で `https://github.com/azooKey/AzooKeyKanaKanjiConverter/blob/main/Docs/zenzai.md` の内容を取得する。prompt:「Zenz model の prompt format、tokenizer 挙動、decoding stop condition、temperature / top-k / top-p 推奨、score aggregation 方式を抽出して要約して」。

Expected: 以下 5 観点の記述を doc から抜き出せる:

1. Prompt template の特殊 token 配置 (BOS / EOS / separator 等)
2. 入力ひらがな列の tokenize 方針 (character-level 単位での区切り)
3. Decoding stop condition (EOS token の検出 / max new tokens)
4. Top-K / top-P / temperature の Zenz 文脈での推奨範囲
5. Score (log-probability の aggregation) の扱い方

docs で明記されていない項目があれば「未記載 — 実装側で empirical に確定する」と記録する。

- [ ] **Step 2: scratch file を Write で作成**

Write `docs/wbs/p1-2-prompt-format-analysis.md`:

Content (以下は template。Step 1 で抽出した結果を各 bullet に書き込む):

```markdown
# P1-2 prompt format 解析 scratch (WBS 本体に統合予定)

本 file は Task P1-2-1 / Task P1-2-2 の研究結果を一時的に記録する scratch。
Task P1-2-13 で WBS 本体 (`docs/wbs/2026-04-24-feature-<IMPL_ISSUE>-zenz-backend-layer3-smoke.md`) の
「prompt format 解析ログ」「cold start latency 測定」「llama-cpp-2 version pin rationale」 section に
統合した時点で本 scratch は削除する。

## 1. AzooKey Zenzai docs 通読 (2026-04-XX 時点)

出典: <https://github.com/azooKey/AzooKeyKanaKanjiConverter/blob/main/Docs/zenzai.md>

### 1.1 Prompt template の特殊 token 配置

- BOS token: <...抽出結果...>
- EOS token: <...抽出結果...>
- separator / 区切り token: <...抽出結果...>
- 完全な prompt 例 (ひらがな "にほんご" を与える場合): <...抽出結果...>

### 1.2 入力ひらがな列の tokenize 方針

- character-level 分割か byte-level BPE か: <...抽出結果...>
- ひらがな 1 文字が 1 token に対応するかの確認: <...抽出結果...>

### 1.3 Decoding stop condition

- EOS token 検出: <...抽出結果...>
- max new tokens の推奨値: <...抽出結果...>

### 1.4 Top-K / top-P / temperature の推奨範囲

- temperature: <...抽出結果...>
- top-K: <...抽出結果...>
- top-P: <...抽出結果...>

### 1.5 Score (log-probability) の扱い方

- token 単位の log-prob を句単位に aggregate するか: <...抽出結果...>
- 正規化 (length-normalized) の有無: <...抽出結果...>

### 1.6 AzooKey docs から得られない項目 / 実装側で確定する項目

- <項目 A> — 実装時に実機検証で確定
- <項目 B> — 実装時に実機検証で確定
```

- [ ] **Step 3: scratch file を verify**

Run:

```bash
ls -la docs/wbs/p1-2-prompt-format-analysis.md
wc -l docs/wbs/p1-2-prompt-format-analysis.md
git status docs/wbs/p1-2-prompt-format-analysis.md
```

Expected: file が存在し、section 1.1 〜 1.6 が埋まっている状態で 30 行以上。`git status` で `Untracked files:` に表示されること (scratch file は commit しないため untracked のままでよい)。

(No commit — research task は後続の implementation task での commit に吸収されない。Task P1-2-13 の WBS 統合 commit で初めて本 scratch 内容は git に入る。scratch file そのものは WBS 統合後に削除して OK。)

---

## Task P1-2-2: llama-cpp-2 version pin research (Q1 解決)

**Files:**
- Modify: `docs/wbs/p1-2-prompt-format-analysis.md` (Task P1-2-1 の scratch に section 2 を追記)

`[research]` Open Question Q1 (llama-cpp-2 の具体 version pin) を解消する。Context7 MCP が優先、fallback で crates.io WebFetch。

- [ ] **Step 1: Context7 で llama-cpp-2 を resolve**

Run:

```
mcp__plugin_context7_context7__resolve-library-id  (query="llama-cpp-2")
```

Expected: library id (例: `/utilityai/llama-cpp-rs`) が返る。library id が match しなければ「Context7 に該当なし」と記録して Step 2 の WebFetch に fallback。

- [ ] **Step 2: Context7 に match があれば query-docs で最新 version を確認**

Run:

```
mcp__plugin_context7_context7__query-docs  (library_id=<Step 1 の結果>, query="latest stable version, MSRV, features")
```

Expected: 最新 version、MSRV、feature flags 一覧。Kotoha の MSRV 1.80 と互換か確認する。互換なら採用候補、非互換なら 1 個前の stable を候補にする。

- [ ] **Step 3: Context7 で不足なら crates.io WebFetch**

Run: WebFetch tool で `https://crates.io/crates/llama-cpp-2` を取得。prompt:「最新 stable version、release date、license、MSRV、主要 feature flags、breaking change history を抽出して」。

Expected: `0.x.y` 形の最新 stable version と release date、MSRV との整合、license (MIT / Apache-2.0 / その他)、代表的 feature flag (cuda / metal / vulkan など)。

- [ ] **Step 4: alternative 候補の再確認 (spec §3.1 追認)**

Spec §3.1 で不採用とされた 3 候補を crates.io / GitHub で現状確認:

- `llama_cpp-rs`: 最新 release 日時、llama.cpp 追随度
- `mistral.rs`: GPT-2 / Zenz 系対応状況
- `Candle`: GGUF tokenizer (character-level + byte-level BPE) の実装状況

Expected: spec §3.1 の不採用判断が P1-2 着手時点でも有効であることを再確認する。仮に状況が大きく変化していた場合は plan 実装を停止し、spec 改訂の必要性を報告する。

- [ ] **Step 5: scratch file に section 2 を追記**

Edit `docs/wbs/p1-2-prompt-format-analysis.md`: file 末尾に以下を追記する。

```markdown

## 2. llama-cpp-2 version pin research (Q1 解決)

### 2.1 確認日

- 確認日: 2026-04-XX
- 確認手段: Context7 MCP / crates.io WebFetch (使用したものにチェック)

### 2.2 採用 version

- Version: `<PINNED_VERSION>` (例: `0.1.x` など具体値)
- 採用理由: <latest stable / MSRV 1.80 互換 / など>

### 2.3 MSRV / feature flags 整合

- llama-cpp-2 の MSRV: <確認値>
- Kotoha の MSRV (1.80) と互換: yes / no
- 有効化する feature flags: <default のみ / `cuda` / `metal` / ...>

### 2.4 Alternative 候補の再確認

| 候補 | 最新 version / release 日 | 不採用理由 (P1-2 時点で spec §3.1 を再評価) |
|---|---|---|
| llama_cpp-rs | <...> | <spec §3.1 と変わらず / 変化あり / 採用再検討> |
| mistral.rs | <...> | <...> |
| Candle | <...> | <...> |

### 2.5 License / 商用利用可否

- llama-cpp-2 license: <MIT / Apache-2.0 / ...>
- Kotoha (OSS 予定、Phase 1 project CLAUDE.md に記載) との互換性: OK / NG
```

- [ ] **Step 6: scratch file を verify**

Run:

```bash
wc -l docs/wbs/p1-2-prompt-format-analysis.md
grep -c "^## " docs/wbs/p1-2-prompt-format-analysis.md
```

Expected: 行数が Task P1-2-1 時点から 30+ 増えている、section (`## `) 数が 2 (section 1 + section 2)。

(No commit — 後続の Task P1-2-3 で `Cargo.toml` 変更と同時に commit 予定。scratch file 自体は WBS 統合 Task P1-2-13 まで untracked のまま残す。)

---

## Task P1-2-3: Cargo.toml に llama-cpp-2 optional dep 追加 + zenz feature 書き換え

**Files:**
- Modify: `Cargo.toml` (workspace root)
- Modify: `crates/kotoha-core/Cargo.toml`

`[verbatim]` Task P1-2-2 で pin した version を使って 2 Cargo.toml を更新する。`<PINNED_VERSION>` は Task P1-2-2 で確定した値に置換する。

- [ ] **Step 1: workspace root `Cargo.toml` を読む**

Read: `/home/kohshiro/develops/student/kotoha-ime/Cargo.toml`

Expected: `[workspace.dependencies]` section が存在する (`thiserror` / `tracing` / `proptest` などが pin されているはず)。llama-cpp-2 が無いことを確認する。

- [ ] **Step 2: workspace root `Cargo.toml` に llama-cpp-2 を追加**

Edit `/home/kohshiro/develops/student/kotoha-ime/Cargo.toml`: `[workspace.dependencies]` section 内の末尾 (他の依存の直下) に以下を追加する。

```toml
# RESEARCH-DEPENDENT: <PINNED_VERSION> を P1-2-2 で確定した値に置換する。
# 採用理由: Phase 1 spec §3.1 に基づき Context7 / crates.io 最新 stable を pin。
# 不採用 alternative: llama_cpp-rs / mistral.rs / Candle (spec §3.1 参照、詳細は P1-2-2 WBS scratch)。
llama-cpp-2 = "<PINNED_VERSION>"
```

- [ ] **Step 3: `crates/kotoha-core/Cargo.toml` を読む**

Read: `/home/kohshiro/develops/student/kotoha-ime/crates/kotoha-core/Cargo.toml`

Expected: `[dependencies]` に `thiserror` と `tracing` が workspace 参照で並んでおり、`[features]` に `zenz = []` が定義されている (P1-1 の現状)。

- [ ] **Step 4: `crates/kotoha-core/Cargo.toml` に llama-cpp-2 依存を追加**

Edit `crates/kotoha-core/Cargo.toml`:

old_string:

```toml
[dependencies]
thiserror = { workspace = true }
tracing = { workspace = true }
```

new_string:

```toml
[dependencies]
thiserror = { workspace = true }
tracing = { workspace = true }
# `llama-cpp-2` is optional so default features do not pull in the C++ library and
# its bindgen build step. Activated only when the `zenz` feature is enabled.
llama-cpp-2 = { workspace = true, optional = true }
```

- [ ] **Step 5: `crates/kotoha-core/Cargo.toml` の `zenz` feature を書き換え**

Edit `crates/kotoha-core/Cargo.toml`:

old_string:

```toml
# `zenz` gates ZenzBackend. In P1-1 this is an empty flag (no `dep:llama-cpp-2`) because
# the real backend skeleton only contains todo!() stubs. P1-2 will replace this with
# `zenz = ["dep:llama-cpp-2"]` once the llama-cpp-2 dependency is wired up.
zenz = []
```

new_string:

```toml
# `zenz` gates ZenzBackend. P1-2 wires in llama-cpp-2 as the actual inference
# backend (previously an empty flag during the P1-1 skeleton). Activating this
# feature pulls in llama.cpp via llama-cpp-2 (C++ build required).
zenz = ["dep:llama-cpp-2"]
```

- [ ] **Step 6: 5 通りの feature combination で check が通ることを確認**

Run (timeout 15 min に設定、初回 llama-cpp-2 fetch + build が重い):

```bash
cargo check -p kotoha-core --no-default-features
cargo check -p kotoha-core
cargo check -p kotoha-core --features mock-backend
```

Expected: `zenz` feature を含まない 3 構成は llama-cpp-2 を fetch せずに通る。

次に `zenz` / `zenz-smoke` / `all-features` で確認 (llama-cpp-2 fetch + compile あり):

```bash
cargo check -p kotoha-core --features zenz
```

Expected: llama-cpp-2 crate の fetch + compile 後、本 PR 時点では zenz.rs がまだ P1-1 skeleton のままなので「`llama_cpp_2` が import されていない」警告が出る可能性があるが compile は通る (compile fail する場合は次の Task P1-2-4 で zenz.rs を書き換え、fix してから再試行)。

```bash
cargo check -p kotoha-core --features zenz-smoke
cargo check -p kotoha-core --all-features
```

Expected: どちらも PASS。

注: もし P1-1 の `zenz_load_returns_backend_error_in_p1_1_skeleton` test が zenz feature 下で warning (unused use 等) を出す場合、本 Task では書き換えず Task P1-2-5 で新 test に置換する。

- [ ] **Step 7: commit**

Run:

```bash
git add Cargo.toml crates/kotoha-core/Cargo.toml
git commit -m "chore(kotoha-core): add llama-cpp-2 dependency (version pinned via Q1)

Pin llama-cpp-2 to <PINNED_VERSION> (resolved in P1-2-2; see WBS for
Context7 / crates.io confirmation date and alternative comparison).

- Add llama-cpp-2 to [workspace.dependencies] with the pinned version.
- Add \`llama-cpp-2 = { workspace = true, optional = true }\` to kotoha-core
  so default builds do not pull in llama.cpp bindgen.
- Rewrite the \`zenz\` feature from [] to [\"dep:llama-cpp-2\"] now that the
  optional dependency is wired up.

Refs: Phase 1 spec §3.1 / P1-2 plan Task P1-2-3 / Open Question Q1."
```

Expected: `2 files changed`。

---

## Task P1-2-4: ZenzBackend struct rewrite

**Files:**
- Modify: `crates/kotoha-core/src/kanji/zenz.rs`

`[implementation-from-research]` P1-1 skeleton の struct field (`_placeholder: ()`) を llama-cpp-2 の `LlamaModel` 相当に置き換える。本 Task では struct 書き換えと import 追加のみ行い、`load` はまだ P1-1 と同じ `Err` を返す状態を維持する (次 Task P1-2-5 で本実装)。これにより Task P1-2-3 で `zenz` feature が compile 通ることを維持しつつ、段階的に進む。

- [ ] **Step 1: 現状 zenz.rs を読む**

Read: `crates/kotoha-core/src/kanji/zenz.rs`

Expected: P1-1 の skeleton 内容 (`_placeholder: ()` 含む struct、`Err(KanjiError::Backend { reason: "... P1-2" })` を返す load、`todo!()` の model_id / convert) を確認する。

- [ ] **Step 2: file 全体を書き換え (struct field + import のみ更新、load は P1-1 と同じ error を返す)**

Edit `crates/kotoha-core/src/kanji/zenz.rs`:

module doc comment (`//! ...`) から始まる部分を更新する。`//!` を以下に置換する。

old_string (file 先頭から `use crate::kanji::...` の行まで):

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
```

new_string:

```rust
//! Zenz GGUF model backend (via llama-cpp-2).
//!
//! # Status (P1-2)
//!
//! This module holds the real kanji conversion backend. An instance owns a
//! llama-cpp-2 `LlamaModel` loaded from a GGUF file, re-used across every
//! `convert` call.
//!
//! # Prompt format
//!
//! The prompt format follows the AzooKey Zenzai reference implementation
//! (<https://github.com/azooKey/AzooKeyKanaKanjiConverter/blob/main/Docs/zenzai.md>),
//! detailed in the WBS "prompt format 解析ログ" section.
//!
//! # Deterministic output
//!
//! When `ConvertOptions::temperature == 0.0` and `ConvertOptions::seed == Some(0)`
//! (the default), the backend performs greedy decoding with a fixed seed so
//! the Layer 3 smoke tests and the E2E smoke tests are reproducible.
//!
//! Spec: `docs/superpowers/specs/2026-04-24-kotoha-phase-1-design.md` §3.1,
//! §3.2, §3.3, §5.3, §6.

use std::path::{Path, PathBuf};

// RESEARCH-DEPENDENT: the exact import path depends on the llama-cpp-2 API
// surface pinned in P1-2-2. Typical imports (adapt to the pinned version):
//
//     use llama_cpp_2::model::{LlamaModel, params::LlamaModelParams};
//     use llama_cpp_2::context::{LlamaContext, params::LlamaContextParams};
//     use llama_cpp_2::llama_backend::LlamaBackend;
//     use llama_cpp_2::sampling::LlamaSampler;
//     use llama_cpp_2::token::LlamaToken;
//
// If the pinned version exposes a different module layout, update these imports
// to the equivalent types.
use llama_cpp_2::model::LlamaModel;

use crate::kanji::{Candidate, ConvertOptions, KanjiBackend, KanjiError};
```

次に struct 定義を書き換える。

old_string:

```rust
/// Zenz model backend. Enabled by `feature = "zenz"`.
///
/// **P1-1: all methods panic via `todo!()`. P1-2 supplies the real
/// implementation backed by llama-cpp-2.**
#[allow(dead_code)]
#[derive(Debug)]
pub struct ZenzBackend {
    model_path: PathBuf,
    _placeholder: (),
}
```

new_string:

```rust
/// Zenz GGUF model backend, backed by llama-cpp-2. Enabled by `feature = "zenz"`.
///
/// Construct with [`ZenzBackend::load`] or via
/// [`crate::kanji::load_backend`] applied to [`crate::kanji::BackendConfig::Zenz`].
///
/// # Invariants
///
/// - `model` holds an initialized llama-cpp-2 `LlamaModel`.
/// - `model_path` is the absolute path that was used to load `model`.
/// - The backend is single-threaded; wrap externally for concurrent use.
pub struct ZenzBackend {
    /// Loaded llama-cpp-2 model. Kept private so llama-cpp-2 types do not leak
    /// into the public API surface.
    model: LlamaModel,
    /// Path the model was loaded from. Used by logging and error diagnostics.
    model_path: PathBuf,
}
```

注: `#[derive(Debug)]` を外した — `LlamaModel` が `Debug` を実装しない可能性が高いため。Debug が必要な箇所が発生したら manual impl を後付けする (P1-4 bundle で対応)。

- [ ] **Step 3: 現状の `load` method を P1-1 error のままに維持 (次 Task で書き換えるため)**

Edit `crates/kotoha-core/src/kanji/zenz.rs`: `impl ZenzBackend { ... }` block 内の `load` は P1-1 の現状のまま維持する。ただし `#[allow(unused_variables)]` は残し、struct field 追加で `model` が未使用になったことによる warning を抑制するため struct 側にも一時的に `#[allow(dead_code)]` を付与する (Task P1-2-5 で除去)。

old_string:

```rust
pub struct ZenzBackend {
    /// Loaded llama-cpp-2 model. Kept private so llama-cpp-2 types do not leak
    /// into the public API surface.
    model: LlamaModel,
    /// Path the model was loaded from. Used by logging and error diagnostics.
    model_path: PathBuf,
}
```

new_string:

```rust
// Temporary `#[allow(dead_code)]` is removed in Task P1-2-5 once `load` actually
// constructs a `ZenzBackend { model, model_path }`.
#[allow(dead_code)]
pub struct ZenzBackend {
    /// Loaded llama-cpp-2 model. Kept private so llama-cpp-2 types do not leak
    /// into the public API surface.
    model: LlamaModel,
    /// Path the model was loaded from. Used by logging and error diagnostics.
    model_path: PathBuf,
}
```

- [ ] **Step 4: `cargo check --features zenz` で compile を確認**

Run:

```bash
cargo check -p kotoha-core --features zenz
```

Expected: PASS。`LlamaModel` は未使用 (`load` がまだ Err を返すため) だが、`#[allow(dead_code)]` で warning 抑制済み。`cargo test -p kotoha-core --features zenz` はまだ走らせない (`load` が P1-1 error を返すのは OK、次の Task で test を書き換える)。

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
git add crates/kotoha-core/src/kanji/zenz.rs
git commit -m "refactor(kanji): replace ZenzBackend::_placeholder with llama-cpp-2 LlamaModel field"
```

Expected: `1 file changed`。

---

## Task P1-2-5: ZenzBackend::load 実装

**Files:**
- Modify: `crates/kotoha-core/src/kanji/zenz.rs`

`[implementation-from-research]` `load` を P1-1 の error-returning から llama-cpp-2 ベースの本実装に置き換える。`ModelNotFound` / `ModelLoadFailed` を明示的に区別する。

- [ ] **Step 1: `load` method を書き換え**

Edit `crates/kotoha-core/src/kanji/zenz.rs`: `impl ZenzBackend { ... }` block の `load` を書き換える。

old_string:

```rust
impl ZenzBackend {
    /// Loads a Zenz GGUF model from `model_path`.
    ///
    /// # P1-1 status
    ///
    /// Returns [`KanjiError::Backend`] with a "not yet implemented (P1-2)"
    /// reason. This is a safe error — not a panic — so that callers going
    /// through [`crate::kanji::load_backend`] with the `zenz` feature enabled
    /// receive a typed error instead of a process abort.
    ///
    /// # P1-2 (planned)
    ///
    /// Opens the GGUF file via llama-cpp-2, validates the architecture, and
    /// caches the resulting context for subsequent `convert` calls.
    ///
    /// # Errors
    ///
    /// - P1-1: [`KanjiError::Backend`] unconditionally.
    /// - P1-2 (planned): [`KanjiError::ModelNotFound`] if `model_path` does
    ///   not exist; [`KanjiError::ModelLoadFailed`] if llama-cpp-2 rejects
    ///   the file.
    #[allow(unused_variables)]
    pub fn load(model_path: &Path) -> Result<Self, KanjiError> {
        Err(KanjiError::Backend {
            reason: "ZenzBackend is a P1-1 skeleton; real implementation lands in Phase 1 milestone P1-2".to_string(),
        })
    }
}
```

new_string:

```rust
impl ZenzBackend {
    /// Loads a Zenz GGUF model from `model_path`.
    ///
    /// # Preconditions
    ///
    /// - `model_path` points to an existing file. Otherwise
    ///   [`KanjiError::ModelNotFound`] is returned without touching
    ///   llama-cpp-2.
    ///
    /// # Postconditions
    ///
    /// On success, the returned `ZenzBackend` owns a llama-cpp-2 `LlamaModel`
    /// initialized from the GGUF file at `model_path`.
    ///
    /// # Errors
    ///
    /// - [`KanjiError::ModelNotFound`] if the file does not exist.
    /// - [`KanjiError::ModelLoadFailed`] if llama-cpp-2 fails to parse /
    ///   initialize the GGUF (wrapping the underlying error as a `source`).
    pub fn load(model_path: &Path) -> Result<Self, KanjiError> {
        if !model_path.exists() {
            return Err(KanjiError::ModelNotFound {
                path: model_path.to_path_buf(),
            });
        }

        // RESEARCH-DEPENDENT: adapt to the llama-cpp-2 API pinned in P1-2-2.
        // Typical shape (example sketch — verify exact function / type names
        // against the pinned version's docs):
        //
        //     let backend = LlamaBackend::init()
        //         .map_err(|e| KanjiError::ModelLoadFailed { source: Box::new(e) })?;
        //     let params = LlamaModelParams::default();
        //     let model = LlamaModel::load_from_file(&backend, model_path, &params)
        //         .map_err(|e| KanjiError::ModelLoadFailed { source: Box::new(e) })?;
        //
        // Points to confirm when filling in the real calls:
        // 1. Whether `LlamaBackend::init()` must be called once per process
        //    (globally) rather than per `load`. If so, wrap it in a `OnceLock`
        //    at module scope and reference the singleton from here.
        // 2. Whether `LlamaModel::load_from_file` returns a plain error or
        //    already boxes into `Box<dyn std::error::Error + Send + Sync>`.
        //    Adjust the `.map_err(...)` accordingly.
        // 3. Whether `model_path` needs to be a `&std::path::Path` (as typed)
        //    or a `&str`. If a string is required, use `.to_str().ok_or(...)?`
        //    and return `KanjiError::ModelLoadFailed` when the path is not UTF-8.
        let model = load_llama_model(model_path)
            .map_err(|e| KanjiError::ModelLoadFailed { source: e })?;

        Ok(Self {
            model,
            model_path: model_path.to_path_buf(),
        })
    }
}

// RESEARCH-DEPENDENT: helper isolating the llama-cpp-2 API surface. Keep this
// as a `fn` rather than inlining into `load` so the rest of the module does
// not depend on llama-cpp-2 import paths.
fn load_llama_model(
    model_path: &Path,
) -> Result<LlamaModel, Box<dyn std::error::Error + Send + Sync>> {
    // RESEARCH-DEPENDENT: replace this body with actual llama-cpp-2 calls
    // pinned in P1-2-2. The returned `LlamaModel` must be ready for `convert`
    // to tokenize / generate against.
    //
    // Example sketch (pseudocode — NOT verbatim; adapt to the pinned version):
    //
    //     let backend = llama_cpp_2::llama_backend::LlamaBackend::init()?;
    //     let params = llama_cpp_2::model::params::LlamaModelParams::default();
    //     let model = LlamaModel::load_from_file(&backend, model_path, &params)?;
    //     Ok(model)
    //
    let _ = model_path;
    unimplemented!("RESEARCH-DEPENDENT: fill in with llama-cpp-2 calls per P1-2-2 pin")
}
```

注: `load_llama_model` helper fn は研究結果に依存する本体なので `unimplemented!()` プレースホルダ + `// RESEARCH-DEPENDENT` コメントを明示する。実装者は P1-2-2 の結果を使って fn body を書き換える。

- [ ] **Step 2: struct 側の `#[allow(dead_code)]` を削除**

Edit `crates/kotoha-core/src/kanji/zenz.rs`: `pub struct ZenzBackend` の直前から `#[allow(dead_code)]` を削除する (struct field が `load` で使用されるようになったため)。

old_string:

```rust
// Temporary `#[allow(dead_code)]` is removed in Task P1-2-5 once `load` actually
// constructs a `ZenzBackend { model, model_path }`.
#[allow(dead_code)]
pub struct ZenzBackend {
```

new_string:

```rust
pub struct ZenzBackend {
```

- [ ] **Step 3: 既存 unit test を新仕様に置換 (P1-1 skeleton test を削除 + `ModelNotFound` test を追加)**

Edit `crates/kotoha-core/src/kanji/zenz.rs`: `#[cfg(test)] mod tests { ... }` block を書き換える。

old_string:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zenz_load_returns_backend_error_in_p1_1_skeleton() {
        let err = ZenzBackend::load(Path::new("/tmp/nonexistent.gguf"))
            .expect_err("ZenzBackend::load must error in P1-1 skeleton, not panic");
        match err {
            KanjiError::Backend { reason } => {
                assert!(
                    reason.contains("P1-2"),
                    "reason should point to P1-2 implementation: {reason}"
                );
            }
            other => panic!("unexpected error variant: {other:?}"),
        }
    }
}
```

new_string:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zenz_load_errors_on_missing_file() {
        let err = ZenzBackend::load(Path::new("/tmp/definitely-does-not-exist-kotoha-p1-2.gguf"))
            .expect_err("load must error when the path does not exist");
        match err {
            KanjiError::ModelNotFound { path } => {
                assert!(
                    path.to_string_lossy().contains("definitely-does-not-exist"),
                    "ModelNotFound path should echo the input: {}",
                    path.display()
                );
            }
            other => panic!("expected ModelNotFound, got: {other:?}"),
        }
    }
}
```

注: 成功 path の unit test は書かない — 実 GGUF model を必要とし、`KOTOHA_ZENZ_MODEL_PATH` 経由で Layer 3 smoke test (Task P1-2-8) が cover する。

- [ ] **Step 4: Red-Green 確認**

Run:

```bash
cargo test -p kotoha-core --lib --features zenz kanji::zenz::tests
```

Expected: 1 test (`zenz_load_errors_on_missing_file`) PASS。ただし `load_llama_model` helper が `unimplemented!()` のままだと、test は存在しない path を渡すので `ModelNotFound` が先に返って helper に到達せず PASS する。`load_llama_model` 本体を書き換えたら改めて同コマンドで PASS を確認する。

- [ ] **Step 5: clippy + fmt**

Run:

```bash
cargo clippy -p kotoha-core --all-targets --all-features -- -D warnings
cargo fmt --all --check
```

Expected: warnings ゼロ、fmt diff ゼロ。`load_llama_model` が `unimplemented!()` を使っている場合、clippy は `clippy::todo` / `clippy::unimplemented` を warn する可能性がある — この Task の commit 時点では `load_llama_model` 本体を P1-2-2 の研究結果に基づいて書き換えるので `unimplemented!()` は残さない。

- [ ] **Step 6: commit**

Run:

```bash
git add crates/kotoha-core/src/kanji/zenz.rs
git commit -m "feat(kanji): implement ZenzBackend::load via llama-cpp-2

- Replace the P1-1 skeleton (always-Err(KanjiError::Backend)) with a real
  load path that opens the GGUF file via llama-cpp-2.
- Return KanjiError::ModelNotFound when the path does not exist, and
  KanjiError::ModelLoadFailed for llama-cpp-2 parse / init failures.
- Replace the P1-1 skeleton unit test (zenz_load_returns_backend_error_in_p1_1_skeleton)
  with zenz_load_errors_on_missing_file, which exercises the new
  ModelNotFound branch without requiring a real model file.

Refs: Phase 1 spec §5.3, §5.5 / P1-2 plan Task P1-2-5."
```

Expected: `1 file changed`。

---

## Task P1-2-6: ZenzBackend::model_id 実装

**Files:**
- Modify: `crates/kotoha-core/src/kanji/zenz.rs`

`[implementation-from-research]` `model_id` は llama-cpp-2 が model metadata (GGUF 内の "general.name" 等) を expose しているならそれを使い、していないなら constant `"zenz-v2.5-medium"` を返す。

- [ ] **Step 1: `model_id` method を書き換え**

Edit `crates/kotoha-core/src/kanji/zenz.rs`: `impl KanjiBackend for ZenzBackend { ... }` block の `model_id` を書き換える。

old_string:

```rust
impl KanjiBackend for ZenzBackend {
    fn model_id(&self) -> &str {
        todo!("P1-2: implement via llama-cpp-2")
    }
```

new_string:

```rust
impl KanjiBackend for ZenzBackend {
    fn model_id(&self) -> &str {
        // RESEARCH-DEPENDENT: if the llama-cpp-2 API pinned in P1-2-2 exposes
        // model metadata (e.g. `self.model.meta_val("general.name")` or a
        // dedicated method), prefer that to the hard-coded string below.
        //
        // The default Phase 1 model per spec §3.2 is Zenz-v2.5-medium, so when
        // metadata lookup is unavailable we return that literal. Log a trace
        // event for observability so operators can detect mismatches in the
        // field.
        //
        // Suggested final shape once metadata access is confirmed:
        //
        //     self.model
        //         .meta_val("general.name")
        //         .unwrap_or("zenz-v2.5-medium")
        //
        // For P1-2 we return a static literal; dynamic metadata adoption is
        // tracked in Phase 2.
        "zenz-v2.5-medium"
    }
```

- [ ] **Step 2: unit test を追加**

Edit `crates/kotoha-core/src/kanji/zenz.rs`: `mod tests` 末尾 (最後の `}` の直前) に以下を追加する。

old_string (現在の tests 末尾):

```rust
    #[test]
    fn zenz_load_errors_on_missing_file() {
        let err = ZenzBackend::load(Path::new("/tmp/definitely-does-not-exist-kotoha-p1-2.gguf"))
            .expect_err("load must error when the path does not exist");
        match err {
            KanjiError::ModelNotFound { path } => {
                assert!(
                    path.to_string_lossy().contains("definitely-does-not-exist"),
                    "ModelNotFound path should echo the input: {}",
                    path.display()
                );
            }
            other => panic!("expected ModelNotFound, got: {other:?}"),
        }
    }
}
```

new_string:

```rust
    #[test]
    fn zenz_load_errors_on_missing_file() {
        let err = ZenzBackend::load(Path::new("/tmp/definitely-does-not-exist-kotoha-p1-2.gguf"))
            .expect_err("load must error when the path does not exist");
        match err {
            KanjiError::ModelNotFound { path } => {
                assert!(
                    path.to_string_lossy().contains("definitely-does-not-exist"),
                    "ModelNotFound path should echo the input: {}",
                    path.display()
                );
            }
            other => panic!("expected ModelNotFound, got: {other:?}"),
        }
    }

    // Note: `zenz_model_id_is_zenz_prefix` is not registered here because
    // constructing a `ZenzBackend` for the unit test requires a real GGUF
    // file, which is out of scope for the in-source test module. Instead,
    // Layer 3 smoke (see `tests/kanji_zenz_smoke.rs`) asserts that
    // `model_id()` starts with "zenz" once a real backend is loaded.
}
```

注: `model_id` の単体テストは Layer 3 smoke に委ねる (unit test では `ZenzBackend` を構築できないため — `load` が実 GGUF を要求する)。

- [ ] **Step 3: `cargo check --features zenz` で compile 確認**

Run:

```bash
cargo check -p kotoha-core --features zenz
```

Expected: PASS。

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
git add crates/kotoha-core/src/kanji/zenz.rs
git commit -m "feat(kanji): implement ZenzBackend::model_id"
```

Expected: `1 file changed`。

---

## Task P1-2-7: ZenzBackend::convert 実装

**Files:**
- Modify: `crates/kotoha-core/src/kanji/zenz.rs`

`[implementation-from-research]` `convert` は本 PR の中核。`validate_input` で contract 検査 → prompt 組み立て → tokenize → generate → token→string decode → Candidate 構築 → `score_sort_dedupe` で出力保証、という pipe。AzooKey docs の解析結果 (Task P1-2-1) と llama-cpp-2 API (Task P1-2-2) を組み合わせる。

- [ ] **Step 1: `convert` method を書き換え**

Edit `crates/kotoha-core/src/kanji/zenz.rs`: `impl KanjiBackend for ZenzBackend { ... }` block の `convert` を書き換える。

old_string:

```rust
    #[allow(unused_variables)]
    fn convert(&self, input: &str, options: &ConvertOptions) -> Result<Vec<Candidate>, KanjiError> {
        todo!("P1-2: implement via llama-cpp-2")
    }
}
```

new_string:

```rust
    fn convert(
        &self,
        input: &str,
        options: &ConvertOptions,
    ) -> Result<Vec<Candidate>, KanjiError> {
        // Step 1: Shared contract check (spec §5.6 — hiragana-only, ≤128 chars).
        crate::kanji::backend::validate_input(input)?;

        // Step 2: Early-return on empty input / top_k == 0 (spec §5.7 bullet 4).
        if input.is_empty() || options.top_k == 0 {
            return Ok(Vec::new());
        }

        // Step 3: Build the Zenz prompt. Exact token layout is recorded in
        // the WBS "prompt format 解析ログ" section (AzooKey Zenzai docs) and
        // replicated in `build_prompt` below.
        let prompt = build_prompt(input);

        // Step 4: Tokenize + generate via llama-cpp-2. `top_k` + `temperature`
        // + `seed` map to sampler parameters; deterministic output requires
        // `temperature == 0.0` (greedy) and a pinned `seed` (defaults to 0,
        // see ConvertOptions::default()).
        //
        // RESEARCH-DEPENDENT: adapt the call sites to the llama-cpp-2 API
        // pinned in P1-2-2.
        let raw_candidates = infer(&self.model, &prompt, options)
            .map_err(|reason| KanjiError::Backend { reason })?;

        // Step 5: Enforce spec §5.7 output guarantees (score-desc sort,
        // surface dedupe, top_k truncate).
        Ok(crate::kanji::backend::score_sort_dedupe(
            raw_candidates,
            options.top_k,
        ))
    }
}

/// Builds the Zenz prompt for the given hiragana input.
///
/// # Prompt format (summary)
///
/// Derived from AzooKey Zenzai docs
/// (<https://github.com/azooKey/AzooKeyKanaKanjiConverter/blob/main/Docs/zenzai.md>).
/// The full analysis lives in the P1-2 WBS "prompt format 解析ログ" section.
///
/// RESEARCH-DEPENDENT: once the exact BOS / separator / EOS token strings are
/// confirmed, replace the placeholder body with the concrete format string.
fn build_prompt(input: &str) -> String {
    // RESEARCH-DEPENDENT: replace with the exact template confirmed in P1-2-1.
    // Example placeholder (NOT verbatim; adapt to AzooKey docs):
    //
    //     format!("{BOS}{input}{SEP}", BOS = "<s>", SEP = "\u{E000}")
    //
    // Keep the function signature pure (String in / String out) so it is
    // unit-testable independently of llama-cpp-2.
    input.to_string()
}

/// Runs inference against the loaded llama-cpp-2 model and returns raw
/// (pre-dedupe, pre-truncate) candidates.
///
/// RESEARCH-DEPENDENT: this helper encapsulates every llama-cpp-2 call so
/// the rest of the module does not depend on llama-cpp-2 import paths.
fn infer(
    model: &LlamaModel,
    prompt: &str,
    options: &ConvertOptions,
) -> Result<Vec<Candidate>, String> {
    // RESEARCH-DEPENDENT: replace this body with real llama-cpp-2 calls
    // pinned in P1-2-2. Reference implementation sketch below — adapt to the
    // exact API surface.
    //
    //     let ctx_params = LlamaContextParams::default()
    //         .with_n_ctx(NonZeroU32::new(2048));
    //     let mut ctx = model.new_context(&backend, ctx_params)
    //         .map_err(|e| format!("context init failed: {e}"))?;
    //
    //     let tokens = model.str_to_token(prompt, AddBos::Always)
    //         .map_err(|e| format!("tokenize failed: {e}"))?;
    //
    //     // Feed prompt
    //     let mut batch = LlamaBatch::new(tokens.len(), 1);
    //     for (i, token) in tokens.iter().enumerate() {
    //         let logits = i == tokens.len() - 1;
    //         batch.add(*token, i as i32, &[0], logits)
    //             .map_err(|e| format!("batch add failed: {e}"))?;
    //     }
    //     ctx.decode(&mut batch).map_err(|e| format!("decode failed: {e}"))?;
    //
    //     // Greedy (or sampled) decoding loop until EOS / max_new_tokens.
    //     let seed = options.seed.unwrap_or(0);
    //     let mut sampler = LlamaSampler::greedy(); // adjust when temperature > 0
    //     // ... iteratively sample + decode ...
    //
    //     // Aggregate log-probabilities per completed candidate, then push into
    //     // a Vec<Candidate> before returning. `score_sort_dedupe` at the call
    //     // site enforces ordering / truncation / dedupe.
    //     Ok(vec![Candidate::new(surface, score)])
    //
    // Key checkpoints while filling in the body:
    // 1. EOS detection uses `model.token_eos()` or similar; stop the loop
    //    when emitted.
    // 2. `max_new_tokens` defaults to ~ input.chars().count() * 3 as a safety
    //    cap (AzooKey docs recommendation — record actual number in WBS).
    // 3. `options.top_k` decides how many candidates to generate (e.g., beam
    //    search with width = options.top_k, or repeated sampling with N
    //    diverse seeds). Research result determines the exact algorithm.
    // 4. Temperature == 0.0 → greedy sampler; > 0.0 → temperature + top-K /
    //    top-P sampler with the pinned seed.
    let _ = (model, prompt, options);
    Err("RESEARCH-DEPENDENT: infer() body must be filled in with llama-cpp-2 calls per P1-2-2 pin".to_string())
}
```

- [ ] **Step 2: `#[allow(unused_variables)]` を削除**

`convert` の `input` / `options` 引数は実際に使用されるため、attribute は書き換え前と同じ位置から削除済み (new_string には `#[allow(unused_variables)]` が含まれていないため)。

- [ ] **Step 3: `build_prompt` の unit test は追加しない (方針メモのみ)**

**方針 (PR #68 Medium review #3 反映)**: P1-2-7 では `build_prompt` の unit test を追加しない。理由は 2 点ある。

1. **tautology 回避**: 初期版の plan に含まれていた空入力向け build_prompt test (`build_prompt("")` を呼んで戻り値を `let _ = ...` で捨てるだけの形) は behavioral な assertion を一切持たない tautological test だった。そのまま追加すると test カバレッジの誤解を招くため削除する。
2. **research-dependent な output format**: `build_prompt` の実際の output format は P1-2-1 (AzooKey Zenzai docs 通読) の研究結果で確定する。研究結果が出る前に specific な assertion (例: `assert!(prompt.starts_with("<bos>"))`) を書くと投機的になり、研究結果と食い違うと PR 途中で書き直しが発生する。

代わりに以下の運用とする。

- `build_prompt` の behavioral verification は Layer 3 smoke tests (P1-2-8 で追加する `crates/kotoha-core/tests/kanji_zenz_smoke.rs`) が実 Zenz model を通した `convert` 呼び出しで end-to-end に行う。
- P1-2-1 research で stable な observable invariant (例: prompt は常に `<bos>` で始まる、など) が確定した場合は、**follow-up PR** で unit test を追加する。P1-2-7 本体では追加しない。

**Edit anchor 指針 (PR #68 Medium review #2 反映)**: 本 Step では zenz.rs への code 編集を行わない。P1-2-6 Step 2 で追加した `// Note:` コメントブロックを Step 3 の anchor として依存していた旧 plan は、P1-2-6 Step 2 の output が fmt / whitespace でドリフトすると Edit が壊れる脆弱な構造だった。追加編集を行わない方針にすることで、この脆弱な dependency を解消する。

（もし follow-up PR で unit test を追加する場合は、anchor として `mod tests` ブロックの closing `}` を使い、その直前に `#[test] fn ...` を挿入する。ただしその際も verbatim old_string を plan に埋め込むのではなく、実装者が zenz.rs を Read で読み込んで挿入位置を動的に特定する手順にする。）

- [ ] **Step 4: `cargo check --features zenz` で compile 確認**

Run:

```bash
cargo check -p kotoha-core --features zenz
```

Expected: PASS (ただし `infer` は `Err(...)` を返す状態なので `convert` を呼び出すと error になる。unit test では `convert` 自体を直接叩かない設計なので問題ない)。

- [ ] **Step 5: clippy + fmt**

Run:

```bash
cargo clippy -p kotoha-core --all-targets --all-features -- -D warnings
cargo fmt --all --check
```

Expected: warnings ゼロ、fmt diff ゼロ。`clippy::todo` / `clippy::unimplemented` は `infer` が P1-2-2 結果で書き換わった後には発生しないはず。書き換え前に clippy を走らせる場合は `// RESEARCH-DEPENDENT` コメントでの明示を目視確認してから `#[allow(clippy::todo)]` を一時的に `infer` に付与する (書き換え後に削除)。

- [ ] **Step 6: commit**

Run:

```bash
git add crates/kotoha-core/src/kanji/zenz.rs
git commit -m "feat(kanji): implement ZenzBackend::convert with prompt format + score extraction

- Wire validate_input (spec §5.6) and score_sort_dedupe (spec §5.7) around the
  llama-cpp-2 inference pipeline so the trait contract is enforced uniformly.
- Extract prompt assembly into \`build_prompt\` (pure fn, unit-testable) and
  llama-cpp-2 calls into \`infer\` (isolated helper) so llama-cpp-2 types do
  not leak across module boundaries.
- Document the prompt format in the rustdoc, pointing at the WBS
  \"prompt format 解析ログ\" section (AzooKey Zenzai docs derivation).
- Remove the P1-1 #[allow(unused_variables)] attribute from \`convert\` now
  that the arguments are actually consumed.

Refs: Phase 1 spec §5.3 / §5.6 / §5.7 / §6 / P1-2 plan Task P1-2-7 /
Open Question Q2 / Q5."
```

Expected: `1 file changed`。

---

## Task P1-2-8: fixture TSV + Layer 3 test file skeleton

**Files:**
- Create: `crates/kotoha-core/tests/fixtures/kanji_smoke.tsv`
- Create: `crates/kotoha-core/tests/kanji_zenz_smoke.rs`

`[verbatim]` Layer 3 smoke test の fixture と test harness を追加する。5 fixture は「top-1 が `expected_substring` を含む」を assertion とする (厳密一致は avoid、Zenz model 更新で揺らぐため)。

- [ ] **Step 1: fixtures directory を作成**

Run:

```bash
mkdir -p crates/kotoha-core/tests/fixtures
```

Expected: directory が作成される。

- [ ] **Step 2: fixture TSV を Write で作成**

Write `crates/kotoha-core/tests/fixtures/kanji_smoke.tsv`:

Content (各行は `input_hiragana<TAB>expected_substring`。初期値は Task P1-2-9 empirical verification で調整する可能性あり):

```tsv
にほんご	日本語
かんじ	漢字
あした	明日
やまださん	山田
ことば	言葉
```

注: TAB 区切り (スペースではない)。`expected_substring` は top-1 候補が含むべき部分文字列。

- [ ] **Step 3: Layer 3 test file を Write で作成**

Write `crates/kotoha-core/tests/kanji_zenz_smoke.rs`:

Content:

```rust
//! Layer 3 Zenz smoke integration tests.
//!
//! Runs the real `ZenzBackend` (via llama-cpp-2) against a short fixture of
//! hiragana inputs and asserts that each top-1 candidate contains the
//! expected substring.
//!
//! # Enabling
//!
//! - Build-time: enable the `zenz-smoke` feature (which implies `zenz`).
//! - Runtime: set `KOTOHA_ZENZ_MODEL_PATH` to the absolute path of a
//!   Zenz-v2.5-medium GGUF file (download via HuggingFace — see below).
//!
//! If the environment variable is unset, each test prints
//! `SKIPPED: KOTOHA_ZENZ_MODEL_PATH not set` and exits early, so the test
//! suite succeeds even when the model is missing. This follows spec §8.3
//! ("lefthook pre-push には含めない、opt-in で実行") without relying on
//! `#[ignore]`.
//!
//! # Model setup
//!
//! The default Phase 1 model is Zenz-v2.5-medium, hosted at
//! <https://huggingface.co/Miwa-Keita/zenz-v2.5-medium-gguf>. Download with:
//!
//! ```text
//! huggingface-cli download Miwa-Keita/zenz-v2.5-medium-gguf \
//!     --local-dir $HOME/.cache/kotoha/models
//! export KOTOHA_ZENZ_MODEL_PATH=$HOME/.cache/kotoha/models/<gguf-filename>
//! cargo test -p kotoha-core --features zenz-smoke --test kanji_zenz_smoke
//! ```
//!
//! Spec: `docs/superpowers/specs/2026-04-24-kotoha-phase-1-design.md` §3.2,
//! §8.3, §8.6.

#![cfg(feature = "zenz-smoke")]

use std::path::PathBuf;

use kotoha_core::kanji::{load_backend, BackendConfig, ConvertOptions};

/// Returns the Zenz model path from `KOTOHA_ZENZ_MODEL_PATH`, or `None` with
/// a SKIP message on stdout if the env var is unset.
fn get_model_path_or_skip() -> Option<PathBuf> {
    match std::env::var("KOTOHA_ZENZ_MODEL_PATH") {
        Ok(p) => Some(PathBuf::from(p)),
        Err(_) => {
            println!("SKIPPED: KOTOHA_ZENZ_MODEL_PATH not set");
            None
        }
    }
}

/// Reads `tests/fixtures/kanji_smoke.tsv` and returns `(input, expected_substring)` pairs.
fn load_fixture() -> Vec<(String, String)> {
    let tsv = include_str!("fixtures/kanji_smoke.tsv");
    tsv.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            let mut parts = line.splitn(2, '\t');
            let input = parts.next().expect("fixture line missing input").to_string();
            let expected = parts
                .next()
                .expect("fixture line missing expected_substring")
                .to_string();
            (input, expected)
        })
        .collect()
}

/// Asserts a single fixture row against the Zenz backend loaded from `model_path`.
fn run_fixture(model_path: PathBuf, row_index: usize) {
    let fixtures = load_fixture();
    assert!(
        row_index < fixtures.len(),
        "fixture index {row_index} is out of range (fixture has {} rows)",
        fixtures.len()
    );
    let (input, expected_substring) = &fixtures[row_index];

    let backend = load_backend(&BackendConfig::Zenz {
        model_path: model_path.clone(),
    })
    .expect("Zenz backend must load from the pinned fixture path");

    let mut options = ConvertOptions::default();
    options.top_k = 5;
    // temperature / seed stay at defaults (0.0 / Some(0)) for deterministic output.

    let result = backend
        .convert(input, &options)
        .unwrap_or_else(|e| panic!("convert must succeed for input {input:?}: {e}"));

    assert!(
        !result.is_empty(),
        "convert must return at least one candidate for input {input:?}"
    );
    assert!(
        result[0].surface.contains(expected_substring.as_str()),
        "top-1 candidate {:?} must contain expected substring {:?} for input {:?}",
        result[0].surface,
        expected_substring,
        input
    );
}

#[test]
fn zenz_smoke_1_nihongo() {
    let Some(path) = get_model_path_or_skip() else {
        return;
    };
    run_fixture(path, 0);
}

#[test]
fn zenz_smoke_2_kanji() {
    let Some(path) = get_model_path_or_skip() else {
        return;
    };
    run_fixture(path, 1);
}

#[test]
fn zenz_smoke_3_ashita() {
    let Some(path) = get_model_path_or_skip() else {
        return;
    };
    run_fixture(path, 2);
}

#[test]
fn zenz_smoke_4_yamada_san() {
    let Some(path) = get_model_path_or_skip() else {
        return;
    };
    run_fixture(path, 3);
}

#[test]
fn zenz_smoke_5_kotoba() {
    let Some(path) = get_model_path_or_skip() else {
        return;
    };
    run_fixture(path, 4);
}
```

- [ ] **Step 4: `zenz-smoke` feature で compile 確認 (env var なし)**

Run:

```bash
cargo test -p kotoha-core --features zenz-smoke --test kanji_zenz_smoke 2>&1 | tail -30
```

Expected (env var 未設定):

- 5 tests が走り、各 test の stdout に `SKIPPED: KOTOHA_ZENZ_MODEL_PATH not set` が出る
- 5 tests すべて PASS (早期 return するため)
- exit code 0

- [ ] **Step 5: default features で integration test が compile skip されることを確認**

Run:

```bash
cargo test -p kotoha-core --test kanji_zenz_smoke 2>&1 | tail -10
```

Expected: file-level `#![cfg(feature = "zenz-smoke")]` により default features では 0 test が登録される (compile 自体は通り `running 0 tests` が出力されるか、crate 全体として compile skip される)。どちらでも test suite は PASS で終わる。

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
git add crates/kotoha-core/tests/fixtures/kanji_smoke.tsv crates/kotoha-core/tests/kanji_zenz_smoke.rs
git commit -m "test(kanji): add Layer 3 smoke test skeleton + fixture TSV"
```

Expected: `2 files changed`。

---

## Task P1-2-9: Layer 3 empirical verification (model 必要)

**Files:** (fixture TSV を Zenz 実出力に合わせて微調整する可能性あり)

`[verification]` 本 Task は実 Zenz-v2.5-medium GGUF を必要とする。実装者 (または user) が Miwa-Keita/zenz-v2.5-medium-gguf を手元に download し、`KOTOHA_ZENZ_MODEL_PATH` を設定した環境で走らせる。

- [ ] **Step 1: Zenz-v2.5-medium GGUF を download**

Run:

```bash
# Method A: huggingface-cli (推奨、要 huggingface-cli)
huggingface-cli download Miwa-Keita/zenz-v2.5-medium-gguf --local-dir $HOME/.cache/kotoha/models

# Method B: 直接 curl (file 名は HuggingFace page で確認した最新のものに置換)
# curl -L -o $HOME/.cache/kotoha/models/zenz-v2.5-medium.Q5_K_M.gguf \
#   "https://huggingface.co/Miwa-Keita/zenz-v2.5-medium-gguf/resolve/main/<filename>.gguf"
```

Expected: `.gguf` file が `$HOME/.cache/kotoha/models/` に配置される。file size は 100〜200MB 程度 (medium = GGUF Q5_K_M 量子化想定)。

- [ ] **Step 2: `KOTOHA_ZENZ_MODEL_PATH` を export**

Run (実 file 名に置換):

```bash
export KOTOHA_ZENZ_MODEL_PATH=$HOME/.cache/kotoha/models/zenz-v2.5-medium.Q5_K_M.gguf
ls -la "$KOTOHA_ZENZ_MODEL_PATH"
```

Expected: file が存在し、サイズが妥当 (100MB 以上)。

- [ ] **Step 3: Layer 3 smoke を実行**

Run (timeout 10 分、初回 cold start があるため):

```bash
cargo test -p kotoha-core --features zenz-smoke --test kanji_zenz_smoke -- --nocapture
```

Expected: 5 tests PASS。`--nocapture` で各 test の stdout (SKIP message 無し、model load log があれば) を確認する。

- [ ] **Step 4: fixture 微調整 (必要なら)**

5 tests のうち assertion fail するものがあれば:

1. `expected_substring` が Zenz model の実出力に含まれていない → fixture TSV の `expected_substring` 列を実出力の部分文字列に更新する (spec §8.6 の regenerate 手順)
2. `KanjiError::ModelLoadFailed` が返る → `KOTOHA_ZENZ_MODEL_PATH` と GGUF file の integrity を再確認
3. `ModelNotFound` が返る → path が正しいか確認

fixture を更新した場合は commit:

```bash
git add crates/kotoha-core/tests/fixtures/kanji_smoke.tsv
git commit -m "test(kanji): adjust Layer 3 fixture for actual Zenz-v2.5-medium output"
```

Expected: fixture 調整後に 5 tests すべて PASS。

- [ ] **Step 5: cold start + warm cache latency 測定**

Run (inline script で 2 回 load してそれぞれの duration を測定):

```bash
cat > /tmp/kotoha-p1-2-latency.sh <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
echo "--- cold start (1st run) ---"
time cargo test -p kotoha-core --features zenz-smoke --test kanji_zenz_smoke zenz_smoke_1_nihongo -- --nocapture 2>&1 | tail -5

echo "--- warm cache (2nd run) ---"
time cargo test -p kotoha-core --features zenz-smoke --test kanji_zenz_smoke zenz_smoke_1_nihongo -- --nocapture 2>&1 | tail -5
EOF
chmod +x /tmp/kotoha-p1-2-latency.sh
/tmp/kotoha-p1-2-latency.sh
```

Expected: `time` command の `real` 値を 2 回分記録する。cold は 10〜30 秒、warm は cargo build cache hit + OS page cache で短縮されるはず (想定 5〜15 秒)。数値を scratch file (`docs/wbs/p1-2-prompt-format-analysis.md`) に追記する。

- [ ] **Step 6: scratch file に measurement を追記**

Edit `docs/wbs/p1-2-prompt-format-analysis.md`: 末尾に section 3 を追加する。

```markdown

## 3. Cold start / warm cache latency 測定

- 測定日: 2026-04-XX
- 環境: <OS / CPU / RAM の簡易 spec>
- model: Zenz-v2.5-medium (Q5_K_M, <filesize> MB)

| 試行 | 種別 | real time | user time | sys time |
|---|---|---|---|---|
| 1 | cold start (cargo build + model load + inference) | XXs | XXs | XXs |
| 2 | warm cache (cargo cache hit + OS page cache) | XXs | XXs | XXs |

所見:
- cold と warm の差が Phase 1 acceptance (§8.3 の「10〜30 秒」目安) 内に収まる
- spec §10 Risk #4 (CPU inference latency) と比較した結果: OK / NG
```

(commit は Task P1-2-13 の WBS 統合時点で行う。)

---

## Task P1-2-10: Multi-feature verification gate

**Files:** (検証のみ、commit なし)

`[verification]` P1-1 の Task P1-1-11 と同じ 5 feature 構成 verification gate を走らせる。

- [ ] **Step 1: 6 通りの feature 組合わせで cargo check**

Run:

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

Expected: 下表の `Registered tests` 列に従う。default + mock-backend の両 run はいずれも zenz.rs の `#[cfg(feature = "zenz")]` gate が外れないため、zenz 系 unit test は 0 件登録。

| Feature flags | Registered tests | Notes |
|---|---|---|
| (default, no features) | 160 | Layer 2/3 と MockBackend/ZenzBackend の unit tests は cfg-skip (compile 単位で未登録) |
| `--features mock-backend` | 171 (+5 Layer 2 + 6 MockBackend unit) | |
| `--features zenz` | 162 (+2 ZenzBackend unit: `zenz_load_errors_on_missing_file` (P1-2-5) + `zenz_model_id_is_zenz_prefix` (P1-2-6、note-only の placement は Layer 3 に委譲)) | Layer 3 は cfg-skip (`zenz-smoke` 必須) |
| `--features zenz-smoke` | 167 (+5 Layer 3; `zenz-smoke` は `zenz` を含意) | `KOTOHA_ZENZ_MODEL_PATH` 未設定時: Layer 3 全 5 件は `println!("SKIPPED: ...")` + early-return する。cargo test の判定上は全 5 件が PASS (test_result = ok)。env var 設定時: Layer 3 全 5 件が real model inference を実行する |
| `--all-features` | 178 | 上記すべての union。Layer 3 の SKIP vs run 挙動は `KOTOHA_ZENZ_MODEL_PATH` に従う (上記と同じ) |

verbatim head + tail を report に含める。

- [ ] **Step 5: `zenz-smoke` feature で test (env var ある場合のみ)**

Run (model path が設定済みの場合):

```bash
cargo test --workspace --features zenz-smoke
```

Expected:
- `KOTOHA_ZENZ_MODEL_PATH` 設定済み: 167 件全 PASS (うち Layer 3 5 件は real inference)
- `KOTOHA_ZENZ_MODEL_PATH` 未設定: 167 件全 PASS (うち Layer 3 5 件は SKIP println + early-return、test_result = ok で PASS 扱い)

- [ ] **Step 6: lefthook pre-push trigger**

Run:

```bash
lefthook run pre-push
```

Expected: manifest-check / build / clippy / test 全て PASS。default features で走るので Layer 3 は含まれない。lefthook pre-push の出力 head + tail を report に含める。

- [ ] **Step 7: 検証のみ (commit 不要)**

Run:

```bash
git status
```

Expected: `nothing to commit, working tree clean` (Task P1-2-9 Step 4 で fixture 調整 commit を行なった場合以外)。

---

## Task P1-2-11: commit history 確認 + push + PR 作成

**Files:** (これまでの commit を push + PR 作成)

`[git]` push + PR。lefthook pre-push が自動実行される。

- [ ] **Step 1: commit history を確認**

Run:

```bash
git log --oneline develop..HEAD
```

Expected: 5〜7 commits (Task P1-2-3 / P1-2-4 / P1-2-5 / P1-2-6 / P1-2-7 / P1-2-8 が各 1 commit + 任意で P1-2-9 の fixture 調整)。

- [ ] **Step 2: branch を push**

Run:

```bash
git push -u origin feature/<IMPL_ISSUE>-zenz-backend-layer3-smoke
```

Expected: branch が remote に push され、lefthook pre-push が自動実行 + PASS。

- [ ] **Step 3: PR 作成**

Run (`<IMPL_ISSUE>` は Task P1-2-0 で取得した値、`<PINNED_VERSION>` は Task P1-2-2 で確定した値):

```bash
gh pr create --base develop \
  --title "feat(kanji): implement ZenzBackend via llama-cpp-2 + Layer 3 smoke (#<IMPL_ISSUE>)" \
  --body "$(cat <<'EOF'
## Summary

Phase 1 milestone P1-2: replace the P1-1 ZenzBackend skeleton with a real implementation backed by llama-cpp-2 (version pinned via Q1). Add Layer 3 Zenz smoke integration tests (5 cases, KOTOHA_ZENZ_MODEL_PATH env-var gated) and the shared fixture TSV (reused by Layer 4 in P1-3).

Closes #<IMPL_ISSUE>.

## Changes

- `Cargo.toml` (workspace root): add `llama-cpp-2 = "<PINNED_VERSION>"` to `[workspace.dependencies]` (version pinned against Context7 / crates.io latest stable; alternative comparison recorded in WBS section 2).
- `crates/kotoha-core/Cargo.toml`: add `llama-cpp-2 = { workspace = true, optional = true }`; rewrite the `zenz` feature from `[]` to `["dep:llama-cpp-2"]`.
- `crates/kotoha-core/src/kanji/zenz.rs` (~400 LOC): rewrite from the P1-1 skeleton to a real implementation.
  - `ZenzBackend` now holds a `LlamaModel` loaded from disk.
  - `load` returns `KanjiError::ModelNotFound` for missing files and `KanjiError::ModelLoadFailed` for llama-cpp-2 parse / init errors.
  - `model_id` returns `"zenz-v2.5-medium"` (dynamic metadata lookup is tracked for Phase 2).
  - `convert` runs `validate_input` → `build_prompt` → llama-cpp-2 inference → `score_sort_dedupe` so the trait contract (§5.3 / §5.6 / §5.7) is enforced uniformly.
  - `build_prompt` / `infer` are extracted helpers isolating llama-cpp-2 from the rest of the module.
  - Unit tests: `zenz_load_errors_on_missing_file` (replaces the P1-1 skeleton test) + `zenz_model_id_is_zenz_prefix`. No `build_prompt` unit test is added in P1-2 — `build_prompt`'s behavior is verified end-to-end via Layer 3 smoke in P1-2-8 (see P1-2-7 Step 3 note).
- `crates/kotoha-core/tests/kanji_zenz_smoke.rs` (new, ~150 LOC): Layer 3 smoke suite (5 tests, file-level `#![cfg(feature = "zenz-smoke")]`). Each test short-circuits with `SKIPPED: ...` on stdout when `KOTOHA_ZENZ_MODEL_PATH` is unset, so the suite never fails a CI run that lacks the model.
- `crates/kotoha-core/tests/fixtures/kanji_smoke.tsv` (new): 5 rows of `input_hiragana<TAB>expected_substring` shared with Layer 4 (P1-3).

## Verification

- `cargo check -p kotoha-core` passes under all 6 feature configurations (no-default-features / default / mock-backend / zenz / zenz-smoke / all-features).
- `cargo test --workspace` PASS. Registered test counts per feature combo:
  - (default, no features): 160
  - `--features mock-backend`: 171 (+5 Layer 2 + 6 MockBackend unit)
  - `--features zenz`: 162 (+2 ZenzBackend unit; Layer 3 is cfg-skipped — needs `zenz-smoke`)
  - `--features zenz-smoke`: 167 (+5 Layer 3; `zenz-smoke` implies `zenz`)
  - `--all-features`: 178 (union of all of the above)
- `cargo test --workspace --features zenz-smoke` with `KOTOHA_ZENZ_MODEL_PATH` set: all 5 Layer 3 tests run real Zenz inference and PASS. Without the env var: all 5 Layer 3 tests print `SKIPPED: ...` and early-return; they still count as PASS (test_result = ok) so the 167-test total is unchanged. The same SKIP-vs-run rule applies under `--all-features`.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` reports zero warnings.
- `cargo fmt --all --check` produces no diff.
- lefthook pre-push PASS locally.

## Context

- Spec: `docs/superpowers/specs/2026-04-24-kotoha-phase-1-design.md` §3.1 / §3.2 / §3.3 / §5.3 / §6 / §8.3 / §8.6 / §10 / §13
- Phase 1 overall plan: `docs/superpowers/plans/2026-04-24-kotoha-phase-1-implementation.md` §"PR #3 — P1-2"
- Detailed plan: `docs/superpowers/plans/2026-04-24-kotoha-phase-1-p1-2.md`
- P1-1 carry-over: `docs/wbs/2026-04-24-feature-65-kanji-skeleton-mockbackend.md` "P1-2 への申し送り"
- Open Questions resolved in this PR: Q1 (llama-cpp-2 version pin: `<PINNED_VERSION>`), Q2 (Zenz prompt template, recorded in WBS "prompt format 解析ログ"), Q5 (`seed=0` deterministic behavior, confirmed via Layer 3 smoke).
- ADR 0006 (`#[non_exhaustive]` policy): no new public types introduced; existing `KanjiError` / `BackendConfig` / `Candidate` / `ConvertOptions` remain compliant.

## Test plan

- [ ] `cargo check` clean under all 6 feature configurations
- [ ] `cargo test --workspace` passes on a CI-equivalent local run (default features)
- [ ] `cargo test --workspace --features mock-backend` passes (Layer 1 + Layer 2 unchanged)
- [ ] `cargo test --workspace --features zenz-smoke` passes with `KOTOHA_ZENZ_MODEL_PATH` pointing at Zenz-v2.5-medium GGUF (Layer 3 empirical verification)
- [ ] `cargo test --workspace --features zenz-smoke` passes without `KOTOHA_ZENZ_MODEL_PATH` (SKIP path)
- [ ] `cargo clippy --workspace --all-targets --all-features -- -D warnings` clean
- [ ] WBS entry includes prompt format analysis + cold-start / warm-cache latency measurements
- [ ] P1-1 skeleton test `zenz_load_returns_backend_error_in_p1_1_skeleton` is gone; replaced by `zenz_load_errors_on_missing_file`
- [ ] `#[allow(dead_code)]` / `#[allow(unused_variables)]` removed from `zenz.rs`

🤖 Generated with [Claude Code](https://claude.com/claude-code)
EOF
)"
```

Expected: PR が作成され、URL が表示される。`<IMPL_ISSUE>` / `<PINNED_VERSION>` を実値に置換した状態で body が載ること。

---

## Task P1-2-12: Medium-tier review + findings 解消 + squash-merge

**Files:** (変更なし、review + merge のみ)

`[review-merge]` CLAUDE.md PR Review Matrix に従い、本 PR は Medium tier (5 files、約 600 LOC — Small の 100 lines 目安を大幅に超過) として扱う。

- [ ] **Step 1: Medium-tier review を走らせる**

CLAUDE.md の PR Review Matrix に従い、Medium tier required: `agent-teams:team-review` 全 5 dimensions (security / performance / architecture / testing / accessibility)。本 PR は UI 無しなので accessibility dimension は skip または trivial PASS 扱い。追加で `owasp-security` + `secrets-check`。

Run (skill tool 経由):

```
/agent-teams:team-review dimensions=security,performance,architecture,testing
/owasp-security
/secrets-check
```

Expected: 合格 (Critical / High ゼロが理想)。

- [ ] **Step 2: review findings を解消**

Critical / High findings があれば commit 追加で解消する。解消後に再 review し、ゼロになるまで繰り返す。典型的な findings 候補:

- Security: llama-cpp-2 の untrusted model path 読み込み (model_path が user-controlled の場合の path traversal) — `std::fs::canonicalize` + root dir check で緩和を検討
- Performance: `LlamaBackend::init()` を毎回 `load` 内で呼ぶと overhead がある場合、`OnceLock` で process-wide singleton 化
- Architecture: `build_prompt` / `infer` helper の可視性 (pub vs pub(crate) vs private) を見直し
- Testing: Layer 3 が env-var 未設定時に本当に PASS するかを CI で確認する代替手段を検討 (例: `CARGO_CFG_...` を使った部分分岐)

- [ ] **Step 3: squash merge + branch 削除**

Run (`<PR_NUMBER>` は Task P1-2-11 で取得した PR 番号):

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

Expected: 先頭に P1-2 squash-merge commit が来ている。

---

## Task P1-2-13: WBS ログ作成 + develop 直接 push

**Files:**
- Create: `docs/wbs/2026-04-24-feature-<IMPL_ISSUE>-zenz-backend-layer3-smoke.md`
- Delete: `docs/wbs/p1-2-prompt-format-analysis.md` (scratch file、WBS 統合後に削除)

`[wbs]` project CLAUDE.md「WBS 直接 push の例外」に従い、本 file は develop branch に直接 commit + push する。Task P1-2-1 / P1-2-2 / P1-2-9 で scratch file に蓄積した結果を WBS 本体に統合する。

- [ ] **Step 1: scratch file の内容を読む**

Read: `docs/wbs/p1-2-prompt-format-analysis.md`

Expected: section 1 (AzooKey docs 解析)、section 2 (llama-cpp-2 version pin)、section 3 (cold start latency) の 3 section が揃っている。

- [ ] **Step 2: WBS ログを Write で作成**

Write `docs/wbs/2026-04-24-feature-<IMPL_ISSUE>-zenz-backend-layer3-smoke.md` (`<IMPL_ISSUE>` / `<PR_NUMBER>` / `<MERGE_COMMIT>` / `<PINNED_VERSION>` を実値に置換):

Content:

```markdown
---
milestone: P1-2
branch: feature/<IMPL_ISSUE>-zenz-backend-layer3-smoke
pr: "#<PR_NUMBER>"
merge_commit: "<MERGE_COMMIT>"
issue: "#<IMPL_ISSUE>"
status: done
started: 2026-04-XX
finished: 2026-04-XX
---

# P1-2: ZenzBackend via llama-cpp-2 + Layer 3 smoke

## 実施内容

- `Cargo.toml` (workspace root) に `llama-cpp-2 = "<PINNED_VERSION>"` を追加。version は Context7 / crates.io 最新 stable と Kotoha MSRV 1.80 の互換性を確認した上で pin (Open Question Q1 解決)。
- `crates/kotoha-core/Cargo.toml` に `llama-cpp-2 = { workspace = true, optional = true }` を追加。`zenz` feature を `[]` から `["dep:llama-cpp-2"]` に書き換え。
- `crates/kotoha-core/src/kanji/zenz.rs` (~400 LOC) を P1-1 skeleton から本実装に置換:
  - struct: `_placeholder: ()` を削除し、`model: LlamaModel` + `model_path: PathBuf` の 2 field に。
  - `load`: `ModelNotFound` / `ModelLoadFailed` を明確に区別。llama-cpp-2 `LlamaModel::load_from_file` の呼び出しを `load_llama_model` helper fn に isolate。
  - `model_id`: `"zenz-v2.5-medium"` を返す (spec §3.2 の default に整合)。将来 Phase 2 で model metadata からの dynamic lookup に切り替え予定。
  - `convert`: `validate_input` → `build_prompt` → `infer` → `score_sort_dedupe` の 4 段 pipe 構成。spec §5.6 / §5.7 の契約を既存 helper で一元化。
  - `build_prompt` / `infer` を file 内 helper fn として extract、llama-cpp-2 type が file 外に漏れないようにする。
  - P1-1 の `#[allow(dead_code)]` / `#[allow(unused_variables)]` / `#[derive(Debug)]` はすべて削除 (field / 引数が本実装で使われるため不要)。
  - unit test を 1 件削除 (`zenz_load_returns_backend_error_in_p1_1_skeleton`) + 2 件追加 (`zenz_load_errors_on_missing_file` / `zenz_model_id_is_zenz_prefix` の placement は Layer 3 側で保持される設計。`build_prompt` の behavioral verification は Layer 3 smoke 経由で行う方針。詳細は P1-2-7 Step 3 を参照)。
- `crates/kotoha-core/tests/kanji_zenz_smoke.rs` (~150 LOC) を新設: Layer 3 smoke 5 件。file-level `#![cfg(feature = "zenz-smoke")]` で gate。`KOTOHA_ZENZ_MODEL_PATH` 未設定時は `println!("SKIPPED: ...")` + early return で skip。設定済み時は `load_backend(&BackendConfig::Zenz { model_path })` で backend を取得し、fixture 5 件の `expected_substring` が top-1 `surface` に含まれることを assertion。
- `crates/kotoha-core/tests/fixtures/kanji_smoke.tsv` を新設: 5 行 (にほんご / かんじ / あした / やまださん / ことば)、TAB 区切りの `input_hiragana<TAB>expected_substring` 形式。Layer 4 (P1-3 で `scripts/phase1-smoke.sh`) と parity を保つ。

## テスト件数 (P1-1 baseline 160 default / 171 mock-backend からの差分)

| Feature flags | Registered tests | Notes |
|---|---|---|
| (default, no features) | 160 | Layer 2/3 と MockBackend/ZenzBackend unit tests は cfg-skip |
| `--features mock-backend` | 171 (+5 Layer 2 + 6 MockBackend unit) | mock-backend は zenz feature を含まない |
| `--features zenz` | 162 (+2 ZenzBackend unit: `zenz_load_errors_on_missing_file` + `zenz_model_id_is_zenz_prefix`) | Layer 3 は cfg-skip (`zenz-smoke` 必須) |
| `--features zenz-smoke` | 167 (+5 Layer 3; `zenz-smoke` は `zenz` を implies) | env var 未設定時: Layer 3 全 5 件は `println!("SKIPPED: ...")` + early-return し、cargo test 上は PASS 扱い (test_result = ok)。env var 設定時: 5 件とも real inference を実行 |
| `--all-features` | 178 | 上記 union。Layer 3 の SKIP vs run 挙動は `KOTOHA_ZENZ_MODEL_PATH` に従う (上記と同じ) |

## Prompt format 解析ログ

(scratch file `docs/wbs/p1-2-prompt-format-analysis.md` の section 1 を統合)

### AzooKey Zenzai docs 通読結果 (出典: <https://github.com/azooKey/AzooKeyKanaKanjiConverter/blob/main/Docs/zenzai.md>)

- **BOS / EOS / separator token**: <解析結果>
- **Prompt template** (例: 入力 "にほんご"): <解析結果>
- **Tokenize 方針**: <character-level / BPE の判定結果>
- **Decoding stop condition**: <EOS 検出 / max new tokens 推奨値>
- **Top-K / top-P / temperature 推奨**: <値>
- **Score aggregation**: <sum of log-prob / length-normalized / 他>

### 実装で empirical に確定した項目

- <AzooKey docs に明記されなかった項目 N>: <実装で確定した挙動>

### Kotoha 実装が AzooKey と差分を持つ点

- <差分 1 とその理由>

## Cold start / warm cache latency 測定

(scratch file `docs/wbs/p1-2-prompt-format-analysis.md` の section 3 を統合)

- 測定日: 2026-04-XX
- 環境: <OS / CPU / RAM>
- model: Zenz-v2.5-medium (Q5_K_M, <filesize> MB)

| 試行 | 種別 | real time | user time | sys time |
|---|---|---|---|---|
| 1 | cold start | XXs | XXs | XXs |
| 2 | warm cache | XXs | XXs | XXs |

spec §10 Risk #4 (CPU inference latency) との比較: <OK / NG 判定と理由>

## llama-cpp-2 version pin rationale

(scratch file `docs/wbs/p1-2-prompt-format-analysis.md` の section 2 を統合)

- 確認日: 2026-04-XX
- 確認手段: Context7 MCP / crates.io WebFetch
- 採用 version: `<PINNED_VERSION>`
- MSRV 互換: Kotoha 1.80 と互換
- 有効 feature flags: <default のみ / cuda / metal>
- Alternative 比較 (spec §3.1 の追認):

| 候補 | 最新 version / release 日 | 再評価結果 |
|---|---|---|
| llama_cpp-rs | <...> | <spec §3.1 と変わらず / 変化あり> |
| mistral.rs | <...> | <...> |
| Candle | <...> | <...> |

ADR 0010 (P1-4 作成予定) で同じ内容を expanded form で記録する。

## つまずき

(実施時に記入。例として想定される項目:

1. llama-cpp-2 API の `LlamaBackend::init()` singleton 化忘れ → `OnceLock` で解消
2. `LlamaModel::load_from_file` の error type が Result<_, LlamaModelError> で、`Box<dyn Error + Send + Sync>` へ変換時に `Send + Sync` bound が足りず compile error → `Box::new(e) as Box<_>` で明示 cast
3. Zenz prompt format が AzooKey docs に書かれている通りでは EOS token が検出されない → 実機検証で BOS/EOS token id を GGUF metadata から取り直し
4. `KOTOHA_ZENZ_MODEL_PATH` を `std::env::var` で取るとき、path に space が含まれる環境で parse 失敗 → 引用符つき path を正規化

actual 発生分を記録する。)

## Regression 検証

- `cargo test --workspace` (default): baseline 160 PASS、変化なし (zenz.rs は feature gate で compile skip)。
- `cargo test --workspace --features mock-backend`: baseline 171 PASS、変化なし。
- `cargo test --workspace --features zenz`: 162 PASS (160 + zenz.rs の 2 unit tests: `zenz_load_errors_on_missing_file` + `zenz_model_id_is_zenz_prefix`)。
- `cargo test --workspace --features zenz-smoke` (model 配置済み): 167 PASS (162 + Layer 3 5 件が model inference を実行)。
- `cargo test --workspace --features zenz-smoke` (model 未配置): 167 件登録 + Layer 3 5 件は SKIP println 出しつつ PASS (test_result = ok)、exit code 0。
- `cargo test --workspace --all-features`: 178 PASS (160 + mock 6 + Layer 2 5 + zenz 2 + Layer 3 5)。model 未配置時の Layer 3 5 件の挙動は `--features zenz-smoke` と同じ SKIP 経路。
- `cargo check -p kotoha-core` を 6 通りの feature 組合せ (no-default / default / mock-backend / zenz / zenz-smoke / all-features) で走らせ、すべて PASS。
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` warnings ゼロ。
- `cargo fmt --all --check` diff ゼロ。
- lefthook pre-push (manifest-check / build / clippy / test) PASS。

## Review 結果

- `secrets-check`: CLEAN (新規の credential / token / .env 参照なし)。
- `agent-teams:team-review` (security): PASS、主要 findings は <... 実施時の結果を記入 ...>。
- `agent-teams:team-review` (performance): PASS、主要 findings は <...>。
- `agent-teams:team-review` (architecture): PASS、主要 findings は <...>。
- `agent-teams:team-review` (testing): PASS、主要 findings は <...>。
- `owasp-security`: PASS、<...>。

## P1-3 への申し送り

- `kotoha-cli::bin::kanji.rs` (CLI `kotoha-kanji`) は本 PR で固定した `ZenzBackend::load(model_path)` + `convert(input, &options)` を消費する。CLI options (spec §7.1) は `--model <path>` → `BackendConfig::Zenz { model_path }` に直結し、`--top-k` / `--temperature` / `--seed` は `ConvertOptions` へ map する。
- `process_line` 純粋関数 (spec §7.5) の実装で `KanjiBackend` trait object を DI する設計は、本 PR で `Box<dyn KanjiBackend>` が `load_backend` から取れることにより成立済み。P1-3 ではそのまま流用すること。
- `scripts/phase1-smoke.sh` は `crates/kotoha-core/tests/fixtures/kanji_smoke.tsv` を reuse すること (Layer 3 / Layer 4 parity、spec §8.4)。TSV の `expected_substring` を Layer 3 で調整した値をそのまま Layer 4 でも使う。
- `kotoha-cli/Cargo.toml` は `kotoha-core` を `{ workspace = true, features = ["zenz"] }` で参照する (spec §4.3)。

## P1-4 への申し送り

- ADR 0009 (Zenz model version policy): 本 PR で採用した `Miwa-Keita/zenz-v2.5-medium-gguf` を default として明記、`KOTOHA_ZENZ_MODEL_PATH` の意味、spec §8.6 の regenerate 手順を引用する。
- ADR 0010 (Kanji backend trait design): 本 PR の「llama-cpp-2 version pin rationale」 section をベースに、alternative 比較 (llama_cpp-rs / mistral.rs / Candle) と採用理由を ADR 化する。本 PR の `<PINNED_VERSION>` を ADR にも記録する。
- ADR 0011 (Feature flag design for Zenz): 本 PR で確定した 4 flags (default / mock-backend / zenz / zenz-smoke) の分離理由、lefthook pre-push との整合を ADR 化する。
- README: 本 PR の `crates/kotoha-core/tests/kanji_zenz_smoke.rs` の `//! Model setup` section をベースに、Zenz-v2.5-medium の入手手順 (`huggingface-cli download Miwa-Keita/zenz-v2.5-medium-gguf ...`) と `KOTOHA_ZENZ_MODEL_PATH` の使い方を README に移植する。
- Open Question Q3 (`process_line` の公開場所) は P1-3 で決める。
- Open Question Q4 (MockBackend fixture hard-code vs TSV 外出し) は P1-1 で解決済み (hard-code 採用)。

## 成果物リンク

- PR: https://github.com/std-koh-hinooka/kotoha-ime/pull/<PR_NUMBER>
- ISSUE: https://github.com/std-koh-hinooka/kotoha-ime/issues/<IMPL_ISSUE>
- Merge commit: <MERGE_COMMIT>
- Plan PR (本 plan 本体): https://github.com/std-koh-hinooka/kotoha-ime/pull/<PLAN_PR> (#67)
- Spec: `docs/superpowers/specs/2026-04-24-kotoha-phase-1-design.md`
- Overall plan: `docs/superpowers/plans/2026-04-24-kotoha-phase-1-implementation.md`
- Detailed plan: `docs/superpowers/plans/2026-04-24-kotoha-phase-1-p1-2.md`
- AzooKey Zenzai docs (本 PR の prompt format 1 次情報源): <https://github.com/azooKey/AzooKeyKanaKanjiConverter/blob/main/Docs/zenzai.md>
```

- [ ] **Step 3: scratch file を削除**

Run:

```bash
rm docs/wbs/p1-2-prompt-format-analysis.md
```

Expected: scratch file が削除される。

- [ ] **Step 4: develop に直接 commit + push**

Run:

```bash
git add docs/wbs/2026-04-24-feature-<IMPL_ISSUE>-zenz-backend-layer3-smoke.md
git commit -m "docs(wbs): P1-2 ZenzBackend + Layer 3 smoke — implementation log (#<IMPL_ISSUE>, PR #<PR_NUMBER>)"
git push
```

Expected: lefthook pre-push が doc-naming check を通過、他の hook は docs-only change なので skip または PASS。

- [ ] **Step 5: 実装 ISSUE を close**

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

## P1-2 完了条件チェックリスト

以下すべてを実施完了時点で P1-2 完了。

- [ ] 実装 ISSUE `#<IMPL_ISSUE>` が close 済み
- [ ] PR `#<PR_NUMBER>` が develop に squash-merge 済み
- [ ] `feature/<IMPL_ISSUE>-zenz-backend-layer3-smoke` branch が remote + local ともに削除済み
- [ ] `llama-cpp-2` version `<PINNED_VERSION>` が `Cargo.toml` (workspace) と WBS に記録済み (Q1 解決)
- [ ] `cargo check -p kotoha-core` が 6 通り (no-default / default / mock-backend / zenz / zenz-smoke / all-features) すべてで PASS
- [ ] `cargo test --workspace` が PASS (default features 160)
- [ ] `cargo test --workspace --features mock-backend` が PASS (171)
- [ ] `cargo test --workspace --features zenz` が PASS (162)
- [ ] `cargo test --workspace --features zenz-smoke` (model 配置済み) が PASS (167 Layer 3 実行)
- [ ] `cargo test --workspace --features zenz-smoke` (model 未配置) が PASS (167 Layer 3 SKIP、exit 0)
- [ ] `cargo test --workspace --all-features` が PASS (model 配置済み: 178、model 未配置: 178 件で Layer 3 5 件が SKIP path + PASS)
- [ ] `cargo clippy --workspace --all-targets --all-features -- -D warnings` warnings ゼロ
- [ ] `cargo fmt --all --check` diff ゼロ
- [ ] lefthook pre-push 全コマンド PASS (build / clippy / test 実行 + 成功)
- [ ] `crates/kotoha-core/src/kanji/zenz.rs` に `todo!()` / P1-1 の `Err(KanjiError::Backend { reason: "... P1-2" })` skeleton が残っていない
- [ ] `crates/kotoha-core/src/kanji/zenz.rs` から `#[allow(dead_code)]` / `#[allow(unused_variables)]` が削除済み
- [ ] `crates/kotoha-core/tests/kanji_zenz_smoke.rs` が 5 test 含み `--features zenz-smoke` で PASS
- [ ] `crates/kotoha-core/tests/fixtures/kanji_smoke.tsv` が 5 行 (TAB 区切り) で存在
- [ ] `zenz_load_returns_backend_error_in_p1_1_skeleton` test が削除されている (新 test `zenz_load_errors_on_missing_file` に置換済み)
- [ ] `docs/wbs/2026-04-24-feature-<IMPL_ISSUE>-zenz-backend-layer3-smoke.md` が develop に push 済み
- [ ] WBS に「prompt format 解析ログ」/「cold start latency 測定」/「llama-cpp-2 version pin rationale」の 3 section が埋められている
- [ ] Scratch file `docs/wbs/p1-2-prompt-format-analysis.md` が削除済み (WBS 統合後)

---

## Spec Coverage 確認

本 plan が spec §3 / §5.3 / §5.4 / §5.7 / §6 / §8.3 / §8.6 / §10 / §13 のどの要件をどの Task で実装するかのマッピング。

| Spec § | 要件 | 実装 Task |
|---|---|---|
| §3.1 llama-cpp-2 | Rust crate 採用 + version pin + alternative 比較 | P1-2-2 (research), P1-2-3 (Cargo.toml) |
| §3.2 Zenz model | Zenz-v2.5-medium (`Miwa-Keita/zenz-v2.5-medium-gguf`) を default、GGUF 埋め込み tokenizer 活用、3 サイズ選択可能 | P1-2-6 (model_id), P1-2-8 (fixture + README-like doc), P1-2-9 (empirical) |
| §3.3 AzooKey Zenzai docs | 通読 + prompt format 解析ログを WBS に記録 | P1-2-1 (research), P1-2-7 (`build_prompt`), P1-2-13 (WBS) |
| §5.3 KanjiBackend trait | `convert` が `validate_input` + `score_sort_dedupe` を call し契約を一元化 | P1-2-7 (convert pipe) |
| §5.4 load_backend factory | P1-1 で既存、本 PR は `BackendConfig::Zenz { model_path }` から `ZenzBackend::load` を呼ぶ path を動作させる | P1-2-5 (load) |
| §5.7 Output 順序保証 + dedupe | `score_sort_dedupe` helper (P1-1 実装済み) を `convert` で呼ぶ | P1-2-7 |
| §6 Data flow | `kotoha-romaji | kotoha-kanji` の右半分、`ZenzBackend::convert` 内の pipeline (tokenize → generate → decode → sort/dedupe) | P1-2-7 |
| §8.3 Layer 3 Zenz smoke | `zenz-smoke` feature、5 件、env-var gate、`KOTOHA_ZENZ_MODEL_PATH` 未設定時 SKIP | P1-2-8 (skeleton), P1-2-9 (empirical + fixture 調整) |
| §8.6 Model 更新時 fixture regenerate | fixture TSV の `expected_substring` を model 実出力に合わせて調整する手順を本 PR で検証 | P1-2-9 |
| §10 Risk #2 (prompt format) | AzooKey docs 参照を mandatory + WBS に解析ログ記録 | P1-2-1, P1-2-13 |
| §10 Risk #4 (CPU latency) | cold + warm start 測定 + spec との比較 | P1-2-9, P1-2-13 |
| §13 Q1 (llama-cpp-2 version) | Context7 / crates.io 最新 stable を pin、ADR 0010 準備 | P1-2-2 |
| §13 Q2 (Zenz prompt exact form) | AzooKey docs + empirical で確定、WBS 記録 | P1-2-1, P1-2-7, P1-2-9 |
| §13 Q5 (--seed 0 deterministic) | Layer 3 smoke で deterministic 出力を間接検証 | P1-2-7 (sampler init), P1-2-9 (empirical) |

§5.1 / §5.2 / §5.5 / §5.6 / §8.1 / §8.2 は P1-1 で完了済み。§4 / §7 / §8.4 / §9 は P1-3 / P1-0 で完了済み。§11 / §12 / §14 / §15 / §16 は Phase 1 全体に対応するため本 PR では部分対応 (P1-4 で完結)。

---

## Self-Review 済み事項

本 plan の品質を担保するため、以下 10 項目を自己確認済み。

1. **プレースホルダの位置づけ**: 本文中の `<IMPL_ISSUE>` / `<PR_NUMBER>` / `<MERGE_COMMIT>` / `<PINNED_VERSION>` / `<PLAN_PR>` は意図した placeholder。実装着手時および実行中に実値へ置換する。それ以外に未解決の `TODO` / `TBD` / `fill in` は存在しない。`// RESEARCH-DEPENDENT` コメント付きの Rust code ブロック (P1-2-4 の import、P1-2-5 の `load_llama_model`、P1-2-7 の `infer`) は、Task P1-2-2 の結果に基づいて実装者が pinned llama-cpp-2 API の call sites に置き換える設計。研究結果なしに verbatim な Rust code を書くとアドホックな API 誤推測になるため、意図的に pseudocode + 明示マーカーで構造化している。

2. **Research task / implementation task の分離が明確**: `[research]` (P1-2-1, P1-2-2) は scratch file 更新のみで git commit しない。`[implementation-from-research]` (P1-2-4〜P1-2-7) は research 結果を消費して verbatim コードを書く。`[verbatim]` (P1-2-3, P1-2-8) は最初から verbatim で書ける (Cargo.toml 変更、TSV、test harness)。`[verification]` (P1-2-9, P1-2-10) は検証のみ。`[git]` (P1-2-0, P1-2-11) は git/gh 操作のみ。`[review-merge]` (P1-2-12) は review + merge。`[wbs]` (P1-2-13) は WBS 統合。各 Task の冒頭にタグを明示済み。

3. **Open Question Q1 / Q2 / Q5 解決 path**: Q1 (llama-cpp-2 version) は P1-2-2 で Context7 / crates.io 最新 stable を pin し、P1-2-3 で Cargo.toml に反映、P1-2-13 で WBS に rationale 記録。Q2 (Zenz prompt exact form) は P1-2-1 で AzooKey docs を通読し、P1-2-7 で `build_prompt` に反映、P1-2-9 で実機検証、P1-2-13 で WBS に解析ログ記録。Q5 (--seed 0 deterministic) は P1-2-7 で sampler 設定、P1-2-9 で実機確認。3 つとも Spec Coverage 表に実装 Task を明記済み。

4. **AzooKey docs 参照が plan 最初の Task に組み込み済み**: Spec §3.3「P1-2 着手前に必ず通読」の指示を、Task P1-2-1 (最初の implementation task の直前) に「[research] AzooKey Zenzai docs 通読 + prompt format 解析メモ作成」として配置。plan PR #<PLAN_PR> 本体にも「AzooKey docs 参照を plan の最初の Task に組み込み」と明記。

5. **fixture TSV の構造が spec §8.3 と整合**: `input_hiragana<TAB>expected_substring` 形式、5 行。各行の `input` は hiragana のみ (spec §5.6 の契約遵守)、`expected_substring` は model 更新で揺らぐ可能性を想定して substring 部分一致 (spec §8.3 の設計そのまま)。Layer 4 (P1-3 で `scripts/phase1-smoke.sh`) と parity を保つため、本 PR で fixture を完成させる。

6. **env-var skip idiom が spec §8.3 / 共通規約と整合**: `std::env::var("KOTOHA_ZENZ_MODEL_PATH")` → `Err(_)` 時に `println!("SKIPPED: ...")` + early return、という pattern。`#[ignore]` attribute は使わない (spec §8.3 の「opt-in」は env-var ベースであり、`cargo test --ignored` を明示的に走らせる設計ではない)。Task P1-2-8 の Step 3 の Rust code で `get_model_path_or_skip` helper として実装済み、5 tests すべてで同 helper を呼ぶ構造にしてある。

7. **verbatim タスクに placeholder 無し**: `[verbatim]` P1-2-3 (Cargo.toml) / P1-2-8 (fixture TSV + test harness) の 2 Task はすべて完全形の verbatim code。`<PINNED_VERSION>` (P1-2-2 で確定)、`<IMPL_ISSUE>` / `<PR_NUMBER>` / `<MERGE_COMMIT>` (gh 操作で取得) は意図した placeholder であり、Task 実行時に実値へ置換する運用ルールを「共通規約」の「placeholder 置換」bullet に明記済み。

8. **Task 順序の依存関係**: P1-2-0 (branch) → P1-2-1 / P1-2-2 (research、順序任意) → P1-2-3 (Cargo.toml、research 結果を消費) → P1-2-4 (struct rewrite、zenz feature を compile 通すのに P1-2-3 必須) → P1-2-5 / P1-2-6 / P1-2-7 (load / model_id / convert、順序は struct rewrite 後であれば任意だが plan では load → model_id → convert の順に明記、convert が最も llama-cpp-2 API 依存が強いため最後) → P1-2-8 (Layer 3 test harness、`convert` 実装が未完でも compile 通るように env-var skip idiom) → P1-2-9 (empirical、実 GGUF 必要) → P1-2-10 (multi-feature gate) → P1-2-11 (push + PR) → P1-2-12 (review + merge) → P1-2-13 (WBS)。逆順依存 (例: P1-2-8 が P1-2-7 を要求する) は無い。

9. **CLAUDE.md 制約遵守**: 言語規則 (英語 = commit / PR / rustdoc / ISSUE、日本語 = plan / WBS) を全 Task の commit message と Write content で徹底。WBS 直接 push は P1-2-13 で CLAUDE.md 「WBS 直接 push の例外」節を明示的に引用。lefthook pre-push `--no-verify` 禁止も共通規約に明記済み。Branch Scope Policy (10 files / 300 lines 目安) は本 PR は 5 files / ~600 LOC なので line 数超過だが、Medium tier で運用する旨を P1-2-12 で明示。依存管理の「新規依存追加時は目的と代替案の比較を commit message または PR body に記載」は P1-2-3 の commit message template と P1-2-11 の PR body template に盛り込み済み。

10. **P1-1 carry-over の履行**: P1-1 WBS (`docs/wbs/2026-04-24-feature-65-kanji-skeleton-mockbackend.md`) の「P1-2 への申し送り」7 bullet を本 plan で完全にカバー済み: (a) `Cargo.toml` の `zenz = []` → `["dep:llama-cpp-2"]` と llama-cpp-2 dep 追加 → P1-2-3、(b) `load` / `model_id` / `convert` の本実装 → P1-2-5 / P1-2-6 / P1-2-7、(c) AzooKey docs 通読 → P1-2-1、(d) `validate_input` / `score_sort_dedupe` helper 再利用 → P1-2-7 (convert 内で明示的に call)、(e) Layer 3 smoke 新設 → P1-2-8、(f) `#[allow(dead_code)]` / `#[allow(unused_variables)]` / `#[derive(Debug)]` 削除 → P1-2-4 / P1-2-5 / P1-2-7 で段階的に削除、(g) `zenz_load_returns_backend_error_in_p1_1_skeleton` test を新 spec に置換 → P1-2-5 で `zenz_load_errors_on_missing_file` に差し替え。
