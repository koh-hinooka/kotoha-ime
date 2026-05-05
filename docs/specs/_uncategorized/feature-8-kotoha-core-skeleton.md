---
feature: feature-8-kotoha-core-skeleton
status: implemented
bounded_context: _uncategorized
related_issues: ["#8"]
related_prs: []
glossary_refs: ["hiragana","kana","katakana","lefthook","romaji"]
last_reviewed: 2026-05-05
---

# M2: kotoha-core skeleton + error + kana utilities

> **Migration note**: 本 spec は `docs/wbs/2026-04-22-feature-8-kotoha-core-skeleton.md` から spec 化 (Global CLAUDE.md §Documentation Structure 準拠の `docs/wbs/` 廃止対応、2026-05-05)。
> 元 WBS は実装ログ性質。14-section restructure は別 follow-up 起票予定 (各 project の既存 14-section follow-up issue と合流可能)。

## 元 WBS 内容 (実装ログ由来)

milestone: M2
branch: feature/8-kotoha-core-skeleton
pr: "#9"
merge_commit: "290cce7b7664536c8d1b96e919668c583be185a2"
issue: "#8"
status: done
started: 2026-04-22
finished: 2026-04-22
---

# M2: kotoha-core skeleton + error + kana utilities

## 実施内容

- root Cargo.toml の workspace members に `crates/kotoha-core` を追加、`[workspace.dependencies]` に thiserror/anyhow/tracing/tracing-subscriber を pin
- `Cargo.lock` を commit(binary crate を含む workspace のため)
- M2 implementation plan を `docs/superpowers/plans/2026-04-22-kotoha-phase-0-m2.md` に記録
- `crates/kotoha-core/Cargo.toml` を workspace 継承形で作成
- `src/error.rs`: Error enum (#[non_exhaustive], InvalidCharacter/InvalidState) + Result<T> alias + 3 unit tests
- `src/kana/mod.rs`: hiragana/katakana サブモジュールの再エクスポート
- `src/kana/hiragana.rs`: is_hiragana + hiragana_to_katakana + 11 unit tests
- `src/kana/katakana.rs`: is_katakana + katakana_to_hiragana + 11 unit tests
- `src/lib.rs`: 公開 API ルート(pub mod error; pub mod kana; pub use error::{Error, Result};)
- テスト合計 25 件 (error 3 + kana 22)、すべて PASS
- lefthook pre-push が skip → execute に自動昇格、build/clippy/test 全て PASS

## つまずき

1. `cargo fmt` のトレーリング newline 要求で lefthook fmt-check が 0-byte lib.rs を reject(touch で作った初期状態)。`cargo fmt` で 1-byte (newline only) に補正して解決。
2. plan Task M2-4 Step 4 の Red フェーズ実行時、`mod kana/mod.rs` が `mod katakana` を宣言しているため `katakana.rs` が早期に必要。Step 7 の内容でスタブだけ先に配置(テストは後)して Red を成立させた。
3. PR review で Testing dimension から Medium 2 件(borderline 拒否テストと repeat-mark 変換テスト)の指摘。同 branch で 4 件テスト追加し、25 tests に拡張して merge。

## M3 への申し送り

- `RomajiConverter` は `kotoha-core::input::InputContext` に内包される予定(spec §7.5)。M3 で `romaji` モジュール実装後、M4 で `input` モジュール実装。
- `proptest` の dev-dependency 追加は M3 の最初のタスク。Testing review で挙がった round-trip test (L1) は M3 で property-based に拡張できる。
- Architecture review H1(Edition 2021 / thiserror v1 採用)は follow-up task #13 の M7 ADR 0004 候補に統合済み。M3 着手前に現状維持 or 切替を決定。
- Architecture review M2(未使用 workspace deps = anyhow/tracing-subscriber)は「M6 kotoha-cli 向け先行投資」の旨を Cargo.toml にコメントするか、M6 で追加する判断を ADR 化。

## 成果物リンク

- PR: #9 (squash merge)
- ISSUE: #8
- Spec: `docs/superpowers/specs/2026-04-22-kotoha-phase-0-design.md`
- Plan: `docs/superpowers/plans/2026-04-22-kotoha-phase-0-m2.md`
