# Kotoha Phase 0 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Kotoha 日本語 IME プロジェクトの Phase 0(Rust workspace + ローマ字→かな変換コア + 入力モード管理 + CLI)を、複数マイルストーンに分割して段階的に実装する。

**Architecture:** Cargo workspace として `kotoha-core`(変換ライブラリ)と `kotoha-cli`(動作確認ツール)の 2 crate を構成する。`kotoha-core` は純粋ドメインロジックのみを扱い、IME フレームワーク統合(IBus/fcitx5)は Phase 3 以降に委ねる。入力モード管理(`InputContext`)を `kotoha-core` に配置することで、Karukan のように統合層にモード遷移をハードコードする設計ミスを回避する。

**Tech Stack:** Rust(最新 stable、MSRV 1.80)、Cargo workspace、thiserror / anyhow / tracing / tracing-subscriber / clap / proptest、lefthook(Git hooks)、gh CLI(GitHub 操作)。

**Spec:** `docs/superpowers/specs/2026-04-22-kotoha-phase-0-design.md` (revision 2)。

---

## マイルストーン概要

Phase 0 全体を 7 つのマイルストーンに分割する。各マイルストーンは独立した ISSUE / branch / PR として扱い、CLAUDE.md の Branch Scope Policy(10 files 以下、300 lines 以下、2 日以下)に収める。

本 plan は **M1 のみ** を bite-sized で詳細化する。M2 着手前に writing-plans スキルを再実行して M2 用の plan を生成する。

| # | マイルストーン | 主な成果物 | 工数 | 本 plan での扱い |
|---|---|---|---|---|
| **M1** | Project setup | Cargo workspace, lefthook, docs/ 骨組み, git init, GitHub repo | 0.7 日 | **詳細化済み(本 plan)** |
| M2 | kotoha-core スケルトン + error + kana | `kotoha-core/src/{lib,error}.rs` + `kana/` | 0.8 日 | 概要のみ |
| M3 | romaji モジュール | `romaji/{mod,rules,trie,state}.rs` + golden test 200 ケース | 3 日 | 概要のみ |
| M4 | input モジュール | `input/{mod,mode,context}.rs` + 単体テスト 20 件 | 1.5〜2 日 | 概要のみ |
| M5 | モードテスト拡張 | `mode_golden.rs` + TSV 70 件 + Karukan 差分 10 件 + property test 4 条件 | 0.8 日 | 概要のみ |
| M6 | kotoha-cli | `romaji.rs`(`--mode` / `--show-mode`)+ phase0-smoke.sh | 0.8 日 | 概要のみ |
| M7 | ADR + ドキュメント | ADR 3 件 + ROADMAP 反映 + WBS ログ | 1 日 | 概要のみ |

---

## M1: Project setup

### M1 のゴール

以下すべてが成立している状態を M1 完了とする:

- GitHub repo が `kotoha-ime` 名で作成され、`main` / `develop` 両 branch が push 済み
- `main` ブランチの default branch は `develop` に変更済み
- Cargo workspace の `Cargo.toml` が存在し、`cargo build --workspace` が空の workspace で PASS する
- `lefthook` が install され、pre-commit で fmt チェックとドキュメントファイル命名規則チェックが動く
- `docs/` 配下に骨組み(`ROADMAP.md` + `adr/0000-template.md` + `wbs/template.md`)が配置されている
- `.github/` に ISSUE / PR テンプレートが配置されている
- `CLAUDE.md`、`LICENSE-MIT`、`LICENSE-APACHE` が配置されている
- M1 の作業 branch `feature/1-project-setup` から M1 の PR が作成され、review を経て develop に merge されている

### M1 で触るファイル

```
kotoha-ime/
├── .gitignore                                   # 新規
├── Cargo.toml                                   # 新規(workspace root)
├── CLAUDE.md                                    # 新規(project 固有規約)
├── LICENSE-MIT                                  # 新規
├── LICENSE-APACHE                               # 新規
├── lefthook.yml                                 # 新規
├── scripts/
│   └── pre-commit-doc-naming.sh                 # 新規
├── docs/
│   ├── ROADMAP.md                               # 新規
│   ├── adr/0000-template.md                     # 新規(template コピー)
│   └── wbs/template.md                          # 新規(template コピー)
├── .github/
│   ├── ISSUE_TEMPLATE.md                        # 新規
│   └── PULL_REQUEST_TEMPLATE.md                 # 新規
└── README.md                                    # 既存(変更なし)
```

以下は M2 以降で作成する(M1 では触らない):

- `crates/kotoha-core/`、`crates/kotoha-cli/`(M2 以降)
- `docs/wiki/glossary.md`(用語が出てきたタイミングで M2 以降に追加)
- `docs/adr/0001-*.md` などの実 ADR(M7)

---

### Task M1-1: Git リポジトリ初期化

**Files:**
- Create: `.gitignore`

- [ ] **Step 1: git init(main 開始ブランチ)**

Run:

```bash
cd /home/kohshiro/develops/student/kotoha-ime
git init -b main
```

Expected: `Initialized empty Git repository in /home/kohshiro/develops/student/kotoha-ime/.git/`

- [ ] **Step 2: .gitignore を作成**

Create `.gitignore`:

```gitignore
# Rust
/target
**/*.rs.bk

# IDE
/.vscode/
/.idea/
*.swp
*.swo

# macOS
.DS_Store

# direnv
.envrc.local

# lefthook
.lefthook-local.yml
```

> **注**: `Cargo.lock` はコミットする(CLI バイナリを含む workspace のため)。そのため ignore に入れない。

- [ ] **Step 3: 既存の README.md / docs/ と .gitignore を main に初期コミット**

Run:

```bash
git add .gitignore README.md docs/
git commit -m "chore: initial commit with design spec"
```

Expected: `3 files changed`(.gitignore、README.md、docs/superpowers/specs/2026-04-22-kotoha-phase-0-design.md)。

> **Commit message 言語**: 本プロジェクトでは global CLAUDE.md の「commit は日本語」ルールから例外として **英語** を採用する。理由は Task M1-7(CLAUDE.md 配置)で project CLAUDE.md に明記する。

- [ ] **Step 4: develop branch 作成**

Run:

```bash
git branch develop
git branch
```

Expected: `git branch` の出力に `* main` と `develop` が表示される。

- [ ] **Step 5: feature/1-project-setup branch を develop から切って checkout**

Run:

```bash
git checkout -b feature/1-project-setup develop
```

Expected: `Switched to a new branch 'feature/1-project-setup'`。以後の作業はこのブランチで行う。

---

### Task M1-2: GitHub repo 作成と初期 push

**Files:** (ローカル変更なし、GitHub 上の操作)

- [ ] **Step 1: GitHub repo を作成して current branch を push**

Run:

```bash
gh repo create kotoha-ime --source=. --private --push --description "Kotoha: a Japanese IME written in Rust"
```

Expected: Repo が GitHub 上に作成され、current branch(feature/1-project-setup)が push される。リモート `origin` が設定される。

> **--private / --public**: 本 plan では `--private` で開始し、Phase 0 完了後に public 化を推奨。

- [ ] **Step 2: main / develop branch もリモートに push**

Run:

```bash
git push -u origin main develop
```

Expected: 両 branch が push され、tracking 設定される。

- [ ] **Step 3: default branch を develop に変更**

Run:

```bash
gh repo edit --default-branch develop
```

Expected: エラーなく終了。GitHub UI の repo 設定で default branch が `develop` になっていることを確認。

- [ ] **Step 4: main branch の保護ルール設定(任意、推奨)**

Run:

```bash
gh api --method PUT "repos/$(gh repo view --json nameWithOwner -q .nameWithOwner)/branches/main/protection" \
  --field required_pull_request_reviews='{"required_approving_review_count":0}' \
  --field enforce_admins=false \
  --field restrictions=null \
  --field required_status_checks=null
```

Expected: main branch が保護される(直接 push 禁止)。失敗した場合は skip してよい(後で GitHub UI から設定可能)。

---

### Task M1-3: M1 用 ISSUE 作成

**Files:** (ローカル変更なし、GitHub 上の操作)

- [ ] **Step 1: ISSUE #1 を作成**

Run:

```bash
gh issue create \
  --title "M1: Project setup (Cargo workspace, lefthook, docs scaffold, licenses)" \
  --body "Phase 0 Milestone 1: Project foundation setup. See docs/superpowers/plans/2026-04-22-kotoha-phase-0-implementation.md for detailed tasks."
```

Expected: ISSUE #1 が作成され、URL が表示される。

> **Title / body 言語**: 英語(project CLAUDE.md の例外規定による)。

- [ ] **Step 2: ISSUE 番号を確認し、branch 名との整合性を確認**

Run:

```bash
gh issue list --limit 5
git branch --show-current
```

Expected: ISSUE #1 と branch `feature/1-project-setup` が一致する(`1-` で始まる)。ISSUE 番号が 1 でない場合は branch を rename する:

```bash
# ISSUE 番号が N だった場合
git branch -m feature/N-project-setup
```

---

### Task M1-4: Cargo workspace 初期化

**Files:**
- Create: `Cargo.toml`

- [ ] **Step 1: Cargo.toml(workspace root)を作成**

Create `Cargo.toml`:

```toml
[workspace]
resolver = "2"
members = [
    # crates/kotoha-core は M2 で追加
    # crates/kotoha-cli は M6 で追加
]

[workspace.package]
edition = "2021"
rust-version = "1.80"
version = "0.1.0"
license = "MIT OR Apache-2.0"
repository = "https://github.com/std-koh-hinooka/kotoha-ime"
authors = ["std-koh-hinooka <koh.hinooka@student.it.com>"]

[workspace.dependencies]
# 共通 dependencies の version pinning は M2 以降で追加する。
# 本 M1 時点では workspace members がまだ存在しないので空とする。
```

> **埋め込み値の補足**: `repository` / `authors` の値は plan 作成時に確定済み(`std-koh-hinooka` / `koh.hinooka@student.it.com`)。別の著者名・メールアドレスに変更したい場合は、Cargo.toml 作成時にこの 2 箇所を差し替える。`git config user.name` / `git config user.email` が plan の値と異なる場合も、commit 署名は git config 側が優先される(Cargo.toml の `authors` は crates.io / docs 表示用の独立した値)。

- [ ] **Step 2: cargo build で空 workspace が通ることを確認**

Run:

```bash
cargo build --workspace
```

Expected: `Finished dev profile [unoptimized + debuginfo] target(s) in ...`。警告は `virtual workspace defaulting to resolver ...` が出る場合があるが OK。

- [ ] **Step 3: commit**

Run:

```bash
git add Cargo.toml
git commit -m "chore: scaffold cargo workspace"
```

Expected: `1 file changed`。

---

### Task M1-5: docs/ 骨組み配置

**Files:**
- Create: `docs/ROADMAP.md`
- Create: `docs/adr/0000-template.md`(~/.claude/templates からコピー)
- Create: `docs/wbs/template.md`(~/.claude/templates からコピー)

- [ ] **Step 1: docs/adr/0000-template.md をコピー**

Run:

```bash
mkdir -p docs/adr docs/wbs
cp ~/.claude/templates/docs/adr/0000-template.md docs/adr/0000-template.md
```

Expected: `docs/adr/0000-template.md` が存在する。

- [ ] **Step 2: docs/wbs/template.md をコピー**

Run:

```bash
cp ~/.claude/templates/docs/wbs/template.md docs/wbs/template.md
```

Expected: `docs/wbs/template.md` が存在する。

- [ ] **Step 3: docs/ROADMAP.md を作成**

Create `docs/ROADMAP.md`:

```markdown
# Kotoha Roadmap

Kotoha プロジェクトの開発フェーズと、各フェーズの到達目標を記録する。

## Phase 一覧

| Phase | 名称 | 内容 | 状態 |
|---|---|---|---|
| 0 | Foundation | Cargo workspace + ローマ字→かな変換 + 入力モード管理 + CLI | 実装中 |
| 1 | Kana→Kanji conversion | Zenz + llama.cpp によるかな→漢字変換 | 未着手 |
| 2 | Dictionary and learning | システム辞書 + ユーザ辞書 + 学習キャッシュ | 未着手 |
| 3 | IBus integration | IBus engine(GNOME Mutter 用) | 未着手 |
| 4 | fcitx5 integration | fcitx5 addon(KDE / wlroots 用) | 未着手 |
| 5 | Advanced features | タイポ訂正 + 文脈リランキング | 未着手 |
| 6 | UX polish | 設定 UI + 辞書自動更新 + 同期 | 未着手 |

## Phase 0 マイルストーン

Phase 0 の詳細マイルストーン分割は `docs/superpowers/plans/2026-04-22-kotoha-phase-0-implementation.md` を参照。

## 注記

- Phase 5 の「Shift 挙動設定」は、設計書 revision 2 の判断により Phase 0 に前倒しされた。Phase 5 は「タイポ訂正 + 文脈リランキング」のみ。
- 各 Phase の設計書は `docs/superpowers/specs/` に配置する。
```

- [ ] **Step 4: commit**

Run:

```bash
git add docs/ROADMAP.md docs/adr/0000-template.md docs/wbs/template.md
git commit -m "docs: add ROADMAP and ADR/WBS templates"
```

Expected: `3 files changed`。

---

### Task M1-6: .github/ テンプレート配置

**Files:**
- Create: `.github/ISSUE_TEMPLATE.md`(~/.claude/templates からコピー)
- Create: `.github/PULL_REQUEST_TEMPLATE.md`(~/.claude/templates からコピー)

- [ ] **Step 1: ISSUE / PR テンプレートをコピー**

Run:

```bash
mkdir -p .github
cp ~/.claude/templates/github/ISSUE_TEMPLATE.md .github/ISSUE_TEMPLATE.md
cp ~/.claude/templates/github/PULL_REQUEST_TEMPLATE.md .github/PULL_REQUEST_TEMPLATE.md
```

Expected: 2 files copied。

- [ ] **Step 2: テンプレート内のプレースホルダを実値に置換**

テンプレート内容を Read で確認し、`<OWNER>` / `<REPO>` 等のプレースホルダがあれば Edit ツールで実値に置き換える(repo 名は `kotoha-ime`、OWNER は Task M1-4 と同じ値)。

- [ ] **Step 3: commit**

Run:

```bash
git add .github/
git commit -m "chore: add GitHub issue and PR templates"
```

Expected: `2 files changed`。

---

### Task M1-7: CLAUDE.md(project-specific)配置

**Files:**
- Create: `CLAUDE.md`

- [ ] **Step 1: CLAUDE.md を新規作成(project 固有の規約のみ記載)**

Create `CLAUDE.md`:

```markdown
# Kotoha — Project-Specific Claude Instructions

本文書は global `~/.claude/CLAUDE.md` の規約を前提とし、Kotoha プロジェクト固有の例外・追加規約のみを記載する。

## プロジェクト概要

- GNOME Wayland ネイティブに動作する自作日本語 IME
- Rust で実装、Cargo workspace 構成
- 設計書: `docs/superpowers/specs/`
- 実装計画: `docs/superpowers/plans/`

## Language 例外

global CLAUDE.md は「commit message / PR / ISSUE は日本語」だが、**Kotoha プロジェクトは例外として英語を使用** する。

理由: 将来 OSS として公開する想定であり、外部貢献者との互換性を優先するため。

英語で記述する対象:

- commit message
- PR タイトル / body
- GitHub ISSUE タイトル / body
- GitHub ラベル名

日本語を維持する対象:

- 設計書 (`docs/superpowers/specs/`)
- 実装計画 (`docs/superpowers/plans/`)
- ADR (`docs/adr/`)
- WBS (`docs/wbs/`)
- コード内コメント(ドメイン由来のみ。API doc は英語でもよい)
- Claude Code との対話

## Rust 開発規約

- edition: 2021
- rust-version: 1.80(`Cargo.toml` で pinned)
- フォーマッタ: `cargo fmt --all`
- リンタ: `cargo clippy --workspace --all-targets -- -D warnings`(警告ゼロを必須)

## 依存管理

- workspace root の `[workspace.dependencies]` で全 crate の依存 version を pin する
- 個別 crate は `{ workspace = true }` で参照する
- 新規依存追加時は、目的と代替案の比較を commit message または PR body に記載する

## テスト規約

- 単体テスト: `#[cfg(test)]` で同ファイル内
- 統合テスト: `crates/*/tests/` 配下
- golden テスト: `crates/*/tests/fixtures/*.tsv` を `tests/*_golden.rs` から読み込む
- property test: `proptest` を使用

## Phase 状態の参照

現在の Phase、完了条件、マイルストーン分割は以下を参照:

- `docs/ROADMAP.md` — Phase 全体像
- `docs/superpowers/specs/` — 各 Phase の設計書
- `docs/superpowers/plans/` — マイルストーン単位の実装計画
- `docs/wbs/` — 実装ログ

## WBS 直接 push の例外

WBS ログ(`docs/wbs/*.md`)は、対象 PR の merge 後に develop へ直接 push して OK とする。
理由: 実装内容に影響しない純粋なメタデータ記録であり、PR レビューの対象ではないため。
```

- [ ] **Step 2: commit**

Run:

```bash
git add CLAUDE.md
git commit -m "docs: add project-specific CLAUDE.md (English exception)"
```

Expected: `1 file changed`。

---

### Task M1-8: ライセンスファイル配置

**Files:**
- Create: `LICENSE-MIT`
- Create: `LICENSE-APACHE`

- [ ] **Step 1: LICENSE-MIT 作成**

Create `LICENSE-MIT`:

```
MIT License

Copyright (c) 2026 std-koh-hinooka

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

> **置換**: `std-koh-hinooka` を実値に置換。年 2026 は着手年に合わせる(2026 で OK)。

- [ ] **Step 2: LICENSE-APACHE 作成(公式全文をダウンロード)**

Run:

```bash
curl -fsSL https://www.apache.org/licenses/LICENSE-2.0.txt -o LICENSE-APACHE
```

Expected: `LICENSE-APACHE` がダウンロードされる。`head -3 LICENSE-APACHE` の先頭が以下で始まること:

```
                                 Apache License
                           Version 2.0, January 2004
                        http://www.apache.org/licenses/
```

- [ ] **Step 3: commit**

Run:

```bash
git add LICENSE-MIT LICENSE-APACHE
git commit -m "chore: add dual MIT/Apache-2.0 license files"
```

Expected: `2 files changed`。

---

### Task M1-9: lefthook 設定

**Files:**
- Create: `lefthook.yml`
- Create: `scripts/pre-commit-doc-naming.sh`

- [ ] **Step 1: lefthook.yml を作成**

Create `lefthook.yml`:

```yaml
# Kotoha lefthook configuration
# Origin: ~/.claude/templates/lefthook.yml.template

pre-commit:
  parallel: true
  commands:
    fmt-check:
      glob: "*.rs"
      run: cargo fmt --all --check
    doc-naming:
      run: ./scripts/pre-commit-doc-naming.sh

pre-push:
  parallel: false
  commands:
    build:
      run: cargo build --workspace
    clippy:
      run: cargo clippy --workspace --all-targets -- -D warnings
    test:
      run: cargo test --workspace
```

- [ ] **Step 2: scripts/pre-commit-doc-naming.sh をテンプレートからコピー**

Run:

```bash
mkdir -p scripts
cp ~/.claude/templates/scripts/pre-commit-doc-naming.sh scripts/pre-commit-doc-naming.sh
chmod +x scripts/pre-commit-doc-naming.sh
```

Expected: `ls -l scripts/pre-commit-doc-naming.sh` で実行権限(`x`)が確認できる。

- [ ] **Step 3: lefthook install**

Run:

```bash
lefthook install
```

Expected: `sync hooks: ✔️` が表示される。`.git/hooks/pre-commit` が作成される。

- [ ] **Step 4: fmt-check 動作確認(意図的違反で commit を試みる)**

Run:

```bash
cat > bad_format.rs <<'EOS'
fn main(){println!("bad format")}
EOS
git add bad_format.rs
git commit -m "test: lefthook fmt-check should block this"
```

Expected: commit がブロックされる(fmt-check が失敗し、pre-commit hook が exit non-zero)。

Cleanup:

```bash
git reset HEAD bad_format.rs
rm bad_format.rs
```

> **注**: workspace に Rust ファイルがまだ 0 件の場合、`cargo fmt --check` は対象なしで PASS する可能性がある。その場合はこの Step の確認を M2(kotoha-core 配置後)に持ち越す。

- [ ] **Step 5: doc-naming 動作確認(意図的違反で commit を試みる)**

Run:

```bash
touch docs/wbs/bad-name.md
git add docs/wbs/bad-name.md
git commit -m "test: lefthook doc-naming should block this"
```

Expected: commit がブロックされる(doc-naming スクリプトが違反を検出)。

Cleanup:

```bash
git reset HEAD docs/wbs/bad-name.md
rm docs/wbs/bad-name.md
```

- [ ] **Step 6: commit**

Run:

```bash
git add lefthook.yml scripts/pre-commit-doc-naming.sh
git commit -m "chore: add lefthook config and doc naming check"
```

Expected: `2 files changed`。

---

### Task M1-10: M1 総合チェックと PR 作成・merge

**Files:** (変更なし、検証と PR 作成・merge 操作のみ)

- [ ] **Step 1: 全 commit 内容を確認**

Run:

```bash
git log --oneline develop..HEAD
```

Expected: Task M1-1 から M1-9 までの commit(6 コミット前後)が順に並ぶ。

- [ ] **Step 2: diff 統計を確認(ブランチ規約:10 ファイル以下 / 300 行以下)**

Run:

```bash
git diff develop..HEAD --stat
```

Expected: 変更ファイル数が 10 程度、変更行数合計が 300 を超えている可能性がある(`LICENSE-APACHE` が約 200 行あるため)。

> **Branch Scope 例外**: LICENSE-APACHE は機械生成の定型文書であり、レビュー負荷の実質は小さい。行数超過分を除くと実質 100 行以下。PR 本文に「LICENSE ファイル 200 行を除く実質変更 XXX 行」と明記することで M1 内完結を許容する。

- [ ] **Step 3: cargo build 最終確認**

Run:

```bash
cargo build --workspace
```

Expected: `Finished` が表示される。

- [ ] **Step 4: lefthook pre-push を手動実行して検証**

Run:

```bash
lefthook run pre-push
```

Expected: build / clippy / test のすべてが PASS。workspace が空なので test は 0 件で PASS。

- [ ] **Step 5: push**

Run:

```bash
git push -u origin feature/1-project-setup
```

Expected: branch が push される。pre-push hook が自動実行され、再度 build / clippy / test が PASS する。push 完了後に PR 作成 URL が表示される。

- [ ] **Step 6: PR 作成**

Run:

```bash
gh pr create --base develop --head feature/1-project-setup \
  --title "M1: Project setup (Cargo workspace, lefthook, docs scaffold, licenses)" \
  --body "$(cat <<'EOS'
## Summary

Phase 0 Milestone 1: Project foundation setup.

- Cargo workspace (empty, members added in M2+)
- lefthook config: pre-commit (fmt + doc-naming), pre-push (build + clippy + test)
- docs/ scaffold (ROADMAP, ADR/WBS templates)
- Project-specific CLAUDE.md (English commit messages as exception)
- Dual MIT/Apache-2.0 licenses
- GitHub issue/PR templates

## Related

- Spec: docs/superpowers/specs/2026-04-22-kotoha-phase-0-design.md §5.2
- Plan: docs/superpowers/plans/2026-04-22-kotoha-phase-0-implementation.md M1
- Closes #1

## Notes on branch size

LICENSE-APACHE (~200 lines, machine-generated) pushes the diff past the 300-line guideline. Actual reviewable change is ~100 lines.

## Test plan

- [ ] cargo build --workspace passes (empty workspace)
- [ ] lefthook run pre-commit passes
- [ ] lefthook run pre-push passes
- [ ] doc-naming script blocks badly named doc files
- [ ] ISSUE / PR templates render correctly on GitHub
EOS
)"
```

Expected: PR 作成成功、URL 表示。

- [ ] **Step 7: PR レビュー(Small tier、CLAUDE.md の PR Review Matrix に従う)**

M1 の PR は規約上 Small tier(≤5 files AND ≤100 lines)を超えるが、LICENSE を除く実質変更が Small 相当のため Small tier で review する。

Claude Code 側で以下のスラッシュコマンドを並列起動する:

```
/agent-teams:team-review dimensions=security,architecture,testing
/secrets-check
```

Expected: review findings を取得。High / Critical はこの PR 内で解決、Low は別 ISSUE に切り出す。

- [ ] **Step 8: review findings を解消 → 再 review → 問題ゼロになるまで繰り返す**

残 finding がゼロになるまで修正 commit と再 review を繰り返す。

Expected: 全 finding 解消、approved 状態。

- [ ] **Step 9: PR を develop に squash merge して branch を削除**

Run:

```bash
gh pr merge --squash --delete-branch
```

Expected: PR が develop に squash merge され、feature/1-project-setup branch が削除される。

- [ ] **Step 10: WBS ログを develop に直接 push**

Run:

```bash
git checkout develop
git pull
```

Create `docs/wbs/2026-04-XX-feature-1-project-setup.md`(日付 XX は実施日、PR 番号 Y は Step 9 で merge された PR 番号):

```markdown
---
milestone: M1
branch: feature/1-project-setup
pr: "#Y"
issue: "#1"
status: done
started: 2026-04-XX
finished: 2026-04-XX
---

# M1: Project setup

## 実施内容

- Cargo workspace の骨格配置(空 members)
- lefthook 設定(pre-commit: fmt + doc-naming、pre-push: build + clippy + test)
- docs/ 骨組み(ROADMAP、ADR/WBS template)
- project-specific CLAUDE.md(commit message 英語例外)
- LICENSE-MIT / LICENSE-APACHE
- .github/ ISSUE/PR テンプレート
- git init + develop branch 作成 + GitHub repo 作成

## つまずき

(実施時に記入。例: "lefthook pre-commit の fmt-check が空 workspace 対象で PASS になり、検証が M2 に持ち越しになった" 等)

## 次マイルストーンへの申し送り

- Cargo workspace の `[workspace.dependencies]` は M2 で `thiserror`、`anyhow`、`tracing`、`tracing-subscriber` を追加
- lefthook の fmt-check は M2 で kotoha-core が追加されて初めて意味のある検証が走る
- ADR 0001 / 0002 / 0003 は M7 で作成

## 成果物リンク

- PR: #Y
- ISSUE: #1
- Spec: `docs/superpowers/specs/2026-04-22-kotoha-phase-0-design.md`
- Plan: `docs/superpowers/plans/2026-04-22-kotoha-phase-0-implementation.md` (M1 section)
```

Run:

```bash
git add docs/wbs/2026-04-XX-feature-1-project-setup.md
git commit -m "docs: M1 implementation log"
git push
```

> **例外**: WBS ログは project CLAUDE.md で定義した例外により、develop への直接 push を許容する。

---

### M1 完了条件チェックリスト

以下すべてが ✅ になったら M2 に進む:

- [ ] GitHub repo が存在し、main / develop が push 済み、default branch が develop
- [ ] ISSUE #1 が close されている
- [ ] PR が develop に merge され、feature branch が削除されている
- [ ] `cargo build --workspace` が PASS する
- [ ] `lefthook install` 済み、pre-commit / pre-push が動作する
- [ ] `docs/ROADMAP.md`、`docs/adr/0000-template.md`、`docs/wbs/template.md` が存在する
- [ ] `.github/ISSUE_TEMPLATE.md`、`.github/PULL_REQUEST_TEMPLATE.md` が存在する
- [ ] `CLAUDE.md`(project-specific)が存在する
- [ ] `LICENSE-MIT`、`LICENSE-APACHE` が存在する
- [ ] `lefthook.yml`、`scripts/pre-commit-doc-naming.sh` が存在する
- [ ] WBS ログ `docs/wbs/2026-04-XX-feature-1-project-setup.md` が存在する

---

## M2〜M7: 高レベル概要

以下は各マイルストーンのゴール・触るファイル・受入条件の概要。bite-sized タスク化は、各マイルストーン着手前に writing-plans スキルを再実行して作成する。

### M2: kotoha-core skeleton + error + kana

**Goal:** `kotoha-core` crate を新規作成し、公開 API の骨格(`lib.rs`)、`Error` / `Result` 型、`kana` ユーティリティを実装する。

**Files to create:**

- `crates/kotoha-core/Cargo.toml`
- `crates/kotoha-core/src/lib.rs`
- `crates/kotoha-core/src/error.rs`
- `crates/kotoha-core/src/kana/mod.rs`
- `crates/kotoha-core/src/kana/hiragana.rs`
- `crates/kotoha-core/src/kana/katakana.rs`

**Files to modify:**

- `Cargo.toml`(workspace の `members` に `crates/kotoha-core` を追加、`[workspace.dependencies]` に `thiserror = "1"`、`anyhow = "1"`、`tracing = "0.1"`、`tracing-subscriber = "0.3"` を追加)

**Acceptance:**

- `cargo build -p kotoha-core` PASS
- `cargo test -p kotoha-core` で kana 単体テスト 10 件以上が PASS
- `is_hiragana` / `is_katakana` / `hiragana_to_katakana` / `katakana_to_hiragana` が公開 API として利用可能
- `Error` / `Result` が `#[non_exhaustive]` 付きで定義されている

**Reference:** Spec §7.1, §7.7, §7.8

---

### M3: romaji module

**Goal:** ローマ字→かな変換のコアロジックを実装する。trie データ構造、state machine、ルール表を持ち、`RomajiConverter` の公開 API を提供する。Karukan 互換の挙動(Shift 挙動を除く)を golden test 200 ケース以上で検証する。

**Files to create:**

- `crates/kotoha-core/src/romaji/mod.rs`
- `crates/kotoha-core/src/romaji/rules.rs`(ローマ字変換ルール表)
- `crates/kotoha-core/src/romaji/trie.rs`
- `crates/kotoha-core/src/romaji/state.rs`
- `crates/kotoha-core/tests/romaji_golden.rs`
- `crates/kotoha-core/tests/fixtures/romaji_cases.tsv`

**Files to modify:**

- `crates/kotoha-core/src/lib.rs`(`pub mod romaji` + `pub use romaji::{RomajiConverter, ConvertStep}` を追加)
- `Cargo.toml`(workspace dev-dependencies に `proptest = "1.5"` 追加)

**Acceptance:**

- `cargo test -p kotoha-core` で romaji 単体テスト 20 件 + golden 200 件以上が PASS
- property test 3 条件(冪等性 / 結合性 / 可逆性)が PASS
- API 直接テストで `RomajiConverter::convert("konnichiwa")` が `("こんにちは", "")` を返す

**Reference:** Spec §7.2, §9, §11.1, §11.2, §11.3

**Work breakdown estimate:** 3 日(rule 表移植に約 1 日、trie + state に約 1 日、golden + property test に約 1 日)

---

### M4: input module

**Goal:** 入力モード管理(`InputContext`)を実装する。`(Hiragana, Sticky)` / `(Direct, Transient)` / `(Direct, Sticky)` の 3 状態機械を Mozc 式の Shift 挙動(Enter で Transient 自動復帰)で実装する。

**Files to create:**

- `crates/kotoha-core/src/input/mod.rs`
- `crates/kotoha-core/src/input/mode.rs`
- `crates/kotoha-core/src/input/context.rs`

**Files to modify:**

- `crates/kotoha-core/src/lib.rs`(`pub mod input` + `pub use input::{InputContext, InputMode, InputStep}` を追加)

**Acceptance:**

- `cargo test -p kotoha-core input::` で `InputContext` 単体テスト 20 件以上が PASS
- Spec §8.2 遷移表の全遷移が unit test でカバー
- `allow_transient_to_sticky_promotion: true` 固定で動作(Phase 3 で設定化を想定した field 定義)

**Reference:** Spec §7.3, §7.4, §7.5, §7.6, §8

**Work breakdown estimate:** 1.5〜2 日

---

### M5: mode tests (golden + Karukan diff + property)

**Goal:** `InputContext` の動作を golden test と property test で網羅検証する。

**Files to create:**

- `crates/kotoha-core/tests/mode_golden.rs`
- `crates/kotoha-core/tests/fixtures/mode_cases.tsv`(70 ケース以上)
- `crates/kotoha-core/tests/fixtures/mode_cases_karukan_diff.tsv`(10 ケース以上)

**Files to modify:**

- 必要なら `crates/kotoha-core/tests/romaji_golden.rs` の共通 TSV パーサを抽出して `tests/common/` 配下に配置

**Acceptance:**

- `cargo test -p kotoha-core --test mode_golden` で 70 ケース以上 + Karukan 差分 10 ケース以上が PASS
- property test 4 条件(Hiragana/Sticky 安定性、Transient 必ず復帰、Sticky Direct 持続、reset 冪等性)が PASS

**Reference:** Spec §11.2, §11.3

---

### M6: kotoha-cli (kotoha-romaji command)

**Goal:** 動作確認用 CLI を実装する。stdin 行単位で読み込み、モード切替(`--mode`、`--show-mode`)をサポートする。

**Files to create:**

- `crates/kotoha-cli/Cargo.toml`
- `crates/kotoha-cli/src/bin/romaji.rs`
- `scripts/phase0-smoke.sh`(Spec §13.2 の CLI 手動確認コマンド一括実行)

**Files to modify:**

- `Cargo.toml`(workspace の `members` に `crates/kotoha-cli` を追加、`[workspace.dependencies]` に `clap = { version = "4.5", features = ["derive"] }` 追加)

**Acceptance:**

- `cargo build -p kotoha-cli` PASS、バイナリ `kotoha-romaji` が生成される
- Spec §13.2 の CLI 手動確認コマンド 10 件が全て期待出力を返す
- `scripts/phase0-smoke.sh` が exit 0 で終了する

**Reference:** Spec §10, §13.2

---

### M7: ADRs + finishing docs

**Goal:** Phase 0 で確立した設計判断を ADR として記録し、ROADMAP / README を最終化する。

**Files to create:**

- `docs/adr/0001-input-mode-transient-vs-sticky.md`
- `docs/adr/0002-shift-via-uppercase-char.md`
- `docs/adr/0003-cli-line-based-commit.md`
- `docs/wbs/2026-04-XX-feature-N-phase0-adrs.md`(N は対応 ISSUE 番号)

**Files to modify:**

- `docs/ROADMAP.md`(Phase 0 を "完了" に更新、Phase 1 への申し送りを追加)
- `README.md`(Phase 状態を更新、`kotoha-romaji` の使用例を 5 件以上追記)

**Acceptance:**

- ADR 3 件が `docs/adr/` に存在する
- 各 ADR に "Context" / "Decision" / "Consequences" が記述されている
- README.md に `kotoha-romaji` の使用例が 5 件以上記載されている

**Reference:** Spec §13.3, §19 変更履歴

---

## Spec Coverage 確認

Spec(revision 2)の主要要件と本 plan でのカバー位置:

| Spec 要件 | Plan のカバー位置 |
|---|---|
| §3.1 Cargo workspace | M1, M2 |
| §3.2 kotoha-core ローマ字→かな変換 | M3 |
| §3.3 kotoha-core 入力モード管理 | M4 |
| §3.4 kotoha-cli `kotoha-romaji` | M6 |
| §3.5 Karukan 差分 golden test | M5 |
| §3.6 lefthook 品質ゲート | M1 |
| §5.2 ディレクトリ構造(全体) | M1(骨組み)→ M2-M6(crates 充実) |
| §7.1 公開 API ルート | M2 |
| §7.2 RomajiConverter | M3 |
| §7.3-7.6 Input モジュール API | M4 |
| §7.7 Kana ユーティリティ | M2 |
| §7.8 Error 型 | M2 |
| §8 モード管理仕様 | M4 実装 / M5 テスト |
| §9 変換ルール | M3 |
| §10 CLI 仕様 | M6 |
| §11.1 単体テスト(50 件) | M2(10 件)+ M3(20 件)+ M4(20 件) |
| §11.2 Golden テスト(270 件 + Karukan 差分) | M3(200 件)+ M5(70 件 + 差分 10 件) |
| §11.3 プロパティテスト(7 条件) | M3(3 条件)+ M5(4 条件) |
| §11.4 実行時間見積もり | M5 完了時に計測、WBS に記録 |
| §12 品質ゲート | M1 |
| §13.1 ビルド・テスト系成功基準 | M1-M6 各マイルストーンで確認 |
| §13.2 CLI 手動確認成功基準 | M6 の phase0-smoke.sh |
| §13.3 ドキュメント成果物 | M7 |
| §13.4 テスト成果物 | M3 + M5 |
| §14 工数目安 | 本 plan マイルストーン工数と整合 |
| §15 リスク #1-4(既存) | M3 / 各 PR review / CLAUDE.md |
| §15 リスク #5 状態遷移漏れ | M4 / M5 |
| §15 リスク #6 Phase 3 API ミスマッチ | Spec §17 で先出済み、Phase 3 着手前にレビュー |
| §15 リスク #7 Mozc 差分 | M5 mode_cases_karukan_diff、Phase 1 以降で Mozc 比較 |
| §15 リスク #8 昇格ルール不評 | M4 で `allow_transient_to_sticky_promotion` field 配置 |
| §16 Phase 1 への橋渡し | M7 ROADMAP 更新で明記 |
| §17 付録 Phase 3 想定インタフェース | Spec 内付録(本 plan では参照のみ) |
| §18 参考 | M7 README 更新時に反映 |
| §19 変更履歴 | Spec に記録済み、本 plan では触れない |

---

## Self-Review 済み事項

本 plan 書き起こし後のセルフレビューで確認した項目:

1. **プレースホルダスキャン**: GitHub 関連(アカウント名 `std-koh-hinooka`、公開メール `koh.hinooka@student.it.com`)は plan 更新時に実値へ置換済み。残プレースホルダは `2026-04-XX`(実施日)、`#Y`(PR 番号)、`#N`(関連 ISSUE 番号)のみで、着手時に確定する。
2. **型・API 一貫性**: Spec §7 の公開 API シグネチャと M2-M4 の概要で定義する型名は一致(`InputMode` / `InputStep` / `RomajiConverter` / `ConvertStep` / `Error` / `Result`)。
3. **Spec カバレッジ**: 上表のとおり、Spec の全章節をいずれかのマイルストーンに割り当て済み。
4. **CLAUDE.md 制約**: 各マイルストーンは Branch Scope Policy(10 files / 300 lines / 2 日)に収まる。M1 のみ LICENSE ファイルで行数を超えるが、PR 本文で明示する例外扱い。
