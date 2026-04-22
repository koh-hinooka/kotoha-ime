# Kotoha Phase 0 — M2 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `kotoha-core` crate を workspace に初めて追加し、公開 API のルート(`lib.rs`)、`Error` / `Result` 型(`thiserror` ベース)、`kana` ユーティリティ(`is_hiragana` / `is_katakana` / `hiragana_to_katakana` / `katakana_to_hiragana` の 4 関数)を TDD で実装する。

**Architecture:** `kana` は Unicode コードポイント範囲(ひらがな `U+3041..=U+3096` + `U+309D..=U+309F`、カタカナ `U+30A1..=U+30FA` + `U+30FC` + `U+30FD..=U+30FF`)で判定し、変換はひらがな⇄カタカナ間の `0x60` オフセットを加減する。`Error` は `#[non_exhaustive]` な enum として将来の拡張に備え、現時点では `InvalidCharacter(char)` と `InvalidState` の 2 バリアントのみを持つ。

**Tech Stack:** Rust 2021 / MSRV 1.80、`thiserror ~= 1`、`tracing ~= 0.1`、`anyhow ~= 1`(CLI 層のみ、M2 では未使用)、`tracing-subscriber ~= 0.3`(CLI 層のみ、M2 では未使用)。

**Spec:** `docs/superpowers/specs/2026-04-22-kotoha-phase-0-design.md` §7.1(公開 API)、§7.7(kana)、§7.8(Error)、§11.1(単体テスト 10 件以上)。

**Phase 0 全体計画:** `docs/superpowers/plans/2026-04-22-kotoha-phase-0-implementation.md`(M1 完了、本 M2 が次のマイルストーン)。

---

## マイルストーン位置付け

| 観点 | 内容 |
|---|---|
| Phase | 0 / Foundation |
| マイルストーン | M2 of 7 |
| 前提 | M1 完了(develop は `01a7c1d chore: harden lefthook guards and expand .gitignore (#7)` が最新) |
| 後続 | M3(`romaji` モジュール実装)、M4(`input` モジュール実装) |
| 工数見積 | 0.8 日(暦上では週末の半日分) |

## ファイル構成

### 新規作成(6 ファイル)

```
crates/
└── kotoha-core/
    ├── Cargo.toml                          # crate 定義、workspace 継承
    └── src/
        ├── lib.rs                          # 公開 API ルート (pub mod + pub use)
        ├── error.rs                        # Error / Result
        └── kana/
            ├── mod.rs                      # 公開関数、内部 pub(crate) 利用
            ├── hiragana.rs                 # is_hiragana、hiragana_to_katakana
            └── katakana.rs                 # is_katakana、katakana_to_hiragana
```

### 変更(1 ファイル)

- `Cargo.toml`(root workspace):
  - `members` に `"crates/kotoha-core"` を追加
  - `[workspace.dependencies]` に `thiserror`、`anyhow`、`tracing`、`tracing-subscriber` を追加

各ファイルの責務は以下のとおり:

| ファイル | 責務 |
|---|---|
| `crates/kotoha-core/Cargo.toml` | crate メタデータ、workspace メンバーとしての宣言、`thiserror` と `tracing` への依存 |
| `crates/kotoha-core/src/lib.rs` | 公開 API の宣言(`pub mod error;` `pub mod kana;` + `pub use`)。M3 で `romaji`、M4 で `input` が追加される場所 |
| `crates/kotoha-core/src/error.rs` | `Error` enum(`#[non_exhaustive]`)、`type Result<T>` 別名 |
| `crates/kotoha-core/src/kana/mod.rs` | `kana` モジュールの公開エントリポイント、下位 `hiragana` / `katakana` モジュールを `pub use` で再エクスポート |
| `crates/kotoha-core/src/kana/hiragana.rs` | `is_hiragana(ch: char) -> bool`、`hiragana_to_katakana(s: &str) -> String` |
| `crates/kotoha-core/src/kana/katakana.rs` | `is_katakana(ch: char) -> bool`、`katakana_to_hiragana(s: &str) -> String` |

## M2 完了条件

以下がすべて成立したら M2 完了:

- [ ] GitHub ISSUE (M2) が作成され、merge 済みの PR で close されている
- [ ] `cargo build --workspace` が PASS(M1 で deferred していた workspace-wide build がここで初めて通る)
- [ ] `cargo build -p kotoha-core` が PASS
- [ ] `cargo test -p kotoha-core` が PASS(kana 単体テスト 10 件以上 + error 単体テスト 2 件以上)
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` が warnings ゼロ
- [ ] `cargo fmt --all --check` が PASS
- [ ] lefthook pre-push が全コマンド PASS(manifest-check + build + clippy + test が実際に走る)
- [ ] WBS ログ `docs/wbs/2026-04-22-feature-N-kotoha-core-skeleton.md` が develop に push 済み(N は ISSUE 番号)

---

## Task M2-0: M2 用 ISSUE 作成 + branch 作成

**Files:** (ローカル変更なし、GitHub 操作と branch 作成のみ)

- [ ] **Step 1: 作業ディレクトリと状態確認**

Run:

```bash
cd /home/kohshiro/develops/student/kotoha-ime
git checkout develop
git pull
git status
git log --oneline -3
```

Expected: `develop` が `origin/develop` と同期、working tree clean、最新 commit が `01a7c1d`。

- [ ] **Step 2: M2 用 ISSUE を作成**

Run:

```bash
gh issue create \
  --title "M2: kotoha-core skeleton + error + kana utilities" \
  --body "Phase 0 Milestone 2: add the first crate to the workspace.

## Scope

- New crate \`crates/kotoha-core\`
- Update root \`Cargo.toml\` (workspace members + [workspace.dependencies])
- Implement \`error\` module (\`Error\` enum + \`Result\` alias)
- Implement \`kana\` module (4 functions: \`is_hiragana\`, \`is_katakana\`, \`hiragana_to_katakana\`, \`katakana_to_hiragana\`)
- 10+ unit tests for kana, 2+ unit tests for error

## Out of Scope

- \`romaji\` module (M3)
- \`input\` module (M4)
- property tests (M3)
- \`kotoha-cli\` (M6)

## Reference

- Spec: \`docs/superpowers/specs/2026-04-22-kotoha-phase-0-design.md\` §7.1, §7.7, §7.8, §11.1
- Plan: \`docs/superpowers/plans/2026-04-22-kotoha-phase-0-m2.md\`"
```

Expected: ISSUE が作成され、URL と ISSUE 番号が表示される(他 ISSUE の状態により #8 以降になる想定)。

- [ ] **Step 3: ISSUE 番号を取得し branch 作成**

Run(N は Step 2 で表示された ISSUE 番号):

```bash
git checkout -b feature/N-kotoha-core-skeleton develop
```

Expected: `Switched to a new branch 'feature/N-kotoha-core-skeleton'`。

- [ ] **Step 4: branch 確認**

Run:

```bash
git branch --show-current
```

Expected: `feature/N-kotoha-core-skeleton`(N は実値)。

---

## Task M2-1: Root Cargo.toml の workspace 更新

**Files:**
- Modify: `Cargo.toml`(workspace root)

- [ ] **Step 1: 現状の Cargo.toml を確認**

Run:

```bash
cat Cargo.toml
```

Expected: 現状は `members = []`(空、コメントのみ)、`[workspace.dependencies]` も空。

- [ ] **Step 2: Edit で `members` を更新**

Edit 対象ブロック(old_string):

```toml
[workspace]
resolver = "2"
members = [
    # crates/kotoha-core は M2 で追加
    # crates/kotoha-cli は M6 で追加
]
```

new_string:

```toml
[workspace]
resolver = "2"
members = [
    "crates/kotoha-core",
    # crates/kotoha-cli は M6 で追加
]
```

- [ ] **Step 3: Edit で `[workspace.dependencies]` を更新**

Edit 対象ブロック(old_string):

```toml
[workspace.dependencies]
# 共通 dependencies の version pinning は M2 以降で追加する。
# 本 M1 時点では workspace members がまだ存在しないので空とする。
```

new_string:

```toml
[workspace.dependencies]
thiserror = "1"
anyhow = "1"
tracing = "0.1"
tracing-subscriber = "0.3"
```

- [ ] **Step 4: `cargo metadata` が通ることを確認(member 未存在のためまだ build はできない)**

Run:

```bash
cargo metadata --no-deps --format-version=1 > /dev/null
echo "exit=$?"
```

Expected: `exit=0`。`crates/kotoha-core/Cargo.toml` はまだ作っていないが、`cargo metadata` はメンバーディレクトリを要求するので失敗する可能性がある。失敗した場合は Task M2-2 完了後にまとめて commit する(Step 5 の commit を Task M2-2 直後に延期)。

> **もし cargo metadata が "failed to load manifest for workspace member" 系エラーで失敗したら**: この Step では commit せず、Task M2-2 で `crates/kotoha-core/Cargo.toml` 作成後に `cargo metadata` が通るようになってから、まとめて 1 つの commit として「M2-1 + M2-2」をまとめる。Task M2-1 の独立 commit にこだわらない。

- [ ] **Step 5: commit(cargo metadata が通った場合のみ)**

Run:

```bash
git add Cargo.toml
git commit -m "chore: register kotoha-core as workspace member and pin shared deps"
```

Expected: `1 file changed`。

(cargo metadata が通らなかった場合はこの Step はスキップし、Task M2-2 Step 5 でまとめて commit する。)

---

## Task M2-2: `crates/kotoha-core/Cargo.toml` 作成

**Files:**
- Create: `crates/kotoha-core/Cargo.toml`

- [ ] **Step 1: ディレクトリ作成**

Run:

```bash
mkdir -p crates/kotoha-core/src/kana
```

Expected: 追加 4 階層が作成される(`crates/`、`crates/kotoha-core/`、`crates/kotoha-core/src/`、`crates/kotoha-core/src/kana/`)。

- [ ] **Step 2: Write ツールで `crates/kotoha-core/Cargo.toml` を作成**

内容(character-for-character):

```toml
[package]
name = "kotoha-core"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true
repository.workspace = true
authors.workspace = true
description = "Kotoha: core library for a Japanese IME (conversion, input mode management)"

[dependencies]
thiserror = { workspace = true }
tracing = { workspace = true }
```

`anyhow` と `tracing-subscriber` は CLI 層(M6)専用なので、`kotoha-core` の `[dependencies]` には入れない。

- [ ] **Step 3: `cargo metadata` で workspace が正しく認識されるか確認**

Run:

```bash
cargo metadata --no-deps --format-version=1 | jq '.workspace_members | length'
```

Expected: `1`(kotoha-core が member として認識)。

> `jq` が未インストールの場合は `cargo metadata --no-deps --format-version=1 | grep -c workspace_members` などで代替。

- [ ] **Step 4: `cargo build -p kotoha-core` が通ることを確認(空 lib でも OK)**

Run:

```bash
# まず src/lib.rs を一時的に空ファイルで作る(M2-3 で本実装、ここでは build 確認のみ)
touch crates/kotoha-core/src/lib.rs
cargo build -p kotoha-core
```

Expected: 空 `lib.rs` でも `cargo build` は成功する(`rustc` は empty crate を許容する)。`Finished dev profile` が出たら OK。

- [ ] **Step 5: commit**

Run:

```bash
git add Cargo.toml crates/kotoha-core/Cargo.toml crates/kotoha-core/src/lib.rs
git commit -m "feat(kotoha-core): create crate skeleton

- New crate crates/kotoha-core with workspace-inherited metadata
- Depend on thiserror and tracing via [workspace.dependencies]
- Empty lib.rs (real content added in subsequent tasks)
- Register in root Cargo.toml members"
```

Expected: `3 files changed` (root `Cargo.toml`、`crates/kotoha-core/Cargo.toml`、`crates/kotoha-core/src/lib.rs`)。Task M2-1 Step 5 で既に root `Cargo.toml` のみ commit 済みの場合は `2 files changed`。

---

## Task M2-3: `error` モジュールを TDD で実装

**Files:**
- Create: `crates/kotoha-core/src/error.rs`
- Modify: `crates/kotoha-core/src/lib.rs`

- [ ] **Step 1: lib.rs に error モジュール宣言を追加(テスト実行の前提)**

現状 `crates/kotoha-core/src/lib.rs` は空ファイル。Write ツールで以下を書き込む:

```rust
//! kotoha-core: the core library for the Kotoha Japanese IME.
//!
//! Phase 0 scope: error types and kana utilities.
//! Later milestones add romaji conversion (M3) and input mode state machine (M4).

pub mod error;

pub use error::{Error, Result};
```

- [ ] **Step 2: error モジュールの失敗するテストを書く**

Write で `crates/kotoha-core/src/error.rs` を新規作成:

```rust
//! Error types for kotoha-core.

use thiserror::Error;

/// The error type returned from kotoha-core operations.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum Error {
    /// A character outside the supported conversion rules was encountered.
    #[error("invalid character: {0:?}")]
    InvalidCharacter(char),

    /// The internal state machine reached an impossible configuration. Likely a bug.
    #[error("internal state inconsistency")]
    InvalidState,
}

/// Convenience alias: `Result<T, kotoha_core::Error>`.
pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_character_display_contains_char() {
        let err = Error::InvalidCharacter('Q');
        let msg = format!("{}", err);
        assert!(msg.contains('Q'), "display should include the char: {msg}");
    }

    #[test]
    fn invalid_state_display_is_stable() {
        let err = Error::InvalidState;
        let msg = format!("{}", err);
        assert_eq!(msg, "internal state inconsistency");
    }

    #[test]
    fn result_alias_matches_std() {
        fn returns_err() -> Result<()> {
            Err(Error::InvalidState)
        }
        assert!(returns_err().is_err());
    }
}
```

- [ ] **Step 3: テストを実行して PASS するか確認**

Run:

```bash
cargo test -p kotoha-core --lib error::tests
```

Expected: 3 tests passed(`invalid_character_display_contains_char`、`invalid_state_display_is_stable`、`result_alias_matches_std`)。

> Step 2 で仕様と実装を同時に書き下しているため、TDD の厳密な Red フェーズは省略している。これは `Error` の仕様が spec §7.8 で確定しており、テストケース選定と実装選定が一意に決まるため。`kana` モジュール(Task M2-4 以降)では仕様にバリエーションがあるので、Red → Green の手順をより明示的に踏む。

- [ ] **Step 4: clippy 確認**

Run:

```bash
cargo clippy -p kotoha-core --all-targets -- -D warnings
```

Expected: warnings ゼロ。

- [ ] **Step 5: commit**

Run:

```bash
git add crates/kotoha-core/src/lib.rs crates/kotoha-core/src/error.rs
git commit -m "feat(kotoha-core): add Error enum and Result alias

- Error is #[non_exhaustive] with InvalidCharacter(char) and InvalidState variants
- Uses thiserror for Display derivation
- Result<T> aliases std::result::Result<T, Error>
- 3 unit tests cover Display, variant equality, and Result alias usage"
```

Expected: `2 files changed`。

---

## Task M2-4: `kana` 判定関数(`is_hiragana` / `is_katakana`)を TDD で実装

**Files:**
- Create: `crates/kotoha-core/src/kana/mod.rs`
- Create: `crates/kotoha-core/src/kana/hiragana.rs`
- Create: `crates/kotoha-core/src/kana/katakana.rs`
- Modify: `crates/kotoha-core/src/lib.rs`

- [ ] **Step 1: lib.rs に kana モジュール宣言を追加**

Edit `crates/kotoha-core/src/lib.rs`:

old_string:

```rust
pub mod error;

pub use error::{Error, Result};
```

new_string:

```rust
pub mod error;
pub mod kana;

pub use error::{Error, Result};
```

- [ ] **Step 2: `crates/kotoha-core/src/kana/mod.rs` を Write で作成**

内容:

```rust
//! Utilities for manipulating Japanese kana characters.
//!
//! The functions in this module classify characters (hiragana vs katakana)
//! and convert between the two scripts using the fixed `0x60` code-point offset
//! between the Hiragana (`U+3040`) and Katakana (`U+30A0`) Unicode blocks.

mod hiragana;
mod katakana;

pub use hiragana::{hiragana_to_katakana, is_hiragana};
pub use katakana::{is_katakana, katakana_to_hiragana};
```

- [ ] **Step 3: `is_hiragana` の失敗するテストを `hiragana.rs` に書く(Red)**

Write `crates/kotoha-core/src/kana/hiragana.rs`:

```rust
//! Hiragana classification and conversion.

/// Returns `true` iff `ch` is a hiragana code point.
///
/// Supported ranges:
/// - `U+3041..=U+3096` (ぁ..ゖ, 86 code points)
/// - `U+309D..=U+309F` (ゝ, ゞ, ゟ)
pub fn is_hiragana(_ch: char) -> bool {
    unimplemented!("implemented in Task M2-4 Step 5")
}

/// Converts hiragana in `s` to katakana. Non-hiragana characters pass through unchanged.
pub fn hiragana_to_katakana(_s: &str) -> String {
    unimplemented!("implemented in Task M2-5 Step 3")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_hiragana_basic_five_vowels() {
        assert!(is_hiragana('あ'));
        assert!(is_hiragana('い'));
        assert!(is_hiragana('う'));
        assert!(is_hiragana('え'));
        assert!(is_hiragana('お'));
    }

    #[test]
    fn is_hiragana_small_forms() {
        assert!(is_hiragana('ぁ'));
        assert!(is_hiragana('ゃ'));
        assert!(is_hiragana('っ'));
    }

    #[test]
    fn is_hiragana_obsolete_forms() {
        assert!(is_hiragana('ゐ'));
        assert!(is_hiragana('ゑ'));
        assert!(is_hiragana('ゖ'));
    }

    #[test]
    fn is_hiragana_repeat_marks() {
        assert!(is_hiragana('ゝ'));
        assert!(is_hiragana('ゞ'));
        assert!(is_hiragana('ゟ'));
    }

    #[test]
    fn is_hiragana_rejects_katakana_and_ascii() {
        assert!(!is_hiragana('ア'));
        assert!(!is_hiragana('カ'));
        assert!(!is_hiragana('a'));
        assert!(!is_hiragana('1'));
        assert!(!is_hiragana('漢'));
        assert!(!is_hiragana(' '));
    }
}
```

- [ ] **Step 4: テストを実行して失敗することを確認(Red)**

Run:

```bash
cargo test -p kotoha-core --lib kana::hiragana::tests::is_hiragana 2>&1 | tail -30
```

Expected: 5 tests all FAIL with panic `not implemented: implemented in Task M2-4 Step 5`。5 件失敗している状態を確認する(Red フェーズ)。

- [ ] **Step 5: `is_hiragana` を実装(Green)**

Edit `crates/kotoha-core/src/kana/hiragana.rs`:

old_string:

```rust
pub fn is_hiragana(_ch: char) -> bool {
    unimplemented!("implemented in Task M2-4 Step 5")
}
```

new_string:

```rust
pub fn is_hiragana(ch: char) -> bool {
    matches!(ch, '\u{3041}'..='\u{3096}' | '\u{309D}'..='\u{309F}')
}
```

- [ ] **Step 6: テストが PASS することを確認(Green)**

Run:

```bash
cargo test -p kotoha-core --lib kana::hiragana::tests::is_hiragana
```

Expected: 5 passed(`is_hiragana_basic_five_vowels`、`is_hiragana_small_forms`、`is_hiragana_obsolete_forms`、`is_hiragana_repeat_marks`、`is_hiragana_rejects_katakana_and_ascii`)。

- [ ] **Step 7: `katakana.rs` を作成して `is_katakana` を同様に TDD**

Write `crates/kotoha-core/src/kana/katakana.rs`:

```rust
//! Katakana classification and conversion.

/// Returns `true` iff `ch` is a katakana code point.
///
/// Supported ranges:
/// - `U+30A1..=U+30FA` (ァ..ヺ, 90 code points)
/// - `U+30FC` (ー, prolonged sound mark)
/// - `U+30FD..=U+30FF` (ヽ, ヾ, ヿ)
pub fn is_katakana(_ch: char) -> bool {
    unimplemented!("implemented in Task M2-4 Step 9")
}

/// Converts katakana in `s` to hiragana. Non-katakana characters pass through unchanged.
/// Note: `ヷ`, `ヸ`, `ヹ`, `ヺ` (U+30F7..=U+30FA) and `ー` (U+30FC) have no hiragana counterpart
/// and pass through unchanged.
pub fn katakana_to_hiragana(_s: &str) -> String {
    unimplemented!("implemented in Task M2-5 Step 6")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_katakana_basic_five_vowels() {
        assert!(is_katakana('ア'));
        assert!(is_katakana('イ'));
        assert!(is_katakana('ウ'));
        assert!(is_katakana('エ'));
        assert!(is_katakana('オ'));
    }

    #[test]
    fn is_katakana_small_forms_and_prolonged_mark() {
        assert!(is_katakana('ァ'));
        assert!(is_katakana('ャ'));
        assert!(is_katakana('ッ'));
        assert!(is_katakana('ー'));
    }

    #[test]
    fn is_katakana_v_row_and_repeat_marks() {
        assert!(is_katakana('ヴ'));
        assert!(is_katakana('ヶ'));
        assert!(is_katakana('ヽ'));
        assert!(is_katakana('ヾ'));
        assert!(is_katakana('ヿ'));
    }

    #[test]
    fn is_katakana_rejects_hiragana_and_ascii() {
        assert!(!is_katakana('あ'));
        assert!(!is_katakana('か'));
        assert!(!is_katakana('a'));
        assert!(!is_katakana('1'));
        assert!(!is_katakana('漢'));
    }
}
```

- [ ] **Step 8: テストを実行して失敗することを確認(Red)**

Run:

```bash
cargo test -p kotoha-core --lib kana::katakana::tests::is_katakana 2>&1 | tail -20
```

Expected: 4 tests all FAIL with `unimplemented` panic。

- [ ] **Step 9: `is_katakana` を実装(Green)**

Edit `crates/kotoha-core/src/kana/katakana.rs`:

old_string:

```rust
pub fn is_katakana(_ch: char) -> bool {
    unimplemented!("implemented in Task M2-4 Step 9")
}
```

new_string:

```rust
pub fn is_katakana(ch: char) -> bool {
    matches!(
        ch,
        '\u{30A1}'..='\u{30FA}' | '\u{30FC}' | '\u{30FD}'..='\u{30FF}'
    )
}
```

- [ ] **Step 10: テストが PASS することを確認(Green)**

Run:

```bash
cargo test -p kotoha-core --lib kana::katakana::tests::is_katakana
```

Expected: 4 passed。

- [ ] **Step 11: clippy + fmt チェック**

Run:

```bash
cargo clippy -p kotoha-core --all-targets -- -D warnings
cargo fmt --all --check
```

Expected: warnings ゼロ、fmt diff なし。

- [ ] **Step 12: commit**

Run:

```bash
git add crates/kotoha-core/src/lib.rs crates/kotoha-core/src/kana/
git commit -m "feat(kotoha-core): add kana classification (is_hiragana, is_katakana)

- kana/mod.rs re-exports public functions from hiragana and katakana submodules
- is_hiragana: U+3041..=U+3096 | U+309D..=U+309F
- is_katakana: U+30A1..=U+30FA | U+30FC | U+30FD..=U+30FF
- 9 unit tests total (5 hiragana + 4 katakana)
- Conversion functions are stubbed (implemented in next task)"
```

Expected: 4 files changed(`lib.rs`、`kana/mod.rs`、`kana/hiragana.rs`、`kana/katakana.rs`)。

---

## Task M2-5: `kana` 変換関数(`hiragana_to_katakana` / `katakana_to_hiragana`)を TDD で実装

**Files:**
- Modify: `crates/kotoha-core/src/kana/hiragana.rs`
- Modify: `crates/kotoha-core/src/kana/katakana.rs`

- [ ] **Step 1: `hiragana_to_katakana` の失敗するテストを追加**

Edit `crates/kotoha-core/src/kana/hiragana.rs` の `#[cfg(test)] mod tests` ブロック内に以下を追加(末尾の `}` の直前に挿入):

new テスト関数(既存の最後のテスト `is_hiragana_rejects_katakana_and_ascii` の閉じ `}` の後、`mod tests` 自体の閉じ `}` の直前に挿入):

```rust
    #[test]
    fn hiragana_to_katakana_basic() {
        assert_eq!(hiragana_to_katakana("あいうえお"), "アイウエオ");
    }

    #[test]
    fn hiragana_to_katakana_mixed() {
        assert_eq!(hiragana_to_katakana("こんにちは"), "コンニチハ");
    }

    #[test]
    fn hiragana_to_katakana_passes_through_non_hiragana() {
        assert_eq!(hiragana_to_katakana("ABC"), "ABC");
        assert_eq!(hiragana_to_katakana("あA1"), "アA1");
    }

    #[test]
    fn hiragana_to_katakana_empty() {
        assert_eq!(hiragana_to_katakana(""), "");
    }

    #[test]
    fn hiragana_to_katakana_small_forms() {
        assert_eq!(hiragana_to_katakana("ぁっゃゅょ"), "ァッャュョ");
    }
```

- [ ] **Step 2: テストを実行して失敗することを確認(Red)**

Run:

```bash
cargo test -p kotoha-core --lib kana::hiragana::tests::hiragana_to_katakana 2>&1 | tail -20
```

Expected: 5 tests all FAIL with `unimplemented` panic。

- [ ] **Step 3: `hiragana_to_katakana` を実装(Green)**

Edit `crates/kotoha-core/src/kana/hiragana.rs`:

old_string:

```rust
pub fn hiragana_to_katakana(_s: &str) -> String {
    unimplemented!("implemented in Task M2-5 Step 3")
}
```

new_string:

```rust
pub fn hiragana_to_katakana(s: &str) -> String {
    s.chars()
        .map(|ch| {
            if is_hiragana(ch) {
                // Hiragana → Katakana: add the fixed +0x60 offset.
                // All 3 covered hiragana ranges land inside katakana ranges after this shift.
                char::from_u32(ch as u32 + 0x60).unwrap_or(ch)
            } else {
                ch
            }
        })
        .collect()
}
```

- [ ] **Step 4: テストが PASS することを確認(Green)**

Run:

```bash
cargo test -p kotoha-core --lib kana::hiragana::tests::hiragana_to_katakana
```

Expected: 5 passed。

- [ ] **Step 5: `katakana_to_hiragana` の失敗するテストを追加**

Edit `crates/kotoha-core/src/kana/katakana.rs` の `#[cfg(test)] mod tests` ブロック内に以下を追加(既存最後のテスト `is_katakana_rejects_hiragana_and_ascii` の閉じ `}` の後、`mod tests` 自体の閉じ `}` の直前に挿入):

```rust
    #[test]
    fn katakana_to_hiragana_basic() {
        assert_eq!(katakana_to_hiragana("アイウエオ"), "あいうえお");
    }

    #[test]
    fn katakana_to_hiragana_mixed() {
        assert_eq!(katakana_to_hiragana("コンニチハ"), "こんにちは");
    }

    #[test]
    fn katakana_to_hiragana_keeps_prolonged_mark() {
        // U+30FC has no hiragana counterpart and must pass through unchanged.
        assert_eq!(katakana_to_hiragana("コーヒー"), "こーひー");
    }

    #[test]
    fn katakana_to_hiragana_keeps_v_row_without_counterpart() {
        // U+30F7..=U+30FA have no hiragana counterpart and must pass through unchanged.
        assert_eq!(katakana_to_hiragana("ヷヸヹヺ"), "ヷヸヹヺ");
    }

    #[test]
    fn katakana_to_hiragana_handles_v_with_counterpart() {
        // ヴ U+30F4 → ゔ U+3094 (within the convertible range).
        assert_eq!(katakana_to_hiragana("ヴ"), "ゔ");
    }

    #[test]
    fn katakana_to_hiragana_empty() {
        assert_eq!(katakana_to_hiragana(""), "");
    }
```

- [ ] **Step 6: テストを実行して失敗することを確認(Red)**

Run:

```bash
cargo test -p kotoha-core --lib kana::katakana::tests::katakana_to_hiragana 2>&1 | tail -20
```

Expected: 6 tests all FAIL with `unimplemented` panic。

- [ ] **Step 7: `katakana_to_hiragana` を実装(Green)**

Edit `crates/kotoha-core/src/kana/katakana.rs`:

old_string:

```rust
pub fn katakana_to_hiragana(_s: &str) -> String {
    unimplemented!("implemented in Task M2-5 Step 6")
}
```

new_string:

```rust
pub fn katakana_to_hiragana(s: &str) -> String {
    s.chars()
        .map(|ch| {
            // Convert only katakana that have a hiragana counterpart:
            // - U+30A1..=U+30F6 maps to U+3041..=U+3096
            // - U+30FD..=U+30FF maps to U+309D..=U+309F
            // U+30F7..=U+30FA (ヷヸヹヺ) and U+30FC (ー) have no hiragana peer and pass through.
            if matches!(ch, '\u{30A1}'..='\u{30F6}' | '\u{30FD}'..='\u{30FF}') {
                char::from_u32(ch as u32 - 0x60).unwrap_or(ch)
            } else {
                ch
            }
        })
        .collect()
}
```

- [ ] **Step 8: テストが PASS することを確認(Green)**

Run:

```bash
cargo test -p kotoha-core --lib kana::katakana::tests::katakana_to_hiragana
```

Expected: 6 passed。

- [ ] **Step 9: 全 kana テストをまとめて実行**

Run:

```bash
cargo test -p kotoha-core --lib kana
```

Expected: 20 tests passed(hiragana 判定 5 + hiragana 変換 5 + katakana 判定 4 + katakana 変換 6)。

- [ ] **Step 10: clippy + fmt チェック**

Run:

```bash
cargo clippy -p kotoha-core --all-targets -- -D warnings
cargo fmt --all --check
```

Expected: warnings ゼロ、fmt diff なし。

- [ ] **Step 11: commit**

Run:

```bash
git add crates/kotoha-core/src/kana/hiragana.rs crates/kotoha-core/src/kana/katakana.rs
git commit -m "feat(kotoha-core): add kana conversion (hiragana_to_katakana, katakana_to_hiragana)

- Uses the fixed 0x60 code-point offset between Hiragana and Katakana blocks
- hiragana_to_katakana: covers U+3041..=U+3096 and U+309D..=U+309F
- katakana_to_hiragana: covers U+30A1..=U+30F6 and U+30FD..=U+30FF
  (U+30F7..=U+30FA and U+30FC have no hiragana counterpart and pass through)
- 11 new unit tests (5 hiragana→katakana + 6 katakana→hiragana)"
```

Expected: `2 files changed`。

---

## Task M2-6: 最終ビルド確認と総合チェック

**Files:** (変更なし、検証のみ)

- [ ] **Step 1: 全体 workspace が通ることを確認**

Run:

```bash
cargo build --workspace
```

Expected: `Finished dev profile` が表示される(M1 では members 空で失敗していたが、M2 で初めて通るようになる)。

- [ ] **Step 2: 全テスト実行**

Run:

```bash
cargo test --workspace
```

Expected: 合計 23 tests passed(error 3 + kana 20)。

- [ ] **Step 3: clippy 全体チェック**

Run:

```bash
cargo clippy --workspace --all-targets -- -D warnings
```

Expected: warnings ゼロ。

- [ ] **Step 4: fmt チェック**

Run:

```bash
cargo fmt --all --check
```

Expected: diff なし。

- [ ] **Step 5: lefthook pre-push の実行(M2 でついに build/clippy/test が実行される)**

Run:

```bash
lefthook run pre-push
```

Expected:
- `manifest-check` PASS
- `build` 実行(M1 では skip だった)→ PASS
- `clippy` 実行 → PASS(warnings ゼロ)
- `test` 実行 → 23 件 PASS

> M1 plan で設計した `if [ -d crates ] && [ -n "$(ls -A crates 2>/dev/null)" ]` ガードが `crates/kotoha-core` の登場で真になり、自動的に skip から実行に切り替わることを確認する重要な Step。

- [ ] **Step 6: commit 不要(検証のみ)**

変更ファイルなし。`git status` が clean であることを確認:

```bash
git status
```

Expected: `nothing to commit, working tree clean`。

---

## Task M2-7: PR 作成と review + merge + WBS ログ

**Files:** (変更なし → PR 作成 → merge → WBS ログ)

- [ ] **Step 1: push**

Run:

```bash
git push -u origin feature/N-kotoha-core-skeleton
```

Expected: branch が push され、pre-push hook が自動実行される。build/clippy/test が全 PASS。

- [ ] **Step 2: PR 作成**

Run(N は Task M2-0 で作成した ISSUE 番号):

```bash
gh pr create --base develop --head feature/N-kotoha-core-skeleton \
  --title "M2: kotoha-core skeleton + error + kana utilities" \
  --body "$(cat <<'EOS'
## Summary

Phase 0 Milestone 2: add the first crate to the workspace.

- New crate `crates/kotoha-core`
- Update root `Cargo.toml` (workspace members + [workspace.dependencies]: thiserror, anyhow, tracing, tracing-subscriber)
- `error` module: `Error` enum (#[non_exhaustive], 2 variants) + `Result<T>` alias
- `kana` module: `is_hiragana`, `is_katakana`, `hiragana_to_katakana`, `katakana_to_hiragana`
- 23 unit tests (3 error + 20 kana)

This is the first milestone where `cargo build --workspace` actually runs `rustc`.
The lefthook `[ -d crates ] && [ -n "$(ls -A crates 2>/dev/null)" ]` guard transitions
from "skip" to "execute" automatically once this PR lands on develop.

## Related

- Spec: docs/superpowers/specs/2026-04-22-kotoha-phase-0-design.md §7.1, §7.7, §7.8, §11.1
- Plan: docs/superpowers/plans/2026-04-22-kotoha-phase-0-m2.md
- Closes #N

## Test plan

- [ ] cargo build --workspace passes (first time since M1 deferred empty-workspace check)
- [ ] cargo test --workspace passes with 23 unit tests
- [ ] cargo clippy --workspace --all-targets -- -D warnings: zero warnings
- [ ] cargo fmt --all --check: no diff
- [ ] lefthook pre-push transitions from skip mode to full execution successfully
EOS
)"
```

Replace `#N` with the ISSUE number from Task M2-0. Expected: PR 作成成功、URL 表示。

- [ ] **Step 3: PR review(Small tier)**

CLAUDE.md の PR Review Matrix: Small tier(7 files 変更、実コード部分は kana で 2 file, error で 1 file)。Rust 実装コードを含むため:

- `agent-teams:team-review` dimensions=security,architecture,testing(または experimental flag 無しなら `superpowers:code-reviewer` を 3 dimension で並列 dispatch)
- `secrets-check`

Run(Skill ツール経由、または controller agent の判断で):

```
/agent-teams:team-review dimensions=security,architecture,testing
/secrets-check
```

Expected: 合格(Critical / High ゼロが理想)。High 以上があれば next Step で解消。

- [ ] **Step 4: review findings 解消**

findings があれば implementer subagent に修正依頼、再 review を繰り返す。Critical / High ゼロになったら merge。

- [ ] **Step 5: squash merge + branch 削除**

Run:

```bash
gh pr merge <PR番号> --squash --delete-branch
gh pr view <PR番号> --json state,mergeCommit -q '{state, merge: .mergeCommit.oid}'
```

Expected: `state: MERGED`、merge commit SHA が取得できる。

- [ ] **Step 6: develop に切り替え + pull**

Run:

```bash
git checkout develop
git pull
git log --oneline -5
```

Expected: squash-merge commit が develop 先頭に来ている。

- [ ] **Step 7: WBS ログ作成**

Write `docs/wbs/2026-04-22-feature-N-kotoha-core-skeleton.md`(`N` は ISSUE 番号、`<MERGE_COMMIT>` は Step 5 で取得した merge commit SHA):

```markdown
---
milestone: M2
branch: feature/N-kotoha-core-skeleton
pr: "#<PR_NUMBER>"
merge_commit: "<MERGE_COMMIT>"
issue: "#N"
status: done
started: 2026-04-22
finished: 2026-04-22
---

# M2: kotoha-core skeleton + error + kana utilities

## 実施内容

- root Cargo.toml の workspace members に `crates/kotoha-core` を追加
- [workspace.dependencies] に thiserror / anyhow / tracing / tracing-subscriber を追加
- `crates/kotoha-core/Cargo.toml` を workspace 継承形で作成
- `src/error.rs`: `Error` enum (#[non_exhaustive]) + `Result<T>` alias + 3 unit tests
- `src/kana/mod.rs`: `hiragana` / `katakana` サブモジュールの再エクスポート
- `src/kana/hiragana.rs`: `is_hiragana` + `hiragana_to_katakana` + 10 unit tests
- `src/kana/katakana.rs`: `is_katakana` + `katakana_to_hiragana` + 10 unit tests
- `src/lib.rs`: 公開 API のルート(`pub mod error; pub mod kana; pub use error::{Error, Result};`)

## つまずき

(実施時に記入。例: cargo clippy の特定 lint で変換関数の実装を修正した等)

## M3 への申し送り

- `RomajiConverter` は `kotoha-core::input::InputContext` に内包される予定(spec §7.5)。M3 で `romaji` モジュール実装後、M4 で `input` モジュール実装。
- `proptest` の dev-dependency 追加は M3 の最初のタスク。
- `kana` ユーティリティは `input::InputContext` の `preedit()` 整形で将来利用する可能性がある。API 互換性を維持すること。

## 成果物リンク

- PR: #<PR_NUMBER>
- ISSUE: #N
- Spec: `docs/superpowers/specs/2026-04-22-kotoha-phase-0-design.md`
- Plan: `docs/superpowers/plans/2026-04-22-kotoha-phase-0-m2.md`
```

- [ ] **Step 8: WBS を develop に直接 push**

Run:

```bash
git add docs/wbs/2026-04-22-feature-N-kotoha-core-skeleton.md
git commit -m "docs: M2 implementation log"
git push
```

> project CLAUDE.md の「WBS 直接 push の例外」により develop への直接 push が許容される。

Expected: lefthook pre-push が M1 lefthook 改善後の設定で走る。`[ -d crates ] && [ -n "$(ls -A crates 2>/dev/null)" ]` が true になって build/clippy/test 実行、PASS。

---

## M2 完了条件チェックリスト

- [ ] `cargo build --workspace` PASS
- [ ] `cargo build -p kotoha-core` PASS
- [ ] `cargo test -p kotoha-core` PASS(kana 20 件 + error 3 件 = 計 23 件)
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` warnings ゼロ
- [ ] `cargo fmt --all --check` diff ゼロ
- [ ] `lefthook run pre-push` 全コマンド PASS(skip なし)
- [ ] `crates/kotoha-core/src/lib.rs` が `pub mod error; pub mod kana; pub use error::{Error, Result};` を export
- [ ] `is_hiragana`、`is_katakana`、`hiragana_to_katakana`、`katakana_to_hiragana` が `kotoha_core::kana::` で利用可能
- [ ] `Error` が `#[non_exhaustive]` + `thiserror::Error` 実装
- [ ] `Result<T>` が `std::result::Result<T, Error>` の別名
- [ ] PR が develop に squash-merge 済み
- [ ] `feature/N-kotoha-core-skeleton` branch が削除済み
- [ ] `docs/wbs/2026-04-22-feature-N-kotoha-core-skeleton.md` が develop に存在

---

## Spec Coverage 確認

本 plan が実装するもの(Spec §との対応):

| Spec 要件 | Plan のカバー位置 |
|---|---|
| §7.1 公開 API ルート | Task M2-3 Step 1、Task M2-4 Step 1 |
| §7.7 Kana ユーティリティ(4 関数) | Task M2-4 + M2-5 |
| §7.8 Error 型 | Task M2-3 |
| §11.1 単体テスト(kotoha-core 分で 10 件以上) | 合計 23 件実装(error 3 + kana 20) |
| §12 品質ゲート(lefthook pre-commit/pre-push) | Task M2-6 Step 5 で lefthook pre-push を初めて実運用 |

Spec §13.1 の「`cargo build --workspace` PASS」は M1 plan で M2 に deferred した項目であり、本 plan の Task M2-6 Step 1 で達成する。

## Self-Review 済み事項

1. **プレースホルダスキャン**: `N`(ISSUE 番号)、`<PR_NUMBER>`、`<MERGE_COMMIT>` は意図した placeholder。着手時に実値へ置換する。
2. **型・シグネチャ一貫性**: `Error` / `Result` / `is_hiragana` / `is_katakana` / `hiragana_to_katakana` / `katakana_to_hiragana` のシグネチャは Spec §7.7 / §7.8 と完全一致。
3. **Spec カバレッジ**: §7.1 / §7.7 / §7.8 / §11.1 の M2 scope 該当部分をすべて Task に割り当て済み。§7.2 / §7.3 / §7.4 / §7.5 / §7.6 は M3 / M4 で扱う(M2 scope 外)。
4. **TDD 遵守**: kana モジュールは Red → Green の明示的 Step 分割あり(Step 3/4/5、Step 7/8/9 など)。error モジュールは仕様が一意に確定しているため Red フェーズは簡略化(Step 2 でテスト + 実装を同一ファイル内に書き、Step 3 で検証)。
5. **CLAUDE.md 制約**: 本 M2 scope の実質変更行数は 200 行前後(実装 + テスト + Cargo.toml 更新)で、Branch Scope Policy(10 files / 300 lines / 2 日)に収まる。
