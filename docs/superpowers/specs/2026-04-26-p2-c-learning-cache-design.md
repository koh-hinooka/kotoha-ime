---
title: Phase 2-C — LearningCache 本実装 + arch-M-2 ISP split 設計
phase: 2
sub-phase: C
status: draft
related-adr:
  - 0014  # phase-2 dictionary layer architecture
  - 0015  # kotoha-storage SQLite adoption
related-spec:
  - 2026-04-25-kotoha-phase-2-design.md
  - 2026-04-25-p2-b-user-dictionary-design.md
created-at: 2026-04-26
---

# Phase 2-C — LearningCache 本実装 + arch-M-2 ISP split 設計書

本設計書は Phase 2「Dictionary and learning」の 3 番目の milestone P2-C を詳細化する子 spec である。Phase 2 spec(parent-spec)§3.2 Learning cache および §5.2 persistence を前提とし、P2-B で skeleton として配置した `LearningCacheStore` の 3 method を本実装する。同時に arch-M-2(Interface Segregation Principle: Reader/Writer 分離)を消化し、`UserVocabStore` / `LearningCacheStore` の両 trait を Reader/Writer に分割する。Phase 2 spec / ADR 0014 と本設計書の記述が矛盾する箇所は、本設計書が新しい(P2-C kick-off で確定した最新方針)とする。

## 目次

- [1. 概要 / 目的 / 非目的](#1-概要--目的--非目的)
- [2. アーキテクチャ全体像](#2-アーキテクチャ全体像)
- [3. ISP split: trait 設計](#3-isp-split-trait-設計)
- [4. SQL / migration / index 設計](#4-sql--migration--index-設計)
- [5. エラー処理と edge case](#5-エラー処理と-edge-case)
- [6. テスト戦略](#6-テスト戦略)
- [7. 実装順序と branch / PR 戦略](#7-実装順序と-branch--pr-戦略)
- [8. 受容基準](#8-受容基準acceptance-criteria)
- [9. 参考 / 関連文書](#9-参考--関連文書)

---

## 1. 概要 / 目的 / 非目的

### 1.1 目的

P2-C は以下 3 点を達成する milestone である。

1. P2-B で skeleton として配置した `LearningCacheStore` trait の 3 method(`lookup` / `record_choice` / `evict_lru`)を `SqliteLearningCacheStore` に本実装する。P2-B 着地時点で `learning_cache` table schema(v001 migration)は既に存在しており、本 Phase はビジネスロジックとインデックス追加のみを担う。
2. arch-M-2 として `UserVocabStore` / `LearningCacheStore` の両 trait を SOLID ISP(Interface Segregation Principle)に従い Reader/Writer の 4 trait に分割する。
3. v002 migration を追加し、`learning_cache` table に `idx_learning_cache_last_used` index を付与する。

### 1.2 P2-B 着地状態の確認

P2-B 完了時点において以下のコンポーネントが確立済である。

- `LearningCacheStore` trait(`lookup` / `record_choice` / `evict_lru` の 3 method)は `crates/kotoha-storage/src/learning_cache/mod.rs` に宣言済
- `SqliteLearningCacheStore` は `crates/kotoha-storage/src/learning_cache/sqlite.rs` に stub 実装済(すべての method が no-op を返す)
- `learning_cache` table は v001 migration(`migrations/v001_initial.sql`)で schema 作成済
- `Database::learning_cache_store()` factory は `crates/kotoha-storage/src/database.rs` に存在し、stub instance を返す
- test count baseline は `dict-persist` feature で 355 PASS

### 1.3 本 Phase の射程

P2-C は以下を実装対象とする。

- `kotoha-storage` 内 `LearningCacheStore` の本実装(UPSERT / auto eviction / lookup ordering)
- `UserVocabReader` / `UserVocabWriter` / `LearningCacheReader` / `LearningCacheWriter` の 4 trait 新設と旧 `UserVocabStore` / `LearningCacheStore` trait の削除
- `Database` factory の 4 method 化(`user_vocab_reader` / `user_vocab_writer` / `learning_cache_reader` / `learning_cache_writer`)
- v002 migration(`idx_learning_cache_last_used`)
- L1 / L2 / proptest の全 test 整備

### 1.4 非目的(Out of scope)

以下は本 Phase の対象外である。

- **Hybrid backend / Ranker 統合(P2-D)**: `BackendConfig::Hybrid { llm, dict, learning }` および LLM 候補 / Dict 候補 / LearningCache hit を merge / dedupe / rerank する Ranker 実装は P2-D で行う
- **IBus engine 接続(Phase 3-A)**: `kotoha-dict` CLI から IBus engine への LearningCache 公開は Phase 3 で判断する。P2-C の LearningCache は CLI に露出させない
- **connection pool(perf-M-1)**: `Mutex<Connection>` を Pool に置き換える最適化は別 Issue で扱う
- **prefix lookup**: `kana_input` の prefix match は Phase 3 で必要性が顕在化してから追加する
- **v002 migration の down 整備**: forward-only 方針(P2-B §3.6 決定)を継続する

---

## 2. アーキテクチャ全体像

### 2.1 データフロー図(ASCII)

```
┌─────────────────────────────────────────────────────────────┐
│  呼び出し元                                                   │
│  ┌────────────┐  ┌───────────────────────┐  ┌────────────┐  │
│  │ kotoha-cli  │  │ Phase 3 IBus engine  │  │ P2-D Ranker│  │
│  │ (dict_cli) │  │ (kotoha-ibus, 未着手) │  │ (P2-D 未着手)│  │
│  └─────┬──────┘  └──────────┬────────────┘  └─────┬──────┘  │
└────────┼────────────────────┼────────────────────┼──────────┘
         │                   │                    │
         ▼                   ▼                    ▼
┌──────────────────────────────────────────────────────────────┐
│  kotoha-storage: Database factory 4 method                   │
│                                                              │
│  db.user_vocab_reader()   ──►  Box<dyn UserVocabReader>      │
│  db.user_vocab_writer()   ──►  Box<dyn UserVocabWriter>      │
│  db.learning_cache_reader()──► Box<dyn LearningCacheReader>  │
│  db.learning_cache_writer()──► Box<dyn LearningCacheWriter>  │
│                              │                               │
│  (全 factory は内部で Arc<SqliteXXXStore> を共有する)         │
└──────────────────────────────┬───────────────────────────────┘
                               │
                               ▼
┌──────────────────────────────────────────────────────────────┐
│  SQLite kotoha.db                                            │
│  ┌──────────────────────┐  ┌──────────────────────────────┐  │
│  │ user_vocab table     │  │ learning_cache table         │  │
│  │  idx_user_vocab_reading│ │  idx_learning_cache_kana    │  │
│  │  (v001)              │  │  idx_learning_cache_last_used│  │
│  │                      │  │  (v002 で追加)               │  │
│  └──────────────────────┘  └──────────────────────────────┘  │
└──────────────────────────────────────────────────────────────┘
```

### 2.2 P2-C 着地後の crate 構造

```
crates/kotoha-storage/
├── migrations/
│   ├── v001_initial.sql                    (既存、変更なし)
│   └── v002_learning_cache_index.sql       (新規: idx_learning_cache_last_used 追加)
├── src/
│   ├── lib.rs                              (pub use 再 export、ISP 4 trait を公開)
│   ├── database.rs                         (factory 4 method 化、旧 2 method 削除)
│   ├── migrations.rs                       (LATEST_VERSION = 2 に更新、v002 登録)
│   ├── error.rs                            (変更なし)
│   ├── path.rs                             (変更なし)
│   ├── validation.rs                       (変更なし)
│   ├── learning_cache/
│   │   ├── mod.rs                          (LearningCacheReader + LearningCacheWriter trait
│   │   │                                    + LearningCacheRecord 型、旧 LearningCacheStore
│   │   │                                    trait 削除)
│   │   └── sqlite.rs                       (本実装: UPSERT + auto eviction + cap const
│   │                                        + cap override RAII guard + 23 単体 test)
│   └── user_vocab/
│       ├── store.rs                         (UserVocabReader + UserVocabWriter に分割、
│       │                                    旧 UserVocabStore trait 削除)
│       ├── sqlite.rs                        (UserVocabReader + UserVocabWriter 双方を
│       │                                    impl、既存 test を移行)
│       └── mock.rs                          (UserVocabReader + UserVocabWriter 双方を
│                                            impl、既存 test を移行)
├── tests/
│   ├── learning_cache_store.rs             (新規: L2 integration test 3 件)
│   ├── learning_cache_proptest.rs          (新規: proptest 3 invariant)
│   └── dict_user_vocab.rs                  (既存修正: factory split 反映)
```

`kotoha-cli/src/dict_cli.rs` は factory API の変更に追従するよう書換える。

### 2.3 設計境界

本 Phase は「1 Phase 1 ISSUE 1 PR」原則(global CLAUDE.md Branch Scope Policy: ≤20 file / ≤1000 line)に従う。ISP split は LearningCache 本実装の前提条件であるため、単一 PR に統合する。Branch Scope Policy 上限超過が見込まれる場合は §7.2 のフォールバック戦略を適用する。

---

## 3. ISP split: trait 設計

### 3.1 LearningCacheReader / LearningCacheWriter(新設)

P2-B の `LearningCacheStore` trait を ISP に従い 2 trait に分割する。旧 `LearningCacheStore` trait は本 Phase 完了時点で削除する。

```rust
//! `crates/kotoha-storage/src/learning_cache/mod.rs`

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

### 3.2 UserVocabReader / UserVocabWriter(arch-M-2 split)

旧 `UserVocabStore` trait を ISP に従い Reader / Writer の 2 trait に分割する。旧 `UserVocabStore` trait は本 Phase 完了時点で削除する。`kotoha-cli/src/dict_cli.rs` が唯一の consumer であり、書換範囲は同 1 ファイルに収まる。

```rust
//! `crates/kotoha-storage/src/user_vocab/store.rs`

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
    pub id: Option<i64>,
    pub surface: String,
    pub reading: String,
    pub pos: String,
    pub score: f32,
    pub created_at: i64,
    pub updated_at: i64,
}
```

`SqliteUserVocabStore` は `UserVocabReader` と `UserVocabWriter` の両方を impl する。`MockUserVocabStore` も同様に両方を impl する。

### 3.3 Database factory 4 method 化

```rust
//! `crates/kotoha-storage/src/database.rs`(抜粋)

impl Database {
    /// `Arc<Database>` を `Box<dyn UserVocabReader>` として公開する。
    ///
    /// # Postconditions
    ///
    /// - 戻り値の trait object は内部で `Arc<SqliteUserVocabStore>` を保持し、
    ///   同一 `Database` から生成された他の factory の戻り値と同一 SQLite connection を共有する
    pub fn user_vocab_reader(self: &Arc<Self>) -> Box<dyn crate::user_vocab::store::UserVocabReader> {
        Box::new(crate::user_vocab::sqlite::SqliteUserVocabStore::new(
            Arc::clone(self),
        ))
    }

    /// `Arc<Database>` を `Box<dyn UserVocabWriter>` として公開する。
    ///
    /// # Postconditions
    ///
    /// - 戻り値の trait object は内部で `Arc<SqliteUserVocabStore>` を保持する
    pub fn user_vocab_writer(self: &Arc<Self>) -> Box<dyn crate::user_vocab::store::UserVocabWriter> {
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
}
```

旧 `user_vocab_store()` と `learning_cache_store()` の 2 method は本 Phase 完了時点で削除する。各 factory は内部で `Arc<SqliteXXXStore>` を新規生成して `Box<dyn TraitX>` で wrap して返す。同一 `Arc<Database>` から生成された複数の factory 戻り値は、同一の `Mutex<Connection>` を介して SQLite connection を共有する。

---

## 4. SQL / migration / index 設計

### 4.1 v002 migration

```sql
-- crates/kotoha-storage/migrations/v002_learning_cache_index.sql
-- spec: docs/superpowers/specs/2026-04-26-p2-c-learning-cache-design.md §4.1

CREATE INDEX idx_learning_cache_last_used ON learning_cache(last_used_at);
```

v002 migration を追加することにより、`migrations.rs` の `LATEST_VERSION` を `2` に更新し、`MIGRATIONS` 定数配列に `(2, include_str!("../migrations/v002_learning_cache_index.sql"))` を追加する。

本 migration は forward-only(P2-B §3.6 決定)とする。既存 v001 DB を持つ環境では `Database::open()` 実行時に `PRAGMA user_version` が `1` であることを検出し、v002 を自動 apply する。新規 DB 初期化では v001 と v002 を順に apply する。

追加する index の目的は、`evict_lru` の LRU 削除(`ORDER BY last_used_at ASC`) および `lookup` の `ORDER BY frequency DESC, last_used_at DESC` における `last_used_at` 列のソートを index seek で高速化することである。

### 4.2 record_choice の実装

`record_choice` は以下の手順で動作する。

1. `validate_reading(kana_input)` を実行し、ひらがな以外が含まれる場合は `StorageError::InvalidField` を返す。
2. `validate_surface(chosen_kanji)` を実行し、validation 違反の場合は `StorageError::InvalidField` を返す。
3. `now = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)` で現在時刻を取得する。
4. `self.db.lock_conn()` で `Mutex<Connection>` を取得する。
5. UPSERT SQL を `prepare_cached` で実行する。
6. 総行数が `effective_max_rows()` を超えた場合、共通 helper `evict_to_cap` を呼び出して超過分を削除する。

```rust
// record_choice の UPSERT SQL(SQLite 3.24+ ON CONFLICT UPSERT 構文)
const UPSERT_SQL: &str =
    "INSERT INTO learning_cache (kana_input, chosen_kanji, frequency, last_used_at)
     VALUES (?1, ?2, 1, ?3)
     ON CONFLICT(kana_input, chosen_kanji)
     DO UPDATE SET
         frequency     = frequency + 1,
         last_used_at  = excluded.last_used_at";
```

`prepare_cached` を使用することで Phase 3 IBus engine の打鍵毎呼び出しでも SQL コンパイルを 1 度きりにする(perf-H1: P2-B `SqliteUserVocabStore` と同方針)。

### 4.3 lookup の実装

```rust
// lookup SQL
const LOOKUP_SQL: &str =
    "SELECT id, kana_input, chosen_kanji, frequency, last_used_at
     FROM learning_cache
     WHERE kana_input = ?1
     ORDER BY frequency DESC, last_used_at DESC
     LIMIT ?2";
```

`idx_learning_cache_kana`(v001 で作成済)経由で index seek を行い、`kana_input` 完全一致の行セットを取得する。取得行セットを `frequency DESC, last_used_at DESC` でソートして `limit` 件を返す。該当 entry が存在しない場合は `Ok(Vec::new())` を返す。

### 4.4 evict_lru の実装(明示呼び出し用)

```rust
// evict_lru の総行数チェック SQL
const COUNT_SQL: &str = "SELECT count(*) FROM learning_cache";

// evict_to_cap が使用する LRU 削除 SQL
const EVICT_SQL: &str =
    "DELETE FROM learning_cache
     WHERE id IN (
         SELECT id FROM learning_cache
         ORDER BY last_used_at ASC, id ASC
         LIMIT ?1
     )";
```

`evict_lru` は以下の手順で動作する。

1. `self.db.lock_conn()` で `Mutex<Connection>` を取得する。
2. 共通 helper `evict_to_cap(&conn, max_entries)` を呼び出す。`evict_to_cap` は内部で `COUNT_SQL` を実行し、総行数が `max_entries` 以下なら `Ok(0)` を early return する。
3. 総行数が `max_entries` を超えている場合、超過分(`total - max_entries` 件)を `EVICT_SQL` で削除する。
4. 削除した行数を `Ok(n)` で返す。

```rust
// evict_lru の実装例
impl LearningCacheWriter for SqliteLearningCacheStore {
    fn evict_lru(&self, max_entries: usize) -> Result<usize, StorageError> {
        let conn = self.db.lock_conn();
        evict_to_cap(&conn, max_entries)
    }
}
```

`record_choice` が内部で呼び出す `evict_to_cap` は `cap = effective_max_rows()` を渡す。`evict_lru` は呼び出し元が指定した `max_entries` を `cap` として `evict_to_cap` に渡す。両者は同一 helper を共有するが、cap の出所が異なる。

### 4.5 共通 eviction helper `evict_to_cap`

`record_choice` と `evict_lru` の両方から呼び出す共通 helper を抽出する。

```rust
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
fn evict_to_cap(conn: &rusqlite::Connection, cap: usize) -> Result<usize, StorageError> {
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

### 4.6 cap const と test override pattern

`LEARNING_CACHE_MAX_ROWS` は production の行数上限を定義する。P2-B の `USER_VOCAB_MAX_ROWS` / `QuotaOverrideGuard` パターンを完全に踏襲する。

```rust
//! `crates/kotoha-storage/src/learning_cache/sqlite.rs`(cap override 部分)

use std::sync::atomic::{AtomicUsize, Ordering};

/// Learning cache の行数上限(production 値)。
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

---

## 5. エラー処理と edge case

### 5.1 StorageError 既存 variant 流用

P2-C は `StorageError` に新規 variant を追加しない。追加しない理由を以下に示す。

- LearningCache は cap 超過を eviction で自動吸収するため、`QuotaExceeded` を呼び出し元に返す必要がない(`UserVocabStore` の `insert` が `QuotaExceeded` を返すのと対称的に設計する必要はない)
- validation 失敗は既存 `InvalidField` variant で網羅できる
- SQLite backend 障害は既存 `Sqlite` variant で網羅できる

ケース別の variant 対応表を以下に示す。

| ケース | 使用する StorageError variant |
|---|---|
| `kana_input` が hiragana 以外を含む | `InvalidField { name: "kana_input", .. }` |
| `chosen_kanji` が空 / 256 bytes 超 / control char / bidi / disallowed PUA | `InvalidField { name: "chosen_kanji", .. }` |
| SQLite backend 障害(disk full / SQLITE_BUSY / SQLITE_LOCKED) | `Sqlite(rusqlite::Error)` |
| `Mutex<Connection>` の lock_conn での poison | lock_conn 内で `expect` により panic(既存実装と同方針、本 Phase で変更なし。recover 経路の追加は §7.3 の sec-L-2 Issue で後続対応) |

### 5.2 edge case 一覧

以下の 10 件の edge case について、入力 / 期待動作 / 設計判断を定義する。

**E1: 同エントリを連続して record_choice した場合**

- 入力: `record_choice("あい", "愛")` を N 回呼び出す
- 期待動作: `(kana_input="あい", chosen_kanji="愛")` の `frequency` が N に増加し、`last_used_at` が最後の呼び出し時刻に更新される
- 設計判断: UPSERT の `ON CONFLICT DO UPDATE SET frequency = frequency + 1` が正確に 1 ずつ増分する

**E2: 同一 kana_input に異なる chosen_kanji を record_choice した場合**

- 入力: `record_choice("あい", "愛")` と `record_choice("あい", "哀")` を別々に呼び出す
- 期待動作: `UNIQUE(kana_input, chosen_kanji)` の制約により 2 件の別 entry として保持される
- 設計判断: `(kana_input, chosen_kanji)` の複合 UNIQUE が entry の識別単位である

**E3: cap 直上 / 直下 / burst insert した場合**

- 入力(直下): 行数が `cap - 1` の状態で `record_choice` を 1 件実行する
- 期待動作: 行数が `cap` になり eviction は発生しない
- 入力(直上): 行数が `cap` の状態で新規 `record_choice` を実行する
- 期待動作: 新規 entry が insert された後、`last_used_at` が最小の entry 1 件が削除され行数が `cap` に戻る
- 入力(burst): 行数 0 の状態から `cap + 10` 件を連続して insert する
- 期待動作: 最終行数は `cap` 以下であり、`cap + 10` にはならない

**E4: last_used_at が tie した場合の tie-break**

- 入力: 2 件の entry が同一の `last_used_at` を持ち、eviction が 1 件のみ必要な場合
- 期待動作: `id ASC` で小さい entry(先に insert された entry)が削除される
- 設計判断: `EVICT_SQL` の `ORDER BY last_used_at ASC, id ASC` が tie-break を担保する

**E5: record_choice で更新した entry が eviction 対象から外れる場合**

- 入力: 行数 cap の状態で、既存の最古エントリに対して `record_choice` を実行する
- 期待動作: 対象 entry の `last_used_at` が現在時刻に更新されるため LRU 最古でなくなり、eviction 対象から外れる。代わりに次に古い entry が削除される
- 設計判断: UPSERT で `last_used_at = excluded.last_used_at` を更新することで LRU 保護が自動的に機能する

**E6: lookup で該当 entry が存在しない場合**

- 入力: `lookup("ほげほげ", 10)` を呼び出す(該当 kana_input が存在しない)
- 期待動作: `Ok(Vec::new())` を返す。`Err` を返すことは仕様違反である
- 設計判断: 変換候補の不在は正常状態であり、エラーとして扱わない

**E7: lookup の ordering と limit**

- 入力: `(kana_input="あ", chosen_kanji="亜")` frequency=5、`(kana_input="あ", chosen_kanji="阿")` frequency=3 が存在する状態で `lookup("あ", 1)` を呼び出す
- 期待動作: frequency が高い `chosen_kanji="亜"` の entry のみが返される
- 設計判断: `ORDER BY frequency DESC, last_used_at DESC LIMIT ?2` が ordering と件数制限を担保する

**E8: validation reject の場合**

- 入力(非ひらがな kana_input): `record_choice("abc", "亜")` を呼び出す
- 期待動作: `Err(StorageError::InvalidField { name: "kana_input", .. })`
- 入力(空 chosen_kanji): `record_choice("あ", "")` を呼び出す
- 期待動作: `Err(StorageError::InvalidField { name: "chosen_kanji", .. })`
- 入力(PUA in chosen_kanji、allowlist 外): `record_choice("あ", "\u{E001}")` を呼び出す
- 期待動作: `Err(StorageError::InvalidField { name: "chosen_kanji", reason: "PUA char" })`
- 入力(256 bytes 超 kana_input): `"あ".repeat(86)` (= 258 bytes) を kana_input として渡す
- 期待動作: `Err(StorageError::InvalidField { name: "kana_input", reason: "byte size > 256" })`

**E9: 並行 record_choice の直列化**

- 入力: 複数スレッドが同時に `record_choice` を呼び出す
- 期待動作: `Mutex<Connection>` による直列化により deadlock / data race が発生せず、全 insert が完了する
- 設計判断: `lock_conn()` が `Mutex::lock().expect(...)` で排他取得するため、並行呼び出しは直列化される

**E10: system clock の backward jump**

- 入力: 2 回連続の `record_choice` 呼び出しの間に system clock が過去に戻る
- 期待動作: `last_used_at` が後の呼び出しで小さくなる可能性がある。LRU ordering の厳密性は保証されないが、eviction 動作は継続する。crash や `Err` は返さない
- 設計判断: NTP 補正や VM suspend/resume による backward jump は稀であり、self-recovering とみなす。明示的な単調増加保証は導入しない(実装コストに対して効果が薄い)

---

## 6. テスト戦略

### 6.1 Test layer 構成

P2-B の慣習(L1 単体 / L2 integration / L4 e2e)を踏襲する。L3 golden test は本 Phase の実装に対して適用しない。

| Layer | 配置 | 焦点 |
|---|---|---|
| L1 単体 | `crates/kotoha-storage/src/learning_cache/sqlite.rs` の `#[cfg(test)] mod tests` | SQL semantics / UPSERT / eviction / validation / cap override |
| L1 単体(ISP split 退行確認) | `crates/kotoha-storage/src/user_vocab/{sqlite,mock}.rs` の既存 test | trait split 後の退行ゼロを確認 |
| L2 integration | `crates/kotoha-storage/tests/learning_cache_store.rs`(新規) | factory 経由の wiring / 並行性 / 永続性 |
| L2 integration | `crates/kotoha-storage/tests/dict_user_vocab.rs`(既存修正) | factory split 後の API 変更に追従 |
| L2 proptest | `crates/kotoha-storage/tests/learning_cache_proptest.rs`(新規) | 3 invariant の property-based 検証 |
| L4 e2e | `crates/kotoha-cli/tests/dict_cli.rs`(既存修正) | CLI から ISP split 経由の動作確認 |

### 6.2 単体 test 一覧(23 件)

以下の 23 件の単体 test を `crates/kotoha-storage/src/learning_cache/sqlite.rs` の `#[cfg(test)] mod tests` に実装する。

| test 関数名 | 検証内容 |
|---|---|
| `record_choice_inserts_new_entry` | 新規 entry が learning_cache table に 1 行挿入される |
| `record_choice_increments_frequency_on_duplicate` | 同一 entry を 2 回 record すると frequency が 2 になる |
| `record_choice_creates_separate_entries_for_different_kanji` | 同一 kana_input / 異なる chosen_kanji が別 entry として保持される |
| `record_choice_updates_last_used_at_on_duplicate` | duplicate record 時に last_used_at が更新される |
| `record_choice_does_not_evict_below_cap` | 行数が cap 未満の間は eviction が発生しない |
| `record_choice_evicts_oldest_when_over_cap` | cap + 1 件目の insert 後に最古 entry が削除される |
| `record_choice_maintains_cap_on_burst_insert` | burst insert 後の行数が cap 以下を維持する |
| `record_choice_protects_recently_updated_entry_from_eviction` | record_choice で更新した entry は eviction 対象から外れる |
| `record_choice_rejects_non_hiragana_kana_input` | ASCII の kana_input は InvalidField を返す |
| `record_choice_rejects_empty_chosen_kanji` | 空文字列の chosen_kanji は InvalidField を返す |
| `record_choice_rejects_pua_in_chosen_kanji` | allowlist 外 PUA を含む chosen_kanji は InvalidField を返す |
| `record_choice_rejects_oversize_kana_input` | 257 bytes 超の kana_input は InvalidField を返す |
| `lookup_returns_empty_for_unknown_kana_input` | 存在しない kana_input に対して空 Vec を返す |
| `lookup_returns_single_match` | 1 件の entry を持つ kana_input に対して 1 件を返す |
| `lookup_orders_by_frequency_desc_then_last_used_at_desc` | 複数 entry が frequency DESC, last_used_at DESC 順に並ぶ |
| `lookup_respects_limit` | limit=1 を渡すと 1 件のみ返す |
| `lookup_rejects_non_hiragana` | ASCII の kana_input に対して InvalidField を返す |
| `evict_lru_returns_zero_when_under_cap` | `evict_lru(cap)` を呼び出したとき行数が `cap` 以下の場合は `Ok(0)` を返す |
| `evict_lru_deletes_excess_rows` | `evict_lru(cap)` を呼び出したとき行数が `cap` を超えている場合は超過分を削除して行数を `cap` に収める |
| `evict_lru_tiebreaks_by_id_when_last_used_at_equal` | `evict_lru(cap)` を呼び出したとき `last_used_at` が同値の entry が複数ある場合は `id ASC` で削除順を決定する |
| `cap_override_guard_restores_default_on_drop` | CapOverrideGuard を drop すると effective_max_rows が LEARNING_CACHE_MAX_ROWS に戻る |
| `effective_max_rows_returns_override_in_test` | override 設定中は effective_max_rows が override 値を返す |
| `effective_max_rows_returns_default_when_override_zero` | override が 0 の場合は effective_max_rows が LEARNING_CACHE_MAX_ROWS を返す |

### 6.3 L2 integration test(新規 / 修正)

**新規: `crates/kotoha-storage/tests/learning_cache_store.rs`**

| test 関数名 | 検証内容 |
|---|---|
| `factory_returns_reader_and_writer_sharing_same_data` | `learning_cache_reader` と `learning_cache_writer` が同一 DB を共有し、writer で記録した entry を reader が参照できる |
| `concurrent_record_choice_is_serialized` | 8 スレッドが同時に record_choice を呼び出しても deadlock が発生せず全件が記録される |
| `eviction_persists_across_factory_reopens` | file-backed DB で eviction 後に Database を再 open しても行数が cap 以下を維持する |

**既存修正: `crates/kotoha-storage/tests/dict_user_vocab.rs`**

`user_vocab_store()` factory 呼び出しを `user_vocab_reader()` / `user_vocab_writer()` に置換する。PASS 数は修正前後で同数を維持する。

### 6.4 L4 e2e test(既存修正)

`crates/kotoha-cli/tests/dict_cli.rs` の factory 呼び出し部分を ISP split 後の API に追従して書き換える。P2-C では LearningCache を CLI に公開しないため、新規 subcommand の追加は行わない。

### 6.5 proptest

`crates/kotoha-storage/tests/learning_cache_proptest.rs` に以下の 3 invariant を検証する proptest を実装する。

**Invariant 1: 任意の record sequence 後も行数が cap 以下**

```
input strategy:
  hiragana 1〜32 char (kana_input) ×
  kanji 1〜10 char (chosen_kanji) ×
  0〜200 件の record sequence
invariant:
  record sequence 完了後の total row count ≤ cap
  cap は CapOverrideGuard で 20 に設定し、record_choice が内部で参照する effective_max_rows() = 20 を使用する
  本 invariant は record_choice 経由の自動 eviction を対象とする。
  evict_lru(max_entries) の直接呼び出しは本 invariant の対象外であり、L1 単体 test で個別に検証する。
```

**Invariant 2: lookup 結果は frequency DESC で単調非増加**

```
input strategy:
  上記と同一の record sequence を実行後、kana_input を 1 件取り出して lookup を呼び出す
invariant:
  lookup 結果 results[i].frequency >= results[i+1].frequency (全 i に対して成立)
```

**Invariant 3: insert 対象全 entry が validate_surface を通過**

```
input strategy:
  validate_surface が Ok を返す文字列のみを生成する strategy
invariant:
  record_choice が Ok(()) を返す ⟺ validate_surface(chosen_kanji).is_ok() かつ validate_reading(kana_input).is_ok()
```

proptest の実行設定は `PROPTEST_CASES=64`(P2-B 同水準)とする。

### 6.6 test count baseline 予測

以下の表は P2-C 着地後の test count baseline 予測を示す。実値は実装後に確定する。

| 構成 | P2-B 着地(実測) | P2-C 着地予測 |
|---|---|---|
| `default` | 281 | 304 程度 |
| `dict` | 340 | 363 程度 |
| `dict-persist` | 355 | 384 程度 |

予測根拠: L1 単体 23 件 + L2 integration 3 件 + proptest 3 件 = 29 件の新規 test が追加される。既存 test の修正によるカウント変動は発生しない(削除 / 統合なし)。退行ゼロ(P2-B 着地時の全 355+ test PASS 維持)を必須条件とする。

---

## 7. 実装順序と branch / PR 戦略

### 7.1 実装 phase A〜E

実装を以下の 5 phase に分割して順次実行する。

| Phase | 内容 | 主要影響ファイル | 推定工数 |
|---|---|---|---|
| P2-C-A | UserVocab ISP split 先行(機械換装: 旧 `UserVocabStore` を Reader/Writer に分割、全 consumer を書換) | `user_vocab/store.rs` + `user_vocab/sqlite.rs` + `user_vocab/mock.rs` + `database.rs` + `dict_cli.rs` + `tests/dict_user_vocab.rs` | 1 day |
| P2-C-B | LearningCache 本実装(UPSERT / eviction / lookup / validate) | `learning_cache/mod.rs` + `learning_cache/sqlite.rs` | 1 day |
| P2-C-C | v002 migration + idx_learning_cache_last_used + factory LearningCache split | `migrations/v002_learning_cache_index.sql` + `migrations.rs` + `database.rs` | 半日 |
| P2-C-D | test 整備(L1 23 件 / L2 integration 3 件 / proptest 3 invariant / L4 修正) | `tests/` 配下 + `src/learning_cache/sqlite.rs` 内 `#[cfg(test)]` | 1 day |
| P2-C-E | team-review feedback 対応 + WBS 記録 + PR 作成 | review feedback 次第 | 半日〜1 day |

合計推定工数は 3.5〜4 day である。Branch Scope Policy の 20 file / 1000 line 上限に収まる想定であるが、P2-C-A(ISP split)で 1 PR の変更ファイル数が 20 を超えることが見込まれる場合は §7.2 のフォールバックを適用する。

### 7.2 branch / PR 戦略

- branch 名: `feature/<issue#>-p2-c-learning-cache`(ISSUE 番号は本 spec 承認後に起票)
- PR: 単一 PR を基本とする(ISP split は LearningCache 本実装の前提条件であるため分離すると中間状態でビルドが壊れる)
- review: Medium tier を適用する(team-review 4 dimension: security / performance / architecture / testing + owasp-security + secrets-check + simplify)
- フォールバック: Branch Scope Policy 上限超過時は「P2-C-A ISP split のみ先行 PR」→「P2-C-B 以降 LearningCache 本実装 PR」の 2 PR 分割を適用する。分割時は先行 PR に `LearningCacheStore` stub が残ること(互換ビルド可能)を確認してから merge する

### 7.3 後段 Issue 起票候補(本 Phase 着地後)

P2-C 着地後に以下の Issue を新規起票する。

- **connection pool(perf-M-1)**: `Mutex<Connection>` をコネクションプールに置き換える。Phase 3 IBus engine から高頻度で呼び出す前提で遅延を測定し、必要性が確認された場合に実施する
- **Mutex poison handling 改善(sec-L-2)**: `lock_conn` が panic する代わりに `StorageError::Sqlite` を返す recover 経路を追加する。Phase 3 前提として実施する
- **LearningCache CLI 公開検討(Phase 3-A以降)**: `kotoha-dict cache list` / `kotoha-dict cache flush` の subcommand を追加するかを Phase 3 kick-off で判断する
- **cap 値の動的化 / config 化(Phase 5)**: `LEARNING_CACHE_MAX_ROWS` を設定ファイルから読み込む機構を Phase 5 personalization で追加する
- **v002 migration の down 整備(YAGNI 解除時)**: forward-only から bidirectional migration に切り替える必要が生じた場合に別 ADR で方針確定後に実施する

### 7.4 Phase 状態への影響

P2-C 着地後の Phase 2 進捗マトリクスを以下に示す。

| milestone | 状態 |
|---|---|
| P2-A | 完了 |
| P2-A hardening | 完了 |
| P2-B | 完了 |
| P2-C | **完了 ← 本 Phase** |
| P2-D | 未着手(Hybrid backend / Ranker) |

P2-D(Hybrid backend / Ranker 統合)の着手前に LearningCache の本実装 + Reader/Writer ISP boundary が確立した状態を作ることが本 Phase の最大成果である。P2-D は本 Phase が確立した `LearningCacheReader` / `LearningCacheWriter` の 4 trait を組み合わせて Ranker を構成する。

---

## 8. 受容基準(Acceptance Criteria)

以下の全項目を満たした状態を P2-C 完了とする。

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

## 9. 参考 / 関連文書

本設計書は以下の文書を参照した。矛盾がある場合は本設計書が優先する。

- `docs/adr/0014-phase-2-dictionary-layer-architecture.md` — Phase 2 dictionary layer の全体 ADR
- `docs/adr/0015-kotoha-storage-sqlite-adoption.md` — kotoha-storage SQLite 採用根拠
- `docs/superpowers/specs/2026-04-25-kotoha-phase-2-design.md` §3.2 / §5.2 / §6.3 — Phase 2 全体 spec の LearningCache 関連節
- `docs/superpowers/specs/2026-04-25-p2-b-user-dictionary-design.md` §5.2 / §6.3 — P2-B の schema 設計と trait 定義
- `docs/wbs/2026-04-25-feature-98-p2-b-user-dictionary.md` — P2-B 実装ログ(cap override / proptest / validation パターンの根拠)
- session handoff memory `project_session_handoff_2026-04-25.md` — P2-C 着手前の状態確認
- `crates/kotoha-storage/migrations/v001_initial.sql` — `learning_cache` table schema(本設計書が前提とする schema)
- `crates/kotoha-storage/src/learning_cache/mod.rs` — P2-B 着地時の `LearningCacheStore` trait skeleton
- `crates/kotoha-storage/src/learning_cache/sqlite.rs` — P2-B 着地時の stub 実装(本 Phase で本実装に置換)
- `crates/kotoha-storage/src/user_vocab/sqlite.rs` — `QuotaOverrideGuard` パターンの原典(`CapOverrideGuard` がこれを踏襲する)
- `crates/kotoha-storage/src/validation.rs` — `validate_reading` / `validate_surface` の実装(本 Phase で再利用)
- `crates/kotoha-storage/src/error.rs` — `StorageError` variant 定義(本 Phase で追加なし)
