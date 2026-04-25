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
    fn find_by_reading(
        &self,
        reading: &str,
        limit: usize,
    ) -> Result<Vec<UserVocabRecord>, StorageError>;
    fn list_all(&self, limit: usize, offset: usize) -> Result<Vec<UserVocabRecord>, StorageError>;
    fn insert(&self, record: UserVocabRecord) -> Result<i64, StorageError>;
    fn delete_by_id(&self, id: i64) -> Result<(), StorageError>;
    fn delete_by_surface_reading(&self, surface: &str, reading: &str) -> Result<(), StorageError>;
    /// `reading` の prefix match で entries を返す(spec §7.4 `--reading <PREFIX>`)。
    ///
    /// SQL `reading LIKE 'PREFIX%'` を使用し、score 降順で `limit` 件返す。
    fn find_by_prefix(
        &self,
        reading_prefix: &str,
        limit: usize,
    ) -> Result<Vec<UserVocabRecord>, StorageError>;
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
        let c = UserVocabRecord {
            id: Some(2),
            ..a.clone()
        };
        assert_ne!(a, c);
    }
}
