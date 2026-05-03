//! Domain-side ports for learning cache and user vocabulary lookups.
//!
//! Phase 3-A spec §3.1 凍結事項である「`kotoha-engine-core` は host / persistence
//! 詳細層に依存しない domain core layer」を実装で実現するために、本 module は
//! engine が必要とする学習・ユーザ辞書の **抽象境界** を engine-core 側に定義する。
//!
//! Hexagonal Architecture の用語で言えば、本 module の trait は engine の
//! **driven port**(engine → 外部)に該当する。`kotoha-storage` 側の SQLite /
//! Mock 実装は本 trait の adapter として `kotoha-storage::engine_adapter`
//! module に置かれる(C3 / B0h-a、ISSUE #149 / #153)。
//!
//! # 依存方向
//!
//! - 本 module は `kotoha-storage` を一切参照しない
//! - `kotoha-storage` 側が `engine-core` に依存して trait を `impl` する
//! - 結果として `kotoha-engine-core/Cargo.toml [dependencies]` から
//!   `kotoha-storage` が消え、循環依存も発生しない
//!
//! # Record types との重複について
//!
//! `kotoha-storage::learning_cache::LearningCacheRecord` /
//! `kotoha-storage::user_vocab::UserVocabRecord` は永続化詳細層の row 表現として
//! 引き続き storage crate に残す。本 module の
//! [`LearningCacheRecord`] / [`UserVocabRecord`] は domain layer 用の値型で、
//! adapter 層で相互に [`From`] 変換される。フィールド形状を意図的に一致させて
//! 変換コストを `Move` 1 回に抑えてある。

use thiserror::Error;

/// Domain-side error returned by [`LearningRecorder`] / [`LearningLookup`] /
/// [`UserVocabLookup`].
///
/// 永続化層の `kotoha_storage::error::StorageError` から adapter 層で本型に
/// 変換される(`From<StorageError> for LearningError`、`engine_adapter`
/// module 参照)。具体 backend(SQLite / in-memory mock / 将来の別 backend)に
/// 依存しない opaque な variant にする。
///
/// # Variants
///
/// - [`LearningError::InvalidField`]:呼び出し側 input が validation を通らない
///   (e.g. ASCII を含む `kana_input`、空文字 `chosen_kanji` 等)
/// - [`LearningError::NotFound`]:`delete_by_*` 系で対象 row が存在しない
/// - [`LearningError::QuotaExceeded`]:行数 cap に到達して insert を断った
/// - [`LearningError::Backend`]:上記以外の adapter 内部障害(SQLite I/O / disk
///   full / migration 失敗 / mutex poisoning 等)を opaque に伝える単一 variant。
///   呼び出し側は具体原因を識別する必要が無い場合の包括 fallback として使う
#[derive(Debug, Error)]
pub enum LearningError {
    /// `kana_input` / `chosen_kanji` / `surface` 等の field validation 違反。
    #[error("invalid field {field}: {reason}")]
    InvalidField {
        /// validation を通らなかった field 名(例: `kana_input` / `chosen_kanji`)。
        field: String,
        /// 違反内容(例: `"non-hiragana char"`、`"PUA char"`、`"empty"`)。
        reason: String,
    },
    /// 削除 / 参照対象 row が存在しなかった。
    #[error("entry not found")]
    NotFound,
    /// 行数上限に達したため insert を拒否した(`user_vocab` の `QuotaExceeded`)。
    #[error("quota exceeded: {table} max {max} rows")]
    QuotaExceeded {
        /// 対象 table 名(例: `"user_vocab"`)。
        table: String,
        /// 上限値。
        max: usize,
    },
    /// 上記以外の adapter 内部障害(SQLite / IO / migration / mutex poison 等)。
    #[error("backend error: {0}")]
    Backend(String),
}

/// 学習キャッシュ 1 行(domain layer 表現)。
///
/// `kotoha-storage::learning_cache::LearningCacheRecord` と field 形状が一致する。
/// adapter 層で [`From`] 変換される。
///
/// # Invariants
///
/// - `id` は DB から取得した場合のみ有効(`> 0`)
/// - `frequency` は `1` 以上
/// - `last_used_at` は UNIX epoch seconds(非負)
#[derive(Debug, Clone, PartialEq)]
pub struct LearningCacheRecord {
    /// 永続化層が割り当てた行 id。
    pub id: i64,
    /// 変換前のひらがな入力。
    pub kana_input: String,
    /// ユーザが選択した変換後文字列。
    pub chosen_kanji: String,
    /// `(kana_input, chosen_kanji)` の組み合わせが選ばれた累積回数。
    pub frequency: u32,
    /// 最後に選択された時刻(UNIX epoch seconds)。
    pub last_used_at: i64,
}

/// ユーザ辞書 1 行(domain layer 表現)。
///
/// `kotoha-storage::user_vocab::UserVocabRecord` と field 形状が一致する。
///
/// # Invariants
///
/// - `id` は insert 前は `None`、DB から取得した場合は `Some`
/// - `score` は finite + 非負
/// - `created_at` / `updated_at` は UNIX epoch seconds
#[derive(Debug, Clone, PartialEq)]
pub struct UserVocabRecord {
    /// 永続化層が割り当てた行 id(insert 前は `None`)。
    pub id: Option<i64>,
    /// 変換後の表記文字列。
    pub surface: String,
    /// ひらがな読み(canonical)。
    pub reading: String,
    /// 品詞。
    pub pos: String,
    /// スコア(finite + 非負)。
    pub score: f32,
    /// 作成時刻(UNIX epoch seconds)。
    pub created_at: i64,
    /// 更新時刻(UNIX epoch seconds)。
    pub updated_at: i64,
}

/// Engine が確定操作で使う学習キャッシュへの **書き込み port**。
///
/// 主な用途:`KotohaEngine` が `commit_text` 時に
/// `(kana_input, chosen_kanji)` を学習する経路。
///
/// # Preconditions
///
/// - `kana_input` はひらがな canonical(`U+3040..=U+309F + U+30FC + U+30FB`)
/// - `chosen_kanji` は non-empty / ≤256 bytes / no control char / no bidi /
///   no disallowed PUA
///
/// # Postconditions
///
/// - `record_choice` 成功後は該当 entry の `frequency` が 1 以上増加し、
///   `last_used_at` が現在時刻以上の値に更新される
/// - 行数 cap は adapter 側の責務(`record_choice` 完了時点で cap 以内)
///
/// # Errors
///
/// - [`LearningError::InvalidField`]:field validation 違反
/// - [`LearningError::Backend`]:adapter 内部障害
pub trait LearningRecorder: Send + Sync {
    /// `(kana_input, chosen_kanji)` を学習キャッシュに記録する(UPSERT)。
    fn record_choice(&self, kana_input: &str, chosen_kanji: &str) -> Result<(), LearningError>;

    /// LRU(最終使用が最古)entry を `max_entries` 行に収まるまで削除する。
    ///
    /// # Postconditions
    ///
    /// - 戻り値は実際に削除された行数
    /// - 行数が `max_entries` 以下の場合は `Ok(0)` を返す
    fn evict_lru(&self, max_entries: usize) -> Result<usize, LearningError>;
}

/// Engine が候補生成で使う学習キャッシュへの **読み出し port**。
///
/// 主な用途:`HybridRanker` が dict 候補と並列で `kana_input` 一致 entry を
/// 取得して候補リストに合流させる経路。
///
/// # Preconditions
///
/// - `kana_input` はひらがな canonical
/// - `limit` は 0 より大きい値を推奨する(0 を渡した場合は空 `Vec` を返す)
///
/// # Postconditions
///
/// - 戻り値は `frequency DESC, last_used_at DESC` 順で `limit` 件以下
/// - 該当 entry が存在しない場合は `Ok(Vec::new())` を返す
///
/// # Errors
///
/// - [`LearningError::InvalidField`]:`kana_input` validation 違反
/// - [`LearningError::Backend`]:adapter 内部障害
pub trait LearningLookup: Send + Sync {
    /// `kana_input` に対応する entry を `frequency DESC, last_used_at DESC` 順で返す。
    fn lookup(
        &self,
        kana_input: &str,
        limit: usize,
    ) -> Result<Vec<LearningCacheRecord>, LearningError>;
}

/// Engine が候補生成で使うユーザ辞書への **読み出し port**。
///
/// 主な用途:`HybridRanker` が SudachiDict と並列で `reading` 一致 entry を
/// 取得して候補リストに合流させる経路(spec §6.6)。
///
/// # Preconditions
///
/// - `reading` / `reading_prefix` はひらがな canonical
///
/// # Postconditions
///
/// - 戻り値は `score DESC` 順で `limit` 件以下
/// - 該当 entry が存在しない場合は `Ok(Vec::new())` を返す
/// - `find_by_id` の対象不在は `Ok(None)`(エラーではない)
///
/// # Errors
///
/// - [`LearningError::InvalidField`]:`reading` validation 違反
/// - [`LearningError::Backend`]:adapter 内部障害
pub trait UserVocabLookup: Send + Sync {
    /// `reading` が完全一致する entry を `score DESC` 順で返す。
    fn find_by_reading(
        &self,
        reading: &str,
        limit: usize,
    ) -> Result<Vec<UserVocabRecord>, LearningError>;

    /// 主キー `id` で entry 1 件を引く。不在の場合は `Ok(None)`。
    fn find_by_id(&self, id: i64) -> Result<Option<UserVocabRecord>, LearningError>;

    /// `reading` が `reading_prefix` で前方一致する entry を `score DESC` 順で返す。
    fn find_by_prefix(
        &self,
        reading_prefix: &str,
        limit: usize,
    ) -> Result<Vec<UserVocabRecord>, LearningError>;

    /// 全 entry を `id ASC` 順でページネーション付きで返す。
    fn list_all(&self, limit: usize, offset: usize) -> Result<Vec<UserVocabRecord>, LearningError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn learning_error_display_includes_field_name() {
        let err = LearningError::InvalidField {
            field: "kana_input".to_string(),
            reason: "non-hiragana char".to_string(),
        };
        let msg = format!("{err}");
        assert!(msg.contains("kana_input"));
        assert!(msg.contains("non-hiragana"));
    }

    #[test]
    fn learning_error_not_found_is_stable_string() {
        let err = LearningError::NotFound;
        assert_eq!(format!("{err}"), "entry not found");
    }

    #[test]
    fn learning_error_quota_exceeded_includes_table_and_max() {
        let err = LearningError::QuotaExceeded {
            table: "user_vocab".to_string(),
            max: 50_000,
        };
        let msg = format!("{err}");
        assert!(msg.contains("user_vocab"));
        assert!(msg.contains("50000") || msg.contains("50_000"));
    }

    #[test]
    fn learning_error_backend_wraps_inner_message() {
        let err = LearningError::Backend("disk I/O failure".to_string());
        let msg = format!("{err}");
        assert!(msg.contains("disk I/O failure"));
    }

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
        assert_eq!(r.kana_input, "あい");
        assert_eq!(r.chosen_kanji, "愛");
    }

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
        assert_eq!(r.reading, "ひのおか");
    }
}
