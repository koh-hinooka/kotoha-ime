---
feature: docs-20-romaji-design-adrs
status: implemented
bounded_context: _uncategorized
related_issues: ["#20"]
related_prs: []
glossary_refs: ["lefthook","romaji"]
last_reviewed: 2026-05-05
---

# ISSUE #20: romaji design decisions を ADR 0005 / 0006 として記録

> **Migration note**: 本 spec は `docs/wbs/2026-04-23-docs-20-romaji-design-adrs.md` から spec 化 (Global CLAUDE.md §Documentation Structure 準拠の `docs/wbs/` 廃止対応、2026-05-05)。
> 元 WBS は実装ログ性質。14-section restructure は別 follow-up 起票予定 (各 project の既存 14-section follow-up issue と合流可能)。

## 元 WBS 内容 (実装ログ由来)


## 実施内容

- `docs/adr/0005-romaji-trie-over-hashmap.md` を新設。ローマ字→かな変換の 207 エントリルール表に対し、`HashMap<&'static str, &'static str>` + 別途 prefix set ではなくカスタム Trie を採用した根拠を記録。3-way 分類 (`Match | Partial | None`) を 1 回走査で得られること、`&'static str` 寿命適合性、ISSUE #19 (OnceLock キャッシュ化) との互換性を理由として明記。Phase 3 perf tuning で `phf` crate を再評価候補として deferring する方針も記載。
- `docs/adr/0006-non-exhaustive-on-streaming-enums.md` を新設。`ConvertStep` (公開 `pub`) と `PushResult` (内部 `pub(crate)`) の両 streaming 結果 enum に `#[non_exhaustive]` を付与する決定を記録。M3a 時点の variants `{Committed(Cow<'static, str>), Pending, Invalid(char)}` を明示し、将来 variants 追加 (`EmittedPunctuation` / `WouldCommitOnFlush` 等) を SemVer minor bump で行える設計ポリシーを normative 化。
- rustdoc 1-line citation を 2 箇所追加。
  - `crates/kotoha-core/src/romaji/trie.rs` module-level doc (`//!` ブロック末尾) に `設計判断根拠: docs/adr/0005-romaji-trie-over-hashmap.md` を追記。
  - `crates/kotoha-core/src/romaji/mod.rs` の `ConvertStep` doc (`///` ブロック) に `#[non_exhaustive]` 採用根拠: docs/adr/0006-non-exhaustive-on-streaming-enums.md` を追記。
- Small tier team-review (security / architecture / testing) を inline fallback で実施(`CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS` 未設定のため)。Critical / High / Medium / Low の findings は全ディメンションでゼロ。
- secrets-check を手動パターン scan (gitleaks / trufflehog 未インストールのため fallback) で実施。機密情報・高エントロピ文字列・sensitive ファイル種の検出はゼロ。
- cargo build --workspace / cargo test --workspace (6 unit + 2 golden + 5 property + 2 doc-tests, 全 15 件 PASS) / cargo clippy --workspace --all-targets -- -D warnings (警告ゼロ) / cargo fmt --all --check (差分ゼロ) を全て成功。lefthook pre-commit (doc-naming / fmt-check) と pre-push (build / clippy / manifest-check / test) も全て PASS。

## つまずき

- 当初の ADR 本文下書きは ADR 0002 の 96 行スタイルに寄せて 68 行 / 67 行になり、Small tier の「≤100 行 target」合計ラインを超過する懸念が発生した。ADR 0005 を 55 行、ADR 0006 を 52 行まで圧縮し、合計 107 行 (+ rustdoc 4 行) の 111 行 / 4 ファイルで Small tier の許容範囲内 (≤5 ファイル / ≤100 行 target) に収めた。
- ISSUE #20 の下書き本文には streaming enum の variant 名として `Partial` が記載されていたが、実コードの variant 名は `Pending` だった (mod.rs:34 / state.rs:33 で確認)。ADR 0006 の variant 列挙は実コード準拠の `{Committed(Cow<'static, str>), Pending, Invalid(char)}` に訂正して記載した。
- M7 plan (`docs/superpowers/plans/2026-04-22-kotoha-phase-0-implementation.md`) には canonical 採番 (0001=retraction / 0002=input-mode / 0003=shift reserved / 0004=cli-line reserved) 以前の古い ADR 参照が 3 箇所残っていた (line 29 / line 873 / lines 1049–1078)。ISSUE #20 の scope を逸脱するため修正は行わず、follow-up ISSUE #46 として登録した。本 PR commit body および PR body の Out of scope セクションに明示。

## 成果物リンク

- ISSUE: #20
- PR: #45 (squash-merge commit on develop: `3da30b4`)
- follow-up ISSUE: #46 (M7 plan stale ADR refs, Low priority)
- ADR 新設:
  - `docs/adr/0005-romaji-trie-over-hashmap.md` (55 行)
  - `docs/adr/0006-non-exhaustive-on-streaming-enums.md` (52 行)
- rustdoc citation 追加:
  - `crates/kotoha-core/src/romaji/trie.rs` (+2 行)
  - `crates/kotoha-core/src/romaji/mod.rs` (+2 行)
