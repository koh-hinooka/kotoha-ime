# 0007: Rust toolchain と OSS publish ポリシー (M1 設計判断の事後記録)

## ステータス

承認 (2026-04-24)

## コンテキスト

M1 (Cargo workspace 立ち上げ, ISSUE #3 / PR #8) で採用された 4 件の設計判断は、当時の PR レビューで「ADR として記録するのが望ましい」と指摘されたものの、M1 merge 時点では ADR 化されなかった。M7 (Phase 0 finishing docs) で本 ADR を作成し、これら 4 件の事後記録を行う。

対象となる 4 件:

1. **MSRV 1.80 + Edition 2021 の採用** (`~/.claude/rules/lang-rust.md` が Edition 2024 を推奨しているにも関わらず Edition 2021 を選んだ)
2. **M1 期間中の `cargo build` 検証例外** (empty virtual workspace に対して Cargo 1.94+ が `could not find Cargo.toml` エラーを出す事象により、M1 の pre-push gate では build 検証をスキップした)
3. **LICENSE-MIT の copyright holder に GitHub handle `std-koh-hinooka` を採用** (本名を OSS 配布物に直接書き込むのを避け、GitHub account handle で識別する方針)
4. **`Cargo.toml` の `authors` 欄に公開メール `koh.hinooka@student.it.com` を埋め込み** (crates.io への publish 時に露出する連絡先として採用)

本 ADR はこれら 4 件を 1 つの文書にまとめる。各決定は単独で短い判断であり、独立した ADR に分割するほどの記述量がないため、「toolchain 運用と OSS publish ポリシー」という共通テーマの下で束ねる。

## 検討した選択肢

### 決定 1: MSRV 1.80 + Edition 2021 (採用)

- 検討: Edition 2024 (let chain 安定版等を使える) vs Edition 2021 (対象ディストロの Rust バージョンでサポートされる最新)
- 選択: Edition 2021, MSRV 1.80
- 理由: 本プロジェクトの想定配布先である Ubuntu 24.04 LTS (Rust 1.75 標準) および Debian trixie でのビルド可能性を優先する。Phase 0 の実装範囲 (ローマ字変換、入力モード、CLI) で Edition 2024 の新機能は必要とされない。

### 決定 2: M1 期間中の `cargo build` 検証例外 (採用)

- 検討: `cargo build` を pre-push gate に含める vs M1 のみ例外とする
- 選択: M1 のみ例外
- 理由: M1 時点ではメンバー crate が未作成の virtual workspace であり、Cargo 1.94+ が `could not find Cargo.toml` エラーを返していた。M2 以降 (kotoha-core crate 追加以降) は当該エラーが発生しないため、通常の build gate に戻した。M2-M6 で本例外は既に解除済み。

### 決定 3: LICENSE-MIT copyright holder に GitHub handle 採用 (採用)

- 検討: 本名 vs GitHub handle (`std-koh-hinooka`)
- 選択: GitHub handle
- 理由: OSS 配布物 (LICENSE-MIT / Cargo.toml metadata) は公開される前提であり、本名の露出は不要。GitHub handle により、本プロジェクトが個人名義の学習プロジェクトであることを明示しつつ、legal copyright holder としての identifier の一意性を保てる。

### 決定 4: `Cargo.toml` authors 欄に公開メール埋め込み (採用)

- 検討: メール欄を空にする vs 公開メールを埋め込む
- 選択: 公開メール `koh.hinooka@student.it.com` を埋め込む
- 理由: crates.io publish 時に「authors 欄にメール記載」は一般的慣習であり、OSS 貢献者が連絡する窓口として機能する。spam 耐性は mail server 側の filter で担保し、メール公開そのものは受容する。

## 影響

### 決定 1 (MSRV 1.80) の影響

- Edition 2024 特有の構文 (安定化された let chain 等) は本プロジェクトで使用しない。必要になった場合は MSRV 引き上げの別 ADR で判断する。
- `rust-toolchain.toml` / `Cargo.toml` の `rust-version = "1.80"` pin により、古い toolchain でのビルド失敗を早期検出できる。

### 決定 2 (cargo build 例外) の影響

- M1 merge 後 (M2 の kotoha-core crate 追加後) は通常の `cargo build --workspace` を pre-push gate に含める状態に復帰済み。現時点 (M7) では本例外は履歴情報として残るのみ。

### 決定 3 (copyright handle) の影響

- 本プロジェクトに外部 contributor が参加した場合、各 contributor 自身の copyright notice は GitHub handle または本名のいずれを選んでも良い (各自の判断に委ねる)。プロジェクト全体の copyright は `std-koh-hinooka` (プロジェクト owner) の下に集約する。

### 決定 4 (publish メール) の影響

- crates.io に publish した場合、本メールは crates.io の package metadata ページに公開される。spam filter 側で抑制する前提とする。
- メール変更が必要になった場合は `Cargo.toml` の `authors` 欄のみ変更すればよく、コードへの影響はない。

## 参照

- M1 PR #8 — 本 ADR の対象となる 4 件の設計判断が実装された PR。
- M1 PR #8 review comments — 「ADR 化を推奨」という指摘の出処。
- `~/.claude/rules/lang-rust.md` — Edition 2024 推奨ルール (決定 1 で意図的に逸脱)。
- `Cargo.toml` — `rust-version = "1.80"`, `edition = "2021"`, `authors` 欄の実装。
- `LICENSE-MIT` — copyright holder に `std-koh-hinooka` を採用した実装。
- `rust-toolchain.toml` — toolchain pin の実装。
