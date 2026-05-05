---
feature: feature-40-kotoha-input-tests
status: implemented
bounded_context: _uncategorized
related_issues: ["#40"]
related_prs: []
glossary_refs: ["kana","karukan","lefthook","preedit","romaji"]
last_reviewed: 2026-05-05
---

# M4c: input module — mode golden + Karukan diff + 4 property tests

> **Migration note**: 本 spec は `docs/wbs/2026-04-23-feature-40-kotoha-input-tests.md` から spec 化 (Global CLAUDE.md §Documentation Structure 準拠の `docs/wbs/` 廃止対応、2026-05-05)。
> 元 WBS は実装ログ性質。14-section restructure は別 follow-up 起票予定 (各 project の既存 14-section follow-up issue と合流可能)。

## 元 WBS 内容 (実装ログ由来)

milestone: M4c
branch: feature/40-kotoha-input-tests
pr: "#41"
merge_commit: "2e45b7ab683cee5cb0a75d55682af9aac4b7a5cf"
issue: "#40"
status: done
started: 2026-04-23
finished: 2026-04-23
---

# M4c: input module — mode golden + Karukan diff + 4 property tests

## 実施内容

- `crates/kotoha-core/tests/fixtures/mode_cases.tsv` — 70 ケース TSV (7 カテゴリ網羅、spec §11.2 coverage table 準拠)
- `crates/kotoha-core/tests/fixtures/mode_cases_karukan_diff.tsv` — 10 ケース TSV (Karukan 差分 inline コメント付き、ADR 0002 / spec §8.5 準拠)
- `crates/kotoha-core/tests/mode_golden.rs` — TSV リーダ + 集約 assert runner (3 test 関数: 2 件の row-count sanity + 1 件の aggregated every-row)
- `crates/kotoha-core/tests/mode_property.rs` — proptest 4 条件 (各 256 ケース) + 2 regression (合計 6 test 関数)

## 検証結果

- `cargo test -p kotoha-core --test mode_golden`: 3 tests PASS (80 行全てマッチ)
- `cargo test -p kotoha-core --test mode_property`: 6 tests PASS
- `cargo test --workspace`: 100 unit + 3 mode_golden + 6 mode_property + 2 romaji_golden + 5 romaji_property + 2 doctest = 計 118 tests PASS
- `cargo clippy --workspace --all-targets -- -D warnings`: warnings ゼロ
- `cargo fmt --all --check`: diff ゼロ
- `lefthook run pre-push`: 全コマンド PASS (build / clippy / manifest-check / test)

## つまずき

1. **`commit()` 戻り値のみ収集する runner は不十分だった**: 初版の `run_input` は `commit()` の返り値のみを蓄積していたため、`input_char` が mid-stream に `InputStep::Committed("あ")` として emit する kana を取りこぼし、37 行の fixture が失敗した。`input_char` の `InputStep::Committed(s)` も chunk 単位で蓄積するよう修正して解決。
2. **plan 記載の `by\n` / `ky\n` 期待値が M4b 実装と不一致だった**: plan Note 3 は「`flush()` が pending 部を drop する」と述べていたが、実装 (M3b pinned test `flush_normalizes_streaming_byb_residue`) は `by` / `ky` / `y` などの trie partial prefix を tail としてそのまま返す。plan の失敗ハンドリング指示 (2427 行「多くの場合 fixture の期待値ミス」) に従い、fixture 3 行を preserve-partial-prefix 挙動に合わせて修正した。Karukan 差分 `X\ny\n` の inline comment も「output coincides but mode differs」という挙動差異説明に更新。
3. **rustfmt 単一化**: `fs::read_to_string(path).unwrap_or_else(...)` を意図的に 2 行 split していたが、rustfmt が 1 行化を要求して pre-commit が失敗。1 行に統一して解決。

## Review 実施 (Medium tier inline fallback)

`CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS` 環境変数が未設定のため、`agent-teams:team-review` は inline fallback として 5 dimensions (security / performance / architecture / testing / a11y) のチェックリストを main-agent が順次適用する形で実施した (M4b と同じ運用)。

- **Critical / High findings**: 0
- **Medium findings**: 0
- **Low findings**: 2 件 (いずれも accept as-is)
  - row-count sanity test が fixture を再 load する (冗長だが truncation 検知に有用)
  - `run_input` の final-chunk 分岐の `else` が defensive dead branch (読みやすさ優先で残置)
- **Info**: 1 件 (Direct-mode の mid-stream accumulation を検証するテストは不要 — Direct mode は常に `Preedit` を返すため現行 runner で正しく扱える)
- **secrets-check**: CLEAN (gitleaks / trufflehog が未インストールのため手動 regex scan、0 件)
- **owasp-security**: CLEAN (Top 10:2025 全項目 N/A、test-only で runtime / network / auth / crypto surface ゼロ)

## M5 / Phase 0 完了への申し送り

- Phase 0 の残作業: M5 = `kotoha-cli` crate (`kotoha-romaji` CLI) 実装、spec §10 / §13.2 完了条件 (CLI 手動確認) を満たす
- 付随作業: ADR 0003 (CLI line-based commit 抽象化)、ADR 0004 (Shift = 大文字表現の限界) を M5 段階で検討
- `scripts/phase0-smoke.sh` は spec §13.2 の 10 項目を一括実行する smoke テスト、M5 で整備
- M4 完了により Phase 0 の `kotoha-core` 公開 API は確定 (`InputContext`, `RomajiConverter`, `InputMode`, `InputStep`, `kana::*`)
- M4c の fixture 修正 (`by` / `ky` / `y` の preserve-partial-prefix 挙動) は spec §9 の kana rule table 安定性契約に合致しており、plan 側の記述が古かっただけ。後続の plan 起票時には Note 3 の扱いを見直すこと

## 成果物リンク

- PR: #41
- ISSUE: #40 (closed by merge)
- 親 tracking ISSUE: #32 (merge 後に別途 close 予定)
- ADR: `docs/adr/0002-input-mode-transient-vs-sticky.md`
- Spec: `docs/superpowers/specs/2026-04-22-kotoha-phase-0-design.md` §11.2, §11.3
- Plan: `docs/superpowers/plans/2026-04-23-kotoha-phase-0-m4.md`
