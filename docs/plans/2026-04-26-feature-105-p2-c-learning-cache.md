# Phase 2-C — LearningCache 本実装 + arch-M-2 ISP split 実装計画

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** P2-B で skeleton として配置した `LearningCacheStore` の 3 method を `SqliteLearningCacheStore` に本実装し、同時に arch-M-2(ISP split: Reader/Writer 分離)を消化する。

**Architecture:** kotoha-storage 内で `UserVocabStore` / `LearningCacheStore` を 4 trait(`*Reader` / `*Writer`)に分割、`Database` factory を 4 method 化、`SqliteLearningCacheStore` に UPSERT(`ON CONFLICT DO UPDATE`)+ 自動 LRU eviction(`evict_to_cap` 共通 helper)+ exact-match lookup を実装し、v002 migration で `idx_learning_cache_last_used` を追加する。Branch Scope Policy 20 file / 1000 line 上限を意識した単一 PR を目指す。

**Tech Stack:** Rust 1.80(edition 2021)、rusqlite 0.32(bundled)、proptest、tempfile、Cargo workspace、lefthook、cargo-audit

---

## 全体構成

| Phase | 内容 | 推定 | task 数 |
|---|---|---|---|
| P2-C-A | UserVocab ISP split 先行(機械換装、退行ゼロ) | ~1 day | 8 |
| P2-C-B | LearningCache 本実装(UPSERT + auto eviction + lookup) | ~1 day | 11 |
| P2-C-C | v002 migration + idx_learning_cache_last_used | 半日 | 3 |
| P2-C-D | test 整備(L1 23 unit + L2 3 integration + proptest 3 invariant) | ~1 day | 4 |
| P2-C-E | review fix + WBS + PR | 半日〜1 day | 7 |

合計 33 task、~3-4 day。Branch Scope Policy 20 file / 1000 line 内に収める。

## File 構造マッピング(Phase 着地後)

**Modify:**
- `crates/kotoha-storage/src/user_vocab/store.rs`(trait split: `UserVocabStore` → `UserVocabReader` + `UserVocabWriter`)
- `crates/kotoha-storage/src/user_vocab/sqlite.rs`(両 trait impl 分離)
- `crates/kotoha-storage/src/user_vocab/mock.rs`(両 trait impl 分離)
- `crates/kotoha-storage/src/learning_cache/mod.rs`(trait split: `LearningCacheStore` → `LearningCacheReader` + `LearningCacheWriter`)
- `crates/kotoha-storage/src/learning_cache/sqlite.rs`(stub → 本実装)
- `crates/kotoha-storage/src/database.rs`(factory 2 → 4 method、旧 method 削除)
- `crates/kotoha-storage/src/migrations.rs`(MIGRATIONS array に v002 追加、LATEST_VERSION = 2)
- `crates/kotoha-core/src/dict/user_vocab.rs`(依存 trait を `UserVocabStore` → `UserVocabReader` に変更)
- `crates/kotoha-cli/src/dict_cli.rs`(依存 trait を `UserVocabStore` → `UserVocabReader` + `UserVocabWriter` に変更)
- `crates/kotoha-cli/src/bin/dict.rs`(factory 呼出し `user_vocab_store()` → `user_vocab_reader()` / `user_vocab_writer()` に変更)
- `crates/kotoha-core/tests/dict_user_vocab.rs`(factory/trait 変更に追従)
- `crates/kotoha-cli/tests/dict_cli.rs`(factory 変更に追従)

**Create:**
- `crates/kotoha-storage/migrations/v002_learning_cache_index.sql`
- `crates/kotoha-storage/tests/learning_cache_store.rs`(L2 integration、新規)
- `crates/kotoha-storage/tests/learning_cache_proptest.rs`(proptest、新規)
- `docs/wbs/2026-04-26-feature-105-p2-c-learning-cache.md`(WBS、P2-C-E で作成)

## 共通実装規約

- **TDD**: red → green → commit を最小単位とする
- **commit boundary**: 1 task 1 commit 原則(step 内で複数 commit が必要な場合は step 内で明示)
- **test 退行禁止**: 各 phase の最後で `cargo test --workspace --features kotoha-core/mock-backend,kotoha-core/dict-persist` を実行し全 PASS 確認
- **lefthook gate**: phase 切替時に `lefthook run pre-push --commands rust-test-default,rust-test-dict,rust-test-dict-persist` で 3 構成 PASS を確認
- **doc comment**: 新設 pub trait method には Preconditions / Postconditions / Errors を `///` で記載(global CLAUDE.md `lang-rust.md` 厳守)
- **subagent dispatch 時の verbatim output 要求**: 大規模 task を subagent に dispatch する場合、subagent prompt で「最終 verification command の出力 head/tail 5 行を verbatim で報告」を必須化(global CLAUDE.md「Sub-agent Self-Report is Untrusted」)
- **commit message 言語**: 英語(global CLAUDE.md 例外、Kotoha プロジェクト規約)、issue ref `Refs: #105` を末尾付与

---

## Phase P2-C-A: UserVocab ISP split

ISP split は LearningCache 本実装の前提条件である。`UserVocabStore` を `UserVocabReader` / `UserVocabWriter` の 2 trait に分割し、全 consumer を書換える。本 Phase 完了時点でビルドエラーゼロ・退行ゼロを必須とする。

### Task A1: `UserVocabReader` / `UserVocabWriter` trait 分割(`store.rs`)

**Files:**
- Modify: `crates/kotoha-storage/src/user_vocab/store.rs`

- [ ] **Step 1: 旧 `UserVocabStore` trait 全体を削除し、`UserVocabReader` / `UserVocabWriter` の 2 trait と `UserVocabRecord` struct に置換する**

`crates/kotoha-storage/src/user_vocab/store.rs` の全内容を以下に置換する。

```rust
//! `UserVocabReader` / `UserVocabWriter` trait + `UserVocabRecord`
//! (arch-M-2 ISP split、spec §3.2 / §6.1 / §6.2)。

use crate::error::StorageError;

/// User vocabulary の read 操作を提供する抽象境界。
///
/// # Preconditions
///
/// - `reading` はひらがな canonical(`U+3040..=U+309F + U+30FC + U+30FB`)
///
/// # Postconditions
///
/// - `find_by_reading` は `score DESC` 順で最大 `limit` 件返す
/// - `find_by_prefix` は `score DESC` 順で最大 `limit` 件返す
/// - `find_by_id` は不在の場合 `Ok(None)` を返す(エラーではない)
///
/// # Errors
///
/// - [`StorageError::InvalidField`]: `reading` が validation 違反の場合
/// - [`StorageError::Sqlite`]: SQLite backend 障害の場合
pub trait UserVocabReader: Send + Sync {
    /// `reading` が完全一致する entry を `score DESC` 順で返す。
    ///
    /// # Preconditions
    ///
    /// - `reading` はひらがな canonical
    /// - `limit` は呼び出し元が必要な件数の上限を示す
    ///
    /// # Errors
    ///
    /// - [`StorageError::InvalidField`]: `reading` が hiragana 以外を含む場合
    /// - [`StorageError::Sqlite`]: SQLite backend 障害の場合
    fn find_by_reading(
        &self,
        reading: &str,
        limit: usize,
    ) -> Result<Vec<UserVocabRecord>, StorageError>;

    /// 主キー `id` で entry 1 件を引く。
    ///
    /// # Postconditions
    ///
    /// - 不在の場合は `Ok(None)` を返す
    ///
    /// # Errors
    ///
    /// - [`StorageError::Sqlite`]: SQLite backend 障害の場合
    fn find_by_id(&self, id: i64) -> Result<Option<UserVocabRecord>, StorageError>;

    /// `reading` が `reading_prefix` で前方一致する entry を `score DESC` 順で返す。
    ///
    /// SQL `reading LIKE 'PREFIX%'` を使用する。
    ///
    /// # Preconditions
    ///
    /// - `reading_prefix` はひらがな canonical
    ///
    /// # Errors
    ///
    /// - [`StorageError::InvalidField`]: `reading_prefix` が validation 違反の場合
    /// - [`StorageError::Sqlite`]: SQLite backend 障害の場合
    fn find_by_prefix(
        &self,
        reading_prefix: &str,
        limit: usize,
    ) -> Result<Vec<UserVocabRecord>, StorageError>;

    /// 全 entry を `id ASC` 順でページネーション付きで返す。
    ///
    /// # Errors
    ///
    /// - [`StorageError::Sqlite`]: SQLite backend 障害の場合
    fn list_all(&self, limit: usize, offset: usize) -> Result<Vec<UserVocabRecord>, StorageError>;
}

/// User vocabulary の write 操作を提供する抽象境界。
///
/// # Preconditions
///
/// - `record.surface` / `record.reading` / `record.pos` / `record.score` は各 validation を通過
///
/// # Postconditions
///
/// - `insert` 成功時は採番された `id` を返す
/// - UNIQUE(surface, reading) 違反は [`StorageError::DuplicateEntry`]
/// - `delete_by_id` / `delete_by_surface_reading` の対象不在は [`StorageError::NotFound`]
///
/// # Errors
///
/// - [`StorageError::InvalidField`]: validation 違反の場合
/// - [`StorageError::DuplicateEntry`]: UNIQUE 制約違反の場合
/// - [`StorageError::NotFound`]: 削除対象が存在しない場合
/// - [`StorageError::Sqlite`]: SQLite backend 障害の場合
pub trait UserVocabWriter: Send + Sync {
    /// `record` を `user_vocab` table に insert し、採番された `id` を返す。
    ///
    /// # Preconditions
    ///
    /// - `record.surface` / `record.reading` / `record.pos` は各 validate_* を通過
    /// - `record.score` は finite + non-negative
    ///
    /// # Postconditions
    ///
    /// - 戻り値は `AUTOINCREMENT` で採番された `id`
    ///
    /// # Errors
    ///
    /// - [`StorageError::InvalidField`]: field validation 違反の場合
    /// - [`StorageError::DuplicateEntry`]: UNIQUE(surface, reading) 違反の場合
    /// - [`StorageError::QuotaExceeded`]: 行数が `USER_VOCAB_MAX_ROWS` に到達している場合
    /// - [`StorageError::Sqlite`]: SQLite backend 障害の場合
    fn insert(&self, record: UserVocabRecord) -> Result<i64, StorageError>;

    /// 主キー `id` で entry を 1 件削除する。
    ///
    /// # Errors
    ///
    /// - [`StorageError::NotFound`]: `id` が存在しない場合
    /// - [`StorageError::Sqlite`]: SQLite backend 障害の場合
    fn delete_by_id(&self, id: i64) -> Result<(), StorageError>;

    /// `(surface, reading)` で entry を 1 件削除する。
    ///
    /// # Errors
    ///
    /// - [`StorageError::NotFound`]: 対象 entry が存在しない場合
    /// - [`StorageError::Sqlite`]: SQLite backend 障害の場合
    fn delete_by_surface_reading(&self, surface: &str, reading: &str) -> Result<(), StorageError>;
}

/// User vocabulary の row。
///
/// # Invariants
///
/// - `id` は insert 前は `None`、DB から取得した場合は `Some`
/// - `score` は finite + non-negative(`validate_score` 通過後に保証)
/// - `created_at` / `updated_at` は UNIX epoch seconds
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
        let c = UserVocabRecord {
            id: Some(2),
            ..a.clone()
        };
        assert_ne!(a, c);
    }
}
```

- [ ] **Step 2: commit**

```bash
git add crates/kotoha-storage/src/user_vocab/store.rs
git commit -m "refactor(storage): split UserVocabStore into UserVocabReader + UserVocabWriter (arch-M-2)

Refs: #105"
```

---

### Task A2: `SqliteUserVocabStore` の trait impl 分離(`sqlite.rs`)

**Files:**
- Modify: `crates/kotoha-storage/src/user_vocab/sqlite.rs`

- [ ] **Step 1: import を `UserVocabStore` から `UserVocabReader` + `UserVocabWriter` に切り替え、`impl UserVocabStore for SqliteUserVocabStore` を 2 つの impl block に分割する**

`sqlite.rs` の冒頭 import を以下に変更する。

```rust
use crate::user_vocab::store::{UserVocabReader, UserVocabRecord, UserVocabWriter};
```

旧 `impl UserVocabStore for SqliteUserVocabStore { ... }` の 1 ブロックを削除し、以下の 2 ブロックに置換する。

```rust
impl UserVocabReader for SqliteUserVocabStore {
    fn find_by_reading(
        &self,
        reading: &str,
        limit: usize,
    ) -> Result<Vec<UserVocabRecord>, StorageError> {
        validate_reading(reading)?;
        let conn = self.db.lock_conn();
        let mut stmt = conn.prepare_cached(
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

    fn find_by_id(&self, id: i64) -> Result<Option<UserVocabRecord>, StorageError> {
        let conn = self.db.lock_conn();
        let mut stmt = conn.prepare_cached(
            "SELECT id, surface, reading, pos, score, created_at, updated_at \
             FROM user_vocab WHERE id = ?1",
        )?;
        let mut rows = stmt.query(rusqlite::params![id])?;
        if let Some(row) = rows.next()? {
            Ok(Some(UserVocabRecord {
                id: Some(row.get(0)?),
                surface: row.get(1)?,
                reading: row.get(2)?,
                pos: row.get(3)?,
                score: row.get::<_, f64>(4)? as f32,
                created_at: row.get(5)?,
                updated_at: row.get(6)?,
            }))
        } else {
            Ok(None)
        }
    }

    fn find_by_prefix(
        &self,
        reading_prefix: &str,
        limit: usize,
    ) -> Result<Vec<UserVocabRecord>, StorageError> {
        if !reading_prefix.is_empty() {
            validate_reading(reading_prefix)?;
        }
        let conn = self.db.lock_conn();
        let pattern = format!("{}%", reading_prefix);
        let mut stmt = conn.prepare_cached(
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

    fn list_all(&self, limit: usize, offset: usize) -> Result<Vec<UserVocabRecord>, StorageError> {
        let conn = self.db.lock_conn();
        let mut stmt = conn.prepare_cached(
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
}

impl UserVocabWriter for SqliteUserVocabStore {
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
        let max_rows = effective_max_rows();
        {
            let mut count_stmt =
                conn.prepare_cached("SELECT EXISTS(SELECT 1 FROM user_vocab LIMIT 1 OFFSET ?1)")?;
            let at_or_over_cap: i64 =
                count_stmt.query_row(rusqlite::params![max_rows as i64 - 1], |r| r.get(0))?;
            if at_or_over_cap != 0 {
                return Err(StorageError::QuotaExceeded {
                    table: "user_vocab".to_string(),
                    max: max_rows,
                });
            }
        }
        let result = {
            let mut insert_stmt = conn.prepare_cached(
                "INSERT INTO user_vocab (surface, reading, pos, score, created_at, updated_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            )?;
            insert_stmt.execute(rusqlite::params![
                record.surface,
                record.reading,
                record.pos,
                record.score as f64,
                created_at,
                updated_at,
            ])
        };
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

    fn delete_by_id(&self, id: i64) -> Result<(), StorageError> {
        let conn = self.db.lock_conn();
        let mut stmt = conn.prepare_cached("DELETE FROM user_vocab WHERE id = ?1")?;
        let affected = stmt.execute(rusqlite::params![id])?;
        if affected == 0 {
            return Err(StorageError::NotFound);
        }
        Ok(())
    }

    fn delete_by_surface_reading(&self, surface: &str, reading: &str) -> Result<(), StorageError> {
        validate_surface(surface)?;
        validate_reading(reading)?;
        let conn = self.db.lock_conn();
        let mut stmt =
            conn.prepare_cached("DELETE FROM user_vocab WHERE surface = ?1 AND reading = ?2")?;
        let affected = stmt.execute(rusqlite::params![surface, reading])?;
        if affected == 0 {
            return Err(StorageError::NotFound);
        }
        Ok(())
    }
}
```

- [ ] **Step 2: `#[cfg(test)] mod tests` 内の helper `fresh_store` が `SqliteUserVocabStore` を直接使う部分はそのまま維持する。ただし test 内で `UserVocabStore` trait の method を呼ぶ箇所は、`UserVocabReader` / `UserVocabWriter` の各 method として呼び出す形で変更が不要(具体型に対して dereference で両 trait の method が利用可能)であることを `cargo check --workspace` で確認する**

```bash
cargo check --workspace
```

- [ ] **Step 3: commit**

```bash
git add crates/kotoha-storage/src/user_vocab/sqlite.rs
git commit -m "refactor(storage): migrate SqliteUserVocabStore to UserVocabReader + UserVocabWriter impls

Refs: #105"
```

---

### Task A3: `MockUserVocabStore` の trait impl 分離(`mock.rs`)

**Files:**
- Modify: `crates/kotoha-storage/src/user_vocab/mock.rs`

- [ ] **Step 1: import を `UserVocabStore` から `UserVocabReader` + `UserVocabWriter` に切り替え、`impl UserVocabStore for MockUserVocabStore` を 2 つの impl block に分割する**

`mock.rs` の冒頭 import を以下に変更する。

```rust
use crate::user_vocab::store::{UserVocabReader, UserVocabRecord, UserVocabWriter};
```

旧 `impl UserVocabStore for MockUserVocabStore { ... }` を削除し、以下の 2 ブロックに置換する。

```rust
impl UserVocabReader for MockUserVocabStore {
    fn find_by_reading(
        &self,
        reading: &str,
        limit: usize,
    ) -> Result<Vec<UserVocabRecord>, StorageError> {
        validate_reading(reading)?;
        let records = self.records.lock().unwrap();
        let mut filtered: Vec<UserVocabRecord> = records
            .iter()
            .filter(|r| r.reading == reading)
            .cloned()
            .collect();
        filtered.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        filtered.truncate(limit);
        Ok(filtered)
    }

    fn find_by_id(&self, id: i64) -> Result<Option<UserVocabRecord>, StorageError> {
        let records = self.records.lock().unwrap();
        Ok(records.iter().find(|r| r.id == Some(id)).cloned())
    }

    fn find_by_prefix(
        &self,
        reading_prefix: &str,
        limit: usize,
    ) -> Result<Vec<UserVocabRecord>, StorageError> {
        if !reading_prefix.is_empty() {
            validate_reading(reading_prefix)?;
        }
        let records = self.records.lock().unwrap();
        let mut filtered: Vec<UserVocabRecord> = records
            .iter()
            .filter(|r| r.reading.starts_with(reading_prefix))
            .cloned()
            .collect();
        filtered.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        filtered.truncate(limit);
        Ok(filtered)
    }

    fn list_all(&self, limit: usize, offset: usize) -> Result<Vec<UserVocabRecord>, StorageError> {
        let records = self.records.lock().unwrap();
        Ok(records.iter().skip(offset).take(limit).cloned().collect())
    }
}

impl UserVocabWriter for MockUserVocabStore {
    fn insert(&self, mut record: UserVocabRecord) -> Result<i64, StorageError> {
        validate_surface(&record.surface)?;
        validate_reading(&record.reading)?;
        validate_pos(&record.pos)?;
        validate_score(record.score)?;

        let mut records = self.records.lock().unwrap();
        let max_rows = crate::user_vocab::sqlite::effective_max_rows();
        if records.len() >= max_rows {
            return Err(StorageError::QuotaExceeded {
                table: "user_vocab".to_string(),
                max: max_rows,
            });
        }
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
```

- [ ] **Step 2: `#[cfg(test)] mod tests` 内と `mod prop_tests` 内の `UserVocabStore` 使用箇所が存在しないことを確認する。`MockUserVocabStore` の各 test は具体型を直接使うため、trait import は不要**

```bash
cargo check --workspace
```

- [ ] **Step 3: commit**

```bash
git add crates/kotoha-storage/src/user_vocab/mock.rs
git commit -m "refactor(storage): migrate MockUserVocabStore to UserVocabReader + UserVocabWriter impls

Refs: #105"
```

---

### Task A4: `Database` factory 4 method 化 + 旧 `user_vocab_store` 削除(`database.rs`)

**Files:**
- Modify: `crates/kotoha-storage/src/database.rs`

- [ ] **Step 1: 旧 `user_vocab_store` method と旧 `learning_cache_store` method を削除し、4 つの新 factory method を追加する**

`database.rs` の以下の 2 method を削除する。

```rust
// 削除対象 (before)
pub fn user_vocab_store(self: &Arc<Self>) -> Box<dyn crate::user_vocab::store::UserVocabStore> {
    Box::new(crate::user_vocab::sqlite::SqliteUserVocabStore::new(
        Arc::clone(self),
    ))
}

pub fn learning_cache_store(
    self: &Arc<Self>,
) -> Box<dyn crate::learning_cache::LearningCacheStore> {
    Box::new(crate::learning_cache::SqliteLearningCacheStore::new(
        Arc::clone(self),
    ))
}
```

削除した位置に以下の 4 method を追加する。

```rust
/// `Arc<Database>` を `Box<dyn UserVocabReader>` として公開する。
///
/// # Postconditions
///
/// - 戻り値の trait object は内部で `Arc<SqliteUserVocabStore>` を保持し、
///   同一 `Database` から生成された他の factory の戻り値と同一 SQLite connection を共有する
pub fn user_vocab_reader(
    self: &Arc<Self>,
) -> Box<dyn crate::user_vocab::store::UserVocabReader> {
    Box::new(crate::user_vocab::sqlite::SqliteUserVocabStore::new(
        Arc::clone(self),
    ))
}

/// `Arc<Database>` を `Box<dyn UserVocabWriter>` として公開する。
///
/// # Postconditions
///
/// - 戻り値の trait object は内部で `Arc<SqliteUserVocabStore>` を保持する
pub fn user_vocab_writer(
    self: &Arc<Self>,
) -> Box<dyn crate::user_vocab::store::UserVocabWriter> {
    Box::new(crate::user_vocab::sqlite::SqliteUserVocabStore::new(
        Arc::clone(self),
    ))
}

/// `Arc<Database>` を `Box<dyn LearningCacheReader>` として公開する。
///
/// # Postconditions
///
/// - 戻り値の trait object は内部で `Arc<SqliteLearningCacheStore>` を保持する
pub fn learning_cache_reader(
    self: &Arc<Self>,
) -> Box<dyn crate::learning_cache::LearningCacheReader> {
    Box::new(crate::learning_cache::SqliteLearningCacheStore::new(
        Arc::clone(self),
    ))
}

/// `Arc<Database>` を `Box<dyn LearningCacheWriter>` として公開する。
///
/// # Postconditions
///
/// - 戻り値の trait object は内部で `Arc<SqliteLearningCacheStore>` を保持する
pub fn learning_cache_writer(
    self: &Arc<Self>,
) -> Box<dyn crate::learning_cache::LearningCacheWriter> {
    Box::new(crate::learning_cache::SqliteLearningCacheStore::new(
        Arc::clone(self),
    ))
}
```

- [ ] **Step 2: `database.rs` の `#[cfg(test)] mod tests` 内で旧 factory method を参照する 2 件の test を新 factory method に書換える**

変更前後の diff を以下に示す。

```rust
// before (test: user_vocab_store_factory_returns_owned_box)
let _store: Box<dyn crate::user_vocab::store::UserVocabStore> = db.user_vocab_store();

// after
let _store: Box<dyn crate::user_vocab::store::UserVocabReader> = db.user_vocab_reader();
```

```rust
// before (test: user_vocab_store_factory_shares_arc)
let store1 = db.user_vocab_store();
let store2 = db.user_vocab_store();
// ...
store1.insert(r).expect("insert ok");
let result = store2.find_by_reading("あ", 10).expect("find ok");

// after: writer で insert、reader で find
let store1 = db.user_vocab_writer();
let store2 = db.user_vocab_reader();
// ...
store1.insert(r).expect("insert ok");
let result = store2.find_by_reading("あ", 10).expect("find ok");
```

`parallel_inserts_via_arc_share_does_not_deadlock` test 内の `db.user_vocab_store()` は `db.user_vocab_writer()` に書換え、`store.insert(r)` / `store.list_all(100, 0)` の呼出し側も `UserVocabWriter` / `UserVocabReader` に分離する。具体的には `Arc::new(db.user_vocab_writer())` で writer を共有し、`list_all` 呼出しは `db.user_vocab_reader().list_all(...)` に変更する。

- [ ] **Step 3: `cargo check --workspace` でビルドエラーがゼロであることを確認する**

```bash
cargo check --workspace
```

- [ ] **Step 4: commit**

```bash
git add crates/kotoha-storage/src/database.rs
git commit -m "refactor(storage): replace user_vocab_store/learning_cache_store with 4-method factory (arch-M-2)

Refs: #105"
```

---

### Task A5: `kotoha-cli/src/dict_cli.rs` と `kotoha-cli/src/bin/dict.rs` 書換

**Files:**
- Modify: `crates/kotoha-cli/src/dict_cli.rs`
- Modify: `crates/kotoha-cli/src/bin/dict.rs`

- [ ] **Step 1: `dict_cli.rs` の `UserVocabStore` 依存を `UserVocabReader` / `UserVocabWriter` に分離する**

`dict_cli.rs` の import を以下に変更する。

```rust
// before
use kotoha_storage::user_vocab::store::{UserVocabRecord, UserVocabStore};

// after
use kotoha_storage::user_vocab::store::{UserVocabReader, UserVocabRecord, UserVocabWriter};
```

`run_add` の引数型を変更する。

```rust
// before (line 163)
pub fn run_add(store: &dyn UserVocabStore, args: &AddArgs, quiet: bool) -> Result<i32, String> {

// after
pub fn run_add(store: &dyn UserVocabWriter, args: &AddArgs, quiet: bool) -> Result<i32, String> {
```

`run_remove` の引数型を変更する。

```rust
// before (line 214)
pub fn run_remove(
    store: &dyn UserVocabStore,
    args: &RemoveArgs,
    quiet: bool,
) -> Result<i32, String> {

// after
pub fn run_remove(
    store: &dyn UserVocabWriter,
    args: &RemoveArgs,
    quiet: bool,
) -> Result<i32, String> {
```

`run_list` の引数型を変更する。

```rust
// before (line 268)
pub fn run_list(
    store: &dyn UserVocabStore,
    args: &ListArgs,
    json_global: bool,
) -> Result<i32, String> {

// after
pub fn run_list(
    store: &dyn UserVocabReader,
    args: &ListArgs,
    json_global: bool,
) -> Result<i32, String> {
```

`run_show` の引数型を変更する。

```rust
// before (line 344)
pub fn run_show(
    store: &dyn UserVocabStore,
    args: &ShowArgs,
    json_global: bool,
) -> Result<i32, String> {

// after
pub fn run_show(
    store: &dyn UserVocabReader,
    args: &ShowArgs,
    json_global: bool,
) -> Result<i32, String> {
```

- [ ] **Step 2: `bin/dict.rs` の `user_vocab_store()` 呼出しを reader / writer に分割し、各 subcommand に適切な trait object を渡す**

`bin/dict.rs` の変更対象行(30〜39 行目)を以下のように書換える。

```rust
// before
let store = db.user_vocab_store();

let exit_code = match &cli.command {
    Command::Add(args) => run_add(store.as_ref(), args, cli.quiet).unwrap_or(EXIT_INTERNAL),
    Command::Remove(args) => {
        run_remove(store.as_ref(), args, cli.quiet).unwrap_or(EXIT_INTERNAL)
    }
    Command::List(args) => run_list(store.as_ref(), args, cli.json).unwrap_or(EXIT_INTERNAL),
    Command::Show(args) => run_show(store.as_ref(), args, cli.json).unwrap_or(EXIT_INTERNAL),
};

// after
let reader = db.user_vocab_reader();
let writer = db.user_vocab_writer();

let exit_code = match &cli.command {
    Command::Add(args) => run_add(writer.as_ref(), args, cli.quiet).unwrap_or(EXIT_INTERNAL),
    Command::Remove(args) => {
        run_remove(writer.as_ref(), args, cli.quiet).unwrap_or(EXIT_INTERNAL)
    }
    Command::List(args) => run_list(reader.as_ref(), args, cli.json).unwrap_or(EXIT_INTERNAL),
    Command::Show(args) => run_show(reader.as_ref(), args, cli.json).unwrap_or(EXIT_INTERNAL),
};
```

- [ ] **Step 3: `cargo check --workspace` でビルドエラーがゼロであることを確認する**

```bash
cargo check --workspace
```

- [ ] **Step 4: commit**

```bash
git add crates/kotoha-cli/src/dict_cli.rs crates/kotoha-cli/src/bin/dict.rs
git commit -m "refactor(cli): adapt dict_cli.rs and bin/dict.rs to UserVocabReader/Writer split

Refs: #105"
```

---

### Task A6: `kotoha-core/src/dict/user_vocab.rs` 書換

**Files:**
- Modify: `crates/kotoha-core/src/dict/user_vocab.rs`

- [ ] **Step 1: `UserVocabStore` 依存を `UserVocabReader` に変更する**

`user_vocab.rs` の変更対象箇所を以下のように書換える。

```rust
// before (line 3)
use kotoha_storage::user_vocab::store::UserVocabStore;

// after
use kotoha_storage::user_vocab::store::UserVocabReader;
```

```rust
// before (line 7〜9)
/// User-managed vocabulary、`Box<dyn UserVocabStore>` に依存(DIP、spec §4.2)。
pub struct UserVocab {
    store: Box<dyn UserVocabStore>,

// after
/// User-managed vocabulary、`Box<dyn UserVocabReader>` に依存(DIP、spec §4.2)。
pub struct UserVocab {
    store: Box<dyn UserVocabReader>,
```

```rust
// before (line 14〜16)
/// `Box<dyn UserVocabStore>` を inject して構築する。
pub fn new(store: Box<dyn UserVocabStore>) -> Self {

// after
/// `Box<dyn UserVocabReader>` を inject して構築する。
///
/// # Preconditions
///
/// - `store` は `UserVocabReader` を実装した任意の backend を受け付ける
pub fn new(store: Box<dyn UserVocabReader>) -> Self {
```

`#[cfg(test)] mod tests` 内の `use kotoha_storage::user_vocab::mock::MockUserVocabStore;` の行は変更不要(`MockUserVocabStore` が `UserVocabReader` を impl しているため、`Box::new(MockUserVocabStore::new())` は `Box<dyn UserVocabReader>` として渡せる)。

- [ ] **Step 2: `crates/kotoha-core/tests/dict_user_vocab.rs` の `UserVocabStore` 参照を `UserVocabReader` / `UserVocabWriter` に変更する**

```rust
// before (line 7)
use kotoha_storage::user_vocab::store::{UserVocabRecord, UserVocabStore};

// after
use kotoha_storage::user_vocab::store::{UserVocabRecord, UserVocabWriter};
```

`seed` 関数の引数型を変更する。

```rust
// before (line 9)
fn seed(mock: &MockUserVocabStore, surface: &str, reading: &str, score: f32) {
    mock.insert(UserVocabRecord { ... }).unwrap();
}

// after: MockUserVocabStore が UserVocabWriter を impl しているため、
// 関数内の mock.insert() 呼出しは型変更なしに動作する。
// ただし `UserVocab::new(mock)` の引数型が `Box<dyn UserVocabReader>` になるため、
// test 内の mock 生成を以下のように変更する。
```

各 test 内で `Box::new(MockUserVocabStore::new())` を `UserVocab::new` に渡す箇所はそのまま動作する。`seed` 関数が `&MockUserVocabStore` を受けて直接 `insert` を呼ぶ箇所も、`MockUserVocabStore` が `UserVocabWriter` を impl しているため変更不要。ただし `UserVocabStore` の import だけ削除し、`UserVocabWriter` は `seed` 内の `mock.insert()` 呼出しに必要なため明示 import が必要かどうか `cargo check` で確認する。

```bash
cargo check --workspace
```

- [ ] **Step 3: commit**

```bash
git add crates/kotoha-core/src/dict/user_vocab.rs crates/kotoha-core/tests/dict_user_vocab.rs
git commit -m "refactor(core): update UserVocab to depend on UserVocabReader (ISP, arch-M-2)

Refs: #105"
```

---

### Task A7: `kotoha-cli/tests/dict_cli.rs` 書換

**Files:**
- Modify: `crates/kotoha-cli/tests/dict_cli.rs`

- [ ] **Step 1: `kotoha-cli/tests/dict_cli.rs` は binary `kotoha-dict` を `assert_cmd::Command` 経由で起動するため、factory API 変更の影響は `bin/dict.rs` 側で既に吸収済である。`dict_cli.rs` test ファイル自体は trait 名を直接参照していないことを確認し、`cargo check --workspace --features kotoha-cli/dict-persist` が通ることだけ検証する**

```bash
cargo check --workspace
```

もし `dict_cli.rs` test 内で `UserVocabStore` を直接参照する行が存在する場合は、`UserVocabReader` / `UserVocabWriter` に差し替える。現行ファイルには直接参照は存在しないため変更不要。

- [ ] **Step 2: commit(変更なしの場合はスキップし、Task A8 に進む)**

変更が発生した場合のみ commit する。

```bash
git add crates/kotoha-cli/tests/dict_cli.rs
git commit -m "refactor(cli-test): adapt dict_cli integration tests to ISP split factory

Refs: #105"
```

---

### Task A8: Phase A 退行ゼロ確認

**Files:**
- (変更なし、確認のみ)

- [ ] **Step 1: `kotoha-storage` の `lib.rs` が `UserVocabStore` を pub re-export している場合は削除し、`UserVocabReader` / `UserVocabWriter` を pub re-export するよう更新する**

`kotoha-storage/src/lib.rs` 内の `pub use user_vocab::store::UserVocabStore` の行が存在する場合は削除する。`UserVocabReader` / `UserVocabWriter` / `UserVocabRecord` を re-export する行を追加する。

```bash
grep -n "UserVocabStore" /home/kohshiro/develops/student/kotoha-ime/crates/kotoha-storage/src/lib.rs
```

- [ ] **Step 2: `cargo clippy --workspace --all-targets -- -D warnings` を実行し、警告ゼロを確認する**

```bash
cargo clippy --workspace --all-targets -- -D warnings
```

- [ ] **Step 3: `cargo test --workspace --features kotoha-core/mock-backend,kotoha-core/dict-persist` を実行し、P2-B baseline 355 件以上 PASS を確認する。実行結果の head/tail 各 10 行を verbatim で記録する**

```bash
cargo test --workspace --features kotoha-core/mock-backend,kotoha-core/dict-persist 2>&1 | tee /tmp/p2c-a-test-result.txt
head -10 /tmp/p2c-a-test-result.txt
tail -10 /tmp/p2c-a-test-result.txt
```

- [ ] **Step 4: commit(clippy / test の両方が PASS した場合のみ)**

```bash
git add -u
git commit -m "chore(storage): verify Phase A ISP split — zero regression, all tests PASS

Refs: #105"
```

---

## Phase P2-C-B: LearningCache 本実装

Phase A の ISP split が完了していることを前提とする。本 Phase では `LearningCacheReader` / `LearningCacheWriter` trait の本実装を TDD サイクル(red → green)で進め、cap const + RAII guard による eviction 制御を組み込む。最終 task B11 で `Database` factory を整備し、旧 `learning_cache_store()` を削除する。

---

### Task B1: `LearningCacheReader` / `LearningCacheWriter` trait 分割(`learning_cache/mod.rs`)

**Files:**
- Modify: `crates/kotoha-storage/src/learning_cache/mod.rs`

- [ ] **Step 1: `mod.rs` を以下の内容に全体置換する**

旧 `LearningCacheStore` trait と旧 `LearningCacheRecord` struct を削除し、`LearningCacheReader` / `LearningCacheWriter` / `LearningCacheRecord` を定義する。

```rust
//! `crates/kotoha-storage/src/learning_cache/mod.rs`
//! LearningCacheReader / LearningCacheWriter trait(spec §3.1、P2-C 本実装)。

pub mod sqlite;

pub use sqlite::SqliteLearningCacheStore;

use crate::error::StorageError;

/// Learning cache の read 操作を提供する抽象境界。
///
/// # Preconditions
///
/// - `kana_input` はひらがな canonical(`U+3040..=U+309F + U+30FC + U+30FB`)
/// - `limit` は 0 より大きい値を推奨する(0 を渡した場合は空 `Vec` を返す)
///
/// # Postconditions
///
/// - `lookup` の戻り値は `frequency DESC, last_used_at DESC` 順に `limit` 件以下を含む
/// - 該当 entry が存在しない場合は `Ok(Vec::new())` を返し、`Err` は返さない
///
/// # Errors
///
/// - [`StorageError::InvalidField`]: `kana_input` が validation 違反の場合
/// - [`StorageError::Sqlite`]: SQLite backend 障害の場合
pub trait LearningCacheReader: Send + Sync {
    /// `kana_input` に対応する変換候補を `frequency DESC, last_used_at DESC` 順で返す。
    ///
    /// # Preconditions
    ///
    /// - `kana_input` はひらがな canonical(`U+3040..=U+309F + U+30FC + U+30FB`)
    /// - `limit` は呼び出し元が必要な件数の上限を示す
    ///
    /// # Postconditions
    ///
    /// - 戻り値の長さは `limit` 以下
    /// - 戻り値の `frequency` は先頭 ≥ 末尾(単調非増加、同値は `last_used_at DESC` で整列)
    ///
    /// # Errors
    ///
    /// - [`StorageError::InvalidField`]: `kana_input` が hiragana 以外の文字を含む場合
    /// - [`StorageError::Sqlite`]: SQLite backend 障害の場合
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use kotoha_storage::learning_cache::LearningCacheReader;
    /// # fn doc(reader: &dyn LearningCacheReader) -> Result<(), Box<dyn std::error::Error>> {
    /// let results = reader.lookup("ちょうかん", 5)?;
    /// // results[0].frequency >= results[1].frequency (先頭が最多)
    /// # Ok(())
    /// # }
    /// ```
    fn lookup(
        &self,
        kana_input: &str,
        limit: usize,
    ) -> Result<Vec<LearningCacheRecord>, StorageError>;
}

/// Learning cache の write 操作を提供する抽象境界。
///
/// # Preconditions
///
/// - `kana_input` はひらがな canonical(`U+3040..=U+309F + U+30FC + U+30FB`)
/// - `chosen_kanji` は non-empty / ≤256 bytes / no control char / no bidi / no disallowed PUA
///
/// # Postconditions
///
/// - `record_choice` 成功後は該当 `(kana_input, chosen_kanji)` の `frequency` が 1 以上増加し、
///   `last_used_at` が現在時刻(UNIX epoch seconds)以上の値に更新される
/// - `record_choice` 成功後にキャッシュ行数が cap を超えた場合、`last_used_at` が最も古い entry
///   から順に削除され、行数が cap に収まる状態を保つ
///
/// # Errors
///
/// - [`StorageError::InvalidField`]: validation 違反の場合
/// - [`StorageError::Sqlite`]: SQLite backend 障害の場合
pub trait LearningCacheWriter: Send + Sync {
    /// ユーザの変換選択を `learning_cache` table に記録する。
    ///
    /// `(kana_input, chosen_kanji)` が既存の場合は `frequency += 1` かつ
    /// `last_used_at` を現在時刻に更新する。新規の場合は `frequency = 1` で insert する。
    /// insert / update 後に行数が cap を超えた場合は `last_used_at` が最も古い entry を
    /// 自動的に削除し、行数を cap 以内に収める。
    ///
    /// # Preconditions
    ///
    /// - `kana_input` はひらがな canonical(`U+3040..=U+309F + U+30FC + U+30FB`)
    /// - `chosen_kanji` は validate_surface の制約を満たす(non-empty / ≤256 bytes /
    ///   no control char / no bidi / no disallowed PUA)
    ///
    /// # Postconditions
    ///
    /// - 戻り値 `Ok(())` の時点でトランザクションがコミットされている
    /// - 行数は `effective_max_rows()` 以下を維持する
    ///
    /// # Errors
    ///
    /// - [`StorageError::InvalidField`]: `kana_input` / `chosen_kanji` が validation 違反
    /// - [`StorageError::Sqlite`]: SQLite backend 障害(disk full / SQLITE_BUSY など)
    fn record_choice(&self, kana_input: &str, chosen_kanji: &str) -> Result<(), StorageError>;

    /// `learning_cache` table から LRU(最終使用が最も古い)entry を削除して行数を `max_entries` 以内に収める。
    ///
    /// 呼び出し元が明示的に eviction を要求する場合に使用する。
    /// 通常の record_choice では内部で `effective_max_rows()` を cap として自動 eviction が動作するため、
    /// 本 method を明示呼び出しする必要は少ない。本 method は test / 手動 GC /
    /// Phase 5 personalization での動的 cap 変更を想定して `max_entries` 引数を維持する。
    ///
    /// # Preconditions
    ///
    /// - `max_entries` は 1 以上を推奨する(0 を渡した場合は全行削除が発生する)
    ///
    /// # Postconditions
    ///
    /// - 戻り値は削除した行数を示す
    /// - 行数が `max_entries` 以下の場合は削除を行わず `Ok(0)` を返す
    ///
    /// # Errors
    ///
    /// - [`StorageError::Sqlite`]: SQLite backend 障害の場合
    fn evict_lru(&self, max_entries: usize) -> Result<usize, StorageError>;
}

/// Learning cache の 1 行を表す struct。
///
/// # Invariants
///
/// - `id` は DB 内の `INTEGER PRIMARY KEY AUTOINCREMENT` 値(DB から取得した場合のみ有効)
/// - `frequency` は 1 以上(insert 時点で 1、以降 `record_choice` 毎に +1 される)
/// - `last_used_at` は UNIX epoch seconds 形式の非負整数
#[derive(Debug, Clone, PartialEq)]
pub struct LearningCacheRecord {
    /// `learning_cache.id`(DB 自動採番)。
    pub id: i64,
    /// 変換前のひらがな入力。
    pub kana_input: String,
    /// ユーザが選択した変換後文字列。
    pub chosen_kanji: String,
    /// `(kana_input, chosen_kanji)` の組合せが選ばれた累積回数。
    pub frequency: u32,
    /// 最後に選択された時刻(UNIX epoch seconds)。
    pub last_used_at: i64,
}
```

- [ ] **Step 2: `cargo clippy -p kotoha-storage -- -D warnings` を実行し、警告ゼロを確認する**

```bash
cargo clippy -p kotoha-storage -- -D warnings
```

- [ ] **Step 3: commit**

```bash
git add crates/kotoha-storage/src/learning_cache/mod.rs
git commit -m "refactor(storage): split LearningCacheStore into Reader/Writer traits (ISP)

Removes the monolithic LearningCacheStore trait and introduces
LearningCacheReader / LearningCacheWriter following the Interface
Segregation Principle. LearningCacheRecord is preserved verbatim.

Refs: #105"
```

---

### Task B2: TDD red — `lookup` 単体 test 4 件を記述する

**Files:**
- Test: `crates/kotoha-storage/src/learning_cache/sqlite.rs`

- [ ] **Step 1: `sqlite.rs` の `#[cfg(test)] mod tests` 内に以下の 4 件の test 関数を追加する**

既存の P2-B stub test 3 件(p2b_stub_*)はこの時点ではまだ残す。新規 test は stub impl に対して RED(失敗)になることを確認するための記述である。

```rust
// --- lookup tests (B2 red phase) ---

fn fresh_store_b() -> SqliteLearningCacheStore {
    let db = Database::open_in_memory().expect("memory open");
    SqliteLearningCacheStore::new(db)
}

/// `lookup` は存在しない kana_input に対して空 Vec を返す。
/// (B2) TDD red: stub は Ok(Vec::new()) を返すので本 test は PASS するが、
/// B3 実装後も引き続き PASS することを確認する。
#[test]
fn lookup_returns_empty_for_unknown_kana() {
    let store = fresh_store_b();
    let result = store.lookup("みず", 10).expect("ok");
    assert!(result.is_empty());
}

/// `lookup` は record_choice で記録した entry を返す。
/// (B2) TDD red: stub は常に空 Vec を返すので本 test は FAIL する。
#[test]
fn lookup_returns_recorded_entry() {
    let store = fresh_store_b();
    store.record_choice("あい", "愛").expect("record ok");
    let result = store.lookup("あい", 10).expect("ok");
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].chosen_kanji, "愛");
    assert_eq!(result[0].frequency, 1);
}

/// `lookup` は frequency DESC 順で複数 entry を返す。
/// (B2) TDD red: stub は常に空 Vec を返すので本 test は FAIL する。
#[test]
fn lookup_orders_by_frequency_desc() {
    let store = fresh_store_b();
    // "愛" を 2 回、"哀" を 1 回記録する。
    store.record_choice("あい", "愛").expect("record ok");
    store.record_choice("あい", "愛").expect("record ok");
    store.record_choice("あい", "哀").expect("record ok");
    let result = store.lookup("あい", 10).expect("ok");
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].chosen_kanji, "愛"); // frequency=2
    assert_eq!(result[1].chosen_kanji, "哀"); // frequency=1
}

/// `lookup` は limit を超えた件数を返さない。
/// (B2) TDD red: stub は常に空 Vec を返すので本 test は FAIL する。
#[test]
fn lookup_respects_limit() {
    let store = fresh_store_b();
    store.record_choice("あい", "愛").expect("ok");
    store.record_choice("あい", "哀").expect("ok");
    store.record_choice("あい", "藍").expect("ok");
    let result = store.lookup("あい", 2).expect("ok");
    assert!(result.len() <= 2);
}
```

- [ ] **Step 2: `cargo test -p kotoha-storage learning_cache::sqlite::tests::lookup` を実行し、`lookup_returns_recorded_entry` / `lookup_orders_by_frequency_desc` / `lookup_respects_limit` が FAIL(red)であることを確認する**

```bash
cargo test -p kotoha-storage learning_cache::sqlite::tests::lookup 2>&1
```

期待する末尾出力(例):
```
test learning_cache::sqlite::tests::lookup_returns_empty_for_unknown_kana ... ok
test learning_cache::sqlite::tests::lookup_returns_recorded_entry ... FAILED
test learning_cache::sqlite::tests::lookup_orders_by_frequency_desc ... FAILED
test learning_cache::sqlite::tests::lookup_respects_limit ... FAILED
```

- [ ] **Step 3: commit(red phase 記録)**

```bash
git add crates/kotoha-storage/src/learning_cache/sqlite.rs
git commit -m "test(storage): add lookup unit tests — TDD red phase (B2)

Four tests covering empty result, entry retrieval, frequency ordering,
and limit enforcement. Three tests are currently FAIL (stub returns empty).

Refs: #105"
```

---

### Task B3: TDD green — `lookup` 本実装

**Files:**
- Modify: `crates/kotoha-storage/src/learning_cache/sqlite.rs`

- [ ] **Step 1: `sqlite.rs` に `LOOKUP_SQL` const と `LearningCacheReader` impl を追加する**

```rust
// lookup SQL(spec §4.3)
const LOOKUP_SQL: &str =
    "SELECT id, kana_input, chosen_kanji, frequency, last_used_at
     FROM learning_cache
     WHERE kana_input = ?1
     ORDER BY frequency DESC, last_used_at DESC
     LIMIT ?2";

impl LearningCacheReader for SqliteLearningCacheStore {
    fn lookup(
        &self,
        kana_input: &str,
        limit: usize,
    ) -> Result<Vec<LearningCacheRecord>, StorageError> {
        use crate::validation::validate_reading;
        validate_reading(kana_input)?;
        let conn = self.db.lock_conn();
        // perf-H1: prepare_cached により Phase 3 IBus engine の打鍵毎呼び出しでも
        // SQL コンパイルを 1 度きりにする。
        let mut stmt = conn.prepare_cached(LOOKUP_SQL)?;
        let rows = stmt.query_map(rusqlite::params![kana_input, limit as i64], |row| {
            Ok(LearningCacheRecord {
                id: row.get(0)?,
                kana_input: row.get(1)?,
                chosen_kanji: row.get(2)?,
                frequency: row.get::<_, u32>(3)?,
                last_used_at: row.get(4)?,
            })
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }
}
```

`LearningCacheReader` を `mod.rs` から import するため、`sqlite.rs` の use 宣言に以下を追加する。

```rust
use crate::learning_cache::{LearningCacheReader, LearningCacheRecord};
```

- [ ] **Step 2: `cargo test -p kotoha-storage learning_cache::sqlite::tests::lookup` を実行し、4 件すべてが PASS(green)であることを確認する**

```bash
cargo test -p kotoha-storage learning_cache::sqlite::tests::lookup 2>&1
```

期待する末尾出力(例):
```
test learning_cache::sqlite::tests::lookup_returns_empty_for_unknown_kana ... ok
test learning_cache::sqlite::tests::lookup_returns_recorded_entry ... ok
test learning_cache::sqlite::tests::lookup_orders_by_frequency_desc ... ok
test learning_cache::sqlite::tests::lookup_respects_limit ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured
```

- [ ] **Step 3: commit**

```bash
git add crates/kotoha-storage/src/learning_cache/sqlite.rs
git commit -m "feat(storage): implement LearningCacheReader::lookup (B3)

Uses prepare_cached + LOOKUP_SQL (ORDER BY frequency DESC, last_used_at DESC
LIMIT ?2). All 4 lookup unit tests now pass.

Refs: #105"
```

---

### Task B4: TDD red — `record_choice` 基本 UPSERT 単体 test 5 件を記述する

**Files:**
- Test: `crates/kotoha-storage/src/learning_cache/sqlite.rs`

- [ ] **Step 1: `tests` mod に以下の 5 件の test 関数を追加する**

eviction なし(cap は production の 10_000 行のままで、5 件 insert 程度では eviction が発生しない)前提で動作を検証する。

```rust
// --- record_choice tests (B4 red phase) ---

/// `record_choice` は新規 entry を frequency=1 で insert する。
/// (B4) TDD red: stub は Ok(()) を返すが lookup で空 Vec が返るので FAIL。
#[test]
fn record_choice_inserts_new_entry_with_frequency_one() {
    let store = fresh_store_b();
    store.record_choice("かわ", "川").expect("record ok");
    let result = store.lookup("かわ", 10).expect("ok");
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].chosen_kanji, "川");
    assert_eq!(result[0].frequency, 1);
}

/// `record_choice` は同じ `(kana_input, chosen_kanji)` の重複呼び出しで frequency を +1 する。
/// (B4) TDD red: stub は Ok(()) を返すが lookup で正しい frequency が返らない。
#[test]
fn record_choice_increments_frequency_on_duplicate() {
    let store = fresh_store_b();
    store.record_choice("かわ", "川").expect("1st ok");
    store.record_choice("かわ", "川").expect("2nd ok");
    let result = store.lookup("かわ", 10).expect("ok");
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].frequency, 2);
}

/// `record_choice` は同じ kana_input でも chosen_kanji が異なる場合は別行として insert する。
/// (B4) TDD red: stub は Ok(()) を返すが lookup で 1 件しか返らない。
#[test]
fn record_choice_separates_different_chosen_kanji() {
    let store = fresh_store_b();
    store.record_choice("かわ", "川").expect("ok");
    store.record_choice("かわ", "河").expect("ok");
    let result = store.lookup("かわ", 10).expect("ok");
    assert_eq!(result.len(), 2);
}

/// `record_choice` 呼び出し後は `last_used_at` が 0 より大きい値になる。
/// (B4) TDD red: stub は Ok(()) を返すが lookup で last_used_at=0 が返る。
#[test]
fn record_choice_sets_last_used_at_to_nonzero() {
    let store = fresh_store_b();
    store.record_choice("かわ", "川").expect("ok");
    let result = store.lookup("かわ", 10).expect("ok");
    assert!(!result.is_empty());
    assert!(result[0].last_used_at > 0);
}

/// `record_choice` はひらがな以外の kana_input を拒否する。
/// (B4) TDD red: stub は validation を行わないので Ok(()) を返し FAIL。
#[test]
fn record_choice_rejects_non_hiragana_kana_input() {
    let store = fresh_store_b();
    let err = store.record_choice("abc", "川").unwrap_err();
    assert!(matches!(err, StorageError::InvalidField { .. }));
}
```

- [ ] **Step 2: `cargo test -p kotoha-storage learning_cache::sqlite::tests::record_choice` を実行し、上記 5 件の大部分が FAIL(red)であることを確認する**

```bash
cargo test -p kotoha-storage learning_cache::sqlite::tests::record_choice 2>&1
```

期待する末尾出力(例):
```
test learning_cache::sqlite::tests::record_choice_inserts_new_entry_with_frequency_one ... FAILED
test learning_cache::sqlite::tests::record_choice_increments_frequency_on_duplicate ... FAILED
test learning_cache::sqlite::tests::record_choice_separates_different_chosen_kanji ... FAILED
test learning_cache::sqlite::tests::record_choice_sets_last_used_at_to_nonzero ... FAILED
test learning_cache::sqlite::tests::record_choice_rejects_non_hiragana_kana_input ... FAILED
```

- [ ] **Step 3: commit(red phase 記録)**

```bash
git add crates/kotoha-storage/src/learning_cache/sqlite.rs
git commit -m "test(storage): add record_choice unit tests — TDD red phase (B4)

Five tests covering new insert, frequency increment, kanji separation,
last_used_at timestamp, and validation rejection. All five currently FAIL.

Refs: #105"
```

---

### Task B5: TDD green — `record_choice` UPSERT 実装(eviction なし)

**Files:**
- Modify: `crates/kotoha-storage/src/learning_cache/sqlite.rs`

- [ ] **Step 1: `sqlite.rs` に `UPSERT_SQL` const と `LearningCacheWriter` の `record_choice` 実装を追加する**

この段階では eviction 呼び出しを含まない。eviction は B8 で統合する。

```rust
use crate::learning_cache::{LearningCacheReader, LearningCacheRecord, LearningCacheWriter};
use crate::validation::{validate_reading, validate_surface};

// record_choice の UPSERT SQL(spec §4.2、SQLite 3.24+ ON CONFLICT UPSERT 構文)
const UPSERT_SQL: &str =
    "INSERT INTO learning_cache (kana_input, chosen_kanji, frequency, last_used_at)
     VALUES (?1, ?2, 1, ?3)
     ON CONFLICT(kana_input, chosen_kanji)
     DO UPDATE SET
         frequency     = frequency + 1,
         last_used_at  = excluded.last_used_at";

impl LearningCacheWriter for SqliteLearningCacheStore {
    fn record_choice(&self, kana_input: &str, chosen_kanji: &str) -> Result<(), StorageError> {
        validate_reading(kana_input)?;
        validate_surface(chosen_kanji)?;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        let conn = self.db.lock_conn();
        // perf-H1: prepare_cached により SQL コンパイルを 1 度きりにする。
        let mut stmt = conn.prepare_cached(UPSERT_SQL)?;
        stmt.execute(rusqlite::params![kana_input, chosen_kanji, now])?;
        // NOTE: eviction は B8 でここに追加する。
        Ok(())
    }

    fn evict_lru(&self, _max_entries: usize) -> Result<usize, StorageError> {
        // B10 で実装する。
        Ok(0)
    }
}
```

- [ ] **Step 2: `cargo test -p kotoha-storage learning_cache::sqlite::tests::record_choice` を実行し、5 件すべてが PASS(green)であることを確認する**

```bash
cargo test -p kotoha-storage learning_cache::sqlite::tests::record_choice 2>&1
```

期待する末尾出力(例):
```
test learning_cache::sqlite::tests::record_choice_inserts_new_entry_with_frequency_one ... ok
test learning_cache::sqlite::tests::record_choice_increments_frequency_on_duplicate ... ok
test learning_cache::sqlite::tests::record_choice_separates_different_chosen_kanji ... ok
test learning_cache::sqlite::tests::record_choice_sets_last_used_at_to_nonzero ... ok
test learning_cache::sqlite::tests::record_choice_rejects_non_hiragana_kana_input ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured
```

- [ ] **Step 3: commit**

```bash
git add crates/kotoha-storage/src/learning_cache/sqlite.rs
git commit -m "feat(storage): implement record_choice UPSERT without eviction (B5)

Uses ON CONFLICT(kana_input, chosen_kanji) DO UPDATE to increment
frequency and update last_used_at. Eviction hook will be added in B8.

Refs: #105"
```

---

### Task B6: cap const + RAII guard 実装

**Files:**
- Modify: `crates/kotoha-storage/src/learning_cache/sqlite.rs`

- [ ] **Step 1: `sqlite.rs` の先頭(use 宣言の直後、`SqliteLearningCacheStore` struct 定義の前)に cap override 一式を追加する**

P2-B `user_vocab/sqlite.rs` の `QuotaOverrideGuard` パターンを踏襲し、命名だけ `LearningCache` 用に変更する。

```rust
use std::sync::atomic::{AtomicUsize, Ordering};

/// Learning cache の行数上限(production 値、spec §4.6)。
pub const LEARNING_CACHE_MAX_ROWS: usize = 10_000;

/// テスト時の行数上限上書き(0 = unset、`LEARNING_CACHE_MAX_ROWS` を使用)。
///
/// `cargo test` ビルドでのみ意味を持ち、production binary には含まれない。
/// 10,000 行を実際に挿入するテストは時間 / メモリの観点で非現実的なため、
/// 単体テストはこの override を介して小さな上限値で eviction 動作を検証する。
#[cfg(test)]
pub(crate) static LEARNING_CACHE_MAX_ROWS_TEST_OVERRIDE: AtomicUsize = AtomicUsize::new(0);

/// override の直列化用 Mutex(複数 test が同時 override しないよう直列化)。
#[cfg(test)]
pub(crate) static CAP_OVERRIDE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// 行数上限の effective value を返す(`#[cfg(test)]` 時のみ override を考慮)。
///
/// # Postconditions
///
/// - `#[cfg(not(test))]` 時は常に `LEARNING_CACHE_MAX_ROWS` を返す
/// - `#[cfg(test)]` かつ override が 0 の場合は `LEARNING_CACHE_MAX_ROWS` を返す
/// - `#[cfg(test)]` かつ override が 0 より大きい場合は override 値を返す
#[inline]
pub(crate) fn effective_max_rows() -> usize {
    #[cfg(test)]
    {
        let v = LEARNING_CACHE_MAX_ROWS_TEST_OVERRIDE.load(Ordering::SeqCst);
        if v != 0 {
            return v;
        }
    }
    LEARNING_CACHE_MAX_ROWS
}

/// テスト中だけ `LEARNING_CACHE_MAX_ROWS` を `cap` に上書きする RAII guard。
///
/// drop 時に自動で 0(無効)に戻すので、test 同士の干渉を防ぐ。
/// override は process 全体の static なため、複数 test が同時に
/// override を活性化すると競合する。よって `CAP_OVERRIDE_LOCK` を
/// 取得して直列化する。
///
/// # Examples
///
/// ```no_run
/// # #[cfg(test)]
/// # fn example() {
/// let _guard = CapOverrideGuard::new(5);
/// // このブロック内では cap = 5 で動作する
/// // _guard が drop されると cap = LEARNING_CACHE_MAX_ROWS に戻る
/// # }
/// ```
#[cfg(test)]
pub(crate) struct CapOverrideGuard {
    _lock: std::sync::MutexGuard<'static, ()>,
}

#[cfg(test)]
impl CapOverrideGuard {
    pub(crate) fn new(cap: usize) -> Self {
        // poison していても続行(直前 test の panic でも次 test を回したい)。
        let lock = CAP_OVERRIDE_LOCK
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        LEARNING_CACHE_MAX_ROWS_TEST_OVERRIDE.store(cap, Ordering::SeqCst);
        Self { _lock: lock }
    }
}

#[cfg(test)]
impl Drop for CapOverrideGuard {
    fn drop(&mut self) {
        LEARNING_CACHE_MAX_ROWS_TEST_OVERRIDE.store(0, Ordering::SeqCst);
    }
}
```

- [ ] **Step 2: `cargo clippy -p kotoha-storage -- -D warnings` を実行し、警告ゼロを確認する**

```bash
cargo clippy -p kotoha-storage -- -D warnings
```

- [ ] **Step 3: `cargo test -p kotoha-storage learning_cache::sqlite::tests` を実行し、既存 test が引き続き PASS することを確認する**

```bash
cargo test -p kotoha-storage learning_cache::sqlite::tests 2>&1
```

- [ ] **Step 4: commit**

```bash
git add crates/kotoha-storage/src/learning_cache/sqlite.rs
git commit -m "feat(storage): add LEARNING_CACHE_MAX_ROWS cap const + CapOverrideGuard RAII (B6)

Mirrors the QuotaOverrideGuard pattern from user_vocab/sqlite.rs.
Production cap is 10_000 rows; test override uses AtomicUsize + Mutex
for serialization. effective_max_rows() selects the active limit.

Refs: #105"
```

---

### Task B7: TDD red — 自動 eviction の test 4 件を記述する

**Files:**
- Test: `crates/kotoha-storage/src/learning_cache/sqlite.rs`

- [ ] **Step 1: `tests` mod に以下の 4 件の test 関数を追加する**

各 test は `CapOverrideGuard` で cap を小さな値に設定し、eviction 動作を検証する。

```rust
// --- eviction tests (B7 red phase) ---

/// cap 未満の行数では record_choice 後に削除が発生しない。
/// (B7) TDD red: B5 実装は eviction を含まないので本 test は PASS する(eviction なし ⇒ 行数保持)。
/// B8 実装後も PASS を維持することを確認する。
#[test]
fn eviction_does_not_occur_when_below_cap() {
    let _guard = CapOverrideGuard::new(5);
    let store = fresh_store_b();
    // cap=5 に対して 3 件 insert する。
    for kanji in ["愛", "哀", "藍"] {
        store.record_choice("あい", kanji).expect("ok");
    }
    let result = store.lookup("あい", 10).expect("ok");
    assert_eq!(result.len(), 3);
}

/// cap を 1 件超過した場合、LRU の 1 件が削除されて行数が cap に収まる。
/// (B7) TDD red: B5 実装は eviction を含まないので insert 後に cap+1 件が残り FAIL する。
#[test]
fn eviction_removes_one_lru_entry_when_cap_exceeded_by_one() {
    let _guard = CapOverrideGuard::new(3);
    let store = fresh_store_b();
    // 3 件 insert して cap ぴったりにする。
    store.record_choice("あ", "亜").expect("ok");
    store.record_choice("い", "以").expect("ok");
    store.record_choice("う", "宇").expect("ok");
    // 4 件目で cap を 1 超過する。最も ancient な "あ/亜" が evict されるべき。
    store.record_choice("え", "江").expect("ok");
    // 総行数が cap=3 以内に収まっていることを確認する。
    let conn = store.db.lock_conn();
    let total: i64 = conn
        .query_row("SELECT count(*) FROM learning_cache", [], |r| r.get(0))
        .unwrap();
    assert!(total <= 3, "total={total} must be <= cap 3");
}

/// burst insert で行数が cap を超えない。
/// (B7) TDD red: B5 実装は eviction を含まないので cap を超えた行数が残り FAIL する。
#[test]
fn eviction_keeps_row_count_bounded_on_burst_insert() {
    let _guard = CapOverrideGuard::new(4);
    let store = fresh_store_b();
    // cap=4 に対して 10 件 insert する。
    let kanjis = ["一", "二", "三", "四", "五", "六", "七", "八", "九", "十"];
    for (i, kanji) in kanjis.iter().enumerate() {
        let reading = char::from_u32(0x3041 + i as u32)
            .expect("valid hiragana")
            .to_string();
        store.record_choice(&reading, kanji).expect("ok");
    }
    let conn = store.db.lock_conn();
    let total: i64 = conn
        .query_row("SELECT count(*) FROM learning_cache", [], |r| r.get(0))
        .unwrap();
    assert!(total <= 4, "total={total} must be <= cap 4 after burst");
}

/// LRU 保護: record_choice で update された entry は eviction 対象から外れる。
/// (B7) TDD red: B5 実装は eviction を含まないので挙動を検証できず行数超過で FAIL する。
#[test]
fn eviction_protects_recently_updated_entry_from_lru() {
    let _guard = CapOverrideGuard::new(2);
    let store = fresh_store_b();
    // "亜" を先に insert する(古い entry)。
    store.record_choice("あ", "亜").expect("ok");
    // "以" を後で insert する(新しい entry)。
    store.record_choice("い", "以").expect("ok");
    // "亜" を再度 record_choice して last_used_at を更新する。
    store.record_choice("あ", "亜").expect("ok");
    // 3 件目 insert で cap=2 を超える。"以" のほうが古いので evict される。
    store.record_choice("う", "宇").expect("ok");
    // "亜" は残っており、"以" は evict されている。
    let result_a = store.lookup("あ", 10).expect("ok");
    assert!(
        !result_a.is_empty(),
        "亜 must survive because it was recently updated"
    );
    let result_i = store.lookup("い", 10).expect("ok");
    assert!(
        result_i.is_empty(),
        "以 must be evicted as the LRU entry"
    );
}
```

- [ ] **Step 2: `cargo test -p kotoha-storage learning_cache::sqlite::tests::eviction` を実行し、cap 超過に関する test が FAIL(red)であることを確認する**

```bash
cargo test -p kotoha-storage learning_cache::sqlite::tests::eviction 2>&1
```

期待する末尾出力(例):
```
test learning_cache::sqlite::tests::eviction_does_not_occur_when_below_cap ... ok
test learning_cache::sqlite::tests::eviction_removes_one_lru_entry_when_cap_exceeded_by_one ... FAILED
test learning_cache::sqlite::tests::eviction_keeps_row_count_bounded_on_burst_insert ... FAILED
test learning_cache::sqlite::tests::eviction_protects_recently_updated_entry_from_lru ... FAILED
```

- [ ] **Step 3: commit(red phase 記録)**

```bash
git add crates/kotoha-storage/src/learning_cache/sqlite.rs
git commit -m "test(storage): add auto-eviction unit tests — TDD red phase (B7)

Four tests covering: below-cap no-op, single-excess eviction,
burst insert bounded, and LRU protection for recently-updated entries.
Three tests are currently FAIL (no eviction in B5 impl).

Refs: #105"
```

---

### Task B8: TDD green — `evict_to_cap` 共通 helper 抽出 + `record_choice` 内自動 eviction 統合

**Files:**
- Modify: `crates/kotoha-storage/src/learning_cache/sqlite.rs`

- [ ] **Step 1: `sqlite.rs` に `COUNT_SQL` / `EVICT_SQL` const と `evict_to_cap` 共通 helper を追加する**

`evict_to_cap` は `pub(crate)` とし、`record_choice` と `evict_lru` の両方から呼び出す。

```rust
// 総行数チェック SQL(spec §4.4)
const COUNT_SQL: &str = "SELECT count(*) FROM learning_cache";

// LRU 削除 SQL(spec §4.4)
const EVICT_SQL: &str =
    "DELETE FROM learning_cache
     WHERE id IN (
         SELECT id FROM learning_cache
         ORDER BY last_used_at ASC, id ASC
         LIMIT ?1
     )";

/// `learning_cache` の行数が `cap` を超えている場合、LRU から `count - cap` 行削除する。
///
/// # Preconditions
///
/// - `conn` は `lock_conn()` で取得済の `MutexGuard<Connection>`
/// - `cap` は呼び出し元が指定する行数上限値。`record_choice` からは `effective_max_rows()` を渡し、
///   `evict_lru` からは呼び出し元が引数として指定した `max_entries` を渡す
///
/// # Postconditions
///
/// - 戻り値は削除した行数
/// - 行数 ≤ cap が保証される
///
/// # Errors
///
/// - [`StorageError::Sqlite`]: SQLite backend 障害の場合
pub(crate) fn evict_to_cap(
    conn: &rusqlite::Connection,
    cap: usize,
) -> Result<usize, StorageError> {
    let total: i64 = conn.query_row(COUNT_SQL, [], |r| r.get(0))?;
    let total = total as usize;
    if total <= cap {
        return Ok(0);
    }
    let excess = total - cap;
    let mut stmt = conn.prepare_cached(EVICT_SQL)?;
    let deleted = stmt.execute(rusqlite::params![excess as i64])?;
    Ok(deleted)
}
```

- [ ] **Step 2: `LearningCacheWriter::record_choice` の末尾に eviction 呼び出しを追加する**

B5 で追加した `record_choice` 実装の `// NOTE: eviction は B8 でここに追加する。` の行を以下に置き換える。

```rust
        // 行数が effective_max_rows() を超えた場合、LRU entry を自動削除する(spec §4.2)。
        evict_to_cap(&conn, effective_max_rows())?;
```

- [ ] **Step 3: `cargo test -p kotoha-storage learning_cache::sqlite::tests::eviction` を実行し、4 件すべてが PASS(green)であることを確認する**

```bash
cargo test -p kotoha-storage learning_cache::sqlite::tests::eviction 2>&1
```

期待する末尾出力(例):
```
test learning_cache::sqlite::tests::eviction_does_not_occur_when_below_cap ... ok
test learning_cache::sqlite::tests::eviction_removes_one_lru_entry_when_cap_exceeded_by_one ... ok
test learning_cache::sqlite::tests::eviction_keeps_row_count_bounded_on_burst_insert ... ok
test learning_cache::sqlite::tests::eviction_protects_recently_updated_entry_from_lru ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured
```

- [ ] **Step 4: commit**

```bash
git add crates/kotoha-storage/src/learning_cache/sqlite.rs
git commit -m "feat(storage): extract evict_to_cap helper + integrate auto-eviction in record_choice (B8)

evict_to_cap(conn, cap) counts rows and deletes excess LRU entries via
ORDER BY last_used_at ASC, id ASC. record_choice now calls it with
effective_max_rows() after each UPSERT. All 4 eviction tests pass.

Refs: #105"
```

---

### Task B9: TDD red — `evict_lru(max_entries)` 単体 test 3 件を記述する

**Files:**
- Test: `crates/kotoha-storage/src/learning_cache/sqlite.rs`

- [ ] **Step 1: `tests` mod に以下の 3 件の test 関数を追加する**

`evict_lru` は `LearningCacheWriter` を経由して呼び出す。B5 の stub impl は `Ok(0)` を返すため、行数が cap を超えた場合に 0 を返す test は FAIL になる。

```rust
// --- evict_lru tests (B9 red phase) ---

/// `evict_lru` は行数が max_entries 以下の場合 0 を返す。
/// (B9) TDD red: stub は Ok(0) を返すので本 test は PASS する。B10 後も PASS を維持する。
#[test]
fn evict_lru_returns_zero_when_below_cap() {
    let store = fresh_store_b();
    store.record_choice("あ", "亜").expect("ok");
    store.record_choice("い", "以").expect("ok");
    // 行数 2 < max_entries=5 なので 0 が返る。
    let deleted = store.evict_lru(5).expect("ok");
    assert_eq!(deleted, 0);
}

/// `evict_lru` は超過分の行数を削除して削除件数を返す。
/// (B9) TDD red: stub は Ok(0) を返すので行数が変化せず FAIL する。
#[test]
fn evict_lru_deletes_excess_entries_and_returns_count() {
    let store = fresh_store_b();
    // 5 件 insert する。
    for kanji in ["亜", "以", "宇", "江", "尾"] {
        let kana = match kanji {
            "亜" => "あ",
            "以" => "い",
            "宇" => "う",
            "江" => "え",
            "尾" => "お",
            _ => unreachable!(),
        };
        store.record_choice(kana, kanji).expect("ok");
    }
    // max_entries=3 で呼び出す。5-3=2 件削除される。
    let deleted = store.evict_lru(3).expect("ok");
    assert_eq!(deleted, 2);
    let conn = store.db.lock_conn();
    let total: i64 = conn
        .query_row("SELECT count(*) FROM learning_cache", [], |r| r.get(0))
        .unwrap();
    assert_eq!(total, 3);
}

/// `evict_lru` は last_used_at が同値の場合 id ASC で tie-break する。
/// (B9) TDD red: stub は Ok(0) を返すので削除されず FAIL する。
#[test]
fn evict_lru_uses_id_asc_as_tiebreak_for_same_last_used_at() {
    let store = fresh_store_b();
    // last_used_at を同値(0)に固定して 3 件 insert する。
    {
        let conn = store.db.lock_conn();
        conn.execute(
            "INSERT INTO learning_cache (kana_input, chosen_kanji, frequency, last_used_at)
             VALUES ('あ', '亜', 1, 0)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO learning_cache (kana_input, chosen_kanji, frequency, last_used_at)
             VALUES ('い', '以', 1, 0)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO learning_cache (kana_input, chosen_kanji, frequency, last_used_at)
             VALUES ('う', '宇', 1, 0)",
            [],
        )
        .unwrap();
    }
    // max_entries=1 で呼び出す。id の小さい順に 2 件が削除される。
    let deleted = store.evict_lru(1).expect("ok");
    assert_eq!(deleted, 2);
    // 残っているのは id が最大の "う/宇" である。
    let result = store.lookup("う", 10).expect("ok");
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].chosen_kanji, "宇");
}
```

- [ ] **Step 2: `cargo test -p kotoha-storage learning_cache::sqlite::tests::evict_lru` を実行し、超過系 test が FAIL(red)であることを確認する**

```bash
cargo test -p kotoha-storage learning_cache::sqlite::tests::evict_lru 2>&1
```

期待する末尾出力(例):
```
test learning_cache::sqlite::tests::evict_lru_returns_zero_when_below_cap ... ok
test learning_cache::sqlite::tests::evict_lru_deletes_excess_entries_and_returns_count ... FAILED
test learning_cache::sqlite::tests::evict_lru_uses_id_asc_as_tiebreak_for_same_last_used_at ... FAILED
```

- [ ] **Step 3: commit(red phase 記録)**

```bash
git add crates/kotoha-storage/src/learning_cache/sqlite.rs
git commit -m "test(storage): add evict_lru unit tests — TDD red phase (B9)

Three tests: below-cap zero, excess deletion with count, and id ASC
tie-break when last_used_at is equal. Two tests currently FAIL.

Refs: #105"
```

---

### Task B10: TDD green — `evict_lru(max_entries)` 実装

**Files:**
- Modify: `crates/kotoha-storage/src/learning_cache/sqlite.rs`

- [ ] **Step 1: B5 で追加した `LearningCacheWriter::evict_lru` stub を本実装に置き換える**

`evict_to_cap` 共通 helper を `max_entries` を cap として呼び出す(spec §4.4)。

```rust
impl LearningCacheWriter for SqliteLearningCacheStore {
    // ... record_choice は B8 で完成済み ...

    fn evict_lru(&self, max_entries: usize) -> Result<usize, StorageError> {
        let conn = self.db.lock_conn();
        evict_to_cap(&conn, max_entries)
    }
}
```

B5 の stub `fn evict_lru(&self, _max_entries: usize) -> Result<usize, StorageError> { Ok(0) }` を上記で置き換える。

- [ ] **Step 2: `cargo test -p kotoha-storage learning_cache::sqlite::tests::evict_lru` を実行し、3 件すべてが PASS(green)であることを確認する**

```bash
cargo test -p kotoha-storage learning_cache::sqlite::tests::evict_lru 2>&1
```

期待する末尾出力(例):
```
test learning_cache::sqlite::tests::evict_lru_returns_zero_when_below_cap ... ok
test learning_cache::sqlite::tests::evict_lru_deletes_excess_entries_and_returns_count ... ok
test learning_cache::sqlite::tests::evict_lru_uses_id_asc_as_tiebreak_for_same_last_used_at ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured
```

- [ ] **Step 3: `cargo test -p kotoha-storage learning_cache::sqlite::tests` を実行し、全 test(lookup + record_choice + eviction + evict_lru + p2b_stub = 4+5+4+3+3 = 19 件以上)が PASS することを確認する**

```bash
cargo test -p kotoha-storage learning_cache::sqlite::tests 2>&1
```

- [ ] **Step 4: commit**

```bash
git add crates/kotoha-storage/src/learning_cache/sqlite.rs
git commit -m "feat(storage): implement evict_lru via evict_to_cap helper (B10)

evict_lru(max_entries) delegates to evict_to_cap(conn, max_entries).
All 3 evict_lru tests pass. evict_to_cap is reused by record_choice
(auto-eviction) and evict_lru (explicit eviction) from a single impl.

Refs: #105"
```

---

### Task B11: `Database::learning_cache_reader/writer` factory 追加 + 旧 `learning_cache_store` 削除(`database.rs`)

**Files:**
- Modify: `crates/kotoha-storage/src/database.rs`

- [ ] **Step 1: `database.rs` の旧 `learning_cache_store` method を削除し、`learning_cache_reader` / `learning_cache_writer` factory を追加する**

変更前(削除する箇所):

```rust
    /// (P2-B では unimplemented stub、P2-C で本実装、spec §6.3)
    pub fn learning_cache_store(
        self: &Arc<Self>,
    ) -> Box<dyn crate::learning_cache::LearningCacheStore> {
        Box::new(crate::learning_cache::SqliteLearningCacheStore::new(
            Arc::clone(self),
        ))
    }
```

変更後(追加する箇所):

```rust
    /// `Arc<Database>` を `Box<dyn LearningCacheReader>` として公開する(spec §3.1 / §4.6)。
    ///
    /// # Postconditions
    ///
    /// - 戻り値の trait object は内部で `Arc<SqliteLearningCacheStore>` を保持する
    /// - 同一 `Arc<Database>` から生成した reader / writer は同一 `Mutex<Connection>` を共有する
    pub fn learning_cache_reader(
        self: &Arc<Self>,
    ) -> Box<dyn crate::learning_cache::LearningCacheReader> {
        Box::new(crate::learning_cache::SqliteLearningCacheStore::new(
            Arc::clone(self),
        ))
    }

    /// `Arc<Database>` を `Box<dyn LearningCacheWriter>` として公開する(spec §3.1 / §4.6)。
    ///
    /// # Postconditions
    ///
    /// - 戻り値の trait object は内部で `Arc<SqliteLearningCacheStore>` を保持する
    /// - 同一 `Arc<Database>` から生成した reader / writer は同一 `Mutex<Connection>` を共有する
    pub fn learning_cache_writer(
        self: &Arc<Self>,
    ) -> Box<dyn crate::learning_cache::LearningCacheWriter> {
        Box::new(crate::learning_cache::SqliteLearningCacheStore::new(
            Arc::clone(self),
        ))
    }
```

- [ ] **Step 2: `learning_cache_store()` を参照していた箇所をすべて `learning_cache_reader()` / `learning_cache_writer()` に置き換える**

```bash
grep -rn "learning_cache_store" /home/kohshiro/develops/student/kotoha-ime/
```

発見した箇所を確認し、用途が read のみであれば `learning_cache_reader()`、write が必要であれば `learning_cache_writer()` に書き換える。

- [ ] **Step 3: `cargo build -p kotoha-storage` を実行し、コンパイルエラーなしを確認する**

```bash
cargo build -p kotoha-storage 2>&1
```

期待する出力(例):
```
   Compiling kotoha-storage v0.1.0 (.../crates/kotoha-storage)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in X.XXs
```

- [ ] **Step 4: `cargo build -p kotoha-cli` を実行し、コンパイルエラーなしを確認する**

```bash
cargo build -p kotoha-cli 2>&1
```

期待する出力(例):
```
   Compiling kotoha-cli v0.1.0 (.../crates/kotoha-cli)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in X.XXs
```

- [ ] **Step 5: `cargo test --workspace --features kotoha-core/mock-backend,kotoha-core/dict-persist` を実行し、P2-B baseline 355 件以上 PASS を確認する。実行結果の head/tail 各 10 行を verbatim で記録する**

```bash
cargo test --workspace --features kotoha-core/mock-backend,kotoha-core/dict-persist 2>&1 | tee /tmp/p2c-b-test-result.txt
head -10 /tmp/p2c-b-test-result.txt
tail -10 /tmp/p2c-b-test-result.txt
```

- [ ] **Step 6: commit**

```bash
git add crates/kotoha-storage/src/database.rs
git commit -m "feat(storage): add learning_cache_reader/writer factory methods, remove learning_cache_store (B11)

Replaces the monolithic learning_cache_store() with ISP-split factories:
learning_cache_reader() -> Box<dyn LearningCacheReader>
learning_cache_writer() -> Box<dyn LearningCacheWriter>
Both return Arc<SqliteLearningCacheStore> sharing the same Mutex<Connection>.

Refs: #105"
```

---

## Phase P2-C-C: v002 migration

### Task C1: v002 migration SQL ファイル作成

**Files:**
- Create: `crates/kotoha-storage/migrations/v002_learning_cache_index.sql`

- [ ] **Step 1: `crates/kotoha-storage/migrations/v002_learning_cache_index.sql` を作成する**

下記の SQL を記述する。`idx_learning_cache_last_used` index は `evict_lru` の `ORDER BY last_used_at ASC` および `lookup` の `ORDER BY frequency DESC, last_used_at DESC` における `last_used_at` 列のソートを index seek で高速化する目的で追加する(spec §4.1)。

```sql
-- v002_learning_cache_index: learning_cache テーブルへの last_used_at index 追加
-- spec: docs/superpowers/specs/2026-04-26-p2-c-learning-cache-design.md §4.1
--
-- 目的:
--   evict_lru(ORDER BY last_used_at ASC) および
--   lookup(ORDER BY frequency DESC, last_used_at DESC) における
--   last_used_at 列のソートを index seek で高速化する。
--
-- 適用条件:
--   本 migration は forward-only(P2-B §3.6 決定)。
--   既存 v001 DB を持つ環境では Database::open() が PRAGMA user_version = 1 を検出し、
--   v002 を自動 apply する。新規 DB では v001 と v002 を順に apply する。

CREATE INDEX idx_learning_cache_last_used ON learning_cache(last_used_at);
```

- [ ] **Step 2: SQL の文法妥当性をローカルで確認する**

```bash
sqlite3 /tmp/v002_syntax_check.db < /dev/null
sqlite3 /tmp/v002_syntax_check.db "
CREATE TABLE learning_cache (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    kana_input   TEXT    NOT NULL,
    chosen_kanji TEXT    NOT NULL,
    frequency    INTEGER NOT NULL DEFAULT 1,
    last_used_at INTEGER NOT NULL,
    UNIQUE(kana_input, chosen_kanji)
);
CREATE INDEX idx_learning_cache_last_used ON learning_cache(last_used_at);
SELECT name FROM sqlite_master WHERE type='index' AND name='idx_learning_cache_last_used';
"
rm -f /tmp/v002_syntax_check.db
```

期待する出力:
```
idx_learning_cache_last_used
```

---

### Task C2: `migrations.rs` へ v002 追加 + `LATEST_VERSION` 更新

**Files:**
- Modify: `crates/kotoha-storage/src/migrations.rs`

- [ ] **Step 1: `LATEST_VERSION` を `2` に更新し、`MIGRATIONS` 配列に v002 エントリを追加する**

`crates/kotoha-storage/src/migrations.rs` の定数部分を以下のとおり修正する。

修正前:
```rust
pub const LATEST_VERSION: i32 = 1;

pub const MIGRATIONS: &[(i32, &str)] = &[(1, include_str!("../migrations/v001_initial.sql"))];
```

修正後:
```rust
/// 最新 schema version。新 migration を `MIGRATIONS` に追加する際は本値を増やす。
pub const LATEST_VERSION: i32 = 2;

/// (version, sql) 配列。version 昇順厳守(spec §10.1.1)。
///
/// `include_str!` でバイナリ同梱するため、distribution 時に migrations
/// directory を別配布する必要はない(spec §3.6)。
pub const MIGRATIONS: &[(i32, &str)] = &[
    (1, include_str!("../migrations/v001_initial.sql")),
    (2, include_str!("../migrations/v002_learning_cache_index.sql")),
];
```

- [ ] **Step 2: `cargo build -p kotoha-storage` を実行し、コンパイルエラーなしを確認する**

```bash
cargo build -p kotoha-storage 2>&1
```

期待する出力(例):
```
   Compiling kotoha-storage v0.1.0 (.../crates/kotoha-storage)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in X.XXs
```

- [ ] **Step 3: commit**

```bash
git add crates/kotoha-storage/migrations/v002_learning_cache_index.sql \
        crates/kotoha-storage/src/migrations.rs
git commit -m "feat(storage): add v002 migration — idx_learning_cache_last_used (C1-C2)

Adds migrations/v002_learning_cache_index.sql which creates
idx_learning_cache_last_used on learning_cache(last_used_at).
Updates LATEST_VERSION to 2 and appends the v002 entry to MIGRATIONS.

The index accelerates evict_lru (ORDER BY last_used_at ASC) and
lookup (ORDER BY frequency DESC, last_used_at DESC) (spec §4.1).

Refs: #105"
```

---

### Task C3: migrations runner test 拡張

**Files:**
- Modify: `crates/kotoha-storage/src/migrations.rs`(`#[cfg(test)] mod tests` 内に追加)

- [ ] **Step 1: 既存の `#[cfg(test)] mod tests` ブロック末尾に 3 件の test 関数を追加する**

追加する test 関数の完全コードを以下に示す。

```rust
    // ---- v002 migration 検証 ----

    #[test]
    fn apply_migrations_creates_learning_cache_last_used_index() {
        // v001 + v002 を一括 apply した後に idx_learning_cache_last_used が存在することを確認する。
        let conn = Connection::open_in_memory().expect("memory open");
        apply_migrations(&conn).expect("apply");
        let count: i64 = conn
            .query_row(
                "SELECT count(*) FROM sqlite_master \
                 WHERE type='index' AND name='idx_learning_cache_last_used'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 1, "idx_learning_cache_last_used must exist after migration");
    }

    #[test]
    fn apply_migrations_upgrades_from_v001_to_v002() {
        // v001 を apply 済みの DB に対して apply_migrations を再実行すると
        // v002 だけが追加で apply され user_version が 2 になることを確認する。
        let conn = Connection::open_in_memory().expect("memory open");

        // v001 のみを手動 apply して v001 相当の初期状態を作る。
        let (_, v001_sql) = MIGRATIONS
            .iter()
            .find(|(v, _)| *v == 1)
            .expect("v001 must exist");
        conn.execute_batch(v001_sql).expect("v001 apply");
        conn.execute_batch("PRAGMA user_version = 1").expect("set user_version = 1");

        // apply_migrations は v002 のみを適用し、user_version を 2 に更新するはずである。
        apply_migrations(&conn).expect("upgrade v001 -> v002");

        let v: i32 = conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(v, 2, "user_version must be 2 after upgrade");

        let idx_count: i64 = conn
            .query_row(
                "SELECT count(*) FROM sqlite_master \
                 WHERE type='index' AND name='idx_learning_cache_last_used'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(idx_count, 1, "idx_learning_cache_last_used must be created by v002");
    }

    #[test]
    fn apply_migrations_v002_is_idempotent() {
        // apply_migrations を 2 回連続で呼び出しても user_version と index 数が変わらないことを確認する。
        let conn = Connection::open_in_memory().expect("memory open");
        apply_migrations(&conn).expect("first apply");
        apply_migrations(&conn).expect("second apply must be no-op");

        let v: i32 = conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(v, LATEST_VERSION, "user_version must equal LATEST_VERSION after double apply");

        let idx_count: i64 = conn
            .query_row(
                "SELECT count(*) FROM sqlite_master \
                 WHERE type='index' AND name='idx_learning_cache_last_used'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(idx_count, 1, "index count must remain 1 after idempotent apply");
    }
```

- [ ] **Step 2: `cargo test -p kotoha-storage` を実行し、追加した 3 件を含む全 test が PASS することを確認する**

```bash
cargo test -p kotoha-storage 2>&1 | tail -10
```

期待する出力(例、行数は前後の test 数に依存する):
```
test migrations::tests::apply_migrations_creates_learning_cache_last_used_index ... ok
test migrations::tests::apply_migrations_upgrades_from_v001_to_v002 ... ok
test migrations::tests::apply_migrations_v002_is_idempotent ... ok
...
test result: ok. X passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

- [ ] **Step 3: commit**

```bash
git add crates/kotoha-storage/src/migrations.rs
git commit -m "test(storage): add v002 migration runner tests — upgrade + idempotency (C3)

Adds 3 tests to migrations::tests:
- apply_migrations_creates_learning_cache_last_used_index
- apply_migrations_upgrades_from_v001_to_v002
- apply_migrations_v002_is_idempotent

Verifies that idx_learning_cache_last_used is created by v002,
that an existing v001 DB is correctly upgraded to v002,
and that double-apply is a no-op.

Refs: #105"
```

---

## Phase P2-C-D: test 整備

### Task D1: L2 integration test 新規作成

**Files:**
- Create: `crates/kotoha-storage/tests/learning_cache_store.rs`

- [ ] **Step 1: `crates/kotoha-storage/tests/learning_cache_store.rs` を新規作成する**

以下の完全コードを記述する。

```rust
//! L2 integration tests — LearningCache factory wiring / concurrency / persistence.
//! spec: docs/superpowers/specs/2026-04-26-p2-c-learning-cache-design.md §6.3

use kotoha_storage::{Database, LearningCacheReader, LearningCacheWriter};
use std::sync::Arc;

// ──────────────────────────────────────────────────────────────────────────────
// helper
// ──────────────────────────────────────────────────────────────────────────────

/// in-memory Database を開いて learning_cache_reader / learning_cache_writer を返す。
fn open_factory() -> (
    Box<dyn LearningCacheReader>,
    Box<dyn LearningCacheWriter>,
) {
    let db = Database::open_in_memory().expect("open_in_memory");
    let reader = db.learning_cache_reader();
    let writer = db.learning_cache_writer();
    (reader, writer)
}

// ──────────────────────────────────────────────────────────────────────────────
// tests
// ──────────────────────────────────────────────────────────────────────────────

/// writer で記録した entry を reader が参照できることを確認する。
///
/// `learning_cache_reader` と `learning_cache_writer` が同一の SQLite Connection
/// (Mutex<Connection> 経由)を共有していることを factory wiring レベルで検証する。
#[test]
fn factory_returns_reader_and_writer_sharing_same_data() {
    let (reader, writer) = open_factory();

    writer
        .record_choice("あいう", "愛")
        .expect("record_choice must succeed");

    let results = reader
        .lookup("あいう", 10)
        .expect("lookup must succeed");

    assert_eq!(results.len(), 1, "reader must see the entry written by writer");
    assert_eq!(results[0].chosen_kanji, "愛");
    assert_eq!(results[0].frequency, 1);
}

/// 10 スレッドが同時に record_choice を呼び出しても deadlock が発生せず
/// 全件が記録されることを確認する。
///
/// `Mutex<Connection>` による直列化が正しく機能することを検証する(spec §4.2 / E9)。
/// 同一 entry を 10 スレッドが record するため、完了後の frequency が 10 になるはずである。
#[test]
fn concurrent_record_choice_is_serialized() {
    let db = Database::open_in_memory().expect("open_in_memory");
    // Arc で writer を複数スレッドに共有する。
    let writer: Arc<dyn LearningCacheWriter> =
        Arc::from(db.learning_cache_writer());
    let reader = db.learning_cache_reader();

    const THREAD_COUNT: usize = 10;
    let mut handles = Vec::with_capacity(THREAD_COUNT);

    for _ in 0..THREAD_COUNT {
        let w = Arc::clone(&writer);
        handles.push(std::thread::spawn(move || {
            w.record_choice("きょう", "今日")
                .expect("record_choice must not fail under concurrency");
        }));
    }

    for h in handles {
        h.join().expect("thread must not panic");
    }

    let results = reader
        .lookup("きょう", 10)
        .expect("lookup must succeed");

    assert_eq!(results.len(), 1, "concurrent inserts for the same entry must be UPSERT-merged");
    assert_eq!(
        results[0].frequency, THREAD_COUNT as i64,
        "frequency must equal the number of concurrent record_choice calls"
    );
}

/// file-backed DB で eviction 後に Database を再 open しても行数が cap 以下であることを確認する。
///
/// 永続ファイルを使い、record + cap 超過 eviction + close + 再 open の後に
/// 削除済 entry が消えていることを検証する(spec §6.3)。
#[test]
fn eviction_persists_across_factory_reopens() {
    use kotoha_storage::learning_cache::CapOverrideGuard;

    let dir = tempfile::tempdir().expect("tempdir");
    let db_path = dir.path().join("test_eviction.db");

    // --- フェーズ 1: cap=3 で 5 件 record して eviction を発生させる ---
    {
        let db = Database::open(&db_path).expect("open phase1");
        let _guard = CapOverrideGuard::new(3);
        let writer = db.learning_cache_writer();

        // 5 件を順に record する。5 件目の insert 後に eviction が走り行数が 3 になるはずである。
        for i in 0u32..5 {
            let kana = format!("あ{i:02}");
            writer
                .record_choice(&kana, "愛")
                .expect("record_choice must succeed");
        }

        let writer2 = db.learning_cache_writer();
        // cap=3 に収まっていることをフェーズ 1 内で確認する。
        // evict_lru(3) が Ok(0) を返す = 既に cap 以内であることを確認する。
        let evicted = writer2.evict_lru(3).expect("evict_lru must succeed");
        assert_eq!(evicted, 0, "row count must already be <= cap after auto-eviction");
        drop(writer);
    } // db を drop して SQLite connection を close する。

    // --- フェーズ 2: 再 open して行数が 3 以下であることを確認する ---
    {
        let db = Database::open(&db_path).expect("open phase2");
        let writer = db.learning_cache_writer();

        // cap 以内であれば evict_lru(3) は Ok(0) を返すはずである。
        let evicted = writer.evict_lru(3).expect("evict_lru must succeed in phase2");
        assert_eq!(
            evicted, 0,
            "eviction result must persist across Database reopen"
        );
    }
}
```

`tempfile` crate が `kotoha-storage` の dev-dependency に含まれているかを確認し、含まれていない場合は `Cargo.toml` に追加する。

```bash
grep "tempfile" /home/kohshiro/develops/student/kotoha-ime/crates/kotoha-storage/Cargo.toml
```

含まれていない場合は `kotoha-storage/Cargo.toml` の `[dev-dependencies]` セクションに以下を追加する。

```toml
tempfile = { workspace = true }
```

workspace root `Cargo.toml` の `[workspace.dependencies]` に `tempfile` が存在しない場合は以下を追加する。

```toml
tempfile = "3"
```

- [ ] **Step 2: `cargo test -p kotoha-storage --test learning_cache_store` を実行し、3 件全 PASS を確認する**

```bash
cargo test -p kotoha-storage --test learning_cache_store 2>&1
```

期待する出力:
```
running 3 tests
test concurrent_record_choice_is_serialized ... ok
test eviction_persists_across_factory_reopens ... ok
test factory_returns_reader_and_writer_sharing_same_data ... ok
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

- [ ] **Step 3: commit**

```bash
git add crates/kotoha-storage/tests/learning_cache_store.rs \
        crates/kotoha-storage/Cargo.toml \
        Cargo.toml \
        Cargo.lock
git commit -m "test(storage): add L2 integration tests for LearningCache factory (D1)

Adds tests/learning_cache_store.rs with 3 integration tests:
- factory_returns_reader_and_writer_sharing_same_data
- concurrent_record_choice_is_serialized (10 threads, UPSERT merge)
- eviction_persists_across_factory_reopens (tempfile, reopen check)

Refs: #105"
```

---

### Task D2: proptest 新規作成

**Files:**
- Create: `crates/kotoha-storage/tests/learning_cache_proptest.rs`

`proptest` が `kotoha-storage` の dev-dependency に登録されていることを確認する。

```bash
grep "proptest" /home/kohshiro/develops/student/kotoha-ime/crates/kotoha-storage/Cargo.toml
```

含まれていない場合は `kotoha-storage/Cargo.toml` の `[dev-dependencies]` に以下を追加する。

```toml
proptest = { workspace = true }
```

- [ ] **Step 1: `crates/kotoha-storage/tests/learning_cache_proptest.rs` を新規作成する**

以下の完全コードを記述する。

strategy の設計方針:
- `hiragana_string()`: ひらがな Unicode ブロック `'\u{3041}'..='\u{309F}'` から 1〜32 文字を生成する。`validate_reading` が通過する入力のみを生成するため、ASCII 等の reject 文字を含まない。
- `kanji_string()`: 実用的な漢字集合として `['愛', '夢', '空', '花', '光', '山', '川', '海', '月', '日']` から 1〜10 文字を生成する。`validate_surface` が通過する文字列のみを生成するため、Invariant 3 において `record_choice` が `Err(InvalidField)` を返す分岐は strategy 上発生しない。
- Invariant 3 は strategy が valid 入力のみを生成することで保証するアプローチ(strategy-level guarantee)を採用し、`Result::Err` ケースを test 内で許容する方式は採用しない。

```rust
//! proptest — LearningCache の 3 invariant を property-based で検証する。
//! spec: docs/superpowers/specs/2026-04-26-p2-c-learning-cache-design.md §6.5
//!
//! PROPTEST_CASES=64 で実行する(ProptestConfig::with_cases(64))。

use kotoha_storage::{Database, LearningCacheReader, LearningCacheWriter};
use kotoha_storage::learning_cache::CapOverrideGuard;
use proptest::prelude::*;

// ──────────────────────────────────────────────────────────────────────────────
// strategy 定義
// ──────────────────────────────────────────────────────────────────────────────

/// ひらがな 1〜32 文字からなる文字列を生成する。
/// 生成される全文字列は validate_reading を通過する。
fn hiragana_string() -> impl Strategy<Value = String> {
    proptest::collection::vec(
        '\u{3041}'..='\u{309F}',
        1..=32,
    )
    .prop_map(|chars| chars.into_iter().collect())
}

/// 実用漢字集合 10 種から 1〜10 文字を繰り返し生成する。
/// 生成される全文字列は validate_surface を通過する。
fn kanji_string() -> impl Strategy<Value = String> {
    proptest::collection::vec(
        proptest::sample::select(vec![
            '愛', '夢', '空', '花', '光', '山', '川', '海', '月', '日',
        ]),
        1..=10,
    )
    .prop_map(|chars| chars.into_iter().collect())
}

/// (kana_input, chosen_kanji) のペアを 0〜200 件生成する。
fn record_sequence() -> impl Strategy<Value = Vec<(String, String)>> {
    proptest::collection::vec(
        (hiragana_string(), kanji_string()),
        0..=200,
    )
}

// ──────────────────────────────────────────────────────────────────────────────
// proptest
// ──────────────────────────────────────────────────────────────────────────────

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    /// Invariant 1: 任意の record sequence 後も行数が cap 以下である。
    ///
    /// CapOverrideGuard で cap=20 を設定し、record_choice の自動 eviction により
    /// 行数が 20 を超えないことを確認する。
    #[test]
    fn invariant_row_count_stays_within_cap(
        records in record_sequence()
    ) {
        let db = Database::open_in_memory().expect("open_in_memory");
        let _guard = CapOverrideGuard::new(20);
        let writer = db.learning_cache_writer();

        for (kana, kanji) in &records {
            // strategy が valid 入力のみを生成するため、Ok 以外は発生しない。
            writer.record_choice(kana, kanji)
                .expect("record_choice must succeed for valid inputs");
        }

        // evict_lru(20) が Ok(0) を返す = 行数が 20 以下であることを確認する。
        let evicted = writer.evict_lru(20)
            .expect("evict_lru must succeed");
        prop_assert_eq!(
            evicted, 0,
            "row count must be <= cap=20 after record sequence; evicted={evicted}"
        );
    }

    /// Invariant 2: lookup 結果が frequency DESC で単調非増加である。
    ///
    /// record sequence 後に kana_input を 1 件取り出して lookup を呼び出し、
    /// 結果 results[i].frequency >= results[i+1].frequency が全 i で成立することを確認する。
    #[test]
    fn invariant_lookup_result_is_frequency_desc(
        records in record_sequence()
    ) {
        // records が空の場合は lookup 対象が存在しないため、1 件以上を前提とする。
        prop_assume!(!records.is_empty());

        let db = Database::open_in_memory().expect("open_in_memory");
        let _guard = CapOverrideGuard::new(20);
        let writer = db.learning_cache_writer();
        let reader = db.learning_cache_reader();

        for (kana, kanji) in &records {
            writer.record_choice(kana, kanji)
                .expect("record_choice must succeed for valid inputs");
        }

        // 最初のペアの kana_input に対して lookup を実行する。
        let target_kana = &records[0].0;
        let results = reader.lookup(target_kana, 100)
            .expect("lookup must succeed");

        // 結果が 2 件以上の場合にのみ順序を検証する。
        for i in 0..results.len().saturating_sub(1) {
            prop_assert!(
                results[i].frequency >= results[i + 1].frequency,
                "lookup results must be frequency DESC: results[{}].frequency={} < results[{}].frequency={}",
                i, results[i].frequency, i + 1, results[i + 1].frequency
            );
        }
    }

    /// Invariant 3: strategy が生成した全入力に対して record_choice が Ok(()) を返す。
    ///
    /// hiragana_string / kanji_string は validate_reading / validate_surface を
    /// 通過する文字列のみを生成する。record_choice が Err を返した場合は
    /// strategy の生成ロジックかバリデーション実装のどちらかに誤りがある。
    #[test]
    fn invariant_valid_inputs_always_succeed(
        records in record_sequence()
    ) {
        let db = Database::open_in_memory().expect("open_in_memory");
        let _guard = CapOverrideGuard::new(20);
        let writer = db.learning_cache_writer();

        for (kana, kanji) in &records {
            let result = writer.record_choice(kana, kanji);
            prop_assert!(
                result.is_ok(),
                "record_choice must return Ok for valid inputs: kana={kana:?}, kanji={kanji:?}, err={result:?}"
            );
        }
    }
}
```

- [ ] **Step 2: `cargo test -p kotoha-storage --test learning_cache_proptest` を実行し、3 件全 PASS を確認する**

```bash
PROPTEST_CASES=64 cargo test -p kotoha-storage --test learning_cache_proptest 2>&1
```

期待する出力:
```
running 3 tests
test invariant_lookup_result_is_frequency_desc ... ok
test invariant_row_count_stays_within_cap ... ok
test invariant_valid_inputs_always_succeed ... ok
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

- [ ] **Step 3: commit**

```bash
git add crates/kotoha-storage/tests/learning_cache_proptest.rs \
        crates/kotoha-storage/Cargo.toml \
        Cargo.lock
git commit -m "test(storage): add proptest for LearningCache 3 invariants (D2)

Adds tests/learning_cache_proptest.rs with PROPTEST_CASES=64:
- invariant_row_count_stays_within_cap (cap=20, CapOverrideGuard)
- invariant_lookup_result_is_frequency_desc (frequency DESC monotone)
- invariant_valid_inputs_always_succeed (strategy-level valid input guarantee)

Strategies generate only validate_reading/validate_surface-passing inputs
to ensure Invariant 3 holds without requiring result-level error tolerance.

Refs: #105"
```

---

### Task D3: 全 feature 構成 test 確認

**Files:**
- 変更なし(test 実行 + 結果記録のみ)

P2-C 実装完了時点で 3 feature 構成の test 件数が baseline 予測(spec §6.6)と一致することを確認する。

- [ ] **Step 1: `default` 構成で `cargo test --workspace` を実行する**

```bash
cargo test --workspace 2>&1 | tee /tmp/p2c-d-test-default.txt
tail -5 /tmp/p2c-d-test-default.txt
```

期待する末尾 5 行(件数は 304 程度、exact 値は実装後に確定):
```
test result: ok. X passed; 0 failed; 0 ignored; 0 measured; 0 filtered out in Xs
...
test result: ok. X passed; 0 failed; 0 ignored; 0 measured; 0 filtered out in Xs
...
test result: ok. X passed; 0 failed; 0 ignored; 0 measured; 0 filtered out in Xs
...
test result: ok. X passed; 0 failed; 0 ignored; 0 measured; 0 filtered out in Xs

```

全 workspace crate の `test result: ok` を確認し、`FAILED` が 0 件であることを確認する。合計 PASS 数が 304 程度であることを確認する。

- [ ] **Step 2: `dict` 構成で `cargo test --workspace --features kotoha-core/mock-backend,kotoha-core/dict` を実行する**

```bash
cargo test --workspace --features kotoha-core/mock-backend,kotoha-core/dict 2>&1 \
    | tee /tmp/p2c-d-test-dict.txt
tail -5 /tmp/p2c-d-test-dict.txt
```

期待する末尾 5 行(件数は 363 程度):
```
test result: ok. X passed; 0 failed; 0 ignored; 0 measured; 0 filtered out in Xs
...
test result: ok. X passed; 0 failed; 0 ignored; 0 measured; 0 filtered out in Xs
...
test result: ok. X passed; 0 failed; 0 ignored; 0 measured; 0 filtered out in Xs
...
test result: ok. X passed; 0 failed; 0 ignored; 0 measured; 0 filtered out in Xs

```

全 workspace crate の `test result: ok` を確認し、合計 PASS 数が 363 程度であることを確認する。

- [ ] **Step 3: `dict-persist` 構成で `cargo test --workspace --features kotoha-core/mock-backend,kotoha-core/dict-persist` を実行する**

```bash
cargo test --workspace --features kotoha-core/mock-backend,kotoha-core/dict-persist 2>&1 \
    | tee /tmp/p2c-d-test-dict-persist.txt
tail -5 /tmp/p2c-d-test-dict-persist.txt
```

期待する末尾 5 行(件数は 384 程度):
```
test result: ok. X passed; 0 failed; 0 ignored; 0 measured; 0 filtered out in Xs
...
test result: ok. X passed; 0 failed; 0 ignored; 0 measured; 0 filtered out in Xs
...
test result: ok. X passed; 0 failed; 0 ignored; 0 measured; 0 filtered out in Xs
...
test result: ok. X passed; 0 failed; 0 ignored; 0 measured; 0 filtered out in Xs

```

全 workspace crate の `test result: ok` を確認し、合計 PASS 数が 384 程度であることを確認する。

- [ ] **Step 4: 3 構成の実測 PASS 数と予測値の差分を記録する**

実行結果から各構成の合計 PASS 数を集計し、以下の表を埋める。

| 構成 | 予測 PASS 数 | 実測 PASS 数 | 差分 |
|---|---|---|---|
| `default` | 304 程度 | (実測値) | (差分) |
| `dict` | 363 程度 | (実測値) | (差分) |
| `dict-persist` | 384 程度 | (実測値) | (差分) |

差分が予測値から ±5 件を超える場合、増加要因または減少要因(削除 / 統合した test がないか)を調査する。`FAILED` が 0 件であれば退行ゼロと判定する。

---

### Task D4: lefthook pre-push 全 PASS 確認

**Files:**
- 変更なし(lefthook 実行 + 結果記録のみ)

P2-C 完了時の最終 gate として lefthook pre-push の全コマンドが PASS することを確認する。

- [ ] **Step 1: lefthook pre-push の全コマンドを実行する**

`lefthook.yml` が定義する pre-push コマンド群を順次実行する。`parallel: false` なので実行順序は `manifest-check` → `build` → `clippy` → `test` → `test-dict` → `test-dict-persist` である。

```bash
lefthook run pre-push 2>&1 | tee /tmp/p2c-d-lefthook-pre-push.txt
tail -20 /tmp/p2c-d-lefthook-pre-push.txt
```

期待する末尾 20 行(各 step が PASS または `✔` で終わる):
```
  EXECUTE manifest-check
  ...
  ✔  manifest-check

  EXECUTE build
  ...
  ✔  build

  EXECUTE clippy
  ...
  ✔  clippy

  EXECUTE test
  ...
  ✔  test

  EXECUTE test-dict
  ...
  ✔  test-dict

  EXECUTE test-dict-persist
  ...
  ✔  test-dict-persist

SUMMARY: 6 tasks passed
```

いずれかの step が失敗した場合は、該当 step の出力を調査して修正してから再実行する。

- [ ] **Step 2: lefthook 全 PASS を確認後に Phase P2-C-D 完了を宣言する**

D4 は新規ファイルを生成しないため、commit は作成しない。D1〜D3 の commit が全て完了し、lefthook pre-push が PASS した時点で Phase P2-C-D は完了となる。

実行結果の verbatim 出力(`/tmp/p2c-d-lefthook-pre-push.txt` の末尾 20 行)を WBS ログに転記する。

---

## Phase P2-C-E: review / WBS / PR

Phase A〜D の実装が完了し、lefthook pre-push が全 PASS した後に Phase P2-C-E を開始する。Phase E は WBS 記録作成、静的解析クリア、PR 作成、Medium tier team-review、finding 修正、re-review、merge の 7 task で構成される。

### Task E1: WBS 記録 `docs/wbs/2026-04-26-feature-105-p2-c-learning-cache.md` を作成する

**Files:**
- Create: `docs/wbs/2026-04-26-feature-105-p2-c-learning-cache.md`

- [ ] **Step 1: WBS ファイルを Write ツールで作成する**

  P2-B WBS(`docs/wbs/2026-04-25-feature-98-p2-b-user-dictionary.md`)の frontmatter + 章立てを踏襲する。固定値(spec / plan path、ISSUE 番号、branch 名)は具体値で記載し、実装後に確定する数値(test count 着地値、commit 数、変更 file 数、変更 lines)は「実装後に確定」と明記するプレースホルダを置く。

  ```markdown
  ---
  title: P2-C LearningCache 本実装ログ
  date: 2026-04-26
  branch: feature/105-p2-c-learning-cache
  issue: 105
  pr: <PR# が確定したら追記>
  follow-up-issue: <Medium / Low deferred Issue# が確定したら追記>
  parent-spec: docs/superpowers/specs/2026-04-26-p2-c-learning-cache-design.md
  parent-plan: docs/superpowers/plans/2026-04-26-feature-105-p2-c-learning-cache.md
  ---

  # P2-C(LearningCache 本実装)実装ログ

  ## サマリ

  - `UserVocabReader` / `UserVocabWriter` ISP split(Phase A 完了、旧 `UserVocabStore` を 4 trait に分割)
  - `LearningCacheReader` / `LearningCacheWriter` ISP split(Phase A 完了、旧 `LearningCacheStore` を 4 trait に分割)
  - `SqliteLearningCacheStore` 本実装(Phase B 完了、UPSERT / auto eviction / LRU / lookup ordering の 23 件 L1 test + proptest 3 invariant)
  - v002 migration 追加(Phase C 完了、`v002_learning_cache_index.sql` + migration test)
  - proptest 拡張 + golden test 拡張(Phase D 完了、退行ゼロ確認)
  - Medium tier team-review(security / architecture / testing / performance の 4 dimension、Critical + High を全消化)

  ## metric

  | 項目 | 値 |
  |---|---|
  | default features 合計 | 304 程度 PASS(実装後に確定) |
  | `dict` features 合計 | 363 程度 PASS(実装後に確定) |
  | `dict-persist` features 合計 | **384 程度 PASS(P2-C 新 baseline、実装後に確定)** |
  | 工数(実) | 実装後に確定 |
  | 変更 file 数 | 実装後に確定(PR diff から記載) |
  | 変更 lines | 実装後に確定(PR diff から記載) |
  | commit 数 | 実装後に確定(develop からの差分) |

  ## Phase 別実装結果

  | Phase | task 数 | 完了 | 主要 commit | 学び |
  |---|---|---|---|---|
  | Phase A | 8 | 実装後に確定 | A1 ISP split → A8 invariant | 実装後に記録 |
  | Phase B | 11 | 実装後に確定 | B1 UPSERT → B11 proptest | 実装後に記録 |
  | Phase C | 4 | 実装後に確定 | C1 migration → C4 migration test | 実装後に記録 |
  | Phase D | 4 | 実装後に確定 | D1 proptest → D4 lefthook gate | 実装後に記録 |
  | Phase E | 7 | (本 WBS で進行中) | E1 WBS / E4 PR / E6 review fix | — |

  ## 学び / 判断ログ

  - (本 plan 実行中に発見した知見、想定との差異、判断の根拠を記録)

  ## 4-dim review 結果

  | dimension | Critical | High | Medium | Low |
  |---|---|---|---|---|
  | security | 実装後に確定 | 実装後に確定 | 実装後に確定 | 実装後に確定 |
  | architecture | 実装後に確定 | 実装後に確定 | 実装後に確定 | 実装後に確定 |
  | testing | 実装後に確定 | 実装後に確定 | 実装後に確定 | 実装後に確定 |
  | performance | 実装後に確定 | 実装後に確定 | 実装後に確定 | 実装後に確定 |
  | **合計** | **0(全消化)** | **0(全消化)** | **新規 Issue に移管** | **新規 Issue に移管** |

  ## deferred 項目(新規 Issue に移管)

  以下の Medium / Low finding は PR merge 後に新規 follow-up Issue を起票して管理する。

  - **connection pool(perf-M-1)**: `Mutex<Connection>` をコネクションプールに置き換える。Phase 3 IBus engine から高頻度で呼び出す前提で遅延を測定し、必要性が確認された場合に実施する
  - **Mutex poison handling 改善(sec-L-2)**: `lock_conn` が panic する代わりに `StorageError::Sqlite` を返す recover 経路を追加する
  - **LearningCache CLI 公開検討(Phase 3-A 以降)**: `kotoha-dict cache list` / `kotoha-dict cache flush` の subcommand を Phase 3 kick-off で判断する
  - **cap 値の動的化 / config 化(Phase 5)**: `LEARNING_CACHE_MAX_ROWS` を設定ファイルから読み込む機構を Phase 5 personalization で追加する
  - **review 由来の Medium / Low finding**: 実装後に確定

  ## 参照

  - spec: `docs/superpowers/specs/2026-04-26-p2-c-learning-cache-design.md`
  - plan: `docs/superpowers/plans/2026-04-26-feature-105-p2-c-learning-cache.md`
  - P2-B WBS: `docs/wbs/2026-04-25-feature-98-p2-b-user-dictionary.md`
  - ISSUE: https://github.com/std-koh-hinooka/kotoha-ime/issues/105
  ```

- [ ] **Step 2: WBS ファイルを commit する(develop merge 後に直接 push する)**

  WBS 記録は対象 PR の merge 後に `develop` へ直接 push する例外扱いである(project CLAUDE.md「WBS 直接 push の例外」)。そのため、commit は feature branch 上で作成するが、push のタイミングは E7 merge 後とする。

  ```bash
  git add docs/wbs/2026-04-26-feature-105-p2-c-learning-cache.md
  git commit -m "docs(wbs): add P2-C WBS reflection log (#105)"
  ```

---

### Task E2: 静的解析クリア確認(commit 直前の最終チェック)

**Files:**
- 変更なし(静的解析の確認のみ)

- [ ] **Step 1: `cargo fmt --all` を実行して差分がないことを確認する**

  ```bash
  cargo fmt --all
  git diff --exit-code
  ```

  `cargo fmt --all` は出力なしで終了する。`git diff --exit-code` が exit 0 を返さない場合は、差分ファイルを `git add` して追加 commit を作成してから先に進む。

  Expected output:
  ```
  (no output — cargo fmt produces no stdout when all files are already formatted)
  ```

- [ ] **Step 2: `cargo clippy` で警告ゼロを確認する**

  ```bash
  cargo clippy --workspace --all-targets -- -D warnings 2>&1 | tee /tmp/p2c-e-clippy.txt
  tail -5 /tmp/p2c-e-clippy.txt
  ```

  Expected output(末尾 5 行):
  ```
      Checking kotoha-cli v0.1.0 (...)
      Checking kotoha-dict v0.1.0 (...)
      Finished `dev` profile [unoptimized + debuginfo] target(s) in X.XXs
  warning: 0 warnings emitted
  ```

  警告が 1 件でも残っている場合は、警告箇所を修正してから Step 3 に進む。

- [ ] **Step 3: `cargo audit` で 0 vulnerability を確認する**

  ```bash
  cargo audit 2>&1 | tee /tmp/p2c-e-audit.txt
  tail -5 /tmp/p2c-e-audit.txt
  ```

  Expected output(末尾 5 行):
  ```
  Crate:     kotoha
  Version:   0.1.0
  Warning:   0 warnings
  Vulnerabilities: 0
  Successful audit, no vulnerabilities found
  ```

  vulnerability が 0 件でない場合は、該当 crate のバージョンを更新するか、audit.toml で ignore 登録して理由を PR body に記載する。

---

### Task E3: branch を origin に push する

**Files:**
- 変更なし(git 操作のみ)

- [ ] **Step 1: feature branch を origin に push して upstream tracking branch を設定する**

  ```bash
  git push -u origin feature/105-p2-c-learning-cache
  ```

  Expected output:
  ```
  Enumerating objects: X, done.
  Counting objects: 100% (X/X), done.
  Delta compression using up to X threads
  Compressing objects: 100% (X/X), done.
  Writing objects: 100% (X/X), X KiB | X MiB/s, done.
  Total X (delta X), reused 0 (delta 0), pack-reused 0
  remote: Resolving deltas: 100% (X/X), done.
  To github.com:std-koh-hinooka/kotoha-ime.git
   * [new branch]      feature/105-p2-c-learning-cache -> feature/105-p2-c-learning-cache
  Branch 'feature/105-p2-c-learning-cache' set up to track remote branch 'feature/105-p2-c-learning-cache' from 'origin'.
  ```

  lefthook pre-push フックが自動起動し、`fmt-check` / `clippy` / `test` / `test-dict` / `test-dict-persist` の 5 step が全 PASS することを確認する。いずれかの step が FAIL した場合は push がブロックされるため、該当箇所を修正してから再実行する。

---

### Task E4: `gh pr create` で PR を作成する

**Files:**
- 変更なし(gh CLI 操作のみ)

- [ ] **Step 1: PR を `--base develop` 指定で作成する**

  以下の bash コマンドを実行する。PR title は 70 文字以内の英語。PR body は HEREDOC で渡す。

  ```bash
  gh pr create \
    --base develop \
    --title "feat(p2-c): LearningCache full impl + ISP split (4-trait boundary)" \
    --body "$(cat <<'EOF'
  ## Summary

  - Split `UserVocabStore` and `LearningCacheStore` monolithic traits into 4 ISP-aligned traits each: `UserVocabReader`, `UserVocabWriter`, `LearningCacheReader`, `LearningCacheWriter`
  - Implement `SqliteLearningCacheStore` with full UPSERT, auto eviction (LRU), lookup ordering, and cap enforcement (replacing the Phase 2-B stub)
  - Add `v002_learning_cache_index.sql` migration and auto-upgrade path from v001 DB
  - Add 23 L1 unit tests + 3 proptest invariants for `LearningCacheStore`
  - Remove deprecated `user_vocab_store` / `learning_cache_store` factory methods from `Database`

  ## Scope

  - Single-PR scope per CLAUDE.md Branch Scope Policy (1 ISSUE = 1 branch)
  - Medium tier review per CLAUDE.md PR Review Matrix: `agent-teams:team-review` (security / architecture / testing / performance) + `owasp-security` + `secrets-check`

  ## Test count baseline (post-merge target)

  | feature set | expected PASS count |
  |---|---|
  | default | 304 approx. |
  | dict | 363 approx. |
  | dict-persist | 384 approx. |

  ## Test plan

  - [ ] `cargo test --workspace` passes with zero failures (default features)
  - [ ] `cargo test --workspace --features dict` passes with zero failures
  - [ ] `cargo test --workspace --features kotoha-cli/dict-persist` passes with zero failures
  - [ ] `cargo clippy --workspace --all-targets -- -D warnings` emits zero warnings
  - [ ] `cargo audit` reports 0 vulnerabilities
  - [ ] lefthook pre-push gate (fmt / clippy / test / test-dict / test-dict-persist) all PASS
  - [ ] `LearningCacheReader` / `LearningCacheWriter` / `UserVocabReader` / `UserVocabWriter` are established as public API
  - [ ] Deprecated `UserVocabStore` and `LearningCacheStore` traits are removed
  - [ ] v002 migration runs correctly on both fresh DB and v001 existing DB
  - [ ] 23 L1 tests for `SqliteLearningCacheStore` all PASS
  - [ ] 3 proptest invariants PASS at `PROPTEST_CASES=64`
  - [ ] WBS log `docs/wbs/2026-04-26-feature-105-p2-c-learning-cache.md` is created
  - [ ] Medium tier team-review Critical + High findings are all resolved (Medium / Low moved to follow-up Issue)

  ## References

  - Plan: `docs/superpowers/plans/2026-04-26-feature-105-p2-c-learning-cache.md`
  - Spec: `docs/superpowers/specs/2026-04-26-p2-c-learning-cache-design.md`
  - WBS: `docs/wbs/2026-04-26-feature-105-p2-c-learning-cache.md`

  Refs: #105
  EOF
  )"
  ```

- [ ] **Step 2: PR URL を確認して以降の task で使用する**

  ```bash
  gh pr view --json url,number | jq '{url, number}'
  ```

  出力された PR number を E6 / E7 の `gh pr view <number>` コマンドに使用する。

---

### Task E5: Medium tier team-review を実行する

**Files:**
- 変更なし(review 実行のみ)

global CLAUDE.md「Sub-agent Self-Report is Untrusted」規定に従い、各 reviewer subagent には verbatim output を必須とする。

global CLAUDE.md「System Resource Management」規定に従い、利用可能メモリが 8〜16GiB 帯の場合は 4 dimension を並列実行せず逐次実行する。実行前に `free -h` でメモリ状況を確認する。

- [ ] **Step 1: メモリ使用状況を確認する**

  ```bash
  free -h
  free -h | awk '/Swap/{split($3,a,"G"); split($2,b,"G"); if (length(a[1])>0 && length(b[1])>0) printf "Swap usage: %.1f%%\n", a[1]/b[1]*100}'
  ```

  利用可能メモリが 16GiB 超であれば 4 dimension 並列実行、8〜16GiB 帯であれば逐次実行、8GiB 未満であればユーザに中止を確認する。

- [ ] **Step 2: `agent-teams:team-review` skill を Medium tier(4 dimension)で実行する**

  main agent は以下の内容で `agent-teams:team-review` skill を呼び出す。

  **skill 呼出引数:**

  ```
  skill: agent-teams:team-review
  args: |
    dimensions: security, architecture, testing, performance
    scope: |
      Branch: feature/105-p2-c-learning-cache
      Issue: #105
      Spec: docs/superpowers/specs/2026-04-26-p2-c-learning-cache-design.md
      Plan: docs/superpowers/plans/2026-04-26-feature-105-p2-c-learning-cache.md
      P2-B WBS: docs/wbs/2026-04-25-feature-98-p2-b-user-dictionary.md
      Primary changed crates: kotoha-storage (src/learning_cache/, src/user_vocab/, migrations/)
      Test files: crates/kotoha-storage/tests/
    reviewer_prompt_template: |
      あなたは <dimension> dimension の reviewer である。
      以下の scope を対象に review を実施し、finding を Critical / High / Medium / Low の 4 段階で分類する。
      各 finding には以下の形式で記載する:
        - severity: Critical | High | Medium | Low
        - file: <file_path>:<line_number>
        - recommendation: <修正方針を 1〜3 文で記述>
        - verbatim_code: <該当コードの抜粋>
      IMPORTANT: 報告の末尾に、最終確認コマンドの verbatim 出力の head 5 行と tail 5 行を必ず含めること。
      "PASS" "ALL OK" などの要約のみの報告は不可。
  ```

- [ ] **Step 3: `owasp-security` skill を実行する**

  ```
  skill: owasp-security
  args: |
    scope: kotoha-storage crate の LearningCache 実装
    files:
      - crates/kotoha-storage/src/learning_cache/sqlite.rs
      - crates/kotoha-storage/src/learning_cache/mod.rs
      - crates/kotoha-storage/migrations/v002_learning_cache_index.sql
    mode: review-only (no code modification)
    IMPORTANT: 最終 verification コマンド出力の head/tail 5 行を verbatim で報告に含めること
  ```

- [ ] **Step 4: `secrets-check` skill を実行する**

  ```
  skill: secrets-check
  args: |
    scope: branch feature/105-p2-c-learning-cache の全変更ファイル
    mode: review-only (no code modification)
    IMPORTANT: スキャン結果の verbatim 出力(最終 5 行)を報告に含めること
  ```

- [ ] **Step 5: finding を Critical / High / Medium / Low に整理する**

  4 dimension + owasp + secrets-check の finding を集約し、以下の表に整理する。

  | finding-id | dimension | severity | file:line | summary |
  |---|---|---|---|---|
  | sec-C1 | security | Critical | (実行後に記入) | (実行後に記入) |
  | arch-H1 | architecture | High | (実行後に記入) | (実行後に記入) |
  | ... | ... | ... | ... | ... |

  Critical / High finding の一覧を WBS ログの「4-dim review 結果」セクションに転記する。

---

### Task E6: Critical / High finding を全消化する

**Files:**
- finding の severity と対象 file に応じて異なる(実行後に確定)

E5 で収集した Critical / High finding を 1 件ごとに独立 commit として修正する(P2-B PR #101 の 7 commit 慣習踏襲)。

- [ ] **Step 1: finding を件数順に確認して修正順序を決定する**

  Critical finding を先に処理し、次に High finding を処理する。1 件の finding が複数ファイルに影響する場合は同一 commit にまとめる。

- [ ] **Step 2: 各 finding を修正して commit を作成する**

  commit message の format は `fix(<dim>-<finding-id>): <summary>` とする。以下に各 dimension の例を示す。

  ```bash
  # security dimension の Critical finding の例
  git add crates/kotoha-storage/src/learning_cache/sqlite.rs
  git commit -m "fix(sec-c1): sanitize SQL parameter binding in LearningCacheStore UPSERT"

  # architecture dimension の High finding の例
  git add crates/kotoha-storage/src/learning_cache/mod.rs
  git commit -m "fix(arch-h1): extract CapEnforcement into separate module per SRP"

  # testing dimension の High finding の例
  git add crates/kotoha-storage/tests/learning_cache_tests.rs
  git commit -m "fix(test-h1): add boundary-value tests for LEARNING_CACHE_MAX_ROWS cap"

  # performance dimension の High finding の例
  git add crates/kotoha-storage/src/learning_cache/sqlite.rs
  git commit -m "fix(perf-h1): replace sequential LRU scan with indexed ORDER BY for eviction"
  ```

- [ ] **Step 3: 各 finding 修正後に lefthook pre-push を手動実行して退行がないことを確認する**

  ```bash
  # lefthook の pre-push hook を手動実行する(全 5 step)
  lefthook run pre-push 2>&1 | tee /tmp/p2c-e6-lefthook.txt
  tail -10 /tmp/p2c-e6-lefthook.txt
  ```

  Expected output(末尾部分):
  ```
  SUMMARY: 6 tasks passed
  ```

  いずれかの task が FAIL した場合は、退行を修正してから次の finding に進む。

- [ ] **Step 4: 修正済み commit を origin に push して PR を最新化する**

  ```bash
  git push origin feature/105-p2-c-learning-cache
  gh pr view <PR_NUMBER>
  ```

  GitHub 上の PR に新 commit が反映されていることを確認する。

---

### Task E7: re-review + merge までのチェックリスト確認 + squash merge

**Files:**
- 変更なし(review 実行 + git 操作のみ)

- [ ] **Step 1: E5 の 4-dim team-review を再実行して Critical + High が 0 件になったことを確認する**

  E6 で修正した finding に対して team-review を再実行する。再実行時は E5 Step 2 と同一の引数を使用する。Critical + High finding が 0 件になるまで E6 → E7 Step 1 のサイクルを繰り返す。

  Critical + High が 0 件になった時点で Step 2 に進む。Medium / Low finding は新規 follow-up Issue に移管する(spec §7.3 deferred Issue 方針準拠)。

- [ ] **Step 2: merge ready チェックリストを全項目確認する**

  spec §8 Acceptance Criteria の全 13 項目を確認する。

  - [ ] `cargo test --workspace` が `default` / `dict` / `dict-persist` の 3 構成すべてで PASS し、退行ゼロ(全 355+ test PASS を維持)
  - [ ] `cargo clippy --workspace --all-targets -- -D warnings` で警告ゼロ
  - [ ] `cargo audit` で 0 vulnerability を維持
  - [ ] lefthook pre-push の 3-feature gate(fmt / clippy / test)が全 PASS
  - [ ] P2-C 着地後の test count baseline: `default` 304 程度 / `dict` 363 程度 / `dict-persist` 384 程度
  - [ ] `LearningCacheReader` / `LearningCacheWriter` / `UserVocabReader` / `UserVocabWriter` の 4 trait が `kotoha-storage` の public API として確立されている
  - [ ] 旧 `UserVocabStore` trait および旧 `LearningCacheStore` trait が削除されている
  - [ ] `Database` factory が `user_vocab_reader` / `user_vocab_writer` / `learning_cache_reader` / `learning_cache_writer` の 4 method を持ち、旧 `user_vocab_store` / `learning_cache_store` の 2 method が削除されている
  - [ ] v002 migration(`v002_learning_cache_index.sql`)が新規 DB 初期化と既存 v001 DB の auto-upgrade の双方で正常に動作する
  - [ ] `SqliteLearningCacheStore` が UPSERT / auto eviction / LRU 削除 / lookup ordering の本実装を持ち、stub でないことを 23 件の L1 test が証明する
  - [ ] proptest 3 invariant が `PROPTEST_CASES=64` で全 PASS
  - [ ] WBS 記録(`docs/wbs/2026-04-26-feature-105-p2-c-learning-cache.md`)が作成されている
  - [ ] Medium tier team-review(security / performance / architecture / testing の 4 dimension)で Critical / High finding が全消化されている(Medium / Low finding は新規 follow-up Issue に移管)

  上記チェックリストの全項目が確認できた時点で Step 3 に進む。

- [ ] **Step 3: squash merge を実行する**

  P2-B PR #101 の慣習(squash merge)を踏襲する。

  ```bash
  gh pr merge <PR_NUMBER> --squash --delete-branch
  ```

  Expected output:
  ```
  ✓ Squashed and merged pull request #<PR_NUMBER> (feat(p2-c): LearningCache full impl + ISP split (4-trait boundary))
  ✓ Deleted branch feature/105-p2-c-learning-cache
  ```

- [ ] **Step 4: merge 後に WBS 記録を `develop` へ直接 push する**

  WBS 記録(`docs/wbs/*.md`)は対象 PR の merge 後に `develop` へ直接 push する例外扱いである(project CLAUDE.md「WBS 直接 push の例外」)。

  ```bash
  git checkout develop
  git pull origin develop
  # WBS ファイルが feature branch の squash merge に含まれていない場合は以下を実行する
  # (squash merge で WBS commit が含まれた場合は git add / commit は不要)
  git add docs/wbs/2026-04-26-feature-105-p2-c-learning-cache.md
  git commit -m "docs(wbs): add P2-C WBS reflection log (#105)"
  git push origin develop
  ```

  push 後に `git log origin/develop --oneline | head -5` で WBS commit が develop に存在することを確認する。

- [ ] **Step 5: Medium / Low finding の follow-up Issue を起票する**

  E5 Step 5 でまとめた Medium / Low finding のうち、spec §7.3 deferred Issue 一覧に記載されていない finding を新規 GitHub Issue として起票する。

  ```bash
  gh issue create \
    --title "P2-C follow-up: Medium/Low findings from team-review" \
    --body "$(cat <<'EOF'
  ## Context

  These findings were identified during the Medium-tier team-review of PR #<PR_NUMBER> (P2-C LearningCache implementation).
  Critical and High findings were resolved before merge. Medium and Low findings are deferred per spec §7.3.

  ## Findings

  (E5 Step 5 の Medium / Low finding 一覧をここに転記する)

  ## References

  - Spec §7.3: docs/superpowers/specs/2026-04-26-p2-c-learning-cache-design.md
  - WBS: docs/wbs/2026-04-26-feature-105-p2-c-learning-cache.md
  EOF
  )"
  ```

---

## Self-review 履歴

本 plan は writing-plans skill に基づき以下の手順で作成した。

| Phase | 範囲 | 着地行数 | 担当 |
|---|---|---|---|
| header + Phase A | 8 task(UserVocab ISP split) | 1075 行(累計) | docs-architect subagent #1 |
| Phase B | 11 task(LearningCache 本実装) | 2224 行(累計) | docs-architect subagent #2 |
| Phase C + D | 7 task(migration + test 整備) | 2998 行(累計) | docs-architect subagent #3 |
| Phase E | 7 task(review / WBS / PR) | 3790 行(累計) | docs-architect subagent #4 |

## 受容基準(spec §8 から再掲)

- [ ] `cargo test --workspace` が `default` / `dict` / `dict-persist` の 3 構成すべてで PASS し、退行ゼロ(全 355+ test PASS を維持)
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` で警告ゼロ
- [ ] `cargo audit` で 0 vulnerability を維持
- [ ] lefthook pre-push の 3-feature gate(fmt / clippy / test)が全 PASS
- [ ] P2-C 着地後の test count baseline: `default` 304 程度 / `dict` 363 程度 / `dict-persist` 384 程度
- [ ] `LearningCacheReader` / `LearningCacheWriter` / `UserVocabReader` / `UserVocabWriter` の 4 trait が `kotoha-storage` の public API として確立されている
- [ ] 旧 `UserVocabStore` trait および旧 `LearningCacheStore` trait が削除されている
- [ ] `Database` factory が `user_vocab_reader` / `user_vocab_writer` / `learning_cache_reader` / `learning_cache_writer` の 4 method を持ち、旧 `user_vocab_store` / `learning_cache_store` の 2 method が削除されている
- [ ] v002 migration(`v002_learning_cache_index.sql`)が新規 DB 初期化と既存 v001 DB の auto-upgrade の双方で正常に動作する
- [ ] `SqliteLearningCacheStore` が UPSERT / auto eviction / LRU 削除 / lookup ordering の本実装を持ち、stub でないことを 23 件の L1 test が証明する
- [ ] proptest 3 invariant が `PROPTEST_CASES=64` で全 PASS
- [ ] WBS 記録(`docs/wbs/2026-04-26-feature-105-p2-c-learning-cache.md`)が作成されている
- [ ] Medium tier team-review(security / performance / architecture / testing の 4 dimension)で Critical / High finding が全消化されている(Medium / Low finding は新規 follow-up Issue に移管)

---

end of plan(33 task / 5 phase)
