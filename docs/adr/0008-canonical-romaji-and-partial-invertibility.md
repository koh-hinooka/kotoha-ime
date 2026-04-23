# 0008: canonical romaji 定義と部分的可逆性の回復 (Phase 1 検討事項)

## ステータス

提案 (2026-04-23) — Phase 1 で最終決定する placeholder

## コンテキスト

Phase 0 の romaji 変換は多対一マッピングを採用している: `ji` / `zi` → じ、`tu` / `tsu` → つ、`si` / `shi` → し、`ti` / `chi` → ち などの異体ペアが複数存在する。strict な round-trip invertibility (`romaji → kana → romaji == romaji`) は数学的に定義不能なため、spec §11.3 revision 2 でプロパティテスト集合から除外した。

§11.3 の該当段落は「canonical romaji を定義して部分的に可逆性を回復する拡張は Phase 1 以降で検討する(ADR 候補)」で結ばれている。本 ADR はその「ADR 候補」を placeholder として開設し、Phase 1 計画時に忘れず参照されることを保証する tracking hook の役割を果たす。

## 検討した選択肢 (Phase 1 で正式評価)

- **選択肢 1: canonical romaji を定義しない (現状維持)** — 多対一のまま。Phase 1 以降のかな漢字変換層は invariants を romaji 側に戻せない前提で設計。
- **選択肢 2: canonical romaji を定義し partial invertibility を確立** — 各異体ペアから 1 つを正規形として選択 (例: `ji` / `tsu` / `shi` / `chi` を採用)。property test に `canonicalize_then_invert` を追加。
- **選択肢 3: canonical romaji を configuration で選択可能にする** — ユーザ / ライブラリ利用者が正規形を選ぶ。複雑性が高い。

## 決定

**保留 (Phase 1 で決定)**。本 ADR は tracking hook であり、Phase 1 計画開始時に本 ADR をレビューし上記 3 選択肢から決定する。決定が確定した時点で本 ADR のステータスを「承認」に更新し、必要に応じて別 ADR 番号として fork する。

## 影響 (決定後に記述)

Phase 1 の計画で選択肢を決定した後に追記する。

## 参照

- `docs/superpowers/specs/2026-04-22-kotoha-phase-0-design.md` §11.3 (invariants 集合と canonical romaji の言及)
- `docs/ROADMAP.md` Phase 1 carry-over セクション
- ISSUE #16 (本 ADR 作成 tracking)
- PR #14 architecture review (発見経緯)
