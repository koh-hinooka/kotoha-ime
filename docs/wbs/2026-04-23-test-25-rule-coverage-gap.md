# 2026-04-23 test/25-rule-coverage-gap 実装ログ

## 対象

- ISSUE: [#25 — test: close the 55-key golden fixture coverage gap in `romaji::rules::RULES`](https://github.com/std-koh-hinooka/kotoha-ime/issues/25)
- PR: [#50 — test(kotoha-core): close 55-key golden fixture coverage gap (#25)](https://github.com/std-koh-hinooka/kotoha-ime/pull/50)
- merge commit: `11d7222`
- branch: `test/25-rule-coverage-gap` (merge 後削除)

## 実施内容

- `crates/kotoha-core/src/lib.rs` の crate root に test 専用 accessor `#[doc(hidden)] pub fn __test_only_romaji_rules() -> &'static [(&'static str, &'static str)]` を追加した。`romaji::rules::RULES` は `pub(crate)` のままに保ち、integration test からの参照のみを accessor 経由で許可する設計とした。`__test_only_` prefix + `#[doc(hidden)]` を明示することで外部 crate からの依存を実質的に遮断している。
- `crates/kotoha-core/tests/fixtures/romaji_cases.tsv` の末尾に 55 行の golden row と 14 本の section comment を追加した。形態素カテゴリに沿って以下 13 グループに編成した: yoon k-row / s-row / t-row / n-row / h-row / m-row / r-row / g-row 各拡張 (yi/ye 変種)、z-row + j-row、dy-row、b-row、p-row、gw-row 拡張、wh-row、small x-prefixed kana (xya/xyu/xyo/xn/xwa/xtsu + lwa/ltsu)。既存 section style (`# --- <category> ---`) を踏襲した。
- `crates/kotoha-core/tests/romaji_golden.rs` に `every_rule_key_has_golden_coverage` test を追加した。`kotoha_core::__test_only_romaji_rules()` の戻り値を走査し、fixture の input 列に含まれない key を `(key, kana)` ペアで列挙して assert する構造で、将来 `RULES` に entry が追加された際に未カバレッジをコンパイル可能状態で検知する。
- 旧 `fixture_has_at_least_200_cases` (≥200 の floor check) は新 cross-check が厳密上位であるため、in-place で置換し削除した。
- 新規追加した 55 行は全て `every_fixture_row_matches_converter` で `RomajiConverter::convert` 出力と突合され pass した (初回実行で green) ため、`rules.rs` の kana 値と fixture の kana 値が完全一致していることが機械的に保証された。

## 検証結果

pre-push hook と main-agent spot-check の両方で以下 5 コマンドが全て green:

| コマンド | 結果 |
|---------|------|
| `cargo build --workspace` | clean (finished in 0.10s) |
| `cargo test --workspace` | 全 integration / lib / doc test pass (romaji_golden の 2 test を含む) |
| `cargo test -p kotoha-core --test romaji_golden` | `every_rule_key_has_golden_coverage` / `every_fixture_row_matches_converter` いずれも pass |
| `cargo clippy --workspace --all-targets -- -D warnings` | warnings ゼロ |
| `cargo fmt --all --check` | diff なし |

## カバレッジ変化

- before: 152/207 rule keys (73.4%)
- after: 207/207 rule keys (100%)

fixture の data row 総数は 207 から 262 へ増加した (55 新規 + 既存 207 は net +55 で一致。既存 row には RULES 非対応の sokuon 合成 / pending tail / composite word が含まれるため row 数 > rule 数)。

## レビュー結果

Medium tier 5 dimensions (security / performance / architecture / testing / API-ergonomics) + `secrets-check` を実施。

- Critical: 0, High: 0, Medium: 0, Low: 0
- Informational: 5 (各 dimension で CLEAN)
- `secrets-check`: CLEAN (TSV は ASCII romaji + 平仮名のみ、secret pattern との類似度ゼロ)
- `owasp-security` は test-only PR のため省略 (size matrix の想定どおり)

## つまずき

- ISSUE body には「RULES は 213 entries」と記述されていたが、実際の `rules.rs` は 207 entries であった。55 missing keys の集合は ISSUE list と完全一致 (差分ゼロ) であったため、PR 範囲は当初想定どおりに進行した。213 → 207 の差分は ISSUE 起票時と現在の `rules.rs` との drift であり、本 PR では対処しない (RULES 追加は別 ISSUE のスコープ)。
- 初版 fixture 編集で data row 末尾の trailing tab (pending 列の空文字列) を 3 行だけ誤って省略した。loader が `cols.len() >= 3` を要求するため、そのまま test を流すと panic する状態となった。Python 1-liner で全 data row に trailing tab を補填してリカバーした。
- xtsu / ltsu / xn / xwa / lwa 等の `x`-prefix 系は `rules.rs` 上で 3 グループに分散しており (yoon 近傍 / small-form / n-row special)、grouping 方針を「RULES 上の位置」ではなく「形態素的 family」に統一した。結果として xn は `small-form x-prefixed kana` セクションにまとめ、読者が x-prefix 系の全体像を一箇所で俯瞰できるよう整理した。

## 成果物リンク

- ISSUE: https://github.com/std-koh-hinooka/kotoha-ime/issues/25
- PR: https://github.com/std-koh-hinooka/kotoha-ime/pull/50
- merge commit: [11d7222](https://github.com/std-koh-hinooka/kotoha-ime/commit/11d7222)
- 関連 fixture: `crates/kotoha-core/tests/fixtures/romaji_cases.tsv`
- 関連 test: `crates/kotoha-core/tests/romaji_golden.rs`
- 関連 accessor: `crates/kotoha-core/src/lib.rs`
