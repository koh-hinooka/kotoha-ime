---
feature: feature-52-kotoha-cli
status: deprecated
deprecated_reason: "Phase E migration で旧 docs/wbs/ から spec 化した実装ログ性質の文書。Global CLAUDE.md §Development Flow legacy spec 取扱いルール (実装ログ性質 → status: deprecated、本文 14-section restructure 不要) に基づき deprecated 扱い。git history は参照点として保持 (2026-05-06)。"
bounded_context: _uncategorized
related_issues: ["#52"]
related_prs: []
glossary_refs: ["hiragana","kana","romaji","shift-trigger"]
last_reviewed: 2026-05-06
---

# M6: kotoha-cli — `kotoha-romaji` binary + `scripts/phase0-smoke.sh`

> **Migration note**: 本 spec は `docs/wbs/2026-04-23-feature-52-kotoha-cli.md` から spec 化 (Global CLAUDE.md §Documentation Structure 準拠の `docs/wbs/` 廃止対応、2026-05-05)。
> 元 WBS は実装ログ性質。14-section restructure は別 follow-up 起票予定 (各 project の既存 14-section follow-up issue と合流可能)。

## 元 WBS 内容 (実装ログ由来)

milestone: M6
branch: feature/52-kotoha-cli
pr: "#53"
merge_commit: "1305449e2c3de6039ba1ffdd8c9925825d1f376b"
issue: "#52"
status: done
started: 2026-04-23
finished: 2026-04-23
---

# M6: kotoha-cli — `kotoha-romaji` binary + `scripts/phase0-smoke.sh`

## 実施内容

- `crates/kotoha-cli/` を新規 workspace member として追加 (`Cargo.toml` に `clap = { version = "4.5", features = ["derive"] }` を workspace 依存として pin)
- `crates/kotoha-cli/src/lib.rs` — pure helper 2 関数 (`process_line` / `format_line_output`) を切り出し、binary を薄く保ったうえで単体テストを 9 ケース埋め込み (spec §10 の契約と plan M6-3 T1–T9 を 1:1 対応)
  - T1: 基本ローマ字 `konnnichiha` → `こんにちは`
  - T2: Shift 大文字 `Hello` が Transient Direct → commit で Hiragana Sticky へ自動復帰
  - T3: `ctx.set_mode(Direct)` 経由の Sticky Direct は commit 後も Direct のまま
  - T4: 空行入力は空文字列を返し mode は不変
  - T5/T6/T7: `format_line_output` の Hiragana/Direct/show_mode=false 3 分岐
  - T8: 複数行にまたがる Sticky Direct の mode carryover
  - T9: Invalid (非 ASCII) char を silent drop しつつ後続 `a` → `あ` は正常 commit
- `crates/kotoha-cli/src/bin/romaji.rs` — clap derive で `--mode hiragana|direct` / `--show-mode` 2 flag を受け、stdin を `BufRead::lines()` で行単位に読み、per-line に `process_line` → `format_line_output` → `writeln!` を回す。stdin read 失敗 / stdout write 失敗は exit 1、clap parse error は clap default の exit 2 (spec §10.5 準拠)。`--mode direct` 指定時のみ起動直後に `ctx.set_mode(InputMode::Direct)` を呼んで Sticky Direct で loop に入る (plan M6 既知懸念 3 の扱い)
- `scripts/phase0-smoke.sh` — spec §13.2 の 10 assertion を verbatim で実行する bash スクリプト。`set -euo pipefail`、`cargo build -p kotoha-cli --quiet` を先行して wall-clock を平準化、`cargo run --quiet -p kotoha-cli --bin kotoha-romaji --` を base command に PASS/FAIL をカウント、最終行に `=== phase0-smoke: N/10 PASS, N/10 FAIL ===` を emit

## 検証結果

- `cargo build --workspace`: 0.03s cached build, exit 0
- `cargo test --workspace`: 9 (kotoha-cli) + 102 (kotoha-core unit) + 3 (mode_golden) + 6 (mode_property) + 2 (romaji_golden) + 5 (romaji_property) + 2 (doctest) = 計 129 tests PASS
- `cargo test -p kotoha-cli`: 9/9 PASS (T1–T9 全て)
- `cargo clippy --workspace --all-targets -- -D warnings`: warnings ゼロ
- `cargo fmt --all --check`: diff ゼロ
- `bash scripts/phase0-smoke.sh`: `10/10 PASS, 0/10 FAIL` (spec §13.2 完了条件を充足)

## つまずき

1. **LOC が plan 見積もり (250–350 行) を超過 (実績 505 行 / +505 -1)**: 内訳は `Cargo.lock` が clap 依存追加で +128 行、`crates/kotoha-cli/src/lib.rs` が TDD 9 ケース分のテスト本体で +163 行 (うちテスト部分が 90 行強)、`scripts/phase0-smoke.sh` が spec §13.2 の 10 assertion を verbatim 実装したことで +100 行、`crates/kotoha-cli/src/bin/romaji.rs` が +92 行、`crates/kotoha-cli/Cargo.toml` が +20 行、workspace 側 `Cargo.toml` が +3 -1 行。Cargo.lock (128 行) は自動生成で実質的な review 対象外、smoke スクリプト (100 行) は spec §13.2 の assertion を verbatim で写経した結果であり、lib.rs のテスト 9 ケースも plan M6-3 の T1–T9 を忠実に実装した結果。いずれも scope creep ではなく plan 追従による機械的帰結。Branch Scope Policy の 300 行 soft target には抵触するが、これを split した場合 TDD サイクル / smoke 完了条件検証 / CLI 実装のいずれかが単独で残ってしまい deliverable として成立しないため、split は採用せず single PR で merge した。
2. **global CLAUDE.md の Memory Monitoring (available < 8GB で sequential only)**: 実行時のメモリ状態は available 6.0GB / Swap 38.7% で parallel cargo は禁止帯。`cargo build` → `cargo test --workspace` → `cargo test -p kotoha-cli` → `cargo clippy` → `cargo fmt --check` を strict sequential で実行した。build は cached (0.03s) で memory pressure は発生せず。

## Review 実施 (Medium tier inline fallback)

`CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS` 環境変数が未設定のため、`agent-teams:team-review` は inline fallback として 5 dimensions (security / performance / architecture / testing / a11y-as-CLI-UX) を main-agent が順次適用した (M4c / M5 と同じ運用)。

- **Critical / High findings**: 0
- **Medium findings**: 0
- **Low findings**: 1 件 (accept as-is)
  - `format_line_output` は `show_mode=false` で `.to_string()` による不要 allocation が 1 回発生する (`Cow<'a, str>` 返却なら回避可能)。per-line 1 回かつ CLI 用途のため性能影響は無視できる。
- **Info**: 1 件 (defensive code 未 test)
  - `format_line_output` の `InputMode` `_ =>` fallback (`"?"`) は `#[non_exhaustive]` に備えた defensive branch で現在到達不能、unit test 対象外。
- **secrets-check**: CLEAN (gitleaks / trufflehog が未インストールのため手動 regex scan、credentials / high-entropy string / secret file いずれも 0 件)
- **owasp-security**: CLEAN
  - A01/A02/A05/A07/A10: N/A (ローカル CLI、auth / crypto / network surface なし)
  - A03 Injection: clap-parsed args は型安全、stdin chars は pure state machine に forward されるのみで format string / shell injection の余地なし
  - A04 Insecure Design: 巨大 stdin line に対する allocation bomb の理論リスクあり。ただし Phase 0 scope では明示的に cap を設けず、ISSUE #39 (direct_buffer 長制限) と並んで Phase 3 で解消予定
  - A08 Software/Data Integrity: 追加依存は `clap 4.5` のみで audited、`Cargo.lock` を commit
  - A09 Logging: I/O 失敗は stderr に `kotoha-romaji:` prefix 付きで出力、silent swallow なし

## Smoke test 結果

`bash scripts/phase0-smoke.sh` (develop merge 後 / feature branch 最終 commit で実行):

```
=== phase0-smoke: pre-building kotoha-cli (debug) ===
=== phase0-smoke: running 10 assertions ===
PASS  1. konnnichiha → こんにちは
PASS  2. tsumugi → つむぎ
PASS  3. n'ya → んや
PASS  4. nya → にゃ
PASS  5. HELLO (Shift trigger) → HELLO
PASS  6. Ko\nkonnnichiha (Transient auto-return) → Ko\nこんにちは
PASS  7. hello\nworld --mode direct (Sticky) → hello\nworld
PASS  8. Konnichiwa\nkonnnichiha (mixed) → Konnichiwa\nこんにちは
PASS  9. Hi --show-mode → Hi [H]
PASS  10. hi --mode direct --show-mode → hi [D]
=== phase0-smoke: 10/10 PASS, 0/10 FAIL ===
```

spec §13.2 の CLI 手動確認 10 項目を自動化で満たした。exit code は 0。

## Phase 0 進捗

- M1 (workspace skeleton) → merged
- M2 (kana module) → merged
- M3 (romaji converter) → merged
- M4a/b/c (input module + golden + property) → merged
- M5 (ADR 0002 / kotoha-core API 確定) → merged (M4c に包含)
- **M6 (kotoha-cli) → merged (本 WBS の対象)**
- 残作業: M7 (ADR 追補 + Phase 0 締めのドキュメント整備)
  - ADR 候補: CLI line-based commit 抽象 (plan M6 §既知懸念 1)、Shift=大文字 の限界 (plan M6 §既知懸念 4 と関連)
  - `docs/ROADMAP.md` の Phase 0 完了宣言

残 open ISSUE:

- #39 (perf/security: direct_buffer 長制限): Phase 3 dependency (CLI 側の line-length cap と同じ流儀で `InputContext` 側 buffer cap を入れる)

## 成果物リンク

- ISSUE: #52 (closed by merge via `Closes #52`)
- PR: #53 (squash-merged to develop)
- Merge commit: `1305449e2c3de6039ba1ffdd8c9925825d1f376b`
- Binary: `target/debug/kotoha-romaji` (workspace root 起点、`cargo build -p kotoha-cli` で生成)
- Smoke script: `scripts/phase0-smoke.sh`
- Spec: `docs/superpowers/specs/2026-04-22-kotoha-phase-0-design.md` §10, §13.2
- Plan: `docs/superpowers/plans/2026-04-23-kotoha-phase-0-m6.md`
- Crate: `crates/kotoha-cli/`
