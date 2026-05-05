---
feature: feature-17-kotoha-romaji-core
status: deprecated
deprecated_reason: "Phase E migration で旧 docs/wbs/ から spec 化した実装ログ性質の文書。Global CLAUDE.md §Development Flow legacy spec 取扱いルール (実装ログ性質 → status: deprecated、本文 14-section restructure 不要) に基づき deprecated 扱い。git history は参照点として保持 (2026-05-06)。"
bounded_context: _uncategorized
related_issues: ["#17"]
related_prs: []
glossary_refs: ["canonical-romaji","hatsuon","karukan","lefthook","romaji","sokuon"]
last_reviewed: 2026-05-06
---

# M3a: romaji module core — rules table + trie + state machine + RomajiConverter facade

> **Migration note**: 本 spec は `docs/wbs/2026-04-23-feature-17-kotoha-romaji-core.md` から spec 化 (Global CLAUDE.md §Documentation Structure 準拠の `docs/wbs/` 廃止対応、2026-05-05)。
> 元 WBS は実装ログ性質。14-section restructure は別 follow-up 起票予定 (各 project の既存 14-section follow-up issue と合流可能)。

## 元 WBS 内容 (実装ログ由来)

milestone: M3a
branch: feature/17-kotoha-romaji-core
pr: "#18"
merge_commit: "184ece0"
issue: "#17"
status: done
started: 2026-04-23
finished: 2026-04-23
---

# M3a: romaji module core — rules table + trie + state machine + RomajiConverter facade

## 実施内容

- M3a-1: `crates/kotoha-core/src/romaji/rules.rs` を新規作成。Karukan 互換の 207 エントリを `&'static [(&'static str, &'static str)]` 型の static `RULES` table として配置。mod tree に wire せずに `dead_code` 警告を回避(M3a-2 で trie が consume するまで未 wire)
- M3a-2: `crates/kotoha-core/src/romaji/trie.rs` を追加。`Trie` / `Node` / `Lookup`(`Match | Partial | None`) を実装、6 unit tests(空 trie / 単一 match / partial prefix / 完全非一致 / 長音 / 促音 prefix)。`rules.rs` を mod tree に wire し、rustdoc の ordering claim を「longest-first」から実態の「category-grouped(trie なので ordering 非依存)」へ修正
- M3a-3: `crates/kotoha-core/src/romaji/state.rs` を追加。`StateMachine` + `PushResult` + `settle()` を実装、11 unit tests(sokuon / hatsuon / long-vowel / invalid / reset 等)。`trie.rs` の file-scope `#![allow(dead_code)]` を削除
- M3a-3 code-quality review 対応: `settle` の precondition を rustdoc に文書化。`state.rs` の `#![allow(dead_code)]` を file-scope から item-level の `#[allow(dead_code)]` に narrow
- M3a-4: `crates/kotoha-core/src/romaji/mod.rs` に `RomajiConverter` facade + `ConvertStep` public API を実装、18 unit tests。`lib.rs` に `pub mod romaji;` + `pub use romaji::{RomajiConverter, ConvertStep};` を追加。`state.rs` の item-level `#[allow(dead_code)]` を全削除(facade が全 API を consume した)
- M3a-4 spec alignment: `konnichiwa → こんにちは` は literal rule では到達不能(romaji → かな の多対一性 + particle heuristics 不在)と判明。Option A(spec 側を rule-faithful に合わせる)を採用。spec §9.1 を新設して「IME standard input は rule-faithful 打鍵 convention に従う」旨を明文化、§13.2 / §11.2 / §13.4 の examples を `konnichiwa → konnnichiha` に置換、plan / ISSUE #17 body も連動更新
- PR #18 4-dim review 対応(PR 内):(a) `ConvertStep::Committed` を `String` から `Cow<'static, str>` に変更して static rule 経路の heap allocation を削減、(b) `rules.rs` に invariant tests 3 本(keys が ASCII / keys が unique / values が期待 Unicode 範囲)を追加、(c) `state::settle` の rustdoc に ISSUE #15 trace link を追加
- テスト合計 63 unit + 1 doc = 64 tests、すべて PASS(baseline 25 → 63 = +38 件:trie 6 + state 11 + facade 18 + rules invariants 3)
- 7 files changed、+1128 / -32 で squash merge(base `2608dd7` → merge `184ece0`)

### Branch 上の 8 commits

1. `5750a21` — feat(kotoha-core): add romaji rules table (not yet wired up) [M3a-1]
2. `5fb441d` — style(kotoha-core): reformat romaji::rules and correct ordering rustdoc [M3a-2 prep]
3. `a1531ea` — feat(kotoha-core): add romaji::trie prefix-match data structure [M3a-2]
4. `ce10f95` — feat(kotoha-core): add romaji::state stream state machine [M3a-3]
5. `1062ae0` — refactor(kotoha-core): address M3a-3 code-quality review
6. `95a7fb6` — feat(kotoha-core): add RomajiConverter public API [M3a-4]
7. `81855a7` — docs: align spec + plan with rule-faithful romaji convention (M3a-4 alignment)
8. `3c4deab` — refactor(kotoha-core): address PR #18 review findings

## つまずき

1. M3a-1 完了時点で sub-agent が `cargo fmt --check` PASS と報告していたが、M3a-2 開始時に rustfmt が再 format を実行して diff が発生した。原因は M3a-1 時点で `rules.rs` が mod tree に wire されておらず、一部の format check 対象から外れていた可能性。M3a-2 の prep commit `5fb441d` で reformat と rustdoc 修正を同時に解決した
2. M3a-1 の `rules.rs` rustdoc が「longest-first ordering」と宣言していたが実態は category-grouped で、sub-agent の self-verification が不正確だった。M3a-2 の prep commit で rustdoc を実態(category-grouped、trie なので ordering 非依存)に修正した
3. M3a-2 で `trie.rs` を mod tree に wire した結果 `#![allow(dead_code)]` が file-scope で必要になった。M3a-3 で state が trie を consume した時点で file-scope allow を削除したが、review で「M3a-3 が同じ pattern を `state.rs` に再導入した」と指摘され、item-level allow に narrow し直した
4. M3a-4 で facade を実装し全 test を書いた結果、spec §13.2 / plan / ISSUE #17 body の `konnichiwa → こんにちは` acceptance が **literal rule では到達不能** と判明。原因は romaji → かな が多対一であり、strict な IME standard input は `konnnichiha` である点。sub-agent が rule-faithful に test を書き、spec alignment commit `81855a7` で全 example を IME standard input に置換。§9.1 新設で convention を明文化した
5. PR #18 review findings 修正で `rules.rs` invariant test の accepted Unicode range が狭すぎ、punctuation entries(`,` → `、` 等の 7 entries)で fail した。sub-agent が BLOCKED で正しく報告し、range を broaden(CJK Symbols/Punctuation + 限定 ASCII passthrough)して解決した

## Review

Multi-dimensional review(security / performance / architecture / testing)を `agent-teams:team-review` で並列 dispatch + secrets-check(主エージェントの grep)。

- Security: **APPROVED**(Low×2 observational)
- Performance: **APPROVED_WITH_OBSERVATIONS**(Medium×2: OnceLock trie cache / Cow API for `ConvertStep::Committed`)
- Architecture: **APPROVED_WITH_OBSERVATIONS**(Low×多数: ADR candidates)
- Testing: **APPROVED_WITH_OBSERVATIONS**(Low×6: rules invariant tests 不足、helper direct tests 等)
- 指摘対応: Cow API + rules invariant tests + ISSUE #15 trace link を PR 内 commit `3c4deab` で実装。残りの findings は follow-up ISSUE #19 / #20 に切り出し、Low findings は M3b / Phase 1 の implementer judgment に委ねた

## 起票済み follow-up ISSUEs

- ISSUE #15(pre-existing、Open): spec §9.1 pending-buffer backtrack rule を正式化(本 PR で rustdoc に trace link を追加済み)
- ISSUE #16(pre-existing、Open): canonical-romaji ADR tracking hook を M7 plan / ROADMAP に追加
- ISSUE #19(新規、Open): perf の OnceLock trie cache(Phase 1 向け)
- ISSUE #20(新規、Open): romaji design decisions の ADR drafts(trie vs hashmap / `#[non_exhaustive]` on streaming enums)

## M3b への申し送り

1. **API 確定**: `ConvertStep::Committed` は `Cow<'static, str>`、`RomajiConverter::convert` は `(String, String)` 返却、`push` は `ConvertStep` を返す。M3b の golden runner と property test はこの API を base にする
2. **proptest dev-dependency**: M3b-1 で workspace root `Cargo.toml` の `[workspace.dev-dependencies]` に `proptest = "1.5"` を追加し、`crates/kotoha-core/Cargo.toml` の `[dev-dependencies]` で `{ workspace = true }` 参照する
3. **Golden fixture**: `crates/kotoha-core/tests/fixtures/romaji_cases.tsv` に 200+ の Karukan 互換ケースを配置する。`konnnichiha → こんにちは`(IME standard input)と `konnichiwa → こんいちわ`(rule-faithful literal)の両方を含めること
4. **Property test 2 条件**: (a) 冪等性(`prop_idempotence_on_committed` — committed 出力を再度 convert に通すと committed が安定し pending が発生しない)、(b) 結合性(incremental `push` が batch `convert` と同じ commit stream を生む)。3 番目の「可逆性」は PR #14 で spec から除外済み
5. **lefthook pre-push gate**: `crates/` 配下が充実したため `cargo build / clippy / test` が毎 push で走る状態(既に機能している)。M3b では proptest 追加による test 時間増に注意する

## 成果物リンク

- PR: #18(squash merge、merge commit `184ece0`)
- ISSUE: #17(Closed)
- Follow-up ISSUEs: #15 / #16(pre-existing、Open)、#19 / #20(新規、Open)
- Spec: `docs/superpowers/specs/2026-04-22-kotoha-phase-0-design.md`(§9.1 新設 + §13.2 / §11.2 / §13.4 の example 置換)
- Plan: `docs/superpowers/plans/2026-04-23-kotoha-phase-0-m3.md`(M3a セクション完了マーク)
- 影響ファイル: `crates/kotoha-core/src/romaji/rules.rs`(新規)、`crates/kotoha-core/src/romaji/trie.rs`(新規)、`crates/kotoha-core/src/romaji/state.rs`(新規)、`crates/kotoha-core/src/romaji/mod.rs`(新規)、`crates/kotoha-core/src/lib.rs`、`docs/superpowers/specs/2026-04-22-kotoha-phase-0-design.md`、`docs/superpowers/plans/2026-04-23-kotoha-phase-0-m3.md`
