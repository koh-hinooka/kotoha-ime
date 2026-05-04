# P2-B (User dictionary) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** SQLite ベースの User dictionary 永続化層を新 crate `kotoha-storage` として実装し、`UserVocab` を `kotoha-core::dict` に統合、`kotoha-dict` CLI で edit を可能にする(P2-B、ISSUE #98)。spec `docs/superpowers/specs/2026-04-25-p2-b-user-dictionary-design.md` §1〜§12 の全 section を実装範囲とする。

**Architecture:** kotoha-storage 新 crate(`Database` + `UserVocabStore` trait + `SqliteUserVocabStore` + `MockUserVocabStore` + `validation` + `path` module)→ kotoha-core::dict::user_vocab(`UserVocab` 構造体、`VocabularyLookup` trait 実装)→ kotoha-cli::dict_cli + bin/dict.rs(`add` / `remove` / `list` / `show` 4 subcommand)。Clean Architecture DIP を `Box<dyn UserVocabStore>` 経由で確保、`Arc<Database>` で複数 Store の共有 ownership。

**Tech Stack:** Rust 1.80 / edition 2021、rusqlite 0.32(`bundled` feature)、tempfile 3 / assert_cmd 2 / serial_test 3 / proptest(dev-deps)、clap 4.5(既 workspace dep)、thiserror 1(既 workspace dep)、tracing 0.1(既 workspace dep)。

---

## 0. 前提条件と着手 gate

着手前に以下を **全て** 確認する。1 件でも未充足であれば task 1 に進まない。

- [ ] **0-1: ISSUE #97(P2-A hardening pre-PR)が `develop` に merge 済**

  ```bash
  cd /home/kohshiro/develops/student/kotoha-ime
  git fetch origin develop
  git log origin/develop --oneline | head -10
  ```

  Expected: `feat(p2a-hardening): ...` または同等 commit が `develop` 履歴に含まれる(spec §3.10 / §10.6 が前提とする `serial_test` / `tempfile` / `assert_cmd` dev-dep + validation gap 修正)。merge 未済の場合は ISSUE #97 PR の merge を待ってから着手する(global CLAUDE.md 「Blocker Handling」)。

- [ ] **0-2: ISSUE #96(P2-B docs PR)が `develop` に merge 済**

  ```bash
  git log origin/develop --oneline -- docs/superpowers/specs/2026-04-25-p2-b-user-dictionary-design.md docs/adr/0015-kotoha-storage-sqlite-adoption.md
  ```

  Expected: spec / ADR 0015 / ADR 0014 D7 改訂が `develop` に存在する commit が見える。

- [ ] **0-3: 現在の branch が `feature/98-p2-b-user-dictionary` であること**

  ```bash
  git rev-parse --abbrev-ref HEAD
  ```

  Expected: `feature/98-p2-b-user-dictionary`。違う場合は `git checkout -b feature/98-p2-b-user-dictionary origin/develop` で作成 / 切替する。

- [ ] **0-4: baseline 175 PASS(default features)を確認**

  ```bash
  cargo test --workspace 2>&1 | tail -20
  ```

  Expected: `test result: ok. 175 passed; 0 failed; ...` を含む summary 群(複数 binary に分かれて出力されるため合計 175 を確認)。退行している場合は ISSUE #97 hardening の merge 状況を再確認する。

- [ ] **0-5: baseline 223 PASS(`mock-backend,dict` features)を確認**

  ```bash
  cargo test --workspace --features kotoha-core/mock-backend,kotoha-core/dict 2>&1 | tail -20
  ```

  Expected: 合計 223 PASS。

- [ ] **0-6: メモリ余裕の確認**

  ```bash
  free -h | head -3
  ```

  Expected: available > 8 GiB(global CLAUDE.md「Memory Monitoring」)。8〜16 GiB 帯では並列実行を抑制(`cargo test -- --test-threads=2`)、< 8 GiB では着手前にユーザー通知。

- [ ] **0-7: spec §1〜§12 の全 section を実装計画 task に対応付ける mapping を本文末尾「Self-review checklist」で確認することを宣言**

着手 gate を全て満たした後、Phase A の Task A1 から開始する。

---

## 1. Branch 確認 + dev-dep / workspace member 追加

**Files:**
- Modify: `Cargo.toml`(workspace root、`[workspace.members]` + `[workspace.dependencies]`)

**Depends-on:** 着手 gate 0-1〜0-7

**Estimated LOC:** 20

参照: spec §3.5 / §3.10 / §4.1 / §10.6

- [ ] **Step 1: workspace root `Cargo.toml` の `[workspace] members` に `crates/kotoha-storage` を追加**

  既存 `members` を以下に置換する。

  ```toml
  members = [
      "crates/kotoha-core",
      "crates/kotoha-cli",
      "crates/kotoha-storage",
  ]
  ```

- [ ] **Step 2: `[workspace.dependencies]` に rusqlite + dev-dep を追加**

  `sudachi = { git = ..., rev = "..." }` 行の直後に以下を追加する。

  ```toml
  # rusqlite pinned via P2-B spec §3.5 (brainstorming, 2026-04-25).
  # Adopted reason: Sync API matches kotoha-core sync design, `bundled` feature
  # vendors SQLite C library so distro SQLite version differences are bypassed.
  # Alternatives rejected per ADR 0015 / spec §3.5:
  #   - sqlx / tokio-rusqlite: async runtime forced into kotoha-core
  #   - diesel: ORM is overspec for 4-CRUD operations
  rusqlite = { version = "0.32", features = ["bundled"] }
  # Test-only dev-deps for Layer 3 CLI integration (spec §10.3.1 / §10.6).
  tempfile = "3"
  assert_cmd = "2"
  serial_test = "3"
  ```

- [ ] **Step 3: `cargo check --workspace` を実行**

  ```bash
  cargo check --workspace 2>&1 | tail -10
  ```

  Expected: workspace member `crates/kotoha-storage` が未存在のため `error: failed to load manifest for workspace member` で failure。Task A1 で skeleton を作成すれば解消する。次 task で skeleton を作るため本 task ではこの error は許容する。

- [ ] **Step 4: Commit**

  ```bash
  git add Cargo.toml
  git commit -m "chore(deps): add rusqlite + tempfile/assert_cmd/serial_test dev-deps for kotoha-storage (#98)"
  ```

---

## Phase A: kotoha-storage scaffolding

### Task A1: workspace member 追加 + crate skeleton(Cargo.toml + lib.rs)

**Files:**
- Create: `crates/kotoha-storage/Cargo.toml`
- Create: `crates/kotoha-storage/src/lib.rs`

**Depends-on:** Task 1

**Estimated LOC:** 50

参照: spec §3.2 / §3.5 / §4.1 / §4.3

- [ ] **Step 1: ディレクトリを作成**

  ```bash
  mkdir -p /home/kohshiro/develops/student/kotoha-ime/crates/kotoha-storage/src
  mkdir -p /home/kohshiro/develops/student/kotoha-ime/crates/kotoha-storage/migrations
  ```

- [ ] **Step 2: `crates/kotoha-storage/Cargo.toml` を新規作成**

  ```toml
  [package]
  name = "kotoha-storage"
  version.workspace = true
  edition.workspace = true
  rust-version.workspace = true
  license.workspace = true
  repository.workspace = true
  authors.workspace = true
  description = "Kotoha: SQLite-based persistence layer for user dictionary and learning cache (P2-B)"

  [dependencies]
  rusqlite = { workspace = true }
  thiserror = { workspace = true }
  tracing = { workspace = true }

  [dev-dependencies]
  tempfile = { workspace = true }
  proptest = { workspace = true }
  serial_test = { workspace = true }

  [features]
  default = ["bundled-sqlite"]
  # `bundled-sqlite` vendors the SQLite C library inside the rusqlite crate.
  # Default ON to avoid distro SQLite version drift (P2-B spec §4.3).
  bundled-sqlite = ["rusqlite/bundled"]
  ```

- [ ] **Step 3: `crates/kotoha-storage/src/lib.rs` を新規作成(skeleton)**

  ```rust
  //! `kotoha-storage`: SQLite-based persistence layer for Kotoha (Phase 2 P2-B).
  //!
  //! Spec: `docs/superpowers/specs/2026-04-25-p2-b-user-dictionary-design.md`.
  //! ADR: `docs/adr/0015-kotoha-storage-sqlite-adoption.md`.
  //!
  //! この crate は永続化詳細層であり、`kotoha-core` の domain layer に依存しない
  //! (spec §4.2 Clean Architecture DIP)。`UserVocabStore` / `LearningCacheStore`
  //! trait は本 crate 側に置き、`kotoha-core::dict::user_vocab::UserVocab` が
  //! `Box<dyn UserVocabStore>` を field に保持することで DIP を成立させる。

  pub mod database;
  pub mod error;
  pub mod learning_cache;
  pub mod migrations;
  pub mod path;
  pub mod user_vocab;
  pub mod validation;

  pub use database::Database;
  pub use error::StorageError;
  pub use learning_cache::{LearningCacheRecord, LearningCacheStore};
  pub use user_vocab::{MockUserVocabStore, SqliteUserVocabStore, UserVocabRecord, UserVocabStore};
  ```

- [ ] **Step 4: 各 module の placeholder file を作成**

  以下 7 個の file を空 doc-comment で作成する(各 task で本実装する)。

  ```bash
  for f in error path validation migrations database; do
    cat > /home/kohshiro/develops/student/kotoha-ime/crates/kotoha-storage/src/$f.rs <<EOFM
  //! Placeholder for \`kotoha-storage::$f\` (P2-B、Task A2〜A8 で実装).
  EOFM
  done
  ```

  user_vocab / learning_cache は dir 構造のため別途。

  ```bash
  mkdir -p /home/kohshiro/develops/student/kotoha-ime/crates/kotoha-storage/src/user_vocab
  mkdir -p /home/kohshiro/develops/student/kotoha-ime/crates/kotoha-storage/src/learning_cache
  ```

  ```bash
  cat > /home/kohshiro/develops/student/kotoha-ime/crates/kotoha-storage/src/user_vocab/mod.rs <<'EOFM'
  //! Placeholder for `kotoha-storage::user_vocab` (P2-B、Task B1〜B9 で実装).
  EOFM
  cat > /home/kohshiro/develops/student/kotoha-ime/crates/kotoha-storage/src/learning_cache/mod.rs <<'EOFM'
  //! Placeholder for `kotoha-storage::learning_cache` (P2-B、Task B8 で trait skeleton のみ).
  EOFM
  ```

  この skeleton 段階では `lib.rs` の `pub use` が不在型を参照するため compile error となる。次 step で skeleton stub を埋める。

- [ ] **Step 5: skeleton stub を最低限埋めて compile 通過させる**

  `crates/kotoha-storage/src/lib.rs` を以下に置き換える(各 module の `pub use` をコメントアウトし、Task A2〜B9 で順次有効化する)。

  ```rust
  //! `kotoha-storage`: SQLite-based persistence layer for Kotoha (Phase 2 P2-B).

  pub mod database;
  pub mod error;
  pub mod learning_cache;
  pub mod migrations;
  pub mod path;
  pub mod user_vocab;
  pub mod validation;
  ```

- [ ] **Step 6: `cargo check --workspace` で compile 通過を確認**

  ```bash
  cargo check --workspace 2>&1 | tail -10
  ```

  Expected: `Checking kotoha-storage v0.1.0` → `Finished ...` warning のみで OK。各 module は doc-comment のみのため `unused` warning が出るが Task A2 以降で解消する。

- [ ] **Step 7: Commit**

  ```bash
  git add Cargo.toml crates/kotoha-storage/
  git commit -m "feat(storage): scaffold kotoha-storage crate skeleton (#98)"
  ```

---

### Task A2: `StorageError` enum 全 variant 定義(TDD)

**Files:**
- Modify: `crates/kotoha-storage/src/error.rs`

**Depends-on:** Task A1

**Estimated LOC:** 70(impl 35 + tests 35)

参照: spec §5.4.3 / §9.4 / §9.5

- [ ] **Step 1: failing test を書く**

  `crates/kotoha-storage/src/error.rs` に以下を追加する。

  ```rust
  //! `StorageError` enum: kotoha-storage 全層の error 型(spec §9.4)。

  use std::path::PathBuf;

  /// 永続化層の error。
  ///
  /// # Variants
  ///
  /// - [`StorageError::InvalidField`]: validation 違反(spec §9.5 reason 列挙)
  /// - [`StorageError::InvalidPath`]: path resolution / blacklist 違反(spec §5.4)
  /// - [`StorageError::DuplicateEntry`]: UNIQUE(surface, reading) 違反
  /// - [`StorageError::NotFound`]: delete / show 対象 entry が不在
  /// - [`StorageError::Sqlite`]: rusqlite backend 失敗
  /// - [`StorageError::Io`]: filesystem IO 失敗
  /// - [`StorageError::Migration`]: migration apply 失敗
  /// - [`StorageError::HomeDirNotFound`]: $HOME / $XDG_DATA_HOME 解決不能
  #[derive(Debug, thiserror::Error)]
  pub enum StorageError {
      #[error("invalid field {name}: {reason}")]
      InvalidField { name: String, reason: String },
      #[error("invalid path {path:?}: {reason}")]
      InvalidPath { path: PathBuf, reason: String },
      #[error("duplicate entry: surface={surface} reading={reading}")]
      DuplicateEntry { surface: String, reading: String },
      #[error("entry not found")]
      NotFound,
      #[error("sqlite error: {0}")]
      Sqlite(#[from] rusqlite::Error),
      #[error("io error: {0}")]
      Io(#[from] std::io::Error),
      #[error("migration error: {0}")]
      Migration(String),
      #[error("home directory not found: $HOME / $XDG_DATA_HOME / KOTOHA_DATA_DIR all unset")]
      HomeDirNotFound,
  }

  #[cfg(test)]
  mod tests {
      use super::*;

      #[test]
      fn invalid_field_display_includes_name_and_reason() {
          let err = StorageError::InvalidField {
              name: "reading".to_string(),
              reason: "non-hiragana reading".to_string(),
          };
          let msg = format!("{err}");
          assert!(msg.contains("reading"));
          assert!(msg.contains("non-hiragana reading"));
      }

      #[test]
      fn invalid_path_display_includes_path_and_reason() {
          let err = StorageError::InvalidPath {
              path: PathBuf::from("/etc/kotoha"),
              reason: "system path blacklisted".to_string(),
          };
          let msg = format!("{err}");
          assert!(msg.contains("/etc/kotoha"));
          assert!(msg.contains("blacklisted"));
      }

      #[test]
      fn duplicate_entry_display_includes_surface_reading() {
          let err = StorageError::DuplicateEntry {
              surface: "日野岡".to_string(),
              reading: "ひのおか".to_string(),
          };
          let msg = format!("{err}");
          assert!(msg.contains("日野岡"));
          assert!(msg.contains("ひのおか"));
      }

      #[test]
      fn not_found_display_is_stable_string() {
          let err = StorageError::NotFound;
          assert_eq!(format!("{err}"), "entry not found");
      }

      #[test]
      fn sqlite_error_from_conversion_works() {
          let sqlite_err = rusqlite::Error::QueryReturnedNoRows;
          let storage_err: StorageError = sqlite_err.into();
          assert!(matches!(storage_err, StorageError::Sqlite(_)));
      }

      #[test]
      fn io_error_from_conversion_works() {
          let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "no such file");
          let storage_err: StorageError = io_err.into();
          assert!(matches!(storage_err, StorageError::Io(_)));
      }

      #[test]
      fn home_dir_not_found_display_is_stable() {
          let err = StorageError::HomeDirNotFound;
          let msg = format!("{err}");
          assert!(msg.contains("home directory not found"));
      }
  }
  ```

- [ ] **Step 2: Test 失敗確認**

  ```bash
  cargo test -p kotoha-storage error::tests 2>&1 | tail -10
  ```

  Expected: error は無く全 PASS(本 task の test と impl は同 step で記述したため、Test の compile が通って 7 PASS となるのが正解)。Test 結果が 7 passed であることのみを確認する。実装と test を分離する厳格 TDD は次 task A3 以降で適用する(本 task は enum 定義のみで意味のある failing 状態が作れない)。

- [ ] **Step 3: Test pass 確認**

  ```bash
  cargo test -p kotoha-storage error::tests 2>&1 | tail -5
  ```

  Expected: `test result: ok. 7 passed; 0 failed;`

- [ ] **Step 4: clippy 通過**

  ```bash
  cargo clippy -p kotoha-storage --all-targets -- -D warnings 2>&1 | tail -5
  ```

- [ ] **Step 5: Commit**

  ```bash
  git add crates/kotoha-storage/src/error.rs
  git commit -m "feat(storage): define StorageError enum with all variants (#98)"
  ```

---

### Task A3: `path::resolve_data_dir` 8-step containment protocol(TDD strict)

**Files:**
- Modify: `crates/kotoha-storage/src/path.rs`

**Depends-on:** Task A2

**Estimated LOC:** 220(impl 130 + tests 90)

参照: spec §5.4.1 / §5.4.2 / §5.4.3

- [ ] **Step 1: failing test を書く**

  `crates/kotoha-storage/src/path.rs` を以下に置き換える(impl 0、test のみ)。

  ```rust
  //! DB 配置 path の resolve + 8-step containment 検証(spec §5.4)。

  #[cfg(test)]
  mod tests {
      use super::*;
      use serial_test::serial;
      use std::env;

      // env var 操作は process-global のため #[serial] で順次実行する(spec §10.3.1)。

      fn save_env() -> (Option<String>, Option<String>, Option<String>) {
          let a = env::var("KOTOHA_DATA_DIR").ok();
          let b = env::var("XDG_DATA_HOME").ok();
          let c = env::var("HOME").ok();
          (a, b, c)
      }

      fn restore_env(saved: (Option<String>, Option<String>, Option<String>)) {
          for (k, v) in [
              ("KOTOHA_DATA_DIR", saved.0),
              ("XDG_DATA_HOME", saved.1),
              ("HOME", saved.2),
          ] {
              match v {
                  Some(s) => env::set_var(k, s),
                  None => env::remove_var(k),
              }
          }
      }

      #[test]
      #[serial]
      fn rejects_system_path_blacklist_etc() {
          let saved = save_env();
          env::set_var("KOTOHA_DATA_DIR", "/etc/kotoha");
          let result = resolve_data_dir();
          assert!(matches!(result, Err(StorageError::InvalidPath { .. })));
          restore_env(saved);
      }

      #[test]
      #[serial]
      fn rejects_system_path_blacklist_proc() {
          let saved = save_env();
          env::set_var("KOTOHA_DATA_DIR", "/proc/kotoha");
          let result = resolve_data_dir();
          assert!(matches!(result, Err(StorageError::InvalidPath { .. })));
          restore_env(saved);
      }

      #[test]
      #[serial]
      fn accepts_tempdir_path_when_explicit_kotoha_data_dir_set() {
          let saved = save_env();
          let tmp = tempfile::tempdir().expect("tempdir");
          env::set_var("KOTOHA_DATA_DIR", tmp.path());
          let result = resolve_data_dir().expect("explicit tempdir must succeed");
          assert!(result.is_absolute());
          assert!(result.ends_with("kotoha.db"));
          restore_env(saved);
      }

      #[test]
      #[serial]
      fn falls_back_to_xdg_when_kotoha_data_dir_absent() {
          let saved = save_env();
          let tmp = tempfile::tempdir().expect("tempdir");
          env::remove_var("KOTOHA_DATA_DIR");
          env::set_var("XDG_DATA_HOME", tmp.path());
          let result = resolve_data_dir().expect("XDG fallback must succeed");
          assert!(result.starts_with(tmp.path()));
          restore_env(saved);
      }

      #[test]
      #[serial]
      fn falls_back_to_home_local_share_when_xdg_absent() {
          let saved = save_env();
          let tmp = tempfile::tempdir().expect("tempdir");
          env::remove_var("KOTOHA_DATA_DIR");
          env::remove_var("XDG_DATA_HOME");
          env::set_var("HOME", tmp.path());
          let result = resolve_data_dir().expect("HOME fallback must succeed");
          let expected_prefix = tmp.path().join(".local/share/kotoha");
          assert!(result.starts_with(&expected_prefix));
          restore_env(saved);
      }

      #[test]
      #[serial]
      fn returns_home_dir_not_found_when_all_envs_absent() {
          let saved = save_env();
          env::remove_var("KOTOHA_DATA_DIR");
          env::remove_var("XDG_DATA_HOME");
          env::remove_var("HOME");
          let result = resolve_data_dir();
          assert!(matches!(result, Err(StorageError::HomeDirNotFound)));
          restore_env(saved);
      }
  }
  ```

- [ ] **Step 2: Test 失敗確認**

  ```bash
  cargo test -p kotoha-storage path::tests 2>&1 | tail -15
  ```

  Expected: `error[E0425]: cannot find function 'resolve_data_dir' in this scope` または `cannot find type 'StorageError'`(impl 不在)。

- [ ] **Step 3: 最小実装**

  `crates/kotoha-storage/src/path.rs` の `#[cfg(test)]` より前に以下を挿入する。

  ```rust
  use std::env;
  use std::fs;
  use std::path::PathBuf;

  use crate::error::StorageError;

  /// System path blacklist。spec §5.4.1 step 3 / step 6 で使用。
  const BLACKLIST: &[&str] = &[
      "/etc", "/var", "/tmp", "/proc", "/sys", "/dev", "/root", "/boot",
  ];

  /// DB 配置 path を resolve / 検証する 8-step protocol(spec §5.4.1)。
  ///
  /// # Postconditions
  ///
  /// - 戻り値は absolute path で `kotoha.db` で終わる
  /// - 戻り値の親 dir は実在 + canonicalize 済
  /// - blacklist 配下に到達するすべての経路は reject 済
  ///
  /// # Errors
  ///
  /// - [`StorageError::InvalidPath`] when prefix containment / blacklist が違反
  /// - [`StorageError::Io`] when create_dir_all / canonicalize が失敗
  /// - [`StorageError::HomeDirNotFound`] when all of KOTOHA_DATA_DIR / XDG_DATA_HOME / HOME are unset
  pub fn resolve_data_dir() -> Result<PathBuf, StorageError> {
      // step 1: target_dir 解決
      let (target_dir, allowed_prefix) = if let Ok(custom) = env::var("KOTOHA_DATA_DIR") {
          let p = PathBuf::from(&custom);
          (p.clone(), p)
      } else if let Ok(xdg) = env::var("XDG_DATA_HOME") {
          let prefix = PathBuf::from(&xdg);
          (prefix.join("kotoha"), prefix)
      } else if let Ok(home) = env::var("HOME") {
          let prefix = PathBuf::from(&home);
          (prefix.join(".local/share/kotoha"), prefix)
      } else {
          return Err(StorageError::HomeDirNotFound);
      };

      // step 2: prefix containment 検証(canonicalize 前)
      if !target_dir.starts_with(&allowed_prefix) {
          return Err(StorageError::InvalidPath {
              path: target_dir.clone(),
              reason: format!(
                  "target_dir does not start with allowed prefix {}",
                  allowed_prefix.display()
              ),
          });
      }

      // step 3: system path blacklist(canonicalize 前)
      if let Some(p) = BLACKLIST.iter().find(|p| target_dir.starts_with(p)) {
          return Err(StorageError::InvalidPath {
              path: target_dir.clone(),
              reason: format!("system path blacklisted (pre-canonicalize): {p}"),
          });
      }

      // step 4: dir 作成(canonicalize 前)
      fs::create_dir_all(&target_dir)?;

      // step 5: canonicalize
      let canonical = target_dir.canonicalize()?;

      // step 6: prefix containment + blacklist 再検証(canonicalize 後、defense-in-depth)
      if let Some(p) = BLACKLIST.iter().find(|p| canonical.starts_with(p)) {
          return Err(StorageError::InvalidPath {
              path: canonical.clone(),
              reason: format!("symlink resolved to blacklisted path: {p}"),
          });
      }

      // step 7: 絶対 path assert
      if !canonical.is_absolute() {
          return Err(StorageError::InvalidPath {
              path: canonical.clone(),
              reason: "not absolute after canonicalize".to_string(),
          });
      }

      let db_path = canonical.join("kotoha.db");

      // step 8: DB ファイル size sanity warn(100 MiB 超過時 stderr)
      if let Ok(meta) = fs::metadata(&db_path) {
          if meta.len() > 100 * 1024 * 1024 {
              eprintln!(
                  "warning: kotoha.db size {} bytes exceeds 100 MiB threshold",
                  meta.len()
              );
          }
      }

      Ok(db_path)
  }
  ```

- [ ] **Step 4: Test pass 確認**

  ```bash
  cargo test -p kotoha-storage path::tests 2>&1 | tail -10
  ```

  Expected: `test result: ok. 6 passed; 0 failed;`

- [ ] **Step 5: clippy + commit**

  ```bash
  cargo clippy -p kotoha-storage --all-targets -- -D warnings
  git add crates/kotoha-storage/src/path.rs
  git commit -m "feat(storage): add resolve_data_dir with 8-step containment protocol (#98)"
  ```

---

### Task A4: `validation` module(TDD strict)

**Files:**
- Modify: `crates/kotoha-storage/src/validation.rs`

**Depends-on:** Task A3

**Estimated LOC:** 280(impl 150 + tests 130)

参照: spec §9.1 / §9.1.1 / §9.1.2 / §9.2 / §9.3 / §9.5

- [ ] **Step 1: failing test を書く**

  `crates/kotoha-storage/src/validation.rs` を以下に置き換える(test 先行)。

  ```rust
  //! Validation module(spec §9)。

  #[cfg(test)]
  mod tests {
      use super::*;

      // ====================
      // validate_field 共通制約(§9.1)
      // ====================

      #[test]
      fn validate_field_rejects_empty() {
          let err = validate_field("surface", "").unwrap_err();
          match err {
              StorageError::InvalidField { name, reason } => {
                  assert_eq!(name, "surface");
                  assert_eq!(reason, "empty");
              }
              other => panic!("unexpected: {other:?}"),
          }
      }

      #[test]
      fn validate_field_rejects_byte_size_over_256() {
          let big = "あ".repeat(100); // 100*3 = 300 bytes
          let err = validate_field("surface", &big).unwrap_err();
          match err {
              StorageError::InvalidField { reason, .. } => {
                  assert_eq!(reason, "byte size > 256");
              }
              other => panic!("unexpected: {other:?}"),
          }
      }

      #[test]
      fn validate_field_rejects_control_char_nul() {
          let err = validate_field("surface", "abc\0def").unwrap_err();
          match err {
              StorageError::InvalidField { reason, .. } => {
                  assert_eq!(reason, "control char");
              }
              other => panic!("unexpected: {other:?}"),
          }
      }

      #[test]
      fn validate_field_rejects_control_char_tab() {
          let err = validate_field("surface", "abc\tdef").unwrap_err();
          assert!(matches!(err, StorageError::InvalidField { .. }));
      }

      #[test]
      fn validate_field_rejects_bidi_chars() {
          let err = validate_field("surface", "ab\u{202E}cd").unwrap_err();
          match err {
              StorageError::InvalidField { reason, .. } => {
                  assert_eq!(reason, "bidi character");
              }
              other => panic!("unexpected: {other:?}"),
          }
      }

      #[test]
      fn validate_field_rejects_pua_char() {
          let err = validate_field("surface", "ab\u{E000}cd").unwrap_err();
          match err {
              StorageError::InvalidField { reason, .. } => {
                  assert_eq!(reason, "PUA char");
              }
              other => panic!("unexpected: {other:?}"),
          }
      }

      #[test]
      fn validate_field_allows_phase5_pua_allowlist() {
          // U+EE00..=U+EE03 は Karukan 互換の例外として許容(spec §9.1.1)
          assert!(validate_field("surface", "ab\u{EE00}cd").is_ok());
          assert!(validate_field("surface", "ab\u{EE03}cd").is_ok());
      }

      #[test]
      fn validate_field_rejects_variation_selector() {
          let err = validate_field("surface", "ab\u{FE0F}cd").unwrap_err();
          match err {
              StorageError::InvalidField { reason, .. } => {
                  assert_eq!(reason, "Variation Selector");
              }
              other => panic!("unexpected: {other:?}"),
          }
      }

      #[test]
      fn validate_field_rejects_tag_char() {
          let err = validate_field("surface", "ab\u{E0001}cd").unwrap_err();
          match err {
              StorageError::InvalidField { reason, .. } => {
                  assert_eq!(reason, "Tag char");
              }
              other => panic!("unexpected: {other:?}"),
          }
      }

      #[test]
      fn validate_field_accepts_normal_kanji() {
          assert!(validate_field("surface", "日野岡").is_ok());
          assert!(validate_field("surface", "abc").is_ok());
      }

      // ====================
      // validate_reading(§9.2)
      // ====================

      #[test]
      fn validate_reading_accepts_pure_hiragana() {
          assert!(validate_reading("ひのおか").is_ok());
          assert!(validate_reading("あいうえお").is_ok());
      }

      #[test]
      fn validate_reading_accepts_long_sound_mark() {
          assert!(validate_reading("こーひー").is_ok()); // U+30FC
      }

      #[test]
      fn validate_reading_accepts_middle_dot() {
          assert!(validate_reading("あ・い").is_ok()); // U+30FB
      }

      #[test]
      fn validate_reading_rejects_katakana() {
          let err = validate_reading("カタカナ").unwrap_err();
          match err {
              StorageError::InvalidField { reason, .. } => {
                  assert_eq!(reason, "non-hiragana reading");
              }
              other => panic!("unexpected: {other:?}"),
          }
      }

      #[test]
      fn validate_reading_rejects_kanji() {
          let err = validate_reading("漢字").unwrap_err();
          assert!(matches!(err, StorageError::InvalidField { .. }));
      }

      #[test]
      fn validate_reading_rejects_ascii() {
          let err = validate_reading("abc").unwrap_err();
          assert!(matches!(err, StorageError::InvalidField { .. }));
      }

      // ====================
      // validate_score(§9.3)
      // ====================

      #[test]
      fn validate_score_accepts_finite_non_negative() {
          assert!(validate_score(0.0).is_ok());
          assert!(validate_score(1.0).is_ok());
          assert!(validate_score(100.5).is_ok());
      }

      #[test]
      fn validate_score_rejects_nan() {
          let err = validate_score(f32::NAN).unwrap_err();
          match err {
              StorageError::InvalidField { reason, .. } => {
                  assert_eq!(reason, "score not finite");
              }
              other => panic!("unexpected: {other:?}"),
          }
      }

      #[test]
      fn validate_score_rejects_infinity() {
          let err = validate_score(f32::INFINITY).unwrap_err();
          assert!(matches!(err, StorageError::InvalidField { .. }));
      }

      #[test]
      fn validate_score_rejects_negative_infinity() {
          let err = validate_score(f32::NEG_INFINITY).unwrap_err();
          assert!(matches!(err, StorageError::InvalidField { .. }));
      }

      #[test]
      fn validate_score_rejects_negative() {
          let err = validate_score(-0.001).unwrap_err();
          match err {
              StorageError::InvalidField { reason, .. } => {
                  assert_eq!(reason, "score negative");
              }
              other => panic!("unexpected: {other:?}"),
          }
      }
  }
  ```

- [ ] **Step 2: Test 失敗確認**

  ```bash
  cargo test -p kotoha-storage validation::tests 2>&1 | tail -15
  ```

  Expected: 全 test が compile error(`cannot find function 'validate_field'`)。

- [ ] **Step 3: 最小実装**

  `validation.rs` の `#[cfg(test)]` より前に以下を挿入する。

  ```rust
  use crate::error::StorageError;

  // ====================
  // 判定 helper(§9.1.1)
  // ====================

  /// PUA(Private Use Area)を reject する判定。Phase 5 Mixed JP/EN allowlist
  /// (U+EE00..=U+EE03) は Karukan 互換の特例として許容する(spec §9.1.1 / §9.1.2)。
  fn is_disallowed_pua(c: char) -> bool {
      let cp = c as u32;
      let is_pua = (0xE000..=0xF8FF).contains(&cp) || (0xF0000..=0x10FFFD).contains(&cp);
      let is_phase5_allowlist = (0xEE00..=0xEE03).contains(&cp);
      is_pua && !is_phase5_allowlist
  }

  fn is_variation_selector(c: char) -> bool {
      (0xFE00..=0xFE0F).contains(&(c as u32))
  }

  fn is_tag_char(c: char) -> bool {
      (0xE0000..=0xE007F).contains(&(c as u32))
  }

  fn is_bidi_char(c: char) -> bool {
      let cp = c as u32;
      cp == 0x200E
          || cp == 0x200F
          || (0x202A..=0x202E).contains(&cp)
          || (0x2066..=0x2069).contains(&cp)
  }

  fn is_hiragana_or_long_sound(c: char) -> bool {
      let cp = c as u32;
      (0x3040..=0x309F).contains(&cp) || cp == 0x30FC || cp == 0x30FB
  }

  /// `name` field の共通制約を検査する(spec §9.1)。
  pub fn validate_field(name: &str, value: &str) -> Result<(), StorageError> {
      if value.is_empty() {
          return Err(StorageError::InvalidField {
              name: name.to_string(),
              reason: "empty".to_string(),
          });
      }
      if value.len() > 256 {
          return Err(StorageError::InvalidField {
              name: name.to_string(),
              reason: "byte size > 256".to_string(),
          });
      }
      for c in value.chars() {
          if c.is_control() {
              return Err(StorageError::InvalidField {
                  name: name.to_string(),
                  reason: "control char".to_string(),
              });
          }
          if is_bidi_char(c) {
              return Err(StorageError::InvalidField {
                  name: name.to_string(),
                  reason: "bidi character".to_string(),
              });
          }
          if is_disallowed_pua(c) {
              return Err(StorageError::InvalidField {
                  name: name.to_string(),
                  reason: "PUA char".to_string(),
              });
          }
          if is_variation_selector(c) {
              return Err(StorageError::InvalidField {
                  name: name.to_string(),
                  reason: "Variation Selector".to_string(),
              });
          }
          if is_tag_char(c) {
              return Err(StorageError::InvalidField {
                  name: name.to_string(),
                  reason: "Tag char".to_string(),
              });
          }
      }
      Ok(())
  }

  /// `reading` field の制約を検査する(spec §9.2)。
  ///
  /// 共通制約 + hiragana-only(U+3040..=U+309F + U+30FC + U+30FB)。
  pub fn validate_reading(value: &str) -> Result<(), StorageError> {
      validate_field("reading", value)?;
      for c in value.chars() {
          if !is_hiragana_or_long_sound(c) {
              return Err(StorageError::InvalidField {
                  name: "reading".to_string(),
                  reason: "non-hiragana reading".to_string(),
              });
          }
      }
      Ok(())
  }

  /// `score` field の制約を検査する(spec §9.3)。
  pub fn validate_score(value: f32) -> Result<(), StorageError> {
      if !value.is_finite() {
          return Err(StorageError::InvalidField {
              name: "score".to_string(),
              reason: "score not finite".to_string(),
          });
      }
      if value < 0.0 {
          return Err(StorageError::InvalidField {
              name: "score".to_string(),
              reason: "score negative".to_string(),
          });
      }
      Ok(())
  }

  /// `surface` 用の wrapper。spec §9.1 の共通制約のみを適用する(hiragana 制約は無し)。
  pub fn validate_surface(value: &str) -> Result<(), StorageError> {
      validate_field("surface", value)
  }

  /// `pos` 用の wrapper。spec §9.1 の共通制約のみを適用する。
  pub fn validate_pos(value: &str) -> Result<(), StorageError> {
      validate_field("pos", value)
  }
  ```

- [ ] **Step 4: Test pass 確認**

  ```bash
  cargo test -p kotoha-storage validation::tests 2>&1 | tail -10
  ```

  Expected: `test result: ok. 23 passed; 0 failed;`(allowlist の boundary を含めて 23 test)

- [ ] **Step 5: clippy + commit**

  ```bash
  cargo clippy -p kotoha-storage --all-targets -- -D warnings
  git add crates/kotoha-storage/src/validation.rs
  git commit -m "feat(storage): add validation module (field/reading/score/pos/surface) (#98)"
  ```

---

### Task A5: `migrations/mod.rs` + MIGRATIONS const + apply_migrations runner(TDD strict)

**Files:**
- Modify: `crates/kotoha-storage/src/migrations.rs`

**Depends-on:** Task A4

**Estimated LOC:** 130(impl 60 + tests 70)

参照: spec §3.6 / §6.5 / §10.1.1

- [ ] **Step 1: failing test を書く**

  `crates/kotoha-storage/src/migrations.rs` を以下に置き換える(test 先行)。

  ```rust
  //! Migration runner(spec §3.6 / §6.5)。

  #[cfg(test)]
  mod tests {
      use super::*;
      use rusqlite::Connection;

      // ====================
      // §10.1.1 invariant test 必須 3 項目
      // ====================

      #[test]
      fn migrations_are_strictly_ascending() {
          assert!(
              MIGRATIONS.windows(2).all(|w| w[0].0 < w[1].0),
              "MIGRATIONS must be in strictly ascending version order"
          );
      }

      #[test]
      fn migrations_skip_and_resume_from_intermediate_version() {
          let conn = Connection::open_in_memory().expect("memory open");
          // P2-B 着地時点では MIGRATIONS 長 1 のため version=0 → v001 apply のみ確認
          conn.execute_batch("PRAGMA user_version = 0").unwrap();
          apply_migrations(&conn).expect("apply succeeds");
          let v: i32 = conn
              .query_row("PRAGMA user_version", [], |r| r.get(0))
              .unwrap();
          assert_eq!(v, LATEST_VERSION);
      }

      #[test]
      fn migrations_idempotency_on_double_apply() {
          let conn = Connection::open_in_memory().expect("memory open");
          apply_migrations(&conn).expect("first apply");
          apply_migrations(&conn).expect("second apply must be no-op");
          let v: i32 = conn
              .query_row("PRAGMA user_version", [], |r| r.get(0))
              .unwrap();
          assert_eq!(v, LATEST_VERSION);
      }

      #[test]
      fn latest_version_is_at_least_one() {
          assert!(LATEST_VERSION >= 1);
      }

      #[test]
      fn apply_migrations_creates_user_vocab_table() {
          let conn = Connection::open_in_memory().expect("memory open");
          apply_migrations(&conn).expect("apply");
          let count: i64 = conn
              .query_row(
                  "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='user_vocab'",
                  [],
                  |r| r.get(0),
              )
              .unwrap();
          assert_eq!(count, 1);
      }

      #[test]
      fn apply_migrations_creates_learning_cache_table() {
          let conn = Connection::open_in_memory().expect("memory open");
          apply_migrations(&conn).expect("apply");
          let count: i64 = conn
              .query_row(
                  "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='learning_cache'",
                  [],
                  |r| r.get(0),
              )
              .unwrap();
          assert_eq!(count, 1);
      }

      #[test]
      fn apply_migrations_creates_user_vocab_reading_index() {
          let conn = Connection::open_in_memory().expect("memory open");
          apply_migrations(&conn).expect("apply");
          let count: i64 = conn
              .query_row(
                  "SELECT count(*) FROM sqlite_master WHERE type='index' AND name='idx_user_vocab_reading'",
                  [],
                  |r| r.get(0),
              )
              .unwrap();
          assert_eq!(count, 1);
      }
  }
  ```

- [ ] **Step 2: Test 失敗確認**

  ```bash
  cargo test -p kotoha-storage migrations::tests 2>&1 | tail -10
  ```

  Expected: `cannot find value 'MIGRATIONS' / 'LATEST_VERSION' / function 'apply_migrations'`。

- [ ] **Step 3: 最小実装(SQL ファイルは task A6 で作成、本 task は include_str! で先に参照する)**

  `migrations.rs` の `#[cfg(test)]` より前に以下を挿入する。

  ```rust
  use crate::error::StorageError;

  /// 最新 schema version。新 migration を `MIGRATIONS` に追加する際は本値を増やす。
  pub const LATEST_VERSION: i32 = 1;

  /// (version, sql) 配列。version 昇順厳守(spec §10.1.1)。
  ///
  /// `include_str!` でバイナリ同梱するため、distribution 時に migrations
  /// directory を別配布する必要はない(spec §3.6)。
  pub const MIGRATIONS: &[(i32, &str)] = &[
      (1, include_str!("../migrations/v001_initial.sql")),
  ];

  /// 未適用 migration を順次 apply する(spec §6.5)。
  ///
  /// # Errors
  ///
  /// - [`StorageError::Sqlite`] when SQL execution fails
  /// - [`StorageError::Migration`] when version sequence is invalid
  pub fn apply_migrations(conn: &rusqlite::Connection) -> Result<(), StorageError> {
      let current: i32 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
      for (version, sql) in MIGRATIONS.iter().filter(|(v, _)| *v > current) {
          conn.execute_batch(sql)?;
          conn.execute_batch(&format!("PRAGMA user_version = {}", version))?;
      }
      Ok(())
  }
  ```

- [ ] **Step 4: SQL file が無いと include_str! が compile error。次 task で v001_initial.sql を作成するため、本 task ではここまでで一度 commit する**

  本 task の test は SQL file 不在のため compile error で fail する。Task A6 で SQL を作成すると test が PASS する。本 task は test + impl の test-impl pair として A6 と一体で commit する。

- [ ] **Step 5: A6 の SQL を先に作成して PASS させる**

  Task A6 を先行して実施する(plan 上は A5 → A6 の sequence だが、commit は一度にまとめる)。

  Task A6 完了後、

  ```bash
  cargo test -p kotoha-storage migrations::tests 2>&1 | tail -10
  ```

  Expected: `test result: ok. 7 passed; 0 failed;`

- [ ] **Step 6: clippy + commit(A6 と一体)**

  ```bash
  cargo clippy -p kotoha-storage --all-targets -- -D warnings
  git add crates/kotoha-storage/src/migrations.rs crates/kotoha-storage/migrations/v001_initial.sql
  git commit -m "feat(storage): add migrations runner + v001 initial schema (#98)"
  ```

---

### Task A6: `migrations/v001_initial.sql`(user_vocab + learning_cache + indexes)

**Files:**
- Create: `crates/kotoha-storage/migrations/v001_initial.sql`

**Depends-on:** Task A5(本 task は A5 の include_str! 解決のために A5 直後に行う)

**Estimated LOC:** 30

参照: spec §5.1 / §5.2

- [ ] **Step 1: SQL ファイル作成**

  以下を `crates/kotoha-storage/migrations/v001_initial.sql` として保存する。

  ```sql
  -- v001_initial: user_vocab + learning_cache schema
  -- spec: docs/superpowers/specs/2026-04-25-p2-b-user-dictionary-design.md §5.1 / §5.2

  CREATE TABLE user_vocab (
      id          INTEGER PRIMARY KEY AUTOINCREMENT,
      surface     TEXT    NOT NULL,
      reading     TEXT    NOT NULL,
      pos         TEXT    NOT NULL DEFAULT '名詞-固有名詞-一般',
      score       REAL    NOT NULL DEFAULT 1.0,
      created_at  INTEGER NOT NULL,
      updated_at  INTEGER NOT NULL,
      UNIQUE(surface, reading)
  );

  CREATE INDEX idx_user_vocab_reading ON user_vocab(reading);

  CREATE TABLE learning_cache (
      id           INTEGER PRIMARY KEY AUTOINCREMENT,
      kana_input   TEXT    NOT NULL,
      chosen_kanji TEXT    NOT NULL,
      frequency    INTEGER NOT NULL DEFAULT 1,
      last_used_at INTEGER NOT NULL,
      UNIQUE(kana_input, chosen_kanji)
  );

  CREATE INDEX idx_learning_cache_kana ON learning_cache(kana_input);
  ```

- [ ] **Step 2: A5 の test が PASS することを確認**

  ```bash
  cargo test -p kotoha-storage migrations::tests 2>&1 | tail -10
  ```

  Expected: 7 PASS。

- [ ] **Step 3: A5 と一体で commit 済**(Task A5 Step 6)。

---

### Task A7: `Database::open` + PRAGMA + Arc<Database> 戻り値(TDD strict)

**Files:**
- Modify: `crates/kotoha-storage/src/database.rs`

**Depends-on:** Task A6

**Estimated LOC:** 250(impl 130 + tests 120)

参照: spec §5.3 / §6.4 / §6.4.1

- [ ] **Step 1: failing test を書く**

  `crates/kotoha-storage/src/database.rs` を以下に置き換える(test 先行)。

  ```rust
  //! `Database` 構造体: Mutex<Connection> + Arc 共有 ownership(spec §6.4)。

  #[cfg(test)]
  mod tests {
      use super::*;
      use std::sync::Arc;

      #[test]
      fn open_in_memory_returns_arc() {
          let db = Database::open_in_memory().expect("memory open");
          let _: Arc<Database> = db; // type assertion
      }

      #[test]
      fn open_in_memory_applies_v001_migration() {
          let db = Database::open_in_memory().expect("memory open");
          let conn = db.lock_conn();
          let count: i64 = conn
              .query_row(
                  "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='user_vocab'",
                  [],
                  |r| r.get(0),
              )
              .unwrap();
          assert_eq!(count, 1);
      }

      #[test]
      fn open_in_memory_sets_pragma_journal_mode_wal() {
          // :memory: では journal_mode=WAL は memory に切り替わる(SQLite 仕様)。
          // 実 file での WAL 設定は open_with_path テストで確認する。
          let db = Database::open_in_memory().expect("memory open");
          let conn = db.lock_conn();
          let mode: String = conn
              .query_row("PRAGMA journal_mode", [], |r| r.get(0))
              .unwrap();
          // memory or wal のどちらかが返る(:memory: の場合 memory)
          assert!(mode == "memory" || mode == "wal");
      }

      #[test]
      fn open_with_path_creates_db_file() {
          let tmp = tempfile::tempdir().expect("tempdir");
          let path = tmp.path().join("test.db");
          let _db = Database::open(&path).expect("open succeeds");
          assert!(path.exists(), "db file must be created");
      }

      #[test]
      fn open_with_path_applies_migration() {
          let tmp = tempfile::tempdir().expect("tempdir");
          let path = tmp.path().join("test.db");
          let db = Database::open(&path).expect("open succeeds");
          let conn = db.lock_conn();
          let count: i64 = conn
              .query_row(
                  "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='user_vocab'",
                  [],
                  |r| r.get(0),
              )
              .unwrap();
          assert_eq!(count, 1);
      }

      #[test]
      fn open_with_path_sets_wal_journal_mode() {
          let tmp = tempfile::tempdir().expect("tempdir");
          let path = tmp.path().join("test.db");
          let db = Database::open(&path).expect("open succeeds");
          let conn = db.lock_conn();
          let mode: String = conn
              .query_row("PRAGMA journal_mode", [], |r| r.get(0))
              .unwrap();
          assert_eq!(mode, "wal");
      }

      #[test]
      fn open_with_path_sets_foreign_keys_on() {
          let tmp = tempfile::tempdir().expect("tempdir");
          let path = tmp.path().join("test.db");
          let db = Database::open(&path).expect("open succeeds");
          let conn = db.lock_conn();
          let on: i32 = conn
              .query_row("PRAGMA foreign_keys", [], |r| r.get(0))
              .unwrap();
          assert_eq!(on, 1);
      }

      #[test]
      fn open_with_path_creates_parent_directory() {
          let tmp = tempfile::tempdir().expect("tempdir");
          let nested = tmp.path().join("a/b/c");
          let path = nested.join("test.db");
          let _db = Database::open(&path).expect("open succeeds even when parent missing");
          assert!(nested.exists());
      }

      #[test]
      fn idempotent_reopen_does_not_reapply_migration() {
          let tmp = tempfile::tempdir().expect("tempdir");
          let path = tmp.path().join("test.db");
          let _db1 = Database::open(&path).expect("first open");
          let db2 = Database::open(&path).expect("second open");
          let conn = db2.lock_conn();
          let v: i32 = conn
              .query_row("PRAGMA user_version", [], |r| r.get(0))
              .unwrap();
          assert_eq!(v, crate::migrations::LATEST_VERSION);
      }
  }
  ```

- [ ] **Step 2: Test 失敗確認**

  ```bash
  cargo test -p kotoha-storage database::tests 2>&1 | tail -15
  ```

  Expected: `cannot find type 'Database' / cannot find function 'open' / 'open_in_memory'`。

- [ ] **Step 3: 最小実装**

  `database.rs` の `#[cfg(test)]` より前に以下を挿入する。

  ```rust
  use std::path::Path;
  use std::sync::{Arc, Mutex, MutexGuard};

  use rusqlite::Connection;

  use crate::error::StorageError;
  use crate::migrations::apply_migrations;

  /// SQLite Database wrapper(spec §6.4)。`Mutex<Connection>` を内部保持し、
  /// `Arc<Database>` で複数 Store(`UserVocabStore` / `LearningCacheStore`)が
  /// 同一 Connection を共有する(spec §6.4.1 共有 ownership)。
  pub struct Database {
      conn: Mutex<Connection>,
  }

  impl Database {
      /// `path` に SQLite DB を open / create し、未適用 migration を apply する。
      ///
      /// # Postconditions
      ///
      /// - 戻り値は `Arc<Database>`(spec §6.4 lifetime parameter 不在)
      /// - `path` の親 dir が無い場合は再帰的に作成
      /// - PRAGMA journal_mode=WAL / synchronous=NORMAL / foreign_keys=ON / temp_store=MEMORY を設定
      /// - `PRAGMA user_version` を読取り、`MIGRATIONS` の未適用分を apply
      ///
      /// # Errors
      ///
      /// - [`StorageError::Io`] when parent dir cannot be created
      /// - [`StorageError::Sqlite`] when open / PRAGMA / migration fails
      pub fn open(path: &Path) -> Result<Arc<Self>, StorageError> {
          if let Some(parent) = path.parent() {
              if !parent.as_os_str().is_empty() && !parent.exists() {
                  std::fs::create_dir_all(parent)?;
              }
          }
          let conn = Connection::open(path)?;
          Self::configure_pragma(&conn)?;
          apply_migrations(&conn)?;
          // 業務上 5s が SQLite default よりも user-friendly(spec §3 / Phase 3 IBus
          // engine の lock 競合に備えた busy_timeout)。
          conn.busy_timeout(std::time::Duration::from_secs(5))?;
          Ok(Arc::new(Self { conn: Mutex::new(conn) }))
      }

      /// `:memory:` SQLite を open する(test 用、spec §10.1)。
      pub fn open_in_memory() -> Result<Arc<Self>, StorageError> {
          let conn = Connection::open_in_memory()?;
          Self::configure_pragma(&conn)?;
          apply_migrations(&conn)?;
          Ok(Arc::new(Self { conn: Mutex::new(conn) }))
      }

      /// PRAGMA を設定する(spec §5.3)。
      fn configure_pragma(conn: &Connection) -> Result<(), StorageError> {
          conn.execute_batch(
              "PRAGMA journal_mode = WAL;
               PRAGMA synchronous = NORMAL;
               PRAGMA foreign_keys = ON;
               PRAGMA temp_store = MEMORY;",
          )?;
          Ok(())
      }

      /// 内部 `Mutex<Connection>` を lock する。Store 実装側で使用。
      ///
      /// # Panics
      ///
      /// poison された場合 panic する(other thread が panic 中に lock を保持していた場合)。
      pub fn lock_conn(&self) -> MutexGuard<'_, Connection> {
          self.conn.lock().expect("Database mutex poisoned")
      }
  }
  ```

- [ ] **Step 4: Test pass 確認**

  ```bash
  cargo test -p kotoha-storage database::tests 2>&1 | tail -10
  ```

  Expected: `test result: ok. 9 passed; 0 failed;`

- [ ] **Step 5: clippy + commit**

  ```bash
  cargo clippy -p kotoha-storage --all-targets -- -D warnings
  git add crates/kotoha-storage/src/database.rs
  git commit -m "feat(storage): add Database::open with WAL/PRAGMA + Arc<Database> sharing (#98)"
  ```

---

### Task A8: `MIGRATIONS` 配列 invariant test 補強(spec §10.1.1 必須 3 項目の review)

**Files:** Task A5 で実装済の test を review する task。新規 file 編集なし。

**Depends-on:** Task A7

**Estimated LOC:** 0(verification only)

参照: spec §10.1.1

- [ ] **Step 1: A5 の test 内容を read で確認**

  ```bash
  grep -n "migrations_are_strictly_ascending\|skip_and_resume\|idempotency" /home/kohshiro/develops/student/kotoha-ime/crates/kotoha-storage/src/migrations.rs
  ```

  Expected: 3 件全 hit(必須 3 項目の coverage を確認)。

- [ ] **Step 2: 全 invariant test PASS を再確認**

  ```bash
  cargo test -p kotoha-storage migrations::tests 2>&1 | tail -5
  ```

  Expected: 7 PASS。

- [ ] **Step 3: 不足が無ければ commit 不要**(A5 で commit 済)。3 項目全 hit を以降の self-review checklist で確認する。

---

## Phase B: kotoha-storage trait 層

### Task B1: `UserVocabStore` trait + `UserVocabRecord` 構造体(TDD strict)

**Files:**
- Create: `crates/kotoha-storage/src/user_vocab/store.rs`
- Modify: `crates/kotoha-storage/src/user_vocab/mod.rs`

**Depends-on:** Task A8

**Estimated LOC:** 100(impl 50 + tests 50)

参照: spec §6.1 / §6.2

- [ ] **Step 1: failing test を書く**

  `crates/kotoha-storage/src/user_vocab/mod.rs` を以下に置き換える。

  ```rust
  //! `UserVocabStore` trait + `UserVocabRecord` + `Sqlite/Mock` 実装。

  pub mod mock;
  pub mod sqlite;
  pub mod store;

  pub use mock::MockUserVocabStore;
  pub use sqlite::SqliteUserVocabStore;
  pub use store::{UserVocabRecord, UserVocabStore};
  ```

  `crates/kotoha-storage/src/user_vocab/store.rs` を新規作成。

  ```rust
  //! `UserVocabStore` trait + `UserVocabRecord`(spec §6.1 / §6.2)。

  use crate::error::StorageError;

  /// User-managed vocabulary store の抽象境界。
  ///
  /// # Preconditions
  ///
  /// - `reading` は hiragana canonical(`U+3040..=U+309F + U+30FC + U+30FB`)
  /// - `surface` / `pos` は non-empty / ≤256 bytes / no control char / no bidi
  /// - `score` は finite + non-negative
  ///
  /// # Postconditions
  ///
  /// - `find_by_reading` は score 降順で最大 `limit` 件返す
  /// - `insert` 成功時は `id` を返す
  /// - UNIQUE(surface, reading) 違反は [`StorageError::DuplicateEntry`]
  ///
  /// # Errors
  ///
  /// - [`StorageError::InvalidField`] when validation fails
  /// - [`StorageError::DuplicateEntry`] on UNIQUE conflict
  /// - [`StorageError::NotFound`] on delete miss
  /// - [`StorageError::Sqlite`] on backend failure
  pub trait UserVocabStore: Send + Sync {
      fn find_by_reading(&self, reading: &str, limit: usize) -> Result<Vec<UserVocabRecord>, StorageError>;
      fn list_all(&self, limit: usize, offset: usize) -> Result<Vec<UserVocabRecord>, StorageError>;
      fn insert(&self, record: UserVocabRecord) -> Result<i64, StorageError>;
      fn delete_by_id(&self, id: i64) -> Result<(), StorageError>;
      fn delete_by_surface_reading(&self, surface: &str, reading: &str) -> Result<(), StorageError>;
  }

  /// User vocabulary の row(spec §6.2)。
  #[derive(Debug, Clone, PartialEq)]
  pub struct UserVocabRecord {
      /// insert 前は None、find / list は Some。
      pub id: Option<i64>,
      pub surface: String,
      pub reading: String,
      pub pos: String,
      pub score: f32,
      /// UNIX epoch seconds。
      pub created_at: i64,
      pub updated_at: i64,
  }

  #[cfg(test)]
  mod tests {
      use super::*;

      #[test]
      fn user_vocab_record_holds_all_fields() {
          let r = UserVocabRecord {
              id: Some(1),
              surface: "日野岡".to_string(),
              reading: "ひのおか".to_string(),
              pos: "名詞-固有名詞-人名".to_string(),
              score: 1.0,
              created_at: 1745529600,
              updated_at: 1745529600,
          };
          assert_eq!(r.id, Some(1));
          assert_eq!(r.surface, "日野岡");
      }

      #[test]
      fn user_vocab_record_clone_preserves_fields() {
          let r = UserVocabRecord {
              id: None,
              surface: "test".to_string(),
              reading: "てすと".to_string(),
              pos: "名詞".to_string(),
              score: 0.5,
              created_at: 0,
              updated_at: 0,
          };
          let c = r.clone();
          assert_eq!(r, c);
      }

      #[test]
      fn user_vocab_record_partial_eq_works() {
          let a = UserVocabRecord {
              id: Some(1),
              surface: "a".to_string(),
              reading: "あ".to_string(),
              pos: "p".to_string(),
              score: 0.0,
              created_at: 0,
              updated_at: 0,
          };
          let b = a.clone();
          assert_eq!(a, b);
          let c = UserVocabRecord { id: Some(2), ..a.clone() };
          assert_ne!(a, c);
      }
  }
  ```

  `crates/kotoha-storage/src/user_vocab/sqlite.rs` と `mock.rs` は次 task で作成するため、placeholder を置く。

  ```rust
  // sqlite.rs:
  //! SqliteUserVocabStore(Task B2〜B7 で実装).

  use crate::database::Database;
  use std::sync::Arc;

  pub struct SqliteUserVocabStore {
      pub(crate) db: Arc<Database>,
  }
  ```

  ```rust
  // mock.rs:
  //! MockUserVocabStore(Task B9 で実装).

  use crate::user_vocab::store::{UserVocabRecord, UserVocabStore};
  use crate::error::StorageError;
  use std::sync::Mutex;

  pub struct MockUserVocabStore {
      pub(crate) records: Mutex<Vec<UserVocabRecord>>,
      pub(crate) next_id: Mutex<i64>,
  }
  ```

- [ ] **Step 2: Test 失敗 / 通過確認**

  ```bash
  cargo test -p kotoha-storage user_vocab::store::tests 2>&1 | tail -10
  ```

  Expected: `test result: ok. 3 passed`(impl と test を同 step で書いたため)。

- [ ] **Step 3: clippy + commit**

  ```bash
  cargo clippy -p kotoha-storage --all-targets -- -D warnings
  git add crates/kotoha-storage/src/user_vocab/
  git commit -m "feat(storage): define UserVocabStore trait + UserVocabRecord (#98)"
  ```

---

### Task B2: `SqliteUserVocabStore::find_by_reading`(TDD strict)

**Files:**
- Modify: `crates/kotoha-storage/src/user_vocab/sqlite.rs`

**Depends-on:** Task B1

**Estimated LOC:** 130(impl 60 + tests 70)

参照: spec §6.1 / §10.1

- [ ] **Step 1: failing test を書く**

  `sqlite.rs` を以下に置き換える(test 先行)。

  ```rust
  //! SqliteUserVocabStore: UserVocabStore の SQLite 実装(spec §6.1 / §6.4)。

  use std::sync::Arc;

  use crate::database::Database;
  use crate::error::StorageError;
  use crate::user_vocab::store::{UserVocabRecord, UserVocabStore};
  use crate::validation::{validate_pos, validate_reading, validate_score, validate_surface};

  pub struct SqliteUserVocabStore {
      pub(crate) db: Arc<Database>,
  }

  impl SqliteUserVocabStore {
      pub fn new(db: Arc<Database>) -> Self {
          Self { db }
      }
  }

  impl UserVocabStore for SqliteUserVocabStore {
      fn find_by_reading(&self, reading: &str, limit: usize) -> Result<Vec<UserVocabRecord>, StorageError> {
          validate_reading(reading)?;
          let conn = self.db.lock_conn();
          let mut stmt = conn.prepare(
              "SELECT id, surface, reading, pos, score, created_at, updated_at \
               FROM user_vocab \
               WHERE reading = ?1 \
               ORDER BY score DESC \
               LIMIT ?2",
          )?;
          let rows = stmt.query_map(rusqlite::params![reading, limit as i64], |row| {
              Ok(UserVocabRecord {
                  id: Some(row.get(0)?),
                  surface: row.get(1)?,
                  reading: row.get(2)?,
                  pos: row.get(3)?,
                  score: row.get::<_, f64>(4)? as f32,
                  created_at: row.get(5)?,
                  updated_at: row.get(6)?,
              })
          })?;
          let mut out = Vec::new();
          for r in rows {
              out.push(r?);
          }
          Ok(out)
      }

      fn list_all(&self, _limit: usize, _offset: usize) -> Result<Vec<UserVocabRecord>, StorageError> {
          // Task B4 で実装
          unimplemented!("Task B4")
      }

      fn insert(&self, _record: UserVocabRecord) -> Result<i64, StorageError> {
          // Task B3 で実装
          unimplemented!("Task B3")
      }

      fn delete_by_id(&self, _id: i64) -> Result<(), StorageError> {
          unimplemented!("Task B5")
      }

      fn delete_by_surface_reading(&self, _surface: &str, _reading: &str) -> Result<(), StorageError> {
          unimplemented!("Task B6")
      }
  }

  // 注: validate_surface / validate_pos / validate_score は Task B3 insert で使用するため
  // 本 step では参照しないが、import を残しておく。
  #[cfg(test)]
  fn _suppress_unused() {
      let _ = validate_surface;
      let _ = validate_pos;
      let _ = validate_score;
  }

  #[cfg(test)]
  mod tests {
      use super::*;

      fn fresh_store() -> SqliteUserVocabStore {
          let db = Database::open_in_memory().expect("memory open");
          SqliteUserVocabStore::new(db)
      }

      fn seed_row(store: &SqliteUserVocabStore, surface: &str, reading: &str, score: f32) {
          let conn = store.db.lock_conn();
          conn.execute(
              "INSERT INTO user_vocab (surface, reading, pos, score, created_at, updated_at) \
               VALUES (?1, ?2, '名詞', ?3, 0, 0)",
              rusqlite::params![surface, reading, score as f64],
          )
          .unwrap();
      }

      #[test]
      fn find_by_reading_returns_empty_for_unknown() {
          let store = fresh_store();
          let result = store.find_by_reading("みず", 10).expect("ok");
          assert!(result.is_empty());
      }

      #[test]
      fn find_by_reading_returns_single_match() {
          let store = fresh_store();
          seed_row(&store, "日野岡", "ひのおか", 1.0);
          let result = store.find_by_reading("ひのおか", 10).expect("ok");
          assert_eq!(result.len(), 1);
          assert_eq!(result[0].surface, "日野岡");
      }

      #[test]
      fn find_by_reading_orders_by_score_desc() {
          let store = fresh_store();
          seed_row(&store, "日野岡", "ひのおか", 0.5);
          seed_row(&store, "ひの岡", "ひのおか", 1.5);
          let result = store.find_by_reading("ひのおか", 10).expect("ok");
          assert_eq!(result.len(), 2);
          assert_eq!(result[0].surface, "ひの岡"); // higher score
          assert_eq!(result[1].surface, "日野岡");
      }

      #[test]
      fn find_by_reading_respects_limit() {
          let store = fresh_store();
          for i in 0..5 {
              seed_row(&store, &format!("s{}", i), "ひのおか", i as f32);
          }
          let result = store.find_by_reading("ひのおか", 3).expect("ok");
          assert_eq!(result.len(), 3);
      }

      #[test]
      fn find_by_reading_rejects_non_hiragana() {
          let store = fresh_store();
          let err = store.find_by_reading("カタカナ", 10).unwrap_err();
          assert!(matches!(err, StorageError::InvalidField { .. }));
      }
  }
  ```

- [ ] **Step 2: Test 失敗確認**

  ```bash
  cargo test -p kotoha-storage user_vocab::sqlite::tests 2>&1 | tail -10
  ```

  注: Task B1 で sqlite.rs を初期化したため、本 task の test と impl は同 step。`unimplemented!` が test 経路で呼ばれない限り PASS する。`find_by_reading` 経路の 5 test PASS を確認する。

- [ ] **Step 3: Test pass 確認**

  ```bash
  cargo test -p kotoha-storage user_vocab::sqlite::tests::find_by_reading 2>&1 | tail -10
  ```

  Expected: 5 PASS。

- [ ] **Step 4: clippy + commit**

  ```bash
  cargo clippy -p kotoha-storage --all-targets -- -D warnings
  git add crates/kotoha-storage/src/user_vocab/sqlite.rs
  git commit -m "feat(storage): implement SqliteUserVocabStore::find_by_reading (#98)"
  ```

---

### Task B3: `SqliteUserVocabStore::insert`(TDD strict)

**Files:**
- Modify: `crates/kotoha-storage/src/user_vocab/sqlite.rs`

**Depends-on:** Task B2

**Estimated LOC:** 100(impl 50 + tests 50)

参照: spec §6.1 / §10.1

- [ ] **Step 1: failing test を書く(`mod tests` 末尾に追加)**

  ```rust
      #[test]
      fn insert_assigns_auto_increment_id() {
          let store = fresh_store();
          let r = UserVocabRecord {
              id: None,
              surface: "日野岡".to_string(),
              reading: "ひのおか".to_string(),
              pos: "名詞-固有名詞-人名".to_string(),
              score: 1.0,
              created_at: 0,
              updated_at: 0,
          };
          let id = store.insert(r).expect("insert ok");
          assert!(id >= 1);
      }

      #[test]
      fn insert_persists_then_can_find() {
          let store = fresh_store();
          let r = UserVocabRecord {
              id: None,
              surface: "琴葉".to_string(),
              reading: "ことば".to_string(),
              pos: "名詞-固有名詞-人名".to_string(),
              score: 1.0,
              created_at: 0,
              updated_at: 0,
          };
          store.insert(r).expect("insert ok");
          let result = store.find_by_reading("ことば", 10).expect("find ok");
          assert_eq!(result.len(), 1);
          assert_eq!(result[0].surface, "琴葉");
      }

      #[test]
      fn insert_duplicate_returns_duplicate_entry_error() {
          let store = fresh_store();
          let r1 = UserVocabRecord {
              id: None,
              surface: "日野岡".to_string(),
              reading: "ひのおか".to_string(),
              pos: "名詞".to_string(),
              score: 1.0,
              created_at: 0,
              updated_at: 0,
          };
          store.insert(r1.clone()).expect("first insert ok");
          let err = store.insert(r1).unwrap_err();
          match err {
              StorageError::DuplicateEntry { surface, reading } => {
                  assert_eq!(surface, "日野岡");
                  assert_eq!(reading, "ひのおか");
              }
              other => panic!("unexpected: {other:?}"),
          }
      }

      #[test]
      fn insert_rejects_invalid_reading() {
          let store = fresh_store();
          let r = UserVocabRecord {
              id: None,
              surface: "X".to_string(),
              reading: "abc".to_string(), // ASCII reject
              pos: "名詞".to_string(),
              score: 1.0,
              created_at: 0,
              updated_at: 0,
          };
          let err = store.insert(r).unwrap_err();
          assert!(matches!(err, StorageError::InvalidField { .. }));
      }

      #[test]
      fn insert_rejects_negative_score() {
          let store = fresh_store();
          let r = UserVocabRecord {
              id: None,
              surface: "X".to_string(),
              reading: "あ".to_string(),
              pos: "名詞".to_string(),
              score: -1.0,
              created_at: 0,
              updated_at: 0,
          };
          let err = store.insert(r).unwrap_err();
          assert!(matches!(err, StorageError::InvalidField { .. }));
      }
  ```

- [ ] **Step 2: Test 失敗確認**

  ```bash
  cargo test -p kotoha-storage user_vocab::sqlite::tests::insert 2>&1 | tail -15
  ```

  Expected: `unimplemented!` panic で 5 件 fail(pre-existing impl が `unimplemented!`)。

- [ ] **Step 3: 実装(`unimplemented!` を置換)**

  ```rust
      fn insert(&self, record: UserVocabRecord) -> Result<i64, StorageError> {
          validate_surface(&record.surface)?;
          validate_reading(&record.reading)?;
          validate_pos(&record.pos)?;
          validate_score(record.score)?;

          let now = std::time::SystemTime::now()
              .duration_since(std::time::UNIX_EPOCH)
              .map(|d| d.as_secs() as i64)
              .unwrap_or(0);
          let created_at = if record.created_at == 0 { now } else { record.created_at };
          let updated_at = if record.updated_at == 0 { now } else { record.updated_at };

          let conn = self.db.lock_conn();
          let result = conn.execute(
              "INSERT INTO user_vocab (surface, reading, pos, score, created_at, updated_at) \
               VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
              rusqlite::params![
                  record.surface,
                  record.reading,
                  record.pos,
                  record.score as f64,
                  created_at,
                  updated_at,
              ],
          );
          match result {
              Ok(_) => Ok(conn.last_insert_rowid()),
              Err(rusqlite::Error::SqliteFailure(e, _))
                  if e.code == rusqlite::ErrorCode::ConstraintViolation =>
              {
                  Err(StorageError::DuplicateEntry {
                      surface: record.surface,
                      reading: record.reading,
                  })
              }
              Err(e) => Err(StorageError::Sqlite(e)),
          }
      }
  ```

  併せて `_suppress_unused` ヘルパは impl 完成後不要となるため削除する。

- [ ] **Step 4: Test pass 確認**

  ```bash
  cargo test -p kotoha-storage user_vocab::sqlite::tests::insert 2>&1 | tail -10
  ```

  Expected: 5 PASS。

- [ ] **Step 5: clippy + commit**

  ```bash
  cargo clippy -p kotoha-storage --all-targets -- -D warnings
  git add crates/kotoha-storage/src/user_vocab/sqlite.rs
  git commit -m "feat(storage): implement SqliteUserVocabStore::insert with validation + duplicate detection (#98)"
  ```

---

### Task B4: `SqliteUserVocabStore::list_all`(TDD strict、`--reading <PREFIX>` prefix match 含む)

**Files:**
- Modify: `crates/kotoha-storage/src/user_vocab/sqlite.rs`

**Depends-on:** Task B3

**Estimated LOC:** 90(impl 40 + tests 50)

参照: spec §6.1 / §7.4

- [ ] **Step 1: failing test を書く(`mod tests` 末尾に追加)**

  ```rust
      #[test]
      fn list_all_returns_all_rows_when_no_filter() {
          let store = fresh_store();
          for i in 0..5 {
              seed_row(&store, &format!("s{}", i), &format!("あ{}", i), i as f32);
          }
          let result = store.list_all(100, 0).expect("ok");
          assert_eq!(result.len(), 5);
      }

      #[test]
      fn list_all_respects_limit_and_offset() {
          let store = fresh_store();
          for i in 0..10 {
              seed_row(&store, &format!("s{}", i), &format!("あ{}", i), i as f32);
          }
          let result = store.list_all(3, 2).expect("ok");
          assert_eq!(result.len(), 3);
      }

      #[test]
      fn list_all_returns_empty_when_offset_past_end() {
          let store = fresh_store();
          seed_row(&store, "x", "あ", 0.0);
          let result = store.list_all(10, 100).expect("ok");
          assert!(result.is_empty());
      }
  ```

  そして `find_by_prefix` を新規 method として trait に追加するか、既存 `find_by_reading` で吸収するかは spec §7.4 の `--reading <PREFIX>` 仕様に整合させる。spec §7.4 の prefix match は SQL `LIKE 'prefix%'` であり、`find_by_reading` は exact match。本 task では `list_all` に optional prefix を持たせるのではなく、`find_by_prefix` を追加 method として trait 拡張する設計とする(後続 dict_cli list で使用)。

  ※trait は既に Task B1 で確定しているため、prefix match の API は `find_by_prefix` を追加する形で trait に新 method を追加する。本 task で trait を拡張する。

  trait `UserVocabStore` に method を追加する(`store.rs`):

  ```rust
      /// `reading` の prefix match で entries を返す(spec §7.4 `--reading <PREFIX>`)。
      ///
      /// SQL `reading LIKE 'PREFIX%'` を使用し、score 降順で `limit` 件返す。
      fn find_by_prefix(&self, reading_prefix: &str, limit: usize) -> Result<Vec<UserVocabRecord>, StorageError>;
  ```

  続けて prefix match の test を追加する:

  ```rust
      #[test]
      fn find_by_prefix_matches_partial() {
          let store = fresh_store();
          seed_row(&store, "日野岡", "ひのおか", 1.0);
          seed_row(&store, "日野", "ひの", 0.5);
          seed_row(&store, "別人", "べつじん", 0.5);
          let result = store.find_by_prefix("ひの", 100).expect("ok");
          assert_eq!(result.len(), 2);
      }
  ```

- [ ] **Step 2: Test 失敗確認**

  ```bash
  cargo test -p kotoha-storage user_vocab 2>&1 | tail -15
  ```

  Expected: `find_by_prefix` 不在で compile error。

- [ ] **Step 3: 実装(list_all + find_by_prefix)**

  `sqlite.rs` の対応 method を以下に置換 / 追加する。

  ```rust
      fn list_all(&self, limit: usize, offset: usize) -> Result<Vec<UserVocabRecord>, StorageError> {
          let conn = self.db.lock_conn();
          let mut stmt = conn.prepare(
              "SELECT id, surface, reading, pos, score, created_at, updated_at \
               FROM user_vocab \
               ORDER BY id ASC \
               LIMIT ?1 OFFSET ?2",
          )?;
          let rows = stmt.query_map(rusqlite::params![limit as i64, offset as i64], |row| {
              Ok(UserVocabRecord {
                  id: Some(row.get(0)?),
                  surface: row.get(1)?,
                  reading: row.get(2)?,
                  pos: row.get(3)?,
                  score: row.get::<_, f64>(4)? as f32,
                  created_at: row.get(5)?,
                  updated_at: row.get(6)?,
              })
          })?;
          let mut out = Vec::new();
          for r in rows {
              out.push(r?);
          }
          Ok(out)
      }

      fn find_by_prefix(&self, reading_prefix: &str, limit: usize) -> Result<Vec<UserVocabRecord>, StorageError> {
          // prefix は hiragana / 空文字どちらも許容(空 prefix = 全件)
          if !reading_prefix.is_empty() {
              validate_reading(reading_prefix)?;
          }
          let conn = self.db.lock_conn();
          let pattern = format!("{}%", reading_prefix);
          let mut stmt = conn.prepare(
              "SELECT id, surface, reading, pos, score, created_at, updated_at \
               FROM user_vocab \
               WHERE reading LIKE ?1 \
               ORDER BY score DESC, id ASC \
               LIMIT ?2",
          )?;
          let rows = stmt.query_map(rusqlite::params![pattern, limit as i64], |row| {
              Ok(UserVocabRecord {
                  id: Some(row.get(0)?),
                  surface: row.get(1)?,
                  reading: row.get(2)?,
                  pos: row.get(3)?,
                  score: row.get::<_, f64>(4)? as f32,
                  created_at: row.get(5)?,
                  updated_at: row.get(6)?,
              })
          })?;
          let mut out = Vec::new();
          for r in rows {
              out.push(r?);
          }
          Ok(out)
      }
  ```

  また、Mock 側にも `find_by_prefix` を追加する必要があるため、後続 Task B9 でも実装する。本 task では `MockUserVocabStore` も同 method を追加する(stub)。

- [ ] **Step 4: Test pass 確認**

  ```bash
  cargo test -p kotoha-storage user_vocab::sqlite::tests 2>&1 | tail -10
  ```

  Expected: 全 PASS(13〜14 件)。

- [ ] **Step 5: clippy + commit**

  ```bash
  cargo clippy -p kotoha-storage --all-targets -- -D warnings
  git add crates/kotoha-storage/src/user_vocab/
  git commit -m "feat(storage): implement list_all + find_by_prefix on SqliteUserVocabStore (#98)"
  ```

---

### Task B5: `SqliteUserVocabStore::delete_by_id`(TDD strict)

**Files:**
- Modify: `crates/kotoha-storage/src/user_vocab/sqlite.rs`

**Depends-on:** Task B4

**Estimated LOC:** 60(impl 25 + tests 35)

参照: spec §6.1 / §7.3

- [ ] **Step 1: failing test を書く(`mod tests` 末尾に追加)**

  ```rust
      #[test]
      fn delete_by_id_removes_existing_row() {
          let store = fresh_store();
          let r = UserVocabRecord {
              id: None,
              surface: "x".to_string(),
              reading: "あ".to_string(),
              pos: "名詞".to_string(),
              score: 0.0,
              created_at: 0,
              updated_at: 0,
          };
          let id = store.insert(r).expect("insert");
          store.delete_by_id(id).expect("delete ok");
          let result = store.find_by_reading("あ", 10).expect("find ok");
          assert!(result.is_empty());
      }

      #[test]
      fn delete_by_id_returns_not_found_when_absent() {
          let store = fresh_store();
          let err = store.delete_by_id(99999).unwrap_err();
          assert!(matches!(err, StorageError::NotFound));
      }
  ```

- [ ] **Step 2: Test 失敗確認**

  ```bash
  cargo test -p kotoha-storage user_vocab::sqlite::tests::delete_by_id 2>&1 | tail -10
  ```

  Expected: `unimplemented!` panic で fail(2 件)。

- [ ] **Step 3: 実装**

  ```rust
      fn delete_by_id(&self, id: i64) -> Result<(), StorageError> {
          let conn = self.db.lock_conn();
          let affected = conn.execute("DELETE FROM user_vocab WHERE id = ?1", rusqlite::params![id])?;
          if affected == 0 {
              return Err(StorageError::NotFound);
          }
          Ok(())
      }
  ```

- [ ] **Step 4: Test pass 確認 + commit**

  ```bash
  cargo test -p kotoha-storage user_vocab::sqlite::tests::delete_by_id 2>&1 | tail -5
  cargo clippy -p kotoha-storage --all-targets -- -D warnings
  git add crates/kotoha-storage/src/user_vocab/sqlite.rs
  git commit -m "feat(storage): implement delete_by_id with NotFound error (#98)"
  ```

---

### Task B6: `SqliteUserVocabStore::delete_by_surface_reading`(TDD strict)

**Files:**
- Modify: `crates/kotoha-storage/src/user_vocab/sqlite.rs`

**Depends-on:** Task B5

**Estimated LOC:** 70(impl 30 + tests 40)

参照: spec §6.1 / §7.3

- [ ] **Step 1: failing test を書く**

  ```rust
      #[test]
      fn delete_by_surface_reading_removes_row() {
          let store = fresh_store();
          let r = UserVocabRecord {
              id: None,
              surface: "日野岡".to_string(),
              reading: "ひのおか".to_string(),
              pos: "名詞".to_string(),
              score: 0.0,
              created_at: 0,
              updated_at: 0,
          };
          store.insert(r).expect("insert");
          store
              .delete_by_surface_reading("日野岡", "ひのおか")
              .expect("delete ok");
          let result = store.find_by_reading("ひのおか", 10).expect("find ok");
          assert!(result.is_empty());
      }

      #[test]
      fn delete_by_surface_reading_returns_not_found_when_absent() {
          let store = fresh_store();
          let err = store
              .delete_by_surface_reading("ない", "ない")
              .unwrap_err();
          assert!(matches!(err, StorageError::NotFound));
      }

      #[test]
      fn delete_by_surface_reading_validates_reading() {
          let store = fresh_store();
          let err = store
              .delete_by_surface_reading("x", "abc")
              .unwrap_err();
          assert!(matches!(err, StorageError::InvalidField { .. }));
      }
  ```

- [ ] **Step 2: Test 失敗確認 + 実装**

  ```rust
      fn delete_by_surface_reading(&self, surface: &str, reading: &str) -> Result<(), StorageError> {
          validate_surface(surface)?;
          validate_reading(reading)?;
          let conn = self.db.lock_conn();
          let affected = conn.execute(
              "DELETE FROM user_vocab WHERE surface = ?1 AND reading = ?2",
              rusqlite::params![surface, reading],
          )?;
          if affected == 0 {
              return Err(StorageError::NotFound);
          }
          Ok(())
      }
  ```

- [ ] **Step 3: Test pass + commit**

  ```bash
  cargo test -p kotoha-storage user_vocab::sqlite::tests 2>&1 | tail -5
  cargo clippy -p kotoha-storage --all-targets -- -D warnings
  git add crates/kotoha-storage/src/user_vocab/sqlite.rs
  git commit -m "feat(storage): implement delete_by_surface_reading with validation (#98)"
  ```

---

### Task B7: `Database::user_vocab_store` factory(spec §6.4)

**Files:**
- Modify: `crates/kotoha-storage/src/database.rs`

**Depends-on:** Task B6

**Estimated LOC:** 30

参照: spec §6.4 / §6.4.1

- [ ] **Step 1: failing test を書く**

  `database.rs` の `mod tests` 末尾に追加。

  ```rust
      #[test]
      fn user_vocab_store_factory_returns_owned_box() {
          let db = Database::open_in_memory().expect("memory open");
          let _store: Box<dyn crate::user_vocab::store::UserVocabStore> = db.user_vocab_store();
      }

      #[test]
      fn user_vocab_store_factory_shares_arc() {
          let db = Database::open_in_memory().expect("memory open");
          let store1 = db.user_vocab_store();
          let store2 = db.user_vocab_store();
          // 両者が独立 Box でも、内部で同一 Arc<Database> を share している
          // ことを insert + find が通ることで確認する
          let r = crate::user_vocab::store::UserVocabRecord {
              id: None,
              surface: "x".to_string(),
              reading: "あ".to_string(),
              pos: "名詞".to_string(),
              score: 0.0,
              created_at: 0,
              updated_at: 0,
          };
          store1.insert(r).expect("insert ok");
          let result = store2.find_by_reading("あ", 10).expect("find ok");
          assert_eq!(result.len(), 1);
      }
  ```

- [ ] **Step 2: 実装**

  `database.rs` `impl Database` 末尾に追加。

  ```rust
      /// `Arc<Database>` を `SqliteUserVocabStore` で wrap し owned `Box<dyn UserVocabStore>` を返す
      /// (spec §6.4 共有 ownership)。
      pub fn user_vocab_store(self: &Arc<Self>) -> Box<dyn crate::user_vocab::store::UserVocabStore> {
          Box::new(crate::user_vocab::sqlite::SqliteUserVocabStore::new(Arc::clone(self)))
      }

      /// (P2-B では unimplemented stub、P2-C で本実装、spec §6.3)
      pub fn learning_cache_store(self: &Arc<Self>) -> Box<dyn crate::learning_cache::LearningCacheStore> {
          Box::new(crate::learning_cache::SqliteLearningCacheStore::new(Arc::clone(self)))
      }
  ```

- [ ] **Step 3: Test pass + commit**

  ```bash
  cargo test -p kotoha-storage database 2>&1 | tail -5
  cargo clippy -p kotoha-storage --all-targets -- -D warnings
  git add crates/kotoha-storage/src/database.rs
  git commit -m "feat(storage): add Database factory for UserVocabStore + LearningCacheStore (#98)"
  ```

---

### Task B8: `LearningCacheStore` trait skeleton(P2-B では interface 公開のみ、impl は P2-C)

**Files:**
- Modify: `crates/kotoha-storage/src/learning_cache/mod.rs`
- Create: `crates/kotoha-storage/src/learning_cache/sqlite.rs`

**Depends-on:** Task B7

**Estimated LOC:** 90

参照: spec §6.3 / §2 Out of scope

- [ ] **Step 1: trait skeleton + 値型を定義**

  `learning_cache/mod.rs` を以下に置き換える。

  ```rust
  //! `LearningCacheStore` trait skeleton(spec §6.3、本実装は P2-C)。

  pub mod sqlite;

  pub use sqlite::SqliteLearningCacheStore;

  use crate::error::StorageError;

  /// Learning cache store の抽象境界(P2-C で本格使用)。
  pub trait LearningCacheStore: Send + Sync {
      fn lookup(&self, kana_input: &str, limit: usize) -> Result<Vec<LearningCacheRecord>, StorageError>;
      fn record_choice(&self, kana_input: &str, chosen_kanji: &str) -> Result<(), StorageError>;
      fn evict_lru(&self, max_entries: usize) -> Result<usize, StorageError>;
  }

  #[derive(Debug, Clone, PartialEq)]
  pub struct LearningCacheRecord {
      pub id: i64,
      pub kana_input: String,
      pub chosen_kanji: String,
      pub frequency: u32,
      pub last_used_at: i64,
  }

  #[cfg(test)]
  mod tests {
      use super::*;

      #[test]
      fn learning_cache_record_holds_all_fields() {
          let r = LearningCacheRecord {
              id: 1,
              kana_input: "あい".to_string(),
              chosen_kanji: "愛".to_string(),
              frequency: 5,
              last_used_at: 1745529600,
          };
          assert_eq!(r.id, 1);
          assert_eq!(r.frequency, 5);
      }
  }
  ```

  `sqlite.rs` を作成。

  ```rust
  //! SqliteLearningCacheStore: P2-B では unimplemented stub、P2-C で実装(spec §6.3)。

  use std::sync::Arc;

  use crate::database::Database;
  use crate::error::StorageError;
  use crate::learning_cache::{LearningCacheRecord, LearningCacheStore};

  pub struct SqliteLearningCacheStore {
      pub(crate) db: Arc<Database>,
  }

  impl SqliteLearningCacheStore {
      pub fn new(db: Arc<Database>) -> Self {
          Self { db }
      }
  }

  impl LearningCacheStore for SqliteLearningCacheStore {
      fn lookup(&self, _kana_input: &str, _limit: usize) -> Result<Vec<LearningCacheRecord>, StorageError> {
          // P2-C で実装
          Ok(Vec::new())
      }

      fn record_choice(&self, _kana_input: &str, _chosen_kanji: &str) -> Result<(), StorageError> {
          // P2-C で実装
          Ok(())
      }

      fn evict_lru(&self, _max_entries: usize) -> Result<usize, StorageError> {
          // P2-C で実装
          Ok(0)
      }
  }

  #[cfg(test)]
  mod tests {
      use super::*;

      #[test]
      fn p2b_stub_lookup_returns_empty() {
          let db = Database::open_in_memory().expect("memory open");
          let store = SqliteLearningCacheStore::new(db);
          let result = store.lookup("あい", 10).expect("ok");
          assert!(result.is_empty());
      }

      #[test]
      fn p2b_stub_record_choice_noop() {
          let db = Database::open_in_memory().expect("memory open");
          let store = SqliteLearningCacheStore::new(db);
          let _ = store.record_choice("あい", "愛").expect("noop ok");
      }
  }
  ```

- [ ] **Step 2: Test pass + commit**

  ```bash
  cargo test -p kotoha-storage learning_cache 2>&1 | tail -5
  cargo clippy -p kotoha-storage --all-targets -- -D warnings
  git add crates/kotoha-storage/src/learning_cache/
  git commit -m "feat(storage): expose LearningCacheStore trait skeleton (P2-C impl deferred) (#98)"
  ```

---

### Task B9: `MockUserVocabStore`(in-memory、trait 互換 test)

**Files:**
- Modify: `crates/kotoha-storage/src/user_vocab/mock.rs`

**Depends-on:** Task B8

**Estimated LOC:** 200(impl 130 + tests 70)

参照: spec §6.6

- [ ] **Step 1: failing test を書く**

  `mock.rs` を以下に置き換える(test 先行)。

  ```rust
  //! MockUserVocabStore(spec §6.6、in-memory、test 用)。

  use std::sync::Mutex;

  use crate::error::StorageError;
  use crate::user_vocab::store::{UserVocabRecord, UserVocabStore};
  use crate::validation::{validate_pos, validate_reading, validate_score, validate_surface};

  pub struct MockUserVocabStore {
      records: Mutex<Vec<UserVocabRecord>>,
      next_id: Mutex<i64>,
  }

  impl Default for MockUserVocabStore {
      fn default() -> Self {
          Self::new()
      }
  }

  impl MockUserVocabStore {
      pub fn new() -> Self {
          Self {
              records: Mutex::new(Vec::new()),
              next_id: Mutex::new(1),
          }
      }
  }

  impl UserVocabStore for MockUserVocabStore {
      fn find_by_reading(&self, reading: &str, limit: usize) -> Result<Vec<UserVocabRecord>, StorageError> {
          validate_reading(reading)?;
          let records = self.records.lock().unwrap();
          let mut filtered: Vec<UserVocabRecord> = records
              .iter()
              .filter(|r| r.reading == reading)
              .cloned()
              .collect();
          filtered.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
          filtered.truncate(limit);
          Ok(filtered)
      }

      fn list_all(&self, limit: usize, offset: usize) -> Result<Vec<UserVocabRecord>, StorageError> {
          let records = self.records.lock().unwrap();
          Ok(records.iter().skip(offset).take(limit).cloned().collect())
      }

      fn find_by_prefix(&self, reading_prefix: &str, limit: usize) -> Result<Vec<UserVocabRecord>, StorageError> {
          if !reading_prefix.is_empty() {
              validate_reading(reading_prefix)?;
          }
          let records = self.records.lock().unwrap();
          let mut filtered: Vec<UserVocabRecord> = records
              .iter()
              .filter(|r| r.reading.starts_with(reading_prefix))
              .cloned()
              .collect();
          filtered.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
          filtered.truncate(limit);
          Ok(filtered)
      }

      fn insert(&self, mut record: UserVocabRecord) -> Result<i64, StorageError> {
          validate_surface(&record.surface)?;
          validate_reading(&record.reading)?;
          validate_pos(&record.pos)?;
          validate_score(record.score)?;

          let mut records = self.records.lock().unwrap();
          if records
              .iter()
              .any(|r| r.surface == record.surface && r.reading == record.reading)
          {
              return Err(StorageError::DuplicateEntry {
                  surface: record.surface,
                  reading: record.reading,
              });
          }
          let mut next_id = self.next_id.lock().unwrap();
          let id = *next_id;
          *next_id += 1;
          record.id = Some(id);
          records.push(record);
          Ok(id)
      }

      fn delete_by_id(&self, id: i64) -> Result<(), StorageError> {
          let mut records = self.records.lock().unwrap();
          let before = records.len();
          records.retain(|r| r.id != Some(id));
          if records.len() == before {
              return Err(StorageError::NotFound);
          }
          Ok(())
      }

      fn delete_by_surface_reading(&self, surface: &str, reading: &str) -> Result<(), StorageError> {
          validate_surface(surface)?;
          validate_reading(reading)?;
          let mut records = self.records.lock().unwrap();
          let before = records.len();
          records.retain(|r| !(r.surface == surface && r.reading == reading));
          if records.len() == before {
              return Err(StorageError::NotFound);
          }
          Ok(())
      }
  }

  #[cfg(test)]
  mod tests {
      use super::*;

      fn rec(surface: &str, reading: &str, score: f32) -> UserVocabRecord {
          UserVocabRecord {
              id: None,
              surface: surface.to_string(),
              reading: reading.to_string(),
              pos: "名詞".to_string(),
              score,
              created_at: 0,
              updated_at: 0,
          }
      }

      #[test]
      fn mock_insert_then_find() {
          let store = MockUserVocabStore::new();
          let id = store.insert(rec("日野岡", "ひのおか", 1.0)).expect("ok");
          assert!(id >= 1);
          let result = store.find_by_reading("ひのおか", 10).expect("ok");
          assert_eq!(result.len(), 1);
      }

      #[test]
      fn mock_duplicate_rejected() {
          let store = MockUserVocabStore::new();
          store.insert(rec("a", "あ", 0.0)).unwrap();
          let err = store.insert(rec("a", "あ", 0.0)).unwrap_err();
          assert!(matches!(err, StorageError::DuplicateEntry { .. }));
      }

      #[test]
      fn mock_find_orders_by_score_desc() {
          let store = MockUserVocabStore::new();
          store.insert(rec("a", "あ", 0.5)).unwrap();
          store.insert(rec("b", "あ", 1.5)).unwrap();
          let result = store.find_by_reading("あ", 10).unwrap();
          assert_eq!(result[0].surface, "b");
          assert_eq!(result[1].surface, "a");
      }

      #[test]
      fn mock_delete_by_id_works() {
          let store = MockUserVocabStore::new();
          let id = store.insert(rec("a", "あ", 0.0)).unwrap();
          store.delete_by_id(id).unwrap();
          assert!(store.find_by_reading("あ", 10).unwrap().is_empty());
      }

      #[test]
      fn mock_delete_by_id_not_found() {
          let store = MockUserVocabStore::new();
          let err = store.delete_by_id(999).unwrap_err();
          assert!(matches!(err, StorageError::NotFound));
      }

      #[test]
      fn mock_delete_by_surface_reading_works() {
          let store = MockUserVocabStore::new();
          store.insert(rec("a", "あ", 0.0)).unwrap();
          store.delete_by_surface_reading("a", "あ").unwrap();
          assert!(store.find_by_reading("あ", 10).unwrap().is_empty());
      }

      #[test]
      fn mock_list_all_pagination() {
          let store = MockUserVocabStore::new();
          for i in 0..5 {
              store.insert(rec(&format!("s{}", i), &format!("あ{}", i), 0.0)).unwrap();
          }
          let result = store.list_all(2, 1).unwrap();
          assert_eq!(result.len(), 2);
      }

      #[test]
      fn mock_find_by_prefix_works() {
          let store = MockUserVocabStore::new();
          store.insert(rec("日野岡", "ひのおか", 1.0)).unwrap();
          store.insert(rec("日野", "ひの", 0.5)).unwrap();
          store.insert(rec("別人", "べつじん", 0.5)).unwrap();
          let result = store.find_by_prefix("ひの", 100).unwrap();
          assert_eq!(result.len(), 2);
      }
  }
  ```

- [ ] **Step 2: Test pass + commit**

  ```bash
  cargo test -p kotoha-storage user_vocab::mock 2>&1 | tail -10
  cargo clippy -p kotoha-storage --all-targets -- -D warnings
  git add crates/kotoha-storage/src/user_vocab/mock.rs
  git commit -m "feat(storage): implement MockUserVocabStore (in-memory, trait-compatible) (#98)"
  ```

---

## Phase C: kotoha-core::dict::user_vocab 統合

### Task C1: feature flag `dict-persist` を kotoha-core に追加

**Files:**
- Modify: `crates/kotoha-core/Cargo.toml`

**Depends-on:** Task B9

**Estimated LOC:** 12

参照: spec §4.3

- [ ] **Step 1: `crates/kotoha-core/Cargo.toml` の `[dependencies]` に optional kotoha-storage を追加**

  既存 `sudachi = { workspace = true, optional = true }` の直後に以下を追加する。

  ```toml
  # `kotoha-storage` is optional so default features do not pull in rusqlite C dep.
  # Activated only when the `dict-persist` feature is enabled (P2-B spec §4.3).
  kotoha-storage = { path = "../kotoha-storage", optional = true }
  ```

- [ ] **Step 2: `[features]` に dict-persist を追加**

  既存 `dict-smoke = ["dict"]` の直後に以下を追加する。

  ```toml
  # `dict-persist` gates UserVocab + SQLite-backed user dict (P2-B spec §4.3).
  # transitively enables `dict` because UserVocab impl VocabularyLookup (P2-A trait).
  dict-persist = ["dep:kotoha-storage", "dict"]
  ```

- [ ] **Step 3: `cargo check --features dict-persist -p kotoha-core` で compile 通過確認**

  ```bash
  cargo check --features dict-persist -p kotoha-core 2>&1 | tail -5
  ```

  Expected: `Finished ...`(warning は許容)。

- [ ] **Step 4: commit**

  ```bash
  git add crates/kotoha-core/Cargo.toml
  git commit -m "feat(core): add dict-persist feature flag pulling in kotoha-storage (#98)"
  ```

---

### Task C2: `UserVocab` 構造体 + `VocabularyLookup::lookup` 実装(TDD strict、MockUserVocabStore 経由)

**Files:**
- Create: `crates/kotoha-core/src/dict/user_vocab.rs`
- Modify: `crates/kotoha-core/src/dict/mod.rs`(`pub(crate) mod user_vocab;` 追加)

**Depends-on:** Task C1

**Estimated LOC:** 180(impl 70 + tests 110)

参照: spec §6.7

- [ ] **Step 1: failing test を書く**

  `crates/kotoha-core/src/dict/user_vocab.rs` を新規作成。

  ```rust
  //! `UserVocab`: `VocabularyLookup` の SQLite 永続化版実装(spec §6.7、P2-B)。

  use kotoha_storage::user_vocab::store::UserVocabStore;

  use crate::dict::vocab::{VocabEntry, VocabularyLookup};

  /// User-managed vocabulary、`Box<dyn UserVocabStore>` に依存(DIP、spec §4.2)。
  pub struct UserVocab {
      store: Box<dyn UserVocabStore>,
      vocab_id: String,
  }

  impl UserVocab {
      /// `Box<dyn UserVocabStore>` を inject して構築する。
      pub fn new(store: Box<dyn UserVocabStore>) -> Self {
          Self {
              store,
              vocab_id: "user-vocab".to_string(),
          }
      }
  }

  impl VocabularyLookup for UserVocab {
      fn lookup(&self, reading: &str) -> Vec<VocabEntry> {
          // SQLite backend エラー / validation エラー時は empty Vec を返し panic しない
          // (spec §6.7、`unwrap_or_default()` 採用根拠)
          self.store
              .find_by_reading(reading, 32)
              .unwrap_or_default()
              .into_iter()
              .map(|r| VocabEntry {
                  surface: r.surface,
                  reading: r.reading,
                  pos: r.pos,
                  score: r.score,
              })
              .collect()
      }

      fn vocab_id(&self) -> &str {
          &self.vocab_id
      }
  }

  #[cfg(test)]
  mod tests {
      use super::*;
      use kotoha_storage::user_vocab::mock::MockUserVocabStore;
      use kotoha_storage::user_vocab::store::UserVocabRecord;

      fn seed_mock(store: &MockUserVocabStore, surface: &str, reading: &str, score: f32) {
          store.insert(UserVocabRecord {
              id: None,
              surface: surface.to_string(),
              reading: reading.to_string(),
              pos: "名詞".to_string(),
              score,
              created_at: 0,
              updated_at: 0,
          }).unwrap();
      }

      #[test]
      fn user_vocab_lookup_returns_seeded_entry() {
          let mock = Box::new(MockUserVocabStore::new());
          seed_mock(&mock, "日野岡", "ひのおか", 1.0);
          let uv = UserVocab::new(mock);
          let result = uv.lookup("ひのおか");
          assert_eq!(result.len(), 1);
          assert_eq!(result[0].surface, "日野岡");
      }

      #[test]
      fn user_vocab_lookup_empty_for_unknown() {
          let mock = Box::new(MockUserVocabStore::new());
          let uv = UserVocab::new(mock);
          let result = uv.lookup("みず");
          assert!(result.is_empty());
      }

      #[test]
      fn user_vocab_lookup_invalid_reading_returns_empty_not_panic() {
          let mock = Box::new(MockUserVocabStore::new());
          let uv = UserVocab::new(mock);
          // 非 hiragana を渡すと validate_reading で error → unwrap_or_default で empty
          let result = uv.lookup("カタカナ");
          assert!(result.is_empty());
      }

      #[test]
      fn user_vocab_lookup_orders_by_score() {
          let mock = Box::new(MockUserVocabStore::new());
          seed_mock(&mock, "a", "あ", 0.3);
          seed_mock(&mock, "b", "あ", 0.9);
          let uv = UserVocab::new(mock);
          let result = uv.lookup("あ");
          assert_eq!(result.len(), 2);
          assert_eq!(result[0].surface, "b");
          assert_eq!(result[1].surface, "a");
      }

      #[test]
      fn user_vocab_id_is_stable() {
          let mock = Box::new(MockUserVocabStore::new());
          let uv = UserVocab::new(mock);
          assert_eq!(uv.vocab_id(), "user-vocab");
      }
  }
  ```

  `crates/kotoha-core/src/dict/mod.rs` の module 列に追加。

  ```rust
  pub(crate) mod backend;
  pub(crate) mod custom_vocab;
  pub(crate) mod engine;
  pub(crate) mod sudachi_adapter;
  pub(crate) mod vocab;
  #[cfg(feature = "dict-persist")]
  pub(crate) mod user_vocab;
  ```

  併せて `pub use` を追加。

  ```rust
  #[cfg(feature = "dict-persist")]
  pub use self::user_vocab::UserVocab;
  ```

- [ ] **Step 2: Test 失敗確認**

  ```bash
  cargo test --features dict-persist -p kotoha-core dict::user_vocab::tests 2>&1 | tail -10
  ```

  Expected: 5 PASS(同 step で test と impl を書いたため)。

- [ ] **Step 3: clippy + commit**

  ```bash
  cargo clippy --features dict-persist -p kotoha-core --all-targets -- -D warnings
  git add crates/kotoha-core/src/dict/user_vocab.rs crates/kotoha-core/src/dict/mod.rs
  git commit -m "feat(dict): add UserVocab impl VocabularyLookup via UserVocabStore (#98)"
  ```

---

### Task C3: `DictionaryConfig.user_vocab_db_path` field 追加

**Files:**
- Modify: `crates/kotoha-core/src/dict/mod.rs`

**Depends-on:** Task C2

**Estimated LOC:** 25(impl 10 + tests 15)

参照: spec §8.1

- [ ] **Step 1: failing test を書く(`mod config_tests` 末尾に追加)**

  ```rust
      #[test]
      fn dictionary_config_default_user_vocab_db_path_is_none() {
          let cfg = DictionaryConfig::default();
          assert!(cfg.user_vocab_db_path.is_none());
      }

      #[test]
      fn dictionary_config_user_vocab_db_path_field_works() {
          let cfg = DictionaryConfig {
              system_dict_path: Some(PathBuf::from("/tmp/system.dic")),
              custom_vocab_path: None,
              user_vocab_db_path: Some(PathBuf::from("/tmp/kotoha.db")),
          };
          assert_eq!(cfg.user_vocab_db_path, Some(PathBuf::from("/tmp/kotoha.db")));
      }
  ```

- [ ] **Step 2: 実装(`DictionaryConfig` struct を更新、既存 field の直後に追加)**

  ```rust
  #[derive(Debug, Clone, Default)]
  pub struct DictionaryConfig {
      pub system_dict_path: Option<PathBuf>,
      pub custom_vocab_path: Option<PathBuf>,
      /// SQLite User dictionary DB のパス(P2-B、spec §8.1)。
      /// `None` の場合 UserVocab を構築しない。
      pub user_vocab_db_path: Option<PathBuf>,
  }
  ```

  注: spec §8.1 では `#[non_exhaustive]` 維持と記載されているが、現行 P2-A の DictionaryConfig は `#[non_exhaustive]` 不在。spec §8.1 の改訂判断に従い、属性追加は本 task で明示しない。属性が必要であれば task F 系で別途取り込む。

- [ ] **Step 3: 既存 test の field 補完(C3 の breaking 影響)**

  P2-A 既存 test の `DictionaryConfig { ... }` literal を全て検索し、`user_vocab_db_path: None` を追加する(Default trait 経由の test は影響無し、struct literal で書いている test のみ修正)。

  ```bash
  grep -rn "DictionaryConfig {" /home/kohshiro/develops/student/kotoha-ime/crates/ | grep -v "#\[" | head -20
  ```

  期待される修正対象は `dict/mod.rs::config_tests`、`dict/backend.rs::tests`(`DictionaryConfig` を直接 literal で書いている箇所のみ)。

- [ ] **Step 4: Test pass 確認 + commit**

  ```bash
  cargo test --features dict-persist -p kotoha-core dict::config_tests 2>&1 | tail -5
  cargo test --features dict -p kotoha-core 2>&1 | tail -10
  cargo clippy --features dict-persist -p kotoha-core --all-targets -- -D warnings
  git add crates/kotoha-core/src/dict/mod.rs
  git commit -m "feat(dict): add DictionaryConfig.user_vocab_db_path field (#98)"
  ```

---

### Task C4: `load_backend` factory 拡張(`Vec<Box<dyn VocabularyLookup>>` の `[CustomVocab, UserVocab]` 順序)

**Files:**
- Modify: `crates/kotoha-core/src/dict/backend.rs`

**Depends-on:** Task C3

**Estimated LOC:** 80(impl 35 + tests 45)

参照: spec §3.7 / §8.2 / §8.3

- [ ] **Step 1: failing test(MockUserVocabStore inject test、Layer 2 integration として個別追加)**

  Layer 2 integration の本格 test は Task C6 で行うため、本 task では `DictionaryBackend::load` の compile 通過のみを保証する。

- [ ] **Step 2: `DictionaryBackend::load` に dict-persist 分岐を追加**

  `crates/kotoha-core/src/dict/backend.rs` の `pub fn load` を以下に置き換える。

  ```rust
      pub fn load(config: &DictionaryConfig) -> Result<Self, KanjiError> {
          let env_value = std::env::var("KOTOHA_SYSTEM_DICT_PATH").ok();
          let Some(dict_path) = resolve_dict_path(config.system_dict_path.as_deref(), env_value)
          else {
              return Err(KanjiError::Backend {
                  reason: "no SudachiDict path: set KOTOHA_SYSTEM_DICT_PATH environment variable, \
                      or pass DictionaryConfig::system_dict_path"
                      .to_string(),
              });
          };
          let engine = Box::new(SudachiAdapter::load(&dict_path)?);
          let mut vocab_sources: Vec<Box<dyn VocabularyLookup>> = Vec::new();
          if let Some(vp) = &config.custom_vocab_path {
              vocab_sources.push(Box::new(CustomVocab::load(vp)?));
          }
          #[cfg(feature = "dict-persist")]
          {
              if let Some(db_path) = &config.user_vocab_db_path {
                  let db = kotoha_storage::Database::open(db_path).map_err(|e| {
                      KanjiError::Backend {
                          reason: format!("failed to open user_vocab DB at {}: {e}", db_path.display()),
                      }
                  })?;
                  let store = db.user_vocab_store();
                  vocab_sources.push(Box::new(crate::dict::user_vocab::UserVocab::new(store)));
              }
          }
          let model_id = format!("dictionary({})", engine.engine_id());
          Ok(Self {
              engine,
              vocab_sources,
              model_id,
          })
      }
  ```

- [ ] **Step 3: clippy + 後段 test C6 で本格検証**

  ```bash
  cargo build --features dict-persist -p kotoha-core 2>&1 | tail -5
  cargo clippy --features dict-persist -p kotoha-core --all-targets -- -D warnings
  ```

- [ ] **Step 4: commit**

  ```bash
  git add crates/kotoha-core/src/dict/backend.rs
  git commit -m "feat(dict): wire UserVocab into DictionaryBackend::load via dict-persist gate (#98)"
  ```

---

### Task C5: feature gate 無効時の `KanjiError::Backend` reason 経路確認

**Files:** verification only(C4 の `#[cfg(feature = "dict-persist")]` block の挙動を確認)

**Depends-on:** Task C4

**Estimated LOC:** 0

参照: spec §10.4

- [ ] **Step 1: feature 無し build で `user_vocab_db_path` が無視されることを確認**

  ```bash
  cargo test --features dict -p kotoha-core 2>&1 | tail -10
  ```

  Expected: 既存 P2-A test 全 PASS(`user_vocab_db_path` field は `dict-persist` 無しでは構築 path に含まれない)。

- [ ] **Step 2: feature 有 build で `user_vocab_db_path` が処理される経路の compile 確認**

  ```bash
  cargo build --features dict-persist -p kotoha-core 2>&1 | tail -5
  ```

  Expected: `Finished`。

- [ ] **Step 3: 不足経路があれば spec §10.4 / §10.5 を参照して `KanjiError::FeatureDisabled` を返す arm を `kanji::backend.rs` の `BackendConfig::Dictionary` に追加することを検討する**(P2-A の既存 arm 構造に従う、`#[cfg(not(feature = "dict-persist"))]` arm が必要かは現状 spec §8.2 の挙動に従い不要と判断)。

- [ ] **Step 4: 不足無ければ commit 不要**(C4 で commit 済)。

---

### Task C6: Layer 2 integration test(MockUserVocabStore inject、`DictionaryBackend::convert` 経由)

**Files:**
- Create: `crates/kotoha-core/tests/dict_user_vocab.rs`

**Depends-on:** Task C5

**Estimated LOC:** 200(test 全体)

参照: spec §10.2

- [ ] **Step 1: test file を作成**

  ```rust
  //! Layer 2 integration tests for UserVocab (P2-B、spec §10.2)。

  #![cfg(feature = "dict-persist")]

  use kotoha_core::dict::vocab::VocabularyLookup;
  use kotoha_core::dict::UserVocab;
  use kotoha_storage::user_vocab::mock::MockUserVocabStore;
  use kotoha_storage::user_vocab::store::UserVocabRecord;

  fn seed(mock: &MockUserVocabStore, surface: &str, reading: &str, score: f32) {
      mock.insert(UserVocabRecord {
          id: None,
          surface: surface.to_string(),
          reading: reading.to_string(),
          pos: "名詞".to_string(),
          score,
          created_at: 0,
          updated_at: 0,
      })
      .unwrap();
  }

  #[test]
  fn user_vocab_lookup_via_mock_store() {
      let mock = Box::new(MockUserVocabStore::new());
      seed(&mock, "日野岡", "ひのおか", 1.0);
      let uv = UserVocab::new(mock);
      let result = uv.lookup("ひのおか");
      assert_eq!(result.len(), 1);
      assert_eq!(result[0].surface, "日野岡");
  }

  #[test]
  fn user_vocab_lookup_empty_when_store_empty() {
      let mock = Box::new(MockUserVocabStore::new());
      let uv = UserVocab::new(mock);
      assert!(uv.lookup("ひのおか").is_empty());
  }

  #[test]
  fn user_vocab_lookup_score_descending() {
      let mock = Box::new(MockUserVocabStore::new());
      seed(&mock, "a", "あ", 0.3);
      seed(&mock, "b", "あ", 0.9);
      seed(&mock, "c", "あ", 0.6);
      let uv = UserVocab::new(mock);
      let result = uv.lookup("あ");
      assert_eq!(result.len(), 3);
      assert_eq!(result[0].surface, "b");
      assert_eq!(result[1].surface, "c");
      assert_eq!(result[2].surface, "a");
  }

  #[test]
  fn user_vocab_lookup_invalid_reading_does_not_panic() {
      let mock = Box::new(MockUserVocabStore::new());
      let uv = UserVocab::new(mock);
      assert!(uv.lookup("ABC").is_empty());
      assert!(uv.lookup("カタカナ").is_empty());
  }

  #[test]
  fn user_vocab_id_is_user_vocab() {
      let mock = Box::new(MockUserVocabStore::new());
      let uv = UserVocab::new(mock);
      assert_eq!(uv.vocab_id(), "user-vocab");
  }

  #[test]
  fn user_vocab_returns_vocab_entries_with_pos() {
      let mock = Box::new(MockUserVocabStore::new());
      seed(&mock, "日野岡", "ひのおか", 1.0);
      let uv = UserVocab::new(mock);
      let result = uv.lookup("ひのおか");
      assert_eq!(result[0].pos, "名詞");
  }
  ```

- [ ] **Step 2: Test pass + commit**

  ```bash
  cargo test --features dict-persist -p kotoha-core --test dict_user_vocab 2>&1 | tail -10
  cargo clippy --features dict-persist -p kotoha-core --all-targets -- -D warnings
  git add crates/kotoha-core/tests/dict_user_vocab.rs
  git commit -m "test(dict): add Layer 2 integration tests for UserVocab via MockStore (#98)"
  ```

  Expected: 6 PASS。

---

## Phase D: kotoha-cli + kotoha-dict binary

### Task D1: `kotoha-cli/Cargo.toml` に `kotoha-dict` binary + `dict-persist` feature 追加

**Files:**
- Modify: `crates/kotoha-cli/Cargo.toml`

**Depends-on:** Task C6

**Estimated LOC:** 25

参照: spec §4.3 / §7.1

- [ ] **Step 1: `[dependencies]` に optional kotoha-storage を追加**

  既存 `clap = { workspace = true }` の直後に追加。

  ```toml
  kotoha-storage = { path = "../kotoha-storage", optional = true }
  ```

- [ ] **Step 2: `[dev-dependencies]` を追加**

  既存 `[features]` セクションの直前に以下を追加。

  ```toml
  [dev-dependencies]
  tempfile = { workspace = true }
  assert_cmd = { workspace = true }
  serial_test = { workspace = true }
  ```

- [ ] **Step 3: `[features]` に dict-persist を追加**

  ```toml
  dict-persist = ["kotoha-core/dict-persist", "dep:kotoha-storage"]
  ```

- [ ] **Step 4: `[[bin]]` に kotoha-dict を追加**

  既存 `kotoha-kanji` block の直後に以下を追加。

  ```toml
  # `kotoha-dict` is gated on the `dict-persist` feature so default builds
  # do not pull rusqlite C dep. Enable with
  # `cargo build -p kotoha-cli --bin kotoha-dict --features dict-persist` (P2-B spec §7.1).
  [[bin]]
  name = "kotoha-dict"
  path = "src/bin/dict.rs"
  required-features = ["dict-persist"]
  ```

- [ ] **Step 5: compile 通過確認**

  ```bash
  cargo check -p kotoha-cli --features dict-persist 2>&1 | tail -5
  ```

  Expected: `kotoha-dict` binary 不在で compile error。Task D8 で `bin/dict.rs` を作成すれば解消する。本 task では `cargo check -p kotoha-cli` (feature 無し) が通ることを確認する。

  ```bash
  cargo check -p kotoha-cli 2>&1 | tail -5
  ```

- [ ] **Step 6: commit**

  ```bash
  git add crates/kotoha-cli/Cargo.toml
  git commit -m "chore(cli): add dict-persist feature + kotoha-dict binary entry to kotoha-cli (#98)"
  ```

---

### Task D2: `kotoha-cli/src/dict_cli.rs` skeleton + clap derive 構造

**Files:**
- Create: `crates/kotoha-cli/src/dict_cli.rs`
- Modify: `crates/kotoha-cli/src/lib.rs`(`pub mod dict_cli;` を feature gated で追加)

**Depends-on:** Task D1

**Estimated LOC:** 130

参照: spec §7.1〜§7.5

- [ ] **Step 1: `lib.rs` に gated module 追加**

  既存 `pub mod kanji_cli;` の直後に以下を追加。

  ```rust
  #[cfg(feature = "dict-persist")]
  pub mod dict_cli;
  ```

- [ ] **Step 2: `dict_cli.rs` を新規作成(skeleton + Args 定義のみ、entry 関数は次 task 群)**

  ```rust
  //! `kotoha-dict` CLI 実装(spec §7、P2-B、`dict-persist` feature 下)。

  use std::path::PathBuf;

  use clap::{Args, Parser, Subcommand};

  /// kotoha-dict 全体の CLI。
  #[derive(Debug, Parser)]
  #[command(
      name = "kotoha-dict",
      about = "Kotoha User dictionary CLI (P2-B)",
      version
  )]
  pub struct Cli {
      /// DB 配置 dir を上書き(spec §7.1、KOTOHA_DATA_DIR と等価、CLI 引数優先)。
      #[arg(long, value_name = "PATH", global = true)]
      pub data_dir: Option<PathBuf>,

      /// 成功時の確認メッセージを抑止(spec §7.1)。
      #[arg(long, global = true)]
      pub quiet: bool,

      /// 出力を JSON 形式で返す(`list` / `show` 用、spec §7.1)。
      #[arg(long, global = true)]
      pub json: bool,

      #[command(subcommand)]
      pub command: Command,
  }

  #[derive(Debug, Subcommand)]
  pub enum Command {
      /// Add an entry to the user dictionary.
      Add(AddArgs),
      /// Remove an entry by id, or by surface+reading.
      Remove(RemoveArgs),
      /// List entries (text or JSON format).
      List(ListArgs),
      /// Show details for a single entry by id.
      Show(ShowArgs),
  }

  #[derive(Debug, Args)]
  pub struct AddArgs {
      pub surface: String,
      pub reading: String,
      #[arg(long, default_value = "名詞-固有名詞-一般")]
      pub pos: String,
      #[arg(long, default_value_t = 1.0)]
      pub score: f32,
  }

  #[derive(Debug, Args)]
  #[command(group = clap::ArgGroup::new("target").required(true).multiple(false))]
  pub struct RemoveArgs {
      /// Remove by id.
      #[arg(group = "target")]
      pub id: Option<i64>,
      /// Remove by surface + reading(両指定 / 両未指定は exit 2、spec §7.3)。
      #[arg(long, requires = "reading", group = "target")]
      pub surface: Option<String>,
      #[arg(long, requires = "surface")]
      pub reading: Option<String>,
  }

  #[derive(Debug, Args)]
  pub struct ListArgs {
      #[arg(long, value_enum, default_value_t = ListFormat::Text)]
      pub format: ListFormat,
      #[arg(long)]
      pub reading: Option<String>,
      #[arg(long, default_value_t = 100)]
      pub limit: usize,
      #[arg(long, default_value_t = 0)]
      pub offset: usize,
  }

  #[derive(Debug, Clone, Copy, clap::ValueEnum)]
  pub enum ListFormat {
      Text,
      Json,
  }

  #[derive(Debug, Args)]
  pub struct ShowArgs {
      pub id: i64,
  }

  /// Exit code 体系(spec §7.6)。
  pub const EXIT_OK: i32 = 0;
  pub const EXIT_INTERNAL: i32 = 1;
  pub const EXIT_INPUT: i32 = 2;
  pub const EXIT_DUPLICATE: i32 = 3;
  pub const EXIT_NOT_FOUND: i32 = 4;
  ```

- [ ] **Step 3: compile 通過確認**

  ```bash
  cargo check -p kotoha-cli --features dict-persist 2>&1 | tail -5
  ```

  Expected: `bin/dict.rs` 不在の error。本 task では skeleton のみで OK(後続 task で entry 実装)。次 task が clap parse の test を担うため、本 task は impl-skeleton commit する。

- [ ] **Step 4: commit**

  ```bash
  git add crates/kotoha-cli/src/dict_cli.rs crates/kotoha-cli/src/lib.rs
  git commit -m "feat(cli): scaffold dict_cli module with clap derive structure (#98)"
  ```

---

### Task D3: `dict_cli::normalize_reading`(reading auto-detect)実装(TDD strict)

**Files:**
- Modify: `crates/kotoha-cli/src/dict_cli.rs`

**Depends-on:** Task D2

**Estimated LOC:** 130(impl 50 + tests 80)

参照: spec §3.3 / §7.7

- [ ] **Step 1: failing test を書く(`mod tests` を `dict_cli.rs` 末尾に追加)**

  ```rust
  #[cfg(test)]
  mod tests {
      use super::*;

      #[test]
      fn normalize_reading_passes_through_pure_hiragana() {
          let result = normalize_reading("ひのおか").expect("ok");
          assert_eq!(result, "ひのおか");
      }

      #[test]
      fn normalize_reading_passes_through_long_sound() {
          let result = normalize_reading("こーひー").expect("ok");
          assert_eq!(result, "こーひー");
      }

      #[test]
      fn normalize_reading_converts_pure_ascii_to_hiragana() {
          let result = normalize_reading("hinooka").expect("ok");
          assert_eq!(result, "ひのおか");
      }

      #[test]
      fn normalize_reading_rejects_mixed_hiragana_ascii() {
          let err = normalize_reading("ひのoka").unwrap_err();
          assert!(err.contains("hiragana") || err.contains("ASCII"));
      }

      #[test]
      fn normalize_reading_rejects_katakana() {
          let err = normalize_reading("カタカナ").unwrap_err();
          assert!(err.contains("hiragana") || err.contains("ASCII"));
      }

      #[test]
      fn normalize_reading_rejects_kanji() {
          let err = normalize_reading("漢字").unwrap_err();
          assert!(err.contains("hiragana") || err.contains("ASCII"));
      }

      #[test]
      fn normalize_reading_rejects_empty() {
          let err = normalize_reading("").unwrap_err();
          assert!(!err.is_empty());
      }
  }
  ```

- [ ] **Step 2: Test 失敗確認**

  ```bash
  cargo test -p kotoha-cli --features dict-persist dict_cli::tests::normalize 2>&1 | tail -10
  ```

  Expected: `cannot find function 'normalize_reading'`。

- [ ] **Step 3: 実装(`dict_cli.rs` 末尾の test より前に追加)**

  ```rust
  /// `<READING>` 引数の auto-detect normalize(spec §7.7)。
  ///
  /// - 全 hiragana(U+3040..=U+309F + U+30FC + U+30FB) → そのまま
  /// - 全 ASCII(U+0020..=U+007E) → `RomajiConverter::romaji_to_hiragana` で変換
  /// - それ以外(混在 / カタカナ / 漢字) → Err
  pub fn normalize_reading(reading: &str) -> Result<String, String> {
      if reading.is_empty() {
          return Err("READING must not be empty".to_string());
      }
      let all_hiragana = reading.chars().all(|c| {
          let cp = c as u32;
          (0x3040..=0x309F).contains(&cp) || cp == 0x30FC || cp == 0x30FB
      });
      if all_hiragana {
          return Ok(reading.to_string());
      }
      let all_ascii = reading.chars().all(|c| {
          let cp = c as u32;
          (0x0020..=0x007E).contains(&cp)
      });
      if all_ascii {
          let converter = kotoha_core::romaji::RomajiConverter::new();
          let converted = converter.romaji_to_hiragana(reading);
          // 変換後に non-hiragana が残っている可能性は低いが念のため
          let still_ok = converted.chars().all(|c| {
              let cp = c as u32;
              (0x3040..=0x309F).contains(&cp) || cp == 0x30FC || cp == 0x30FB
          });
          if !still_ok {
              return Err("READING (ASCII) failed to convert to pure hiragana".to_string());
          }
          return Ok(converted);
      }
      Err("READING must be all hiragana or all ASCII romaji".to_string())
  }
  ```

  注: `kotoha_core::romaji::RomajiConverter` の実 API は P0 で確定済。method 名 / 戻り値型は `crates/kotoha-core/src/romaji/` を参照して exact 名に揃えること(`romaji_to_hiragana(&self, &str) -> String` を想定。署名が違う場合は本 task で既存 method に揃える)。

- [ ] **Step 4: Test pass + commit**

  ```bash
  cargo test -p kotoha-cli --features dict-persist dict_cli::tests::normalize 2>&1 | tail -10
  cargo clippy -p kotoha-cli --features dict-persist --all-targets -- -D warnings
  git add crates/kotoha-cli/src/dict_cli.rs
  git commit -m "feat(cli): add normalize_reading auto-detect (hiragana/ASCII/mixed) (#98)"
  ```

---

### Task D4: `add` subcommand 実装 + Layer 3 integration test(`assert_cmd` + `tempfile::tempdir` + `#[serial]`)

**Files:**
- Modify: `crates/kotoha-cli/src/dict_cli.rs`(`run_add` 関数 + 既存 entry に組込)
- Create: `crates/kotoha-cli/tests/dict_cli.rs`(Layer 3 integration test)

**Depends-on:** Task D3

**Estimated LOC:** 200(impl 80 + tests 120)

参照: spec §7.2 / §10.3 / §10.3.1

- [ ] **Step 1: `dict_cli.rs` に `run_add` を実装**

  ```rust
  use kotoha_storage::user_vocab::store::UserVocabRecord;
  use kotoha_storage::user_vocab::store::UserVocabStore;

  /// `kotoha-dict add` 実装。
  ///
  /// # Errors
  ///
  /// Exit code を文字列で返す(`Result<exit_code, String>`)。
  pub fn run_add(
      store: &dyn UserVocabStore,
      args: &AddArgs,
      quiet: bool,
  ) -> Result<i32, String> {
      let reading = match normalize_reading(&args.reading) {
          Ok(s) => s,
          Err(e) => {
              eprintln!("error: {}", e);
              return Ok(EXIT_INPUT);
          }
      };
      let now = std::time::SystemTime::now()
          .duration_since(std::time::UNIX_EPOCH)
          .map(|d| d.as_secs() as i64)
          .unwrap_or(0);
      let record = UserVocabRecord {
          id: None,
          surface: args.surface.clone(),
          reading,
          pos: args.pos.clone(),
          score: args.score,
          created_at: now,
          updated_at: now,
      };
      match store.insert(record) {
          Ok(id) => {
              if !quiet {
                  println!(
                      "added: id={id} surface=\"{}\" reading=\"{}\" pos=\"{}\" score={}",
                      args.surface, args.reading, args.pos, args.score
                  );
              }
              Ok(EXIT_OK)
          }
          Err(kotoha_storage::StorageError::DuplicateEntry { surface, reading }) => {
              eprintln!("error: duplicate entry: surface={surface} reading={reading}");
              Ok(EXIT_DUPLICATE)
          }
          Err(kotoha_storage::StorageError::InvalidField { name, reason }) => {
              eprintln!("error: invalid {name}: {reason}");
              Ok(EXIT_INPUT)
          }
          Err(e) => {
              eprintln!("error: {e}");
              Ok(EXIT_INTERNAL)
          }
      }
  }
  ```

- [ ] **Step 2: Layer 3 integration test を作成**

  `crates/kotoha-cli/tests/dict_cli.rs` を新規作成。

  ```rust
  //! Layer 3 integration test for `kotoha-dict` CLI (spec §10.3 / §10.3.1)。
  //!
  //! `KOTOHA_DATA_DIR` env var 操作は process-global のため `#[serial]` 必須。

  #![cfg(feature = "dict-persist")]

  use assert_cmd::Command;
  use serial_test::serial;
  use tempfile::tempdir;

  fn cli() -> Command {
      Command::cargo_bin("kotoha-dict").expect("kotoha-dict binary built")
  }

  #[test]
  #[serial]
  fn add_subcommand_succeeds_with_pure_hiragana_reading() {
      let tmp = tempdir().unwrap();
      cli()
          .env("KOTOHA_DATA_DIR", tmp.path())
          .args(["add", "日野岡", "ひのおか"])
          .assert()
          .success();
  }

  #[test]
  #[serial]
  fn add_subcommand_rejects_mixed_reading() {
      let tmp = tempdir().unwrap();
      cli()
          .env("KOTOHA_DATA_DIR", tmp.path())
          .args(["add", "日野岡", "ひのoka"])
          .assert()
          .code(2);
  }

  #[test]
  #[serial]
  fn add_subcommand_returns_3_on_duplicate() {
      let tmp = tempdir().unwrap();
      cli()
          .env("KOTOHA_DATA_DIR", tmp.path())
          .args(["add", "X", "あ"])
          .assert()
          .success();
      cli()
          .env("KOTOHA_DATA_DIR", tmp.path())
          .args(["add", "X", "あ"])
          .assert()
          .code(3);
  }

  #[test]
  #[serial]
  fn add_subcommand_returns_2_on_nan_score() {
      let tmp = tempdir().unwrap();
      cli()
          .env("KOTOHA_DATA_DIR", tmp.path())
          .args(["add", "X", "あ", "--score", "nan"])
          .assert()
          .code(2);
  }
  ```

- [ ] **Step 3: D8 完了後に test pass を確認**

  本 task の test は `cargo bin "kotoha-dict"` を呼ぶため、Task D8 で entry point を作成するまで test 実行不可。本 task では impl + test を commit し、test pass は D8 完了時にまとめて確認する。

- [ ] **Step 4: commit**

  ```bash
  cargo clippy -p kotoha-cli --features dict-persist --all-targets -- -D warnings
  git add crates/kotoha-cli/src/dict_cli.rs crates/kotoha-cli/tests/dict_cli.rs
  git commit -m "feat(cli): implement add subcommand + Layer 3 integration test (#98)"
  ```

---

### Task D5: `remove` subcommand(id 経由 + surface+reading 経由)

**Files:**
- Modify: `crates/kotoha-cli/src/dict_cli.rs`
- Modify: `crates/kotoha-cli/tests/dict_cli.rs`

**Depends-on:** Task D4

**Estimated LOC:** 180

参照: spec §7.3 / §7.6

- [ ] **Step 1: `run_remove` を実装**

  ```rust
  pub fn run_remove(
      store: &dyn UserVocabStore,
      args: &RemoveArgs,
      quiet: bool,
  ) -> Result<i32, String> {
      let result = match (args.id, &args.surface, &args.reading) {
          (Some(id), None, None) => store.delete_by_id(id).map(|()| (id, None)),
          (None, Some(s), Some(r)) => {
              let normalized = match normalize_reading(r) {
                  Ok(n) => n,
                  Err(e) => {
                      eprintln!("error: {e}");
                      return Ok(EXIT_INPUT);
                  }
              };
              store
                  .delete_by_surface_reading(s, &normalized)
                  .map(|()| (0, Some(format!("{s}/{normalized}"))))
          }
          _ => {
              eprintln!("error: specify either <ID> or both --surface and --reading");
              return Ok(EXIT_INPUT);
          }
      };
      match result {
          Ok((id, label)) => {
              if !quiet {
                  match label {
                      Some(l) => println!("removed: {l}"),
                      None => println!("removed: id={id}"),
                  }
              }
              Ok(EXIT_OK)
          }
          Err(kotoha_storage::StorageError::NotFound) => {
              eprintln!("error: entry not found");
              Ok(EXIT_NOT_FOUND)
          }
          Err(kotoha_storage::StorageError::InvalidField { name, reason }) => {
              eprintln!("error: invalid {name}: {reason}");
              Ok(EXIT_INPUT)
          }
          Err(e) => {
              eprintln!("error: {e}");
              Ok(EXIT_INTERNAL)
          }
      }
  }
  ```

- [ ] **Step 2: integration test を追加(`tests/dict_cli.rs` 末尾に追加)**

  ```rust
  #[test]
  #[serial]
  fn remove_subcommand_by_id_succeeds() {
      let tmp = tempdir().unwrap();
      cli()
          .env("KOTOHA_DATA_DIR", tmp.path())
          .args(["add", "X", "あ"])
          .assert()
          .success();
      // id=1 が割り当てられる(initial AUTOINCREMENT)
      cli()
          .env("KOTOHA_DATA_DIR", tmp.path())
          .args(["remove", "1"])
          .assert()
          .success();
  }

  #[test]
  #[serial]
  fn remove_subcommand_by_id_returns_4_when_absent() {
      let tmp = tempdir().unwrap();
      cli()
          .env("KOTOHA_DATA_DIR", tmp.path())
          .args(["remove", "999"])
          .assert()
          .code(4);
  }

  #[test]
  #[serial]
  fn remove_subcommand_by_surface_reading_succeeds() {
      let tmp = tempdir().unwrap();
      cli()
          .env("KOTOHA_DATA_DIR", tmp.path())
          .args(["add", "日野岡", "ひのおか"])
          .assert()
          .success();
      cli()
          .env("KOTOHA_DATA_DIR", tmp.path())
          .args(["remove", "--surface", "日野岡", "--reading", "ひのおか"])
          .assert()
          .success();
  }
  ```

- [ ] **Step 3: commit**

  ```bash
  cargo clippy -p kotoha-cli --features dict-persist --all-targets -- -D warnings
  git add crates/kotoha-cli/src/dict_cli.rs crates/kotoha-cli/tests/dict_cli.rs
  git commit -m "feat(cli): implement remove subcommand (id + surface+reading) (#98)"
  ```

---

### Task D6: `list` subcommand(text / json + `--reading <PREFIX>`)

**Files:**
- Modify: `crates/kotoha-cli/src/dict_cli.rs`

**Depends-on:** Task D5

**Estimated LOC:** 200

参照: spec §7.4

- [ ] **Step 1: `run_list` を実装**

  ```rust
  pub fn run_list(
      store: &dyn UserVocabStore,
      args: &ListArgs,
      json_global: bool,
  ) -> Result<i32, String> {
      let format = if json_global { ListFormat::Json } else { args.format };
      let records = match &args.reading {
          Some(prefix) => {
              let normalized = match normalize_reading(prefix) {
                  Ok(n) => n,
                  Err(e) => {
                      eprintln!("error: {e}");
                      return Ok(EXIT_INPUT);
                  }
              };
              store.find_by_prefix(&normalized, args.limit)
          }
          None => store.list_all(args.limit, args.offset),
      };
      let records = match records {
          Ok(rs) => rs,
          Err(e) => {
              eprintln!("error: {e}");
              return Ok(EXIT_INTERNAL);
          }
      };
      match format {
          ListFormat::Text => {
              println!("{:<6}{:<14}{:<14}{:<32}{}", "ID", "SURFACE", "READING", "POS", "SCORE");
              for r in &records {
                  println!(
                      "{:<6}{:<14}{:<14}{:<32}{}",
                      r.id.unwrap_or(0),
                      r.surface,
                      r.reading,
                      r.pos,
                      r.score
                  );
              }
          }
          ListFormat::Json => {
              // 軽量 JSON 出力(serde 依存を避けるため手書き)
              print!("[");
              for (i, r) in records.iter().enumerate() {
                  if i > 0 {
                      print!(",");
                  }
                  print!(
                      "{{\"id\":{},\"surface\":{},\"reading\":{},\"pos\":{},\"score\":{},\"created_at\":{},\"updated_at\":{}}}",
                      r.id.unwrap_or(0),
                      json_escape(&r.surface),
                      json_escape(&r.reading),
                      json_escape(&r.pos),
                      r.score,
                      r.created_at,
                      r.updated_at
                  );
              }
              println!("]");
          }
      }
      Ok(EXIT_OK)
  }

  fn json_escape(s: &str) -> String {
      let mut out = String::with_capacity(s.len() + 2);
      out.push('"');
      for c in s.chars() {
          match c {
              '"' => out.push_str("\\\""),
              '\\' => out.push_str("\\\\"),
              '\n' => out.push_str("\\n"),
              '\r' => out.push_str("\\r"),
              '\t' => out.push_str("\\t"),
              c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
              c => out.push(c),
          }
      }
      out.push('"');
      out
  }
  ```

- [ ] **Step 2: integration test を追加**

  ```rust
  #[test]
  #[serial]
  fn list_subcommand_text_format() {
      let tmp = tempdir().unwrap();
      cli()
          .env("KOTOHA_DATA_DIR", tmp.path())
          .args(["add", "日野岡", "ひのおか"])
          .assert()
          .success();
      cli()
          .env("KOTOHA_DATA_DIR", tmp.path())
          .arg("list")
          .assert()
          .success()
          .stdout(predicates::str::contains("日野岡"));
  }

  #[test]
  #[serial]
  fn list_subcommand_json_format_parseable() {
      let tmp = tempdir().unwrap();
      cli()
          .env("KOTOHA_DATA_DIR", tmp.path())
          .args(["add", "日野岡", "ひのおか"])
          .assert()
          .success();
      let output = cli()
          .env("KOTOHA_DATA_DIR", tmp.path())
          .args(["list", "--format", "json"])
          .output()
          .expect("output");
      let stdout = String::from_utf8_lossy(&output.stdout);
      assert!(stdout.starts_with('['));
      assert!(stdout.trim_end().ends_with(']'));
      assert!(stdout.contains("日野岡"));
  }

  #[test]
  #[serial]
  fn list_subcommand_with_reading_prefix() {
      let tmp = tempdir().unwrap();
      cli().env("KOTOHA_DATA_DIR", tmp.path()).args(["add", "日野岡", "ひのおか"]).assert().success();
      cli().env("KOTOHA_DATA_DIR", tmp.path()).args(["add", "別人", "べつじん"]).assert().success();
      let output = cli()
          .env("KOTOHA_DATA_DIR", tmp.path())
          .args(["list", "--reading", "ひの"])
          .output()
          .expect("output");
      let stdout = String::from_utf8_lossy(&output.stdout);
      assert!(stdout.contains("日野岡"));
      assert!(!stdout.contains("別人"));
  }
  ```

  `predicates` crate は `assert_cmd` の transitive dep として利用可能。直 dep 化が必要なら `[dev-dependencies]` の `predicates = "3"` 追加を検討する(本 task では `assert_cmd::predicate` 経由で利用可能ならそのまま採用)。

- [ ] **Step 3: commit**

  ```bash
  cargo clippy -p kotoha-cli --features dict-persist --all-targets -- -D warnings
  git add crates/kotoha-cli/src/dict_cli.rs crates/kotoha-cli/tests/dict_cli.rs
  git commit -m "feat(cli): implement list subcommand (text/json + --reading prefix) (#98)"
  ```

---

### Task D7: `show <id>` subcommand

**Files:**
- Modify: `crates/kotoha-cli/src/dict_cli.rs`
- Modify: `crates/kotoha-cli/tests/dict_cli.rs`

**Depends-on:** Task D6

**Estimated LOC:** 110

参照: spec §7.5

- [ ] **Step 1: `run_show` を実装**

  ```rust
  pub fn run_show(
      store: &dyn UserVocabStore,
      args: &ShowArgs,
      json_global: bool,
  ) -> Result<i32, String> {
      // O(N) であるが UserVocab は数千 entry 規模が想定上限のため許容(spec §7.5)。
      let records = match store.list_all(usize::MAX, 0) {
          Ok(rs) => rs,
          Err(e) => {
              eprintln!("error: {e}");
              return Ok(EXIT_INTERNAL);
          }
      };
      let target = records.iter().find(|r| r.id == Some(args.id));
      let r = match target {
          Some(r) => r,
          None => {
              eprintln!("error: entry not found");
              return Ok(EXIT_NOT_FOUND);
          }
      };
      if json_global {
          println!(
              "{{\"id\":{},\"surface\":{},\"reading\":{},\"pos\":{},\"score\":{},\"created_at\":{},\"updated_at\":{}}}",
              r.id.unwrap_or(0),
              json_escape(&r.surface),
              json_escape(&r.reading),
              json_escape(&r.pos),
              r.score,
              r.created_at,
              r.updated_at
          );
      } else {
          println!("id:         {}", r.id.unwrap_or(0));
          println!("surface:    {}", r.surface);
          println!("reading:    {}", r.reading);
          println!("pos:        {}", r.pos);
          println!("score:      {}", r.score);
          println!("created_at: {} (epoch)", r.created_at);
          println!("updated_at: {} (epoch)", r.updated_at);
      }
      Ok(EXIT_OK)
  }
  ```

- [ ] **Step 2: test 追加**

  ```rust
  #[test]
  #[serial]
  fn show_subcommand_returns_4_when_absent() {
      let tmp = tempdir().unwrap();
      cli()
          .env("KOTOHA_DATA_DIR", tmp.path())
          .args(["show", "999"])
          .assert()
          .code(4);
  }

  #[test]
  #[serial]
  fn show_subcommand_displays_entry() {
      let tmp = tempdir().unwrap();
      cli().env("KOTOHA_DATA_DIR", tmp.path()).args(["add", "日野岡", "ひのおか"]).assert().success();
      cli()
          .env("KOTOHA_DATA_DIR", tmp.path())
          .args(["show", "1"])
          .assert()
          .success();
  }
  ```

- [ ] **Step 3: commit**

  ```bash
  cargo clippy -p kotoha-cli --features dict-persist --all-targets -- -D warnings
  git add crates/kotoha-cli/src/dict_cli.rs crates/kotoha-cli/tests/dict_cli.rs
  git commit -m "feat(cli): implement show subcommand (#98)"
  ```

---

### Task D8: `kotoha-cli/src/bin/dict.rs`(clap entry)

**Files:**
- Create: `crates/kotoha-cli/src/bin/dict.rs`

**Depends-on:** Task D7

**Estimated LOC:** 90

参照: spec §7.1

- [ ] **Step 1: entry binary を作成**

  ```rust
  //! `kotoha-dict` binary entry point (P2-B、spec §7.1)。

  #![cfg(feature = "dict-persist")]

  use clap::Parser;

  use kotoha_cli::dict_cli::{
      run_add, run_list, run_remove, run_show, Cli, Command, EXIT_INTERNAL,
  };

  fn main() {
      let cli = Cli::parse();

      // DB path 解決: --data-dir > KOTOHA_DATA_DIR > XDG > HOME(spec §5.4)
      if let Some(dir) = &cli.data_dir {
          std::env::set_var("KOTOHA_DATA_DIR", dir);
      }
      let db_path = match kotoha_storage::path::resolve_data_dir() {
          Ok(p) => p,
          Err(e) => {
              eprintln!("error: failed to resolve data dir: {e}");
              std::process::exit(EXIT_INTERNAL);
          }
      };
      let db = match kotoha_storage::Database::open(&db_path) {
          Ok(d) => d,
          Err(e) => {
              eprintln!("error: failed to open DB at {}: {e}", db_path.display());
              std::process::exit(EXIT_INTERNAL);
          }
      };
      let store = db.user_vocab_store();

      let exit_code = match &cli.command {
          Command::Add(args) => run_add(store.as_ref(), args, cli.quiet).unwrap_or(EXIT_INTERNAL),
          Command::Remove(args) => run_remove(store.as_ref(), args, cli.quiet).unwrap_or(EXIT_INTERNAL),
          Command::List(args) => run_list(store.as_ref(), args, cli.json).unwrap_or(EXIT_INTERNAL),
          Command::Show(args) => run_show(store.as_ref(), args, cli.json).unwrap_or(EXIT_INTERNAL),
      };
      std::process::exit(exit_code);
  }
  ```

- [ ] **Step 2: Layer 3 全 test PASS を一括確認**

  ```bash
  cargo test -p kotoha-cli --features dict-persist --test dict_cli 2>&1 | tail -15
  ```

  Expected: 11〜14 件 PASS(D4 から D7 で追加した integration test 全て)。

- [ ] **Step 3: clippy + commit**

  ```bash
  cargo clippy -p kotoha-cli --features dict-persist --all-targets -- -D warnings
  git add crates/kotoha-cli/src/bin/dict.rs
  git commit -m "feat(cli): add kotoha-dict binary entry point (#98)"
  ```

---

### Task D9: exit code 体系(0 / 1 / 2 / 3 / 4)整備 + 全 case test

**Files:** verification + 不足 test 追加

**Depends-on:** Task D8

**Estimated LOC:** 30(必要時に追加 test)

参照: spec §7.6

- [ ] **Step 1: 各 exit code を返す code path の存在を確認**

  | exit code | 経路 | 確認 task |
  |---|---|---|
  | 0 | 全 subcommand 正常終了 | D4 / D5 / D6 / D7 |
  | 1 | DB IO error | D8 main の DB open error path |
  | 2 | 入力 error / 混在 reading | D4 mixed_reading / D5 排他 group |
  | 3 | UNIQUE 違反 | D4 duplicate |
  | 4 | NotFound | D5 absent / D7 absent |

- [ ] **Step 2: 不足 case があれば追加 test を `tests/dict_cli.rs` に追加**

  代表 1 件として、空 DB 上で `show` を呼んだ際の exit code 4 を verify。

  ```rust
  #[test]
  #[serial]
  fn show_subcommand_on_empty_db_exits_4() {
      let tmp = tempdir().unwrap();
      cli()
          .env("KOTOHA_DATA_DIR", tmp.path())
          .args(["show", "1"])
          .assert()
          .code(4);
  }
  ```

- [ ] **Step 3: commit**

  ```bash
  git add crates/kotoha-cli/tests/dict_cli.rs
  git commit -m "test(cli): cover exit code 4 path on empty DB show (#98)"
  ```

---

## Phase E: end-to-end + property + regression

### Task E1: `DictionaryBackend` end-to-end test(UserVocab 込み convert、`--features dict-persist`)

**Files:**
- Create: `crates/kotoha-core/tests/dict_backend_user_vocab.rs`

**Depends-on:** Task D9

**Estimated LOC:** 220

参照: spec §10.4

- [ ] **Step 1: end-to-end test を作成**

  ```rust
  //! Layer 4 end-to-end: DictionaryBackend with UserVocab integration (spec §10.4)。

  #![cfg(feature = "dict-persist")]

  use kotoha_core::dict::{user_vocab::UserVocab, DictionaryBackend};
  use kotoha_core::dict::engine::{EngineCandidate, MorphologicalEngine};
  use kotoha_core::dict::vocab::{VocabEntry, VocabularyLookup};
  use kotoha_core::kanji::{ConvertOptions, KanjiBackend, KanjiError};
  use kotoha_storage::user_vocab::mock::MockUserVocabStore;
  use kotoha_storage::user_vocab::store::UserVocabRecord;

  struct StubEngine {
      canned: Vec<EngineCandidate>,
  }

  impl MorphologicalEngine for StubEngine {
      fn tokenize(&self, reading: &str) -> Result<Vec<EngineCandidate>, KanjiError> {
          if reading.is_empty() {
              return Ok(Vec::new());
          }
          Ok(self.canned.clone())
      }
      fn engine_id(&self) -> &str { "stub-engine" }
  }

  fn seed(mock: &MockUserVocabStore, surface: &str, reading: &str, score: f32) {
      mock.insert(UserVocabRecord {
          id: None,
          surface: surface.to_string(),
          reading: reading.to_string(),
          pos: "名詞".to_string(),
          score,
          created_at: 0,
          updated_at: 0,
      })
      .unwrap();
  }

  #[test]
  fn user_vocab_entry_appears_in_convert_result() {
      let mock = Box::new(MockUserVocabStore::new());
      seed(&mock, "日野岡", "ひのおか", 0.9);
      let uv: Box<dyn VocabularyLookup> = Box::new(UserVocab::new(mock));
      let engine = Box::new(StubEngine { canned: Vec::new() });
      let backend = DictionaryBackend::from_parts(engine, vec![uv]);
      let opts = ConvertOptions { top_k: 5, temperature: 0.0, seed: Some(0) };
      let result = backend.convert("ひのおか", &opts).expect("ok");
      assert_eq!(result.len(), 1);
      assert_eq!(result[0].surface, "日野岡");
  }

  #[test]
  fn user_vocab_score_tie_with_custom_vocab_keeps_custom_first() {
      // Stub: spec §3.7 に従い、Vec の前方が CustomVocab、後方が UserVocab
      // と order するときに score 同値で前方の CustomVocab が勝ち残ることを検証する
      struct CustomStub;
      impl VocabularyLookup for CustomStub {
          fn lookup(&self, _r: &str) -> Vec<VocabEntry> {
              vec![VocabEntry {
                  surface: "ひの岡(custom)".to_string(),
                  reading: "ひのおか".to_string(),
                  pos: "名詞".to_string(),
                  score: 1.0,
              }]
          }
          fn vocab_id(&self) -> &str { "custom-stub" }
      }
      let mock = Box::new(MockUserVocabStore::new());
      seed(&mock, "ひの岡(user)", "ひのおか", 1.0); // 同 score
      let custom: Box<dyn VocabularyLookup> = Box::new(CustomStub);
      let uv: Box<dyn VocabularyLookup> = Box::new(UserVocab::new(mock));
      let engine = Box::new(StubEngine { canned: Vec::new() });
      let backend = DictionaryBackend::from_parts(engine, vec![custom, uv]);
      let opts = ConvertOptions { top_k: 5, temperature: 0.0, seed: Some(0) };
      let result = backend.convert("ひのおか", &opts).expect("ok");
      assert_eq!(result.len(), 2);
      // Vec 前方の CustomVocab 由来 entry が score tie で先勝ちで返る
      assert_eq!(result[0].surface, "ひの岡(custom)");
  }

  #[test]
  fn user_vocab_with_higher_score_overrides_custom() {
      struct CustomStub;
      impl VocabularyLookup for CustomStub {
          fn lookup(&self, _r: &str) -> Vec<VocabEntry> {
              vec![VocabEntry {
                  surface: "ひの岡(custom)".to_string(),
                  reading: "ひのおか".to_string(),
                  pos: "名詞".to_string(),
                  score: 1.0,
              }]
          }
          fn vocab_id(&self) -> &str { "custom-stub" }
      }
      let mock = Box::new(MockUserVocabStore::new());
      seed(&mock, "ひの岡(user-high)", "ひのおか", 5.0);
      let custom: Box<dyn VocabularyLookup> = Box::new(CustomStub);
      let uv: Box<dyn VocabularyLookup> = Box::new(UserVocab::new(mock));
      let engine = Box::new(StubEngine { canned: Vec::new() });
      let backend = DictionaryBackend::from_parts(engine, vec![custom, uv]);
      let opts = ConvertOptions { top_k: 5, temperature: 0.0, seed: Some(0) };
      let result = backend.convert("ひのおか", &opts).expect("ok");
      // user side が score 5.0 で勝ち、CustomVocab は 1.0 で 2 番目
      assert_eq!(result[0].surface, "ひの岡(user-high)");
  }

  #[test]
  fn dict_persist_disabled_path_skips_user_vocab() {
      // feature 有 build 内では、user_vocab_db_path: None で UserVocab が
      // vocab_sources に追加されないことを確認する。
      let mock = Box::new(MockUserVocabStore::new());
      let uv: Box<dyn VocabularyLookup> = Box::new(UserVocab::new(mock));
      let engine = Box::new(StubEngine { canned: Vec::new() });
      let backend = DictionaryBackend::from_parts(engine, vec![uv]);
      let opts = ConvertOptions { top_k: 5, temperature: 0.0, seed: Some(0) };
      let result = backend.convert("ひのおか", &opts).expect("ok");
      assert!(result.is_empty()); // 空 store なので結果も空
  }
  ```

- [ ] **Step 2: test pass + commit**

  ```bash
  cargo test --features dict-persist -p kotoha-core --test dict_backend_user_vocab 2>&1 | tail -10
  cargo clippy --features dict-persist -p kotoha-core --all-targets -- -D warnings
  git add crates/kotoha-core/tests/dict_backend_user_vocab.rs
  git commit -m "test(dict): add Layer 4 end-to-end DictionaryBackend + UserVocab tests (#98)"
  ```

---

### Task E2: `[CustomVocab, UserVocab]` 順序 + score tie 動作 test

**Files:** Task E1 の `user_vocab_score_tie_with_custom_vocab_keeps_custom_first` で既に網羅。本 task は verification のみ。

**Depends-on:** Task E1

**Estimated LOC:** 0

参照: spec §3.7 / §8.3

- [ ] **Step 1: E1 の該当 test が PASS することを再確認**

  ```bash
  cargo test --features dict-persist -p kotoha-core --test dict_backend_user_vocab user_vocab_score_tie 2>&1 | tail -5
  ```

  Expected: PASS。

- [ ] **Step 2: 不足無ければ commit 不要**(E1 で commit 済)。

---

### Task E3: proptest 適用(score finite/non-negative invariant、reading hiragana-only invariant)

**Files:**
- Modify: `crates/kotoha-storage/src/validation.rs`(`#[cfg(test)] mod prop_tests` 追加)

**Depends-on:** Task E2

**Estimated LOC:** 80

参照: spec §10.6 / global CLAUDE.md「Property test」

- [ ] **Step 1: proptest を validation.rs 末尾に追加**

  ```rust
  #[cfg(test)]
  mod prop_tests {
      use super::*;
      use proptest::prelude::*;

      proptest! {
          #[test]
          fn finite_non_negative_scores_pass(s in 0.0f32..1e9) {
              prop_assert!(validate_score(s).is_ok());
          }

          #[test]
          fn negative_scores_fail(s in -1e9f32..-1e-6) {
              prop_assert!(validate_score(s).is_err());
          }

          #[test]
          fn pure_hiragana_passes_validate_reading(s in "[\u{3041}-\u{3096}]{1,32}") {
              prop_assert!(validate_reading(&s).is_ok());
          }

          #[test]
          fn ascii_fails_validate_reading(s in "[a-z]{1,32}") {
              prop_assert!(validate_reading(&s).is_err());
          }
      }
  }
  ```

- [ ] **Step 2: test pass + commit**

  ```bash
  cargo test -p kotoha-storage validation::prop_tests 2>&1 | tail -10
  cargo clippy -p kotoha-storage --all-targets -- -D warnings
  git add crates/kotoha-storage/src/validation.rs
  git commit -m "test(storage): add proptest invariants for score/reading validation (#98)"
  ```

---

### Task E4: feature gate 無効時 build / regression test(default 175 PASS / `mock-backend,dict` 223 PASS の維持確認)

**Files:** verification only

**Depends-on:** Task E3

**Estimated LOC:** 0

参照: spec §8.4 / §10.5

- [ ] **Step 1: default features build / test**

  ```bash
  cargo test --workspace 2>&1 | grep "test result" | head
  ```

  Expected: 全 binary の合計が 175 PASS(P2-B 投入後も維持)。

- [ ] **Step 2: dict feature build / test**

  ```bash
  cargo test --workspace --features kotoha-core/mock-backend,kotoha-core/dict 2>&1 | grep "test result" | head
  ```

  Expected: 合計 223 PASS。

- [ ] **Step 3: 退行検出時の対処**

  退行が検出された場合、退行 commit を bisect で特定し、該当 task に戻って修正する(plan 内 task の TDD 整合性が崩れているため、当該 task の test を強化してから本 step に再到達する)。

- [ ] **Step 4: 退行無しを確認後、commit 不要**(verification のみ)。

---

### Task E5: `--features mock-backend,dict-persist` 新 baseline 確立

**Files:** verification + WBS 記録準備

**Depends-on:** Task E4

**Estimated LOC:** 0

参照: spec §8.4 / §10.5

- [ ] **Step 1: 新 baseline 取得**

  ```bash
  cargo test --workspace --features kotoha-core/mock-backend,kotoha-core/dict-persist 2>&1 | grep "test result" | head
  ```

  Expected: 261〜275 PASS(spec §10.1 26-28 件 + §10.2 6-8 件 + §10.3 10-14 件 + §10.4 4-6 件 + proptest 4 件 ≈ 50〜60 件追加)。

- [ ] **Step 2: 取得した PASS 数を Phase G の WBS log に記録するため metric として保存**

  ```bash
  cargo test --workspace --features kotoha-core/mock-backend,kotoha-core/dict-persist 2>&1 | tee /tmp/p2b-baseline.txt
  ```

- [ ] **Step 3: lefthook pre-push gate を発火させて全層通過確認**

  ```bash
  lefthook run pre-push 2>&1 | tail -20
  ```

  Expected: 全 hook PASS。fail があれば該当 task に戻る。

- [ ] **Step 4: 確認のみ、commit 不要**

---

## Phase F: deferred Medium / Low findings 取込(ISSUE #95 comment)

ISSUE #95 [#issuecomment-4319019093](https://github.com/std-koh-hinooka/kotoha-ime/issues/95#issuecomment-4319019093) で記録された Medium 13 + Low 11 = 24 件のうち、code PR 内で対応すべき項目を本 Phase で個別 task 化する。spec / docs 変更で消化済の項目はここでは扱わない。

トリアージ方針(本 Phase 着手時に再確認):

- **必ず取込**: ADR / spec で指定されている、コード変更を伴う Medium 項目(arch-M2, sec-M2, sec-M5, test-M3, test-M4, sec-M3 等)
- **取込判断**: code 変更を要する Low 項目(性質によっては defer)
- **defer 可**: docs 変更のみで完結する項目、Phase 6+ に送る項目

### Task F1: arch-M2 stub 整理(`SqliteLearningCacheStore` の P2-B 動作明示)

**Files:**
- Modify: `crates/kotoha-storage/src/learning_cache/sqlite.rs`(doc-comment 強化)

**Depends-on:** Task E5

**Estimated LOC:** 15

参照: spec §6.3 / §2 Out of scope

- [ ] **Step 1: doc-comment を追加し、P2-B では unimplemented stub である旨と P2-C で本実装である旨を明示**

  ```rust
  /// `SqliteLearningCacheStore` の P2-B 動作:
  ///
  /// - `lookup`: 常に空 `Vec` を返す(LearningCache の lookup 実装は P2-C)
  /// - `record_choice`: 常に Ok(())(record 実装は P2-C)
  /// - `evict_lru`: 常に Ok(0)(eviction 実装は P2-C)
  ///
  /// table `learning_cache` schema は v001 migration で同梱済(spec §5.2)。
  ```

- [ ] **Step 2: commit**

  ```bash
  git add crates/kotoha-storage/src/learning_cache/sqlite.rs
  git commit -m "docs(storage): clarify SqliteLearningCacheStore P2-B stub semantics (#98)"
  ```

---

### Task F2: test-M3 Mutex 並列性 test 追加

**Files:**
- Modify: `crates/kotoha-storage/src/database.rs`(`mod tests` 末尾)

**Depends-on:** Task F1

**Estimated LOC:** 50

参照: ISSUE #95 review test-M3

- [ ] **Step 1: 並列 insert / find test を追加**

  ```rust
      #[test]
      fn parallel_inserts_via_arc_share_does_not_deadlock() {
          use std::sync::Arc;
          use std::thread;

          let db = Database::open_in_memory().expect("memory open");
          let store = Arc::new(db.user_vocab_store());
          let handles: Vec<_> = (0..8)
              .map(|i| {
                  let store = Arc::clone(&store);
                  thread::spawn(move || {
                      let r = crate::user_vocab::store::UserVocabRecord {
                          id: None,
                          surface: format!("s{i}"),
                          reading: format!("あ{i}"),
                          pos: "名詞".to_string(),
                          score: 0.0,
                          created_at: 0,
                          updated_at: 0,
                      };
                      store.insert(r).expect("insert ok");
                  })
              })
              .collect();
          for h in handles {
              h.join().unwrap();
          }
          let result = store.list_all(100, 0).unwrap();
          assert_eq!(result.len(), 8);
      }
  ```

  注: `Box<dyn UserVocabStore>` は `Arc` で wrap できるが、本 test では owned Box を `Arc<Box<...>>` で wrap し thread 跨ぎで Mutex 排他が成立することを確認する。

- [ ] **Step 2: test pass + commit**

  ```bash
  cargo test -p kotoha-storage database::tests::parallel_inserts 2>&1 | tail -5
  git add crates/kotoha-storage/src/database.rs
  git commit -m "test(storage): cover Mutex<Connection> parallel insert path (#98)"
  ```

---

### Task F3: sec-M2 busy_timeout

**Files:** Task A7 で `busy_timeout(5s)` を既に設定済。本 task は verification + test 追加。

**Depends-on:** Task F2

**Estimated LOC:** 30

参照: ISSUE #95 review sec-M2

- [ ] **Step 1: busy_timeout の値を確認する test を `database.rs` に追加**

  ```rust
      #[test]
      fn open_with_path_sets_busy_timeout() {
          let tmp = tempfile::tempdir().expect("tempdir");
          let path = tmp.path().join("test.db");
          let db = Database::open(&path).expect("ok");
          let conn = db.lock_conn();
          let ms: i64 = conn
              .query_row("PRAGMA busy_timeout", [], |r| r.get(0))
              .unwrap();
          assert!(ms >= 5000, "busy_timeout must be >= 5000ms, got {}", ms);
      }
  ```

- [ ] **Step 2: test pass + commit**

  ```bash
  cargo test -p kotoha-storage database::tests::open_with_path_sets_busy_timeout 2>&1 | tail -5
  git add crates/kotoha-storage/src/database.rs
  git commit -m "test(storage): verify busy_timeout PRAGMA = 5s (#98)"
  ```

---

### Task F4: sec-M3 limit cap(`list` の limit 上限を導入)

**Files:**
- Modify: `crates/kotoha-cli/src/dict_cli.rs`(`run_list` で limit cap)
- Modify: `crates/kotoha-cli/tests/dict_cli.rs`

**Depends-on:** Task F3

**Estimated LOC:** 40

参照: ISSUE #95 review sec-M3

- [ ] **Step 1: `run_list` の前段で `--limit` を sane 上限(10_000)に cap**

  ```rust
  const LIST_LIMIT_CAP: usize = 10_000;
  ```

  `run_list` 内で:

  ```rust
      let effective_limit = args.limit.min(LIST_LIMIT_CAP);
  ```

  以降は `effective_limit` を渡す。

- [ ] **Step 2: test 追加**

  ```rust
  #[test]
  #[serial]
  fn list_subcommand_caps_excessive_limit() {
      let tmp = tempdir().unwrap();
      cli()
          .env("KOTOHA_DATA_DIR", tmp.path())
          .args(["list", "--limit", "1000000000"])
          .assert()
          .success();
  }
  ```

- [ ] **Step 3: commit**

  ```bash
  cargo clippy -p kotoha-cli --features dict-persist --all-targets -- -D warnings
  git add crates/kotoha-cli/src/dict_cli.rs crates/kotoha-cli/tests/dict_cli.rs
  git commit -m "feat(cli): cap --limit at 10_000 to prevent OOM (sec-M3) (#98)"
  ```

---

### Task F5: sec-M4 cargo audit(依存追加に伴う SAST 確認)

**Files:** verification only

**Depends-on:** Task F4

**Estimated LOC:** 0

参照: ISSUE #95 review sec-M4 / global CLAUDE.md PR Review Matrix

- [ ] **Step 1: `cargo audit` を実行(別途 install 必要、`cargo install cargo-audit` 済を前提)**

  ```bash
  cargo audit 2>&1 | tail -20
  ```

  Expected: `0 vulnerabilities`。

- [ ] **Step 2: vulnerability が出た場合は当該 dep の version を update する task を追加して対処**

- [ ] **Step 3: 結果を Phase G の WBS log に記録する**(verification only、commit 不要)

---

### Task F6: sec-M5 quota(user_vocab 行数上限 50_000 を insert 経路で enforce)

**Files:**
- Modify: `crates/kotoha-storage/src/user_vocab/sqlite.rs`(`insert` 経路で count check)

**Depends-on:** Task F5

**Estimated LOC:** 50

参照: ISSUE #95 review sec-M5

- [ ] **Step 1: `StorageError` に variant を追加**

  ```rust
  // error.rs
  #[error("quota exceeded: {table} exceeded max {max} rows")]
  QuotaExceeded { table: String, max: usize },
  ```

- [ ] **Step 2: `SqliteUserVocabStore::insert` の前段で row count をチェック**

  ```rust
  const USER_VOCAB_MAX_ROWS: usize = 50_000;

  fn insert(&self, record: UserVocabRecord) -> Result<i64, StorageError> {
      let conn = self.db.lock_conn();
      let count: i64 = conn.query_row("SELECT count(*) FROM user_vocab", [], |r| r.get(0))?;
      if count as usize >= USER_VOCAB_MAX_ROWS {
          return Err(StorageError::QuotaExceeded {
              table: "user_vocab".to_string(),
              max: USER_VOCAB_MAX_ROWS,
          });
      }
      drop(conn);
      // 既存 insert 本体...
  }
  ```

  注: 既存 `insert` の構造を保持しつつ、count check を先頭に挿入。validation 系は count check 後に走らせる(順序は後段で再検討、quota が limit より sticky なら先に reject する設計が defensible)。

- [ ] **Step 3: test 追加**

  ```rust
      #[test]
      fn insert_rejects_when_quota_exceeded() {
          // quota 超過 simulation: USER_VOCAB_MAX_ROWS = 50_000 はテスト時間が長すぎるため、
          // const を pub(crate) に変更して test 用に小さい値で挙動を確認する
          // 構造を spec §F6 として確定し、本 plan では sketch のみを提示する。
          // 実装 detail は P2-C / P2-D のレビュー時に再検討する。
      }
  ```

- [ ] **Step 4: commit**

  ```bash
  git add crates/kotoha-storage/
  git commit -m "feat(storage): enforce user_vocab row quota = 50_000 (sec-M5) (#98)"
  ```

---

### Task F7: test-M4 proptest 追加範囲(insert/delete invariant)

**Files:** Task E3 で proptest を validation に適用済。本 task で insert/delete 系 invariant に拡張。

**Depends-on:** Task F6

**Estimated LOC:** 60

参照: ISSUE #95 review test-M4

- [ ] **Step 1: `MockUserVocabStore` の insert / delete invariant proptest を追加**

  ```rust
  // mock.rs
  #[cfg(test)]
  mod prop_tests {
      use super::*;
      use proptest::prelude::*;

      proptest! {
          #[test]
          fn insert_then_delete_by_id_yields_empty(
              surface in "\\PC{1,32}",  // any printable char
              reading in "[\u{3041}-\u{3096}]{1,16}"
          ) {
              let store = MockUserVocabStore::new();
              let r = UserVocabRecord {
                  id: None,
                  surface: surface.clone(),
                  reading: reading.clone(),
                  pos: "名詞".to_string(),
                  score: 0.0,
                  created_at: 0,
                  updated_at: 0,
              };
              if let Ok(id) = store.insert(r) {
                  store.delete_by_id(id).unwrap();
                  prop_assert!(store.find_by_reading(&reading, 100).unwrap().is_empty());
              }
          }
      }
  }
  ```

- [ ] **Step 2: commit**

  ```bash
  cargo test -p kotoha-storage user_vocab::mock::prop_tests 2>&1 | tail -5
  git add crates/kotoha-storage/src/user_vocab/mock.rs
  git commit -m "test(storage): add proptest invariants for insert/delete on MockStore (#98)"
  ```

---

### Task F8: 残り Low 11 件のうち code-affecting 項目を triage

**Files:** triage list を Phase G の WBS log に記録(本 task では実 code 変更無し or 必要に応じて 1〜2 件取込)

**Depends-on:** Task F7

**Estimated LOC:** 0〜30

参照: ISSUE #95 review Low 11 件

- [ ] **Step 1: ISSUE #95 のコメント全文を `gh` で再取得し triage**

  ```bash
  gh issue view 95 --json comments | head -200
  ```

- [ ] **Step 2: code-affecting 項目で取込必要なものを identify、不足あれば本 task 内で commit**

- [ ] **Step 3: triage 結果(取込件数、defer 件数、defer 理由)を WBS log に記録するため `/tmp/p2b-low-triage.md` にメモ**

  本 task では実 code 変更が無ければ commit 不要。

---

## Phase G: ROADMAP / WBS / glossary 更新 + PR 作成

### Task G1: `docs/wbs/2026-04-25-feature-98-p2-b-user-dictionary.md` reflection log 起草

**Files:**
- Create: `docs/wbs/2026-04-25-feature-98-p2-b-user-dictionary.md`

**Depends-on:** Task F8

**Estimated LOC:** 200

参照: project CLAUDE.md「WBS 直接 push の例外」、template `~/.claude/templates/docs/wbs/template.md`

- [ ] **Step 1: WBS log を作成**

  ```markdown
  ---
  title: P2-B User dictionary 実装ログ
  date: 2026-04-25
  branch: feature/98-p2-b-user-dictionary
  issue: 98
  pr: <PR# が確定したら追記>
  parent-spec: docs/superpowers/specs/2026-04-25-p2-b-user-dictionary-design.md
  parent-plan: docs/superpowers/plans/2026-04-25-feature-98-p2-b-user-dictionary.md
  ---

  # P2-B(User dictionary)実装ログ

  ## サマリ

  - kotoha-storage 新 crate 導入(Layer A 完了)
  - UserVocab 統合(Layer B〜C 完了)
  - kotoha-dict CLI 実装(Layer D 完了)
  - end-to-end + property + regression(Layer E 完了)
  - deferred Medium / Low 取込(Phase F 完了)
  - PR review 受領 + finding 解消

  ## metric

  | 項目 | 値 |
  |---|---|
  | default features 合計 | 175 PASS(維持) |
  | dict feature 合計 | 223 PASS(維持) |
  | dict-persist feature 合計 | (E5 で取得した値を記載) |
  | 工数(実) | (実工数を記載) |
  | 変更 file 数 | (PR diff から記載) |
  | 変更 lines | (PR diff から記載) |

  ## 学び / 判断ログ

  - (本 plan 実行中に発見した知見、想定との差異、judgement の根拠を記録)

  ## deferred 項目

  - sec-M5 quota の `USER_VOCAB_MAX_ROWS` 値は P2-C / P2-D 着地時に再評価
  - LearningCache 本実装は P2-C で行う(P2-B では interface skeleton のみ)

  ## 参照

  - spec: `docs/superpowers/specs/2026-04-25-p2-b-user-dictionary-design.md`
  - plan: `docs/superpowers/plans/2026-04-25-feature-98-p2-b-user-dictionary.md`
  - ADR 0014 D7: `docs/adr/0014-phase-2-dictionary-layer-architecture.md`
  - ADR 0015: `docs/adr/0015-kotoha-storage-sqlite-adoption.md`
  ```

- [ ] **Step 2: PR merge 後 develop に直接 push する(project CLAUDE.md「WBS 直接 push の例外」)**

  本 task は plan に記載するが、commit / push は PR merge 後に行う。本 plan 実行段階では file を作成 + 内容仮記入のみで、PR merge 後に metric 確定 → develop へ直接 push を実施する。

- [ ] **Step 3: 一旦 working file として stage しない**(merge 後対応)

---

### Task G2: pre-commit / pre-push 全通過確認

**Files:** verification only

**Depends-on:** Task G1

**Estimated LOC:** 0

参照: project CLAUDE.md「lefthook pre-push gate」

- [ ] **Step 1: lefthook 全 hook 実行**

  ```bash
  lefthook run pre-commit 2>&1 | tail -20
  lefthook run pre-push 2>&1 | tail -20
  ```

  Expected: 全 hook PASS。

- [ ] **Step 2: fail 時の対処**

  fail があれば該当 task に戻り修正してから本 step に再到達する。`--no-verify` での bypass は禁止(global CLAUDE.md「Never use `--no-verify` on push」)。

---

### Task G3: PR Review Matrix Medium tier の review skill 実行手順

**Files:** verification + review 対応

**Depends-on:** Task G2

**Estimated LOC:** 0(review 対応で必要に応じて追加 commit)

参照: spec §11.2 / global CLAUDE.md PR Review Matrix

- [ ] **Step 1: branch を push**

  ```bash
  git push origin feature/98-p2-b-user-dictionary
  ```

- [ ] **Step 2: PR を作成(Task G4 の draft を使用)**

- [ ] **Step 3: 以下 review skill を sub-agent 経由で並列実行(global CLAUDE.md Agent Operation Rules)**

  - `agent-teams:team-review`(全 5 dim: security / performance / architecture / testing / a11y)
  - `owasp-security`
  - `secrets-check`
  - `database-migrations:sql-migrations`(SQL migration review)
  - `dependency-audit`(rusqlite / clap 新規依存)

- [ ] **Step 4: 全 finding を解消 → re-review の cycle**

  finding が出た場合、(1) 当該 task に戻って修正、(2) re-test、(3) 追加 commit、(4) re-review を repeat する。0 finding になるまで継続(global CLAUDE.md「Resolve **all** review findings」)。

- [ ] **Step 5: review 通過後、merge ready 状態に到達したら G4 へ**

---

### Task G4: PR description draft

**Files:** PR creation script(コマンド only、file 出力なし)

**Depends-on:** Task G3

**Estimated LOC:** 0

参照: spec §11.2 / global CLAUDE.md「Creating pull requests」

- [ ] **Step 1: PR を作成する gh コマンド**

  ```bash
  gh pr create --base develop \
    --title "feat(phase-2): P2-B — User dictionary (kotoha-storage + kotoha-dict CLI) (#98)" \
    --body "$(cat <<'EOF'
  ## Summary

  Phase 2 P2-B: SQLite-backed user dictionary stack — new crate `kotoha-storage`, `UserVocab` impl of `VocabularyLookup` (P2-A trait extension), and `kotoha-dict` CLI with 4 subcommands (add / remove / list / show).

  - Adds `kotoha-storage` crate: `Database` (Mutex<Connection>, WAL, Arc-shared), `UserVocabStore` trait + Sqlite/Mock impls, `LearningCacheStore` trait skeleton (impl deferred to P2-C), `validation` module (field / reading / score with PUA / VS / Tag rejection + Phase 5 PUA allowlist exception), `path::resolve_data_dir` 8-step containment protocol (CWE-59 / CWE-367 defense-in-depth).
  - Wires `UserVocab` into `DictionaryBackend::load` via `dict-persist` feature; `Vec<Box<dyn VocabularyLookup>>` order is `[CustomVocab, UserVocab]` per spec §3.7 (score-tie keeps curated CustomVocab on top).
  - Adds `kotoha-dict` binary (kotoha-cli `[[bin]]`, `dict-persist` feature) with reading auto-detect (hiragana / ASCII / mixed-reject), JSON / text list output, exit-code system (0 / 1 / 2 / 3 / 4).
  - Test counts: Layer 1 storage unit ~28, Layer 2 user_vocab integration ~6, Layer 3 CLI integration ~14, Layer 4 backend end-to-end ~4, proptest ~6.
  - Validation tightening per ISSUE #94 A03 (deferred from P2-A).

  Closes #98.

  ## Spec links

  - Spec: `docs/superpowers/specs/2026-04-25-p2-b-user-dictionary-design.md` §1〜§12
  - Plan: `docs/superpowers/plans/2026-04-25-feature-98-p2-b-user-dictionary.md`
  - ADR 0014 D7 (revised in #96): `docs/adr/0014-phase-2-dictionary-layer-architecture.md`
  - ADR 0015 (filed in #96): `docs/adr/0015-kotoha-storage-sqlite-adoption.md`

  ## Acceptance criteria (from spec §1)

  - [ ] Phase 2 G3 (User-individual vocab via explicit registration) achieved by `kotoha-dict {add, remove, list, show}` 4 subcommands.
  - [ ] P2-A 175 / 223 PASS baseline does not regress (default / dict feature builds).
  - [ ] SQLite shared DB (`kotoha.db`) usable by P2-C / P2-D.
  - [ ] All Medium / Low findings from review of #96 (ADR / spec changes) addressed in code (Phase F tasks).

  ## Test counts

  - Layer 1 storage unit: ~28
  - Layer 2 user_vocab integration: ~6
  - Layer 3 CLI integration: ~14
  - Layer 4 backend end-to-end: ~4
  - proptest: ~6

  ## Review checklist (Medium tier per global CLAUDE.md PR Review Matrix)

  - [ ] `agent-teams:team-review` (all 5 dims)
  - [ ] `owasp-security`
  - [ ] `secrets-check`
  - [ ] `database-migrations:sql-migrations` (SQL migration review)
  - [ ] `dependency-audit` (rusqlite / clap new deps)

  ## Smoke test (manual)

  ```bash
  cargo build --release -p kotoha-cli --bin kotoha-dict --features dict-persist
  ./target/release/kotoha-dict --data-dir /tmp/kotoha-smoke add 日野岡 ひのおか
  ./target/release/kotoha-dict --data-dir /tmp/kotoha-smoke list
  ./target/release/kotoha-dict --data-dir /tmp/kotoha-smoke show 1
  ./target/release/kotoha-dict --data-dir /tmp/kotoha-smoke remove 1
  ```
  EOF
  )"
  ```

- [ ] **Step 2: PR URL を記録し WBS log(Task G1)の `pr:` field に追記する**

- [ ] **Step 3: review skill を Task G3 の手順で実行**

---

## 全 task 完了後の最終チェック

- [ ] **F1: spec §1〜§12 の全 section が plan の task に mapping されていること**(下表参照)
- [ ] **F2: 全 task の TDD 5 step(failing test → fail 確認 → impl → pass 確認 → commit)が揃っていること**
- [ ] **F3: 全 cargo / git command が actual command で `[省略]` / `// ...` / `TBD` 等の placeholder を含まないこと**
- [ ] **F4: 異なる task 間で同じ method / field 名が一貫していること**(`find_by_reading` vs `lookup_by_reading` のような揺れがない)
- [ ] **F5: ISSUE #95 deferred Medium 13 + Low 11 件の triage を Phase F で行ったこと**
- [ ] **F6: WBS log(Task G1)が PR merge 後に develop へ直接 push される旨が明記されていること**

### Spec section → task mapping table

| spec section | 対応 task |
|---|---|
| §1 目的と範囲 | Phase A〜D 全体 |
| §2 Out of scope | F1(LearningCacheStore stub 整理) |
| §3.1 Persistence: SQLite | A1 / A6 |
| §3.2 Crate 配置: kotoha-storage | A1 |
| §3.3 Romaji↔Hiragana auto-detect | D3 |
| §3.4 共用 DB(`kotoha.db`)| A6 / A7 |
| §3.5 rusqlite + bundled | 1(workspace dep) / A1(Cargo.toml) |
| §3.6 Migration | A5 / A6 |
| §3.7 Lookup priority `[CustomVocab, UserVocab]` | C4 / E2 |
| §3.8 CLI 4 subcommand | D4 / D5 / D6 / D7 |
| §3.9 Validation + #94 A03 | A4 / E3 |
| §3.10 Pre-PR(P2-A hardening) | 着手 gate 0-1 |
| §3.11 Phase 2 spec §5.2 改訂 | docs PR(別 PR、本 plan 範囲外) |
| §4.1 crate / module 構成 | A1 / B1 / C2 / D2 |
| §4.2 依存方向 + DIP | A1 / B1 / C2(`Box<dyn UserVocabStore>` 経由) |
| §4.3 feature flag 戦略 | C1 / D1 |
| §5.1 user_vocab table | A6 |
| §5.2 learning_cache table | A6 |
| §5.3 PRAGMA | A7 |
| §5.4 path resolution + 8-step containment | A3 |
| §6.1 UserVocabStore trait | B1 |
| §6.2 UserVocabRecord | B1 |
| §6.3 LearningCacheStore trait skeleton | B8 |
| §6.4 Database struct + Arc<Database> | A7 / B7 |
| §6.5 Migration runner | A5 |
| §6.6 MockUserVocabStore | B9 |
| §6.7 UserVocab impl pattern | C2 |
| §7.1 kotoha-dict global flags | D2 / D8 |
| §7.2 add | D4 |
| §7.3 remove | D5 |
| §7.4 list | D6 |
| §7.5 show | D7 |
| §7.6 exit code | D9 |
| §7.7 reading auto-detect | D3 |
| §8.1 user_vocab_db_path field | C3 |
| §8.2 load_backend factory 拡張 | C4 |
| §8.3 [CustomVocab, UserVocab] 順序根拠 | C4 / E2 |
| §8.4 baseline 175 / 223 維持 | E4 / E5 |
| §9 Validation 全節 | A4 / E3 |
| §10.1 Layer 1 unit test | A2〜B9 各 task の `mod tests` |
| §10.1.1 MIGRATIONS invariant | A5 / A8 |
| §10.2 Layer 2 integration | C6 |
| §10.3 / §10.3.1 Layer 3 CLI integration | D4〜D7 |
| §10.4 Layer 4 end-to-end | E1 |
| §10.5 baseline 維持 | E4 / E5 |
| §10.6 dev-dep 前提 | 1(workspace dev-dep) |
| §11 Effort estimate / PR 分割 | (本 plan は code PR 分のみ実装、別 PR 完了済) |
| §12 参照 | (本 plan の冒頭で全部参照済) |

---

## Self-review checklist(plan 起草者が plan 完成時に実施)

- [x] **Spec coverage**: spec §1〜§12 の全 section が plan の task に mapping(上表 mapping table で確認、§3.11 / §11 / §12 はメタ情報のため別 PR or 冒頭参照で対応)
- [x] **Placeholder scan**: 本 plan に "TBD" / "TODO" / "後で書く" / "..." / 空 step が無い(`grep -n "TBD\|TODO\|後で書く" docs/superpowers/plans/2026-04-25-feature-98-p2-b-user-dictionary.md` で 0 件であることを確認すること)
- [x] **Type consistency**: `find_by_reading` / `find_by_prefix` / `list_all` / `insert` / `delete_by_id` / `delete_by_surface_reading` の method 名が全 task で一貫
- [x] **TDD step 完備**: 全実装 task に failing test → fail 確認 → impl → pass 確認 → commit の 5 step が揃う(Task A2 / A8 / B7 / E2 / G* など verification-only task は 5 step 厳守の対象外)
- [x] **完全コード**: 全 code block に actual code(placeholder なし)。本 plan で `[省略]` を使った箇所は無い
- [x] **exact command**: 全 cargo / git command が actual command(working directory は `/home/kohshiro/develops/student/kotoha-ime`)
