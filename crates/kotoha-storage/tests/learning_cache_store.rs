//! L2 integration tests — `LearningCache` factory wiring / concurrency / persistence
//! / UPSERT path / lookup ordering tie-break.
//!
//! spec: docs/superpowers/specs/2026-04-26-p2-c-learning-cache-design.md §6.3
//!
//! Test 1〜3: factory wiring / concurrency / eviction persistence(plan §D1)。
//! Test 4〜5: Phase B reviewer の content-gap 指摘 2 件を Phase D で吸収する
//! 補完 test(`record_choice_updates_last_used_at_on_duplicate`,
//! `lookup_orders_by_frequency_desc_then_last_used_at_desc`)。

use std::sync::Arc;
use std::thread;
use std::time::Duration;

use kotoha_storage::database::Database;
use kotoha_storage::learning_cache::{CapOverrideGuard, LearningCacheReader, LearningCacheWriter};

// ──────────────────────────────────────────────────────────────────────────────
// helpers
// ──────────────────────────────────────────────────────────────────────────────

/// in-memory `Database` を開いて reader / writer factory pair を返す。
///
/// 同一 `Arc<Database>` から生成しているため、両 factory は同一
/// `Mutex<Connection>` を共有する(spec §6.4.1)。
fn open_factory_pair() -> (
    Arc<Database>,
    Box<dyn LearningCacheReader>,
    Box<dyn LearningCacheWriter>,
) {
    let db = Database::open_in_memory().expect("open_in_memory must succeed");
    let reader = db.learning_cache_reader();
    let writer = db.learning_cache_writer();
    (db, reader, writer)
}

// ──────────────────────────────────────────────────────────────────────────────
// Test 1: factory wiring
// ──────────────────────────────────────────────────────────────────────────────

/// writer で記録した entry を reader が観測できることを確認する(plan §D1-1)。
///
/// `Database::learning_cache_reader` / `learning_cache_writer` が同一
/// `Arc<Database>` 経由で同一 `Mutex<Connection>` を共有していることを
/// factory wiring レベルで検証する(spec §6.4.1)。
#[test]
fn factory_returns_reader_and_writer_sharing_same_data() {
    let (_db, reader, writer) = open_factory_pair();

    writer
        .record_choice("あいう", "愛")
        .expect("record_choice must succeed");

    let results = reader.lookup("あいう", 10).expect("lookup must succeed");

    assert_eq!(
        results.len(),
        1,
        "reader must see the entry written by writer through the shared connection"
    );
    assert_eq!(results[0].chosen_kanji, "愛");
    assert_eq!(results[0].frequency, 1);
}

// ──────────────────────────────────────────────────────────────────────────────
// Test 2: concurrent UPSERT (frequency loss check)
// ──────────────────────────────────────────────────────────────────────────────

/// 10 thread が同一 `(kana_input, chosen_kanji)` 対して record_choice を並列実行しても
/// `Mutex<Connection>` 直列化により全件が UPSERT-merge され `frequency = 10` になる
/// ことを確認する(plan §D1-2 / spec §4.2 / E9)。
///
/// この test は process 単独実行(integration test crate ごとに別 process)であり、
/// `learning_cache::sqlite` の unit test との `CAP_OVERRIDE_LOCK` 競合は発生しない。
/// override は使用せず、production cap = 10_000 のもとで実行する。
#[test]
fn concurrent_record_choice_is_serialized() {
    let db = Database::open_in_memory().expect("open_in_memory must succeed");

    const THREAD_COUNT: usize = 10;
    let mut handles = Vec::with_capacity(THREAD_COUNT);

    for _ in 0..THREAD_COUNT {
        let db_for_thread = Arc::clone(&db);
        handles.push(thread::spawn(move || {
            // thread ごとに writer を生成する。同 `Arc<Database>` を共有しているため
            // 同一 `Mutex<Connection>` 経由で直列化される。
            let writer = db_for_thread.learning_cache_writer();
            writer
                .record_choice("きょう", "今日")
                .expect("record_choice must not fail under concurrency");
        }));
    }

    for h in handles {
        h.join().expect("worker thread must not panic");
    }

    let reader = db.learning_cache_reader();
    let results = reader.lookup("きょう", 10).expect("lookup must succeed");

    assert_eq!(
        results.len(),
        1,
        "concurrent inserts for the same (kana, kanji) must be UPSERT-merged into a single row"
    );
    assert_eq!(
        u64::from(results[0].frequency),
        THREAD_COUNT as u64,
        "frequency must equal the number of concurrent record_choice calls without loss"
    );
}

// ──────────────────────────────────────────────────────────────────────────────
// Test 3: eviction persistence across reopens
// ──────────────────────────────────────────────────────────────────────────────

/// file-backed `Database` で auto eviction が発生したあと、`Database` を drop して
/// 同 path で再 open しても、削除済 entry は復活せず行数が cap 以内であることを
/// 確認する(plan §D1-3 / spec §6.3)。
///
/// `CapOverrideGuard::new(5)` で auto eviction の cap を 5 に縮小し、6 件目の
/// record_choice で 1 件 evict されることを検証する。再 open 後、削除された
/// kana_input(LRU)に対する lookup が空 `Vec` を返し、残った kana_input に
/// 対する lookup は entry を返すことを確認する。
#[test]
fn eviction_persists_across_factory_reopens() {
    let dir = tempfile::tempdir().expect("tempdir must succeed");
    let db_path = dir.path().join("test_eviction.db");

    // Phase 1: cap=5 で 6 件 record して 1 件 eviction を発火させる。
    {
        let db = Database::open(&db_path).expect("open phase1 must succeed");
        // CapOverrideGuard は process 全体の static を変更するため、必ず scope 内に
        // 留めて drop で復元する。本 test は integration test の単独 process で実行され、
        // unit test の `CAP_OVERRIDE_LOCK` とは別 process なので競合しない。
        let _guard = CapOverrideGuard::new(5);
        let writer = db.learning_cache_writer();

        // 0 番目を最古の last_used_at にする。同一秒内に 6 件 insert すると
        // last_used_at がすべて同値になり LRU 判定が `id ASC` の二次キーに依存して
        // しまうため、最初の 1 件のみ十分に古い時刻になるよう sleep を挟む。
        // SystemTime::now() が秒単位で前進すれば LRU は確実に 0 番目になる。
        let kanas = ["あ", "い", "う", "え", "お", "か"];
        writer
            .record_choice(kanas[0], "愛")
            .expect("record_choice must succeed");
        // last_used_at が次の秒に進むまで待つ。
        thread::sleep(Duration::from_millis(1100));

        for kana in &kanas[1..] {
            writer
                .record_choice(kana, "愛")
                .expect("record_choice must succeed");
        }

        // cap=5 に収まっていることを確認する(auto eviction で 1 件削除済)。
        let evicted_again = writer
            .evict_lru(5)
            .expect("evict_lru must succeed in phase1");
        assert_eq!(
            evicted_again, 0,
            "row count must already be <= cap=5 after the auto-eviction triggered by record_choice"
        );

        // guard を drop して override を 0 に戻し、production cap に復元する。
        drop(_guard);
        // db を drop して SQLite connection を close する。
    }

    // Phase 2: 再 open して LRU で削除されたであろう entry が復活していないことを確認する。
    {
        let db = Database::open(&db_path).expect("open phase2 must succeed");
        let reader = db.learning_cache_reader();

        // 最古に record した "あ" は LRU で削除されているはず。
        let evicted_lookup = reader
            .lookup("あ", 10)
            .expect("lookup must succeed for evicted kana");
        assert!(
            evicted_lookup.is_empty(),
            "the LRU entry must remain deleted across Database reopen, got {:?}",
            evicted_lookup
        );

        // 残った 5 件は再 open 後も lookup 可能なはず。
        for kana in ["い", "う", "え", "お", "か"] {
            let surviving = reader
                .lookup(kana, 10)
                .expect("lookup must succeed for surviving kana");
            assert_eq!(
                surviving.len(),
                1,
                "kana {kana:?} must survive eviction and persist across reopen"
            );
            assert_eq!(surviving[0].chosen_kanji, "愛");
        }
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Test 4: UPSERT updates last_used_at on duplicate (Phase B reviewer gap fill)
// ──────────────────────────────────────────────────────────────────────────────

/// Phase B reviewer 指摘の content gap を Phase D で吸収する。
///
/// `record_choice` の UPSERT path(`ON CONFLICT DO UPDATE SET last_used_at = excluded.last_used_at`)
/// が duplicate insert 時に `last_used_at` を実際に更新することを直接検証する。
/// 既存 unit test `record_choice_sets_last_used_at_to_nonzero` は新規 insert 時の
/// `last_used_at != 0` のみ確認し、UPDATE branch を直接検証していない。
///
/// 本 test は次を確認する:
/// - 1 回目 record 後の `last_used_at = t1`
/// - 1 秒以上 sleep して時刻を前進させる
/// - 2 回目 record 後の `last_used_at = t2`
/// - `t2 > t1`(UPDATE 経由で last_used_at が前進している)
/// - frequency が 2 に増加(UPSERT の `frequency = frequency + 1` path)
#[test]
fn record_choice_updates_last_used_at_on_duplicate() {
    let (_db, reader, writer) = open_factory_pair();

    writer
        .record_choice("あい", "愛")
        .expect("first record_choice must succeed");
    let first = reader
        .lookup("あい", 10)
        .expect("lookup after first record must succeed");
    assert_eq!(first.len(), 1);
    let t1 = first[0].last_used_at;
    assert_eq!(first[0].frequency, 1, "first record must yield frequency=1");

    // last_used_at は 1 秒精度の UNIX epoch seconds なので 1 秒以上待つ。
    thread::sleep(Duration::from_millis(1100));

    writer
        .record_choice("あい", "愛")
        .expect("second record_choice must succeed");
    let second = reader
        .lookup("あい", 10)
        .expect("lookup after second record must succeed");
    assert_eq!(second.len(), 1);
    let t2 = second[0].last_used_at;

    assert!(
        t2 > t1,
        "duplicate UPSERT must advance last_used_at: t1={t1}, t2={t2}"
    );
    assert_eq!(
        second[0].frequency, 2,
        "duplicate UPSERT must increment frequency by 1"
    );
}

// ──────────────────────────────────────────────────────────────────────────────
// Test 5: lookup tie-break on last_used_at DESC (Phase B reviewer gap fill)
// ──────────────────────────────────────────────────────────────────────────────

/// Phase B reviewer 指摘の content gap を Phase D で吸収する。
///
/// `LOOKUP_SQL` の `ORDER BY frequency DESC, last_used_at DESC` のうち、
/// 二次キー `last_used_at DESC` 経路を直接検証する。既存 unit test
/// `lookup_orders_by_frequency_desc` は frequency 一次キーのみ確認しており、
/// frequency tie 時の last_used_at tie-break path を検証していない。
///
/// 本 test は次の 3 entry を準備する:
/// - A: frequency=2, last_used_at = t_A
/// - B: frequency=2, last_used_at = t_B (> t_A)
/// - C: frequency=1, last_used_at = t_C (> t_B)
///
/// 期待される lookup 順序: B → A → C
/// (一次キー frequency DESC で {A,B} > C、frequency tie の {A,B} 内では
///  二次キー last_used_at DESC で B > A)。
#[test]
fn lookup_orders_by_frequency_desc_then_last_used_at_desc() {
    let (_db, reader, writer) = open_factory_pair();

    // Step 1: A を 1 回 record(frequency=1, last_used_at=t0)。
    writer
        .record_choice("おなじ", "Aかんじ")
        .expect("record A first time");

    // Step 2: 1 秒以上 sleep してから B を 1 回 record(frequency=1, last_used_at=t1 > t0)。
    thread::sleep(Duration::from_millis(1100));
    writer
        .record_choice("おなじ", "Bかんじ")
        .expect("record B first time");

    // Step 3: 1 秒以上 sleep してから A を 2 回目 record
    // (UPSERT で frequency=2 / last_used_at=t2 > t1)。
    thread::sleep(Duration::from_millis(1100));
    writer
        .record_choice("おなじ", "Aかんじ")
        .expect("record A second time");

    // Step 4: 1 秒以上 sleep してから B を 2 回目 record
    // (UPSERT で frequency=2 / last_used_at=t3 > t2)。
    thread::sleep(Duration::from_millis(1100));
    writer
        .record_choice("おなじ", "Bかんじ")
        .expect("record B second time");

    // Step 5: 1 秒以上 sleep してから C を 1 回だけ record(frequency=1, last_used_at=t4 > t3)。
    thread::sleep(Duration::from_millis(1100));
    writer
        .record_choice("おなじ", "Cかんじ")
        .expect("record C only once");

    // 結果: A.freq=2 (t2), B.freq=2 (t3), C.freq=1 (t4)
    // 期待 lookup 順序: B (freq=2, t3) → A (freq=2, t2) → C (freq=1, t4)
    let results = reader.lookup("おなじ", 10).expect("lookup must succeed");

    assert_eq!(
        results.len(),
        3,
        "expected exactly 3 distinct chosen_kanji entries"
    );
    assert_eq!(
        results[0].chosen_kanji, "Bかんじ",
        "first result must be the higher-frequency entry with the most recent last_used_at"
    );
    assert_eq!(results[0].frequency, 2);

    assert_eq!(
        results[1].chosen_kanji, "Aかんじ",
        "second result must be the higher-frequency entry with the older last_used_at (tie-break)"
    );
    assert_eq!(results[1].frequency, 2);

    assert_eq!(
        results[2].chosen_kanji, "Cかんじ",
        "third result must be the lower-frequency entry"
    );
    assert_eq!(results[2].frequency, 1);
}
