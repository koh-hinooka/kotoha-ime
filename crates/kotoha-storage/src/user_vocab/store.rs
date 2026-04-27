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
/// - [`StorageError::InvalidField`][]:`reading` が validation 違反の場合
/// - [`StorageError::Sqlite`][]:SQLite backend 障害の場合
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
    /// - [`StorageError::InvalidField`][]:`reading` が hiragana 以外を含む場合
    /// - [`StorageError::Sqlite`][]:SQLite backend 障害の場合
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
    /// - [`StorageError::Sqlite`][]:SQLite backend 障害の場合
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
    /// - [`StorageError::InvalidField`][]:`reading_prefix` が validation 違反の場合
    /// - [`StorageError::Sqlite`][]:SQLite backend 障害の場合
    fn find_by_prefix(
        &self,
        reading_prefix: &str,
        limit: usize,
    ) -> Result<Vec<UserVocabRecord>, StorageError>;

    /// 全 entry を `id ASC` 順でページネーション付きで返す。
    ///
    /// # Errors
    ///
    /// - [`StorageError::Sqlite`][]:SQLite backend 障害の場合
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
/// - [`StorageError::InvalidField`][]:validation 違反の場合
/// - [`StorageError::DuplicateEntry`][]:UNIQUE 制約違反の場合
/// - [`StorageError::NotFound`][]:削除対象が存在しない場合
/// - [`StorageError::Sqlite`][]:SQLite backend 障害の場合
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
    /// - [`StorageError::InvalidField`][]:field validation 違反の場合
    /// - [`StorageError::DuplicateEntry`][]:UNIQUE(surface, reading) 違反の場合
    /// - [`StorageError::QuotaExceeded`][]:行数が `USER_VOCAB_MAX_ROWS` に到達している場合
    /// - [`StorageError::Sqlite`][]:SQLite backend 障害の場合
    fn insert(&self, record: UserVocabRecord) -> Result<i64, StorageError>;

    /// 主キー `id` で entry を 1 件削除する。
    ///
    /// # Errors
    ///
    /// - [`StorageError::NotFound`][]:`id` が存在しない場合
    /// - [`StorageError::Sqlite`][]:SQLite backend 障害の場合
    fn delete_by_id(&self, id: i64) -> Result<(), StorageError>;

    /// `(surface, reading)` で entry を 1 件削除する。
    ///
    /// # Errors
    ///
    /// - [`StorageError::NotFound`][]:対象 entry が存在しない場合
    /// - [`StorageError::Sqlite`][]:SQLite backend 障害の場合
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
