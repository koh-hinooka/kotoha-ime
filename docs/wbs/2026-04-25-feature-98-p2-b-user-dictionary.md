---
title: P2-B User dictionary 実装ログ
date: 2026-04-25
branch: feature/98-p2-b-user-dictionary
issue: 98
pr: 101
follow-up-issue: 102
parent-spec: docs/superpowers/specs/2026-04-25-p2-b-user-dictionary-design.md
parent-plan: docs/superpowers/plans/2026-04-25-feature-98-p2-b-user-dictionary.md
---

# P2-B(User dictionary)実装ログ

## サマリ

- kotoha-storage 新 crate 導入(Phase A 完了、scaffolding + StorageError + path containment + validation + migrations + Database::open)
- UserVocab 統合(Phase B〜C 完了、UserVocabStore trait + Sqlite/Mock + LearningCacheStore skeleton + Database factory + UserVocab impl VocabularyLookup + DictionaryConfig.user_vocab_db_path)
- kotoha-dict CLI 実装(Phase D 完了、4 subcommand + reading auto-detect + exit code 体系)
- end-to-end + property + regression(Phase E 完了、Layer 4 e2e 4 PASS + proptest 4 PASS + 退行ゼロ)
- deferred Medium / Low 取込(Phase F 完了 7/8、F5 cargo audit は PR review 完了後に実施・clean、F8 Low triage は本 WBS + #102 に統合済)
- **4 dimension review 実施**(security / architecture / testing / performance、Critical 1 + High 9 を 7 commit で全消化、Medium 16 + Low 15 を #102 へ集約)

## metric

| 項目 | 値 |
|---|---|
| default features 合計 | 264 PASS(P2-A 175 baseline + 85 kotoha-storage 単体 test + 4 proptest) |
| `mock-backend,dict` features 合計 | 327 PASS(P2-A 223 baseline + ~104 storage / dict-persist 関連) |
| `mock-backend,dict-persist` features 合計 | **348 PASS(P2-B 新 baseline、review 修正後)** |
| 工数(実)| 1 セッション(plan 50 task + 7 review fix を subagent dispatch で実装) |
| 変更 file 数 | 31(plan + WBS 含む)|
| 変更 lines | 8854 insertions(plan 5224 行 + 実装 ~3300 行 + WBS 330 行)|
| commit 数 | 46(develop からの差分、46 = 39 元実装 + 6 review-Critical/High + 1 perf)|

## Phase 別実装結果

| Phase | task 数 | 完了 | 主要 commit | 学び |
|---|---|---|---|---|
| Phase A | 8 | 8/8 ✅ | A1 scaffold → A8 invariant | tempfile blacklist (/tmp 衝突) を A3 で発見、target/test-tmp-path/ 配下に redirect|
| Phase B | 9 | 9/9 ✅ | B1 trait → B9 mock | trait 拡張(B4 find_by_prefix)で全 impl の同期更新が必要 |
| Phase C | 6 | 6/6 ✅ | C1 feature gate → C6 e2e | DictionaryConfig literal site が想定 2 → 実 5(`grep` で網羅必須) |
| Phase D | 9 | 9/9 ✅ | D1 Cargo.toml → D9 exit code | bin/dict.rs を D2 stub で先置きしないと lefthook fmt-check が `[[bin]]` reference の実体不在で fail |
| Phase E | 5 | 5/5 ✅ | E1 e2e → E5 baseline | ConvertOptions が `#[non_exhaustive]` のため struct expression 不可、`Default::default()` で構築 |
| Phase F | 8 | 6/8 ✅ + 2 deferral | F1〜F4 / F6〜F7 / F5 + F8 deferral | F2 並列 test の reading は `format!("あ{i}")` ASCII 混入を `char::from_u32(0x3042 + i)` で hiragana 化 |
| Phase G | 4 | (本 WBS で進行中)| G1 WBS / G2 pre-push / G3 + G4 PR | — |

## 学び / 判断ログ

### 1. Plan の test code に潜む `/tmp` blacklist 衝突
- Task A3 / Task D8 の 2 箇所で plan 提供の test code が `tempfile::tempdir()` 経由で `/tmp` 配下に作成しようとしたが、`kotoha-storage::path::resolve_data_dir()` の system path blacklist `/tmp` と衝突。
- 修正: `target/test-tmp-path/`(A3)/ `target/test-tmp-cli/`(D8)配下の helper(`make_tempdir` / `safe_tempdir`)に redirect。仕様(blacklist の security 方針)は変更せず、test 側の placement のみ調整。

### 2. plan の B7 → B8 順は依存逆転
- Plan の Task B7 が `SqliteLearningCacheStore::new` を参照するが、実装は B8 で行う。strict に plan 順で実行すると B7 で compile error。
- 修正: 実行順を **B8 → B7 → B9** に reorder。dependency-respecting commit で plan 順に従う形より cleaner。

### 3. clippy `-D warnings` と placeholder field
- B1 / B8 の placeholder struct field に `#[allow(dead_code)]` を per-field で付与し、impl 完了 task で削除する pattern を確立。`#[cfg(test)] _suppress_unused()` helper は cfg(test) gate のため non-test build で warning が残り、`#[allow(unused_imports)]` に置換する経験を B2 で得た。

### 4. Plan 期待値と実装差異のうち修正不要なもの
- plan A4 「23 passed」 → 実 21 passed: plan の typo、test 関数列挙数とのズレ。実装は 21 で正解。
- plan A2「migration runner 7 passed」/ A5「7 passed」 → 一致(plan の通り)。
- plan E4 default 「175 PASS」/ E5「dict 223 PASS」: P2-A 当時の baseline 値。P2-B では kotoha-storage 自身の test が default で走るため 264 / 327 が新値。退行ゼロを確認するのが本旨であり、plan baseline 数値は obsolete として扱った。

### 5. ConvertOptions `#[non_exhaustive]` の側面
- E1 で plan 提供の struct expression `ConvertOptions { top_k: 5, ... }` が `#[non_exhaustive]` で cross-crate 構築不可。
- 修正: `ConvertOptions::default()` を採用(`top_k=5, temperature=0.0, seed=Some(0)` と同値、Default trait 実装で網羅)。

### 6. clippy 改善余地の自動取り込み
- A5 const assertion: `assert!(LATEST_VERSION >= 1)` → `clippy::assertions_on_constants` を `const { assert!(...) }` で compile-time gate に格上げ。
- 多数の commit で `format!("...{var}", var)` → inline format args に統一(clippy idiom)。

### 7. PR review (4 dimension)
- security / architecture / testing / performance の 4 dim を逐次実行(memory 制約で並列不可)。accessibility は CLI/backend 性質上適用範囲外。
- 結果: Critical 1 + High 9 + Medium 16 + Low 15 = 41 finding。Critical+High を 7 commit で全消化(`c7ae183` quota / `4a5c925` lefthook / `9cd2750` lock_conn / `59f95a7` find_by_id / `af28bd7` proptest / `003071a` json_escape / `dacb15f` perf-H1+H2)。Medium+Low は #102 へ集約。
- review で得られた知見:
  - **lefthook gate 抜け**: `dict-persist` feature は default / dict feature では実行されず、Layer 2/3/4 の test が pre-push で機械保証されていなかった(A-H1)。3-feature gate 化で恒久対処。
  - **DIP 厳密性**: `UserVocabStore` trait が callee crate (`kotoha-storage`) 側に置かれており strict DIP ではなく DI。ADR 0015 の更新候補(arch-M-5、#102 へ defer)。
  - **prepare_cached の Phase 3 影響**: SQLite Statement の都度 prepare は IBus engine の per-keystroke 経路で累積コスト発生。`prepare_cached` 化で恒久対処。
  - **count(*) → EXISTS+OFFSET**: quota check が full table scan だったため bounded O(max) に変更。

## F5 cargo audit 結果(本セッション内で完了)

`cargo install cargo-audit --locked` で v0.22.1 を install、`cargo audit --json` を実行:

```json
{
  "database": { "advisory-count": 1058, "last-updated": "2026-04-25T11:01:07-04:00" },
  "lockfile": { "dependency-count": 167 },
  "vulnerabilities": { "found": false, "count": 0, "list": [] },
  "warnings": {}
}
```

- 167 transitive dep に対し vulnerability / unmaintained / unsound / notice すべて 0
- 新規 dep(rusqlite 0.32 + bundled libsqlite3-sys / tempfile 3 / assert_cmd 2 / serial_test 3)も clean
- RustSec advisory DB commit `930c3aa`(2026-04-25 時点)で確認

**結論**: F5 は本 PR で clean PASS。継続的な定期実行は global CLAUDE.md「Security Workflow → On dependency update」に従う。

## deferred 項目

### Phase F 内 deferral

- **F5(cargo audit)**: ✅ 本セッション内で完了(↑)。
- **F8(残り Low 11 件 triage)**: ✅ ISSUE #95 [#issuecomment-4319019093](https://github.com/std-koh-hinooka/kotoha-ime/issues/95#issuecomment-4319019093) の Low 11 件 + 本 PR review の 31 件を統合し、follow-up issue #102 で triage 提案付きで記録済。

### 設計上の deferral

- **sec-M5 quota**: `USER_VOCAB_MAX_ROWS = 50_000` は P2-B 着地時点の暫定値。P2-C(LearningCache 本実装)/ P2-D で最大想定 entry 数が定まれば再評価。
- **LearningCache 本実装**: P2-B では `SqliteLearningCacheStore` の skeleton(stub `lookup`/`record_choice`/`evict_lru`)のみ公開。本実装は P2-C で行う(spec §6.3、§2 Out of scope に従う)。

## 参照

- spec: `docs/superpowers/specs/2026-04-25-p2-b-user-dictionary-design.md`
- plan: `docs/superpowers/plans/2026-04-25-feature-98-p2-b-user-dictionary.md`
- ADR 0014 D7: `docs/adr/0014-phase-2-dictionary-layer-architecture.md`
- ADR 0015: `docs/adr/0015-kotoha-storage-sqlite-adoption.md`

---

## PR merge 後の TODO

- ✅ `pr:` field に PR# を追記(101)
- ✅ F5 cargo audit 実行結果を「F5 cargo audit 結果」section に追記(0 vulnerabilities)
- ✅ F8 残り Low 11 件の triage を follow-up issue 化(#102、4 dim review の 31 件と統合)
- ⏳ 本 WBS を develop に直接 push(project CLAUDE.md「WBS 直接 push の例外」に従う、merge 後)
- ⏳ #102 の triage 結果に従って follow-up PR を作成(優先順位: Phase 3 prerequisites → next sprint → defer)
