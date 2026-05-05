---
feature: docs-54-phase0-finishing-docs
status: deprecated
deprecated_reason: "Phase E migration で旧 docs/wbs/ から spec 化した実装ログ性質の文書。Global CLAUDE.md §Development Flow legacy spec 取扱いルール (実装ログ性質 → status: deprecated、本文 14-section restructure 不要) に基づき deprecated 扱い。git history は参照点として保持 (2026-05-06)。"
bounded_context: _uncategorized
related_issues: ["#54"]
related_prs: []
glossary_refs: ["canonical-romaji","lefthook","shift-trigger"]
last_reviewed: 2026-05-06
---

# M7: Phase 0 finishing docs (ADRs 0003/0004/0007 + ROADMAP/README)

> **Migration note**: 本 spec は `docs/wbs/2026-04-24-docs-54-phase0-finishing-docs.md` から spec 化 (Global CLAUDE.md §Documentation Structure 準拠の `docs/wbs/` 廃止対応、2026-05-05)。
> 元 WBS は実装ログ性質。14-section restructure は別 follow-up 起票予定 (各 project の既存 14-section follow-up issue と合流可能)。

## 元 WBS 内容 (実装ログ由来)


- ISSUE: [#54](https://github.com/std-koh-hinooka/kotoha-ime/issues/54)
- PR: [#55](https://github.com/std-koh-hinooka/kotoha-ime/pull/55)
- merge SHA: `7781242`
- 担当 Claude セッション: M7 実装

## 実施内容

- 3 件の ADR を新規作成した。
  - `docs/adr/0003-shift-via-uppercase-char.md` (64 行): Phase 0 CLI における Shift トリガ = ASCII 大文字入力という対応付けを、物理キーコードを観測できない CLI 環境向けの Phase 0 限定判断として記録した。Phase 3 IBus engine では物理 `KeyPress` イベントに置き換える方針も併記した。
  - `docs/adr/0004-cli-line-based-commit.md` (64 行): `kotoha-romaji` が stdin の改行境界を `InputContext::commit()` の契機として扱うことを、spec §10.2 の normative 記述と整合する pragmatic 判断として記録した。
  - `docs/adr/0007-rust-toolchain-and-publishing-policy.md` (73 行): M1 PR #8 レビューで「ADR 化推奨」と指摘された 4 件 (MSRV 1.80 + Edition 2021 / M1 期間中の `cargo build` 検証例外 / LICENSE-MIT copyright holder に GitHub handle / `Cargo.toml` authors 欄に公開メール) を 1 つの ADR として事後記録した。
- `docs/ROADMAP.md` の Phase 0 行の状態を `実装中` から `完了` に更新した。
- `README.md` の開発フェーズ表の Phase 0 状態を `設計完了、実装未着手` から `完了` に更新し、新規に `## kotoha-romaji CLI 使用例` セクション (spec §10.3 由来 6 例 + build 手順 + `scripts/phase0-smoke.sh` への参照) を挿入した。
- 差分は 271 insertions / 2 deletions 計 5 ファイル。M7 スコープ上限 (350 LOC) 内。

## つまずき

特になし。docs-only の事後記録作業であり、実装変更を伴わなかったため cargo / lefthook / review すべて初回で green 通過した。

## レビュー結果

Medium tier inline review (5 dimensions + secrets-check) を実施した。

| dimension | Critical | High | Medium | Low |
|---|---|---|---|---|
| security | 0 | 0 | 0 | 0 |
| performance | 0 | 0 | 0 | 0 |
| architecture | 0 | 0 | 0 | 0 |
| testing | 0 | 0 | 0 | 0 |
| a11y-as-doc-readability | 0 | 0 | 0 | 0 |

secrets-check: CLEAN (password / secret / api-key / token / AKIA / PRIVATE KEY のいずれも 0 マッチ。ADR 0007 に記載の `koh.hinooka@student.it.com` は本人が事前に公開メールとして承認した文字列で、既に `Cargo.toml` に存在する)。

テスト: `cargo build --workspace` / `cargo test --workspace` (unit + integration + doc + property 合計全件 pass) / `cargo clippy --workspace --all-targets -- -D warnings` (警告 0) / `cargo fmt --all --check` (差分 0) の 4 ゲートを sequential に実行し全て green。lefthook pre-commit (doc-naming) と pre-push (build / clippy / manifest-check / test) も green。README の CLI 使用例 6 件のうち 4 件 (basic / shift trigger / mixed / show-mode) を `target/debug/kotoha-romaji` で実行し、documented output と完全一致することを確認した。

## Phase 0 完了宣言

本 M7 merge をもって Phase 0 (Foundation) 全マイルストーン M1–M7 が develop に統合された。

| Milestone | ISSUE / PR | 成果物 |
|---|---|---|
| M1 Cargo workspace 立ち上げ | #3 / #8 | workspace skeleton, Cargo.toml pin |
| M2 kotoha-core crate skeleton | #12 | crates/kotoha-core lib + エラー型 |
| M3 ローマ字→かな変換 | #13 / #14 / #16 | `RomajiConverter`, 207 ルール, `StateMachine` |
| M3 補完 OnceLock trie cache | #19 / PR #24 | `global_trie()` + ADR 0005 |
| M3 補完 rule coverage gap 閉鎖 | #25 / PR #50 | 55 行 fixture 拡張 |
| M4a 入力モード ADR 0002 | #34 / PR #35 | `InputMode` / `ModeOrigin` enum |
| M4b `InputContext` 実装 | #36 | 状態機械 + Transient 自動復帰 |
| M4c mode golden test + property test | #40 / PR #41 | 80 行 golden + 8 properties |
| M5 エラー型公開 + non_exhaustive | ADR 0006 | `Error` 公開 API 固め |
| M6 kotoha-cli + phase0-smoke | #49 / PR #53 | `kotoha-romaji` + smoke スクリプト |
| M7 Phase 0 finishing docs | #54 / PR #55 | ADR 0003/0004/0007 + ROADMAP/README |

ADR 集合: 0001 (retraction) / 0002 (input-mode) / 0003 (shift-trigger) / 0004 (cli-commit) / 0005 (trie) / 0006 (non_exhaustive) / 0007 (toolchain) / 0008 (canonical-romaji, 提案状態で Phase 1 に申し送り) の計 8 件が `docs/adr/` に存在する。

spec §13.3 (ドキュメント成果物) のチェックリストは本 M7 merge により全項目達成済み:

- [x] ADR が 8 件存在すること
- [x] 各 ADR に ステータス / コンテキスト / 検討した選択肢 / 決定 / 影響 / 参照 セクションが揃っていること
- [x] `README.md` に `kotoha-romaji` の使用例が 5 件以上記載されていること (実際は 6 件 + ビルド手順 + smoke スクリプトへの参照)
- [x] `ROADMAP.md` で Phase 0 が `完了` 状態であること

Phase 1 (かな→漢字変換) への申し送りは `docs/ROADMAP.md` §「Phase 1 への申し送り」に既に記載済み (ADR 0008 の canonical-romaji 拡張検討)。

## 成果物リンク

- ISSUE: <https://github.com/std-koh-hinooka/kotoha-ime/issues/54>
- PR: <https://github.com/std-koh-hinooka/kotoha-ime/pull/55>
- merge commit: <https://github.com/std-koh-hinooka/kotoha-ime/commit/7781242>
- 新 ADR:
  - `docs/adr/0003-shift-via-uppercase-char.md`
  - `docs/adr/0004-cli-line-based-commit.md`
  - `docs/adr/0007-rust-toolchain-and-publishing-policy.md`
- 更新:
  - `docs/ROADMAP.md`
  - `README.md`
