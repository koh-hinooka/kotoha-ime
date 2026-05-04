# Kotoha — Project-Specific Claude Instructions

本文書は global `~/.claude/CLAUDE.md` の規約を前提とし、Kotoha プロジェクト固有の例外・追加規約のみを記載する。

## 目次

- [プロジェクト概要](#プロジェクト概要)
- [Language 例外](#language-例外)
- [Rust 開発規約](#rust-開発規約)
- [依存管理](#依存管理)
- [テスト規約](#テスト規約)
- [Phase 状態の参照](#phase-状態の参照)
- [WBS 直接 push の例外](#wbs-直接-push-の例外)
- [Glossary](#glossary)

## プロジェクト概要

- GNOME Wayland ネイティブに動作する自作日本語 IME
- Rust で実装、Cargo workspace 構成
- 設計書: `docs/specs/{_uncategorized,<bounded-context>}/<feature-slug>.md` (frontmatter は global §Spec Frontmatter 参照)
- 実装計画: `docs/plans/<yyyy-MM-dd>-<branch>.md`

## Language 例外

global CLAUDE.md は「commit message / PR / ISSUE は日本語」だが、**Kotoha プロジェクトは例外として英語を使用** する。

理由: 将来 OSS として公開する想定であり、外部貢献者との互換性を優先するため。

英語で記述する対象:

- commit message
- PR タイトル / body
- GitHub ISSUE タイトル / body
- GitHub ラベル名
- rustdoc / API doc(外部公開 API のドキュメントは OSS 貢献者との互換性を優先)

日本語を維持する対象:

- 設計書 (`docs/specs/`)
- 実装計画 (`docs/plans/`)
- ADR (`docs/adr/`)
- WBS (`docs/wbs/`、本 project の例外として保持。詳細は ADR 0019 参照)
- コード内コメント(ドメイン説明など、日本語のほうが意味が通じる箇所)
- Claude Code との対話

## Rust 開発規約

- edition: 2021
- rust-version: 1.80(`Cargo.toml` で pinned)
- フォーマッタ: `cargo fmt --all`
- リンタ: `cargo clippy --workspace --all-targets -- -D warnings`(警告ゼロを必須)

## 依存管理

- workspace root の `[workspace.dependencies]` で全 crate の依存 version を pin する
- 個別 crate は `{ workspace = true }` で参照する
- 新規依存追加時は、目的と代替案の比較を commit message または PR body に記載する

## テスト規約

- 単体テスト: `#[cfg(test)]` で同ファイル内
- 統合テスト: `crates/*/tests/` 配下
- golden テスト: `crates/*/tests/fixtures/*.tsv` を `tests/*_golden.rs` から読み込む
- property test: `proptest` を dev-dependency として使用予定(M3 で追加し romaji 変換の property test に使用、M5 で input mode の property test にも適用)

## Phase 状態の参照

現在の Phase、完了条件、マイルストーン分割は以下を参照:

- `docs/ROADMAP.md` — Phase 全体像 + Active マイルストーン (v0.3.0 = Phase 3) 含む SemVer マッピング
- `docs/specs/_uncategorized/` — 各 Phase の設計書 (kotoha-phase-{0,1,2,5}, p2-{a,b,c}, p3-a-ibus-engine、将来 cluster 化は global §Spec Clustering 参照)
- `docs/plans/` — マイルストーン単位の実装計画 (Branch-scoped: `<yyyy-MM-dd>-<branch>.md`)
- `docs/wbs/` — 過去の実装ログ (本 project 例外、ADR 0019 参照)
- post-merge 必須項目: global §post-merge follow-up checklist 参照 (Spec status / Glossary 同期 / ROADMAP / Vault 同期 / マイルストーン完了判定)

## Obsidian vault

- **scope**: `kotoha-ime`
- **vault**: `$OBSIDIAN_VAULT_DIR` (`.envrc` で export、`/check-direnv` で検証)
- **用語集**: `$OBSIDIAN_VAULT_DIR/glossary/<concept>.md` (project canonical、101 用語、本 PR で `docs/wiki/glossary.md` から migration)
- **vault → spec symlink**: `$OBSIDIAN_VAULT_DIR/specs/kotoha-ime/`

## WBS 直接 push の例外 (狭域化)

WBS ログ(`docs/wbs/*.md`)は本 project 例外として保持される (ADR 0019)。global rules では WBS は廃止だが、Kotoha は 35 件の歴史的実装ログを抱えるため以下を許容する:

- **既存 WBS ファイルへの軽微編集** (typo fix、merge 後の log 追記等): 対象 PR の merge 後に develop へ直接 push 可
- **新規 WBS 起票は禁止**: 既存 WBS file は過去ログとして保持、新規の進行中作業の追跡は in-conversation `TaskCreate` (global rule §Persistent Memory) を使用

例外解消時期: 既存 WBS が active spec / plan / commit message から参照されなくなった時点で `docs/_archive/wbs/` へ移動または削除 (cleanup PR)。

## Glossary

ドメイン固有語彙は `$OBSIDIAN_VAULT_DIR/glossary/<concept>.md` (1 用語 1 ファイル) に集約する。本 PR (#174) で旧 `docs/wiki/glossary.md` から 101 用語を vault へ migration 済 (cluster: `japanese-input-basic` / `input-mode` / `romaji` / `kana-kanji` / `llm` / `testing` / `phase5-custom` / `reference-impl` / `tool-env`)。

新規ドメイン用語を specs / plans / コードで導入する際は、対応する vault concept ファイルを追加し、spec frontmatter `glossary_refs` に slug を追加する。global rule `~/.claude/rules/glossary-consistency.md` に準拠 (本 project では vault 経由で参照)。
