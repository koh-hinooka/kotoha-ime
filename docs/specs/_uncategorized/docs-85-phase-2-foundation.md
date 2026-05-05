---
feature: docs-85-phase-2-foundation
status: implemented
bounded_context: _uncategorized
related_issues: ["#85"]
related_prs: []
glossary_refs: ["backend-trait","candidate","gemma-2-2b-jpn-it","learning-cache","lefthook","row-3","system-dictionary","user-dictionary"]
last_reviewed: 2026-05-05
---

# Phase 2 foundation — ADR 0014 + Phase 2 spec draft + ROADMAP P2-A〜D (#85)

> **Migration note**: 本 spec は `docs/wbs/2026-04-25-docs-85-phase-2-foundation.md` から spec 化 (Global CLAUDE.md §Documentation Structure 準拠の `docs/wbs/` 廃止対応、2026-05-05)。
> 元 WBS は実装ログ性質。14-section restructure は別 follow-up 起票予定 (各 project の既存 14-section follow-up issue と合流可能)。

## 元 WBS 内容 (実装ログ由来)


| 項目         | 値                                                                                         |
| ------------ | ------------------------------------------------------------------------------------------ |
| Milestone    | Phase 2 foundation (P2 kick-off 前の docs 整備)                                            |
| ISSUE        | #85                                                                                        |
| 親 milestone | Phase 2 全体 (P2-A 〜 P2-D)                                                                |
| 親 spec      | `docs/superpowers/specs/2026-04-25-kotoha-phase-2-design.md` (本 PR で新規起票)             |
| 親 ADR       | `docs/adr/0014-phase-2-dictionary-layer-architecture.md` (本 PR で新規起票)                 |
| ブランチ     | `docs/85-phase-2-foundation`                                                               |
| PR           | 本 WBS commit 後に別 session で作成予定                                                    |
| 実装起点     | `95e7df9` (develop, PR #84 merge、Phase 1 close 時点)                                      |
| 性質         | 純粋 docs 変更 (Rust 実装コード / Cargo.toml / shell スクリプトには一切手を入れない)       |
| 着手日       | 2026-04-25                                                                                 |
| 完了日       | 2026-04-25                                                                                 |
| 工数         | 半日 (docs-only の foundation 整備)                                                        |

## 背景 — Phase 1 close → Phase 2 方針確定の timing

Phase 1 は P1-4 (PR #84, merge `95e7df9`) で close した。P1-4 の申し送り (ROADMAP の「Phase 1 → Phase 2 申し送り」節、および WBS `2026-04-25-docs-83-p1-4-phase1-wrap.md` §「Phase 2 への申し送り」) では、Phase 2 着手の前提として以下 5 点を明示した。

1. Gemma-2-2B-jpn-it Q5_K_M 14/15 baseline を Phase 1 の品質基準として維持する
2. ADR 0011 で確定した Backend trait の `#[non_exhaustive]` 拡張点を Phase 2 で消費する
3. ADR 0012 の feature flag 方針を Phase 2 の新規依存 (SudachiDict 等) にも適用する
4. ADR 0013 の latency policy (Phase 2 では hard-gate しない) を継続する
5. Phase 2 の新機能は Phase 1 spec §14 の acceptance checklist を上書きせず、独立の acceptance 表を設ける

本 ISSUE #85 は Phase 2 の実装開始前に、上記 5 前提を反映した foundation docs (ADR + spec + ROADMAP + WBS) を整備することを目的とする。Phase 5 foundation (ISSUE #77, PR #78, ADR 0010 + Phase 5 spec stub) の先行 pattern を踏襲し、方針が確定している範囲で ADR と spec draft を先行起票し、実装詳細 (パラメータ / variant 名 / format) は P2-A kick-off で empirical 確定する。

## 4 文書の要約

### ADR 0014 — Phase 2 dictionary layer architecture (Sudachi-based hybrid)

本 ADR は Phase 2 の方針を以下 6 項目の Decision で確定する。

- **D1** — Sudachi hybrid architecture (LLM + Dictionary) を Phase 2 default とする
- **D2** — Dictionary を System + User の 2 層で構成する
- **D3** — Learning cache を in-memory LRU + 永続化ペアで持つ
- **D4** — `BackendConfig` に新 variant を追加する (variant 名は P2-A kick-off で `DictionaryAugmented` / `Hybrid` の 2 案から empirical 確定)
- **D5** — Ranker 初期重みを dict 0.95 / LLM 1.0 として P2-D で empirical tuning する
- **D6** — 既存 Backend trait を壊さず拡張する (ADR 0011 との整合、Phase 5 `KotohaNative` への継承パス確保)

### Phase 2 spec draft — Dictionary and learning

本 spec は 10 節構成で Phase 2 全体の設計を draft する。stub disclaimer「詳細は P2-A kick-off で確定する」を各節末尾に付し、方針のみ確定し詳細は P2-A で empirical 詰めする運用を明示する。

- §1 背景と動機 (14/15 baseline / Phase 5 3〜6 ヶ月の底上げ需要 / 固有語の実例 3 類型)
- §2 スコープ (In-scope 5 項目 / Out-of-scope 4 項目)
- §3 アーキテクチャ (Dictionary layer / Learning cache / Ranker / BackendConfig 拡張)
- §4 Dictionary 設計 (core vs full / Kotoha 独自語彙 merge / bundling vs runtime download / dict update policy)
- §5 Learning cache 設計 (data model / persistence / load-save timing / eviction / pruning)
- §6 API と Trait 拡張 (Backend trait 非破壊拡張 / ConvertOptions 拡張 / crate 配置)
- §7 テスト戦略 (unit / integration / golden fixture 30+ cases / Phase 1 14/15 regression)
- §8 非スコープ (streaming / personalization ML / sync / GUI editor)
- §9 Open questions (Q1〜Q6、Phase 5 integration plan は Q6 で明示)
- §10 参照

### ROADMAP restructure

ROADMAP.md に以下 3 項目を反映する。

1. Phase 一覧表の Phase 2 行の状態列を「未着手」から「**進行中 (foundation docs 完了)**」に更新
2. Phase 2 マイルストーン分割節 (P2-A / P2-B / P2-C / P2-D) を Phase 5 マイルストーン節の直前に新設
3. 「Phase 1 → Phase 2 への申し送り」節の末尾に 1 段落追記 (foundation docs 整備の完了と Phase 5 integration plan (§9 Q6) の参照)

### WBS (本 docs ファイル)

本 WBS は Phase 2 foundation docs 整備の実装ログである。Phase 5 foundation WBS の style を踏襲し、ADR 0014 の決定経緯と Phase 2 spec stub 方針を Phase 5 WBS と同じ密度で記録する。

## ADR 0014 の Decision / Alternatives の決定経緯

### Decision D1 (Sudachi hybrid architecture) の根拠

Phase 1 P1-2.5 follow-up (PR #76 merge `3eccaa1`) で 14/15 baseline が確定し、row 3 を除く 14 行は Gemma-2-2B-jpn-it で十分な recall を達成した。Phase 2 の目的は「14/15 を退行させない前提で固有名詞 / 敬称 / User 個別語彙の recall を底上げすること」であり、LLM + Dictionary の hybrid 構成が最小工数で目的を達成する。Phase 5 完成 (3〜6 ヶ月) を待たずに shipping 品質を底上げする必要がある (ADR 0014 C2) ため、Phase 2 での hybrid 採用を決定した。

### Decision D4 (BackendConfig 新 variant) の 2 候補保留

P2-A kick-off で確定する 2 案は以下のとおり。

- **候補 1 (集約型)**: `BackendConfig::DictionaryAugmented { model_path, dict_config, learning_config }` — variant 1 つに集約、実装量最小
- **候補 2 (再帰 wrap 型)**: `BackendConfig::Hybrid { llm: Box<BackendConfig>, dict, learning }` — LLM backend を再帰 wrap、Phase 5 `KotohaNative` との組合せにも自動対応

本 PR (ISSUE #85) では 2 案のどちらも「Phase 1 の既存 variant を破壊しない」「ADR 0011 の `#[non_exhaustive]` 拡張点を使用する」の 2 点は共通であるため、詳細 variant 名は P2-A kick-off の実装 pilot で empirical 確定する方針を採った。

### Decision D5 (Ranker 初期重み dict 0.95 / LLM 1.0) の根拠

Phase 1 の 14/15 baseline を与えた LLM 候補を優先 (base weight 1.0) とし、Dictionary 候補をやや低い base weight (0.95) で merge する。敬称 / 固有名詞の hit に対しては Learning cache hit bonus で rerank を逆転させる余地を残す。初期重みは P2-D の golden fixture (§7.3) で empirical tuning する。

### Alternatives A / B / C / D の rejection 根拠

- **A (dict only)**: 汎用語彙 recall の低下により Phase 1 14/15 を維持できない (ADR 0014 Alternatives A 参照)
- **B (LLM only 継続)**: 固有名詞 / 敬称の recall 改善が Phase 5 完成までの 3〜6 ヶ月待たされ、Phase 3 / Phase 4 の shipping 品質が底上げされない (Alternatives B 参照)
- **C (mozc-rs 呼出し)**: Kotoha の Backend trait 設計 (ADR 0011) との interface 不整合、および Phase 5 `KotohaNative` への継承パス断絶 (Alternatives C 参照)
- **D (独自 dict format only)**: SudachiDict の 20 万語規模を再実装する工数が Phase 2 の 3〜6 週の目安を超過する + Phase 5 P5-A PoC の辞書整合が取れない (Alternatives D 参照)

## Phase 2 spec stub の構造と stub disclaimer 方針

Phase 5 spec stub (`2026-04-25-kotoha-phase-5-custom-model.md`) の先行 pattern を踏襲し、以下の方針を採った。

1. **10 節構成**: 背景 / スコープ / アーキテクチャ / Dictionary 設計 / Learning cache 設計 / API と Trait 拡張 / テスト戦略 / 非スコープ / Open questions / 参照 の 10 節で構成
2. **stub disclaimer**: 各節末尾に「詳細は P2-A kick-off で確定する」を付し、P2-A 実装時点で確定すべき項目を明示
3. **Open questions の Q6**: Phase 5 integration plan を明示的に spec §9 Q6 として記録し、Phase 5 spec §3.3 (推論統合概観) と相互参照
4. **regression test の hard-gate**: §7.4 で Phase 1 14/15 baseline を Phase 2 backend で退行させない gate を明示 (ADR 0013 の latency policy とは独立の quality gate)

spec draft の行数は約 350 行、Phase 5 spec stub (348 行) と同等 size を意図した。

## ROADMAP restructure の要点

- **Phase 一覧表**: Phase 2 行の状態列を「未着手」→「**進行中 (foundation docs 完了)**」に更新した。「進行中」の理由は、実装 milestone (P2-A〜P2-D) は未着手であるものの、方針 ADR + spec draft + milestone 分割が確定したため、Phase 2 は kick-off の前段に入った状態であることを表す
- **Phase 2 マイルストーン節の位置**: Phase 5 マイルストーン節の直前に配置した。ROADMAP を上から読むと Phase 番号昇順で milestone 分割が追える構成となる
- **Phase 2 マイルストーン 4 分割**: P2-A (Dictionary layer, 1〜2 週) / P2-B (User dictionary, 3〜5 日) / P2-C (Learning cache, 3〜5 日) / P2-D (Integration + rerank, 1〜2 週)。全体で 3〜6 週を見込む
- **申し送り節の追記**: P1-4 → P2 申し送り節の末尾に 1 段落追加。ADR 0014 / Phase 2 spec draft / Phase 5 との integration plan (§9 Q6) の 3 点を明示

## Phase 5 (ADR 0010) との relation

Phase 5 は Kotoha 専用 romaji-base model (90〜180M parameter, ≤ 200MB, Q5_K_M) を自作する方針 (ADR 0010 D6 / Phase 5 spec §3.3)。Phase 2 の Dictionary / Learning cache / Ranker を Phase 5 で継承する余地を、以下 3 点で ADR 0014 / Phase 2 spec に明示した。

1. **ADR 0014 D6**: 既存 Backend trait を壊さず拡張する。Ranker と Learning cache は backend-agnostic な形で kotoha-core crate 内に配置する
2. **Phase 2 spec §9 Q6**: Phase 5 integration plan を open question として保留し、Phase 5 kick-off で継承可否を確定する
3. **Phase 5 spec §3.3 (既存)**: Phase 5 の推論統合概観に「Sudachi 辞書 fallback を OOV 検知時の後段として併用する」と記述済 (Phase 5 spec §3.4)。Phase 2 で Dictionary / Learning cache を Phase 5 で継承可能な形に配置しておくことで、Phase 5 spec §3.4 の「候補 a / b / c の後段 fallback 案」と Phase 2 の architecture が整合する

Phase 5 は Phase 2 の Dictionary / Learning 層を継承 (backend 交換のみ) する方針を第一候補とするが、Phase 5 kick-off 時点で custom model の data interface が Phase 2 と大きく異なる場合は別 backend として独立構築する余地も残す。

## P2-A kick-off への申し送り

Phase 2 foundation docs の完了時点で、P2-A kick-off 時に empirical 確定する open question を以下にまとめる (Phase 2 spec §9 と同一表、本 WBS で再掲)。

| # | Question | 確定タイミング |
|---|---|---|
| Q1 | SudachiDict-core (70MB) vs full (500MB) | P2-A kick-off 初週 (pilot recall 比較) |
| Q2 | `BackendConfig` 新 variant 名 (`DictionaryAugmented` / `Hybrid`) | P2-A kick-off で確定 |
| Q3 | Learning cache 永続化 format (TOML / JSONL / TSV) | P2-A kick-off で確定 |
| Q4 | Ranker 重み defaults の最終値 | P2-D で golden fixture の pass rate で確定 |
| Q5 | Dictionary load timing (compile-time 埋込 vs runtime load) | P2-A kick-off で確定 (default: runtime load) |
| Q6 | Phase 5 `KotohaNative` の Phase 2 層継承可否 | Phase 5 kick-off で確定 (Phase 2 では継承可能な配置を保つ) |

加えて、本 ISSUE #85 の scope 外として P2-A kick-off 時に別 commit で対応する項目:

1. **glossary.md の新規用語追記**: DictionaryBackend / DictionaryAugmented / System dictionary / User dictionary / Learning cache / Ranker / Reranker / Candidate merge / dedupe の 8 用語程度を Phase 2 kick-off 時に glossary §4〜§7 に追加する
2. **Phase 2 plan 文書**: `docs/superpowers/plans/2026-04-25-kotoha-phase-2-p2-a.md` 相当の implementation plan を P2-A kick-off で別 ISSUE として起票する
3. **Phase 2 fixture の具体 case 収集**: §7.3 golden fixture の敬称 / 固有名詞 / 外来語 30+ cases を P2-A kick-off で確定する

本 Phase 2 foundation docs は上記 3 項目を「本 ISSUE #85 の scope 外」として明示的に保留し、P2-A kick-off で別 ISSUE / 別 commit で対応する運用を取る。

## 検証 commands (final、本 WBS ブランチで実行)

本 ISSUE #85 は docs-only のため、Rust 側の検証は「変更が無いことの確認」として `cargo check --workspace` を 1 回走らせる。

```bash
cargo check --workspace                                                    # PASS (docs-only のため no-op)
ls docs/adr/0014-phase-2-dictionary-layer-architecture.md                  # 存在確認
ls docs/superpowers/specs/2026-04-25-kotoha-phase-2-design.md              # 存在確認
ls docs/wbs/2026-04-25-docs-85-phase-2-foundation.md                       # 存在確認
```

lefthook pre-commit (doc-naming 規約) が本 PR 内の 3 新規ファイルを PASS することを commit 時点で確認する (`--no-verify` は使用禁止)。

## Follow-up (post-merge)

本 ISSUE #85 で残す作業 (P2-A kick-off 時に別 ISSUE として起票予定):

1. **glossary.md への Phase 2 新規用語追記**: 本 ISSUE scope 外、P2-A kick-off 時に別 commit で対応
2. **Phase 2 plan 文書起票**: `docs/superpowers/plans/2026-04-25-kotoha-phase-2-p2-a.md` 相当の implementation plan
3. **Phase 2 fixture の case 収集**: §7.3 golden fixture 30+ cases の具体化
4. **SudachiDict dependency 選定**: Cargo.toml への追加と feature flag (`dict` 等) の設計 — ADR 0012 の方針に従い optional dependency として隔離する

Phase 2 foundation は本 ISSUE #85 をもって完了し、後続は Phase 2 P2-A kick-off (Dictionary layer 実装) へ移行する。
