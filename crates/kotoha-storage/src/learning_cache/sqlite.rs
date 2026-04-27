//! SqliteLearningCacheStore: P2-C で本実装(spec §3.1 / §4.2-§4.5 / §6.3)。

use std::sync::Arc;

use crate::database::Database;
use crate::error::StorageError;
use crate::learning_cache::{LearningCacheReader, LearningCacheRecord, LearningCacheWriter};
use crate::validation::{validate_reading, validate_surface};

/// `lookup` SQL(spec §4.3)。`(kana_input)` index を seek して
/// `frequency DESC, last_used_at DESC` 順に `LIMIT ?2` 件を返す。
const LOOKUP_SQL: &str = "SELECT id, kana_input, chosen_kanji, frequency, last_used_at
     FROM learning_cache
     WHERE kana_input = ?1
     ORDER BY frequency DESC, last_used_at DESC
     LIMIT ?2";

/// `record_choice` の UPSERT SQL(spec §4.2、SQLite 3.24+ ON CONFLICT UPSERT 構文)。
const UPSERT_SQL: &str =
    "INSERT INTO learning_cache (kana_input, chosen_kanji, frequency, last_used_at)
     VALUES (?1, ?2, 1, ?3)
     ON CONFLICT(kana_input, chosen_kanji)
     DO UPDATE SET
         frequency     = frequency + 1,
         last_used_at  = excluded.last_used_at";

pub struct SqliteLearningCacheStore {
    pub(crate) db: Arc<Database>,
}

impl SqliteLearningCacheStore {
    pub fn new(db: Arc<Database>) -> Self {
        Self { db }
    }
}

/// `SqliteLearningCacheStore` の B3 段階の動作:
///
/// - `lookup`: 本実装(`LOOKUP_SQL` + `prepare_cached`)
/// - `record_choice`: 常に Ok(())(B5 で本実装)
/// - `evict_lru`: 常に Ok(0)(B10 で本実装)
///
/// table `learning_cache` schema は v001 migration で同梱済(spec §5.2)。
impl LearningCacheReader for SqliteLearningCacheStore {
    fn lookup(
        &self,
        kana_input: &str,
        limit: usize,
    ) -> Result<Vec<LearningCacheRecord>, StorageError> {
        validate_reading(kana_input)?;
        let conn = self.db.lock_conn();
        // perf-H1: prepare_cached により Phase 3 IBus engine の打鍵毎呼び出しでも
        // SQL コンパイルを 1 度きりにする(同一 SQL ⇒ cache hit)。
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
        // B10 で本実装する。
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
        store.record_choice("あい", "愛").expect("noop ok");
    }

    #[test]
    fn p2b_stub_evict_lru_returns_zero() {
        let db = Database::open_in_memory().expect("memory open");
        let store = SqliteLearningCacheStore::new(db);
        let evicted = store.evict_lru(100).expect("noop ok");
        assert_eq!(evicted, 0);
    }

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
}
