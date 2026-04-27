//! SqliteLearningCacheStore: P2-C で本実装(spec §3.1 / §4.2-§4.5 / §6.3)。

#[cfg(test)]
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use crate::database::Database;
use crate::error::StorageError;
use crate::learning_cache::{LearningCacheReader, LearningCacheRecord, LearningCacheWriter};
use crate::validation::{validate_reading, validate_surface};

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
#[allow(dead_code)] // B7 で test、B8 で record_choice 経由使用開始
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
/// ```ignore
/// // tests-only:
/// let _guard = CapOverrideGuard::new(5);
/// // このブロック内では cap = 5 で動作する
/// // _guard が drop されると cap = LEARNING_CACHE_MAX_ROWS に戻る
/// ```
#[cfg(test)]
#[allow(dead_code)] // B7 で eviction tests から使用開始
pub(crate) struct CapOverrideGuard {
    _lock: std::sync::MutexGuard<'static, ()>,
}

#[cfg(test)]
impl CapOverrideGuard {
    #[allow(dead_code)] // B7 で eviction tests から呼び出し開始
    pub(crate) fn new(cap: usize) -> Self {
        // poison していても続行(直前 test の panic でも次 test を回したい)。
        let lock = CAP_OVERRIDE_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        LEARNING_CACHE_MAX_ROWS_TEST_OVERRIDE.store(cap, Ordering::SeqCst);
        Self { _lock: lock }
    }

    /// override を変更せずに `CAP_OVERRIDE_LOCK` のみ取得する。
    ///
    /// `record_choice` を呼ぶ test が、override を必要としない一方で、
    /// 並列実行されている他 test の override(`CapOverrideGuard::new(N)`)が
    /// 残っている瞬間に `record_choice` の自動 eviction が小さな cap で走るのを
    /// 避けるための serialization 用 guard。
    #[allow(dead_code)] // record_choice 系 test の race 回避で使用
    pub(crate) fn lock_only() -> Self {
        let lock = CAP_OVERRIDE_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        // override は変更しない(0 のまま、effective_max_rows = LEARNING_CACHE_MAX_ROWS)。
        Self { _lock: lock }
    }
}

#[cfg(test)]
impl Drop for CapOverrideGuard {
    fn drop(&mut self) {
        LEARNING_CACHE_MAX_ROWS_TEST_OVERRIDE.store(0, Ordering::SeqCst);
    }
}

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

/// 総行数チェック SQL(spec §4.4)。
const COUNT_SQL: &str = "SELECT count(*) FROM learning_cache";

/// LRU 削除 SQL(spec §4.4)。`last_used_at ASC, id ASC` 順で `LIMIT ?1` 件削除する。
const EVICT_SQL: &str = "DELETE FROM learning_cache
     WHERE id IN (
         SELECT id FROM learning_cache
         ORDER BY last_used_at ASC, id ASC
         LIMIT ?1
     )";

/// `learning_cache` の行数が `cap` を超えている場合、LRU から `count - cap` 行削除する。
///
/// `record_choice` の自動 eviction(`effective_max_rows()` を cap に渡す)と
/// `evict_lru(max_entries)` の明示 eviction の両方から呼ばれる共通 helper。
///
/// # Preconditions
///
/// - `conn` は `Database::lock_conn()` で取得済の `MutexGuard<Connection>`(または
///   その deref を経由した `&Connection`)
/// - `cap` は呼び出し元が指定する行数上限値
///
/// # Postconditions
///
/// - 戻り値は削除した行数
/// - 行数 ≤ cap が保証される
///
/// # Errors
///
/// - [`StorageError::Sqlite`][]: SQLite backend 障害の場合
pub(crate) fn evict_to_cap(conn: &rusqlite::Connection, cap: usize) -> Result<usize, StorageError> {
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

pub struct SqliteLearningCacheStore {
    pub(crate) db: Arc<Database>,
}

impl SqliteLearningCacheStore {
    pub fn new(db: Arc<Database>) -> Self {
        Self { db }
    }
}

/// `SqliteLearningCacheStore` の本実装(P2-C-B 完了時点):
///
/// - `lookup`: `LOOKUP_SQL` + `prepare_cached`、`frequency DESC, last_used_at DESC` 順で `LIMIT ?2` 件
/// - `record_choice`: `UPSERT_SQL` で UPSERT、その後 `evict_to_cap(effective_max_rows())` で自動 eviction
/// - `evict_lru`: `evict_to_cap(max_entries)` を直接呼び出して LRU から超過分を削除
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
        {
            let mut stmt = conn.prepare_cached(UPSERT_SQL)?;
            stmt.execute(rusqlite::params![kana_input, chosen_kanji, now])?;
        }
        // 行数が effective_max_rows() を超えた場合、LRU entry を自動削除する(spec §4.2)。
        evict_to_cap(&conn, effective_max_rows())?;
        Ok(())
    }

    fn evict_lru(&self, max_entries: usize) -> Result<usize, StorageError> {
        let conn = self.db.lock_conn();
        evict_to_cap(&conn, max_entries)
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
        let _lock = CapOverrideGuard::lock_only();
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
        let _lock = CapOverrideGuard::lock_only();
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
        let _lock = CapOverrideGuard::lock_only();
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
        let _lock = CapOverrideGuard::lock_only();
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
        let _lock = CapOverrideGuard::lock_only();
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
        let _lock = CapOverrideGuard::lock_only();
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
        let _lock = CapOverrideGuard::lock_only();
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
        let _lock = CapOverrideGuard::lock_only();
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
        let _lock = CapOverrideGuard::lock_only();
        let store = fresh_store_b();
        let err = store.record_choice("abc", "川").unwrap_err();
        assert!(matches!(err, StorageError::InvalidField { .. }));
    }

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
    ///
    /// last_used_at は秒精度の `SystemTime::now()` のため、同一秒内に複数 record_choice
    /// が走ると tie となり id ASC で評価されてしまう。本 test はそれを避けるため
    /// 各 step 間で `std::thread::sleep(Duration::from_secs(1))` を挟み、
    /// 秒境界を確実に跨ぐようにしている。production code 側の精度問題ではなく
    /// test 構成の精度問題なので、test 側で吸収する。
    /// (B7) TDD red: B5 実装は eviction を含まないので挙動を検証できず行数超過で FAIL する。
    #[test]
    fn eviction_protects_recently_updated_entry_from_lru() {
        use std::thread::sleep;
        use std::time::Duration;
        let _guard = CapOverrideGuard::new(2);
        let store = fresh_store_b();
        // "亜" を先に insert する(古い entry、last_used_at = T0)。
        store.record_choice("あ", "亜").expect("ok");
        sleep(Duration::from_secs(1));
        // "以" を後で insert する(中間 entry、last_used_at = T0 + 1)。
        store.record_choice("い", "以").expect("ok");
        sleep(Duration::from_secs(1));
        // "亜" を再度 record_choice して last_used_at を T0 + 2 に更新する。
        store.record_choice("あ", "亜").expect("ok");
        sleep(Duration::from_secs(1));
        // "う" を insert(last_used_at = T0 + 3)。これで cap=2 を超える。
        // "以" の last_used_at が最も古いので evict される。
        store.record_choice("う", "宇").expect("ok");
        // "亜" は残っており、"以" は evict されている。
        let result_a = store.lookup("あ", 10).expect("ok");
        assert!(
            !result_a.is_empty(),
            "亜 must survive because it was recently updated"
        );
        let result_i = store.lookup("い", 10).expect("ok");
        assert!(result_i.is_empty(), "以 must be evicted as the LRU entry");
    }

    // --- evict_lru tests (B9 red phase) ---

    /// `evict_lru` は行数が max_entries 以下の場合 0 を返す。
    ///
    /// NOTE: `record_choice` を使うと並列に実行される `eviction_*` test が
    /// `CapOverrideGuard` で設定した小さい cap で自動 eviction を発火させる
    /// 可能性がある(`CAP_OVERRIDE_LOCK` は guard 所有者しか直列化しない)。
    /// 本 test は race を避けるために SQL を直接 INSERT する。
    /// (B9) TDD red: stub は Ok(0) を返すので本 test は PASS する。B10 後も PASS を維持する。
    #[test]
    fn evict_lru_returns_zero_when_below_cap() {
        let store = fresh_store_b();
        {
            let conn = store.db.lock_conn();
            conn.execute(
                "INSERT INTO learning_cache (kana_input, chosen_kanji, frequency, last_used_at)
                 VALUES ('あ', '亜', 1, 100)",
                [],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO learning_cache (kana_input, chosen_kanji, frequency, last_used_at)
                 VALUES ('い', '以', 1, 200)",
                [],
            )
            .unwrap();
        }
        // 行数 2 < max_entries=5 なので 0 が返る。
        let deleted = store.evict_lru(5).expect("ok");
        assert_eq!(deleted, 0);
    }

    /// `evict_lru` は超過分の行数を削除して削除件数を返す。
    ///
    /// NOTE: `record_choice` の自動 eviction と並列 test の `CapOverrideGuard`
    /// が干渉するので、本 test も SQL 直 INSERT で 5 件を準備する。
    /// (B9) TDD red: stub は Ok(0) を返すので行数が変化せず FAIL する。
    #[test]
    fn evict_lru_deletes_excess_entries_and_returns_count() {
        let store = fresh_store_b();
        {
            let conn = store.db.lock_conn();
            // last_used_at を昇順に振って LRU 順序を明示する(同値 tie を回避)。
            for (i, (kana, kanji)) in [
                ("あ", "亜"),
                ("い", "以"),
                ("う", "宇"),
                ("え", "江"),
                ("お", "尾"),
            ]
            .iter()
            .enumerate()
            {
                conn.execute(
                    "INSERT INTO learning_cache (kana_input, chosen_kanji, frequency, last_used_at)
                     VALUES (?1, ?2, 1, ?3)",
                    rusqlite::params![kana, kanji, (100 + i) as i64],
                )
                .unwrap();
            }
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
}
