# Phase 0 Milestone 6 (M6: kotoha-cli) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 動作確認用 CLI バイナリ `kotoha-romaji`(workspace member `crates/kotoha-cli`)と、Spec §13.2 の CLI 手動確認 10 件を一括自動検証するシェルスクリプト `scripts/phase0-smoke.sh` を実装し、Phase 0 の CLI レイヤーを完成させる。本 M6 plan は単一 PR (Medium tier) で kotoha-cli crate + smoke script を揃えて deliver し、Phase 0 完了条件 §13.1 / §13.2 を CLI 側から満たす。

**Architecture:** 新規 workspace member `crates/kotoha-cli` を追加する。crate レイアウトは `crates/kotoha-cli/Cargo.toml`(binary crate) と `crates/kotoha-cli/src/bin/romaji.rs`(`kotoha-romaji` バイナリ entry point)の 2 ファイル + ライン処理純粋関数を抽出した `crates/kotoha-cli/src/lib.rs`(単体テスト対象)の 3 ファイル構成とする。CLI は stdin を `BufRead::lines()` で行単位読み込み、1 行 = 1 論理 Enter としてライブラリ `kotoha-core` の `InputContext` に `input_char` / `commit` をディスパッチし、`commit` の戻り値を stdout へ 1 行出力する。`--mode` で初期モード、`--show-mode` でモード表示 suffix を制御する。smoke script は debug ビルドを `cargo run -p kotoha-cli --bin kotoha-romaji --` 経由で呼び、Spec §13.2 の 10 assertion を順次 `diff` で判定する。

**Tech Stack:** Rust 2021 / MSRV 1.80、kotoha-cli crate(新規)、kotoha-core crate(既存、path dependency)、`clap = { version = "4.5", features = ["derive"] }`(新規 workspace dependency)。それ以外の新規依存は追加しない。smoke script は `bash` + `set -euo pipefail`(`modern-toolchain.md` で legacy とされる tool は使わないが、shell script の shebang と interpreter 指定は`bash` を採用する、lefthook が既に bash スクリプトを採用しているため整合する)。

**Spec:** `docs/superpowers/specs/2026-04-22-kotoha-phase-0-design.md` §10 全体(CLI 仕様、10.1 コマンド / 10.2 動作仕様 / 10.3 使用例 / 10.4 Phase 0 非対応事項 / 10.5 終了コード)、§13.1 ビルド・テスト系成功基準、§13.2 CLI 手動確認 10 件。

**Phase 0 全体計画:** `docs/superpowers/plans/2026-04-22-kotoha-phase-0-implementation.md` M6 節(L1025〜L1048)を本 plan で per-task 分解まで拡張する。

**前提となる M4 成果 (公開 API の形):**

- `kotoha_core::InputContext`
  - `new() -> Self` — 初期状態 `(Hiragana, Sticky)` を返す。
  - `mode(&self) -> InputMode` — 現在の user-visible mode。
  - `preedit(&self) -> String` — 未確定バッファ(Phase 0 の CLI では使用しない、Spec §10.4)。
  - `input_char(&mut self, ch: char) -> InputStep` — 1 文字投入。ASCII uppercase が Hiragana 中に来たら自動で `(Direct, Transient)` へ遷移(ADR 0002)。
  - `commit(&mut self) -> String` — Enter 相当。Transient origin なら commit 後に `(Hiragana, Sticky)` へ自動復帰。
  - `cancel(&mut self)` — Esc 相当(CLI では使用しない)。
  - `toggle_mode(&mut self)` — 明示トグル(CLI では使用しない、Spec §10.4 で REPL は Phase 0 非対応)。
  - `set_mode(&mut self, mode: InputMode)` — 明示指定。Sticky origin を生成。`--mode direct` の初期化で 1 回だけ呼ぶ。
  - `reset(&mut self)` — 全状態クリア(CLI では使用しない)。
- `kotoha_core::InputMode` — `#[non_exhaustive]` pub enum、variants `Hiragana` / `Direct`。
- `kotoha_core::InputStep` — `#[non_exhaustive]` pub enum、variants `Preedit` / `Committed(String)` / `Invalid(char)`。本 CLI は `Preedit` を無視(Spec §10.4 非対話 REPL)、`Committed(String)` を行 output buffer に append、`Invalid(char)` をサイレント drop(§9.3.2 で既に `InputContext` 内部でも drop される)。

**本 M6 plan 内で解決する既知の懸念:**

- **既知懸念 1 — preedit / invalid の CLI 側扱いが Spec §10.2 に明記されていない**: Spec §10.2 は「行末を Enter (commit) として扱う」「確定文字列を 1 行 1 出力」とのみ規定し、per-char で発生する `InputStep::Preedit` / `InputStep::Invalid(char)` を CLI がどう扱うかを明示していない。本 plan では「preedit は CLI 出力には surface しない(Spec §10.4 で対話 REPL を Phase 0 で実装しないと宣言している以上、preedit を逐次表示する意味がない)」「invalid は silent drop(`InputContext::input_char` が既に invalid を drop する方針のため、CLI 側で追加ログを出さない)」を normative として plan 内に固定する。
- **既知懸念 2 — `--show-mode` の suffix 書式と Transient / Sticky の区別有無**: Spec §10.3 の使用例は `[H]` と `[D]` のみを示し、Transient と Sticky の区別は行わない(Spec §8.1 の `ModeOrigin` は `pub(crate)` で外部に露出させない方針)。本 plan では「suffix の表示は `mode()` の戻り値のみを見る」「表示タイミングは `commit()` の後(次行の開始時 mode と同一になるように、自動復帰が発生する Transient Direct 行も `[H]` で終わる)」を plan 内に固定する。
- **既知懸念 3 — `--mode direct` 起点の Sticky 持続条件**: `--mode direct` は `InputContext::set_mode(InputMode::Direct)` を 1 回呼ぶことで `(Direct, Sticky)` を生成する。Sticky origin は `commit()` で自動復帰しないため、Spec §10.3 の例 4(`printf "hello\nworld\n" | kotoha-romaji --mode direct` が両行 `hello` / `world` となる)が成立する。この呼び出しは読み込みループに入る前に一度だけ行う。
- **既知懸念 4 — 終了コードと clap のデフォルト挙動の非互換**: Spec §10.5 は exit 0 / 1 / 2 の 3 段階を規定するが、clap 4.5 は不正な引数に対してデフォルトで exit 2 を返す。本 plan では clap の `Command::error_handling` 再マップはコストに対して効果が薄いため、**Spec 側を clap デフォルトに合わせる解釈**(exit 2 = 不正引数 / 内部エラーの包括コード、exit 1 = stdin read failure)を plan 内に記録する。根拠は以下 3 点。(a) clap 4.5 の exit code は upstream 設計で、Kotoha が remap するコストが設計上の利益を上回らない。(b) Spec §10.5 はユーザ向け仕様でありプロセスレベル contract としては弱い。(c) Phase 3 の IBus engine では stdin read も exit code も使われないため、本 remap を入れても Phase 3 に波及しない。本判断は本 PR 内の PR body で言及し、Spec §10.5 の書き換えは別 ISSUE(M7 ADR 0004 範囲)で扱う。
- **既知懸念 5 — 行末改行と stdout 末尾改行の取り扱い**: `BufRead::lines()` は行末の `\n` を strip するため、行ごとに出力する文字列には CLI 側で `\n` を再付与する必要がある。空行(`"\n"`)は `commit()` 直後に空文字列を書き出すだけで処理できる(空行も 1 回の commit cycle として扱う)。EOF 時に末尾改行がない最終行も `lines()` は 1 行として返すため、逐次処理で問題にならない(ただし unit test で covers する)。
- **既知懸念 6 — debug vs release build**: smoke script で `cargo build --release` を事前実行すれば起動が速いが、clean checkout で即座に走らせたい(CI なしのプロジェクトで lefthook pre-push の後に手動検証する流れ)。本 plan では **debug build + `cargo run -p kotoha-cli --bin kotoha-romaji --`** を採用する。起動オーバヘッドは 1 回 < 200ms で 10 assertion 合計 < 3 秒、Phase 0 の開発サイクルで十分許容できる。

---

## Scope check / PR split rationale

Spec §14 の工数目安では「`kotoha-cli` (`kotoha-romaji` コマンド、mode オプション含む)」が 0.8 日、M6 全体の変更規模見積りは:

| 成果物 | 見積 LOC |
|---|---|
| workspace `Cargo.toml` の `members` 追加 + `[workspace.dependencies]` に clap 追加 | 3〜5 |
| `crates/kotoha-cli/Cargo.toml` | 20〜25 |
| `crates/kotoha-cli/src/lib.rs`(ライン処理純粋関数 + 単体テスト) | 120〜150(うち関数本体 30〜40 + テスト 80〜110) |
| `crates/kotoha-cli/src/bin/romaji.rs`(clap 引数パース + stdin ループ) | 60〜80 |
| `scripts/phase0-smoke.sh` | 100〜130(10 assertion + helper + set -euo pipefail + build command) |
| 合計 | 約 300〜390 行 |

= CLAUDE.md の Branch Scope Policy で Medium tier (≤ 10 files / ≤ 300 lines) の実質上限をわずかに超える可能性がある。ただし以下の理由で 1 PR に収める:

- shell script の 100〜130 行は大部分が heredoc / echo / diff 定型パターンであり実質的な "review に時間を要する code" ではない。code density は production Rust と比べて低い。
- test code 80〜110 行は Branch Scope Policy の "auto-generated を除外" には該当しないが、CLAUDE.md 全体の意図として "レビュワーが消化できる実質的な changeset" を測る趣旨であり、本 plan の純粋関数単体テストは単一ファイル単一テーマで review cost が低い。
- `crates/kotoha-cli` の deliverable は "CLI binary + その smoke 検証" として coherent であり、2 PR に分割すると PR #2(smoke script 単体)は PR #1(CLI binary)に完全依存する。分離レビューの利益がほぼない。
- Phase 0 完了条件 §13.2 の 10 assertion は smoke script で満たされるのが規定フロー。CLI binary だけ先行 merge しても acceptance criteria の検証経路がない状態になり、バグ発見時の hotfix が 2 PR cycle になる。

**代替案: 2 PR(M6a impl / M6b smoke)の却下理由**

- smoke script は CLI binary 前提。2 PR にすると M6b の開発中に M6a がマージされ、M6b 単体の review 時に "smoke が fails なら原因は M6a 側" という diagnosis 分離が必要になる。実質レビューコスト増。
- CLAUDE.md 冒頭「複数の小さい PR を好む」原則は、**独立してレビュー・マージできる**単位に分割するのが目的。smoke は CLI に厳密依存のため独立でない。
- 3 PR(workspace wiring / CLI binary / smoke)案は過剰分割、workspace wiring 単体 PR は数行のコメント付き diff でレビュー不要レベル。

**最終決定: 単一 PR(M6)、Medium tier review**。

### Medium tier review 必要 skill(CLAUDE.md の PR Review Matrix に従う)

- `agent-teams:team-review`(dimensions = security / performance / architecture / testing / a11y の 5 軸)
- `owasp-security`(軽量だが Rust/CLI にも関連性あり)
- `secrets-check`(必須)
- 任意: `simplify`(refactoring が発生した場合のみ)
- 任意: `/ultrareview`(通常は team-review で十分、Medium tier なら skip)
- a11y は該当しない(CLI なので WCAG 等は無関係)が、team-review の dim を全 5 軸で回すコストは微小なため a11y dim も流す(「該当なし」判定が findings としてログされる)。

### CLAUDE.md の "Sub-agent Self-Report is Untrusted" 対応

M6 は Medium tier で実装 LOC が 300 行前後、CLI layer は InputContext の公開 API のみを呼び出す thin layer。それでも以下を厳守:

1. lefthook pre-push が `cargo build --workspace` + `cargo clippy --workspace --all-targets -- -D warnings` + `cargo test --workspace` を走らせる(`lefthook.yml` 既存設定で自動適用、本 plan で変更なし)。
2. implementer sub-agent の report は build / test / clippy の verbatim head + tail を含めること。特に Task M6-7 で強制する。
3. main-agent は merge 前に `cargo build --workspace` + `cargo test --workspace` を自力で 1 回走らせる(Task M6-9 Step 2)。
4. `--no-verify` は使用しない。

---

## 目次(本 plan 内のナビゲーション)

- [Scope check / PR split rationale](#scope-check--pr-split-rationale)
- [PR #1 — M6: kotoha-cli binary + phase0-smoke.sh](#pr-1--m6-kotoha-cli-binary--phase0-smokesh)
  - [M6 完了条件](#m6-完了条件)
  - [ファイル構成 (M6)](#ファイル構成-m6)
  - Task M6-0: ISSUE 作成 + branch 作成
  - Task M6-1: workspace `Cargo.toml` の編集(members + clap 追加)
  - Task M6-2: `crates/kotoha-cli/Cargo.toml` を Write で作成
  - Task M6-3: `crates/kotoha-cli/src/lib.rs` にライン処理純粋関数を TDD で実装
  - Task M6-4: `crates/kotoha-cli/src/bin/romaji.rs` に clap + stdin ループを実装
  - Task M6-5: 最初のビルド検証 + 手動 smoke
  - Task M6-6: `scripts/phase0-smoke.sh` を Write で作成
  - Task M6-7: workspace 全体検証 + lefthook pre-push
  - Task M6-8: push + PR 作成
  - Task M6-9: review + findings 解消 + merge
  - Task M6-10: WBS ログ作成 + develop 直接 push
- [フォローアップ](#フォローアップ)

---

## PR #1 — M6: kotoha-cli binary + phase0-smoke.sh

**Goal:** `crates/kotoha-cli` workspace member を新設し、stdin 行単位で `InputContext` を駆動する `kotoha-romaji` バイナリと、Spec §13.2 の 10 assertion を自動実行する `scripts/phase0-smoke.sh` を実装する。Phase 0 完了条件 §13.1 / §13.2 を本 PR で満たす。

### M6 完了条件

- [ ] GitHub ISSUE(M6)が作成され、merge 済み PR で close される
- [ ] `cargo build -p kotoha-cli` が PASS し、`target/debug/kotoha-romaji` バイナリが生成される
- [ ] `cargo test -p kotoha-cli` が PASS し、ライン処理純粋関数の単体テスト(6 件以上)が全 PASS する
- [ ] `cargo test --workspace` 全体 PASS(M2 + M3a + M3b + M4b + M4c + M6 の単体テスト合算)
- [ ] `cargo build --workspace` PASS
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` warnings ゼロ
- [ ] `cargo fmt --all --check` diff ゼロ
- [ ] lefthook pre-commit + pre-push 全 PASS
- [ ] `scripts/phase0-smoke.sh` が exit 0 で終了し、Spec §13.2 の 10 assertion が全 PASS
- [ ] `scripts/phase0-smoke.sh` が実行可能属性(`chmod +x`)を持つ
- [ ] PR review (Medium tier) で Critical / High findings ゼロ
- [ ] WBS ログ `docs/wbs/2026-04-XX-feature-N-kotoha-cli.md` が develop に push 済み

### ファイル構成 (M6)

新規作成:

- `crates/kotoha-cli/Cargo.toml` — binary crate の manifest、clap dependency と kotoha-core path dependency
- `crates/kotoha-cli/src/lib.rs` — ライン処理純粋関数 `process_line` と `format_line_output` + `#[cfg(test)] mod tests`
- `crates/kotoha-cli/src/bin/romaji.rs` — clap derive による CLI 引数パース + stdin BufRead ループ + `lib.rs` の関数呼び出し
- `scripts/phase0-smoke.sh` — Spec §13.2 の 10 assertion を `cargo run` + `diff` で検証

変更:

- `Cargo.toml`(workspace root)
  - `[workspace].members` に `"crates/kotoha-cli"` を追加
  - `[workspace.dependencies]` に `clap = { version = "4.5", features = ["derive"] }` を追加

変更ファイル総数: 新規 4 + 変更 1 = **5 ファイル**。Branch Scope Policy ≤ 10 files を余裕で満たす。

---

### Task M6-0: ISSUE 作成 + branch 作成

**Files:** (GitHub 操作と branch 作成のみ)

- [ ] **Step 1: 作業ディレクトリと develop の最新化**

Run:

```bash
cd /home/kohshiro/develops/student/kotoha-ime
git checkout develop
git pull
git status
git log --oneline -3
```

Expected: `develop` が `origin/develop` と同期、working tree clean、最新 commit は `aec000f docs(wbs): #25 rule coverage gap — implementation log` 以降。

- [ ] **Step 2: M6 用 ISSUE を作成**

Run:

```bash
gh issue create \
  --title "M6 (impl): kotoha-cli — kotoha-romaji binary + phase0-smoke.sh" \
  --body "Phase 0 Milestone 6: implement the manual-verification CLI and the smoke-test script that covers spec §13.2.

## Scope

- New workspace member \`crates/kotoha-cli\` (binary crate, produces \`kotoha-romaji\`)
- \`crates/kotoha-cli/Cargo.toml\`: clap 4.5 with derive feature + kotoha-core path dependency
- \`crates/kotoha-cli/src/lib.rs\`: pure \`process_line\` / \`format_line_output\` helpers + unit tests (6+ cases covering Hiragana, Shift trigger with auto-return, Sticky Direct, --show-mode [H]/[D] suffix, empty line, final line without trailing newline)
- \`crates/kotoha-cli/src/bin/romaji.rs\`: clap derive Parser + stdin BufRead::lines() loop dispatching to \`process_line\`
- \`scripts/phase0-smoke.sh\`: 10 assertions from spec §13.2 run via \`cargo run -p kotoha-cli --bin kotoha-romaji --\` + diff comparison
- Workspace root \`Cargo.toml\`: add \`crates/kotoha-cli\` to members, add clap to [workspace.dependencies]

## Behavior pins (locked in plan §『本 M6 plan 内で解決する既知の懸念』)

- preedit NOT surfaced to stdout (per spec §10.4 REPL is out of scope for Phase 0)
- invalid chars dropped silently (matches spec §9.3.2 InputContext internal behavior)
- --show-mode suffix uses [H]/[D] without Transient/Sticky distinction (ModeOrigin is pub(crate))
- --mode direct → InputContext::set_mode(InputMode::Direct) once before the stdin loop (Sticky origin, persists across commits)
- Exit codes follow clap defaults (exit 2 on arg parse error); spec §10.5 remap deferred to M7/ADR 0004 scope

## Depends on

- M4b (merged): InputContext public API (new / set_mode / input_char / commit / mode)
- M4c (merged): mode golden + property verification of InputContext behavior

## Acceptance

- cargo build -p kotoha-cli passes, target/debug/kotoha-romaji is produced
- cargo test -p kotoha-cli passes with 6+ unit tests
- cargo test --workspace passes overall (no regression)
- cargo clippy --workspace --all-targets -- -D warnings: zero warnings
- cargo fmt --all --check: no diff
- lefthook pre-commit + pre-push all PASS
- scripts/phase0-smoke.sh exits 0 and reports 10 assertions PASS

## Out of Scope

- ADR 0004 (cli-line-based-commit) — reserved for M7 per the canonical numbering
- Interactive REPL (\`:toggle\` etc.) — spec §10.4 Phase 0 non-goal
- Mid-line explicit toggle — spec §10.4 Phase 0 non-goal
- Keycode emulation — Phase 3 IBus engine

## Reference

- Spec: docs/superpowers/specs/2026-04-22-kotoha-phase-0-design.md §10 / §13.2
- Plan: docs/superpowers/plans/2026-04-23-kotoha-phase-0-m6.md
- Parent tracking ISSUE: #32 (Phase 0 umbrella)"
```

Expected: ISSUE が作成され URL + 番号が表示される(以降 `N` と呼ぶ)。

- [ ] **Step 3: branch 作成**

Run(`N` は Step 2 の ISSUE 番号):

```bash
git checkout -b feature/N-kotoha-cli develop
git branch --show-current
```

Expected: `feature/N-kotoha-cli`。

---

### Task M6-1: workspace `Cargo.toml` の編集(members + clap 追加)

**Files:**
- Modify: `Cargo.toml`(workspace root)

**方針:** M6 着手前の workspace root `Cargo.toml` は L3〜L6 に `[workspace].members = [ "crates/kotoha-core", # crates/kotoha-cli は M6 で追加 ]` がコメント付きで記述され、L14〜L18 に `[workspace.dependencies]` が存在する。本 Task で M6 想定のコメントをほどいて member を追加し、clap を workspace dependency に登録する。

- [ ] **Step 1: `[workspace].members` を更新**

Edit `Cargo.toml`:

old_string(正確に一致):

```toml
[workspace]
resolver = "2"
members = [
    "crates/kotoha-core",
    # crates/kotoha-cli は M6 で追加
]
```

new_string:

```toml
[workspace]
resolver = "2"
members = [
    "crates/kotoha-core",
    "crates/kotoha-cli",
]
```

- [ ] **Step 2: `[workspace.dependencies]` に clap 追加**

Edit `Cargo.toml`:

old_string:

```toml
[workspace.dependencies]
thiserror = "1"
anyhow = "1"
tracing = "0.1"
tracing-subscriber = "0.3"
proptest = "1.5"
```

new_string:

```toml
[workspace.dependencies]
thiserror = "1"
anyhow = "1"
tracing = "0.1"
tracing-subscriber = "0.3"
proptest = "1.5"
clap = { version = "4.5", features = ["derive"] }
```

- [ ] **Step 3: 現時点で `crates/kotoha-cli` が存在しないため、cargo コマンドはまだ実行しない**

`crates/kotoha-cli` ディレクトリ + `Cargo.toml` が存在しないと workspace 解析に失敗するため、本 Step では `cargo metadata` 等を走らせない。次 Task M6-2 で crate skeleton を作ってから検証する。

Note: 本 Task 単体でのコミットは作らない。Task M6-2 完了後にまとめて `git add Cargo.toml crates/kotoha-cli/` する。

---

### Task M6-2: `crates/kotoha-cli/Cargo.toml` を Write で作成

**Files:**
- Create: `crates/kotoha-cli/Cargo.toml`

**方針:** binary crate として `src/lib.rs` も併設する(ライン処理純粋関数を単体テストするため)。`kotoha-core` は workspace 内 path dependency として参照。

- [ ] **Step 1: ディレクトリ作成**

Run:

```bash
mkdir -p crates/kotoha-cli/src/bin
ls -la crates/kotoha-cli
```

Expected: `crates/kotoha-cli/src/bin/` ディレクトリが作成され、空。

- [ ] **Step 2: `crates/kotoha-cli/Cargo.toml` を Write で作成**

Write ツールで以下を作成:

```toml
[package]
name = "kotoha-cli"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true
repository.workspace = true
authors.workspace = true
description = "Kotoha: command-line frontend for manual verification of the Kotoha Japanese IME core"

[dependencies]
kotoha-core = { path = "../kotoha-core" }
clap = { workspace = true }

[[bin]]
name = "kotoha-romaji"
path = "src/bin/romaji.rs"

# src/lib.rs は process_line / format_line_output を unit-testable に切り出すために存在する。
# binary 側は `use kotoha_cli::process_line;` 等でライブラリ層を参照する。
```

Note: `kotoha_cli` library target は暗黙に `src/lib.rs` から生成される(Cargo のデフォルト)。

- [ ] **Step 3: `cargo metadata` で workspace 解析が成立することを確認**

Run:

```bash
cargo metadata --no-deps --format-version=1 > /dev/null
```

Expected: エラーなし。ただし `src/lib.rs` と `src/bin/romaji.rs` の実体がまだないため、次 Task M6-3 / M6-4 でファイルを作るまで `cargo build` は走らせない。

---

### Task M6-3: `crates/kotoha-cli/src/lib.rs` にライン処理純粋関数を TDD で実装

**Files:**
- Create: `crates/kotoha-cli/src/lib.rs`

**方針:** CLI binary から切り出した純粋関数 2 つをライブラリ層に置き、単体テスト対象にする:

- `process_line(ctx: &mut InputContext, line: &str) -> String` — 行文字列の各 char を `input_char` に流し、per-char で得た `InputStep::Committed(s)` を連結、行末で `commit()` を呼んで末尾に追加。`InputStep::Preedit` / `InputStep::Invalid` は silently drop(plan "既知懸念 1" 参照)。
- `format_line_output(committed: &str, mode: InputMode, show_mode: bool) -> String` — 確定済み文字列に `--show-mode` suffix(` [H]` / ` [D]`)を条件付きで付加して返す。末尾 `\n` はここでは付けない(呼び出し側の `println!` が担う)。

本 Task は TDD で進める。Red (テスト先行) → Green (実装) → Refactor の 3 step を 1 サイクルとし、6 件以上のテストを順に green にする。

#### テスト 6 件(最低カバレッジ)

| # | name | 観点 | 入力 | 期待 |
|---|---|---|---|---|
| T1 | `process_line_hiragana_basic` | 基本 Hiragana 変換 | 初期 `InputContext` + `"konnnichiha"` | `"こんにちは"` + mode 終端 `Hiragana` |
| T2 | `process_line_shift_trigger_auto_returns` | Transient 自動復帰 | 初期 + `"Hello"` | `"Hello"` + mode 終端 `Hiragana`(自動復帰) |
| T3 | `process_line_sticky_direct_persists` | Sticky Direct 持続 | `set_mode(Direct)` 後 + `"hello"` | `"hello"` + mode 終端 `Direct` |
| T4 | `process_line_empty_is_empty` | 空行 | 初期 + `""` | `""` + mode 終端 `Hiragana` |
| T5 | `format_line_output_with_hiragana_shows_H` | `[H]` suffix | `"こん"` + `Hiragana` + `show_mode=true` | `"こん [H]"` |
| T6 | `format_line_output_with_direct_shows_D` | `[D]` suffix | `"hi"` + `Direct` + `show_mode=true` | `"hi [D]"` |

追加(余裕があれば 8〜10 件まで拡張):

| # | name | 観点 |
|---|---|---|
| T7 | `format_line_output_without_show_mode_no_suffix` | suffix なし経路 |
| T8 | `process_line_carries_mode_between_calls` | 行間モード継承(2 行連続呼び出しで 1 行目終端 mode = 2 行目開始 mode) |
| T9 | `process_line_invalid_chars_are_dropped` | invalid char silent drop |

- [ ] **Step 1: Red — `src/lib.rs` にテストファースト骨格を Write**

Write ツールで `crates/kotoha-cli/src/lib.rs` を以下の内容で新規作成(関数は `unimplemented!()` で開始):

```rust
//! Line-based helpers for the `kotoha-romaji` CLI binary.
//!
//! This crate exposes two pure functions ([`process_line`] and
//! [`format_line_output`]) that the binary entry point
//! (`src/bin/romaji.rs`) calls. Keeping the logic pure keeps the
//! binary thin and the behavior covered by unit tests without spawning
//! a subprocess.
//!
//! # Behavior pins (see plan M6 §『本 M6 plan 内で解決する既知の懸念』)
//!
//! - [`InputStep::Preedit`] is dropped silently (Phase 0 CLI is not a
//!   REPL; spec §10.4 defers interactive preedit to Phase 3).
//! - [`InputStep::Invalid`] is dropped silently (matches spec §9.3.2
//!   `InputContext` internal behavior).
//! - `--show-mode` suffix uses `[H]` for [`InputMode::Hiragana`] and
//!   `[D]` for [`InputMode::Direct`] with no Transient / Sticky
//!   distinction (`ModeOrigin` is `pub(crate)`).

use kotoha_core::{InputContext, InputMode, InputStep};

/// Feeds every char of `line` through the mode state machine, then
/// commits at end-of-line. Returns the committed-per-line string,
/// excluding any trailing newline.
///
/// # Preconditions
/// - `ctx` is in any valid state (the caller guarantees the state-tuple
///   invariant documented on [`InputContext`]).
///
/// # Postconditions
/// - `ctx` has its per-line buffer drained (Hiragana pending flushed,
///   Direct buffer cleared).
/// - If `ctx.mode()` was `(Direct, Transient)` at the start of the
///   call and a commit actually occurred, the post-call mode is
///   [`InputMode::Hiragana`] per ADR 0002.
pub fn process_line(ctx: &mut InputContext, line: &str) -> String {
    let mut out = String::new();
    for ch in line.chars() {
        match ctx.input_char(ch) {
            InputStep::Committed(s) => out.push_str(&s),
            InputStep::Preedit | InputStep::Invalid(_) => {
                // Intentionally silent per plan M6 既知懸念 1.
            }
        }
    }
    out.push_str(&ctx.commit());
    out
}

/// Formats the per-line CLI output with the optional mode suffix.
///
/// # Preconditions
/// - `committed` is the value returned from [`process_line`] (no trailing newline).
///
/// # Postconditions
/// - If `show_mode == false`, returns `committed` verbatim.
/// - If `show_mode == true`, appends ` [H]` for [`InputMode::Hiragana`]
///   or ` [D]` for [`InputMode::Direct`] (separator = single ASCII space).
pub fn format_line_output(committed: &str, mode: InputMode, show_mode: bool) -> String {
    if !show_mode {
        return committed.to_string();
    }
    let tag = match mode {
        InputMode::Hiragana => "H",
        InputMode::Direct => "D",
        _ => "?", // `InputMode` is `#[non_exhaustive]`; defensive fallback.
    };
    format!("{committed} [{tag}]")
}

#[cfg(test)]
mod tests {
    use super::*;

    // T1
    #[test]
    fn process_line_hiragana_basic() {
        let mut ctx = InputContext::new();
        let out = process_line(&mut ctx, "konnnichiha");
        assert_eq!(out, "こんにちは");
        assert_eq!(ctx.mode(), InputMode::Hiragana);
    }

    // T2
    #[test]
    fn process_line_shift_trigger_auto_returns() {
        let mut ctx = InputContext::new();
        let out = process_line(&mut ctx, "Hello");
        assert_eq!(out, "Hello");
        // Transient Direct → commit auto-returns to (Hiragana, Sticky).
        assert_eq!(ctx.mode(), InputMode::Hiragana);
    }

    // T3
    #[test]
    fn process_line_sticky_direct_persists() {
        let mut ctx = InputContext::new();
        ctx.set_mode(InputMode::Direct);
        let out = process_line(&mut ctx, "hello");
        assert_eq!(out, "hello");
        // Sticky Direct → commit does NOT auto-return.
        assert_eq!(ctx.mode(), InputMode::Direct);
    }

    // T4
    #[test]
    fn process_line_empty_is_empty() {
        let mut ctx = InputContext::new();
        let out = process_line(&mut ctx, "");
        assert_eq!(out, "");
        assert_eq!(ctx.mode(), InputMode::Hiragana);
    }

    // T5
    #[test]
    fn format_line_output_with_hiragana_shows_H() {
        let s = format_line_output("こん", InputMode::Hiragana, true);
        assert_eq!(s, "こん [H]");
    }

    // T6
    #[test]
    fn format_line_output_with_direct_shows_D() {
        let s = format_line_output("hi", InputMode::Direct, true);
        assert_eq!(s, "hi [D]");
    }

    // T7
    #[test]
    fn format_line_output_without_show_mode_no_suffix() {
        let s = format_line_output("こん", InputMode::Hiragana, false);
        assert_eq!(s, "こん");
    }

    // T8
    #[test]
    fn process_line_carries_mode_between_calls() {
        let mut ctx = InputContext::new();
        ctx.set_mode(InputMode::Direct);
        // Line 1 in Sticky Direct.
        let o1 = process_line(&mut ctx, "hello");
        assert_eq!(o1, "hello");
        assert_eq!(ctx.mode(), InputMode::Direct);
        // Line 2 also in Sticky Direct.
        let o2 = process_line(&mut ctx, "world");
        assert_eq!(o2, "world");
        assert_eq!(ctx.mode(), InputMode::Direct);
    }

    // T9
    #[test]
    fn process_line_invalid_chars_are_dropped_silently() {
        // Non-ASCII single char in Hiragana mode: InputContext drops it
        // via Invalid(char) path (spec §9.3.2). CLI must not surface it.
        let mut ctx = InputContext::new();
        let out = process_line(&mut ctx, "\u{3042}a"); // U+3042 'あ' then 'a'
        // 'あ' is Invalid in Hiragana context, dropped. 'a' → あ.
        // So output = "あ" (from the 'a' rule).
        assert_eq!(out, "あ");
    }
}
```

Note: T9 の assertion 値は kotoha-core の実際挙動に依存するため、実装後 `cargo test` の出力で妥当性を確認する。仕様として「invalid はサイレント drop される」ことが保証されていれば良く、出力文字列そのものは test を走らせて確認する(§9.3.2 の normalize 挙動により、日本語 U+3042 の後続 push が pending tail flush を起こす可能性があるため)。もし T9 が spec と乖離する場合は T9 の expected を実測値に合わせ、plan を追従更新する(TDD の "test が正しくなければ test を直す" 原則)。

- [ ] **Step 2: Green — テスト実行で 6 件以上 PASS することを確認**

Run:

```bash
cargo test -p kotoha-cli --lib 2>&1 | tail -40
```

Expected: `test result: ok.` で 9 件(T1〜T9)PASS。

失敗時(例: T9 が実測値と合わない):
1. `cargo test -p kotoha-cli --lib 2>&1 | grep -A 5 'FAILED\|panicked'` で期待値と実測値を取得。
2. 実装が spec §9.3.2 に則っているかレビュー(`kotoha-core/src/input/context.rs` の Hiragana path が `normalize_pending` を呼んでいるか等)。
3. spec 側が正しいなら expected を実測値に合わせて test を修正し、Step 2 をリトライ。

- [ ] **Step 3: Refactor — clippy 警告ゼロ + fmt diff ゼロ**

Run:

```bash
cargo clippy -p kotoha-cli --all-targets -- -D warnings 2>&1 | tail -10
cargo fmt --all --check
```

Expected: 両コマンド exit 0。

失敗時: clippy が指摘する idiom violation(例: `String::new()` と `to_string()` の一貫性、`match` と `if let` の選択)を修正して Step 2 に戻る。

---

### Task M6-4: `crates/kotoha-cli/src/bin/romaji.rs` に clap + stdin ループを実装

**Files:**
- Create: `crates/kotoha-cli/src/bin/romaji.rs`

**方針:** clap 4.5 derive Parser で `--mode` / `--show-mode` を受け、stdin を `std::io::stdin().lock().lines()` で行単位読み込み、各行を `kotoha_cli::process_line` → `kotoha_cli::format_line_output` → `println!` の 3 stage で処理する。`--mode direct` 指定時は stdin ループに入る前に `InputContext::set_mode(InputMode::Direct)` を 1 回呼ぶ(plan 既知懸念 3)。

- [ ] **Step 1: `src/bin/romaji.rs` を Write で作成**

Write ツールで `crates/kotoha-cli/src/bin/romaji.rs` を以下の内容で新規作成:

```rust
//! `kotoha-romaji` — Phase 0 manual-verification CLI for the Kotoha IME.
//!
//! Reads stdin line by line, feeds each char through
//! [`kotoha_core::InputContext`], and prints the committed string (with
//! an optional mode suffix) to stdout.
//!
//! See spec §10 for the full contract and spec §13.2 for the 10
//! acceptance assertions covered by `scripts/phase0-smoke.sh`.

use std::io::{self, BufRead, Write};
use std::process::ExitCode;

use clap::{Parser, ValueEnum};
use kotoha_cli::{format_line_output, process_line};
use kotoha_core::{InputContext, InputMode};

/// CLI mode selector.
///
/// Mirrors [`kotoha_core::InputMode`] but is declared locally so we can
/// attach `#[derive(ValueEnum)]`. The conversion to the library type
/// happens in [`Cli::initial_mode`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum CliMode {
    /// Romaji → kana conversion mode (default).
    Hiragana,
    /// Direct ASCII / punctuation passthrough mode (Sticky origin).
    Direct,
}

impl CliMode {
    fn into_input_mode(self) -> InputMode {
        match self {
            CliMode::Hiragana => InputMode::Hiragana,
            CliMode::Direct => InputMode::Direct,
        }
    }
}

/// Phase 0 CLI for manual verification of Kotoha's romaji → kana core.
#[derive(Debug, Parser)]
#[command(
    name = "kotoha-romaji",
    version,
    about = "Phase 0 CLI for manual verification of Kotoha's romaji → kana core"
)]
struct Cli {
    /// Initial input mode.
    #[arg(short = 'm', long = "mode", value_enum, default_value_t = CliMode::Hiragana)]
    mode: CliMode,

    /// Append ` [H]` or ` [D]` to each output line to show the current mode.
    #[arg(long = "show-mode", default_value_t = false)]
    show_mode: bool,
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    let mut ctx = InputContext::new();
    // `--mode direct` sets Sticky Direct before entering the stdin loop
    // so commit() does NOT auto-return to Hiragana (plan 既知懸念 3).
    if cli.mode == CliMode::Direct {
        ctx.set_mode(cli.mode.into_input_mode());
    }

    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut stdout = stdout.lock();

    for line_result in stdin.lock().lines() {
        let line = match line_result {
            Ok(l) => l,
            Err(e) => {
                eprintln!("kotoha-romaji: failed to read from stdin: {e}");
                // Spec §10.5 maps read failure to exit 1. (Arg-parse errors
                // exit 2 via clap default; see plan 既知懸念 4.)
                return ExitCode::from(1);
            }
        };
        let committed = process_line(&mut ctx, &line);
        // `--show-mode` reflects the mode AFTER commit (i.e., what the
        // next line would start in). Transient → Hiragana auto-return
        // has already happened inside process_line.
        let formatted = format_line_output(&committed, ctx.mode(), cli.show_mode);
        if let Err(e) = writeln!(stdout, "{formatted}") {
            eprintln!("kotoha-romaji: failed to write to stdout: {e}");
            return ExitCode::from(1);
        }
    }

    ExitCode::SUCCESS
}
```

- [ ] **Step 2: ビルド確認**

Run:

```bash
cargo build -p kotoha-cli 2>&1 | tail -20
```

Expected: `Compiling kotoha-cli ...` + `Finished ...`。warnings ゼロ。

- [ ] **Step 3: clippy 確認**

Run:

```bash
cargo clippy -p kotoha-cli --all-targets -- -D warnings 2>&1 | tail -10
```

Expected: warnings ゼロ。

- [ ] **Step 4: fmt 確認**

Run:

```bash
cargo fmt --all --check
```

Expected: diff なし。

---

### Task M6-5: 最初のビルド検証 + 手動 smoke

**Files:** (検証のみ、変更なし)

**方針:** smoke script を書く前に、Spec §13.2 の 10 assertion のうち代表 3 件(基本 Hiragana、Shift 自動復帰、`--mode direct`)を手動で回し、バイナリが想定通り動くことを確認する。ここでバグが発覚したら smoke script を書く前に修正する(smoke script は「すでに動いている CLI の regression テスト」として機能する、バグ探索の道具ではない)。

- [ ] **Step 1: 手動 smoke 1(基本 Hiragana)**

Run:

```bash
echo "konnnichiha" | cargo run -p kotoha-cli --bin kotoha-romaji -- 2>/dev/null
```

Expected(stdout 1 行): `こんにちは`

- [ ] **Step 2: 手動 smoke 2(Shift トリガ + 自動復帰)**

Run:

```bash
printf "Ko\nkonnnichiha\n" | cargo run -p kotoha-cli --bin kotoha-romaji -- 2>/dev/null
```

Expected(stdout 2 行):

```
Ko
こんにちは
```

- [ ] **Step 3: 手動 smoke 3(`--mode direct` + `--show-mode`)**

Run:

```bash
echo "hi" | cargo run -p kotoha-cli --bin kotoha-romaji -- --mode direct --show-mode 2>/dev/null
```

Expected(stdout 1 行): `hi [D]`

失敗時:
1. `process_line` / `format_line_output` / `main` のどの層でバグっているかを unit test に戻って隔離(`cargo test -p kotoha-cli` で T1〜T9 に加えて再現 test を追加)。
2. 実装を修正 → Task M6-3 Step 2 と M6-4 Step 2 のコマンドを再実行 → 再度本 Task に戻る。

---

### Task M6-6: `scripts/phase0-smoke.sh` を Write で作成

**Files:**
- Create: `scripts/phase0-smoke.sh`

**方針:** Spec §13.2 に列挙された 10 assertion を順次実行し、すべて PASS なら exit 0、1 件でも fail なら exit 1。`set -euo pipefail` + `trap` によるエラーメッセージ付き exit を採用。`cargo run -p kotoha-cli --bin kotoha-romaji --` で debug build を走らせ、初回呼び出しで compile、以降はキャッシュから起動(1 回 < 200ms)。

Spec §13.2 の 10 assertion(verbatim):

ローマ字→かな変換(4 件):
1. `echo "konnnichiha" | kotoha-romaji` → `こんにちは`
2. `echo "tsumugi" | kotoha-romaji` → `つむぎ`
3. `echo "n'ya" | kotoha-romaji` → `んや`
4. `echo "nya" | kotoha-romaji` → `にゃ`

モード管理(6 件):

5. `echo "HELLO" | kotoha-romaji` → `HELLO`
6. `printf "Ko\nkonnnichiha\n" | kotoha-romaji` → `Ko\nこんにちは\n`
7. `printf "hello\nworld\n" | kotoha-romaji --mode direct` → `hello\nworld\n`
8. `printf "Konnichiwa\nkonnnichiha\n" | kotoha-romaji` → `Konnichiwa\nこんにちは\n`
9. `echo "Hi" | kotoha-romaji --show-mode` → `Hi [H]`
10. `echo "hi" | kotoha-romaji --mode direct --show-mode` → `hi [D]`

- [ ] **Step 1: `scripts/phase0-smoke.sh` を Write で作成**

Write ツールで以下を作成:

```bash
#!/usr/bin/env bash
# Phase 0 smoke test — runs spec §13.2's 10 CLI acceptance assertions.
#
# Usage:
#   scripts/phase0-smoke.sh
#
# Exit codes:
#   0   — all 10 assertions PASS
#   1   — at least one assertion FAILED (or build error)
#
# Invocation model: uses `cargo run -p kotoha-cli --bin kotoha-romaji --`
# (debug build) per plan M6 §『本 M6 plan 内で解決する既知の懸念』. The
# first invocation triggers compilation; subsequent invocations re-use
# the cache and start in < 200 ms.
#
# Reference:
#   - Spec §13.2 (docs/superpowers/specs/2026-04-22-kotoha-phase-0-design.md)
#   - Plan  M6   (docs/superpowers/plans/2026-04-23-kotoha-phase-0-m6.md)

set -euo pipefail

cd "$(dirname "$0")/.."

FAIL_COUNT=0
PASS_COUNT=0
TOTAL=10

# Pre-build once so per-assertion timings are uniform (the first
# `cargo run` otherwise dominates the wall-clock of assertion #1).
echo "=== phase0-smoke: pre-building kotoha-cli (debug) ==="
cargo build -p kotoha-cli --quiet

RUN=(cargo run --quiet -p kotoha-cli --bin kotoha-romaji --)

assert_eq() {
    local name="$1"
    local expected="$2"
    local actual="$3"
    if [ "$actual" = "$expected" ]; then
        PASS_COUNT=$((PASS_COUNT + 1))
        printf 'PASS  %s\n' "$name"
    else
        FAIL_COUNT=$((FAIL_COUNT + 1))
        printf 'FAIL  %s\n' "$name"
        printf '      expected: %q\n' "$expected"
        printf '      actual:   %q\n' "$actual"
    fi
}

echo "=== phase0-smoke: running ${TOTAL} assertions ==="

# --- Romaji → kana (4 assertions, spec §13.2) -------------------------

A1=$(printf 'konnnichiha\n' | "${RUN[@]}")
assert_eq "1. konnnichiha → こんにちは"        "こんにちは" "$A1"

A2=$(printf 'tsumugi\n'     | "${RUN[@]}")
assert_eq "2. tsumugi → つむぎ"                  "つむぎ"     "$A2"

A3=$(printf "n'ya\n"        | "${RUN[@]}")
assert_eq "3. n'ya → んや"                       "んや"       "$A3"

A4=$(printf 'nya\n'         | "${RUN[@]}")
assert_eq "4. nya → にゃ"                        "にゃ"       "$A4"

# --- Mode management (6 assertions, spec §13.2) -----------------------

A5=$(printf 'HELLO\n'       | "${RUN[@]}")
assert_eq "5. HELLO (Shift trigger) → HELLO"     "HELLO"      "$A5"

EXPECTED6=$'Ko\nこんにちは'
A6=$(printf 'Ko\nkonnnichiha\n' | "${RUN[@]}")
assert_eq "6. Ko\\nkonnnichiha (Transient auto-return) → Ko\\nこんにちは" \
          "$EXPECTED6" "$A6"

EXPECTED7=$'hello\nworld'
A7=$(printf 'hello\nworld\n' | "${RUN[@]}" --mode direct)
assert_eq "7. hello\\nworld --mode direct (Sticky) → hello\\nworld" \
          "$EXPECTED7" "$A7"

EXPECTED8=$'Konnichiwa\nこんにちは'
A8=$(printf 'Konnichiwa\nkonnnichiha\n' | "${RUN[@]}")
assert_eq "8. Konnichiwa\\nkonnnichiha (mixed) → Konnichiwa\\nこんにちは" \
          "$EXPECTED8" "$A8"

A9=$(printf 'Hi\n'          | "${RUN[@]}" --show-mode)
assert_eq "9. Hi --show-mode → Hi [H]"           "Hi [H]"     "$A9"

A10=$(printf 'hi\n'         | "${RUN[@]}" --mode direct --show-mode)
assert_eq "10. hi --mode direct --show-mode → hi [D]" "hi [D]" "$A10"

# --- Summary ----------------------------------------------------------

echo "=== phase0-smoke: ${PASS_COUNT}/${TOTAL} PASS, ${FAIL_COUNT}/${TOTAL} FAIL ==="

if [ "$FAIL_COUNT" -gt 0 ]; then
    exit 1
fi

exit 0
```

Note on quoting:
- `printf '%q' ...` は bash の `$'...'` や UTF-8 リテラルをエスケープ表示するため、failure 時の expected / actual 比較ログが読みやすい。
- `EXPECTED6=$'Ko\nこんにちは'` は bash ANSI-C quoting で改行を 1 回だけ埋め込む。末尾の改行は bash の `$(...)` が自動で trim するため不要。
- `A6=$(printf 'Ko\nkonnnichiha\n' | ...)` の `\n` は `printf` が解釈する改行。CLI 側は `lines()` で 2 行として読む。

- [ ] **Step 2: ファイルに実行権限を付与**

Run:

```bash
chmod +x scripts/phase0-smoke.sh
ls -l scripts/phase0-smoke.sh
```

Expected: `-rwxr-xr-x` 相当の権限表示。

- [ ] **Step 3: smoke script 実行**

Run:

```bash
./scripts/phase0-smoke.sh
```

Expected:
- `=== phase0-smoke: pre-building kotoha-cli (debug) ===`
- `=== phase0-smoke: running 10 assertions ===`
- 10 行の `PASS  ...`
- `=== phase0-smoke: 10/10 PASS, 0/10 FAIL ===`
- exit 0

失敗時:
- どの assertion が fail したかを stderr ログから特定。
- 1 件のみの fail なら個別に `printf ... | cargo run ...` で再現確認 → `process_line` / `format_line_output` に unit test を追加して境界を特定。
- 複数 fail なら Task M6-3 / M6-4 のどこかに system 的な問題(例: ctx が行間で壊れている、format が suffix を付け間違えている)がある。

---

### Task M6-7: workspace 全体検証 + lefthook pre-push

**Files:** (検証のみ、変更なし)

**方針:** global CLAUDE.md "Sub-agent Self-Report is Untrusted" に従い、本 Task では main-agent が canonical build-and-test を自力で走らせ、verbatim head + tail を記録する。lefthook pre-push は pre-commit も含めて全 PASS でなければ次 Task に進まない。

- [ ] **Step 1: `cargo build --workspace` 実行 + verbatim head + tail 記録**

Run:

```bash
cargo build --workspace 2>&1 | tee /tmp/m6-build.log | tail -20
head -5 /tmp/m6-build.log
```

Expected:
- head: `Compiling ...` 系の出力開始行
- tail: `Finished ...` で PASS
- exit 0

- [ ] **Step 2: `cargo test --workspace` 実行 + verbatim head + tail 記録**

Run:

```bash
cargo test --workspace 2>&1 | tee /tmp/m6-test.log | tail -30
head -10 /tmp/m6-test.log
grep -c "test result:" /tmp/m6-test.log
```

Expected:
- test result 行が全 suite 分(kotoha-core lib + 各 integration test + kotoha-cli lib = 発足時点で 6〜8 件程度)
- 全 suite `ok.`
- M4c までの合計 + 本 M6 の 9 件で total test 件数が増加している

- [ ] **Step 3: `cargo clippy --workspace --all-targets -- -D warnings` 実行**

Run:

```bash
cargo clippy --workspace --all-targets -- -D warnings 2>&1 | tee /tmp/m6-clippy.log | tail -20
```

Expected:
- `Finished ...` で exit 0
- warnings ゼロ(`-D warnings` で warning = error)

- [ ] **Step 4: `cargo fmt --all --check` 実行**

Run:

```bash
cargo fmt --all --check
```

Expected: exit 0、diff なし。

- [ ] **Step 5: `scripts/phase0-smoke.sh` 再実行(regression 確認)**

Run:

```bash
./scripts/phase0-smoke.sh
```

Expected: 10/10 PASS、exit 0。

- [ ] **Step 6: lefthook pre-commit 実行**

Run:

```bash
lefthook run pre-commit
```

Expected:
- `fmt-check`: `.rs` ファイルが staged に含まれる場合 PASS。staged なしなら skip。
- `doc-naming`: staged の `docs/**/*.md` がないため skip、または staged あれば PASS(本 PR は doc 変更なし)。
- `doc-naming-self-test`: 常時 PASS。

- [ ] **Step 7: lefthook pre-push 実行**

Run:

```bash
lefthook run pre-push
```

Expected:
- `manifest-check` PASS
- `build` PASS
- `clippy` PASS (warnings ゼロ)
- `test` PASS

- [ ] **Step 8: Git staging 確認**

Run:

```bash
git status
git diff --stat
```

Expected(staging 前):
- modified: `Cargo.toml`
- untracked: `crates/kotoha-cli/Cargo.toml`, `crates/kotoha-cli/src/lib.rs`, `crates/kotoha-cli/src/bin/romaji.rs`, `scripts/phase0-smoke.sh`

---

### Task M6-8: push + PR 作成

**Files:** (push + PR 作成、変更なし)

- [ ] **Step 1: staged → commit**

Run:

```bash
git add Cargo.toml crates/kotoha-cli scripts/phase0-smoke.sh
git status
```

Expected: 5 files staged(Cargo.toml + crates/kotoha-cli/{Cargo.toml, src/lib.rs, src/bin/romaji.rs} + scripts/phase0-smoke.sh)。

```bash
git commit -m "feat(kotoha-cli): M6 — kotoha-romaji CLI + phase0-smoke.sh

Implement the Phase 0 manual-verification CLI and the smoke-test
script that covers spec §13.2's 10 acceptance assertions.

- New workspace member crates/kotoha-cli (binary crate, produces
  kotoha-romaji). clap 4.5 with derive feature is registered as a
  workspace dependency.
- crates/kotoha-cli/src/lib.rs exposes pure process_line and
  format_line_output helpers (9 unit tests covering Hiragana, Shift
  trigger with auto-return, Sticky Direct persistence, empty line,
  [H]/[D] suffix formatting, mode carry-over between lines, and
  silent invalid-char drop).
- crates/kotoha-cli/src/bin/romaji.rs wires clap derive Parser
  (--mode hiragana|direct, --show-mode) to a stdin BufRead::lines()
  loop that dispatches to process_line and emits output with
  writeln!.
- scripts/phase0-smoke.sh runs the 10 assertions via
  'cargo run -p kotoha-cli --bin kotoha-romaji --' (debug build) and
  diffs stdout against spec §13.2 expected values.

Behavior pins (plan M6 §『本 M6 plan 内で解決する既知の懸念』):
- preedit events NOT surfaced (spec §10.4 — no REPL in Phase 0)
- invalid chars dropped silently (matches InputContext §9.3.2)
- --show-mode suffix uses [H]/[D] with no Transient/Sticky split
- --mode direct calls InputContext::set_mode once before the loop
- exit codes follow clap defaults; spec §10.5 remap deferred to M7

Closes #N
Refs #32"
```

Expected: `5 files changed`。`#N` は M6-0 Step 2 の ISSUE 番号に置換する。

- [ ] **Step 2: push**

Run(`N` は Task M6-0 の ISSUE 番号):

```bash
git push -u origin feature/N-kotoha-cli
```

Expected: pre-push hook が自動実行、全 PASS 後に push 成功。

- [ ] **Step 3: PR 作成**

Run(`N` は Task M6-0 の ISSUE 番号):

```bash
gh pr create --base develop --head feature/N-kotoha-cli \
  --title "M6 (impl): kotoha-cli — kotoha-romaji binary + phase0-smoke.sh" \
  --body "$(cat <<'EOS'
## Summary

Phase 0 Milestone 6: implement the manual-verification CLI and the smoke-test
script that covers spec §13.2's 10 acceptance assertions.

- New workspace member \`crates/kotoha-cli\` (binary crate → \`kotoha-romaji\`)
- \`crates/kotoha-cli/src/lib.rs\`: \`process_line\` + \`format_line_output\` pure
  helpers (9 unit tests)
- \`crates/kotoha-cli/src/bin/romaji.rs\`: clap 4.5 derive Parser + stdin
  BufRead::lines() loop
- \`scripts/phase0-smoke.sh\`: 10 assertions from spec §13.2 via
  \`cargo run -p kotoha-cli --bin kotoha-romaji --\` + diff
- Workspace \`Cargo.toml\`: add \`crates/kotoha-cli\` to members, add clap to
  [workspace.dependencies]

## Behavior pins (from plan M6)

- preedit events NOT surfaced (spec §10.4: no REPL in Phase 0)
- invalid chars dropped silently (matches \`InputContext\` §9.3.2)
- \`--show-mode\` suffix uses \`[H]\` / \`[D]\` without Transient/Sticky distinction
- \`--mode direct\` calls \`InputContext::set_mode(InputMode::Direct)\` once
  before the stdin loop (Sticky origin persists across commits)
- exit codes follow clap defaults (arg-parse errors → 2); spec §10.5 remap
  deferred to M7 / ADR 0004 scope

## Deferred (Phase 0 non-goals per spec §10.4)

- Interactive REPL (\`:toggle\` etc.)
- Mid-line explicit toggle
- Keycode emulation (Phase 3 IBus engine)
- ADR 0004 (cli-line-based-commit) — canonical numbering reserves 0004 for M7

## Related

- Spec: docs/superpowers/specs/2026-04-22-kotoha-phase-0-design.md §10 / §13.2
- Plan: docs/superpowers/plans/2026-04-23-kotoha-phase-0-m6.md
- Parent tracking ISSUE: #32
- Closes #N

## Test plan

- [ ] \`cargo build --workspace\` passes
- [ ] \`cargo test --workspace\` passes (kotoha-core regression + 9 new
      kotoha-cli unit tests)
- [ ] \`cargo clippy --workspace --all-targets -- -D warnings\`: zero warnings
- [ ] \`cargo fmt --all --check\`: no diff
- [ ] \`scripts/phase0-smoke.sh\` exits 0 with 10/10 PASS
- [ ] lefthook pre-commit + pre-push all PASS
- [ ] Manual smoke: \`echo "konnnichiha" | cargo run -p kotoha-cli --bin
      kotoha-romaji --\` emits \`こんにちは\`
EOS
)"
```

Replace `#N` with the ISSUE number from Task M6-0. Expected: PR URL が表示される。

---

### Task M6-9: review + findings 解消 + merge

**Files:** (検証 + merge)

- [ ] **Step 1: PR review (Medium tier — impl、5 files、約 300〜390 行)**

CLAUDE.md の PR Review Matrix に従い Medium tier を選択。dimensions は security / performance / architecture / testing / a11y の 5 軸(a11y は CLI なので該当なし判定が出る想定):

Run (Skill 経由):

```
/agent-teams:team-review dimensions=security,performance,architecture,testing,a11y
/owasp-security
/secrets-check
```

Expected:
- Critical / High findings ゼロ。
- a11y dim は "CLI なので WCAG 等は該当しない" という non-issue finding を返す想定。
- security: clap の parse 時 panic 挙動、stdin 処理での buffer overflow がないこと、secrets (API key, token) が test fixture に混入していないことを verify。
- performance: debug build 起動 + 10 assertion 合計 < 5 秒で許容範囲。release build への切り替えは将来の ADR 候補(Phase 3 以降)。
- architecture: `lib.rs` / `bin/romaji.rs` の責務分離、`kotoha-core` との依存方向(CLI → core、逆流なし)を verify。`CliMode` と `InputMode` の二重定義は ValueEnum 制約による意図的なものと記録。
- testing: 9 unit tests が T1〜T9 を network cover しているか、smoke script の 10 assertion が spec §13.2 の literal に一致しているか、line-loop が stdin read failure path を test していないギャップ(intentional、integration test 不要範囲)を記録。

- [ ] **Step 2: findings 解消 + 再 review**

Critical / High ゼロになるまで実施。findings が出た場合は implementer subagent に修正依頼 → push → 再 review ループ。典型的に plan-per-task 粒度のコメント(e.g., "`CliMode` の variant 数が `InputMode` と乖離する可能性、将来的に Katakana / Zenkaku を Kotoha が追加した場合に CLI の `--mode` が follow しないリスク")に対しては comment 内で "Phase 1 以降で追加、`InputMode::{Hiragana, Direct}` 外のモードは Phase 0 の CLI には露出しない" と応答し、follow-up ISSUE を作成して close する。

- [ ] **Step 3: main-agent による spot-check build-and-test(CLAUDE.md mandatory mitigation #3)**

Run:

```bash
cargo build --workspace 2>&1 | tail -5
cargo test --workspace 2>&1 | tail -10
./scripts/phase0-smoke.sh
```

Expected: 全 PASS、10/10 smoke PASS。

- [ ] **Step 4: squash merge + branch 削除**

Run(`<PR番号>` は Task M6-8 Step 3 の出力):

```bash
gh pr merge <PR番号> --squash --delete-branch
gh pr view <PR番号> --json state,mergeCommit -q '{state, merge: .mergeCommit.oid}'
```

Expected: `state: MERGED`、merge commit SHA 取得。

- [ ] **Step 5: develop 追従**

Run:

```bash
git checkout develop
git pull
git log --oneline -3
```

Expected: squash merge commit が develop 先頭。

---

### Task M6-10: WBS ログ作成 + develop 直接 push

**Files:**
- Create: `docs/wbs/2026-04-XX-feature-N-kotoha-cli.md`

**方針:** CLAUDE.md「WBS 直接 push の例外」に従い、PR merge 後 develop に直接 push する。本 PR の code diff には含まれない post-merge 記録として機能する。

- [ ] **Step 1: Write で WBS 新規作成**

Write ツールで `docs/wbs/2026-04-XX-feature-N-kotoha-cli.md`(`N` は ISSUE 番号、`2026-04-XX` は実作業日、`<MERGE_COMMIT>` は Task M6-9 Step 4 の merge SHA、`<PR_NUMBER>` は PR 番号)を以下の内容で作成:

```markdown
---
milestone: M6
branch: feature/N-kotoha-cli
pr: "#<PR_NUMBER>"
merge_commit: "<MERGE_COMMIT>"
issue: "#N"
status: done
started: 2026-04-XX
finished: 2026-04-XX
---

# M6: kotoha-cli — kotoha-romaji binary + phase0-smoke.sh

## 実施内容

- 新規 workspace member `crates/kotoha-cli` を追加(binary crate、`kotoha-romaji`)
- `crates/kotoha-cli/src/lib.rs` に `process_line` / `format_line_output` の 2 pure helper を実装、単体テスト 9 件
- `crates/kotoha-cli/src/bin/romaji.rs` に clap 4.5 derive Parser + stdin BufRead::lines() ループを実装
- `scripts/phase0-smoke.sh` に Spec §13.2 の 10 assertion を実装、全 PASS
- workspace root `Cargo.toml` の `members` に `crates/kotoha-cli` を追加、`[workspace.dependencies]` に `clap = { version = "4.5", features = ["derive"] }` を追加

## behavior pin(plan M6 §『本 M6 plan 内で解決する既知の懸念』)

- preedit 事象は CLI 出力に surface しない(Spec §10.4 Phase 0 REPL 非対応の自然な帰結)
- invalid 文字は silent drop(Spec §9.3.2 の `InputContext` 挙動と一貫)
- `--show-mode` suffix は `[H]` / `[D]` のみ、Transient / Sticky 区別なし(`ModeOrigin` は `pub(crate)`)
- `--mode direct` 起点は `InputContext::set_mode` 1 回で Sticky Direct を確立、以降 commit で auto-return しない
- exit code は clap デフォルトに従う(引数エラー → exit 2)。Spec §10.5 の exit 1 への remap は M7 / ADR 0004 で検討する follow-up ISSUE として record

## つまずき

(実施時に記入。想定される typical:
- clap 4.5 の derive 属性で `#[arg(short = 'm', long = "mode", value_enum, default_value_t = ...)]` の default_value_t 構文を忘れた、等)

## M7 / Phase 3 への申し送り

- ADR 0004 (cli-line-based-commit) は M7 で作成。本 M6 の「行末 Enter commit」の設計判断は Phase 0 の CLI 限定であり、Phase 3 の IBus engine では keycode ベースのイベントモデルに置き換わる。CLI の line-loop を過剰一般化しない。
- Spec §10.5 の exit 0 / 1 / 2 規定と clap デフォルトの乖離は ADR 0004 の検討対象。現状は clap デフォルトに合わせて運用。
- `scripts/phase0-smoke.sh` の 10 assertion は Phase 0 完了確認の manual 手段として機能するが、CI がないため lefthook pre-push と別に手動実行を要する。M7 ADR 検討時に「smoke を lefthook pre-push に組み込むか」を評価する follow-up ISSUE を起こす。

## 成果物リンク

- PR: #<PR_NUMBER>
- ISSUE: #N
- Spec: `docs/superpowers/specs/2026-04-22-kotoha-phase-0-design.md` §10 / §13.2
- Plan: `docs/superpowers/plans/2026-04-23-kotoha-phase-0-m6.md`
- CLI crate: `crates/kotoha-cli/`
- Smoke script: `scripts/phase0-smoke.sh`
```

- [ ] **Step 2: WBS を develop に直接 push(CLAUDE.md の例外ルール適用)**

Run:

```bash
git add docs/wbs/2026-04-XX-feature-N-kotoha-cli.md
git commit -m "docs(wbs): M6 kotoha-cli — implementation log"
git push
```

Expected: lefthook pre-push 全 PASS。

---

## フォローアップ

本 M6 実装完了後に残る、Phase 0 完了 / Phase 1 着手に向けた follow-up items:

### M7 で作成する ADR(本 M6 では触れない)

- **ADR 0004 (cli-line-based-commit)**: Phase 0 の CLI が採用した「行末 = 1 Enter = 1 commit」という設計判断の根拠を記録する。想定内容:
  - 背景: Phase 0 では REPL / keycode emulation を非対応とした(Spec §10.4)。もっとも単純な abstraction は POSIX 由来の "1 行 = 1 コマンド" であり、stdin pipe で automated test が容易。
  - 代替案 1: バイト単位 streaming(Phase 3 の IBus keycode event に近いが CLI には過剰)
  - 代替案 2: `:commit` 等の meta command REPL(Phase 0 非対応と決定済)
  - 決定: 行末 = 1 commit を採用。Phase 3 の IBus engine はこの abstraction を使わず独自のキーコードイベントモデルに置き換わる。
- **Spec §10.5 と clap デフォルトの乖離処理**: ADR 0004 の影響分析として、exit 0 / 1 / 2 の規定を clap デフォルトに合わせる方向で更新するか、あるいは clap の error_handling を再マップするかを比較し、Phase 0 時点の判断を明記する。

### Phase 3 IBus engine への申し送り

- 本 M6 で確立した line-based commit abstraction は **Phase 0 限定の CLI 仕様**であり、Phase 3 の IBus engine は採用しない。IBus engine では IBusKeyEvent → `input_char` / `commit` / `cancel` / `toggle_mode` への変換層(Spec §17.1)を通じて state machine を駆動する。`process_line` / `format_line_output` の実装は CLI binary 側に閉じ込めておくことで、Phase 3 着手時に core 側への影響がないことを保証する。
- Phase 3 では stdin ではなく IBus のキーボード event stream を入力ソースとするため、`BufRead::lines()` ループは撤去される。本 M6 の binary コードは Phase 0 完了後もデバッグ tool として保持されるが、production path ではない。

### Phase 0 完了判定(本 PR merge 時点の進捗)

Spec §13 全体の成功基準に対する達成状況:

| 基準 | 本 PR merge 後 |
|---|---|
| §13.1 ビルド・テスト系(lefthook 自動検証) | **全 PASS**(M1〜M6 で順次達成) |
| §13.2 CLI 手動確認 10 件 + `phase0-smoke.sh` | **全 PASS**(本 PR) |
| §13.3 ドキュメント成果物(ADR 4 件、ROADMAP、付録 §17) | **未達成**(ADR 0003 / 0004 が M7 残存、ROADMAP 整備も M7) |
| §13.4 テスト成果物(fixture 件数、property 条件) | **達成済み**(M3b / M4c で達成、本 M6 では関与しない) |

**本 M6 merge 時点で Phase 0 完了ではない**。Phase 0 完了宣言には M7(ADR 0003 / 0004 + ROADMAP 更新 + README 整備)の完了が必要。

### 発生しうる follow-up ISSUE(本 M6 範囲外)

- **F1**: smoke script を lefthook pre-push に組み込むか検討(Medium tier)。組み込めば regression 検出が自動化されるが、pre-push 時間が増加する(現状 < 10 秒 → +2〜3 秒想定)。M7 ADR 0004 のコンテキストで検討。
- **F2**: `CliMode::Katakana` / `Zenkaku` 等への拡張は Phase 1 以降。本 M6 では `Hiragana` / `Direct` 2 種のみ。`InputMode` が `#[non_exhaustive]` のため、CLI 側も value_enum で future variants の追加余地を残す形で実装済み。
- **F3**: release build smoke (`cargo run --release -p kotoha-cli --bin kotoha-romaji --`)で performance 計測。Phase 0 範囲外だが、Phase 1 で kanji conversion が入ると latency が問題になる可能性があり、M6 段階で baseline を取っておく follow-up ISSUE を別途作成する選択肢あり。
- **F4**: CLI に `-h` / `-V` の help text が日本語化されていない(clap derive は English default)。OSS 公開を見据えると `about` / `long_about` を英語のまま維持するのが適切(CLAUDE.md Language 例外で rustdoc は英語推奨)であり、本 PR でも英語を維持する。日本語 help は Phase 5 以降で i18n 対応時に検討。

---

## Self-Review 済み事項

本 plan 書き起こし後のセルフレビューで確認した項目:

1. **型・API 一貫性**: kotoha-core 公開 API(`InputContext::{new, set_mode, input_char, commit, mode}`、`InputMode::{Hiragana, Direct}`、`InputStep::{Preedit, Committed(String), Invalid(char)}`)と本 plan の呼び出し記述は一致。
2. **Spec カバレッジ**: Spec §10 全節(10.1 コマンド / 10.2 動作仕様 / 10.3 使用例 / 10.4 非対応 / 10.5 exit code)を本 plan の Task + 既知懸念で full cover。§13.2 の 10 assertion は Task M6-6 Step 1 の smoke script で literal cover。
3. **Scope cap**: 5 files / 約 300〜390 LOC は CLAUDE.md Branch Scope Policy の Medium tier 実質上限近傍。超過時の split 案は PR split rationale 節で検討し、却下理由を明記。
4. **TDD 順序**: Task M6-3(unit tests)→ Task M6-4(binary)→ Task M6-5(manual smoke)→ Task M6-6(smoke script)の順で、最小ユニットから統合へ進む。unit test を smoke script 前に green にする理由は「smoke 失敗時の diagnosis が unit 側に降りず、binary 側の issue か分かるため」。
5. **CLAUDE.md 遵守**:
   - 非 `--no-verify` push(Task M6-8 Step 2)
   - verbatim head + tail 記録(Task M6-7 Step 1〜3)
   - main-agent spot-check(Task M6-9 Step 3)
   - WBS 直接 push(Task M6-10 Step 2、CLAUDE.md の例外ルール)
6. **Language convention**: Kotoha 例外により commit message / PR title / body / ISSUE title / body / rustdoc は英語、plan / WBS / 内部コメントは日本語。
7. **依存追加の記録**: `clap = { version = "4.5", features = ["derive"] }` の採用理由は (a) Rust CLI のデファクトスタンダード、(b) derive API により boilerplate 最小、(c) `ValueEnum` derive で `--mode hiragana|direct` のバリデーションを型で担保、の 3 点。代替案 `argh` / `lexopt` を却下する理由は機能網羅性と community adoption の観点で劣るため。PR body の "## Related" 節で言及する。
8. **Deferred items の record**: ADR 0004 は M7 で作成(canonical numbering が 0004 を M7 に予約済み)、Spec §10.5 remap も M7 で検討。本 PR で ADR を作らないことは PR body と WBS で明記する。
