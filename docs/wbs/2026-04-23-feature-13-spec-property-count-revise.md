---
milestone: spec-revision + M3-plan
branch: feature/13-spec-property-count-revise
pr: "#14"
merge_commit: "5ed627f"
issue: "#13"
status: done
started: 2026-04-23
finished: 2026-04-23
---

# spec §11.3 revision + M3 implementation plan commit

## 実施内容

- Spec §11.3 の「既存 3 条件(冪等性 / 結合性 / 可逆性)」を「既存 2 条件(冪等性 / 結合性)」へ修正。可逆性を除外した理由(romaji → かな は多対一であり、`ji` と `zi` の両方が `じ` に写像される等、round-trip が原理的に成立しない)を rationale paragraph として同セクションに追加
- Spec §11.3 見出しの「7 条件 → 6 条件」、§11.4 表のプロパティテスト行「7 条件 → 6 条件」、§19 changelog の「プロパティテストを 3 → 7 条件に拡張 → 2 → 6 条件に拡張」へ連動修正。§13.4 に「総数は §11.3 参照: 既存 2 + モード系 4 = 6 条件」の cross-link を追加
- M3 実装計画 `docs/superpowers/plans/2026-04-23-kotoha-phase-0-m3.md` (2398 行)を新規作成。2 PR 分割構成(M3a: rules + trie + state + facade + unit test 33 件、M3b: golden 200+ + property 2 条件 + proptest dev-dep 追加)
- Review findings 5 件を本 PR 内で修正 (commit `7a075cf`): (a) plan の pending-tails 行 `sha<TAB><TAB>sha` の self-contradiction を `fuk<TAB>ふ<TAB>k` に差し替え、(b) hatsuon `na` 重複削除と TSV ブロック consolidation、(c) idempotence property 強化(`prop_purity` → `prop_idempotence_on_committed`)、(d) test-count 算数訂正(`260+` → 60 tests + 200+ fixture rows の内訳明記)、(e) spec §13.4 cross-link 追加
- 3 commits を branch に積み、squash merge で develop に統合(base `6e042ec` → merge `5ed627f`、2 files changed、+2405 / -5)
- 既存テスト 25/25 が PASS のまま(コード変更なし、pre-push gate で検証済み)

### Branch 上の 3 commits

1. `f8c3936` — spec: revise §11.3 property test count (7 → 6, remove invertibility)
2. `aab9a5c` — docs: add M3 implementation plan (2 PRs, property test reduced to 2 conditions)
3. `7a075cf` — docs: address PR #14 review findings (plan fixture cleanup, property strengthening, cross-links)

## つまずき

1. M3 plan 起草時に sub-agent が Spec §11.3 の「可逆性」を独断で「pending-ASCII invariant(pending buffer に残る ASCII が次の入力で再処理可能)」と解釈したが、主エージェントの spot-check で「可逆性」の語義(kana → romaji → kana の round-trip 保存)と乖離していると判定し reject。ユーザーに 4 案提示(A: spec を 6 条件に revise / B: canonical romaji を定義して round-trip / C: reinterpret を採用 / D: ADR で保留)、ユーザーが A を選択
2. 主エージェントの sub-agent への指示が spec の revision-history セクションを「§24」と誤記しており、sub-agent が concern として報告。追加の sub-agent 委任で plan addendum 行 36 の「§24」を「§19」に修正
3. Architecture review で plan の pending-tails 行 `sha<TAB><TAB>sha` が self-contradicting と指摘された(`sha` は yoon rule の完全マッチで既に `しゃ` を commit するため pending-tails にはならない)。隣接コメントで誤りが認識されていたにもかかわらず fixture 本体が未修正だった。`fuk<TAB>ふ<TAB>k` に差し替えて整合させた
4. Review で「`prop_purity` が `&self` + interior mutability 不在により trivially true」と指摘された。Rust の型システムが purity を保証する以上、proptest として無価値。`prop_idempotence_on_committed` へ変更し、committed 出力を再度 convert に通しても committed が安定し pending が発生しないことを assertion する形式とした
5. Architecture review が spec §9 に pending-buffer backtrack rule が仕様化されていないと High で指摘。本 PR の scope は §11.3 修正 + plan commit のため、§9 への仕様追加は follow-up ISSUE #15 に切り出した
6. Architecture review が §11.3 rationale の「canonical romaji ADR 候補」が M7 plan / ROADMAP に tracking されていないと Medium で指摘。同じく follow-up ISSUE #16 に切り出した

## Review

Multi-dimensional review(security + architecture + testing)を `agent-teams:team-review` で並列 dispatch。

- Security: **APPROVED**(findings 無し)
- Architecture: **APPROVED_WITH_OBSERVATIONS**(High×2 / Medium×2 / Low×4)
- Testing: **APPROVED_WITH_OBSERVATIONS**(Medium×2 / Low×7)
- 指摘対応: High×1(plan pending-tails fixture self-contradiction) + Medium×2(idempotence property 強化、spec §13.4 cross-link) + 算数訂正 1(test-count 内訳明記) を commit `7a075cf` に集約。残りの High×1(spec §9 backtrack rule)と Medium×1(canonical-romaji ADR tracking)は follow-up ISSUE #15 / #16 に切り出し、Low findings は M3a 実装時の implementer judgment に委ねた

## M3a への申し送り

1. **ISSUE #15 (spec §9.1 pending-buffer backtrack rule)**: M3a-3 で state machine を実装する際に contract 化する予定。M3a 実装中に rule を具体化し、同 PR 内で spec §9.1 を追加するか、spec-only 別 PR として先行させるかを判断する
2. **M3a branch kickoff**: branch `feature/N-kotoha-romaji-core`(N は M3a ISSUE 番号)を develop から切り、plan `docs/superpowers/plans/2026-04-23-kotoha-phase-0-m3.md` の M3a-0 〜 M3a-5 を `superpowers:subagent-driven-development` で順次実行。想定 2 日
3. **M3b の前提**: M3a merge 後に branch を切る。fixture (220 cases TSV)と golden runner / 2 property tests を 1 PR で納める。想定 1 日
4. **Phase 1 への ADR tracking (ISSUE #16)**: canonical-romaji ADR は Phase 0 完了後でも許容されるが、M7 ADR 起票のタイミングで併せて検討する選択肢がある
5. **WBS ログ作成の連鎖**: M3a merge 後に `docs/wbs/2026-04-23-feature-N-kotoha-romaji-core.md` を develop へ直接 push、M3b も同様の扱い

## 成果物リンク

- PR: #14 (squash merge, merge commit `5ed627f`)
- ISSUE: #13 (Closed)
- Follow-up ISSUE: #15(spec §9.1 backtrack rule、Open)、#16(canonical-romaji ADR tracking、Open)
- Spec: `docs/superpowers/specs/2026-04-22-kotoha-phase-0-design.md`
- Plan: `docs/superpowers/plans/2026-04-23-kotoha-phase-0-m3.md`(新規)
- 影響ファイル: `docs/superpowers/specs/2026-04-22-kotoha-phase-0-design.md`, `docs/superpowers/plans/2026-04-23-kotoha-phase-0-m3.md`(新規)
