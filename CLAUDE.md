# Kotoha — Project-Specific Claude Instructions

本文書は global `~/.claude/CLAUDE.md` の規約を前提とし、Kotoha プロジェクト固有の例外・追加規約のみを記載する。

## プロジェクト概要

- GNOME Wayland ネイティブに動作する自作日本語 IME
- Rust で実装、Cargo workspace 構成
- 設計書: `docs/superpowers/specs/`
- 実装計画: `docs/superpowers/plans/`

## Language 例外

global CLAUDE.md は「commit message / PR / ISSUE は日本語」だが、**Kotoha プロジェクトは例外として英語を使用** する。

理由: 将来 OSS として公開する想定であり、外部貢献者との互換性を優先するため。

英語で記述する対象:

- commit message
- PR タイトル / body
- GitHub ISSUE タイトル / body
- GitHub ラベル名

日本語を維持する対象:

- 設計書 (`docs/superpowers/specs/`)
- 実装計画 (`docs/superpowers/plans/`)
- ADR (`docs/adr/`)
- WBS (`docs/wbs/`)
- コード内コメント(ドメイン由来のみ。API doc は英語でもよい)
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
- property test: `proptest` を使用

## Phase 状態の参照

現在の Phase、完了条件、マイルストーン分割は以下を参照:

- `docs/ROADMAP.md` — Phase 全体像
- `docs/superpowers/specs/` — 各 Phase の設計書
- `docs/superpowers/plans/` — マイルストーン単位の実装計画
- `docs/wbs/` — 実装ログ

## WBS 直接 push の例外

WBS ログ(`docs/wbs/*.md`)は、対象 PR の merge 後に develop へ直接 push して OK とする。
理由: 実装内容に影響しない純粋なメタデータ記録であり、PR レビューの対象ではないため。
