---
feature: docs-15-spec-pending-buffer-backtrack-rule
status: implemented
bounded_context: _uncategorized
related_issues: ["#15"]
related_prs: []
glossary_refs: []
last_reviewed: 2026-05-05
---

# ISSUE #15: spec §9.3 — pending バッファの backtrack 規則 normative 化

> **Migration note**: 本 spec は `docs/wbs/2026-04-23-docs-15-spec-pending-buffer-backtrack-rule.md` から spec 化 (Global CLAUDE.md §Documentation Structure 準拠の `docs/wbs/` 廃止対応、2026-05-05)。
> 元 WBS は実装ログ性質。14-section restructure は別 follow-up 起票予定 (各 project の既存 14-section follow-up issue と合流可能)。

## 元 WBS 内容 (実装ログ由来)


## 実施内容

- spec `docs/superpowers/specs/2026-04-22-kotoha-phase-0-design.md` に §9.3「Pending バッファの backtrack 規則」サブセクションを新設。`StateMachine::push` の既存実装 (sokuon 二重子音 / bare-`n` hatsuon / single-char invalid peel-off) を normative 契約として記述した。内訳は以下の 5 サブセクション。
  - §9.3.1 `Lookup::{Match, Partial, None}` 3 分類と各分類の動作。
  - §9.3.2 `None` 発生時の backtrack 規則(sokuon / bare-`n` / peel-off の 3 優先分岐、および `RomajiConverter::{convert, flush}` が `normalize` を追加呼び出しする理由)。
  - §9.3.3 代表 5 入力 (`ko` / `kon` / `koh` / `kk` / `shz`) の pending 遷移・累積 emission・最終 pending を 3 列 truth table で提示。
  - §9.3.4 不変条件 3 項 (pending 安定性 / flush 決定性 / emission 局所性)。
  - §9.3.5 関連セクション参照 (§11.3 プロパティテスト / 実装計画 Task M3b-2)。
- rustdoc cross-reference を追加(双方向 linking)。
  - `crates/kotoha-core/src/romaji/mod.rs`: `RomajiConverter::{convert, push, flush}` の 3 メソッド doc に「参照: spec §9.3 pending バッファの backtrack 規則。」を追加。
  - `crates/kotoha-core/src/romaji/state.rs`: モジュール doc の "documented only in this module's source" 文言を spec §9.3 への source-of-truth 参照に置き換え。`StateMachine::push` doc と `StateMachine::settle` の `# Normative spec` セクションも §9.3 参照へ更新。
- `crates/kotoha-core/tests/fixtures/romaji_cases.tsv` の `pending tails` セクション冒頭に header comment を追加し、fixture 行が §9.3 の normative 契約を検証する役割であることを明示。
- Small tier team-review (security / architecture / testing) を inline で実施。Critical / High / Medium の findings はゼロ。Low 1 件(§9.3.3 `shz` 行の説明文の密度)は acknowledge のみ。
- cargo build / cargo test (100 unit + 16 integration + 2 doc-tests) / cargo clippy `-D warnings` / cargo fmt --check を全て成功。

## つまずき

- ISSUE #15 タイトルは「§9.1 / §9.2 pending-buffer backtrack rule」だったが、§9.1 と §9.2 は既に ADR 0001 から 3 箇所、M3 plan から 1 箇所の外部 citation を持っていた。§9.1 / §9.2 を書き換えると既存 citation が全て破壊されるため、新設 §9.3 として追記する方針へ再解釈した。判断根拠は PR #44 body で明記。
- 同じ理由により、`state.rs` および rustdoc の既存コメントに残っていた "tracked in ISSUE #15 (spec §9.1 / §9.2)" 文言も §9.3 参照へ一括置換した(cross-reference の source-of-truth を単一化)。
- 実装コードには一切手を入れず、spec 追記 + rustdoc cross-reference + TSV header comment のみに scope を限定。behavior change ゼロを維持した。

## 成果物リンク

- ISSUE: #15
- PR: #44 (merge commit: `baf1eff5ac842bd5ea15ee663c5314c686b0ce73`)
- spec edit: `docs/superpowers/specs/2026-04-22-kotoha-phase-0-design.md` §9.3 (新設)
- rustdoc edits: `crates/kotoha-core/src/romaji/mod.rs`, `crates/kotoha-core/src/romaji/state.rs`
- fixture edit: `crates/kotoha-core/tests/fixtures/romaji_cases.tsv` (pending tails header comment)
