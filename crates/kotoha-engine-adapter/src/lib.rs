//! `kotoha-engine-adapter`: Hexagonal adapter wiring `kotoha-engine-core`
//! domain ports to `kotoha-storage` SQLite / in-memory backends.
//!
//! Phase 3-B B0h-a (ISSUE #149 / sub #153) で C3 hexagonal driven port 反転を
//! 実現するための adapter 専用 crate。`kotoha-engine-core::learning_port` で
//! 定義された domain trait
//! ([`LearningRecorder`] / [`LearningLookup`] / [`UserVocabLookup`])を
//! `kotoha-storage` 既存の SQLite / Mock 実装に対して [`impl`] する。
//!
//! # 依存方向(Hexagonal direction)
//!
//! - 本 crate は `kotoha-engine-core`(domain)と `kotoha-storage`(adapter
//!   下層)の双方に依存する
//! - `kotoha-engine-core` は `kotoha-storage` を一切参照しない
//! - `kotoha-storage` は `kotoha-engine-core` を参照しない
//! - 結果として workspace の dependency graph に循環は発生しない
//!
//! # Record / Error 変換
//!
//! domain layer の [`learning_port::LearningCacheRecord`] /
//! [`learning_port::UserVocabRecord`] /
//! [`learning_port::LearningError`] と、`kotoha-storage` 内部の
//! [`kotoha_storage::learning_cache::LearningCacheRecord`] /
//! [`kotoha_storage::user_vocab::UserVocabRecord`] /
//! [`kotoha_storage::error::StorageError`] は形状互換だが別型として保つ。本
//! crate で [`From`] による境界変換を提供し、呼び出し側の boilerplate を削減する。

use std::sync::Arc;

use kotoha_engine_core::learning_port::{
    self, LearningError, LearningLookup, LearningRecorder, UserVocabLookup,
};

use kotoha_storage::error::StorageError;
use kotoha_storage::learning_cache::{
    LearningCacheReader, LearningCacheRecord as StorageLearningCacheRecord, LearningCacheWriter,
    MockLearningCacheStore, SqliteLearningCacheStore,
};
use kotoha_storage::user_vocab::{
    MockUserVocabStore, SqliteUserVocabStore, UserVocabReader,
    UserVocabRecord as StorageUserVocabRecord,
};

// ----------------------------------------------------------------------------
// Error 変換
// ----------------------------------------------------------------------------

/// Convert a `kotoha-storage` error into the engine-core domain error.
///
/// # Postconditions
///
/// - [`StorageError::InvalidField`] → [`LearningError::InvalidField`]
///   (`name` → `field`、`reason` はそのまま)
/// - [`StorageError::NotFound`] → [`LearningError::NotFound`]
/// - [`StorageError::QuotaExceeded`] → [`LearningError::QuotaExceeded`]
/// - 上記以外(`Sqlite` / `Io` / `Migration` / `InvalidPath` /
///   `DuplicateEntry` / `HomeDirNotFound`)は [`LearningError::Backend`] に
///   `format!("{e}")` で `String` 化して詰める。adapter 層は domain に
///   backend 識別情報を漏らさず opaque な error として扱う
pub fn map_storage_error(err: StorageError) -> LearningError {
    match err {
        StorageError::InvalidField { name, reason } => LearningError::InvalidField {
            field: name,
            reason,
        },
        StorageError::NotFound => LearningError::NotFound,
        StorageError::QuotaExceeded { table, max } => LearningError::QuotaExceeded { table, max },
        other => LearningError::Backend(format!("{other}")),
    }
}

// ----------------------------------------------------------------------------
// Record 変換
// ----------------------------------------------------------------------------

/// Convert storage row representation into engine-core domain value.
fn map_learning_cache_record(r: StorageLearningCacheRecord) -> learning_port::LearningCacheRecord {
    learning_port::LearningCacheRecord {
        id: r.id,
        kana_input: r.kana_input,
        chosen_kanji: r.chosen_kanji,
        frequency: r.frequency,
        last_used_at: r.last_used_at,
    }
}

/// Convert storage row representation into engine-core domain value.
fn map_user_vocab_record(r: StorageUserVocabRecord) -> learning_port::UserVocabRecord {
    learning_port::UserVocabRecord {
        id: r.id,
        surface: r.surface,
        reading: r.reading,
        pos: r.pos,
        score: r.score,
        created_at: r.created_at,
        updated_at: r.updated_at,
    }
}

// ----------------------------------------------------------------------------
// LearningRecorder / LearningLookup / UserVocabLookup 実装(SQLite + Mock)
// ----------------------------------------------------------------------------

/// Adapter wrapping an `Arc<T>` storage-side `LearningCacheWriter` to satisfy
/// the engine-core [`LearningRecorder`] domain port.
///
/// 単純な newtype:内部 `Arc<T>` (`SqliteLearningCacheStore` /
/// `MockLearningCacheStore`)に同名 method を委譲し、storage 側の
/// [`StorageError`] を [`LearningError`] に変換する。`Arc<T>` を保持するので
/// 同じ inner を [`LearningLookupAdapter`] と shared できる(同一 store に
/// `LearningRecorder` と `LearningLookup` の両 port を立てる典型構成を支える)。
pub struct LearningRecorderAdapter<T: LearningCacheWriter + Send + Sync + 'static> {
    inner: Arc<T>,
}

impl<T: LearningCacheWriter + Send + Sync + 'static> LearningRecorderAdapter<T> {
    /// Wrap an existing storage-side writer.
    pub fn new(inner: Arc<T>) -> Self {
        Self { inner }
    }
}

impl<T: LearningCacheWriter + Send + Sync + 'static> LearningRecorder
    for LearningRecorderAdapter<T>
{
    fn record_choice(&self, kana_input: &str, chosen_kanji: &str) -> Result<(), LearningError> {
        LearningCacheWriter::record_choice(self.inner.as_ref(), kana_input, chosen_kanji)
            .map_err(map_storage_error)
    }

    fn evict_lru(&self, max_entries: usize) -> Result<usize, LearningError> {
        LearningCacheWriter::evict_lru(self.inner.as_ref(), max_entries).map_err(map_storage_error)
    }
}

/// Adapter wrapping an `Arc<T>` storage-side `LearningCacheReader` to satisfy
/// the engine-core [`LearningLookup`] domain port.
pub struct LearningLookupAdapter<T: LearningCacheReader + Send + Sync + 'static> {
    inner: Arc<T>,
}

impl<T: LearningCacheReader + Send + Sync + 'static> LearningLookupAdapter<T> {
    /// Wrap an existing storage-side reader.
    pub fn new(inner: Arc<T>) -> Self {
        Self { inner }
    }
}

impl<T: LearningCacheReader + Send + Sync + 'static> LearningLookup for LearningLookupAdapter<T> {
    fn lookup(
        &self,
        kana_input: &str,
        limit: usize,
    ) -> Result<Vec<learning_port::LearningCacheRecord>, LearningError> {
        let rows = LearningCacheReader::lookup(self.inner.as_ref(), kana_input, limit)
            .map_err(map_storage_error)?;
        Ok(rows.into_iter().map(map_learning_cache_record).collect())
    }
}

/// Adapter wrapping an `Arc<T>` storage-side `UserVocabReader` to satisfy the
/// engine-core [`UserVocabLookup`] domain port.
pub struct UserVocabLookupAdapter<T: UserVocabReader + Send + Sync + 'static> {
    inner: Arc<T>,
}

impl<T: UserVocabReader + Send + Sync + 'static> UserVocabLookupAdapter<T> {
    /// Wrap an existing storage-side reader.
    pub fn new(inner: Arc<T>) -> Self {
        Self { inner }
    }
}

impl<T: UserVocabReader + Send + Sync + 'static> UserVocabLookup for UserVocabLookupAdapter<T> {
    fn find_by_reading(
        &self,
        reading: &str,
        limit: usize,
    ) -> Result<Vec<learning_port::UserVocabRecord>, LearningError> {
        let rows = UserVocabReader::find_by_reading(self.inner.as_ref(), reading, limit)
            .map_err(map_storage_error)?;
        Ok(rows.into_iter().map(map_user_vocab_record).collect())
    }

    fn find_by_id(&self, id: i64) -> Result<Option<learning_port::UserVocabRecord>, LearningError> {
        let row =
            UserVocabReader::find_by_id(self.inner.as_ref(), id).map_err(map_storage_error)?;
        Ok(row.map(map_user_vocab_record))
    }

    fn find_by_prefix(
        &self,
        reading_prefix: &str,
        limit: usize,
    ) -> Result<Vec<learning_port::UserVocabRecord>, LearningError> {
        let rows = UserVocabReader::find_by_prefix(self.inner.as_ref(), reading_prefix, limit)
            .map_err(map_storage_error)?;
        Ok(rows.into_iter().map(map_user_vocab_record).collect())
    }

    fn list_all(
        &self,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<learning_port::UserVocabRecord>, LearningError> {
        let rows = UserVocabReader::list_all(self.inner.as_ref(), limit, offset)
            .map_err(map_storage_error)?;
        Ok(rows.into_iter().map(map_user_vocab_record).collect())
    }
}

// ----------------------------------------------------------------------------
// Convenience constructors for the most common backend types
// ----------------------------------------------------------------------------

/// Wrap an `Arc<SqliteLearningCacheStore>` into the engine-core domain port pair.
///
/// 戻り値は `(LearningRecorder, LearningLookup)` の Arc 対。同一 inner を 2 つの
/// adapter newtype が共有する。bin の DI で `Arc::clone` の手間を 1 回に抑える
/// ために用意した便宜 helper。
pub fn arc_sqlite_learning_cache(
    inner: Arc<SqliteLearningCacheStore>,
) -> (Arc<dyn LearningRecorder>, Arc<dyn LearningLookup>) {
    let recorder: Arc<dyn LearningRecorder> =
        Arc::new(LearningRecorderAdapter::new(Arc::clone(&inner)));
    let lookup: Arc<dyn LearningLookup> = Arc::new(LearningLookupAdapter::new(inner));
    (recorder, lookup)
}

/// Wrap an `Arc<MockLearningCacheStore>` into the engine-core domain port pair.
pub fn arc_mock_learning_cache(
    inner: Arc<MockLearningCacheStore>,
) -> (Arc<dyn LearningRecorder>, Arc<dyn LearningLookup>) {
    let recorder: Arc<dyn LearningRecorder> =
        Arc::new(LearningRecorderAdapter::new(Arc::clone(&inner)));
    let lookup: Arc<dyn LearningLookup> = Arc::new(LearningLookupAdapter::new(inner));
    (recorder, lookup)
}

/// Wrap an `Arc<SqliteUserVocabStore>` into the engine-core domain port.
pub fn arc_sqlite_user_vocab(inner: Arc<SqliteUserVocabStore>) -> Arc<dyn UserVocabLookup> {
    Arc::new(UserVocabLookupAdapter::new(inner))
}

/// Wrap an `Arc<MockUserVocabStore>` into the engine-core domain port.
pub fn arc_mock_user_vocab(inner: Arc<MockUserVocabStore>) -> Arc<dyn UserVocabLookup> {
    Arc::new(UserVocabLookupAdapter::new(inner))
}

#[cfg(test)]
mod tests {
    use super::*;
    use kotoha_storage::user_vocab::UserVocabWriter;
    use std::path::PathBuf;

    /// `StorageError::InvalidField { name, reason }` が
    /// `LearningError::InvalidField { field, reason }` に同値マップされる。
    #[test]
    fn invalid_field_maps_to_learning_invalid_field() {
        let err = map_storage_error(StorageError::InvalidField {
            name: "kana_input".to_string(),
            reason: "non-hiragana char".to_string(),
        });
        match err {
            LearningError::InvalidField { field, reason } => {
                assert_eq!(field, "kana_input");
                assert_eq!(reason, "non-hiragana char");
            }
            other => panic!("unexpected variant: {other:?}"),
        }
    }

    /// `StorageError::NotFound` が `LearningError::NotFound` に同値マップされる。
    #[test]
    fn not_found_maps_directly() {
        let err = map_storage_error(StorageError::NotFound);
        assert!(matches!(err, LearningError::NotFound));
    }

    /// `StorageError::QuotaExceeded` が同値マップされる。
    #[test]
    fn quota_exceeded_maps_directly() {
        let err = map_storage_error(StorageError::QuotaExceeded {
            table: "user_vocab".to_string(),
            max: 50_000,
        });
        match err {
            LearningError::QuotaExceeded { table, max } => {
                assert_eq!(table, "user_vocab");
                assert_eq!(max, 50_000);
            }
            other => panic!("unexpected variant: {other:?}"),
        }
    }

    /// 上記以外の variant は [`LearningError::Backend`] に opaque で落ちる
    /// (adapter は backend 識別情報を domain に漏らさない)。
    #[test]
    fn other_variants_map_to_backend_opaque() {
        for src in [
            StorageError::HomeDirNotFound,
            StorageError::Migration("schema mismatch".to_string()),
            StorageError::DuplicateEntry {
                surface: "x".to_string(),
                reading: "y".to_string(),
            },
            StorageError::InvalidPath {
                path: PathBuf::from("/etc/kotoha"),
                reason: "blacklisted".to_string(),
            },
        ] {
            let err = map_storage_error(src);
            assert!(
                matches!(err, LearningError::Backend(_)),
                "expected Backend variant, got {err:?}"
            );
        }
    }

    /// `Backend` variant は元 `Display` 表現を message として保持する。
    #[test]
    fn backend_variant_preserves_display_message() {
        let src = StorageError::Migration("schema v3 missing".to_string());
        let err = map_storage_error(src);
        let msg = format!("{err}");
        assert!(msg.contains("schema v3 missing"));
    }

    /// `LearningCacheRecord` の round-trip 変換が field 値を保持する。
    #[test]
    fn learning_cache_record_conversion_round_trips_fields() {
        let storage = StorageLearningCacheRecord {
            id: 7,
            kana_input: "あい".to_string(),
            chosen_kanji: "愛".to_string(),
            frequency: 3,
            last_used_at: 1745529600,
        };
        let domain = map_learning_cache_record(storage);
        assert_eq!(domain.id, 7);
        assert_eq!(domain.kana_input, "あい");
        assert_eq!(domain.chosen_kanji, "愛");
        assert_eq!(domain.frequency, 3);
        assert_eq!(domain.last_used_at, 1745529600);
    }

    /// `UserVocabRecord` の round-trip 変換が field 値を保持する。
    #[test]
    fn user_vocab_record_conversion_round_trips_fields() {
        let storage = StorageUserVocabRecord {
            id: Some(11),
            surface: "日野岡".to_string(),
            reading: "ひのおか".to_string(),
            pos: "名詞-固有名詞-人名".to_string(),
            score: 1.5,
            created_at: 1745000000,
            updated_at: 1745529600,
        };
        let domain = map_user_vocab_record(storage);
        assert_eq!(domain.id, Some(11));
        assert_eq!(domain.surface, "日野岡");
        assert_eq!(domain.reading, "ひのおか");
        assert_eq!(domain.pos, "名詞-固有名詞-人名");
        assert!((domain.score - 1.5).abs() < f32::EPSILON);
        assert_eq!(domain.created_at, 1745000000);
        assert_eq!(domain.updated_at, 1745529600);
    }

    /// `MockLearningCacheStore` を `arc_mock_learning_cache` で wrap し、
    /// 戻り値の `LearningRecorder + LearningLookup` で record → lookup
    /// round-trip が機能する。
    #[test]
    fn mock_learning_cache_store_satisfies_domain_recorder_and_lookup() {
        let store = Arc::new(MockLearningCacheStore::new());
        let (recorder, lookup) = arc_mock_learning_cache(store);
        recorder.record_choice("あい", "愛").expect("record ok");
        let result = lookup.lookup("あい", 10).expect("lookup ok");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].chosen_kanji, "愛");
        assert_eq!(result[0].frequency, 1);
    }

    /// `MockUserVocabStore` を `arc_mock_user_vocab` で wrap し、
    /// insert(storage trait)→ find_by_reading(domain trait)で取得できる。
    #[test]
    fn mock_user_vocab_store_satisfies_domain_lookup() {
        let store = Arc::new(MockUserVocabStore::new());
        UserVocabWriter::insert(
            store.as_ref(),
            StorageUserVocabRecord {
                id: None,
                surface: "日野岡".to_string(),
                reading: "ひのおか".to_string(),
                pos: "名詞".to_string(),
                score: 1.0,
                created_at: 0,
                updated_at: 0,
            },
        )
        .expect("insert ok");
        let lookup = arc_mock_user_vocab(store);
        let result = lookup.find_by_reading("ひのおか", 10).expect("find ok");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].surface, "日野岡");
    }

    /// `MockUserVocabStore::find_by_id` not-found path が `Ok(None)` を返す。
    #[test]
    fn mock_user_vocab_find_by_id_returns_none_for_absent() {
        let store = Arc::new(MockUserVocabStore::new());
        let lookup = arc_mock_user_vocab(store);
        let got = lookup.find_by_id(999).expect("find ok");
        assert!(got.is_none());
    }

    /// `LearningRecorderAdapter::new` で個別 wrap した recorder が
    /// `LearningRecorder` trait を満たす(adapter 単体構築の sanity check)。
    #[test]
    fn learning_recorder_adapter_implements_recorder_trait() {
        let store = Arc::new(MockLearningCacheStore::new());
        let recorder: Arc<dyn LearningRecorder> =
            Arc::new(LearningRecorderAdapter::new(Arc::clone(&store)));
        recorder.record_choice("あ", "亜").expect("record ok");
        // round-trip via storage's reader to confirm the inner store was actually
        // mutated through the adapter wrapper.
        let inner_rows =
            LearningCacheReader::lookup(store.as_ref(), "あ", 10).expect("storage-side lookup ok");
        assert_eq!(inner_rows.len(), 1);
        assert_eq!(inner_rows[0].chosen_kanji, "亜");
    }
}
