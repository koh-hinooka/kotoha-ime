---
feature: feature-91-p2-a-dictionary-layer
status: implemented
bounded_context: _uncategorized
related_issues: ["#91"]
related_prs: []
glossary_refs: ["hiragana","kotoha-dict","mock-backend","romaji","sudachipy"]
last_reviewed: 2026-05-05
---

# P2-A WBS — Dictionary Layer Implementation Log

> **Migration note**: 本 spec は `docs/wbs/2026-04-25-feature-91-p2-a-dictionary-layer.md` から spec 化 (Global CLAUDE.md §Documentation Structure 準拠の `docs/wbs/` 廃止対応、2026-05-05)。
> 元 WBS は実装ログ性質。14-section restructure は別 follow-up 起票予定 (各 project の既存 14-section follow-up issue と合流可能)。

## 元 WBS 内容 (実装ログ由来)

title: P2-A (Phase 2-A: Dictionary layer) WBS — 実装ログと reflection
date: 2026-04-25
phase: 2
milestone: P2-A
status: completed (merged via #93)
issue: 91
follow_up_issues: [92, 94]
---

# P2-A WBS — Dictionary Layer Implementation Log

## 1. 概要

P2-A は Phase 2「Dictionary and learning」の最初の milestone である。Sudachi-based Dictionary backend(`KanjiBackend` の 2 つ目の実装)を新設し、`MorphologicalEngine` / `VocabularyLookup` の 2 trait を Clean Architecture DIP に基づき先出しした。`BackendConfig::Dictionary` variant を `#[non_exhaustive]` enum に追加(ADR 0011 拡張点活用)、ADR 0014 D4 を「Phase 2 で 2 variants 追加」へ改訂した。

実装範囲:

- 6 module(`dict/{mod,backend,engine,vocab,sudachi_adapter,custom_vocab}.rs`)
- `dict` / `dict-smoke` Cargo features(default = []、ADR 0012 D5)
- `sudachi.rs` git rev pin(`90fd6068c80c` = v0.6.11、Apache-2.0)
- Layer 1 unit tests + Layer 2 integration tests(計 47 tests 新規)
- Python tool `tools/p2a-fixture-gen/`(530-case fixture 生成、530 cases 生成は #92 で実施)
- ADR / spec / README / glossary 同期更新

Layer 3(530-case golden fixture / golden test runner / phase2-smoke.sh)は P5-A sample data の schema mismatch(romaji vs hiragana)+ 規模不足(~100 unique pairs)で **#92 へ deferred**。

## 2. 到達点(15 commits + 2 review fix commits = 17 commits)

- PR #93 squash merge(merge commit `1dd52059`)
- ISSUE #91 自動 close(`Closes #91` 経由)
- 17 commits(plan / spec 2 + 実装 13 + review fix 2)
- 規模: 5017 insertions / 30 files(target: 1000/20、超過率 5x)
  - 内訳: docs(spec/plan/ADR/glossary/README)3284 行 / 6 files、Rust 実装+test 1016 行 / 8 files、Python tools 473 行 / 10 files、Cargo + lock + README 残り
- Test 数: default 175 PASS(Phase 1 baseline 維持)、`mock-backend,dict` 223 PASS(+47 P2-A 新規)、Python pytest 6 PASS

## 3. 実装フローの実績

doc-driven development の trace を以下に示す:

| stage | output | line count | ref |
|---|---|---|---|
| brainstorming | 11 design decisions(Q1-Q11) | session conversation | superpowers:brainstorming skill 起動 |
| design spec | `docs/superpowers/specs/2026-04-25-p2-a-dictionary-layer-design.md` | 622 行 | commit `ba2ce9e` |
| implementation plan | `docs/superpowers/plans/2026-04-25-feature-91-p2-a-dictionary-layer.md` | 2548 行 / 18 tasks | commit `e7fe4a3` |
| 実装 commits(Tasks 1-12, 16-17, 18) | dict module + tests + Python tool + docs sync | 13 atomic commits + 2 review-fix commits | `5a57270` → `2b02fb0` |
| review iteration | 5 skill(team-review / owasp-security / secrets-check / security-sast / pr-review-toolkit:review-pr) | 33 findings | review session |
| review fix | Critical 1 + ROI 4 を 2 commit に集約 | 80 行追加 | `15cb466` + `2b02fb0` |
| merge | squash merge with title `feat(phase-2): P2-A — Dictionary layer kick-off (#91) (#93)` | — | merge commit `1dd52059` |

実装 task 18 件のうち **3 件(13/14/15、Layer 3)を mid-execution で defer** した(ISSUE #92 へ切出し)。原因は P5-A `sample.tsv` の schema mismatch(`clean_romaji` は romaji であり hiragana ではない)+ unique pair 不足が判明したためである。

## 4. レビュー findings の triage 結果

| 対応先 | 件数 | 主な内容 |
|---|---|---|
| 本 PR 追加 commit(Critical) | 1 | stale `#[allow(dead_code)]` 9 箇所削除 + 真 dead `from_str` を `#[cfg(test)]` 化 |
| 本 PR 追加 commit(高 ROI) | 4 | `ModelNotFound { path: "" }` actionability 改善 / placeholder no-op test 削除 / Layer 2 duplicate test 削除 / NaN/Infinity score guard + log injection 対策 |
| #92(Layer 3 follow-up) | 2 | dict-smoke dead state 関連 |
| #94(post-merge follow-up) | 28 | P2-B / P2-D / Phase 5 / Phase 2 wrap に分配 |

合計 35 findings 処理、blocking 0(immediate fix で解消済)、deferred 30(ISSUE 経由 track)。

## 5. Reflection — 良かった点

- **doc-driven development が機能**: brainstorming(11 決定)→ spec(622 行)→ plan(2548 行 / 18 tasks)→ 実装(15 atomic commits)→ review(33 findings)→ fix → merge の直線フローで重大な逆行ゼロを達成した。各 stage の output が次 stage の input として正常に機能した。
- **subagent-driven-development**: 18 task の sequential dispatch で 9 sub-agent 起動、各 task ごとに pre-push gate を通過する atomic commit を生産した。memory pressure(6.7-6.4GB available)で並列限界に達しなかった。
- **Critical の早期発見**: review skill 5 本のうち team-review(testing dimension)が stale `#[allow(dead_code)]` 9 箇所を発見、pr-review-toolkit が「実機検証で 1 件は真 dead」と判定した。fix 前に発見できた。
- **Layer 3 deferral の判断**: Task 12 で fixture-gen 実装中に schema mismatch が判明、即座に user に escalation して D(deferral to #92)を選択した。scope 膨張を防いだ。
- **Engine-neutral naming の徹底**: `MorphologicalEngine` / `VocabularyLookup` trait + `system_dict_path` field + `KOTOHA_SYSTEM_DICT_PATH` env var が Adapter 原則に整合(spec §3.4 Q4)、Phase 5 KotohaNative 統合への継承可能性を構造的に確保した。

## 6. Reflection — 改善点

- **Branch Scope Policy 5x 超過**: 本 PR は target 1000/20 を 5017/30 で大幅超過した。docs(spec 622 + plan 2548 + glossary 30 + ADR + README)が大半を占めた。**次回の phase-foundation PR は docs-only PR(#87 前例)+ code-only PR に必ず分離する**。本 reflection を WBS で formalize する。
- **Layer 3 fixture source の前提検証不足**: brainstorming 段階で「P5-A sample.tsv の schema が hiragana である」と暗黙に仮定したが、実態は romaji だった。設計 stage で 1 度 sample.tsv の冒頭数行を確認すべきだった。次回 fixture-gen 設計時には source data の column schema を spec に明記する。
- **review skill の sequential 実行コスト**: 5 skill(team-review 5 dim parallel + owasp + secrets-check + sast + pr-review-toolkit)で合計 ~30 分。team-review 5 reviewer は parallel で 1 skill 内に収まったが、残り 4 skill は sequential。memory 余裕があれば一部 parallel 化を検討する。
- **`#[allow(dead_code)]` cleanup の自動化不在**: Tasks 3-7 で trait/struct に `#[allow(dead_code)]` を Task 8 までの一時 marker として付加したが、Task 8 完了時の自動削除 step が plan に無かった。次回 plan template に「stale allow scan」step の追加を検討する。
- **Critical fix を本 PR で吸収する判断の遅れ**: pr-review-toolkit が「実機検証で 1 真 dead `from_str`」と発見するまで Critical の重大性に気づけなかった。team-review 段階で実機検証(strip + clippy)があれば早期判明していた。

## 7. follow-up tracker

- **#92**: Layer 3 fixture + golden test runner + phase2-smoke.sh(romaji→hiragana 変換 or sudachipy 直接 query で再構築)
- **#94**: P2-A post-merge triage(28 findings、P2-B/P2-D/Phase 5/Phase 2 wrap に分配)
- **次 milestone**: P2-B(User dict、TOML/JSONL 永続化、`kotoha-dict` CLI subcommand draft)
  - P2-B 開始時に #94 の P2-B-tagged items(security/supply-chain hardening)を fix priority 上位にする
- **Phase 2 wrap 時**: ADR 0014 に Threat Model section 追加、SBOM tooling visibility 改善(#94)

## 8. 参照

- 上位 spec: `docs/superpowers/specs/2026-04-25-kotoha-phase-2-design.md`
- 子 spec: `docs/superpowers/specs/2026-04-25-p2-a-dictionary-layer-design.md`
- plan: `docs/superpowers/plans/2026-04-25-feature-91-p2-a-dictionary-layer.md`
- ADR 0014: `docs/adr/0014-phase-2-dictionary-layer-architecture.md`(D4 改訂済)
- ADR 0011 / 0012(`#[non_exhaustive]` enum / feature flag default 方針)
- Glossary: `docs/wiki/glossary.md`(MorphologicalEngine / VocabularyLookup / SudachiDict / 形態素解析 4 用語追加済)
- ROADMAP: `docs/ROADMAP.md`(Phase 2 マイルストーン分割節)
- Phase 5 ADR 0010(KotohaNative integration)
