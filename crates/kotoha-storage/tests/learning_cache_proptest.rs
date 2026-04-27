//! proptest — `LearningCache` の 3 invariant を property-based で検証する。
//!
//! spec: docs/superpowers/specs/2026-04-26-p2-c-learning-cache-design.md §6.5
//!
//! `PROPTEST_CASES = 64`(`ProptestConfig::with_cases(64)`)で実行する。
//!
//! Strategy 設計方針:
//! - `hiragana_string()`: ひらがな Unicode ブロック `'\u{3041}'..='\u{309F}'` から
//!   1〜32 文字を生成する。`validate_reading` を必ず通過する。
//! - `kanji_string()`: 実用漢字集合 10 種から 1〜10 文字を選択生成する。
//!   `validate_surface` を必ず通過する。
//! - 上記により Invariant 3(valid 入力に対して `record_choice` は `Ok(())` を返す)
//!   は strategy-level で保証される。

use kotoha_storage::database::Database;
use kotoha_storage::learning_cache::CapOverrideGuard;
use proptest::prelude::*;

// ──────────────────────────────────────────────────────────────────────────────
// strategy 定義
// ──────────────────────────────────────────────────────────────────────────────

/// ひらがな 1〜32 文字からなる文字列を生成する。
///
/// 生成される全文字列は `validate_reading` を通過する(spec §9.2、
/// `U+3040..=U+309F` は hiragana block、本 strategy はそのうち
/// `U+3041..=U+3096` に限定して生成する。`U+3097`〜`U+309F` には未割り当て /
/// 結合用文字が含まれるため、ASCII 互換の安定範囲のみを使う。
/// `validation.rs` 側の既存 proptest と同等の regex strategy を採用する)。
fn hiragana_string() -> impl Strategy<Value = String> {
    // proptest の regex strategy は `Strategy<Value = String>` を返すため、
    // 追加の prop_map は不要(`&str` は `Strategy` に対する `Arbitrary`-like
    // shorthand として展開される)。
    "[\u{3041}-\u{3096}]{1,32}"
}

/// 実用漢字集合 10 種から 1〜10 文字を選択して文字列を生成する。
///
/// 生成される全文字列は `validate_surface` を通過する(spec §9.1、
/// 漢字は control char / bidi / PUA / Variation Selector / Tag char に
/// 該当しないため、共通 validate_field 制約を必ず通過する)。
fn kanji_string() -> impl Strategy<Value = String> {
    proptest::collection::vec(
        proptest::sample::select(vec![
            '愛', '夢', '空', '花', '光', '山', '川', '海', '月', '日',
        ]),
        1..=10,
    )
    .prop_map(|chars: Vec<char>| chars.into_iter().collect::<String>())
}

/// `(kana_input, chosen_kanji)` のペアを 0〜200 件含む `Vec` を生成する。
fn record_sequence() -> impl Strategy<Value = Vec<(String, String)>> {
    proptest::collection::vec((hiragana_string(), kanji_string()), 0..=200)
}

// ──────────────────────────────────────────────────────────────────────────────
// proptest
// ──────────────────────────────────────────────────────────────────────────────

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    /// Invariant 1: 任意の record sequence 後も行数が cap 以下である。
    ///
    /// `CapOverrideGuard::new(20)` で cap=20 を設定し、`record_choice` の
    /// 自動 eviction(spec §4.2)により行数が常に 20 以下に収まることを確認する。
    /// 検証手順: sequence を全 record したあと `evict_lru(20)` が `Ok(0)` を返す
    /// ことを確認する(行数 ≤ cap が成立していれば evict_lru は何も削除しないため)。
    #[test]
    fn invariant_row_count_stays_within_cap(records in record_sequence()) {
        let db = Database::open_in_memory().expect("open_in_memory must succeed");
        let _guard = CapOverrideGuard::new(20);
        let writer = db.learning_cache_writer();

        for (kana, kanji) in &records {
            // strategy が valid 入力のみを生成するため、Ok 以外は発生しないはず。
            writer
                .record_choice(kana, kanji)
                .expect("record_choice must succeed for strategy-generated valid inputs");
        }

        // evict_lru(20) が Ok(0) を返すと、record_choice の自動 eviction が
        // すでに行数を 20 以下に収めていることを示す。
        let evicted = writer
            .evict_lru(20)
            .expect("evict_lru must succeed");
        prop_assert_eq!(
            evicted,
            0,
            "row count must be <= cap=20 after the record sequence; \
             follow-up evict_lru(20) returned {} which means auto-eviction did not enforce the cap",
            evicted
        );
    }

    /// Invariant 2: lookup 結果が frequency DESC で単調非増加である。
    ///
    /// record sequence 後に sequence の先頭 kana_input に対して `lookup(_, 100)`
    /// を呼び出し、結果 `results[i].frequency >= results[i + 1].frequency` が
    /// 全 i で成立することを確認する(spec §4.3 一次キー frequency DESC)。
    /// 同 frequency 内 last_used_at DESC の二次キーは本 invariant に含めない
    /// (SQL 仕様にゆだねる、L2 integration test 5 で別途検証)。
    #[test]
    fn invariant_lookup_result_is_frequency_desc(records in record_sequence()) {
        // sequence が空の場合は lookup 対象が存在しないので skip する。
        prop_assume!(!records.is_empty());

        let db = Database::open_in_memory().expect("open_in_memory must succeed");
        let _guard = CapOverrideGuard::new(20);
        let writer = db.learning_cache_writer();
        let reader = db.learning_cache_reader();

        for (kana, kanji) in &records {
            writer
                .record_choice(kana, kanji)
                .expect("record_choice must succeed for strategy-generated valid inputs");
        }

        let target_kana = &records[0].0;
        let results = reader
            .lookup(target_kana, 100)
            .expect("lookup must succeed");

        for i in 0..results.len().saturating_sub(1) {
            prop_assert!(
                results[i].frequency >= results[i + 1].frequency,
                "lookup results must be non-increasing on frequency: \
                 results[{}].frequency={} < results[{}].frequency={}",
                i,
                results[i].frequency,
                i + 1,
                results[i + 1].frequency
            );
        }
    }

    /// Invariant 3: strategy が生成した全入力に対して `record_choice` が `Ok(())` を返す。
    ///
    /// `hiragana_string` / `kanji_string` は `validate_reading` / `validate_surface`
    /// を通過する文字列のみを生成するため、`record_choice` は `Err(InvalidField)` を
    /// 返さないはずである。万が一 `Err` が観測された場合は strategy の生成ロジックか
    /// validation 実装のいずれかにバグがあると判定する。
    #[test]
    fn invariant_valid_inputs_always_succeed(records in record_sequence()) {
        let db = Database::open_in_memory().expect("open_in_memory must succeed");
        let _guard = CapOverrideGuard::new(20);
        let writer = db.learning_cache_writer();

        for (kana, kanji) in &records {
            let result = writer.record_choice(kana, kanji);
            prop_assert!(
                result.is_ok(),
                "record_choice must return Ok for strategy-generated valid inputs: \
                 kana={:?}, kanji={:?}, err={:?}",
                kana,
                kanji,
                result
            );
        }
    }
}
