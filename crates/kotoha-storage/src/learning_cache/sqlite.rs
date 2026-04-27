//! SqliteLearningCacheStore: P2-C で本実装(spec §3.1 / §4.2-§4.5 / §6.3)。

#[cfg(any(test, feature = "test-helpers"))]
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
/// 本 static は **test-only** であり、`#[cfg(any(test, feature = "test-helpers"))]`
/// で gate されているため production / release binary には含まれない
/// (spec §4.5 / §4.6)。
/// integration test crate(`crates/kotoha-storage/tests/*.rs`)から利用するため、
/// crate feature `test-helpers` を有効化したときのみ `pub` として可視化する。
/// 本 feature は `--features kotoha-storage/test-helpers` を渡したテスト時のみ
/// 有効化する想定で、production binary では常に compile-out される。
///
/// 10,000 行を実際に挿入するテストは時間 / メモリの観点で非現実的なため、
/// 単体テスト / integration テストはこの override を介して小さな上限値で
/// eviction 動作を検証する。
#[cfg(any(test, feature = "test-helpers"))]
pub static LEARNING_CACHE_MAX_ROWS_TEST_OVERRIDE: AtomicUsize = AtomicUsize::new(0);

/// override の直列化用 Mutex(複数 test が同時 override しないよう直列化)。
///
/// 本 static は **test-only** であり、`#[cfg(any(test, feature = "test-helpers"))]`
/// で gate されているため production / release binary には含まれない。
/// integration test crate から利用するため、crate feature `test-helpers` を
/// 有効化したときのみ `pub` として可視化する。
#[cfg(any(test, feature = "test-helpers"))]
pub static CAP_OVERRIDE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// 行数上限の effective value を返す。
///
/// production build では本関数の本体は `LEARNING_CACHE_MAX_ROWS` を返すだけで、
/// override の参照を行わない(`LEARNING_CACHE_MAX_ROWS_TEST_OVERRIDE` 自体が
/// `#[cfg(any(test, feature = "test-helpers"))]` で compile-out されるため)。
/// test build (`cargo test` の unit test、または `--features test-helpers` を
/// 有効化した integration test) では override 値を反映する。
///
/// `pub` 公開は test build に限定する(spec §4.5 / §4.6)。production からは
/// `pub(crate)` でのみ参照可能で、外部 crate に test-only API は露出しない。
///
/// # Postconditions
///
/// - production build: 常に [`LEARNING_CACHE_MAX_ROWS`] を返す
/// - test / `test-helpers` build: override が 0 の場合は [`LEARNING_CACHE_MAX_ROWS`]、
///   それ以外の場合は override 値を返す
#[cfg(any(test, feature = "test-helpers"))]
#[inline]
pub fn effective_max_rows() -> usize {
    let v = LEARNING_CACHE_MAX_ROWS_TEST_OVERRIDE.load(Ordering::SeqCst);
    if v != 0 {
        return v;
    }
    LEARNING_CACHE_MAX_ROWS
}

/// production build 用の `effective_max_rows()`。
///
/// override 機構は `#[cfg(any(test, feature = "test-helpers"))]` で
/// compile-out されるため、production では常に [`LEARNING_CACHE_MAX_ROWS`] を返す。
/// crate-internal 利用に限定するため `pub(crate)` で公開する。
#[cfg(not(any(test, feature = "test-helpers")))]
#[inline]
pub(crate) fn effective_max_rows() -> usize {
    LEARNING_CACHE_MAX_ROWS
}

/// テスト中だけ `LEARNING_CACHE_MAX_ROWS` を `cap` に上書きする RAII guard。
///
/// drop 時に自動で 0(無効)に戻すので、test 同士の干渉を防ぐ。
/// override は process 全体の static なため、複数 test が同時に
/// override を活性化すると競合する。よって `CAP_OVERRIDE_LOCK` を
/// 取得して直列化する。
///
/// 本 struct は **test-only** であり、`#[cfg(any(test, feature = "test-helpers"))]`
/// で gate されているため production / release binary には含まれない
/// (spec §4.5 / §4.6)。integration test crate(`crates/kotoha-storage/tests/*.rs`)
/// からは `--features kotoha-storage/test-helpers` を有効化したときに限り `pub`
/// として可視化する。production code では本 struct を構築する手段がないため、
/// override は常に 0 のまま、`effective_max_rows()` は常に
/// `LEARNING_CACHE_MAX_ROWS` を返す。
///
/// # Examples
///
/// ```no_run
/// # #[cfg(any(test, feature = "test-helpers"))]
/// # {
/// use kotoha_storage::learning_cache::sqlite::CapOverrideGuard;
/// // test 内のみで使用:
/// let _guard = CapOverrideGuard::new(5);
/// // このブロック内では cap = 5 で動作する
/// // _guard が drop されると cap = LEARNING_CACHE_MAX_ROWS に戻る
/// # }
/// ```
#[cfg(any(test, feature = "test-helpers"))]
pub struct CapOverrideGuard {
    _lock: std::sync::MutexGuard<'static, ()>,
}

#[cfg(any(test, feature = "test-helpers"))]
impl CapOverrideGuard {
    /// override を `cap` に設定し、`CAP_OVERRIDE_LOCK` を取得する。
    ///
    /// drop 時に override を 0 に戻す。
    pub fn new(cap: usize) -> Self {
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
    pub fn lock_only() -> Self {
        let lock = CAP_OVERRIDE_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        // override は変更しない(0 のまま、effective_max_rows = LEARNING_CACHE_MAX_ROWS)。
        Self { _lock: lock }
    }
}

#[cfg(any(test, feature = "test-helpers"))]
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
    // perf-H1: prepare_cached により COUNT_SQL の compile を初回のみとし、
    // 以降は statement cache から再利用する。`record_choice` は Phase 3 IBus engine の
    // 打鍵毎に呼ばれるため、~1.4 µs/op の compile cost を毎打鍵分削減する。
    let mut stmt = conn.prepare_cached(COUNT_SQL)?;
    let total: i64 = stmt.query_row([], |r| r.get(0))?;
    // sec-F4: bind 方向と対称に、SQLite -> Rust 方向の変換も `as` cast ではなく
    // `try_from + clamp` を使用する。64-bit target では `i64::MAX < usize::MAX`
    // のため変換は必ず成功し、32-bit target では clamp により切り捨てを防ぐ
    // (spec §F4)。
    let total = usize::try_from(total).unwrap_or(usize::MAX);
    if total <= cap {
        return Ok(0);
    }
    let excess = total - cap;
    let mut stmt = conn.prepare_cached(EVICT_SQL)?;
    let excess_i64 = i64::try_from(excess).unwrap_or(i64::MAX);
    let deleted = stmt.execute(rusqlite::params![excess_i64])?;
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
        let limit_i64 = i64::try_from(limit).unwrap_or(i64::MAX);
        let rows = stmt.query_map(rusqlite::params![kana_input, limit_i64], |row| {
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

    // --- trace test infrastructure (sec-F4 TC-1) ---
    //
    // `Connection::trace(Some(fn(&str)))` は plain fn pointer のみ受け付ける
    // (closure 不可)ため、capture した state は static で持つ必要がある。
    // `record_trace` は callback、`TRACED_SQL` は accumulator、`TRACE_LOCK` は
    // 並列 trace test 同士の直列化 mutex。
    static TRACED_SQL: std::sync::Mutex<String> = std::sync::Mutex::new(String::new());

    /// 並列 trace test 同士の直列化 mutex。`Connection::trace`(PR #109、`record_trace`
    /// callback) と `sqlite3_trace_v2`(PR #114、`count_sql_stmt_trace` callback)の
    /// 両方が共有する。新しい trace test を追加する場合も、この同じ `TRACE_LOCK` を
    /// 取得して直列化すること(個別の lock を作らない)。
    static TRACE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn record_trace(sql: &str) {
        let mut buf = TRACED_SQL.lock().unwrap_or_else(|p| p.into_inner());
        // 改行で区切って追記する(複数 SQL の累積に対応)。
        if !buf.is_empty() {
            buf.push('\n');
        }
        buf.push_str(sql);
    }

    // --- trace_v2 (SQLITE_TRACE_STMT) infrastructure ---
    //
    // `sqlite3_trace_v2` を `SQLITE_TRACE_STMT` mask で attach すると、prepared
    // statement が **実行開始** する瞬間に callback が発火し、第 2 引数で当該
    // `sqlite3_stmt*` が渡される。同一の prepared statement instance が
    // 複数回実行されるたびに同じ pointer 値が観測される一方、毎回 new prepare
    // される場合は異なる pointer 値が観測される。
    //
    // すなわち pointer 値の `unique count` を測れば「prepared statement instance が
    // 何個 compile されたか」が機械的に判定できる:
    //
    // - `prepare_cached(SQL)` を N 回呼ぶ → unique pointer = 1
    // - `query_row(SQL, ...)` を N 回呼ぶ(毎回 prepare + finalize) → unique pointer = N
    //
    // SQL text は `sqlite3_sql(stmt)` で取得し、`COUNT_SQL` の prefix で filter する
    // (UPSERT_SQL や EVICT_SQL の compile を分離するため)。
    //
    // pointer は `usize` に cast して static `Mutex<HashSet<usize>>` に蓄える
    // (raw pointer は `Send`/`Sync` でないため)。
    //
    // NOTE: rusqlite 0.32 は `Connection::trace_v2` を safe API として export していない
    // (rusqlite/issues/977 で TODO)。将来 safe wrapper が提供されたら、本 FFI 実装は
    // closure-captured な `Arc<Mutex<...>>` の safe 版に折り畳める。
    //
    // IMPORTANT: 本 statics を使用する test は test 開頭で必ず clear する
    // (`COUNT_SQL_STMT_PTRS.lock()...clear()` と `COUNT_SQL_STMT_EVENTS.store(0, ...)`)。
    // `TRACE_LOCK` は serialization のみで auto-reset しないため、cross-test の
    // state leak を防ぐ責任は test 側にある。
    static COUNT_SQL_STMT_PTRS: std::sync::Mutex<std::collections::BTreeSet<usize>> =
        std::sync::Mutex::new(std::collections::BTreeSet::new());
    static COUNT_SQL_STMT_EVENTS: std::sync::atomic::AtomicUsize =
        std::sync::atomic::AtomicUsize::new(0);

    /// `SQLITE_TRACE_STMT` callback。`COUNT_SQL` を実行している prepared statement
    /// pointer を `COUNT_SQL_STMT_PTRS` に蓄積する。
    ///
    /// # Safety
    ///
    /// - `_p_ctx` / `p_stmt` / `_x` は SQLite が管理する有効ポインタ。
    /// - `p_stmt` は active な prepared statement で、`sqlite3_sql` の呼び出しは
    ///   safe(NUL 終端 C 文字列を返す)。
    /// - callback は `catch_unwind` で panic を抑止し、SQLite 側に unwind を
    ///   逃さない(C ABI からの unwind は UB)。
    unsafe extern "C" fn count_sql_stmt_trace(
        _event: std::os::raw::c_uint,
        _p_ctx: *mut std::os::raw::c_void,
        p_stmt: *mut std::os::raw::c_void,
        _x: *mut std::os::raw::c_void,
    ) -> std::os::raw::c_int {
        let _ = std::panic::catch_unwind(|| {
            if p_stmt.is_null() {
                return;
            }
            let stmt = p_stmt as *mut rusqlite::ffi::sqlite3_stmt;
            // SAFETY: `stmt` is non-null and SQLite guarantees `sqlite3_sql`
            // returns a static C string for the duration of the statement.
            let sql_ptr = unsafe { rusqlite::ffi::sqlite3_sql(stmt) };
            if sql_ptr.is_null() {
                return;
            }
            // SAFETY: SQLite-owned NUL-terminated C string.
            let sql = unsafe { std::ffi::CStr::from_ptr(sql_ptr) }.to_string_lossy();
            // COUNT_SQL の prefix で filter する(UPSERT / EVICT は除外)。
            // ハードコードした SQL literal ではなく定数を直接参照することで、
            // `COUNT_SQL` 定数の表記を変更しても自動的に追従する。
            if sql.contains(COUNT_SQL) {
                let mut set = COUNT_SQL_STMT_PTRS
                    .lock()
                    .unwrap_or_else(|p| p.into_inner());
                set.insert(stmt as usize);
                COUNT_SQL_STMT_EVENTS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            }
        });
        0
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

    // --- additional validation / cap-guard tests (B-followup, plan §6.2) ---

    /// `record_choice` は空文字列の chosen_kanji を reject する(`validate_surface` empty 判定)。
    ///
    /// production は空 kanji を許容してはならない(spec §6.2 / §9.1)。
    /// CapOverrideGuard::lock_only() は record_choice 系 test の race condition 回避規約。
    #[test]
    fn record_choice_rejects_empty_chosen_kanji() {
        let _lock = CapOverrideGuard::lock_only();
        let store = fresh_store_b();
        let err = store.record_choice("あい", "").unwrap_err();
        match err {
            StorageError::InvalidField { name, reason } => {
                assert_eq!(name, "surface");
                assert_eq!(reason, "empty");
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    /// `record_choice` は PUA(U+E000)を含む chosen_kanji を reject する。
    ///
    /// `validate_surface` -> `validate_field` の `is_disallowed_pua` で reject される
    /// (Phase 5 allowlist U+EE00..=U+EE03 を除く)。
    #[test]
    fn record_choice_rejects_pua_in_chosen_kanji() {
        let _lock = CapOverrideGuard::lock_only();
        let store = fresh_store_b();
        let err = store.record_choice("あい", "\u{E000}").unwrap_err();
        match err {
            StorageError::InvalidField { name, reason } => {
                assert_eq!(name, "surface");
                assert_eq!(reason, "PUA char");
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    /// `record_choice` は 256 byte 上限を超える kana_input を reject する。
    ///
    /// "あ" = 3 byte UTF-8 なので 86 文字で 258 byte となり `validate_field` の
    /// `value.len() > 256` 判定で reject される(spec §9.1 / `validate_reading`)。
    #[test]
    fn record_choice_rejects_oversize_kana_input() {
        let _lock = CapOverrideGuard::lock_only();
        let store = fresh_store_b();
        let oversize = "あ".repeat(86); // 86 * 3 = 258 byte > 256
        assert!(oversize.len() > 256, "test fixture must exceed 256 byte");
        let err = store.record_choice(&oversize, "愛").unwrap_err();
        match err {
            StorageError::InvalidField { name, reason } => {
                assert_eq!(name, "reading");
                assert_eq!(reason, "byte size > 256");
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    // --- TC-1 / TC-3 trace-based regression tests (sec-F4) ---
    //
    // rusqlite 0.32 の `Connection::trace(Some(fn(&str)))` は **plain fn pointer**
    // のみを受け付ける(closure 不可)ため、capture したい state は
    // `static Mutex<String>` に置く必要がある。複数 trace test が同一 static を
    // 共有するため、`TRACE_LOCK` で直列化する。
    //
    // trace callback が観測する SQL は parameter 展開後 (`LIMIT -1` のように
    // 数値が literal として埋め込まれる) なので、bind 値が `i64::MAX` ではなく
    // `-1` になっている旧 buggy code を直接検出できる。
    //
    // ## TC-1 (FAIL on old `usize as i64` code)
    //
    // `lookup` の `limit as i64` は `usize::MAX` で `-1` に wrap する。
    // SQLite の `LIMIT -1` は "unlimited" 扱い(silent bypass)。
    // 新 code (`i64::try_from(limit).unwrap_or(i64::MAX)`) では `LIMIT 9223372036854775807`
    // が trace に現れる。
    /// `lookup` は `usize::MAX` を `limit` に渡しても trace 上に `LIMIT -1` を
    /// 書き出さない(sec-F4)。
    ///
    /// 旧実装の `limit as i64` は `usize::MAX` を `-1` に wrap し、SQLite は
    /// `LIMIT -1` を unlimited として解釈するため、API 表面の row count では
    /// 旧 code と新 code を区別できない。本 test は SQLite の trace callback で
    /// expanded SQL を捕捉し、bound LIMIT が `-1` でないことを直接検証する。
    #[test]
    fn lookup_passes_non_negative_limit_to_sqlite() {
        let _serialize = TRACE_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        let _cap_lock = CapOverrideGuard::lock_only();
        let store = fresh_store_b();
        store.record_choice("あい", "愛").expect("ok");
        store.record_choice("あい", "哀").expect("ok");
        store.record_choice("あい", "藍").expect("ok");

        // trace 取得前に既存 buffer をクリアする(他 test の leftover 防止)。
        TRACED_SQL.lock().unwrap().clear();

        // trace を attach -> lookup 実行 -> trace を detach。
        {
            let mut conn = store.db.lock_conn();
            conn.trace(Some(record_trace));
        }
        let _ = store.lookup("あい", usize::MAX).expect("ok");
        {
            let mut conn = store.db.lock_conn();
            conn.trace(None);
        }

        let traced = TRACED_SQL.lock().unwrap().clone();
        // bound LIMIT が `-1` として現れていないことを assert する。
        // 新 code (`i64::try_from(limit).unwrap_or(i64::MAX)`) では `LIMIT 9223372036854775807`、
        // 旧 code (`limit as i64`) では `LIMIT -1` が観測される。
        assert!(
            !traced.contains("LIMIT -1"),
            "SQLite trace must not contain bound LIMIT of -1 (old `usize as i64` regression); got: {traced}"
        );
        // sanity: trace に LOOKUP_SQL の prefix が記録されている(callback が動作している証拠)。
        assert!(
            traced.contains("SELECT id, kana_input"),
            "trace must capture LOOKUP_SQL; got: {traced}"
        );
    }

    /// `sqlite3_trace_v2` を attach した状態で `body` を実行し、終了時 / panic 時の
    /// どちらでも必ず detach する panic-safe な closure helper。
    ///
    /// # Why a closure instead of a Drop-based RAII guard
    ///
    /// `Mutex<Connection>` の都合で、attach / detach は短い lock scope で行い、
    /// `body` 内の `record_choice` は別 lock scope で `db.lock_conn()` を取得する
    /// 必要がある(`MutexGuard<Connection>` を `body` 全体で保持すると
    /// `record_choice` 内部の `lock_conn()` が deadlock する)。
    /// SQLite 側の trace callback は connection 単位で保存されるため、attach 後に
    /// MutexGuard を解放しても `body` の `record_choice` 呼び出しでは callback が
    /// 引き続き発火する。`Drop` ベースの guard は `&'a Connection` を抱え込んで
    /// MutexGuard の lifetime と結合してしまうため、本 helper では closure 形で
    /// 「attach -> `catch_unwind(body)` -> detach -> `resume_unwind`」の順で
    /// 強制 detach を担保する設計を採る。
    ///
    /// # Safety contract
    ///
    /// - `count_sql_stmt_trace` は `unsafe extern "C"` で、内部で `catch_unwind`
    ///   により panic を抑止しているため、C ABI 越しの unwind は起きない。
    /// - attach と detach は同じ connection の `sqlite3*` 上で対称に呼ばれる。
    fn with_trace<F: FnOnce() + std::panic::UnwindSafe>(store: &SqliteLearningCacheStore, body: F) {
        // attach
        {
            let conn = store.db.lock_conn();
            // SAFETY: `handle()` is valid for the lifetime of `conn`. We attach
            // here while holding the MutexGuard and detach later via the same
            // path; SQLite stores the callback per-connection, so the callback
            // remains active even after we release the MutexGuard.
            let rc = unsafe {
                rusqlite::ffi::sqlite3_trace_v2(
                    conn.handle(),
                    rusqlite::ffi::SQLITE_TRACE_STMT as std::os::raw::c_uint,
                    Some(count_sql_stmt_trace),
                    std::ptr::null_mut(),
                )
            };
            assert_eq!(
                rc,
                rusqlite::ffi::SQLITE_OK,
                "sqlite3_trace_v2 attach must succeed (rc={rc})"
            );
        }
        // body を catch_unwind で実行し、panic でも detach に到達させる。
        let result = std::panic::catch_unwind(body);
        // detach (always runs)
        {
            let conn = store.db.lock_conn();
            // SAFETY: same Connection; passing None removes the callback.
            unsafe {
                rusqlite::ffi::sqlite3_trace_v2(
                    conn.handle(),
                    rusqlite::ffi::SQLITE_TRACE_STMT as std::os::raw::c_uint,
                    None,
                    std::ptr::null_mut(),
                );
            }
        }
        if let Err(panic) = result {
            std::panic::resume_unwind(panic);
        }
    }

    /// `evict_to_cap` の `COUNT_SQL` が `prepare_cached` 経由で statement cache から
    /// 再利用され、`record_choice` を 4 連続呼び出ししても **prepared statement instance
    /// は 1 個** しか作られないことを実測検証する(perf-H1)。
    ///
    /// # Background
    ///
    /// 旧実装の `evict_to_cap` は `conn.query_row(COUNT_SQL, [], ...)` を呼び、
    /// 内部で prepare → step → finalize を毎回繰り返していた。Phase 3 IBus engine では
    /// `record_choice` が打鍵毎に呼ばれるため、~1.4 µs/op の compile cost が打鍵数に
    /// 比例して累積する(`record_choice` latency の約 11%)。
    /// 新実装は `prepare_cached(COUNT_SQL)` により初回のみ compile し、以降は
    /// statement cache から同一 `sqlite3_stmt*` を再利用する。
    ///
    /// # Detection mechanism
    ///
    /// SQLite の `sqlite3_trace_v2(db, SQLITE_TRACE_STMT, cb, ctx)` は、prepared
    /// statement が **実行開始** する瞬間に第 3 引数で当該 `sqlite3_stmt*` を渡す。
    /// `prepare_cached` で再利用された場合は同一 pointer 値が観測され、毎回 prepare
    /// される場合は異なる pointer 値が観測される。callback で `sqlite3_sql(stmt)` を
    /// 呼んで SQL text を取得し、`COUNT_SQL` で filter した上で **unique pointer count** を
    /// 数えれば「何個の prepared statement instance が compile されたか」が分かる。
    ///
    /// # Assertions
    ///
    /// - 4 回の `record_choice` 呼び出しで `COUNT_SQL` の trace event が **4 回以上** 発火する
    ///   (実行回数の sanity check、現状は厳密に 4 回)
    /// - `COUNT_SQL` を実行している prepared statement の **unique pointer count = 1**
    ///   (prepare_cached が cache hit している証拠)
    ///
    /// 旧 `query_row` 実装ではこの test は unique pointer count = 4 となり FAIL する。
    #[test]
    fn evict_to_cap_uses_prepare_cached_for_count_sql() {
        let _serialize = TRACE_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        let _cap_lock = CapOverrideGuard::lock_only();
        let store = fresh_store_b();

        // capture 用 static を初期化する(他 test の leftover 防止)。
        COUNT_SQL_STMT_PTRS
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clear();
        COUNT_SQL_STMT_EVENTS.store(0, std::sync::atomic::Ordering::SeqCst);

        // 4 回 record_choice を呼ぶ。各 call の `evict_to_cap` で COUNT_SQL が走る。
        // `with_trace` は attach / detach を panic-safe に巻き取る(panic でも detach)。
        with_trace(&store, || {
            for (reading, kanji) in [("あ", "亜"), ("い", "以"), ("う", "宇"), ("え", "江")]
            {
                store.record_choice(reading, kanji).expect("record ok");
            }
        });

        let unique_ptr_count = COUNT_SQL_STMT_PTRS
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .len();
        let event_count = COUNT_SQL_STMT_EVENTS.load(std::sync::atomic::Ordering::SeqCst);

        // sanity: COUNT_SQL は record_choice 毎に少なくとも 1 回実行される(現状は 4 回)。
        // 厳密に 4 を要求すると将来 evict_to_cap が追加で COUNT_SQL を呼ぶ refactor で
        // false-FAIL するため、`>= 4` で plumbing の正常性を確認するに留める。
        assert!(
            event_count >= 4,
            "COUNT_SQL trace fired fewer times than record_choice calls (got {event_count}, unique_ptrs={unique_ptr_count}); plumbing is broken"
        );

        // perf-H1 assertion: prepare_cached により COUNT_SQL の prepared statement
        // は 1 個しか作られない。旧 `query_row` 実装では unique_ptr_count = 4 となる。
        assert_eq!(
            unique_ptr_count, 1,
            "evict_to_cap must reuse a single cached prepared statement for COUNT_SQL across multiple record_choice calls (got {unique_ptr_count} distinct sqlite3_stmt* over {event_count} executions); old `conn.query_row(COUNT_SQL, ...)` regression would yield {event_count} distinct pointers"
        );
    }

    /// `lookup` はひらがな以外の kana_input を reject する。
    ///
    /// `validate_reading` の `is_hiragana_or_long_sound` 判定で
    /// カタカナは reject される(reason = "non-hiragana reading")。
    #[test]
    fn lookup_rejects_non_hiragana() {
        let store = fresh_store_b();
        let err = store.lookup("カタカナ", 10).unwrap_err();
        match err {
            StorageError::InvalidField { name, reason } => {
                assert_eq!(name, "reading");
                assert_eq!(reason, "non-hiragana reading");
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    /// `CapOverrideGuard` は drop 時に override を 0 に戻し、
    /// `effective_max_rows()` を `LEARNING_CACHE_MAX_ROWS` (10_000) に復元する。
    ///
    /// guard scope を限定するために inner block で `_g` を作成し、
    /// block 終了時の暗黙 drop で復元することを観測する。
    /// `CAP_OVERRIDE_LOCK` の reentrant 不可制約を遵守し、内側 guard を drop
    /// してから次の guard を作成する形にしている。
    #[test]
    fn cap_override_guard_restores_default_on_drop() {
        // 1) 初期状態(他 test が override を残していない前提を `lock_only()` で直列化確認)
        let outer_lock = CapOverrideGuard::lock_only();
        assert_eq!(effective_max_rows(), LEARNING_CACHE_MAX_ROWS);
        drop(outer_lock);

        // 2) inner scope で override = 5 を活性化
        {
            let _g = CapOverrideGuard::new(5);
            assert_eq!(effective_max_rows(), 5);
            // _g は block 終了時に drop され、override は 0 に reset される
        }

        // 3) drop 後は再び default 値に復元されている
        let outer_lock_after = CapOverrideGuard::lock_only();
        assert_eq!(effective_max_rows(), LEARNING_CACHE_MAX_ROWS);
        drop(outer_lock_after);
    }

    /// `effective_max_rows()` は `CapOverrideGuard::new(N)` の scope 内で N を返す。
    ///
    /// API contract test: `LEARNING_CACHE_MAX_ROWS_TEST_OVERRIDE.store()` を
    /// 直接触らず、`CapOverrideGuard` 経由で override が反映されることを確認する。
    #[test]
    fn effective_max_rows_returns_override_in_test() {
        let _g = CapOverrideGuard::new(42);
        assert_eq!(effective_max_rows(), 42);
    }

    /// `effective_max_rows()` は override が 0(unset)の場合 `LEARNING_CACHE_MAX_ROWS` を返す。
    ///
    /// `CAP_OVERRIDE_LOCK` を取得して並列 test の override 干渉を排除した上で、
    /// 明示的に store(0) してから default 復元を確認する。
    #[test]
    fn effective_max_rows_returns_default_when_override_zero() {
        let _lock = CapOverrideGuard::lock_only();
        // 念のため明示的に override を 0(unset)へ。
        LEARNING_CACHE_MAX_ROWS_TEST_OVERRIDE.store(0, Ordering::SeqCst);
        assert_eq!(effective_max_rows(), LEARNING_CACHE_MAX_ROWS);
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
