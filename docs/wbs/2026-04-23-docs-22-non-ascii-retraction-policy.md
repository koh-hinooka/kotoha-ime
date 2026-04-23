---
milestone: hotfix-docs
branch: docs/22-non-ascii-retraction-policy
pr: "#28"
merge_commit: "6b638ba"
issue: "#22"
status: done
started: 2026-04-23
finished: 2026-04-23
---

# #22 ADR: 非 ASCII 入力に対する StateMachine::push の normative policy pin

## 背景

M3b (PR #24) の property test 実装中に、`RomajiConverter::convert` の非 ASCII 入力挙動が normatively pin されていないことが判明 (ISSUE #22)。

具体的な不整合: `convert("[")` は `("「", "")` を返すが、`convert("「")` は `("", "")` を返す (非 ASCII は `StateMachine::push` の `if !ch.is_ascii()` ガードで `PushResult::Invalid(ch)` として drop されるため)。このため「`convert(convert(x).committed).committed == convert(x).committed`」という strict retraction 性は成立しない。M3b plan 初稿は「drop または pass-through のいずれか」と両論併記していたが、実装は drop 採用。property `prop_idempotence_on_committed` は strict 形では破綻するため weakened 形 (pending のみ) で暫定実装されていた。

## 実施内容

### 決定

**drop as Invalid** (現行挙動) を normative として pin。

### 検討した代替案

1. **drop as Invalid** (採用): ASCII ローマ字契約に沿う fail-fast 設計、Mozc/Google IME/MS-IME と一致、実装単純。
2. pass-through unchanged: strict retraction 成立するが境界混入バグを隠蔽しうる、新 PushResult variant 必要。
3. 上位層フィルタ: push の契約純度は保たれるが API 複雑性増加、Phase 0 ユースケース不要。

### 成果物

- **NEW**: `docs/adr/0001-non-ascii-retraction-policy.md` (85 lines) — 3 alternatives 比較 + 決定 + 影響分析。
- **MODIFY**: `docs/superpowers/specs/2026-04-22-kotoha-phase-0-design.md` §9.2 新設 (10 lines) — normative 記述 + ADR 0001 への参照。

### コミット

1. `1e831cd` — ADR 0001 + spec §9.2 初版
2. `3eeb9a7` — review minor fix (line-range 表記の統一 98-101 → 99-101)

## つまずき

- レビュアーが ADR 内の `state.rs::push` 引用行範囲の不整合 (98-101 vs 99-101) を指摘。minor だが独立 commit で修正した。
- 本 ADR は純粋にドキュメント化であり、code 変更なし。`state.rs::push` の既存ガードがすでに normative 挙動と一致しているため、`state.rs:99-101` をそのまま保持した。

## 影響

- コード変更なし。
- spec §9.2 に normative 記述を明記することで、M3b `prop_idempotence_on_committed` の弱化形が ADR 裏付けを得た形で確定。
- 本 ADR により M3b property test strategy の broaden が unblock された (残り blocker は #23 および発見された #29)。

## レビュー

Small tier (docs-only)。Architecture + 内容整合性の単一 reviewer + secrets-check。findings は line-range 不整合 (Minor 1 件) のみ、本 PR 内で解消。

## M4 への申し送り

- 非 ASCII 入力が `StateMachine::push` に到達した場合は `PushResult::Invalid(ch)` を返す (spec §9.2)。`input::InputContext` が `push` を直接呼び出す設計時、この契約に依存できる。
- 将来 Phase 1+ でかな漢字変換層を追加する際、かな層の出力が再度 romaji 層に戻る設計は避ける (本 ADR §影響 項目を参照)。

## 成果物リンク

- PR: https://github.com/std-koh-hinooka/kotoha-ime/pull/28
- ISSUE: https://github.com/std-koh-hinooka/kotoha-ime/issues/22 (CLOSED)
- ADR: `docs/adr/0001-non-ascii-retraction-policy.md`
- Spec: `docs/superpowers/specs/2026-04-22-kotoha-phase-0-design.md` §9.2
