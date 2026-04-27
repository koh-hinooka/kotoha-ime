//! `MockLearningCacheStore`(in-memory、test 用、spec §3.1 / arch-M-2 ISP split)。
//!
//! Phase 3 IBus engine / P2-D Ranker の test injection 用に、
//! `SqliteLearningCacheStore` と振る舞いが対称な in-memory 実装を提供する
//! (spec §3.1 / arch-M-2)。
//!
//! # 振る舞いの対称性(spec §3.1 / `sqlite.rs` を mirror)
//!
//! - `lookup`: `frequency DESC, last_used_at DESC` 順で `limit` 件以下を返す
//! - `record_choice`: `(kana_input, chosen_kanji)` ペアで UPSERT、duplicate 時は
//!   `frequency += 1` かつ `last_used_at` 更新、その後 instance ごとの `max_rows` で
//!   自動 eviction を発火する
//! - `evict_lru`: `last_used_at` 昇順 + `id` 昇順で行を削除して行数を `max_entries` 以内に収める
//!
//! # Thread safety
//!
//! `Mutex` で内部 `Vec<LearningCacheRecord>` を保護する。
//! poison 発生時は `PoisonError::into_inner` で recover する(SQLite-side の
//! `Mutex<Connection>` poison 復旧と同じ方針:Mock の内部 `Vec` には Rust レベルの
//! invariant が無く、poison から守るべき不変条件が無いため)。
//!
//! # Cap configuration
//!
//! `MockLearningCacheStore::new()` は production 値
//! [`crate::learning_cache::sqlite::LEARNING_CACHE_MAX_ROWS`] (= 10_000) を cap に
//! 設定する。P2-D Ranker / Phase 3 IBus engine の test injection では
//! [`MockLearningCacheStore::with_max_rows`] で小さい cap を直接指定し、
//! auto-eviction の境界を制御できる(Mock は SQLite 側の cap override 機構から
//! 完全に独立しているため、test 用の global guard を経由する必要は無い)。
//!
//! # `SqliteLearningCacheStore` との既知の振る舞いの差異
//!
//! `record_choice` は existing entry の frequency 更新に `saturating_add(1)` を
//! 用いる。SQLite 側は `frequency = frequency + 1` を SQL で実行し、`u32::MAX` を
//! 超えた値は SQLite が i64 で保持するが、後続の `lookup` で `row.get::<_, u32>(...)`
//! が `IntegralValueOutOfRange` で失敗し `StorageError::Sqlite` を返す。
//! Mock 側は silently `u32::MAX` で clamp する。
//!
//! 実用上、同一 `(kana_input, chosen_kanji)` ペアで 2^32 - 1 ≈ 4.3e9 回の
//! `record_choice` 呼び出しが必要なため、本差異は理論的なものに留まる。

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, PoisonError};

use crate::error::StorageError;
use crate::learning_cache::sqlite::LEARNING_CACHE_MAX_ROWS;
use crate::learning_cache::{LearningCacheReader, LearningCacheRecord, LearningCacheWriter};
use crate::validation::{validate_reading, validate_surface};

/// `LearningCacheReader` + `LearningCacheWriter` の in-memory 実装。
///
/// # Invariants
///
/// - `records` 内の各 `LearningCacheRecord` は `(kana_input, chosen_kanji)` ペアで unique
/// - `next_id` は単調増加(`record_choice` で新規 insert 毎に +1)
/// - 行数は `record_choice` 完了時点で常に `max_rows` 以下
pub struct MockLearningCacheStore {
    records: Mutex<Vec<LearningCacheRecord>>,
    next_id: Mutex<i64>,
    max_rows: AtomicUsize,
}

impl Default for MockLearningCacheStore {
    fn default() -> Self {
        Self::new()
    }
}

impl MockLearningCacheStore {
    /// 空の `MockLearningCacheStore` を生成する(production cap)。
    ///
    /// # Postconditions
    ///
    /// - 戻り値の `records` は空 `Vec`
    /// - 戻り値の `next_id` は 1
    /// - 戻り値の `max_rows` は [`LEARNING_CACHE_MAX_ROWS`] (= 10_000)
    pub fn new() -> Self {
        Self::with_max_rows(LEARNING_CACHE_MAX_ROWS)
    }

    /// 指定 `cap` で空の `MockLearningCacheStore` を生成する。
    ///
    /// P2-D Ranker / Phase 3 IBus engine の test injection で、小さい cap を
    /// 設定して auto-eviction の境界を再現するために使用する。
    /// Mock は SQLite-side の cap override 機構から完全に独立しているため、
    /// 本コンストラクタを使えば SQLite 側の test 用 global override に影響されずに
    /// per-instance で cap を制御できる。
    ///
    /// # Preconditions
    ///
    /// - `cap >= 1`(0 を渡すと `record_choice` の自動 eviction が即時発火し
    ///   全 entry が drop されるため、実用上意味が無い)
    ///
    /// # Postconditions
    ///
    /// - 戻り値の `records` は空 `Vec`
    /// - 戻り値の `next_id` は 1
    /// - 戻り値の `max_rows` は引数 `cap`
    pub fn with_max_rows(cap: usize) -> Self {
        Self {
            records: Mutex::new(Vec::new()),
            next_id: Mutex::new(1),
            max_rows: AtomicUsize::new(cap),
        }
    }

    /// 現在時刻を UNIX epoch seconds (i64) で取得する内部 helper。
    ///
    /// `SqliteLearningCacheStore::record_choice` と同じ semantics
    /// (秒精度、SystemTime epoch 取得失敗時は 0、i64 overflow 時は `i64::MAX`)。
    fn now_epoch_secs() -> i64 {
        i64::try_from(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
        )
        .unwrap_or(i64::MAX)
    }
}

impl LearningCacheReader for MockLearningCacheStore {
    /// `kana_input` に対応する entry を `frequency DESC, last_used_at DESC` 順で返す。
    ///
    /// trait `LearningCacheReader::lookup` の対称実装。詳細仕様は trait 側の
    /// doc comment を参照。
    fn lookup(
        &self,
        kana_input: &str,
        limit: usize,
    ) -> Result<Vec<LearningCacheRecord>, StorageError> {
        validate_reading(kana_input)?;
        let records = self.records.lock().unwrap_or_else(PoisonError::into_inner);
        let mut filtered: Vec<LearningCacheRecord> = records
            .iter()
            .filter(|r| r.kana_input == kana_input)
            .cloned()
            .collect();
        // sort by frequency DESC, last_used_at DESC(SQLite ORDER BY と対称)。
        filtered.sort_by(|a, b| {
            b.frequency
                .cmp(&a.frequency)
                .then_with(|| b.last_used_at.cmp(&a.last_used_at))
        });
        filtered.truncate(limit);
        Ok(filtered)
    }
}

impl LearningCacheWriter for MockLearningCacheStore {
    /// `(kana_input, chosen_kanji)` ペアで UPSERT する。
    ///
    /// trait `LearningCacheWriter::record_choice` の対称実装。詳細仕様は trait 側の
    /// doc comment を参照。
    ///
    /// # Implementation notes
    ///
    /// - 既存 entry: `frequency = saturating_add(1)`、`last_used_at = now_epoch_secs()`
    /// - 新規 entry: `frequency = 1`、`id = next_id`、`last_used_at = now_epoch_secs()`
    /// - UPSERT 後に `self.max_rows` を超えた場合、`last_used_at ASC, id ASC` 順で
    ///   超過分を drop する(SQLite `evict_to_cap` の semantics と対称)
    fn record_choice(&self, kana_input: &str, chosen_kanji: &str) -> Result<(), StorageError> {
        validate_reading(kana_input)?;
        validate_surface(chosen_kanji)?;
        let now = Self::now_epoch_secs();
        let mut records = self.records.lock().unwrap_or_else(PoisonError::into_inner);
        // UPSERT semantics: existing entry → frequency += 1 + last_used_at = now;
        // else insert new with frequency = 1.
        if let Some(existing) = records
            .iter_mut()
            .find(|r| r.kana_input == kana_input && r.chosen_kanji == chosen_kanji)
        {
            existing.frequency = existing.frequency.saturating_add(1);
            existing.last_used_at = now;
        } else {
            let mut next_id = self.next_id.lock().unwrap_or_else(PoisonError::into_inner);
            let id = *next_id;
            *next_id += 1;
            records.push(LearningCacheRecord {
                id,
                kana_input: kana_input.to_string(),
                chosen_kanji: chosen_kanji.to_string(),
                frequency: 1,
                last_used_at: now,
            });
        }
        // Auto-eviction: drop the entries with the oldest last_used_at until row count ≤ cap.
        // SQLite EVICT_SQL の `ORDER BY last_used_at ASC, id ASC` と対称な tie-break を
        // 保つため、id ASC でも sort する。
        let cap = self.max_rows.load(Ordering::Relaxed);
        if records.len() > cap {
            records.sort_by(|a, b| {
                a.last_used_at
                    .cmp(&b.last_used_at)
                    .then_with(|| a.id.cmp(&b.id))
            });
            let excess = records.len() - cap;
            records.drain(0..excess);
        }
        Ok(())
    }

    /// LRU(`last_used_at` 最古)entry を `max_entries` 行に収まるまで削除する。
    ///
    /// trait `LearningCacheWriter::evict_lru` の対称実装。詳細仕様は trait 側の
    /// doc comment を参照。
    ///
    /// # Implementation notes
    ///
    /// - 行数 ≤ `max_entries`: 削除を行わず `Ok(0)` を返す
    /// - 行数 > `max_entries`: `last_used_at ASC, id ASC` で sort し先頭から
    ///   `len - max_entries` 行を drop する(SQLite `evict_to_cap` と対称)
    fn evict_lru(&self, max_entries: usize) -> Result<usize, StorageError> {
        let mut records = self.records.lock().unwrap_or_else(PoisonError::into_inner);
        if records.len() <= max_entries {
            return Ok(0);
        }
        records.sort_by(|a, b| {
            a.last_used_at
                .cmp(&b.last_used_at)
                .then_with(|| a.id.cmp(&b.id))
        });
        let excess = records.len() - max_entries;
        records.drain(0..excess);
        Ok(excess)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 1 件 record_choice 後に lookup で取得できる(round-trip property)。
    #[test]
    fn mock_record_choice_then_lookup() {
        let store = MockLearningCacheStore::new();
        store.record_choice("あい", "愛").expect("record ok");
        let result = store.lookup("あい", 10).expect("lookup ok");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].chosen_kanji, "愛");
        assert_eq!(result[0].frequency, 1);
    }

    /// duplicate `(kana, kanji)` で frequency が +1 され、
    /// last_used_at が更新される(SQLite UPSERT 対称)。
    #[test]
    fn mock_record_choice_increments_frequency_on_duplicate() {
        let store = MockLearningCacheStore::new();
        store.record_choice("かわ", "川").expect("1st ok");
        store.record_choice("かわ", "川").expect("2nd ok");
        let result = store.lookup("かわ", 10).expect("ok");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].frequency, 2);
        // last_used_at は real epoch でなければならない(SQLite 側
        // `record_choice_sets_last_used_at_to_nonzero` と対称、> 0 を要求)。
        assert!(
            result[0].last_used_at > 0,
            "last_used_at must be a real epoch (> 0)"
        );
    }

    /// lookup は frequency DESC を第一 key、last_used_at DESC を第二 key で
    /// ordering する(SQLite LOOKUP_SQL 対称)。
    ///
    /// `last_used_at` 同値の tie-break を直接観測するため、
    /// `record_choice` の自然時刻に依存せず、record_choice 完了後に
    /// records を直接書き換えて固定 timestamp を設定する。
    #[test]
    fn mock_lookup_orders_by_frequency_desc_then_last_used_at_desc() {
        let store = MockLearningCacheStore::new();
        store.record_choice("あい", "愛").expect("ok"); // becomes id=1
        store.record_choice("あい", "愛").expect("ok"); // freq → 2
        store.record_choice("あい", "哀").expect("ok"); // id=2 freq=1
        store.record_choice("あい", "藍").expect("ok"); // id=3 freq=1
                                                        // 直接 timestamp を上書きして tie-break を観測可能にする
                                                        // (frequency=1 の "哀" / "藍" を last_used_at で tie-break する)。
        {
            let mut records = store.records.lock().unwrap_or_else(PoisonError::into_inner);
            for r in records.iter_mut() {
                match r.chosen_kanji.as_str() {
                    "愛" => {
                        r.frequency = 2;
                        r.last_used_at = 100;
                    }
                    "哀" => {
                        r.frequency = 1;
                        r.last_used_at = 200; // 新しい
                    }
                    "藍" => {
                        r.frequency = 1;
                        r.last_used_at = 150; // 古い
                    }
                    _ => {}
                }
            }
        }
        let result = store.lookup("あい", 10).expect("ok");
        assert_eq!(result.len(), 3);
        // freq DESC: "愛" (freq=2) が先頭。
        assert_eq!(result[0].chosen_kanji, "愛");
        // freq=1 同値の中では last_used_at DESC: "哀" (200) > "藍" (150)。
        assert_eq!(result[1].chosen_kanji, "哀");
        assert_eq!(result[2].chosen_kanji, "藍");
    }

    /// limit 引数で結果件数を切り詰める。
    #[test]
    fn mock_lookup_respects_limit() {
        let store = MockLearningCacheStore::new();
        store.record_choice("あい", "愛").expect("ok");
        store.record_choice("あい", "哀").expect("ok");
        store.record_choice("あい", "藍").expect("ok");
        let result = store.lookup("あい", 2).expect("ok");
        assert_eq!(result.len(), 2);
    }

    /// 未登録 kana_input は空 `Vec` を返す(`Err` ではない)。
    #[test]
    fn mock_lookup_returns_empty_for_unknown_kana() {
        let store = MockLearningCacheStore::new();
        let result = store.lookup("みず", 10).expect("ok");
        assert!(result.is_empty());
    }

    /// `record_choice` は ASCII / カタカナ等の非ひらがな kana_input を
    /// `validate_reading` で reject する。
    #[test]
    fn mock_record_choice_rejects_invalid_kana() {
        let store = MockLearningCacheStore::new();
        let err = store.record_choice("abc", "川").unwrap_err();
        assert!(matches!(err, StorageError::InvalidField { .. }));
    }

    /// `record_choice` は PUA(U+E000)を含む surface を `validate_surface` で reject する。
    #[test]
    fn mock_record_choice_rejects_invalid_surface() {
        let store = MockLearningCacheStore::new();
        let err = store.record_choice("あい", "\u{E000}").unwrap_err();
        match err {
            StorageError::InvalidField { name, reason } => {
                assert_eq!(name, "surface");
                assert_eq!(reason, "PUA char");
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    /// 空文字 `chosen_kanji` は `validate_surface` で reject される(SQLite 対称)。
    #[test]
    fn mock_record_choice_rejects_empty_chosen_kanji() {
        let store = MockLearningCacheStore::new();
        let err = store.record_choice("あい", "").unwrap_err();
        assert!(matches!(err, StorageError::InvalidField { .. }));
    }

    /// 256 byte 超の `kana_input` は `validate_reading` で reject される(SQLite 対称)。
    /// validation.rs の上限は `len() > 256` exclusive reject なので、
    /// ひらがな 1 文字 = 3 byte × 86 文字 = 258 byte で上限を 2 byte 超過する。
    #[test]
    fn mock_record_choice_rejects_oversize_kana_input() {
        let store = MockLearningCacheStore::new();
        let oversized = "あ".repeat(86);
        let err = store.record_choice(&oversized, "愛").unwrap_err();
        assert!(matches!(err, StorageError::InvalidField { .. }));
    }

    /// 同一 `kana_input` で異なる `chosen_kanji` は別 row として保持される
    /// (UPSERT key は `(kana_input, chosen_kanji)` 複合 / SQLite 対称)。
    #[test]
    fn mock_record_choice_separates_different_chosen_kanji() {
        let store = MockLearningCacheStore::new();
        store.record_choice("あい", "愛").expect("ok");
        store.record_choice("あい", "藍").expect("ok");
        let result = store.lookup("あい", 10).expect("ok");
        assert_eq!(
            result.len(),
            2,
            "different chosen_kanji must produce distinct rows"
        );
        assert_eq!(result[0].frequency, 1);
        assert_eq!(result[1].frequency, 1);
    }

    /// `record_choice` 完了後に行数が cap を超えた場合、`last_used_at` が古い entry が
    /// drop される(SQLite UPSERT 自動 eviction 対称)。
    ///
    /// 各 record_choice 間で last_used_at が同一秒に揃わないよう、records を直接
    /// 書き換えて tie-break を制御する。Mock は per-instance cap を持つため、
    /// SQLite-side の test 用 global cap override を経由せず `with_max_rows(2)` で
    /// 直接 cap を設定する。
    #[test]
    fn mock_record_choice_auto_evicts_when_exceeding_cap() {
        let store = MockLearningCacheStore::with_max_rows(2);
        // 1 件目を insert し timestamp を最古に固定する。
        store.record_choice("あ", "亜").expect("ok");
        {
            let mut records = store.records.lock().unwrap_or_else(PoisonError::into_inner);
            records[0].last_used_at = 100;
        }
        // 2 件目を insert し timestamp を中間に固定する。
        store.record_choice("い", "以").expect("ok");
        {
            let mut records = store.records.lock().unwrap_or_else(PoisonError::into_inner);
            // 直前の push で末尾に追加されているはず。
            let last_idx = records.len() - 1;
            records[last_idx].last_used_at = 200;
        }
        // 3 件目 insert で cap=2 を 1 超過する。"あ/亜" (last_used_at=100) が evict される。
        store.record_choice("う", "宇").expect("ok");
        // 行数が cap 以内に収まっている。
        {
            let records = store.records.lock().unwrap_or_else(PoisonError::into_inner);
            assert!(
                records.len() <= 2,
                "row count {} must be <= cap 2",
                records.len()
            );
        }
        // 最古の "あ/亜" は drop されている。
        let removed = store.lookup("あ", 10).expect("ok");
        assert!(
            removed.is_empty(),
            "oldest entry あ/亜 must be evicted, got {removed:?}"
        );
    }

    /// `evict_lru(max_entries)` は最古の `last_used_at` を持つ entry から削除し、
    /// 削除件数を返す。
    #[test]
    fn mock_evict_lru_removes_oldest() {
        let store = MockLearningCacheStore::new();
        store.record_choice("あ", "亜").expect("ok");
        store.record_choice("い", "以").expect("ok");
        store.record_choice("う", "宇").expect("ok");
        // last_used_at を昇順に振って LRU 順序を明示する(同値 tie を回避)。
        {
            let mut records = store.records.lock().unwrap_or_else(PoisonError::into_inner);
            for r in records.iter_mut() {
                match r.chosen_kanji.as_str() {
                    "亜" => r.last_used_at = 100,
                    "以" => r.last_used_at = 200,
                    "宇" => r.last_used_at = 300,
                    _ => {}
                }
            }
        }
        let deleted = store.evict_lru(1).expect("ok");
        assert_eq!(deleted, 2);
        // 最も新しい "う/宇" が残っている。
        let remaining = store.lookup("う", 10).expect("ok");
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].chosen_kanji, "宇");
        // 最古 2 件は drop されている。
        assert!(store.lookup("あ", 10).expect("ok").is_empty());
        assert!(store.lookup("い", 10).expect("ok").is_empty());
    }

    /// `evict_lru` は行数が `max_entries` 以下の場合 0 を返す(no-op)。
    #[test]
    fn mock_evict_lru_returns_zero_when_under_cap() {
        let store = MockLearningCacheStore::new();
        store.record_choice("あ", "亜").expect("ok");
        let deleted = store.evict_lru(10).expect("ok");
        assert_eq!(deleted, 0);
        // entry は残っている。
        let result = store.lookup("あ", 10).expect("ok");
        assert_eq!(result.len(), 1);
    }

    /// `evict_lru` は `last_used_at` 同値 entry を `id ASC` で tie-break する
    /// (SQLite EVICT_SQL の `ORDER BY last_used_at ASC, id ASC` と対称)。
    #[test]
    fn mock_evict_lru_uses_id_asc_as_tiebreak_for_same_last_used_at() {
        let store = MockLearningCacheStore::new();
        store.record_choice("あ", "亜").expect("ok"); // id=1
        store.record_choice("い", "以").expect("ok"); // id=2
        store.record_choice("う", "宇").expect("ok"); // id=3
                                                      // 全 entry の last_used_at を同値に揃える。
        {
            let mut records = store.records.lock().unwrap_or_else(PoisonError::into_inner);
            for r in records.iter_mut() {
                r.last_used_at = 1000;
            }
        }
        // 1 件残るまで evict。tie-break は id ASC のはずなので、id=3 (宇) のみ残る。
        let deleted = store.evict_lru(1).expect("ok");
        assert_eq!(deleted, 2);
        let remaining = store.lookup("う", 10).expect("ok");
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].id, 3);
        assert_eq!(remaining[0].chosen_kanji, "宇");
    }
}
