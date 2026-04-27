//! `crates/kotoha-storage/src/learning_cache/mod.rs`
//! LearningCacheReader / LearningCacheWriter trait(spec §3.1、P2-C 本実装)。

pub mod mock;
pub mod sqlite;

pub use mock::MockLearningCacheStore;
pub use sqlite::SqliteLearningCacheStore;

/// `CapOverrideGuard` is a **test-only** RAII helper that overrides the
/// learning_cache row cap during tests. It is gated behind the
/// `test-helpers` feature so that the symbol is compiled out of
/// production / release builds (spec §4.5 / §4.6). Integration test crates
/// under `crates/kotoha-storage/tests/` enable the feature via
/// `[[test]] required-features = ["test-helpers"]`.
#[cfg(any(test, feature = "test-helpers"))]
pub use sqlite::CapOverrideGuard;

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
/// - [`StorageError::InvalidField`][]: `kana_input` が validation 違反の場合
/// - [`StorageError::Sqlite`][]: SQLite backend 障害の場合
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
    /// - [`StorageError::InvalidField`][]: `kana_input` が hiragana 以外の文字を含む場合
    /// - [`StorageError::Sqlite`][]: SQLite backend 障害の場合
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
/// - [`StorageError::InvalidField`][]: validation 違反の場合
/// - [`StorageError::Sqlite`][]: SQLite backend 障害の場合
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
    /// - [`StorageError::InvalidField`][]: `kana_input` / `chosen_kanji` が validation 違反
    /// - [`StorageError::Sqlite`][]: SQLite backend 障害(disk full / SQLITE_BUSY など)
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
    /// - [`StorageError::Sqlite`][]: SQLite backend 障害の場合
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
