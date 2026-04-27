---
title: P2-C LearningCache 本実装ログ
date: 2026-04-26
branch: feature/105-p2-c-learning-cache
issue: 105
pr: <PR# が確定したら追記>
follow-up-issue: <Medium / Low deferred Issue# が確定したら追記>
parent-spec: docs/superpowers/specs/2026-04-26-p2-c-learning-cache-design.md
parent-plan: docs/superpowers/plans/2026-04-26-feature-105-p2-c-learning-cache.md
---

# P2-C(LearningCache 本実装)実装ログ

## サマリ

- `UserVocabReader` / `UserVocabWriter` の ISP split 完了(Phase A、旧 `UserVocabStore` の 2 trait 分割)
- `LearningCacheReader` / `LearningCacheWriter` の ISP split 完了(Phase A、旧 `LearningCacheStore` の 2 trait 分割)
- `SqliteLearningCacheStore` 本実装完了(Phase B、`lookup` / `record_choice` UPSERT / `evict_lru` / 自動 LRU 退避 / cap 制御)
- v002 migration 追加完了(Phase C、`v002_learning_cache_index.sql` で `idx_learning_cache_last_used` を index 化)
- L2 integration test + proptest 3 invariant 追加完了(Phase D、退行ゼロ確認)
- production binary cleanliness 回復完了(Phase D fix、`CapOverrideGuard` 系を `test-helpers` feature flag で gate)

## metric

実機検証ベース(`cargo test --workspace [...] --features kotoha-storage/test-helpers` 出力)で記録する。

| 項目 | 値 |
|---|---|
| `default` features 合計 | **320 PASS**(P2-B baseline 281 から +39) |
| `dict` features 合計 | **379 PASS**(P2-B baseline 340 から +39) |
| `dict-persist` features 合計(P2-C 新 baseline) | **394 PASS**(P2-B baseline 355 から +39) |
| 工数(実) | 1 セッション(plan 執筆 + 実装 + Phase D fix を subagent dispatch で消化) |
| 変更 file 数 | 20(WBS 含めると 21) |
| 変更 lines | +1756 / -166(WBS 除く、`git diff 130a214..HEAD --stat` 出力) |
| commit 数 | 27(`git log --oneline 130a214..HEAD` 集計、Phase A 8 + B 12 + C 3 + D 2 + Phase D fix 1 + Phase E WBS 1) |
| production binary 検査 | `nm -C` で test-only symbol 0 件(`CapOverrideGuard` / `LEARNING_CACHE_MAX_ROWS_TEST_OVERRIDE` / `CAP_OVERRIDE_LOCK` / `effective_max_rows` を全件 grep で確認) |
| clippy 警告 | 0(`--features kotoha-storage/test-helpers -- -D warnings`) |
| `cargo audit` | 0 vulnerability |
| lefthook pre-push 6 step | 全 PASS(manifest-check / build / clippy / test / test-dict / test-dict-persist) |

## Phase 別実装結果

| Phase | task 数 | 完了 | 主要 commit | 学び |
|---|---|---|---|---|
| Phase A(ISP split) | 7 | 7/7 ✅ | `afaaf5d` UserVocab split → `1d3301d` LearningCache split → `400d0a3` 退行ゼロ verify | Plan A4 が learning_cache_* factory も含んでいたが main agent 判断で user_vocab_* のみに scope 絞り、Phase B B11 で learning_cache_* factory 化 |
| Phase B(LearningCache 本実装) | 12 | 12/12 ✅ | `76534cb` lookup TDD red → `c23d6e3` lookup green → `73e3cd3` UPSERT → `fd124b7` evict_to_cap → `3c8391d` evict_lru → `c48ce02` factory + B11 → `eef7cc9` race fix → `726b07d` test 補完 | Plan B1 と B11 を機能上 swap(B1 で B11 先取り、B11 で factory test 追加に再定義)、`CapOverrideGuard::lock_only()` 追加で race condition 解消、初回実装で 7 件不足した test を `726b07d` followup commit で吸収 |
| Phase C(v002 migration) | 3 | 3/3 ✅ | `694b575` migration file → `d7aa564` MIGRATIONS 配列登録 → `90132af` runner test | v002 を MIGRATIONS array に push する順序が LATEST_VERSION 更新と分離されていたため commit 単位で疎通確認できる構成にした |
| Phase D(test 拡張) | 2 + 1 fix | 3/3 ✅ | `ec9752d` L2 integration → `896c663` proptest 3 invariant → `42f5cd2` test-helpers feature flag | Phase D 当初は integration test crate からアクセスするため `CapOverrideGuard` 系を `pub` 昇格していたが spec deviation だったため `42f5cd2` で `test-helpers` feature flag に変更し production binary cleanliness を回復 |
| Phase E(本 WBS で進行中) | 7 | E1〜E4 を本セッションで実施、E5〜E7 は main agent が引き継ぐ | E1 WBS / E2 静的解析 / E3 push / E4 PR | — |

## 学び / 判断ログ

### 1. Phase A の plan scope と main agent 判断の差異

- Plan の Task A4 は `learning_cache_reader` / `learning_cache_writer` factory も含んでいたが、main agent 判断で **`user_vocab_*` factory のみに scope を絞った**。
- 理由: A4 で 4 factory を一括 swap すると Phase A 完了時点で `learning_cache_*` の test fixture が用意できず、build break が長期化するため。
- 結果: Phase B B11(`c48ce02`)で `learning_cache_*` factory 化 + factory test 追加に再定義し、機能上 plan 通り着地した。

### 2. plan 未記載の 3 file 修正必須(Phase A)

- `kotoha-core/src/dict/backend.rs`、`kotoha-core/tests/dict_backend_user_vocab.rs`、`kotoha-storage/src/user_vocab/mod.rs` の 3 file は plan に列挙されていなかったが、Phase A の ISP split 完了には修正必須だった。
- 教訓: trait 分割系 plan は `grep -r "<old_trait_name>"` を plan 執筆段階で実行し、直接参照のみならず `use` import / re-export / pub use chain も列挙する。

### 3. Phase B B1 と B11 の swap

- Plan は B1 = `learning_cache_reader/writer` factory 追加、B11 = proptest 追加 だったが、B1 で factory を先行追加すると B2〜B10 の test fixture が `Database::learning_cache_reader()` 経由で取れない問題があった。
- 修正: B1 で B11 (proptest) を先取り、B11 を factory finalize + factory test に再定義。
- commit hash: B11 finalize = `c48ce02`、B11 proptest = `896c663`(Phase D に再配置)。

### 4. race condition 修正(`CapOverrideGuard::lock_only()`)

- `record_choice` test を `cargo test` で並列実行すると `LEARNING_CACHE_MAX_ROWS_TEST_OVERRIDE` の thread_local 設定が干渉する race condition が発生した(`eef7cc9`)。
- 修正: `CapOverrideGuard::lock_only()` を追加して mutex のみ取得し override 値は変更しない pattern を確立。production code は不変のまま test 側のみで対応。

### 5. plan §6.2 23 件単体 test の欠落

- Plan §6.2 で「23 件 L1 unit test」を要件としたが、初回実装(`bb5a65a`〜`f056265`)で 16 件しか書けず 7 件不足。
- 修正: `726b07d` followup commit で record_choice / lookup / cap-guard validation の漏れ 7 件を追加。
- 教訓: plan に列挙された test 件数は subagent prompt に明示し、subagent 報告で「N 件中 M 件 PASS」を numerical に検証する。

### 6. Phase D fix(`42f5cd2`)— `pub` 昇格の spec deviation 回復

- Phase D 当初実装で integration test crate(`tests/learning_cache_store.rs`)からアクセスするため `CapOverrideGuard` / `LEARNING_CACHE_MAX_ROWS_TEST_OVERRIDE` / `CAP_OVERRIDE_LOCK` / `effective_max_rows` を `pub` 昇格していた。
- 問題: production binary に test-only symbol が漏出し spec の binary cleanliness 要件に違反。
- 修正: `kotoha-storage/Cargo.toml` に `test-helpers` feature flag を追加し、当該 4 symbol を `#[cfg(any(test, feature = "test-helpers"))]` で gate。lefthook の test step を `--features kotoha-storage/test-helpers` 込みに更新(2026-04-26 PR で同時適用)。
- 検証: `nm -C` で release binary 内に当該 symbol が 0 件であることを確認。

### 7. ISP split による 4 trait 確立

- Phase A〜B の完了時点で `UserVocabReader` / `UserVocabWriter` / `LearningCacheReader` / `LearningCacheWriter` の 4 trait が `kotoha-storage` の public API として確立された。
- 旧 `UserVocabStore` / `LearningCacheStore` は完全削除済み(`afaaf5d` / `1d3301d` で削除確認)。
- factory も `Database::user_vocab_reader()` / `user_vocab_writer()` / `learning_cache_reader()` / `learning_cache_writer()` の 4 method 体系に移行(`1c1da24` / `c48ce02`)。

### 8. v002 migration の auto-upgrade path

- Phase C で `v002_learning_cache_index.sql` を追加し `idx_learning_cache_last_used ON learning_cache(last_used_at)` を導入。
- `evict_lru` の `ORDER BY last_used_at ASC LIMIT N` を index で走査可能にし、cap eviction の計算量を O(N) から O(log N + cap) に改善。
- runner test(`90132af`)で fresh DB / v001 から v002 への自動 upgrade の双方を検証済み。

## 4-dim review 結果

本 Phase E 前半(E1〜E4)時点では未実施。**Phase E 後半(E5)で `agent-teams:team-review` skill を 4 dimension(security / architecture / testing / performance)で実行予定**。

| dimension | Critical | High | Medium | Low |
|---|---|---|---|---|
| security | pending | pending | pending | pending |
| architecture | pending | pending | pending | pending |
| testing | pending | pending | pending | pending |
| performance | pending | pending | pending | pending |
| **合計** | (E5 で確定)| (E5 で確定)| (新規 Issue に移管)| (新規 Issue に移管)|

E6 で Critical / High finding を全消化、Medium / Low は新規 follow-up Issue に移管予定(spec §7.3 deferred Issue 方針準拠)。

## deferred 項目(新規 Issue に移管予定)

以下の Medium / Low finding は PR merge 後に新規 follow-up Issue を起票して管理する候補である。E5 review 結果に応じて確定する。

- **connection pool(perf-M-1)**: `Mutex<Connection>` をコネクションプールに置き換える。Phase 3 IBus engine から高頻度で呼び出す前提で遅延を測定し、必要性が確認された場合に実施する
- **Mutex poison handling 改善(sec-L-2)**: `lock_conn` が panic する代わりに `StorageError::Sqlite` を返す recover 経路を追加する
- **LearningCache CLI 公開検討(Phase 3-A 以降)**: `kotoha-dict cache list` / `kotoha-dict cache flush` の subcommand を Phase 3 kick-off で判断する
- **cap 値の動的化 / config 化(Phase 5)**: `LEARNING_CACHE_MAX_ROWS` を設定ファイルから読み込む機構を Phase 5 personalization で追加する
- **review 由来の Medium / Low finding**: E5 実行後に確定

## 参照

- spec: `docs/superpowers/specs/2026-04-26-p2-c-learning-cache-design.md`
- plan: `docs/superpowers/plans/2026-04-26-feature-105-p2-c-learning-cache.md`
- ADR 0014(LearningCache 設計方針、Phase B 実装根拠)
- ADR 0015(Reader/Writer ISP split、Phase A 実装根拠)
- P2-B WBS: `docs/wbs/2026-04-25-feature-98-p2-b-user-dictionary.md`
- ISSUE: https://github.com/std-koh-hinooka/kotoha-ime/issues/105
