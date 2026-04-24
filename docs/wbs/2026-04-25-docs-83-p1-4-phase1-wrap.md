# P1-4 — Phase 1 最終 wrap (ADR 0009 promote + 0011/0012/0013 新規 + spec §14 完了宣言)

| 項目                | 値                                                                                         |
| ------------------- | ------------------------------------------------------------------------------------------ |
| Milestone           | P1-4 (Phase 1 最終 close)                                                                  |
| ISSUE               | #83                                                                                        |
| 親 milestone        | Phase 1 全体 (P1-0 〜 P1-4)                                                                |
| 親 plan             | `docs/superpowers/plans/2026-04-24-kotoha-phase-1-p1-2-5.md`                               |
| 親 spec             | `docs/superpowers/specs/2026-04-24-kotoha-phase-1-design.md`                               |
| ブランチ            | `docs/83-p1-4-phase1-wrap`                                                                 |
| PR                  | 本 WBS commit 後に別 session で作成予定                                                    |
| 実装起点            | `f9a820c` (develop, PR #82 merge、P1-3 完了時点)                                           |
| 性質                | 純粋 docs 変更 (Rust 実装コード / Cargo.toml / shell スクリプトには一切手を入れない)       |
| close 日            | 2026-04-25                                                                                 |

## 概要

本 P1-4 は Phase 1 の最終 wrap milestone であり、以下を docs-only の変更で確定する。

1. ADR 0009 を prep note から正式 Accepted ADR へ promote する (file rename + 本文 finalize)
2. ADR 0011 / 0012 / 0013 を新規起票する
3. spec §7.1 (§4.1 内の ADR file list) を新番号付与に沿って更新する
4. spec §14 に Phase 1 完了宣言節 §14.1 を追記する
5. ROADMAP.md の Phase 一覧表で Phase 1 を「完了」に更新する
6. ROADMAP.md の Phase 1 → Phase 2 申し送り節に P1-4 の wrap 要約を追記する

Rust 実装コード (`crates/**/*.rs`) / Cargo.toml / shell スクリプトは本 PR では一切変更しない。本 PR は「Phase 1 acceptance の明文化と ADR 整列」に徹する。

## 背景 — ADR 番号競合の経緯

Phase 1 kick-off 時点 (spec 当初) では、P1-4 で作成する ADR を「0009 / 0010 / 0011」の 3 連番で計画していた。内訳は以下であった。

- 0009: Phase 1 default model selection
- 0010: Kanji backend trait design
- 0011: Feature flag design for llama.cpp integration

しかし P1-2.5 follow-up の審議過程で Phase 5 (Kotoha custom romaji-base model) の方針が固まり、Phase 5 方針 ADR を別ブランチ (PR #78、merge commit `7e74556`) で先行 merge することとなった。Phase 5 方針 ADR は canonical numbering に従い ADR 0010 として merge され、develop の `docs/adr/0010-kotoha-custom-romaji-base-model.md` として存在している。

これにより P1-4 予定だった 3 連番のうち、0010 の番号が Phase 5 で既使用となった。P1-4 の着手時点 (2026-04-25) で ADR 0010 を書換えると Phase 5 の decision を破壊するため、P1-4 側の番号を +1 繰上げる判断を行った。本 WBS の対応表は以下となる。

| 旧 spec 計画         | 新番号                           | 内容                                                                                                                       |
| -------------------- | -------------------------------- | -------------------------------------------------------------------------------------------------------------------------- |
| 0009 (prep 昇格)     | **0009** (本 P1-4 で promote)    | ファイル rename (`0009-kanji-backend-model-selection-prep.md` → `0009-phase-1-default-model-selection.md`) + Status: Accepted |
| 0010 (backend trait) | **0011** (+1 繰上)               | Phase 5 ADR 0010 を温存するため                                                                                            |
| 0011 (feature flag)  | **0012** (+1 繰上)               | Phase 5 ADR 0010 を温存するため                                                                                            |
| (新規)               | **0013**                         | Phase 1 latency target relaxation (spec §8.3 の 30s target を empirical に見直す、本 P1-4 で新規追加)                       |

0013 は当初 spec 計画には存在しなかったが、P1-2.5 follow-up の empirical (cold ~10.6s + warm ~3s/case × 14 行 = ~52s) が spec §8.3 の 30 秒 target を超過する事実を P1-4 で closure する必要が生じたため、本 milestone で新規追加した。

## 4 ADR の要約

### ADR 0009 (promote) — Phase 1 default model selection

- prep note (`0009-kanji-backend-model-selection-prep.md`) を `0009-phase-1-default-model-selection.md` に rename し、Status: Prep → Accepted に昇格した
- Decision 6 項目: Gemma-2-2B-jpn-it Q5_K_M の default 採用 / `LlamaCppBackend` rename / plain-text completion + v12 few-shot の固定 / hiragana→katakana preprocessing を backend 側で行わない / row 3 の既知制約受容 / Zenz family を Phase 2+ の再評価候補として保持
- Alternatives に E (Gemma-4-31B upgrade → 18GB で size budget 超過により Rejected) を新規追加した
- Related documents に PR #74 / #76 / #82 の merge commit を追記し、row 3 の Phase 5 deferral を ADR 0010 と相互参照した

### ADR 0011 — Kanji backend trait design

- `KanjiBackend` trait + `BackendConfig` enum + `PromptTemplate` enum + `load_backend` factory の設計意図を記録する
- Decision 5 項目: trait 定義 / `#[non_exhaustive]` enum / factory による dispatch / `Box<dyn Backend>` を enum dispatch より優先する選択 / `validate_input` と `score_sort_dedupe` の `pub(crate)` helper 共有
- Alternatives A (concrete struct + `#[cfg(feature)]` 排他切替) / B (enum dispatch) / C (factory 内 match のみ、enum を設けない) を棄却した
- Phase 5 の `KotohaNative` backend 追加 (ADR 0010 D6) が BackendConfig 新 variant + trait 実装のみで完了する拡張性を保証する

### ADR 0012 — Feature flag design for llama.cpp integration

- `kotoha-core` に 2 feature (`llama-cpp` / `llama-cpp-smoke`)、`kotoha-cli` に 1 feature (`llama-cpp`) を配置した設計意図を記録する
- Decision 5 項目: kotoha-core の 2 feature / kotoha-cli の feature propagation / `[[bin]] kotoha-kanji` の `required-features` / Layer 3 smoke の `#[cfg(feature = "llama-cpp-smoke")]` gate + 環境変数による SKIP / `default = []` 厳守
- Alternatives A (単一 `llama` feature) / B (binary を別 crate に切り出す) / C (always-on llama-cpp) を棄却した
- lefthook pre-push を C++ toolchain 不要で成立させ、新 contributor の onboarding (clone 直後の `cargo build --workspace` 成功) を両立する

### ADR 0013 — Phase 1 latency target (relax)

- spec §8.3 の「30 秒」target を empirical 値 (cold ~10.6s + warm ~3s/case) に合わせて relax する consensus を記録する
- Decision 4 項目: 新 target の文言 (cold ≤ 15s / warm ≤ 5s/case 目安) / pre-push に smoke を含めない方針の維持 / Phase 2+ で必要に応じ新 ADR 起票 / Phase 1 は hard-gate 無しで close
- Alternatives A (30s 厳守 + 軽量モデル差替) / B (target 完全撤廃) / C (5 cases に縮小して 30s 維持) を棄却した
- Phase 5 (Kotoha custom 90M〜180M model、≤ 200MB、p50 ≤ 30ms、ADR 0010 D6 / Phase 5 spec §6.4) 完了時点で latency が大幅改善される期待を bridge として明記した

## spec / ROADMAP の更新要約

### spec §4.1 の ADR file list 更新

crate layout ASCII tree 末尾の `docs/adr/` 配下一覧を、旧 3 行 (0009 / 0010 / 0011) から新 5 行 (0009 / 0010 / 0011 / 0012 / 0013) に書換えた。各行に「P1-4 で作成」「Phase 5 方針、PR #78」「+1 繰上げ」「新規」のコメントを付与し、ADR 番号競合の経緯が spec から辿れるようにした。

### spec §14.1 新規節

§14 Acceptance 表 15 項目の直後に §14.1 「Phase 1 完了宣言 (2026-04-25 P1-4)」を新設した。P1-0 〜 P1-4 の全 milestone 達成を時系列で記録し、完了条件を「§14 の 15 項目が (a) 達成、または (b) ADR で defer が明文化 (row 3 → ADR 0010、latency → ADR 0013)」と明記した上で、Phase 2 kick-off 可能宣言を記述した。

### ROADMAP.md の Phase 1 行

Phase 一覧表の Phase 1 行の状態列を「進行中 (P1-2.5 follow-up 完了)」から「完了 (14/15 PASS, P1-4 で close 2026-04-25)」に更新した。

### ROADMAP.md の Phase 1 → Phase 2 申し送り節

既存の申し送り節 (PR #78 で追記済、row 3 + Phase 5 deferral を記述) の末尾に、P1-4 の wrap 要約 (ADR 0009 promote / 0011 / 0012 / 0013 新規 / spec §14.1) と、Phase 2 着手時に Gemma 14/15 baseline + Backend trait の拡張点を前提とする方針を 1 段落で追記した。

## Phase 1 P1-0 〜 P1-4 全体の総括

Phase 1 の全 milestone を時系列と対応 PR で記録する。

| Milestone             | 成果物 / 状態                                                                                                                      | PR      | merge commit |
| --------------------- | ---------------------------------------------------------------------------------------------------------------------------------- | ------- | ------------ |
| P1-0 (Phase 0 継承)   | `scripts/lib/assert.sh` を Phase 0 で整備済。Phase 1 smoke でも共有                                                               | Phase 0 | (既存)       |
| P1-1                  | `kotoha-core::kanji` module、`KanjiBackend` trait、`Candidate` / `ConvertOptions` / `BackendConfig` / `KanjiError`、`MockBackend`、`load_backend` factory、Layer 1 + Layer 2 test | #66 相当 | (Phase 1 初期) |
| P1-2                  | 初期 `ZenzBackend` (後に P1-2.5 で rename)                                                                                          | #70 相当 | (P1-2 段階) |
| P1-2.5                | `LlamaCppBackend` 汎用化 + Gemma-2-2B-jpn-it 採用 + `PromptTemplate` enum 導入 + 9/9 PASS                                            | #74     | `02cf035`   |
| P1-2.5 follow-up      | Layer 3 15 行復元 + v12 prompt 採用 + plain-text completion pivot + 14/15 PASS (row 3 を Phase 5 deferral)                          | #76     | `3eccaa1`   |
| P1-3                  | `kotoha-cli::bin::kotoha-kanji` バイナリ + `process_line` 純粋関数 + `scripts/phase1-smoke.sh` + Layer 4 E2E smoke                   | #82     | `f9a820c`   |
| P1-4 (本 WBS)         | ADR 0009 promote + ADR 0011 / 0012 / 0013 新規 + spec §14.1 + ROADMAP update                                                        | (本 PR) | (未作成)    |

## Phase 2 への申し送り

Phase 2 (Dictionary and learning) 着手時には以下を前提とする。

- **Gemma-2-2B-jpn-it Q5_K_M 14/15 baseline**: Layer 3 smoke の quality baseline は 14/15。row 3「あした → 翌日」は Phase 5 task-specific fine-tune で根本解決するまで Phase 1〜Phase 4 共通の既知制約として扱う
- **Backend trait の拡張点**: 辞書検索 / 学習キャッシュを取り込む新 backend を追加する際は、`BackendConfig` に新 variant (`Dictionary` / `LearningCache` 等) を追加し、`KanjiBackend` trait を実装する struct を新設する構造を踏襲する。既存 `LlamaCppBackend` / `MockBackend` は変更しない
- **feature flag の拡張**: 辞書 / 学習関連の外部依存 (例: Sudachi 辞書 crate、SQLite) も ADR 0012 の方針に従い optional dependency として `dict` / `learning` 等の feature 下に隔離し、default build を軽量に保つ
- **latency 目標**: Phase 2 の範囲では latency を hard-gate しない (ADR 0013 D3 に従い、Phase 3 IBus 統合時点で新 ADR を起こす)
- **spec 追記方針**: Phase 2 の新機能追加が Phase 1 spec §14 の acceptance checklist に影響する場合は、Phase 2 spec で独立の acceptance 表を設け、Phase 1 spec §14 を上書きしない

## Open Question の close 宣言

| Q                                             | Close 判断                                                                                          |
| --------------------------------------------- | --------------------------------------------------------------------------------------------------- |
| Phase 1 の ADR 番号割当                      | ADR 0009 (promote) / 0011 / 0012 / 0013 の 4 本で close。Phase 5 ADR 0010 は温存し競合を回避        |
| spec §8.3 の 30s latency target 達成可否       | 未達成 (empirical ~52s)。ADR 0013 で relaxation を明文化して close、Phase 1 acceptance は blocking しない |
| row 3「あした → 明日」の Phase 1 での解消可否 | 解消不能と確定。ADR 0009 D5 で既知制約受容、Phase 5 (ADR 0010) で根本解決する deferment として close |

## 検証 commands (final、本 WBS ブランチで実行)

本 P1-4 は docs-only のため、Rust 側の検証は「変更が無いことの確認」として `cargo check --workspace` を 1 回走らせる。実装 binary / test の再実行は不要 (Phase 1 P1-3 で既に 14/15 を確認済、本 PR では Rust ファイル 0 行変更)。

```bash
cargo check --workspace                           # PASS (docs-only のため no-op)
ls docs/adr/0009-phase-1-default-model-selection.md       # 存在確認
ls docs/adr/0011-kanji-backend-trait-design.md             # 存在確認
ls docs/adr/0012-feature-flag-design-for-llama-cpp.md      # 存在確認
ls docs/adr/0013-phase-1-latency-target.md                 # 存在確認
test ! -f docs/adr/0009-kanji-backend-model-selection-prep.md  # 旧 prep が存在しないことを確認
```

## Follow-up (post-merge)

本 P1-4 で残す作業 (別 ISSUE として起票予定):

1. **README.md の Phase 1 section 仕上げ**: Gemma-2-2B-jpn-it の入手手順 + `kotoha-kanji --model` の使用例 + row 3 既知制約の注記を README に追記する (本 P1-4 の scope からは切り離す。別 ISSUE で短時間 PR として起票予定)
2. **Phase 2 spec の着手**: Phase 2 (Dictionary and learning) の設計書を `docs/superpowers/specs/2026-04-25-kotoha-phase-2-design.md` として起票する (ADR 0011 の Backend trait 拡張 + ADR 0012 の feature flag 方針を前提にする)
3. **smoke 実測値の CI 外定期計測**: Phase 2 以降の regression 監視のため、WBS に smoke 実測値を定期記録する運用を検討する (ADR 0013 負の帰結への対応)

Phase 1 は本 P1-4 をもって close 状態となる。後続は Phase 2 (Dictionary and learning) kick-off へ移行する。
