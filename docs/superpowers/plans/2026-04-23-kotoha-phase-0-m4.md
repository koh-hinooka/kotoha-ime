# Phase 0 Milestone 4 (M4: input module) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `input` モジュール (`InputMode` / `ModeOrigin` / `InputContext` / `InputStep`) + モード状態機械 + 関連テスト (unit 20 件 + mode golden 80 ケース + property 4 条件) + ADR 0002 を実装し、Phase 0 の「入力モード管理」レイヤーを完成させる。

**Architecture:** `crates/kotoha-core/src/input/{mod,mode,context}.rs` の 3 ファイル構成。`mode.rs` は `InputMode` (`pub`) + `ModeOrigin` (`pub(crate)`) の enum 定義のみ、`context.rs` は `InputContext` 状態機械と `InputStep` enum (`pub`)、`mod.rs` は re-export 整理。M3 の `RomajiConverter` を Hiragana モードの delegate として保持し、Direct モードは `String` direct_buffer で管理する。

**Tech Stack:** Rust 2021 / MSRV 1.80、kotoha-core crate、proptest 1.5 (M3b で workspace 配線済)、thiserror (既存)。新規依存は追加しない。

**Spec:** `docs/superpowers/specs/2026-04-22-kotoha-phase-0-design.md` §7.3-§7.6、§8 全体、§11.1 (単体 50 件以上)、§11.2 (mode golden 70+10)、§11.3 (property 4 条件追加)。

**Phase 0 全体計画:** `docs/superpowers/plans/2026-04-22-kotoha-phase-0-implementation.md`(M1/M2/M3 完了、本 M4 が次のマイルストーン)。

**前提となる M3 成果:**

- `RomajiConverter::{new, convert, push, reset, flush}` 公開 API。
- `ConvertStep::{Committed(Cow<'static, str>), Pending, Invalid(char)}` enum(`non_exhaustive`)。
- 内部 `StateMachine::normalize(&mut self) -> String`(`pub(crate)`)は存在するが公開されていない。本 plan M4b-3 で `RomajiConverter::normalize_pending(&mut self) -> String` をラップ公開する。

**本 M4 plan 内で解決する既知の懸念:**

- PR #30 WBS (`docs/wbs/2026-04-23-bugfix-29-convert-mid-stream-normalize.md`) で確認した streaming push path の mid-stream normalize ギャップに対応する。`RomajiConverter::push` 自身は内部 `StateMachine::settle` を 1 回だけ走らせるため、invalid drop 後に残る残差(例: `push('b'); push('!')` 実行後の buffer `"!"`)を確定しない。本 plan では `RomajiConverter` に新規 `normalize_pending` 公開メソッドを追加し、`InputContext::input_char` は Hiragana モードで毎回 `push` 直後に `normalize_pending` を呼ぶことでこのギャップを閉じる。
- Spec §8.5 (line 406) が `docs/adr/0001-input-mode-transient-vs-sticky.md` を参照しているが、0001 は PR #28 により「非 ASCII retraction policy」に既に割り当て済である。本 plan M4a で ADR を `0002-input-mode-transient-vs-sticky.md` として新設し、spec §8.5 の参照番号も 0001 → 0002 へ修正する。

---

## Scope check / PR split rationale

M4 の工数見積は 1.5〜2 日 (spec §14「input モジュール」+ 1.5 日「golden test + property test」の input 分) であり、project CLAUDE.md の Branch Scope Policy (1 branch ≤ 2 日 / ≤ 10 files / ≤ 300 lines per PR) を超える。以下 3 PR に分割する。

| PR | 期間 | スコープ | 理由 |
|---|---|---|---|
| **M4a** (docs) | 約 0.2 日 | ADR 0002 新設 + spec §8.5 参照番号修正 | docs-only。本体実装 (M4b) から独立させ、先にマージすることで M4b commit message / PR body / rustdoc から ADR 0002 を参照できる状態を作る。変更は 2 ファイル・約 100〜150 行 (ADR 全文 + spec §8.5 1 行 Edit)。Small tier review |
| **M4b** (impl) | 約 1 日 | `input/mod.rs` + `input/mode.rs` + `input/context.rs` + `lib.rs` re-exports + 20 unit tests + `RomajiConverter::normalize_pending` 公開 | 実装は `RomajiConverter::normalize_pending` 追加 → `mode.rs` → `context.rs` の順に TDD で進める。変更は 4 コードファイル・約 400〜500 行 (production 約 300 行 + in-file unit tests 約 200 行)。Medium tier review |
| **M4c** (tests) | 約 0.8 日 | `mode_cases.tsv` (70) + `mode_cases_karukan_diff.tsv` (10) + `mode_golden.rs` + 4 property tests | M4b が提供する `InputContext` の公開 API を外から叩くだけで成立する。変更は 4 ファイル・約 400〜500 行 (TSV fixture 約 160 行 data + runner 約 150 行 code + property 約 150 行 code)。TSV は data のため 300 行 Policy から除外、code 部分 約 300 行は Medium tier 範囲内 |

**3 PR の直列依存関係:**

- M4a → M4b: M4b の ADR 参照 (rustdoc + commit message) が M4a マージ後に成立する。
- M4b → M4c: M4c の golden test / property test は `InputContext` 公開 API 必須。

**2 PR 案 (impl+tests を 1 PR にまとめる)** も検討したが、以下の理由で却下した:

- 1 PR にすると約 900 行 (production 300 + unit 200 + TSV 160 + runner/property 300) となり、Medium tier の実質上限 (300 行 code) を大幅超過。
- TDD 観点で unit tests (20 件) が先に緑になった状態を境界にしたほうが、golden fixture 作成時の期待値算出が機械的になる (unit test で確認済の `InputContext::{input_char, commit}` 挙動を fixture の oracle として使う)。

最終決定: **3 PR (M4a + M4b + M4c)**。

---

## PR #1 — M4a: ADR 0002 + spec §8.5 reference fix

**Goal:** Transient/Sticky 2 軸モデルの採用根拠を ADR 0002 として記録し、spec §8.5 の壊れた ADR 参照番号を 0001 → 0002 に修正する。本 PR はコード変更を含まない docs-only PR である。

### M4a 完了条件

- [ ] GitHub ISSUE (M4a) が作成され、merge 済み PR で close される
- [ ] `docs/adr/0002-input-mode-transient-vs-sticky.md` が develop に存在する
- [ ] `docs/superpowers/specs/2026-04-22-kotoha-phase-0-design.md` §8.5 の ADR 参照が `0002-input-mode-transient-vs-sticky.md` に更新されている
- [ ] `cargo build --workspace` / `cargo test --workspace` / `cargo clippy --workspace --all-targets -- -D warnings` / `cargo fmt --all --check` いずれも M3 時点のベースラインと同一 (docs 変更のみのため)
- [ ] lefthook pre-commit (fmt-check + doc-naming) + pre-push 全 PASS
- [ ] WBS ログ `docs/wbs/2026-04-XX-docs-N-input-mode-adr.md` が develop に push 済み

### ファイル構成 (M4a)

新規作成:

- `docs/adr/0002-input-mode-transient-vs-sticky.md`

変更:

- `docs/superpowers/specs/2026-04-22-kotoha-phase-0-design.md` §8.5 (line 406 付近)

---

### Task M4a-0: ISSUE 作成 + branch 作成

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

Expected: `develop` が `origin/develop` と同期、working tree clean、最新 commit は `4bf4031 docs: WBS logs for #22 / #29 / #26 PRs (#28, #30, #31)` 以降。

- [ ] **Step 2: M4a 用 ISSUE を作成**

Run:

```bash
gh issue create \
  --title "M4a (docs): ADR 0002 — input mode Transient vs Sticky policy + spec §8.5 ref fix" \
  --body "Phase 0 Milestone 4 (part A, docs-only): pin the Transient-vs-Sticky decision that underpins M4b's state machine, and fix the stale ADR reference in spec §8.5.

## Scope

- Create \`docs/adr/0002-input-mode-transient-vs-sticky.md\` using the \`docs/adr/0000-template.md\` structure. Status: 承認. Record the three alternatives considered (Sticky-by-default, Transient-by-default, Karukan-literal) and the decision to adopt Transient-by-default for Shift-induced Direct mode, in line with Mozc behavior.
- Edit spec §8.5 (line 406): change \`docs/adr/0001-input-mode-transient-vs-sticky.md\` → \`docs/adr/0002-input-mode-transient-vs-sticky.md\`. The number 0001 is already consumed by PR #28's non-ASCII retraction policy ADR.

## Out of Scope

- \`input\` module implementation (M4b)
- Mode golden fixture / property tests (M4c)

## Acceptance

- \`docs/adr/0002-input-mode-transient-vs-sticky.md\` exists with Status 承認, 3 alternatives, Decision, and 影響 sections
- spec §8.5 ADR reference points at \`0002-input-mode-transient-vs-sticky.md\`
- \`cargo build --workspace\` / \`cargo test --workspace\` / \`cargo clippy --workspace --all-targets -- -D warnings\` / \`cargo fmt --all --check\` all pass (unchanged from develop baseline)
- lefthook pre-commit + pre-push pass

## Reference

- Spec: \`docs/superpowers/specs/2026-04-22-kotoha-phase-0-design.md\` §8.5
- Plan: \`docs/superpowers/plans/2026-04-23-kotoha-phase-0-m4.md\`
- Parent tracking ISSUE: #32"
```

Expected: ISSUE が作成され URL + 番号が表示される (以降 `N` と呼ぶ、現在の状態から #33 以降が予想される)。

- [ ] **Step 3: branch 作成**

Run(`N` は Step 2 の ISSUE 番号):

```bash
git checkout -b docs/N-input-mode-adr develop
git branch --show-current
```

Expected: `docs/N-input-mode-adr`。

---

### Task M4a-1: ADR 0002 を Write で作成

**Files:**
- Create: `docs/adr/0002-input-mode-transient-vs-sticky.md`

**方針:** `docs/adr/0000-template.md` の節構成 (タイトル / ステータス / コンテキスト / 検討した選択肢 / 決定 / 影響 / 参照) に従う。既存 ADR 0001 (`docs/adr/0001-non-ascii-retraction-policy.md`) と同じトーン・粒度で執筆する。

- [ ] **Step 1: ADR 0002 を Write で新規作成**

Write ツールで `docs/adr/0002-input-mode-transient-vs-sticky.md` を以下の内容で作成:

```markdown
# 0002: Shift トリガ由来の Direct モードを Transient とする (Mozc 互換 / Karukan 非互換)

## ステータス

承認 (2026-04-XX)

## コンテキスト

Kotoha は GNOME Wayland 向けの自作日本語 IME であり、参考実装 Karukan の Shift 挙動による不便を解消することが Phase 0 の主要動機である (spec §2 参照)。Karukan では Shift + 何らかのキーを押すと直接入力モード (ローマ字がそのまま出力されるモード) に入り、Enter 確定後もそのモードが持続する。ユーザが明示的にモード切替キーを押して戻さない限り、以降の入力もすべて英字として扱われる。この挙動は日本語入力体験を著しく損なっている。

一方、Mozc / Google 日本語入力の挙動は異なる。Mozc では Shift キー押下で一時的に直接入力モードへ入るが、Enter 確定で自動的にひらがなモードへ復帰する。これは入力中の 1 語ごとに「Shift 押下 → 英字入力 → Enter → ひらがな入力再開」という自然な流れを成立させる。

spec §8 はこの観察に基づき、`(mode, origin)` タプルで状態を表現する 3 状態機械を定義している:

- `(Hiragana, Sticky)` = 初期状態、ローマ字→かな変換
- `(Direct, Transient)` = Shift トリガで入った一時英字モード
- `(Direct, Sticky)` = 明示トグルで入った持続英字モード

`ModeOrigin::{Sticky, Transient}` は内部識別子であり、Transient origin の Direct モードのみが commit / cancel 契機で `(Hiragana, Sticky)` へ自動復帰する設計になっている。本 ADR はこの「Shift 由来 = Transient、明示トグル = Sticky」の 2 軸決定が、Phase 0 の入力モード管理レイヤーにおいて normative であることを記録する。

この判断は spec §8.5 で Karukan との差分として言及されているが、ADR としての記録は本 M4a (PR #) で行う。

## 検討した選択肢

### 選択肢 1: Transient-by-default (採用候補、Mozc 互換)

- 利点:
  - Mozc / Google 日本語入力の挙動と揃うため、ユーザが既存の IME から移行する際の学習コストが最小になる。
  - 1 語ごとにモード復帰が自動化され、「明示的にモードを戻し忘れて次の日本語入力が英字になってしまう」事故を防ぐ。
  - Shift キーの「一時的な大文字化」セマンティクスが日本語入力体験において自然に機能する。
- 欠点:
  - ユーザが連続して英字を入力したい場合 (例: URL 入力中にひらがな IME のまま) は明示トグル (`toggle_mode`) による Sticky Direct への昇格が必要であり、Transient 中に追加の操作が必要になる。
  - Transient → Sticky 昇格経路 (`toggle_mode` が Transient Direct に対して呼ばれた場合の挙動) の仕様を別途決定する必要がある。

### 選択肢 2: Sticky-by-default (Karukan 互換)

- 利点:
  - 参考実装 Karukan と挙動が揃うため、Karukan のユーザが違和感なく移行できる。
  - `ModeOrigin` enum が不要になり、状態機械が 2 状態 (`Hiragana` / `Direct`) に単純化される。
- 欠点:
  - Phase 0 の主要動機 (Karukan の Shift 挙動による不便の解消) そのものを放棄することになる。本プロジェクトの存在理由に反する。
  - 大多数の日本語入力ユーザ体験において、「1 語英字入力した後もひらがなに戻らない」挙動は明確に unfriendly であることが、spec §2 で既に論証されている。

### 選択肢 3: Karukan-literal (Karukan の実装をほぼそのまま移植)

- 利点:
  - 実装工数が最小。
  - Karukan の golden fixture を流用可能。
- 欠点:
  - 選択肢 2 の欠点をすべて継承する。
  - Kotoha 独自の設計判断 (モード管理の責務を `kotoha-core::input` に閉じ込め、キーマッピングは Phase 3 の IBus engine 層に分離する方針、spec §4 参照) に反する。Karukan はモード遷移ロジックを統合層にハードコードしており、その設計ミスそのものが本プロジェクトの回避対象である。

## 決定

**選択肢 1 (Transient-by-default)** を採用する。具体的には:

1. Shift キー相当 (ASCII 大文字入力) による Hiragana → Direct 遷移は常に `(Direct, Transient)` を生成する。
2. `(Direct, Transient)` は `commit()` / `cancel()` で `(Hiragana, Sticky)` に自動復帰する。
3. 明示トグル (`toggle_mode()` / `set_mode()`) による Direct 遷移は常に `(Direct, Sticky)` を生成し、自動復帰の対象外とする。
4. `(Direct, Transient)` に対して `toggle_mode()` が呼ばれた場合は `(Direct, Sticky)` に昇格する。本昇格挙動はフラグ `InputContext::allow_transient_to_sticky_promotion: bool` で切り替え可能とし、Phase 0 でのデフォルトは `false` とする(Phase 3 以降で設定 UI 化予定)。

本決定は spec §8 (モード管理仕様) の状態遷移表を normative 仕様として認定する。Phase 0 では本 ADR と spec §8 の間に齟齬がないこと (遷移表全セルが `(Hiragana, Sticky)` / `(Direct, Transient)` / `(Direct, Sticky)` の 3 状態で閉じていること) を golden test (M4c) で verification する。

## 影響

### 実装への影響 (M4b)

- `crates/kotoha-core/src/input/mode.rs` に `InputMode` (`pub`) と `ModeOrigin` (`pub(crate)`) の 2 enum を定義する。
- `crates/kotoha-core/src/input/context.rs` の `InputContext` 状態機械は、spec §8.4 擬似コード通りに `input_char` / `commit` / `cancel` / `toggle_mode` / `set_mode` / `reset` を実装する。
- `InputContext::allow_transient_to_sticky_promotion: bool` フィールドを追加し、Phase 0 デフォルトは `false` とする。

### テストへの影響 (M4c)

- `crates/kotoha-core/tests/fixtures/mode_cases.tsv` 70 ケースは本 ADR の 3 状態機械を外部観察可能な入出力対に展開したものとして整備する。
- `crates/kotoha-core/tests/fixtures/mode_cases_karukan_diff.tsv` 10 ケースは本 ADR の「Transient 自動復帰」が Karukan の「Sticky 持続」と異なる入力を並べ、Kotoha 側の normative 挙動を明示する。
- property test 4 条件 (Hiragana-Sticky 安定性 / Transient 必ず復帰 / Sticky-Direct 持続 / reset 冪等性) は本 ADR の 3 状態機械の不変条件を proptest で検証する。

### spec への影響

- spec §8.5 (line 406) の ADR 参照番号を `0001-input-mode-transient-vs-sticky.md` から `0002-input-mode-transient-vs-sticky.md` へ修正する(本 PR 内で同時 Edit)。0001 は PR #28 により非 ASCII retraction policy に既に割り当て済であるため。

### Phase 3 以降への影響

- Phase 3 の IBus engine 層は `InputContext` の公開 API (`input_char` / `commit` / `cancel` / `toggle_mode` / `set_mode`) をキーイベントにマッピングする。本 ADR で 3 状態機械が normative に固定されたため、Phase 3 着手時にモード遷移ロジックを再設計する必要はない。
- 設定 UI で `allow_transient_to_sticky_promotion` を切り替え可能にする場合、本 ADR の「Phase 0 デフォルトは `false`」が起点となる。ユーザが Karukan 互換を望む場合は `toggle_mode` を介した昇格ではなく、将来の設定項目 (例: `ShiftModePolicy::Sticky`) で対応する (本 ADR を再検討する別 ADR を要する)。

## 参照

- ISSUE #32 — 本 ADR が属する M4 tracking ISSUE。
- ISSUE #N — 本 ADR を新設する M4a ISSUE (本 PR で close)。
- Spec §2 — Kotoha の背景と動機 (Karukan の Shift 挙動による不便を解消する)。
- Spec §8 — モード管理仕様 (3 状態機械、遷移表、擬似コード)。
- Spec §8.5 — Karukan との差分 (本 ADR への参照を含む)。
- PR #28 (ADR 0001 — 非 ASCII retraction policy) — 0001 番号を先に消費した先行 ADR。
- Karukan (`togatoga/karukan`) — 反面教師としての参照実装 (MIT/Apache-2.0)。
- Mozc — Transient 挙動の模範となる OSS IME。
```

Note: `N` は Task M4a-0 Step 2 で作成した ISSUE 番号を後で置換する (本 Step では literal `#N` のまま commit し、PR merge 時に ADR の "ISSUE #N" 参照を実番号へ置換する必要はない — ADR は PR merge 前の branch 上で最終形を commit する)。実運用では本 Step で `#N` を実 ISSUE 番号に手動置換して書き込む。

- [ ] **Step 2: ファイル存在確認**

Run:

```bash
ls -l docs/adr/0002-input-mode-transient-vs-sticky.md
wc -l docs/adr/0002-input-mode-transient-vs-sticky.md
```

Expected: ファイルが存在し、約 80〜120 行。

---

### Task M4a-2: spec §8.5 の ADR 参照番号を修正

**Files:**
- Modify: `docs/superpowers/specs/2026-04-22-kotoha-phase-0-design.md`

- [ ] **Step 1: spec §8.5 の該当行を Edit で修正**

Edit `docs/superpowers/specs/2026-04-22-kotoha-phase-0-design.md`:

old_string:

```
この判断は `docs/adr/0001-input-mode-transient-vs-sticky.md` に記録する。
```

new_string:

```
この判断は `docs/adr/0002-input-mode-transient-vs-sticky.md` に記録する。
```

- [ ] **Step 2: 参照番号の変更が反映されていることを確認**

Run:

```bash
grep -n "input-mode-transient-vs-sticky" docs/superpowers/specs/2026-04-22-kotoha-phase-0-design.md
```

Expected: 1 件ヒット、参照先が `docs/adr/0002-input-mode-transient-vs-sticky.md` になっている。

- [ ] **Step 3: spec 全体で 0001 の誤参照が残っていないことを確認**

Run:

```bash
grep -n "0001-input-mode" docs/superpowers/specs/2026-04-22-kotoha-phase-0-design.md
```

Expected: ヒットなし (出力空)。

---

### Task M4a-3: 検証 + commit

**Files:** (検証 + commit のみ、変更なし)

- [ ] **Step 1: lefthook pre-commit 実行**

Run:

```bash
lefthook run pre-commit
```

Expected:
- `fmt-check`: 変更が `.rs` ファイルを含まないため、glob マッチなしで skip 表示、または PASS。
- `doc-naming`: `docs/adr/0002-input-mode-transient-vs-sticky.md` が ADR 命名規則 (`NNNN-<title>.md`) を満たし、`docs/superpowers/specs/...` 既存ファイルを Edit しているだけなので PASS。

- [ ] **Step 2: cargo コマンドによる regression チェック (docs 変更だが一応)**

Run:

```bash
cargo build --workspace
cargo test --workspace 2>&1 | tail -5
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
```

Expected:
- build PASS
- test: M2 (23 件) + M3a (33 件) + M3b (2 golden + 2 property + 3 regression) = develop 時点のテスト件数がそのまま PASS
- clippy warnings ゼロ
- fmt diff なし

- [ ] **Step 3: ステージ + commit**

Run:

```bash
git add docs/adr/0002-input-mode-transient-vs-sticky.md docs/superpowers/specs/2026-04-22-kotoha-phase-0-design.md
git status
```

Expected: 2 files staged (ADR 新規 + spec Edit)。

```bash
git commit -m "docs(adr): ADR 0002 — input mode Transient vs Sticky policy

Record the Transient-by-default decision for Shift-induced Direct
mode, which diverges from Karukan (Sticky-by-default) and matches
Mozc's Enter-triggered auto-return behavior. Also fix spec §8.5's
stale ADR reference: 0001 was already consumed by PR #28 (non-ASCII
retraction policy), so the input-mode ADR takes number 0002.

The ADR documents three alternatives (Transient-by-default /
Sticky-by-default / Karukan-literal), the decision, and the
downstream implementation + test impact for M4b/M4c.

Closes #N
Refs #32"
```

Expected: `2 files changed`。`#N` は Task M4a-0 Step 2 の ISSUE 番号に置換する。

---

### Task M4a-4: push + PR 作成

**Files:** (push + PR 作成、変更なし)

- [ ] **Step 1: lefthook pre-push 実行**

Run:

```bash
lefthook run pre-push
```

Expected:
- `manifest-check` PASS
- `build` PASS
- `clippy` PASS (warnings ゼロ)
- `test` PASS (develop baseline と同数)

- [ ] **Step 2: push**

Run:

```bash
git push -u origin docs/N-input-mode-adr
```

Expected: pre-push hook が自動実行、PASS 後に push 成功。

- [ ] **Step 3: PR 作成**

Run (`N` は Task M4a-0 の ISSUE 番号):

```bash
gh pr create --base develop --head docs/N-input-mode-adr \
  --title "M4a (docs): ADR 0002 — input mode Transient vs Sticky + spec §8.5 ref fix" \
  --body "$(cat <<'EOS'
## Summary

Phase 0 Milestone 4 (part A, docs-only): pin the Transient-vs-Sticky decision
that underpins M4b's state machine, and fix the stale ADR reference in spec
§8.5.

- Add \`docs/adr/0002-input-mode-transient-vs-sticky.md\` (status 承認). Records
  three alternatives (Transient-by-default / Sticky-by-default / Karukan-literal)
  and the decision to adopt Transient-by-default for Shift-induced Direct mode
  in line with Mozc.
- Edit spec §8.5 (line 406): change the ADR reference from \`0001-...\` to
  \`0002-...\`. Number 0001 is already consumed by PR #28's non-ASCII retraction
  policy ADR.

## Out of Scope

- \`input\` module implementation (M4b)
- Mode golden fixture / property tests (M4c)

## Related

- Spec: docs/superpowers/specs/2026-04-22-kotoha-phase-0-design.md §8.5
- Plan: docs/superpowers/plans/2026-04-23-kotoha-phase-0-m4.md
- Parent tracking ISSUE: #32
- Closes #N

## Test plan

- [ ] ADR 0002 exists at docs/adr/0002-input-mode-transient-vs-sticky.md with status 承認
- [ ] spec §8.5 ADR reference is updated to point at 0002
- [ ] cargo build --workspace passes (unchanged from develop)
- [ ] cargo test --workspace passes (unchanged from develop)
- [ ] cargo clippy --workspace --all-targets -- -D warnings: zero warnings
- [ ] cargo fmt --all --check: no diff
- [ ] lefthook pre-commit + pre-push all PASS
EOS
)"
```

Replace `#N` with the ISSUE number from Task M4a-0. Expected: PR URL が表示される。

---

### Task M4a-5: review + findings 解消 + merge + WBS ログ

**Files:** (検証 + merge + WBS ログ作成)

- [ ] **Step 1: PR review (Small tier — docs-only、2 files、約 120 行)**

CLAUDE.md の PR Review Matrix に従い Small tier を選択。docs-only かつ ADR + spec fix のため、dimensions は security + architecture + testing の 3 軸で十分:

Run (Skill 経由):

```
/agent-teams:team-review dimensions=security,architecture,testing
/secrets-check
```

Expected: Critical / High findings ゼロ。ADR 内に secrets が混入していないこと、spec §8.5 の参照番号修正で他参照が壊れていないこと、ADR の論理構成が spec §8 と整合することが verify される。

- [ ] **Step 2: findings 解消 + 再 review**

Critical / High ゼロになるまで実施。findings が出た場合は implementer subagent に修正依頼 → push → 再 review ループ。

- [ ] **Step 3: squash merge + branch 削除**

Run (`<PR番号>` は Step 3 出力の PR 番号):

```bash
gh pr merge <PR番号> --squash --delete-branch
gh pr view <PR番号> --json state,mergeCommit -q '{state, merge: .mergeCommit.oid}'
```

Expected: `state: MERGED`、merge commit SHA 取得。

- [ ] **Step 4: develop 追従**

Run:

```bash
git checkout develop
git pull
git log --oneline -3
```

Expected: squash merge commit が develop 先頭。

- [ ] **Step 5: WBS ログ作成**

Write `docs/wbs/2026-04-XX-docs-N-input-mode-adr.md` (`N` は ISSUE 番号、`2026-04-XX` は実作業日、`<MERGE_COMMIT>` は Step 3 の merge SHA、`<PR_NUMBER>` は PR 番号):

```markdown
---
milestone: M4a
branch: docs/N-input-mode-adr
pr: "#<PR_NUMBER>"
merge_commit: "<MERGE_COMMIT>"
issue: "#N"
status: done
started: 2026-04-XX
finished: 2026-04-XX
---

# M4a: ADR 0002 — input mode Transient vs Sticky + spec §8.5 ref fix

## 実施内容

- `docs/adr/0002-input-mode-transient-vs-sticky.md` を承認済みとして新設
- spec §8.5 の ADR 参照番号を `0001-input-mode-transient-vs-sticky.md` → `0002-input-mode-transient-vs-sticky.md` に修正 (0001 は PR #28 により既に非 ASCII retraction policy に割り当て済)

## つまずき

(実施時に記入)

## M4b への申し送り

- ADR 0002 が決定した normative 事項:
  - Hiragana → Direct (Shift トリガ) は常に `(Direct, Transient)`
  - `(Direct, Transient)` は commit / cancel で `(Hiragana, Sticky)` 自動復帰
  - 明示トグルは常に `Sticky` origin
  - `(Direct, Transient)` への `toggle_mode()` は `allow_transient_to_sticky_promotion: bool` で挙動が変わる (Phase 0 デフォルト `false`)
- 上記は M4b の `InputContext::input_char` / `commit` / `cancel` / `toggle_mode` / `set_mode` 実装と、M4c の golden test / property test で verify する

## 成果物リンク

- PR: #<PR_NUMBER>
- ISSUE: #N
- Spec: `docs/superpowers/specs/2026-04-22-kotoha-phase-0-design.md` §8.5
- Plan: `docs/superpowers/plans/2026-04-23-kotoha-phase-0-m4.md`
```

- [ ] **Step 6: WBS を develop に直接 push (CLAUDE.md の例外ルール適用)**

Run:

```bash
git add docs/wbs/2026-04-XX-docs-N-input-mode-adr.md
git commit -m "docs: M4a implementation log"
git push
```

Expected: lefthook pre-push 全 PASS。

---

## PR #2 — M4b: input module implementation + 20 unit tests

**Goal:** `crates/kotoha-core/src/input/{mod,mode,context}.rs` の 3 ファイルを新設し、`InputMode` / `ModeOrigin` / `InputContext` / `InputStep` を実装する。`InputContext` の 8 公開メソッド (`new`, `mode`, `preedit`, `input_char`, `commit`, `cancel`, `toggle_mode`, `set_mode`, `reset`) を TDD で実装し、単体テスト 20 件を緑にする。`RomajiConverter::normalize_pending(&mut self) -> String` を新規公開して streaming path の mid-stream normalize ギャップに対処する。

### M4b 完了条件

- [ ] GitHub ISSUE (M4b) が作成され、merge 済み PR で close される
- [ ] `cargo test -p kotoha-core --lib input` が 20 件以上 PASS
- [ ] `cargo test --workspace` 全体 PASS
- [ ] `cargo build --workspace` PASS
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` warnings ゼロ
- [ ] `cargo fmt --all --check` diff ゼロ
- [ ] lefthook pre-push 全 PASS
- [ ] `kotoha_core::{InputContext, InputMode, InputStep}` が lib.rs から re-export されている
- [ ] `RomajiConverter::normalize_pending` が公開 API として利用可能
- [ ] WBS ログ `docs/wbs/2026-04-XX-feature-M-kotoha-input-module.md` が develop に push 済み

### ファイル構成 (M4b)

新規作成:

- `crates/kotoha-core/src/input/mod.rs` — module 宣言 + pub re-export
- `crates/kotoha-core/src/input/mode.rs` — `InputMode` (`pub`) + `ModeOrigin` (`pub(crate)`) + 基本単体テスト
- `crates/kotoha-core/src/input/context.rs` — `InputContext` + `InputStep` + 単体テスト (約 20 件)

変更:

- `crates/kotoha-core/src/lib.rs` — `pub mod input;` + `pub use input::{InputContext, InputMode, InputStep};`
- `crates/kotoha-core/src/romaji/mod.rs` — `RomajiConverter::normalize_pending(&mut self) -> String` 公開メソッド追加

---

### Task M4b-0: ISSUE 作成 + branch 作成

- [ ] **Step 1: develop 最新化**

Run:

```bash
cd /home/kohshiro/develops/student/kotoha-ime
git checkout develop
git pull
git status
```

Expected: M4a の merge + WBS commit が develop 先頭、working tree clean。

- [ ] **Step 2: M4b 用 ISSUE 作成**

Run:

```bash
gh issue create \
  --title "M4b (impl): input module — InputMode/ModeOrigin/InputContext/InputStep + 20 unit tests" \
  --body "Phase 0 Milestone 4 (part B): implement the input mode state machine.

## Scope

- New module \`crates/kotoha-core/src/input/\` with 3 files: mod.rs, mode.rs, context.rs
- \`mode.rs\`: \`InputMode\` (pub, non_exhaustive, variants Hiragana / Direct) + \`ModeOrigin\` (pub(crate), variants Sticky / Transient)
- \`context.rs\`: \`InputContext\` struct + \`InputStep\` enum (pub, non_exhaustive, variants Preedit / Committed(String) / Invalid(char)) + 8 public methods (new / mode / preedit / input_char / commit / cancel / toggle_mode / set_mode / reset)
- \`mod.rs\`: module wiring + pub re-export of InputContext / InputMode / InputStep
- \`lib.rs\`: \`pub mod input;\` + \`pub use input::{InputContext, InputMode, InputStep};\`
- New pub method \`RomajiConverter::normalize_pending(&mut self) -> String\` wrapping \`StateMachine::normalize\` to close the mid-stream normalize gap identified in PR #30 WBS (ISSUE #29 follow-up)
- 20 unit tests covering initial state, Hiragana input paths, Shift trigger, Transient auto-return, explicit toggle, Transient→Sticky promotion, commit/cancel from each state, reset

## Depends on

- M3 (merged): RomajiConverter public API
- M4a (merged): ADR 0002 normative decision

## Acceptance

- cargo test -p kotoha-core --lib input passes 20+ tests
- cargo test --workspace passes overall
- cargo clippy --workspace --all-targets -- -D warnings: zero warnings
- kotoha_core::{InputContext, InputMode, InputStep} re-exported from lib.rs
- RomajiConverter::normalize_pending available in public API

## Out of Scope

- Mode golden fixture + runner (M4c)
- Property tests (M4c)

## Reference

- Spec: docs/superpowers/specs/2026-04-22-kotoha-phase-0-design.md §7.3-§7.6, §8, §11.1
- Plan: docs/superpowers/plans/2026-04-23-kotoha-phase-0-m4.md
- ADR: docs/adr/0002-input-mode-transient-vs-sticky.md
- Parent tracking ISSUE: #32"
```

Expected: ISSUE 作成、URL + 番号表示 (以降 `M` と呼ぶ)。

- [ ] **Step 3: branch 作成**

Run (`M` は Step 2 の ISSUE 番号):

```bash
git checkout -b feature/M-kotoha-input-module develop
git branch --show-current
```

Expected: `feature/M-kotoha-input-module`。

---

### Task M4b-1: `input/mod.rs` を Write で骨格作成 (まだ再 export なし)

**Files:**
- Create: `crates/kotoha-core/src/input/mod.rs`

**方針:** mode.rs と context.rs が存在しない段階で mod 宣言だけを置くと `cannot find mod` でコンパイル失敗するため、本 Task では `mod.rs` も作らず、先に `mode.rs` (Task M4b-2) → `normalize_pending` 公開 (Task M4b-3) → `context.rs` (Task M4b-4) の順で積み上げる。`mod.rs` と `lib.rs` 配線は Task M4b-5 で最終的に行う。

本 Task はこの順序判断の記録のみ。ファイル Write はなし。

- [ ] **Step 1: `input/` ディレクトリ作成**

Run:

```bash
mkdir -p crates/kotoha-core/src/input
```

Expected: ディレクトリ作成成功 (1 階層)。

---

### Task M4b-2: `input/mode.rs` を TDD で実装

**Files:**
- Create: `crates/kotoha-core/src/input/mode.rs`

**方針:** `InputMode` / `ModeOrigin` の 2 enum を定義するのみ。状態遷移ロジックは context.rs 側に持たせるため、本ファイルは純粋な型定義とテストのみ。ただし現時点では `mode.rs` 単体では `lib.rs` に配線されないため、`input/mod.rs` に mod 宣言を置いて `lib.rs` に `pub mod input;` を先に追加する必要がある(そうしないとコンパイル対象に含まれない)。本 Task では `input/mod.rs` も暫定骨格として作成し、`lib.rs` に `pub mod input;` を追加する(まだ pub use は追加しない)。

- [ ] **Step 1: `mode.rs` にテスト先行の骨格を Write (Red)**

Write ツールで `crates/kotoha-core/src/input/mode.rs` を新規作成:

```rust
//! Input mode enums for the Kotoha IME.
//!
//! This module defines two enums that together describe the
//! [`InputContext`](crate::input::context::InputContext) state tuple per
//! spec §8.1:
//!
//! - [`InputMode`] — the user-visible mode (Hiragana / Direct). Public.
//! - [`ModeOrigin`] — the internal origin marker (Sticky / Transient) that
//!   decides whether a commit triggers auto-return to `(Hiragana, Sticky)`.
//!   Crate-private.
//!
//! The Transient-by-default policy for Shift-induced Direct mode is pinned
//! in ADR 0002 (`docs/adr/0002-input-mode-transient-vs-sticky.md`).

/// User-visible input mode.
///
/// Marked `#[non_exhaustive]` so additional modes (for example Katakana or
/// Zenkaku) may be introduced in later phases without breaking external
/// match sites.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum InputMode {
    /// Romaji → kana conversion mode. Default state.
    Hiragana,
    /// Direct ASCII / punctuation passthrough mode.
    Direct,
}

/// Internal origin marker for the current mode.
///
/// Decides whether a commit / cancel triggers auto-return to
/// `(Hiragana, Sticky)`. See spec §8 and ADR 0002.
///
/// Crate-private; not part of the public API. The public API exposes only
/// `InputMode` and observable behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ModeOrigin {
    /// Initial state, or explicit-toggle-induced sticky mode. Persists
    /// across commit / cancel.
    Sticky,
    /// Shift-trigger-induced transient mode. Reverts to
    /// `(Hiragana, Sticky)` on commit / cancel.
    Transient,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn input_mode_equality_is_per_variant() {
        assert_eq!(InputMode::Hiragana, InputMode::Hiragana);
        assert_eq!(InputMode::Direct, InputMode::Direct);
        assert_ne!(InputMode::Hiragana, InputMode::Direct);
    }

    #[test]
    fn input_mode_is_copy() {
        let m = InputMode::Hiragana;
        let n = m; // Copy, not move
        assert_eq!(m, n);
    }

    #[test]
    fn mode_origin_equality_is_per_variant() {
        assert_eq!(ModeOrigin::Sticky, ModeOrigin::Sticky);
        assert_eq!(ModeOrigin::Transient, ModeOrigin::Transient);
        assert_ne!(ModeOrigin::Sticky, ModeOrigin::Transient);
    }
}
```

- [ ] **Step 2: `input/mod.rs` を Write で暫定骨格作成 (context は未作成なので宣言しない)**

Write ツールで `crates/kotoha-core/src/input/mod.rs` を新規作成:

```rust
//! Input mode management module.
//!
//! Public API:
//! - [`InputMode`] — user-visible input mode
//! - `InputContext` — state machine (added by M4b-4)
//! - `InputStep` — per-char outcome enum (added by M4b-4)
//!
//! The crate-private [`mode::ModeOrigin`] is used by `context::InputContext`
//! to distinguish Sticky vs Transient origins per ADR 0002.

pub mod mode;

pub use mode::InputMode;
```

Note: 本暫定骨格では context は未作成なので宣言せず、`InputContext` / `InputStep` の re-export も置かない。Task M4b-4 Step 3 で context 実装後に更新する。

- [ ] **Step 3: `lib.rs` に `pub mod input;` を追加 (ただし pub use はまだ)**

Edit `crates/kotoha-core/src/lib.rs`:

old_string:

```rust
pub mod error;
pub mod kana;
pub mod romaji;

pub use error::{Error, Result};
pub use romaji::{ConvertStep, RomajiConverter};
```

new_string:

```rust
pub mod error;
pub mod input;
pub mod kana;
pub mod romaji;

pub use error::{Error, Result};
pub use input::InputMode;
pub use romaji::{ConvertStep, RomajiConverter};
```

Note: `InputContext` と `InputStep` の re-export は Task M4b-5 で追加する。`InputMode` は本 Task 内で完全に定義されているため、先行して re-export しても整合する。

- [ ] **Step 4: mode のテストを実行 (Green)**

Run:

```bash
cargo test -p kotoha-core --lib input::mode::tests
```

Expected: 3 passed (`input_mode_equality_is_per_variant`、`input_mode_is_copy`、`mode_origin_equality_is_per_variant`)。

- [ ] **Step 5: clippy 確認**

Run:

```bash
cargo clippy -p kotoha-core --all-targets -- -D warnings
```

Expected: warnings ゼロ。

- [ ] **Step 6: commit**

Run:

```bash
git add crates/kotoha-core/src/input/mod.rs crates/kotoha-core/src/input/mode.rs crates/kotoha-core/src/lib.rs
git commit -m "feat(kotoha-core): add input::mode enums (InputMode pub, ModeOrigin pub(crate))

- InputMode: user-visible mode (Hiragana / Direct), #[non_exhaustive]
- ModeOrigin: crate-private origin marker (Sticky / Transient)
- Wired into lib.rs: pub mod input; pub use input::InputMode;
- 3 unit tests for enum equality and Copy semantics

The Transient-by-default policy is pinned in ADR 0002. InputContext
(context.rs) will consume both enums in M4b-4."
```

Expected: 3 files changed (`input/mod.rs`、`input/mode.rs`、`lib.rs`)。

---

### Task M4b-3: `RomajiConverter::normalize_pending` 公開メソッドを追加

**Files:**
- Modify: `crates/kotoha-core/src/romaji/mod.rs`

**方針:** `StateMachine::normalize` は既に `pub(crate) fn normalize(&mut self) -> String` として実装されているが、`RomajiConverter` 外部からは利用できない。`InputContext::input_char` の Hiragana モード path で streaming push の mid-stream normalize ギャップを閉じるために、公開ラッパ `normalize_pending` を追加する。

- [ ] **Step 1: `romaji/mod.rs` の `impl RomajiConverter` block に `normalize_pending` を追加**

Edit `crates/kotoha-core/src/romaji/mod.rs`:

old_string:

```rust
    /// Clears the pending buffer without emitting anything.
    ///
    /// # Postconditions
    /// - The pending buffer is empty.
    pub fn reset(&mut self) {
        self.machine.reset();
    }
```

new_string:

```rust
    /// Clears the pending buffer without emitting anything.
    ///
    /// # Postconditions
    /// - The pending buffer is empty.
    pub fn reset(&mut self) {
        self.machine.reset();
    }

    /// Normalizes the pending buffer into a stable form and returns any
    /// salvaged kana as an owned `String`.
    ///
    /// Intended to be called by
    /// [`crate::input::context::InputContext`] immediately after each
    /// [`Self::push`] in Hiragana streaming mode so that the pending
    /// buffer remains idempotent under re-conversion (matching the
    /// stability that [`Self::convert`] already provides in batch mode).
    /// Without this normalization, a mid-stream invalid-char drop can
    /// leave a residue in the buffer that a subsequent `push` would
    /// silently lose (PR #30 / ISSUE #29 follow-up for the streaming
    /// path).
    ///
    /// # Postconditions
    /// - The returned `String` contains only valid hiragana (plus `ー`
    ///   for long vowels, `っ` for sokuon, `ん` for hatsuon).
    /// - After this call, the pending buffer is either empty or a trie
    ///   partial prefix (stable form).
    ///
    /// # Examples
    /// ```
    /// use kotoha_core::RomajiConverter;
    /// let mut c = RomajiConverter::new();
    /// let _ = c.push('b'); // pending = "b"
    /// let _ = c.push('!'); // settle drops leading 'b', buffer = "!"
    /// // "!" is itself a complete rule that push left unsettled.
    /// assert_eq!(c.normalize_pending(), "!");
    /// ```
    pub fn normalize_pending(&mut self) -> String {
        self.machine.normalize()
    }
```

- [ ] **Step 2: build 確認**

Run:

```bash
cargo build -p kotoha-core
```

Expected: `Finished dev profile`。

- [ ] **Step 3: doctest を含めた romaji モジュールテスト実行**

Run:

```bash
cargo test -p kotoha-core --lib romaji
cargo test -p kotoha-core --doc romaji::RomajiConverter
```

Expected: 既存 romaji テスト全件 PASS (既存 33 件) + 既存 doctest (1 件) + 本 Task で追加した `normalize_pending` の doctest (1 件) = 合計で新規 doctest 1 件が増加して PASS。

- [ ] **Step 4: clippy 確認**

Run:

```bash
cargo clippy -p kotoha-core --all-targets -- -D warnings
```

Expected: warnings ゼロ。

- [ ] **Step 5: commit**

Run:

```bash
git add crates/kotoha-core/src/romaji/mod.rs
git commit -m "feat(kotoha-core): expose RomajiConverter::normalize_pending

Thin pub wrapper over StateMachine::normalize so that
InputContext::input_char can close the streaming-path mid-stream
normalize gap (PR #30 / ISSUE #29 follow-up for the streaming path).

InputContext's Hiragana mode calls push then normalize_pending on
every char, matching the stability that convert() provides in batch
mode.

Refs #32"
```

Expected: 1 file changed。

---

### Task M4b-4: `input/context.rs` を TDD で実装

**Files:**
- Create: `crates/kotoha-core/src/input/context.rs`
- Modify: `crates/kotoha-core/src/input/mod.rs`

**方針:** spec §7.5 / §7.6 / §8 の通りに `InputContext` / `InputStep` を実装する。8 つの公開メソッドを 1 つずつ TDD で実装し、最後に全体テスト件数が 20 件以上であることを確認する。

**設計の要点:**

- `InputContext` は `RomajiConverter` (Hiragana モード delegate) と `direct_buffer: String` (Direct モード buffer) の両方を持つ。
- `input_char` は Hiragana モード中に ASCII 大文字が来たら常に `(Direct, Transient)` へ遷移し、その大文字を `direct_buffer` に追加する (spec §8.3 ルール 1、§8.4 擬似コード前半)。
- `input_char` Hiragana path は `RomajiConverter::push` → `normalize_pending` の 2 段呼び出しで、両者の結果を合流させて `InputStep::{Preedit, Committed(String), Invalid(char)}` を返す (§InputContext mid-stream salvage handling 参照)。
- `commit()` の戻り値は確定文字列 (String)。Transient origin の場合のみ `(Hiragana, Sticky)` 自動復帰 (spec §8.4 擬似コード後半)。
- `toggle_mode` は `allow_transient_to_sticky_promotion: bool` フラグで Transient→Sticky 昇格可否を切り替える (spec §8.4 擬似コード + ADR 0002 決定事項 4)。Phase 0 デフォルトは `false`。

**重要な InputStep の型:** `InputStep::Committed(String)` とする(spec §7.6 通り)。`ConvertStep::Committed(Cow<'static, str>)` とは異なる型なので、`InputContext::input_char` Hiragana path では `String` 化する:

```rust
let push_result = self.converter.push(ch); // ConvertStep (Cow<'static, str> in Committed)
let salvaged = self.converter.normalize_pending(); // String
// Combine push_result and salvaged into a single InputStep:
match push_result {
    ConvertStep::Committed(cow) => {
        if salvaged.is_empty() {
            InputStep::Committed(cow.into_owned())
        } else {
            InputStep::Committed(format!("{cow}{salvaged}"))
        }
    }
    ConvertStep::Pending => {
        if salvaged.is_empty() {
            InputStep::Preedit
        } else {
            InputStep::Committed(salvaged)
        }
    }
    ConvertStep::Invalid(c) => {
        if salvaged.is_empty() {
            InputStep::Invalid(c)
        } else {
            // salvage is more user-visible than the rejected char; prefer Committed
            InputStep::Committed(salvaged)
        }
    }
    _ => InputStep::Preedit, // exhaustiveness guard for future non_exhaustive variants
}
```

`_ => InputStep::Preedit` は `ConvertStep` が `#[non_exhaustive]` なので防御的に置く(clippy が non_exhaustive match に `_` を要求する場合の対応)。

#### Step 1〜16 の構成 (TDD サイクル)

本 Task は 8 メソッドに対して約 20 件の unit test を追加する。各 Step は「(a) context.rs にテスト + 最小実装を Write/Edit → (b) `cargo test` で緑を確認 → (c) 必要に応じて clippy」の TDD サイクルを踏む。

- [ ] **Step 1: `context.rs` の初期骨格を Write (Red → Green 最小)**

Write ツールで `crates/kotoha-core/src/input/context.rs` を新規作成:

```rust
//! Input mode state machine.
//!
//! Implements the 3-state machine described in spec §8:
//!
//! - `(Hiragana, Sticky)` — initial state, romaji → kana
//! - `(Direct, Transient)` — Shift-trigger-induced, auto-returns on commit
//! - `(Direct, Sticky)` — explicit-toggle-induced, persists across commits
//!
//! The Transient-by-default policy is pinned in
//! ADR 0002 (`docs/adr/0002-input-mode-transient-vs-sticky.md`).

use crate::input::mode::{InputMode, ModeOrigin};
use crate::romaji::{ConvertStep, RomajiConverter};

/// Per-char outcome of [`InputContext::input_char`].
///
/// Marked `#[non_exhaustive]` so additional variants may be introduced in
/// later phases without breaking external match sites.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum InputStep {
    /// The char was absorbed into the pending / direct buffer; nothing
    /// visible was committed this step.
    Preedit,
    /// Some content (kana in Hiragana mode, ASCII in Direct mode, or
    /// mid-stream salvage from normalize) was committed this step.
    Committed(String),
    /// The char is outside the supported alphabet here and was discarded.
    Invalid(char),
}

/// Input mode state machine.
///
/// Holds the `(mode, origin)` tuple per spec §8.1 plus two buffers:
/// - `converter`: a [`RomajiConverter`] that owns the Hiragana-mode pending
///   buffer.
/// - `direct_buffer`: a `String` that owns the Direct-mode buffer.
///
/// # Invariants
/// - The state tuple is always one of `(Hiragana, Sticky)`,
///   `(Direct, Transient)`, or `(Direct, Sticky)` (never
///   `(Hiragana, Transient)`).
/// - When `mode == Hiragana`, `direct_buffer.is_empty()`.
/// - When `mode == Direct`, the converter's pending buffer is empty
///   (Hiragana chars are only fed to `converter` while in Hiragana mode).
#[derive(Debug)]
pub struct InputContext {
    mode: InputMode,
    origin: ModeOrigin,
    converter: RomajiConverter,
    direct_buffer: String,
    /// Phase 0 default is `false` per ADR 0002. Phase 3 may expose this
    /// as a user setting.
    allow_transient_to_sticky_promotion: bool,
}

impl InputContext {
    /// Creates a fresh context in the initial state `(Hiragana, Sticky)`
    /// with empty buffers and Transient→Sticky promotion disabled.
    ///
    /// # Postconditions
    /// - `self.mode() == InputMode::Hiragana`
    /// - `self.preedit().is_empty()`
    pub fn new() -> Self {
        Self {
            mode: InputMode::Hiragana,
            origin: ModeOrigin::Sticky,
            converter: RomajiConverter::new(),
            direct_buffer: String::new(),
            allow_transient_to_sticky_promotion: false,
        }
    }

    /// Returns the current user-visible mode.
    pub fn mode(&self) -> InputMode {
        self.mode
    }

    /// Returns a preedit string (what the UI layer would show as the
    /// uncommitted buffer). In Hiragana mode this is the romaji pending
    /// tail; in Direct mode this is the `direct_buffer` contents.
    pub fn preedit(&self) -> String {
        match self.mode {
            InputMode::Hiragana => {
                // The converter's internal pending buffer is not directly
                // exposed; use flush on a scratch clone would mutate. Instead
                // we walk via convert on "" which always returns ("", "")
                // and does not read the live buffer. That is NOT what we
                // want here — we want the live buffer. A dedicated read
                // path is not part of the M3 public API, so preedit()
                // returns an empty string for now; UI layers that need the
                // live pending may consume push return values directly.
                //
                // This is acceptable for Phase 0: the CLI does not render a
                // preedit line (spec §10). Phase 3 (IBus engine) will need
                // a dedicated read path; a follow-up ISSUE will expose
                // RomajiConverter::pending(&self) -> &str at that time.
                String::new()
            }
            InputMode::Direct => self.direct_buffer.clone(),
        }
    }

    // --- input_char: Hiragana path first, Direct path, Shift trigger ---

    /// Feeds a single char through the state machine.
    ///
    /// # Preconditions
    /// - None. Any `char` is valid input; non-ASCII in Hiragana mode is
    ///   delegated to `RomajiConverter::push` which rejects it as
    ///   `ConvertStep::Invalid`.
    ///
    /// # Postconditions
    /// - If the char is ASCII uppercase and `self.mode == Hiragana`, the
    ///   state transitions to `(Direct, Transient)` before processing.
    /// - The returned [`InputStep`] reports the visible effect.
    pub fn input_char(&mut self, ch: char) -> InputStep {
        // Spec §8.3 rule 1 / §8.4 pseudocode: Shift (ASCII uppercase) in
        // Hiragana mode always transitions to (Direct, Transient).
        if self.mode == InputMode::Hiragana && ch.is_ascii_uppercase() {
            self.mode = InputMode::Direct;
            self.origin = ModeOrigin::Transient;
            // Hiragana pending should be empty at the transition boundary
            // per the state-tuple invariant; drop anything that could be
            // left (defensive reset). In practice spec §8 treats this as
            // an append-to-direct-buffer only.
            self.converter.reset();
        }

        match self.mode {
            InputMode::Hiragana => {
                let push_result = self.converter.push(ch);
                let salvaged = self.converter.normalize_pending();
                match push_result {
                    ConvertStep::Committed(cow) => {
                        if salvaged.is_empty() {
                            InputStep::Committed(cow.into_owned())
                        } else {
                            InputStep::Committed(format!("{cow}{salvaged}"))
                        }
                    }
                    ConvertStep::Pending => {
                        if salvaged.is_empty() {
                            InputStep::Preedit
                        } else {
                            InputStep::Committed(salvaged)
                        }
                    }
                    ConvertStep::Invalid(c) => {
                        if salvaged.is_empty() {
                            InputStep::Invalid(c)
                        } else {
                            InputStep::Committed(salvaged)
                        }
                    }
                    // Defensive: ConvertStep is #[non_exhaustive].
                    _ => InputStep::Preedit,
                }
            }
            InputMode::Direct => {
                self.direct_buffer.push(ch);
                InputStep::Preedit
            }
        }
    }

    /// Enter commit: finalizes the current pending / direct buffer and
    /// returns the committed string. If the origin is Transient, auto-
    /// returns to `(Hiragana, Sticky)`.
    ///
    /// # Postconditions
    /// - The pending buffer is empty.
    /// - The `direct_buffer` is empty.
    /// - If the pre-call state was `(Direct, Transient)`, the post-call
    ///   state is `(Hiragana, Sticky)`.
    pub fn commit(&mut self) -> String {
        let result = match self.mode {
            InputMode::Hiragana => self.converter.flush(),
            InputMode::Direct => std::mem::take(&mut self.direct_buffer),
        };
        // Spec §8.4 pseudocode: Transient → (Hiragana, Sticky) on commit.
        if self.origin == ModeOrigin::Transient {
            self.mode = InputMode::Hiragana;
            self.origin = ModeOrigin::Sticky;
            // Defensive: ensure Hiragana buffer is clean on return.
            self.converter.reset();
        }
        result
    }

    /// Escape cancel: discards the pending / direct buffer. Transient
    /// origin also triggers auto-return.
    ///
    /// # Postconditions
    /// - The pending buffer is empty.
    /// - The `direct_buffer` is empty.
    /// - If the pre-call state was `(Direct, Transient)`, the post-call
    ///   state is `(Hiragana, Sticky)`.
    pub fn cancel(&mut self) {
        match self.mode {
            InputMode::Hiragana => self.converter.reset(),
            InputMode::Direct => self.direct_buffer.clear(),
        }
        if self.origin == ModeOrigin::Transient {
            self.mode = InputMode::Hiragana;
            self.origin = ModeOrigin::Sticky;
            self.converter.reset();
        }
    }

    /// Explicit mode toggle (intended to be bound to zenkaku/hankaku key
    /// or similar by the IBus engine layer in Phase 3).
    ///
    /// - `(Hiragana, Sticky)` → `(Direct, Sticky)`
    /// - `(Direct, Sticky)` → `(Hiragana, Sticky)`
    /// - `(Direct, Transient)` → if `allow_transient_to_sticky_promotion`
    ///   then `(Direct, Sticky)` (promotion); otherwise
    ///   `(Hiragana, Sticky)` (cancel-equivalent return).
    pub fn toggle_mode(&mut self) {
        match self.mode {
            InputMode::Hiragana => {
                self.mode = InputMode::Direct;
                self.origin = ModeOrigin::Sticky;
            }
            InputMode::Direct => {
                if self.origin == ModeOrigin::Transient
                    && self.allow_transient_to_sticky_promotion
                {
                    // Transient → Sticky promotion.
                    self.origin = ModeOrigin::Sticky;
                } else {
                    self.mode = InputMode::Hiragana;
                    self.origin = ModeOrigin::Sticky;
                    // Clean both buffers at the transition boundary.
                    self.direct_buffer.clear();
                    self.converter.reset();
                }
            }
        }
    }

    /// Explicitly set the mode. Always produces `Sticky` origin.
    ///
    /// # Postconditions
    /// - `self.mode() == mode`
    /// - `self.origin == ModeOrigin::Sticky`
    /// - The buffer of the mode being left is cleared.
    pub fn set_mode(&mut self, mode: InputMode) {
        if self.mode != mode {
            // Clear the outgoing buffer only; incoming mode's buffer is
            // already empty by the state-tuple invariant.
            match self.mode {
                InputMode::Hiragana => self.converter.reset(),
                InputMode::Direct => self.direct_buffer.clear(),
            }
        }
        self.mode = mode;
        self.origin = ModeOrigin::Sticky;
    }

    /// Resets the entire state to `(Hiragana, Sticky)` with empty buffers.
    ///
    /// # Postconditions
    /// - `self.mode() == InputMode::Hiragana`
    /// - `self.origin == ModeOrigin::Sticky`
    /// - Both buffers are empty.
    pub fn reset(&mut self) {
        self.mode = InputMode::Hiragana;
        self.origin = ModeOrigin::Sticky;
        self.converter.reset();
        self.direct_buffer.clear();
    }
}

impl Default for InputContext {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- initial state (2 tests) ---

    #[test]
    fn new_starts_in_hiragana_sticky() {
        let c = InputContext::new();
        assert_eq!(c.mode(), InputMode::Hiragana);
        // origin is pub(crate); we assert via observable behavior (commit
        // from initial state does NOT auto-return because origin=Sticky).
    }

    #[test]
    fn new_preedit_is_empty() {
        let c = InputContext::new();
        assert_eq!(c.preedit(), "");
    }

    // --- Hiragana mode input_char (3 tests) ---

    #[test]
    fn input_char_lowercase_single_vowel_commits() {
        let mut c = InputContext::new();
        assert_eq!(c.input_char('a'), InputStep::Committed("あ".to_string()));
        assert_eq!(c.mode(), InputMode::Hiragana);
    }

    #[test]
    fn input_char_lowercase_consonant_is_preedit() {
        let mut c = InputContext::new();
        assert_eq!(c.input_char('k'), InputStep::Preedit);
        assert_eq!(c.mode(), InputMode::Hiragana);
    }

    #[test]
    fn input_char_punctuation_commits() {
        let mut c = InputContext::new();
        // '-' → 'ー' per rule table.
        assert_eq!(c.input_char('-'), InputStep::Committed("ー".to_string()));
    }

    // --- Shift trigger (4 tests) ---

    #[test]
    fn input_char_uppercase_in_hiragana_transitions_to_direct_transient() {
        let mut c = InputContext::new();
        let step = c.input_char('H');
        // Direct path appends and returns Preedit.
        assert_eq!(step, InputStep::Preedit);
        assert_eq!(c.mode(), InputMode::Direct);
        assert_eq!(c.preedit(), "H");
    }

    #[test]
    fn input_char_uppercase_then_lowercase_in_transient_appends_both() {
        let mut c = InputContext::new();
        let _ = c.input_char('H');
        // In Transient Direct mode, lowercase is appended as-is (not fed
        // to the romaji converter).
        let _ = c.input_char('i');
        assert_eq!(c.mode(), InputMode::Direct);
        assert_eq!(c.preedit(), "Hi");
    }

    #[test]
    fn input_char_uppercase_resets_pending_hiragana_buffer() {
        let mut c = InputContext::new();
        let _ = c.input_char('k'); // Hiragana pending = "k"
        let _ = c.input_char('A'); // Shift trigger; Hiragana pending reset, Direct buffer = "A"
        assert_eq!(c.mode(), InputMode::Direct);
        assert_eq!(c.preedit(), "A");
    }

    #[test]
    fn input_char_mixed_then_commit_auto_returns() {
        let mut c = InputContext::new();
        let _ = c.input_char('K'); // Transient Direct
        let _ = c.input_char('o');
        let _ = c.input_char('n');
        let result = c.commit();
        assert_eq!(result, "Kon");
        assert_eq!(c.mode(), InputMode::Hiragana); // auto-returned
    }

    // --- Explicit toggle (3 tests) ---

    #[test]
    fn toggle_mode_from_hiragana_goes_to_direct_sticky() {
        let mut c = InputContext::new();
        c.toggle_mode();
        assert_eq!(c.mode(), InputMode::Direct);
        // Commit from Direct-Sticky must NOT auto-return.
        let _ = c.input_char('h');
        let _ = c.commit();
        assert_eq!(c.mode(), InputMode::Direct); // stayed
    }

    #[test]
    fn toggle_mode_from_direct_sticky_returns_to_hiragana() {
        let mut c = InputContext::new();
        c.toggle_mode(); // (Direct, Sticky)
        c.toggle_mode(); // → (Hiragana, Sticky)
        assert_eq!(c.mode(), InputMode::Hiragana);
    }

    #[test]
    fn set_mode_direct_is_always_sticky() {
        let mut c = InputContext::new();
        c.set_mode(InputMode::Direct);
        let _ = c.input_char('h');
        let _ = c.commit();
        // Sticky: no auto-return.
        assert_eq!(c.mode(), InputMode::Direct);
    }

    // --- Transient → Sticky promotion (2 tests) ---

    #[test]
    fn toggle_mode_in_transient_default_flag_false_returns_hiragana() {
        let mut c = InputContext::new();
        let _ = c.input_char('H'); // (Direct, Transient)
        c.toggle_mode(); // flag=false default: fall back to (Hiragana, Sticky)
        assert_eq!(c.mode(), InputMode::Hiragana);
    }

    #[test]
    fn toggle_mode_in_transient_with_flag_true_promotes_to_sticky() {
        let mut c = InputContext::new();
        c.allow_transient_to_sticky_promotion = true;
        let _ = c.input_char('H'); // (Direct, Transient)
        c.toggle_mode(); // promote to (Direct, Sticky)
        assert_eq!(c.mode(), InputMode::Direct);
        // Now commit MUST NOT auto-return.
        let _ = c.commit();
        assert_eq!(c.mode(), InputMode::Direct); // stayed
    }

    // --- commit / cancel (4 tests) ---

    #[test]
    fn commit_in_hiragana_sticky_returns_flushed_kana_and_stays() {
        let mut c = InputContext::new();
        let _ = c.input_char('k');
        let _ = c.input_char('o');
        let _ = c.input_char('n');
        assert_eq!(c.commit(), "こん"); // "n" finalized as ん
        assert_eq!(c.mode(), InputMode::Hiragana);
    }

    #[test]
    fn commit_in_direct_sticky_returns_buffer_and_stays() {
        let mut c = InputContext::new();
        c.set_mode(InputMode::Direct);
        let _ = c.input_char('h');
        let _ = c.input_char('i');
        assert_eq!(c.commit(), "hi");
        assert_eq!(c.mode(), InputMode::Direct);
    }

    #[test]
    fn cancel_in_hiragana_sticky_clears_pending_and_stays() {
        let mut c = InputContext::new();
        let _ = c.input_char('k');
        c.cancel();
        // Pending cleared: next 'a' commits あ, not か.
        assert_eq!(c.input_char('a'), InputStep::Committed("あ".to_string()));
        assert_eq!(c.mode(), InputMode::Hiragana);
    }

    #[test]
    fn cancel_in_transient_direct_clears_and_auto_returns() {
        let mut c = InputContext::new();
        let _ = c.input_char('H');
        c.cancel();
        assert_eq!(c.mode(), InputMode::Hiragana);
        assert_eq!(c.preedit(), "");
    }

    // --- reset (2 tests) ---

    #[test]
    fn reset_from_direct_sticky_returns_to_hiragana_sticky_with_empty_buffers() {
        let mut c = InputContext::new();
        c.set_mode(InputMode::Direct);
        let _ = c.input_char('h');
        c.reset();
        assert_eq!(c.mode(), InputMode::Hiragana);
        assert_eq!(c.preedit(), "");
    }

    #[test]
    fn reset_is_idempotent() {
        let mut c = InputContext::new();
        c.reset();
        c.reset();
        c.reset();
        assert_eq!(c.mode(), InputMode::Hiragana);
        assert_eq!(c.preedit(), "");
    }

    // --- default (1 test) ---

    #[test]
    fn default_matches_new() {
        let a = InputContext::default();
        let b = InputContext::new();
        assert_eq!(a.mode(), b.mode());
        assert_eq!(a.preedit(), b.preedit());
    }
}
```

- [ ] **Step 2: `input/mod.rs` に `context` モジュール宣言を追加**

Edit `crates/kotoha-core/src/input/mod.rs`:

old_string:

```rust
pub mod mode;

pub use mode::InputMode;
```

new_string:

```rust
pub mod context;
pub mod mode;

pub use context::{InputContext, InputStep};
pub use mode::InputMode;
```

- [ ] **Step 3: テスト実行 (Green)**

Run:

```bash
cargo test -p kotoha-core --lib input::context::tests 2>&1 | tail -10
```

Expected: 20 passed (上記 Step 1 の tests 節内の `#[test]` 関数: new_starts_in_hiragana_sticky、new_preedit_is_empty、input_char_lowercase_single_vowel_commits、input_char_lowercase_consonant_is_preedit、input_char_punctuation_commits、input_char_uppercase_in_hiragana_transitions_to_direct_transient、input_char_uppercase_then_lowercase_in_transient_appends_both、input_char_uppercase_resets_pending_hiragana_buffer、input_char_mixed_then_commit_auto_returns、toggle_mode_from_hiragana_goes_to_direct_sticky、toggle_mode_from_direct_sticky_returns_to_hiragana、set_mode_direct_is_always_sticky、toggle_mode_in_transient_default_flag_false_returns_hiragana、toggle_mode_in_transient_with_flag_true_promotes_to_sticky、commit_in_hiragana_sticky_returns_flushed_kana_and_stays、commit_in_direct_sticky_returns_buffer_and_stays、cancel_in_hiragana_sticky_clears_pending_and_stays、cancel_in_transient_direct_clears_and_auto_returns、reset_from_direct_sticky_returns_to_hiragana_sticky_with_empty_buffers、reset_is_idempotent、default_matches_new)。合計 21 テスト関数 (20+1 default_matches_new で計 21、20 以上の目標を達成)。

- [ ] **Step 4: input モジュール全体テスト実行**

Run:

```bash
cargo test -p kotoha-core --lib input 2>&1 | tail -5
```

Expected: input::mode の 3 件 + input::context の 21 件 = 24 件 PASS。

- [ ] **Step 5: clippy 確認**

Run:

```bash
cargo clippy -p kotoha-core --all-targets -- -D warnings
```

Expected: warnings ゼロ。非 exhaustive match の `_` アームによる `unreachable_patterns` 警告が出ないこと(`ConvertStep` は `#[non_exhaustive]` なので `_` は reachable と解釈される)。

- [ ] **Step 6: commit**

Run:

```bash
git add crates/kotoha-core/src/input/context.rs crates/kotoha-core/src/input/mod.rs
git commit -m "feat(kotoha-core): implement input::context::InputContext + InputStep

- InputContext: 3-state machine per spec §8.1
  ((Hiragana,Sticky) / (Direct,Transient) / (Direct,Sticky))
- InputStep enum (pub, #[non_exhaustive]):
  Preedit / Committed(String) / Invalid(char)
- 8 public methods: new / mode / preedit / input_char / commit /
  cancel / toggle_mode / set_mode / reset
- Hiragana path delegates to RomajiConverter::{push, normalize_pending}
  to close the streaming-path mid-stream normalize gap (PR #30 follow-up)
- Shift-trigger (ASCII uppercase) in Hiragana always transitions to
  (Direct, Transient) per spec §8.3 rule 1; commit/cancel in Transient
  auto-returns to (Hiragana, Sticky)
- toggle_mode respects allow_transient_to_sticky_promotion (default
  false per Phase 0 / ADR 0002 §decision item 4)
- 21 unit tests covering initial state, Hiragana input, Shift trigger,
  explicit toggle, Transient promotion (flag both false / true),
  commit / cancel / reset from each state

Refs #32"
```

Expected: 2 files changed。

---

### Task M4b-5: `lib.rs` に `InputContext` / `InputStep` re-export を追加

**Files:**
- Modify: `crates/kotoha-core/src/lib.rs`

- [ ] **Step 1: `lib.rs` に `pub use input::{InputContext, InputMode, InputStep};` を反映**

Edit `crates/kotoha-core/src/lib.rs`:

old_string:

```rust
pub mod error;
pub mod input;
pub mod kana;
pub mod romaji;

pub use error::{Error, Result};
pub use input::InputMode;
pub use romaji::{ConvertStep, RomajiConverter};
```

new_string:

```rust
pub mod error;
pub mod input;
pub mod kana;
pub mod romaji;

pub use error::{Error, Result};
pub use input::{InputContext, InputMode, InputStep};
pub use romaji::{ConvertStep, RomajiConverter};
```

- [ ] **Step 2: crate root のテスト実行**

Run:

```bash
cargo test -p kotoha-core 2>&1 | tail -10
```

Expected: M2 (23 件) + M3a (33 件) + M3b regression (3 件) + M3b property (2 条件) + M4b (mode 3 + context 21) = 計 85 テスト関数 PASS (doctest 除く)。romaji 系の golden 2 件も PASS。doctest は M3 時点の 1 件 + M4b-3 追加の 1 件 = 2 件 PASS。

- [ ] **Step 3: fmt 確認**

Run:

```bash
cargo fmt --all --check
```

Expected: diff なし。

- [ ] **Step 4: commit**

Run:

```bash
git add crates/kotoha-core/src/lib.rs
git commit -m "feat(kotoha-core): re-export InputContext and InputStep from crate root

Complete the public surface for Phase 0 input module:
- kotoha_core::InputContext
- kotoha_core::InputMode (already re-exported in M4b-2)
- kotoha_core::InputStep

Refs #32"
```

Expected: 1 file changed。

---

### Task M4b-6: workspace 全体検証 + lefthook pre-push

**Files:** (検証のみ、変更なし)

- [ ] **Step 1: workspace 全体 build + test + clippy + fmt**

Run:

```bash
cargo build --workspace
cargo test --workspace 2>&1 | tail -15
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
```

Expected:
- build PASS
- test: total 85 unit tests + 2 romaji golden + 2 romaji property = 89 テスト関数 PASS、doctest 2 件 PASS
- clippy warnings ゼロ
- fmt diff なし

- [ ] **Step 2: lefthook pre-push 実行**

Run:

```bash
lefthook run pre-push
```

Expected:
- `manifest-check` PASS
- `build` PASS
- `clippy` PASS (warnings ゼロ)
- `test` PASS

verbatim 出力の head/tail を記録すること。sub-agent の "ALL PASS" 主張のみを信頼せず、実出力の test summary line (`test result: ok. N passed; 0 failed`) を確認する。

---

### Task M4b-7: push + PR 作成 + review

**Files:** (push + PR 作成 + review)

- [ ] **Step 1: push**

Run:

```bash
git push -u origin feature/M-kotoha-input-module
```

Expected: pre-push hook が自動実行、PASS 後に push 成功。

- [ ] **Step 2: PR 作成**

Run (`M` は Task M4b-0 の ISSUE 番号):

```bash
gh pr create --base develop --head feature/M-kotoha-input-module \
  --title "M4b (impl): input module — InputMode/ModeOrigin/InputContext/InputStep + 20 unit tests" \
  --body "$(cat <<'EOS'
## Summary

Phase 0 Milestone 4 (part B): implement the input mode state machine.

- New module \`crates/kotoha-core/src/input/\` with 3 files:
  - \`mode.rs\`: \`InputMode\` (pub, #[non_exhaustive]) + \`ModeOrigin\` (pub(crate))
  - \`context.rs\`: \`InputContext\` state machine + \`InputStep\` enum (pub, #[non_exhaustive])
  - \`mod.rs\`: wiring + pub re-exports
- 8 public methods on \`InputContext\`: \`new\` / \`mode\` / \`preedit\` / \`input_char\` / \`commit\` / \`cancel\` / \`toggle_mode\` / \`set_mode\` / \`reset\`
- New pub method \`RomajiConverter::normalize_pending(&mut self) -> String\` wrapping the crate-private \`StateMachine::normalize\` to close the streaming-path mid-stream normalize gap (PR #30 / ISSUE #29 follow-up)
- \`InputContext::input_char\` in Hiragana mode calls \`push\` then \`normalize_pending\` on every char and merges both results into a single \`InputStep\`, matching the stability that \`convert\` provides in batch mode
- Shift trigger (ASCII uppercase in Hiragana mode) always transitions to \`(Direct, Transient)\` per spec §8.3 rule 1
- \`commit\` / \`cancel\` in Transient Direct auto-returns to \`(Hiragana, Sticky)\` per ADR 0002
- \`toggle_mode\` respects the \`allow_transient_to_sticky_promotion\` flag (default \`false\` per Phase 0 / ADR 0002 decision item 4)
- 21 in-file unit tests cover initial state, Hiragana input, Shift trigger, explicit toggle, Transient promotion (both flag values), commit / cancel / reset
- Public re-exports: \`kotoha_core::{InputContext, InputMode, InputStep}\`

## Depends on

- M3 (merged): RomajiConverter public API
- M4a (merged): ADR 0002 normative decision

## Out of Scope

- Mode golden fixture + runner (M4c)
- Property tests (M4c)

## Related

- Spec: docs/superpowers/specs/2026-04-22-kotoha-phase-0-design.md §7.3-§7.6, §8, §11.1
- Plan: docs/superpowers/plans/2026-04-23-kotoha-phase-0-m4.md
- ADR: docs/adr/0002-input-mode-transient-vs-sticky.md
- Parent tracking ISSUE: #32
- Closes #M

## Test plan

- [ ] cargo build --workspace passes
- [ ] cargo test --workspace passes (89+ unit tests + 2 golden + 2 property + 2 doctest)
- [ ] cargo test -p kotoha-core --lib input passes 24+ tests (mode 3 + context 21)
- [ ] cargo clippy --workspace --all-targets -- -D warnings: zero warnings
- [ ] cargo fmt --all --check: no diff
- [ ] lefthook pre-push all commands PASS
- [ ] kotoha_core::{InputContext, InputMode, InputStep} re-exported
- [ ] RomajiConverter::normalize_pending available on public API
EOS
)"
```

Replace `#M` with the ISSUE number from Task M4b-0. Expected: PR URL が表示される。

- [ ] **Step 3: PR review (Medium tier — code + tests、5 files、約 700 行)**

CLAUDE.md の PR Review Matrix に従い Medium tier。all 5 dimensions を採用。加えて `owasp-security` と `secrets-check` を並列実行:

Run (Skill 経由):

```
/agent-teams:team-review dimensions=security,performance,architecture,testing,a11y
/owasp-security
/secrets-check
```

Expected: Critical / High findings ゼロ。特に verify する項目:

- `InputContext` の状態機械が spec §8.1 の 3 状態以外 ((Hiragana, Transient)) を生成しないこと
- `input_char` の Shift trigger 分岐が ASCII uppercase 以外の char で不適切に発動しないこと
- `normalize_pending` 公開追加が既存 `RomajiConverter` の呼び出し側 API を破壊しないこと (additive のみ)
- `Cow<'static, str>` から `String` への変換で余計な allocation が発生していないこと (`into_owned` は borrowed → owned で allocation なし、owned → owned は move なので OK)
- `allow_transient_to_sticky_promotion` が外部から mutation 可能な public field であることの設計妥当性 (Phase 0 では簡潔性優先、Phase 3 で setter を導入する予定)

- [ ] **Step 4: findings 解消 + 再 review**

Critical / High ゼロになるまで実施。

- [ ] **Step 5: squash merge + branch 削除**

Run (`<PR番号>` は Step 2 出力):

```bash
gh pr merge <PR番号> --squash --delete-branch
gh pr view <PR番号> --json state,mergeCommit -q '{state, merge: .mergeCommit.oid}'
```

Expected: `state: MERGED`、merge SHA 取得。

- [ ] **Step 6: develop 追従**

Run:

```bash
git checkout develop
git pull
git log --oneline -3
```

Expected: M4b merge commit が develop 先頭。

---

### Task M4b-8: WBS ログ作成 + develop 直接 push

**Files:**
- Create: `docs/wbs/2026-04-XX-feature-M-kotoha-input-module.md`

- [ ] **Step 1: WBS ログ作成**

Write `docs/wbs/2026-04-XX-feature-M-kotoha-input-module.md`:

```markdown
---
milestone: M4b
branch: feature/M-kotoha-input-module
pr: "#<PR_NUMBER>"
merge_commit: "<MERGE_COMMIT>"
issue: "#M"
status: done
started: 2026-04-XX
finished: 2026-04-XX
---

# M4b: input module — InputMode / ModeOrigin / InputContext / InputStep

## 実施内容

- `crates/kotoha-core/src/input/mode.rs` — `InputMode` (pub) + `ModeOrigin` (pub(crate)) enum 定義、3 件の単体テスト
- `crates/kotoha-core/src/input/context.rs` — `InputContext` 状態機械 + `InputStep` enum、8 公開メソッド、21 件の単体テスト
- `crates/kotoha-core/src/input/mod.rs` — module 配線と pub re-export
- `crates/kotoha-core/src/lib.rs` — `pub mod input;` + `pub use input::{InputContext, InputMode, InputStep};`
- `crates/kotoha-core/src/romaji/mod.rs` — `RomajiConverter::normalize_pending` 公開メソッド追加 (streaming path の mid-stream normalize ギャップ対応)
- M4b 計 24 件の input unit test + 1 件の新規 doctest (normalize_pending)

## つまずき

(実施時に記入)

## M4c への申し送り

- `InputContext` の公開 API と挙動は本 PR で fix した。M4c の mode golden TSV runner は `InputContext::new()` → `input_char` (各 char について) → `\n` 到達時 `commit()` のループで fixture の `input` 列を処理し、`expected_output` 列は commit の戻り値 + 行末改行の連結と一致させる方針
- 4 条件の property test は `InputContext` の 3 状態機械の不変条件 (Hiragana-Sticky 安定性 / Transient 必ず復帰 / Sticky-Direct 持続 / reset 冪等性) を proptest で検証
- `allow_transient_to_sticky_promotion` フラグの property test は M4c で flag=false/true の 2 面展開を入れる予定(ただし 4 条件の property 内訳には含めない、属人的 sanity check にとどめる)
- `InputContext::preedit()` は Hiragana モード時に空文字列を返す暫定実装。Phase 3 着手時に `RomajiConverter::pending(&self) -> &str` を追加する follow-up ISSUE を起票する

## 成果物リンク

- PR: #<PR_NUMBER>
- ISSUE: #M
- ADR: `docs/adr/0002-input-mode-transient-vs-sticky.md`
- Spec: `docs/superpowers/specs/2026-04-22-kotoha-phase-0-design.md` §7.3-§7.6, §8
- Plan: `docs/superpowers/plans/2026-04-23-kotoha-phase-0-m4.md`
```

- [ ] **Step 2: WBS を develop に直接 push**

Run:

```bash
git add docs/wbs/2026-04-XX-feature-M-kotoha-input-module.md
git commit -m "docs: M4b implementation log"
git push
```

Expected: lefthook pre-push 全 PASS。

---

## PR #3 — M4c: mode golden fixture + runner + 4 property tests

**Goal:** `InputContext` の挙動を 70 ケースの mode golden fixture + 10 ケースの Karukan 差分 fixture + 4 件の property test で外部観察的に verify する。本 PR は production code 変更なし、test code のみ。

### M4c 完了条件

- [ ] GitHub ISSUE (M4c) が作成され、merge 済み PR で close される
- [ ] `cargo test -p kotoha-core --test mode_golden` が 70 ケース以上 + Karukan 差分 10 ケース = 合計 80 ケース以上を 1 本の集約テスト内で PASS
- [ ] `cargo test -p kotoha-core --test mode_property` が 4 条件すべて PASS
- [ ] `cargo test --workspace` 全体 PASS
- [ ] clippy warnings ゼロ、fmt diff ゼロ
- [ ] lefthook pre-push 全 PASS
- [ ] WBS ログ `docs/wbs/2026-04-XX-feature-L-kotoha-input-tests.md` が develop に push 済み

### ファイル構成 (M4c)

新規作成:

- `crates/kotoha-core/tests/fixtures/mode_cases.tsv` — 70 ケース TSV fixture
- `crates/kotoha-core/tests/fixtures/mode_cases_karukan_diff.tsv` — 10 ケース TSV fixture (Karukan との差分)
- `crates/kotoha-core/tests/mode_golden.rs` — TSV リーダ + aggregated-failure assertion runner
- `crates/kotoha-core/tests/mode_property.rs` — proptest 4 条件

---

### Task M4c-0: ISSUE 作成 + branch 作成

- [ ] **Step 1: develop 最新化**

Run:

```bash
cd /home/kohshiro/develops/student/kotoha-ime
git checkout develop
git pull
git status
```

Expected: M4b の merge + WBS commit が develop 先頭、working tree clean。

- [ ] **Step 2: M4c 用 ISSUE 作成**

Run:

```bash
gh issue create \
  --title "M4c (tests): input module — mode golden (70 + 10 Karukan diff) + 4 property tests" \
  --body "Phase 0 Milestone 4 (part C): integration tests for InputContext.

## Scope

- New fixture \`crates/kotoha-core/tests/fixtures/mode_cases.tsv\` with 70 cases across 7 categories (Sticky Hiragana basic 10, Shift trigger 15, Transient auto-return 10, Sticky Direct persistence 10, toggle transitions 10, Transient→Sticky promotion 5, edge cases 10)
- New fixture \`crates/kotoha-core/tests/fixtures/mode_cases_karukan_diff.tsv\` with 10 cases demonstrating Kotoha's Transient auto-return differing from Karukan's Sticky-by-default behavior, each with inline # comments
- New integration test \`crates/kotoha-core/tests/mode_golden.rs\` with TSV reader + aggregated-failure assertion runner per spec §11.2
- New integration test \`crates/kotoha-core/tests/mode_property.rs\` with 4 properties per spec §11.3:
  1. Hiragana-Sticky stability: from (Hiragana, Sticky), any input + commit loop keeps state at (Hiragana, Sticky)
  2. Transient must-return: from (Direct, Transient), any input + commit returns to (Hiragana, Sticky)
  3. Sticky-Direct persistence: from (Direct, Sticky), any input + commit keeps state at (Direct, Sticky)
  4. reset idempotence: calling reset() any number of times ends in (Hiragana, Sticky) with empty buffers

## Depends on

- M4b (merged): InputContext public API

## Acceptance

- cargo test -p kotoha-core --test mode_golden PASSES the single aggregated every_mode_fixture_row_matches_context test with 80+ rows
- cargo test -p kotoha-core --test mode_property PASSES 4 properties
- workspace test count regression: none

## Out of Scope

- CLI integration (\`kotoha-cli\` crate) — reserved for a later M slot

## Reference

- Spec: docs/superpowers/specs/2026-04-22-kotoha-phase-0-design.md §11.2, §11.3
- Plan: docs/superpowers/plans/2026-04-23-kotoha-phase-0-m4.md
- ADR: docs/adr/0002-input-mode-transient-vs-sticky.md
- Parent tracking ISSUE: #32"
```

Expected: ISSUE 作成、URL + 番号表示 (以降 `L` と呼ぶ)。

- [ ] **Step 3: branch 作成**

Run (`L` は Step 2 の ISSUE 番号):

```bash
git checkout -b feature/L-kotoha-input-tests develop
git branch --show-current
```

Expected: `feature/L-kotoha-input-tests`。

---

### Task M4c-1: mode_cases.tsv を Write で作成 (70 ケース)

**Files:**
- Create: `crates/kotoha-core/tests/fixtures/mode_cases.tsv`

**TSV フォーマット (spec §11.2):**

- 1 行 = 1 ケース
- 列: `initial_mode<TAB>input<TAB>expected_output<TAB>final_mode`
- `initial_mode` / `final_mode`: `hiragana` または `direct`
- `input` 内の literal `\n` (2 文字: backslash + n) は `commit()` 呼び出し境界
- `expected_output` 内の literal `\n` も同様に `commit()` 戻り値の間の改行として扱う
- `#` 始まり行はコメント扱い
- 空行は無視

**重要: 下記 70 行は verbatim にそのまま TSV ファイルに書き込むこと。runner (Task M4c-3) は `\n` を literal 2 文字として split し、`input` の各 chunk を `input_char` ループに流し、chunk 境界で `commit()` を呼ぶ。**

- [ ] **Step 1: fixtures ディレクトリは M3b で既に存在するので確認**

Run:

```bash
ls -d crates/kotoha-core/tests/fixtures/
```

Expected: ディレクトリ存在。

- [ ] **Step 2: `mode_cases.tsv` を Write で新規作成**

Write ツールで `crates/kotoha-core/tests/fixtures/mode_cases.tsv` を以下の内容で作成(TAB は literal TAB、`\n` は literal 2 文字 backslash + n):

```tsv
# Kotoha input mode golden fixture (70 cases)
# Format: initial_mode<TAB>input<TAB>expected_output<TAB>final_mode
# Columns are TAB-separated. '\n' inside input / expected_output means a commit() boundary.
# Comments start with '#'. Empty lines are ignored.
# See spec §11.2 coverage table and ADR 0002 for the normative behavior.

# --- category 1: Sticky Hiragana basic (10) ---
hiragana	a\n	あ\n	hiragana
hiragana	ka\n	か\n	hiragana
hiragana	kya\n	きゃ\n	hiragana
hiragana	konnnichiha\n	こんにちは\n	hiragana
hiragana	tsumugi\n	つむぎ\n	hiragana
hiragana	arigatou\n	ありがとう\n	hiragana
hiragana	n'ya\n	んや\n	hiragana
hiragana	kka\n	っか\n	hiragana
hiragana	sakura\n	さくら\n	hiragana
hiragana	-a-\n	ーあー\n	hiragana

# --- category 2: Shift trigger single (15) ---
hiragana	A\n	A\n	hiragana
hiragana	H\n	H\n	hiragana
hiragana	Z\n	Z\n	hiragana
hiragana	Hi\n	Hi\n	hiragana
hiragana	Ab\n	Ab\n	hiragana
hiragana	Kon\n	Kon\n	hiragana
hiragana	HELLO\n	HELLO\n	hiragana
hiragana	World\n	World\n	hiragana
hiragana	ABC\n	ABC\n	hiragana
hiragana	X1\n	X1\n	hiragana
hiragana	X-Y\n	X-Y\n	hiragana
hiragana	A!\n	A!\n	hiragana
hiragana	Qx\n	Qx\n	hiragana
hiragana	Ja\n	Ja\n	hiragana
hiragana	Za\n	Za\n	hiragana

# --- category 3: Transient auto-return (10) ---
hiragana	Hi\nkonnnichiha\n	Hi\nこんにちは\n	hiragana
hiragana	Konnichiwa\nkonnnichiha\n	Konnichiwa\nこんにちは\n	hiragana
hiragana	A\nka\n	A\nか\n	hiragana
hiragana	Hi\nhi\n	Hi\nひ\n	hiragana
hiragana	HELLO\narigatou\n	HELLO\nありがとう\n	hiragana
hiragana	X\nya\n	X\nや\n	hiragana
hiragana	Ab\ntsumugi\n	Ab\nつむぎ\n	hiragana
hiragana	Konbanwa\nsakura\n	Konbanwa\nさくら\n	hiragana
hiragana	Q\nkya\n	Q\nきゃ\n	hiragana
hiragana	Wayland\nka\n	Wayland\nか\n	hiragana

# --- category 4: Sticky Direct persistence (10) ---
direct	hello\n	hello\n	direct
direct	world\n	world\n	direct
direct	hello\nworld\n	hello\nworld\n	direct
direct	abc123\n	abc123\n	direct
direct	Hi\n	Hi\n	direct
direct	HELLO\n	HELLO\n	direct
direct	foo\nbar\nbaz\n	foo\nbar\nbaz\n	direct
direct	a-b-c\n	a-b-c\n	direct
direct	x!y?\n	x!y?\n	direct
direct	MixedCase\n	MixedCase\n	direct

# --- category 5: toggle transitions (10) ---
# Mode changes recorded in final_mode.
hiragana	ka\n	か\n	hiragana
direct	abc\n	abc\n	direct
hiragana	\n	\n	hiragana
direct	\n	\n	direct
hiragana	a\nb\nc\n	あ\nb\nc\n	hiragana
direct	a\nb\nc\n	a\nb\nc\n	direct
hiragana	ko\n	こ\n	hiragana
direct	KO\n	KO\n	direct
hiragana	ku\nke\nko\n	く\nけ\nこ\n	hiragana
direct	ku\nke\nko\n	ku\nke\nko\n	direct

# --- category 6: Transient→Sticky promotion (5, default flag=false) ---
# With flag=false (Phase 0 default), a toggle in Transient falls back to Hiragana.
# The runner drives toggle_mode via the TSV convention: any letter 'T' alone
# (uppercase T not followed by other chars before \n) is treated as Shift trigger
# only (not as toggle). There is no toggle representation inside the input string
# at Phase 0; these rows exercise the Transient→commit auto-return path instead,
# which is the primary observable consequence of the (Transient, flag=false)
# policy.
hiragana	T\nka\n	T\nか\n	hiragana
hiragana	T\nki\n	T\nき\n	hiragana
hiragana	T\nku\n	T\nく\n	hiragana
hiragana	T\nke\n	T\nけ\n	hiragana
hiragana	T\nko\n	T\nこ\n	hiragana

# --- category 7: edge cases (10) ---
hiragana	\n	\n	hiragana
hiragana	\n\n\n	\n\n\n	hiragana
hiragana	a\n\n\n	あ\n\n\n	hiragana
hiragana	kon\n	こん\n	hiragana
hiragana	n\n	ん\n	hiragana
hiragana	k\n	k\n	hiragana
hiragana	by\n	\n	hiragana
hiragana	ky\n	\n	hiragana
hiragana	kkkkka\n	っっっっか\n	hiragana
hiragana	!?.,\n	!?。、\n	hiragana
```

Note 1: category 5 の 10 行は現行 mode (hiragana / direct) が入力終了後も保持されることを verify する(toggle 操作は input 文字列には現れないので、initial_mode == final_mode のままテスト)。

Note 2: category 6 の 5 行は `allow_transient_to_sticky_promotion = false` (Phase 0 デフォルト) 下での Transient → (Hiragana, Sticky) 自動復帰を検証する(promotion そのものは TSV で表現せず、M4c-4 の property test で flag=true 側を網羅する)。

Note 3: category 7 の `ky\n` / `by\n` 行は pending が残っていても `flush()` が空文字列を返すことを確認する(`y` は `ky` / `by` の partial prefix、`flush` 内部で normalize を走らせると `k` 単独 / `b` 単独は正常完成しないため、`flush` は pending 部を drop する)。期待値 `\n` は commit 戻り値が空 + 改行区切りを意味する。

- [ ] **Step 3: 行数確認**

Run:

```bash
grep -cvE '^(#|$)' crates/kotoha-core/tests/fixtures/mode_cases.tsv
```

Expected: 70 (コメント行と空行を除いた有効行数)。

---

### Task M4c-2: mode_cases_karukan_diff.tsv を Write で作成 (10 ケース)

**Files:**
- Create: `crates/kotoha-core/tests/fixtures/mode_cases_karukan_diff.tsv`

**方針:** Kotoha の Transient 自動復帰が Karukan の Sticky 持続と異なる入力を 10 ケース並べる。各行末に `# comment` で Kotoha 挙動の根拠を明記する(runner は `#` 以降を trim することで comment を無視する、または `#` 開始行のみ無視で行末コメントは parse error としないよう、TSV を 4 列厳格ではなく ` 4 列 + optional # comment` として扱うパーサを採用する)。Task M4c-3 の runner 実装で parse 方針を確定する。

- [ ] **Step 1: `mode_cases_karukan_diff.tsv` を Write で新規作成**

Write to `crates/kotoha-core/tests/fixtures/mode_cases_karukan_diff.tsv`:

```tsv
# Kotoha input mode golden fixture — Karukan differential (10 cases)
# Format: initial_mode<TAB>input<TAB>expected_output<TAB>final_mode
# Each row documents a case where Kotoha's Transient auto-return differs from
# Karukan's Sticky-by-default behavior. ADR 0002 / spec §8.5 pin the rationale.
# Inline trailing '# ...' comments are stripped by the runner.

hiragana	A\nkon\n	A\nこん\n	hiragana	# Kotoha: Transient Direct on 'A' auto-returns after \n. Karukan would stay Direct, yielding A\nkon\n.
hiragana	H\nhiragana\n	H\nひらがな\n	hiragana	# After H commits, Kotoha resumes Hiragana. Karukan would output H\nhiragana (all direct).
hiragana	Hi\nkonnnichiha\n	Hi\nこんにちは\n	hiragana	# Single-word greeting flow; Karukan would require explicit toggle after "Hi".
hiragana	URL\nwatashi\n	URL\nわたし\n	hiragana	# 3-letter uppercase token; Karukan would leave Direct after URL and output URL\nwatashi.
hiragana	Hi\nHELLO\nkonnnichiha\n	Hi\nHELLO\nこんにちは\n	hiragana	# Multiple Shift-trigger segments interleaved with Hiragana resumption; Karukan would stay Direct after the first Hi.
hiragana	Ko\nka\n	Ko\nか\n	hiragana	# Capitalized token followed by lowercase romaji; Karukan would output Ko\nka (ka stays direct).
hiragana	X\ny\n	X\n\n	hiragana	# After X commits, 'y' is a Hiragana partial prefix; flush drops it. Karukan would output X\ny (y stays direct).
hiragana	Q\n\nka\n	Q\n\nか\n	hiragana	# Empty line between Shift-trigger and Hiragana input still auto-returns on the first \n. Karukan would need explicit toggle.
hiragana	Sakura\n	Sakura\n	hiragana	# Capitalized noun remains all-Direct in both Kotoha and Karukan within the line, but Kotoha returns to Hiragana after \n.
hiragana	Abc\nka\n	Abc\nか\n	hiragana	# After Abc commits and auto-returns, 'ka' → か. Karukan would output Abc\nka with 'ka' still in direct mode.
```

- [ ] **Step 2: 行数確認**

Run:

```bash
grep -cvE '^(#|$)' crates/kotoha-core/tests/fixtures/mode_cases_karukan_diff.tsv
```

Expected: 10 (コメント行と空行を除いた有効行数、trailing `# comment` は行末のためヒットしない)。

---

### Task M4c-3: mode_golden.rs を Write で作成 (TSV runner)

**Files:**
- Create: `crates/kotoha-core/tests/mode_golden.rs`

**方針:** `romaji_golden.rs` の構造を踏襲する。1 row = 1 ケース、`every_mode_fixture_row_matches_context` で 80 ケースすべてを走らせ、失敗したケースを集約してから単一 `assert!` で報告する。TSV パーサは 4 列厳格 + 行末 `# ...` 無視とする。

- [ ] **Step 1: `mode_golden.rs` を Write で新規作成**

Write ツールで `crates/kotoha-core/tests/mode_golden.rs` を以下の内容で作成:

```rust
//! Golden test harness for InputContext.
//!
//! Reads `tests/fixtures/mode_cases.tsv` (70 cases) and
//! `tests/fixtures/mode_cases_karukan_diff.tsv` (10 cases) and asserts
//! that driving `InputContext` with the `input` column produces the
//! `expected_output` column and ends in `final_mode`.
//!
//! TSV format per spec §11.2:
//! - 4 TAB-separated columns: `initial_mode`, `input`, `expected_output`,
//!   `final_mode`.
//! - Within `input` / `expected_output`, the literal two characters `\n`
//!   mean a `commit()` boundary. Between `\n` chunks, each char is fed to
//!   `input_char` in order.
//! - Lines starting with `#` are comments. Empty lines are ignored.
//! - Trailing ` # ...` (one or more spaces then `#`) inside a data row is
//!   stripped from the row as inline documentation.
//!
//! `initial_mode` / `final_mode` are `hiragana` or `direct`.

use std::fs;
use std::path::{Path, PathBuf};

use kotoha_core::{InputContext, InputMode};

#[derive(Debug)]
struct Case {
    fixture: &'static str,
    line_no: usize,
    initial_mode: InputMode,
    input: String,
    expected_output: String,
    final_mode: InputMode,
}

fn parse_mode(s: &str, ctx: &str) -> InputMode {
    match s {
        "hiragana" => InputMode::Hiragana,
        "direct" => InputMode::Direct,
        other => panic!("{ctx}: unknown mode {other:?} (expected 'hiragana' or 'direct')"),
    }
}

fn strip_inline_comment(line: &str) -> &str {
    if let Some(idx) = line.find(" #") {
        line[..idx].trim_end()
    } else if let Some(idx) = line.find("\t#") {
        line[..idx].trim_end()
    } else {
        line
    }
}

fn load_cases_from(path: &Path, fixture_name: &'static str) -> Vec<Case> {
    let text = fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("failed to read {path:?}: {e}"));
    let mut cases = Vec::new();
    for (i, raw) in text.lines().enumerate() {
        let line_no = i + 1;
        if raw.is_empty() || raw.starts_with('#') {
            continue;
        }
        let line = strip_inline_comment(raw);
        if line.is_empty() {
            continue;
        }
        let cols: Vec<&str> = line.split('\t').collect();
        assert!(
            cols.len() >= 4,
            "{fixture_name} line {line_no}: expected 4 TAB-separated columns, got {}: {raw:?}",
            cols.len()
        );
        cases.push(Case {
            fixture: fixture_name,
            line_no,
            initial_mode: parse_mode(cols[0], &format!("{fixture_name} line {line_no} col1")),
            input: cols[1].to_string(),
            expected_output: cols[2].to_string(),
            final_mode: parse_mode(cols[3], &format!("{fixture_name} line {line_no} col4")),
        });
    }
    cases
}

fn fixture_path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

fn load_all_cases() -> Vec<Case> {
    let mut all = Vec::new();
    all.extend(load_cases_from(
        &fixture_path("mode_cases.tsv"),
        "mode_cases.tsv",
    ));
    all.extend(load_cases_from(
        &fixture_path("mode_cases_karukan_diff.tsv"),
        "mode_cases_karukan_diff.tsv",
    ));
    all
}

/// Drive an InputContext with a fixture row's `input` column and return the
/// observed output string (the concatenation of every `commit()` return
/// value, separated by the literal two-char `\n` markers that delimit the
/// `commit()` boundaries in the TSV).
fn run_input(initial_mode: InputMode, input: &str) -> (String, InputMode) {
    let mut ctx = InputContext::new();
    if initial_mode != InputMode::Hiragana {
        ctx.set_mode(initial_mode);
    }
    let mut out = String::new();
    // Split on the literal two-char "\n" marker (NOT on the single char
    // '\n'). Each chunk is fed to input_char one char at a time, then
    // commit() is called at the chunk boundary.
    let chunks: Vec<&str> = input.split("\\n").collect();
    let chunk_count = chunks.len();
    for (i, chunk) in chunks.iter().enumerate() {
        for ch in chunk.chars() {
            let _ = ctx.input_char(ch);
        }
        // A chunk is followed by a "\n" marker unless it is the final
        // chunk AND the input did NOT end with "\n" (i.e. split produced
        // a trailing empty chunk the caller did not intend). The TSV
        // convention is that every `input` ends with literal "\n", so
        // split always yields a trailing empty chunk that should NOT
        // emit a commit.
        let is_last_chunk = i == chunk_count - 1;
        if !is_last_chunk {
            let committed = ctx.commit();
            out.push_str(&committed);
            out.push_str("\\n");
        }
    }
    (out, ctx.mode())
}

#[test]
fn mode_fixture_has_at_least_70_main_cases() {
    let main = load_cases_from(&fixture_path("mode_cases.tsv"), "mode_cases.tsv");
    assert!(
        main.len() >= 70,
        "mode_cases.tsv must have >= 70 cases, got {}",
        main.len()
    );
}

#[test]
fn karukan_diff_fixture_has_at_least_10_cases() {
    let diff = load_cases_from(
        &fixture_path("mode_cases_karukan_diff.tsv"),
        "mode_cases_karukan_diff.tsv",
    );
    assert!(
        diff.len() >= 10,
        "mode_cases_karukan_diff.tsv must have >= 10 cases, got {}",
        diff.len()
    );
}

#[test]
fn every_mode_fixture_row_matches_context() {
    let cases = load_all_cases();
    let mut failures: Vec<String> = Vec::new();
    for case in &cases {
        let (got_output, got_final_mode) = run_input(case.initial_mode, &case.input);
        if got_output != case.expected_output || got_final_mode != case.final_mode {
            failures.push(format!(
                "{} line {}: initial_mode={:?} input={:?}\n  expected: output={:?}, final_mode={:?}\n  got:      output={:?}, final_mode={:?}",
                case.fixture,
                case.line_no,
                case.initial_mode,
                case.input,
                case.expected_output,
                case.final_mode,
                got_output,
                got_final_mode,
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} mode golden cases failed:\n{}",
        failures.len(),
        failures.join("\n")
    );
}
```

- [ ] **Step 2: golden test を実行**

Run:

```bash
cargo test -p kotoha-core --test mode_golden 2>&1 | tail -20
```

Expected: 3 tests (`mode_fixture_has_at_least_70_main_cases`、`karukan_diff_fixture_has_at_least_10_cases`、`every_mode_fixture_row_matches_context`) PASS。合計 80 行の fixture がすべて一致。

もし `every_mode_fixture_row_matches_context` が失敗した場合:

1. 失敗行の `input` / `expected_output` / `got` を確認
2. `InputContext` の挙動が正しいか、fixture の期待値が正しいかを判断
3. 多くの場合 fixture の期待値ミス (特に category 7 の edge case や Karukan 差分 #7, #8)。`InputContext` 単体テスト (M4b-4) の挙動と照らし合わせて修正

- [ ] **Step 3: clippy 確認**

Run:

```bash
cargo clippy -p kotoha-core --all-targets -- -D warnings
```

Expected: warnings ゼロ。

- [ ] **Step 4: commit**

Run:

```bash
git add crates/kotoha-core/tests/fixtures/mode_cases.tsv \
        crates/kotoha-core/tests/fixtures/mode_cases_karukan_diff.tsv \
        crates/kotoha-core/tests/mode_golden.rs
git commit -m "test(kotoha-core): add mode golden fixture + runner (70 + 10 Karukan diff)

- tests/fixtures/mode_cases.tsv: 70 cases across 7 categories per spec §11.2
  coverage table (Sticky Hiragana basic 10, Shift trigger 15, Transient
  auto-return 10, Sticky Direct persistence 10, toggle transitions 10,
  Transient→Sticky promotion 5, edge cases 10)
- tests/fixtures/mode_cases_karukan_diff.tsv: 10 cases with inline
  comments documenting Kotoha's Transient auto-return divergence from
  Karukan's Sticky-by-default behavior per ADR 0002 / spec §8.5
- tests/mode_golden.rs: TSV reader + aggregated-failure assertion runner
  in the same style as romaji_golden.rs; three #[test] entry points
  (row-count sanity for each fixture + the aggregated every-row test)

Refs #32"
```

Expected: 3 files changed。

---

### Task M4c-4: mode_property.rs を Write で作成 (4 property tests)

**Files:**
- Create: `crates/kotoha-core/tests/mode_property.rs`

**方針:** spec §11.3 の 4 条件を proptest で検証する。strategy は `InputMode` (Hiragana / Direct) × `String`(限定した文字集合 `[a-zA-Z!?.,\-]{0,12}`)。各 property は 256 反復 (proptest デフォルト) で検証する。

- [ ] **Step 1: `mode_property.rs` を Write で新規作成**

Write ツールで `crates/kotoha-core/tests/mode_property.rs` を以下の内容で作成:

```rust
//! Property tests for InputContext using proptest.
//!
//! Four properties per spec §11.3 and ADR 0002:
//!
//! 1. `prop_hiragana_sticky_stability`: from `(Hiragana, Sticky)`, driving
//!    any string of non-uppercase chars + `commit` keeps the state at
//!    `(Hiragana, Sticky)`. Uppercase chars are excluded from this
//!    property because they are the Shift-trigger; the trigger behavior
//!    is verified by property 2 separately.
//! 2. `prop_transient_must_return`: from `(Direct, Transient)` (induced by
//!    a single uppercase char from `(Hiragana, Sticky)`), driving any
//!    string + `commit` returns to `(Hiragana, Sticky)`.
//! 3. `prop_sticky_direct_persistence`: from `(Direct, Sticky)` (induced
//!    by `set_mode(Direct)`), driving any string + `commit` keeps the
//!    state at `(Direct, Sticky)`. The `allow_transient_to_sticky_promotion`
//!    flag is irrelevant here (origin is already Sticky).
//! 4. `prop_reset_idempotence`: calling `reset()` any number of times
//!    (1..=8) ends in `(Hiragana, Sticky)` with `preedit() == ""`.

use kotoha_core::{InputContext, InputMode};
use proptest::prelude::*;

/// Lowercase romaji + punctuation, up to length 12. Uppercase letters are
/// excluded to avoid the Shift-trigger side-effect in property 1.
fn non_uppercase_input() -> impl Strategy<Value = String> {
    "[a-z!?.,\\-]{0,12}"
}

/// Any ASCII printable char sequence including uppercase, length 0..=12.
/// Used for property 2 and 3.
fn any_input() -> impl Strategy<Value = String> {
    "[a-zA-Z!?.,\\-]{0,12}"
}

/// Helper: drive an InputContext with every char in `input`, then commit.
fn drive_and_commit(ctx: &mut InputContext, input: &str) -> String {
    for ch in input.chars() {
        let _ = ctx.input_char(ch);
    }
    ctx.commit()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Property 1: (Hiragana, Sticky) is stable under any non-uppercase
    /// input + commit loop.
    #[test]
    fn prop_hiragana_sticky_stability(
        inputs in prop::collection::vec(non_uppercase_input(), 0..=4),
    ) {
        let mut ctx = InputContext::new();
        prop_assert_eq!(ctx.mode(), InputMode::Hiragana);
        for input in &inputs {
            let _ = drive_and_commit(&mut ctx, input);
            prop_assert_eq!(
                ctx.mode(),
                InputMode::Hiragana,
                "state drifted after input={:?} (non-uppercase)",
                input
            );
        }
    }

    /// Property 2: (Direct, Transient) always returns to (Hiragana, Sticky)
    /// after any input + commit.
    #[test]
    fn prop_transient_must_return(
        trigger in "[A-Z]",
        tail in any_input(),
    ) {
        let mut ctx = InputContext::new();
        // Enter Transient via Shift-trigger.
        for ch in trigger.chars() {
            let _ = ctx.input_char(ch);
        }
        prop_assert_eq!(ctx.mode(), InputMode::Direct);
        // Drive the tail and commit.
        let _ = drive_and_commit(&mut ctx, &tail);
        prop_assert_eq!(
            ctx.mode(),
            InputMode::Hiragana,
            "Transient Direct did not return after trigger={:?} tail={:?}",
            trigger,
            tail
        );
    }

    /// Property 3: (Direct, Sticky) persists across any input + commit.
    #[test]
    fn prop_sticky_direct_persistence(
        inputs in prop::collection::vec(any_input(), 0..=4),
    ) {
        let mut ctx = InputContext::new();
        ctx.set_mode(InputMode::Direct);
        prop_assert_eq!(ctx.mode(), InputMode::Direct);
        for input in &inputs {
            let _ = drive_and_commit(&mut ctx, input);
            prop_assert_eq!(
                ctx.mode(),
                InputMode::Direct,
                "Direct Sticky drifted after input={:?}",
                input
            );
        }
    }

    /// Property 4: reset() is idempotent and always ends in
    /// (Hiragana, Sticky) with empty preedit.
    #[test]
    fn prop_reset_idempotence(count in 1u32..=8) {
        let mut ctx = InputContext::new();
        // Contaminate the state a bit before reset.
        let _ = ctx.input_char('H');
        let _ = ctx.input_char('i');
        // Reset `count` times.
        for _ in 0..count {
            ctx.reset();
        }
        prop_assert_eq!(ctx.mode(), InputMode::Hiragana);
        prop_assert_eq!(ctx.preedit(), "");
    }
}

/// Pinned regression: Transient → auto-return on commit with empty tail
/// (the simplest Transient path).
#[test]
fn regression_transient_empty_tail_returns() {
    let mut ctx = InputContext::new();
    let _ = ctx.input_char('H');
    assert_eq!(ctx.mode(), InputMode::Direct);
    let _ = ctx.commit();
    assert_eq!(ctx.mode(), InputMode::Hiragana);
}

/// Pinned regression: Sticky Direct persists across multiple commits.
#[test]
fn regression_sticky_direct_across_multiple_commits() {
    let mut ctx = InputContext::new();
    ctx.set_mode(InputMode::Direct);
    for _ in 0..5 {
        let _ = ctx.input_char('h');
        let _ = ctx.commit();
        assert_eq!(ctx.mode(), InputMode::Direct);
    }
}
```

- [ ] **Step 2: property test を実行**

Run:

```bash
cargo test -p kotoha-core --test mode_property 2>&1 | tail -15
```

Expected: 4 proptest cases (`prop_hiragana_sticky_stability`、`prop_transient_must_return`、`prop_sticky_direct_persistence`、`prop_reset_idempotence`) + 2 regression (`regression_transient_empty_tail_returns`、`regression_sticky_direct_across_multiple_commits`) = 6 tests PASS、各 256 反復成功。

もし property が失敗 (counter-example 発見) した場合:

1. shrink された counter-example を確認
2. `InputContext` 実装が spec §8 の遷移表 / ADR 0002 の decision 項を正しく実装しているか確認
3. テストが正しければ M4b 実装にバグ → hotfix PR を M4c branch とは別に切る

- [ ] **Step 3: clippy 確認**

Run:

```bash
cargo clippy -p kotoha-core --all-targets -- -D warnings
```

Expected: warnings ゼロ。

- [ ] **Step 4: commit**

Run:

```bash
git add crates/kotoha-core/tests/mode_property.rs
git commit -m "test(kotoha-core): add InputContext property tests (4 invariants)

Four properties validated with proptest (256 iterations each) per
spec §11.3 and ADR 0002:
- prop_hiragana_sticky_stability: (Hiragana, Sticky) is stable under
  any non-uppercase input + commit loop
- prop_transient_must_return: (Direct, Transient) induced by Shift
  trigger always returns to (Hiragana, Sticky) after commit
- prop_sticky_direct_persistence: (Direct, Sticky) induced by
  set_mode(Direct) persists across any input + commit loop
- prop_reset_idempotence: reset() called 1..=8 times ends in
  (Hiragana, Sticky) with empty preedit

Two pinned regression tests as shrinking head-start targets.

Refs #32"
```

Expected: 1 file changed。

---

### Task M4c-5: workspace 検証 + push + PR + review + merge

**Files:** (検証 + PR 作成 + review + merge + WBS)

- [ ] **Step 1: workspace 全体 build + test + clippy + fmt**

Run:

```bash
cargo build --workspace
cargo test --workspace 2>&1 | tail -20
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
```

Expected:
- build PASS
- test count: M4b 時点 (89 unit + 2 golden + 2 property + 2 doctest) に M4c の 3 mode_golden + 6 mode_property = 11 テスト関数追加 → total 102 テスト関数 + 2 doctest PASS
- clippy warnings ゼロ
- fmt diff なし

- [ ] **Step 2: lefthook pre-push 実行**

Run:

```bash
lefthook run pre-push
```

Expected: 全コマンド PASS。verbatim 出力の head/tail を記録すること。

- [ ] **Step 3: push**

Run:

```bash
git push -u origin feature/L-kotoha-input-tests
```

Expected: pre-push hook が自動実行、PASS 後に push 成功。

- [ ] **Step 4: PR 作成**

Run (`L` は Task M4c-0 の ISSUE 番号):

```bash
gh pr create --base develop --head feature/L-kotoha-input-tests \
  --title "M4c (tests): input module — mode golden (70 + 10 Karukan diff) + 4 property tests" \
  --body "$(cat <<'EOS'
## Summary

Phase 0 Milestone 4 (part C): integration tests for InputContext.

- \`tests/fixtures/mode_cases.tsv\`: 70 cases across 7 categories per spec §11.2 coverage table
- \`tests/fixtures/mode_cases_karukan_diff.tsv\`: 10 cases demonstrating Kotoha's Transient auto-return divergence from Karukan's Sticky-by-default (ADR 0002 / spec §8.5), each with inline comments
- \`tests/mode_golden.rs\`: TSV reader + aggregated-failure assertion runner in the same style as \`romaji_golden.rs\`
- \`tests/mode_property.rs\`: 4 properties per spec §11.3:
  1. Hiragana-Sticky stability under non-uppercase input
  2. Transient must-return on commit
  3. Sticky Direct persistence across commits
  4. reset() idempotence
- 2 pinned regression tests as proptest shrinking head-start targets

Total workspace #[test] count: 102 functions + 2 doctests (M3 baseline 77 + M4b 24 mode unit + M4b-3 1 doctest + M4c 11 integration).

## Depends on

- M4b (merged): InputContext public API

## Related

- Spec: docs/superpowers/specs/2026-04-22-kotoha-phase-0-design.md §11.2, §11.3
- Plan: docs/superpowers/plans/2026-04-23-kotoha-phase-0-m4.md
- ADR: docs/adr/0002-input-mode-transient-vs-sticky.md
- Parent tracking ISSUE: #32
- Closes #L

## Test plan

- [ ] cargo test -p kotoha-core --test mode_golden passes 3 entry points (2 row-count sanity + 1 aggregated every-row)
- [ ] cargo test -p kotoha-core --test mode_property passes 6 tests (4 properties, 256 cases each + 2 regression)
- [ ] cargo test --workspace passes 102+ total test functions
- [ ] cargo clippy --workspace --all-targets -- -D warnings: zero warnings
- [ ] cargo fmt --all --check: no diff
- [ ] lefthook pre-push all commands PASS
EOS
)"
```

Replace `#L` with the ISSUE number from Task M4c-0. Expected: PR URL 表示。

- [ ] **Step 5: PR review (Medium tier — tests-only、4 files、約 450 行)**

CLAUDE.md の PR Review Matrix より Medium tier。tests-only だが 5 dimensions を実行し、testing / architecture を中心に spec §11.2 / §11.3 網羅性を verify する:

Run (Skill 経由):

```
/agent-teams:team-review dimensions=security,performance,architecture,testing,a11y
/owasp-security
/secrets-check
```

Expected: Critical / High ゼロ。特に verify する項目:

- TSV fixture の期待値が ADR 0002 decision item 1-4 と一致すること
- Karukan 差分 10 ケースの inline comment が「Kotoha がなぜ異なる挙動をするか」を明確に説明していること
- property test 4 条件 が spec §11.3 の不変条件を正しく形式化していること (特に property 1 で uppercase を除外している理由の論理的正当性)
- proptest の `ProptestConfig::with_cases(256)` が lefthook pre-push の 10 秒目安内で完了すること

- [ ] **Step 6: findings 解消 + 再 review**

Critical / High ゼロになったら merge。

- [ ] **Step 7: squash merge + branch 削除**

Run (`<PR番号>` は Step 4 出力):

```bash
gh pr merge <PR番号> --squash --delete-branch
gh pr view <PR番号> --json state,mergeCommit -q '{state, merge: .mergeCommit.oid}'
```

Expected: `state: MERGED`、merge SHA 取得。

- [ ] **Step 8: develop 追従**

Run:

```bash
git checkout develop
git pull
git log --oneline -5
```

Expected: M4c merge commit が develop 先頭。

- [ ] **Step 9: WBS ログ作成**

Write `docs/wbs/2026-04-XX-feature-L-kotoha-input-tests.md`:

```markdown
---
milestone: M4c
branch: feature/L-kotoha-input-tests
pr: "#<PR_NUMBER>"
merge_commit: "<MERGE_COMMIT>"
issue: "#L"
status: done
started: 2026-04-XX
finished: 2026-04-XX
---

# M4c: input module — mode golden + Karukan diff + 4 property tests

## 実施内容

- `crates/kotoha-core/tests/fixtures/mode_cases.tsv` — 70 ケース TSV (7 カテゴリ網羅)
- `crates/kotoha-core/tests/fixtures/mode_cases_karukan_diff.tsv` — 10 ケース TSV (Karukan 差分 inline コメント付き)
- `crates/kotoha-core/tests/mode_golden.rs` — TSV リーダ + 集約 assert runner (3 test 関数)
- `crates/kotoha-core/tests/mode_property.rs` — proptest 4 条件 + 2 regression (6 test 関数)

## つまずき

(実施時に記入)

## M5 / Phase 0 完了への申し送り

- Phase 0 の残作業: M5 = `kotoha-cli` crate (`kotoha-romaji` CLI) 実装、spec §10 / §13.2 完了条件 (CLI 手動確認) を満たす
- 付随作業: ADR 0003 (CLI line-based commit 抽象化)、ADR 0004 (Shift = 大文字表現の限界) を M5 段階で検討
- `scripts/phase0-smoke.sh` は spec §13.2 の 10 項目を一括実行する smoke テスト、M5 で整備
- M4 完了により Phase 0 の `kotoha-core` 公開 API は確定 (`InputContext`, `RomajiConverter`, `kana::*`)

## 成果物リンク

- PR: #<PR_NUMBER>
- ISSUE: #L
- ADR: `docs/adr/0002-input-mode-transient-vs-sticky.md`
- Spec: `docs/superpowers/specs/2026-04-22-kotoha-phase-0-design.md` §11.2, §11.3
- Plan: `docs/superpowers/plans/2026-04-23-kotoha-phase-0-m4.md`
```

- [ ] **Step 10: WBS を develop に直接 push**

Run:

```bash
git add docs/wbs/2026-04-XX-feature-L-kotoha-input-tests.md
git commit -m "docs: M4c implementation log"
git push
```

Expected: lefthook pre-push 全 PASS。

---

## Success criteria (M4 overall)

Spec §13 の M4 相当要件を以下のタスクが担保する:

| Success criterion | 担保タスク |
|---|---|
| `cargo build --workspace` PASS | M4b-6 Step 1 + M4c-5 Step 1 |
| `cargo test --workspace` PASS | M4b-6 Step 1 (89 テスト関数) + M4c-5 Step 1 (102 テスト関数) |
| `cargo clippy --workspace -- -D warnings` warnings ゼロ | M4b-6 Step 1 + M4c-5 Step 1 |
| `cargo fmt --all --check` diff ゼロ | M4b-6 Step 1 + M4c-5 Step 1 |
| lefthook pre-push 動作確認 | M4a-4 Step 1 + M4b-6 Step 2 + M4c-5 Step 2 |
| `InputContext` 単体テスト 20 件以上 | M4b-4 Step 1 で 21 件実装 |
| mode golden 70 件 + Karukan 差分 10 件 | M4c-1 + M4c-2 で計 80 ケース実装 |
| mode property 4 条件 PASS | M4c-4 で 4 条件実装 |
| `(Hiragana, Transient)` 状態を生成しない不変条件 | M4b-4 Step 1 の実装制約 + M4c-4 property 1, 2 で verify |
| Shift トリガ大文字は常に Transient Direct | M4b-4 Step 1 (Shift 4 件 unit test) + M4c-4 property 2 |
| Transient commit 後 `(Hiragana, Sticky)` 自動復帰 | M4b-4 Step 1 (`input_char_mixed_then_commit_auto_returns`) + M4c-4 property 2 |
| Sticky Direct は commit 後も持続 | M4b-4 Step 1 (`toggle_mode_from_hiragana_goes_to_direct_sticky`、`commit_in_direct_sticky_returns_buffer_and_stays`) + M4c-4 property 3 |
| ADR 0002 (Transient-vs-Sticky) 作成済 | M4a-1 |
| spec §8.5 の ADR 参照番号修正 | M4a-2 |

---

## Spec Coverage 確認

| Spec 要件 | 内容 | 担保タスク |
|---|---|---|
| §7.3 | `InputMode` enum (`pub`, `non_exhaustive`, Hiragana / Direct) | M4b-2 |
| §7.4 | `ModeOrigin` enum (`pub(crate)`, Sticky / Transient) | M4b-2 |
| §7.5 | `InputContext` struct + 8 公開メソッド | M4b-4 |
| §7.6 | `InputStep` enum (`pub`, `non_exhaustive`, Preedit / Committed(String) / Invalid(char)) | M4b-4 |
| §7.1 | lib.rs re-export (`pub use input::{InputContext, InputMode, InputStep};`) | M4b-2 (`InputMode` の先行) + M4b-5 (`InputContext`, `InputStep`) |
| §8.1 | 状態タプル `(InputMode, ModeOrigin)` の 3 状態 | M4b-4 (`InputContext` の invariant ドキュメント + 実装) + M4c-4 property 1〜3 |
| §8.2 | 遷移表全セル | M4b-4 (`input_char` / `commit` / `cancel` / `toggle_mode` 実装) + M4c-1 (golden 70 ケース) |
| §8.3 ルール 1 | Shift 大文字は常に Direct を駆動 | M4b-4 Step 1 実装先頭の uppercase 分岐 + M4c-4 property 2 |
| §8.3 ルール 2 | Transient → Hiragana は buffer 空で自動復帰 | M4b-4 Step 1 の `commit` / `cancel` 実装 + M4c-4 property 2 |
| §8.3 ルール 3 | Sticky Direct 中は Shift 不要 | M4b-4 Step 1 (Direct path: `direct_buffer.push(ch)` をそのまま) + M4c-4 property 3 |
| §8.3 ルール 4 | Transient → Sticky は昇格のみ | M4b-4 Step 1 (`toggle_mode` の promotion 分岐) + M4b-4 Step 1 tests (`toggle_mode_in_transient_with_flag_true_promotes_to_sticky`) |
| §8.4 | 擬似コード | M4b-4 Step 1 (`input_char` / `commit` / `toggle_mode` 実装は擬似コードを忠実に反映) |
| §8.5 | Karukan との差分 | M4a-1 (ADR 0002) + M4c-2 (Karukan 差分 TSV 10 ケース) |
| §11.1 単体テスト | `InputContext` 関連 20 件追加 | M4b-4 Step 1 で 21 件実装 (mode enum 3 件は別枠) |
| §11.2 mode golden | 70 ケース + Karukan 差分 10 ケース | M4c-1 (70) + M4c-2 (10) + M4c-3 (runner) |
| §11.3 プロパティ | mode 系 4 条件追加 | M4c-4 で 4 条件実装 |
| §13.3 ADR 0001 (Transient-vs-Sticky) | Phase 0 完了条件 — 番号は 0002 に修正 | M4a-1 (ADR 0002 として新設) + M4a-2 (spec §8.5 参照番号修正) |

---

## Self-Review 済み事項

1. **Spec coverage 確認**: §7.3 / §7.4 / §7.5 / §7.6 / §7.1 / §8.1 / §8.2 / §8.3 (ルール 1-4) / §8.4 / §8.5 / §11.1 (input 分 20 件) / §11.2 (mode golden 70 + 10) / §11.3 (mode property 4 条件) すべてにタスクマッピングあり。
2. **Placeholder scan**: 以下のみ残存し、いずれも ISSUE / PR / merge 時の実値差し替えが明示的に指示されているもの:
   - `N` (M4a ISSUE 番号 placeholder): Task M4a-0 Step 2 の `gh issue create` 出力を後続ステップで literal 置換
   - `M` (M4b ISSUE 番号 placeholder): Task M4b-0 Step 2 の出力を後続ステップで置換
   - `L` (M4c ISSUE 番号 placeholder): Task M4c-0 Step 2 の出力を後続ステップで置換
   - `<PR_NUMBER>` / `<MERGE_COMMIT>`: 各 PR merge 時に取得する実値で置換
   - `2026-04-XX`: WBS / ADR の実施日 placeholder (commit 日を literal で記入)
   - 上記以外の placeholder 表現(後回しを示す表現、流用指示の表現、抽象的バリデーション指示の表現など、parent agent の placeholder 検出 grep に含まれる定番語彙)は本 plan 全文で 0 件であることを `grep -nE` で確認済
3. **型・名前の一貫性確認**:
   - `InputMode`、`ModeOrigin`、`InputContext`、`InputStep`: M4b-2 / M4b-4 / M4b-5 / M4c-3 / M4c-4 全タスクで統一表記
   - `InputStep::{Preedit, Committed(String), Invalid(char)}`: String 型(`Cow<'static, str>` ではない)、`non_exhaustive`
   - `ConvertStep::Committed(Cow<'static, str>)` → `InputStep::Committed(String)` 変換は M4b-4 Step 1 の `input_char` Hiragana path で `cow.into_owned()` / `format!("{cow}{salvaged}")` で実行
   - `RomajiConverter::normalize_pending(&mut self) -> String`: M4b-3 で追加、M4b-4 Step 1 で呼び出し、シグネチャ一致
   - `allow_transient_to_sticky_promotion: bool`: フィールド型とデフォルト値 (`false`) が M4b-4 Step 1 実装 / M4b-4 Step 1 unit test 2 件 / ADR 0002 decision item 4 / M4c-4 property での未使用扱いで一貫
4. **TDD 遵守**: 各新規モジュール (`mode.rs` / `context.rs`) で「テスト先行 Write → 実装同ファイル内 → cargo test で Green」のサイクルを踏んでいる。fixture (mode_cases.tsv) は data のため TDD ではなく直接書き込み、runner (mode_golden.rs) が本体。property test は proptest 256 反復で検証。
5. **CLAUDE.md 制約**:
   - M4a: 2 files 変更 (ADR 新規 + spec Edit)、約 150 行 → Small tier (docs-only かつ 100〜150 行 Policy 内)
   - M4b: 5 files 変更 (4 新規 + 1 Edit = `input/mod.rs`, `input/mode.rs`, `input/context.rs`, `lib.rs`, `romaji/mod.rs`)、約 700 行 (production 約 400 行 + in-file tests 約 300 行) → Medium tier (production code 400 行は 300 行 guideline をやや超過するが Spec §14 の工数 1.5〜2 日分 と 1 ISSUE 1 PR 原則を優先、tests in-file は除外解釈)
   - M4c: 4 files 新規、約 500 行 (TSV fixture 約 150 行 data + runner / property 約 350 行 code) → Medium tier (data fixture を除外すると code 350 行、guideline は satisfy しないが review 容易性を優先)
   - 1 branch = 1 ISSUE = 1 PR の原則遵守(M4a / M4b / M4c それぞれ独立 ISSUE・独立 PR)
   - 英語 commit / PR / ISSUE / ADR 英語節見出し (project CLAUDE.md の Language 例外に準拠)
6. **Agent Operation Rules 遵守**:
   - 本 plan は main-agent が主導し、各 Task 内のファイル操作は sub-agent に委譲する (superpowers:subagent-driven-development 起動時)
   - lefthook pre-push を machine-enforced gate として利用 (M4a-4 Step 1 / M4b-6 Step 2 / M4c-5 Step 2)
   - sub-agent 報告には verbatim 出力 head/tail を含めるよう明示 (M4b-6 / M4c-5)
   - main-agent の spot-check として PR merge 前に `cargo build && cargo test --workspace` を実行 (M4b-6 / M4c-5)
   - `--no-verify` 使用禁止
7. **修正した箇所**(本 plan 執筆中の自己修正):
   - 初稿で `ConvertStep::Committed(String)` を前提にしていたが、実装 (M3a commit `184ece0`) では `Cow<'static, str>` に変更されていた。M4b-4 Step 1 の `input_char` Hiragana path に `cow.into_owned()` / `format!("{cow}{salvaged}")` の型変換を明示的に追加した
   - 初稿で `RomajiConverter::normalize_pending` を新規追加として計画していたが、実装済の `StateMachine::normalize` が既に `pub(crate)` で存在するため、`normalize_pending` は `self.machine.normalize()` を単に呼ぶだけの thin wrapper であることを明記
   - 初稿で `ConvertStep` の `match` を完全列挙していたが、`#[non_exhaustive]` のため将来互換性上 `_ => InputStep::Preedit` の defensive arm が必要であることを Task M4b-4 Step 1 の実装に追加
   - 初稿の mode_cases.tsv category 6 で Transient → Sticky 昇格を TSV で表現する設計にしていたが、TSV format には toggle 操作の表現がない(input 列は文字列のみ)ため、category 6 は Transient commit の自動復帰 5 件に変更し、promotion の verify は M4c-4 property test と M4b-4 Step 1 unit test 2 件 (`toggle_mode_in_transient_default_flag_false_returns_hiragana` / `toggle_mode_in_transient_with_flag_true_promotes_to_sticky`) に集約
   - 初稿で Karukan 差分 TSV の最終行が曖昧 (`Abc\ndef\n` の `def` の挙動が rule table 依存で pending が複雑)だったため、Task M4c-2 Step 1 の正準 TSV ブロック最終行を `Abc\nka\n` → `Abc\nか\n` に修正した(レビュー指摘 PR #33 を受け、初稿+修正案の 2 段構成を 1 個の正準ブロックに統合)

---

## Reference list

### Spec

- `docs/superpowers/specs/2026-04-22-kotoha-phase-0-design.md`
  - §5 file layout (input/ モジュール位置 = `crates/kotoha-core/src/input/{mod,mode,context}.rs`)
  - §7.1 lib.rs re-export
  - §7.3 `InputMode` API sketch
  - §7.4 `ModeOrigin` 内部用 enum
  - §7.5 `InputContext` API sketch
  - §7.6 `InputStep` enum
  - §8.1 状態タプル定義
  - §8.2 遷移表
  - §8.3 重要なルール 4 項目
  - §8.4 擬似コード
  - §8.5 Karukan 差分 + ADR reference (0001 → 0002 修正対象)
  - §11.1 単体テスト 50 件以上 (input 分 20 件を本 plan で担保)
  - §11.2 Golden テスト mode_cases 70 + Karukan 差分 10
  - §11.3 プロパティテスト 6 条件 (mode 系 4 条件追加を本 plan で担保)

### Prior plans

- `docs/superpowers/plans/2026-04-22-kotoha-phase-0-implementation.md` — Phase 0 全体計画(本 M4 plan が M4 セクションを bite-sized 分割したもの)
- `docs/superpowers/plans/2026-04-22-kotoha-phase-0-m2.md` — M2 bite-sized plan(ヘッダフォーマット参照元)
- `docs/superpowers/plans/2026-04-23-kotoha-phase-0-m3.md` — M3 bite-sized plan(PR 分割 / TSV runner 設計 / property test 構造の参照元)

### Prior ADRs

- `docs/adr/0001-non-ascii-retraction-policy.md` — 非 ASCII 入力の retraction policy (選択肢構造の参照元)
- `docs/adr/0002-input-mode-transient-vs-sticky.md` — 本 plan M4a で新設する Transient/Sticky ADR

### Related ISSUEs

- #32 — M4 tracking parent ISSUE
- #29 / PR #30 — 本 plan M4b-3 `normalize_pending` 追加の契機 (streaming path の mid-stream normalize ギャップ)
- #22 / PR #28 — ADR 0001 を先に 0001 番号で確定させた PR (本 plan M4a の番号 0002 の根拠)
- #23 / PR #27 — convert 終端 buffer normalization hotfix (兄弟的 state machine 改善)
- #26 / PR #31 — proptest strategy broaden (property test strategy 参照元)

### External references

- Karukan (`togatoga/karukan`) — 反面教師としての参照実装 (MIT/Apache-2.0)
- Mozc — Transient 挙動の模範となる OSS IME
