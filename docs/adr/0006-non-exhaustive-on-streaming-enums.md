# 0006: Streaming 結果 enum に `#[non_exhaustive]` を付与する

## ステータス

承認 (2026-04-23)

## コンテキスト

ローマ字変換レイヤは 2 種類の streaming 結果 enum を持つ: `ConvertStep` (`crates/kotoha-core/src/romaji/mod.rs`, 外部公開 `pub`) と `PushResult` (`crates/kotoha-core/src/romaji/state.rs`, クレート内部 `pub(crate)`)。M3a 時点の variants は両 enum とも `{Committed(Cow<'static, str>), Pending, Invalid(char)}` である。将来、両 enum に新 variants (例: "emitted punctuation" / "would-commit-on-flush" / Phase 3 の句読点やモード遷移イベント) を足す余地がある。外部 match サイトの網羅性制約と将来拡張の SemVer コストのトレードオフを決定する必要がある。

## 検討した選択肢

### 選択肢 1: 無印 (exhaustive) のまま維持する

- 利点: downstream の `match` は網羅的に書け、variants 追加を compile error で検知できる。
- 欠点: variants 追加が外部クレート側で breaking change となり SemVer major bump が必要。Phase 3 以降の拡張コストが高い。

### 選択肢 2: `ConvertStep` / `PushResult` の両方に `#[non_exhaustive]` を付与 (採用)

- 利点: クレート境界越しの match サイトで `_ => ...` arm が必須化され、variants 追加を non-breaking (SemVer minor) で行える。`PushResult` にも付与することで、将来 `kotoha-core` から別 crate (例: `kotoha-ibus`) へ再エクスポートする場合も安全性が維持される。streaming 結果 enum 全てが拡張可能という明示的ポリシーとなり、API 設計の一貫性が保たれる。
- 欠点: 外部利用者は必ず `_` arm を書く必要があり、網羅性チェックの強みを 1 段階弱める。

### 選択肢 3: `ConvertStep` のみ `#[non_exhaustive]`、`PushResult` は無印

- 利点: `PushResult` は `pub(crate)` のため後方互換性問題は無く、最小付与で済む。
- 欠点: streaming 結果 enum 同士のポリシーが不揃いとなり、将来 refactor 時に「`PushResult` は公開してよいか」の判断が複雑化する。一貫性を犠牲にする利点が軽微な可読性向上に見合わない。

## 決定

**選択肢 2 (両 enum に `#[non_exhaustive]` を付与)** を採用する。M3a 時点で両 enum には既に `#[non_exhaustive]` が付与済みであり、本 ADR はそのポリシーを normative として記録する。

## 影響

### 実装への影響

- クレート内 match は依然として網羅的に書ける (`#[non_exhaustive]` はクレート境界をまたぐ場合のみ制約するため)。
- 外部クレート (将来の `kotoha-ibus` など) は `ConvertStep` を match する際に `_ => ...` arm が必須となる。

### API 拡張への影響

- 将来 variants の追加 (`EmittedPunctuation` / `WouldCommitOnFlush` 等) は SemVer minor bump で済む。Phase 3 以降の拡張コストが下がる。

### ドキュメントへの影響

- 外部公開 rustdoc に「`_` arm 必須」の注意を明記することが望ましい。`ConvertStep` の rustdoc (`mod.rs:19-37`) には既にその旨が記載されている。

## 参照

- `crates/kotoha-core/src/romaji/mod.rs` — `ConvertStep` 定義 (`pub`, `#[non_exhaustive]`)。
- `crates/kotoha-core/src/romaji/state.rs` — `PushResult` 定義 (`pub(crate)`, `#[non_exhaustive]`)。
- ISSUE #20 — 本 ADR を新設する docs-only ISSUE。
- PR #18 architecture review (Low finding) — 本 ADR 新設の発端。
