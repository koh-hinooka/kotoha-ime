# 0008: canonical romaji 定義と部分的可逆性の回復 — Phase 1 判断: 現状維持

## ステータス

承認 (2026-04-24)

## コンテキスト

Phase 0 の romaji 変換は多対一マッピングを採用している: `ji` / `zi` → じ、`tu` / `tsu` → つ、`si` / `shi` → し、`ti` / `chi` → ち などの異体ペアが複数存在する。strict な round-trip invertibility (`romaji → kana → romaji == romaji`) は数学的に定義不能なため、spec §11.3 revision 2 でプロパティテスト集合から除外した。

§11.3 の該当段落は「canonical romaji を定義して部分的に可逆性を回復する拡張は Phase 1 以降で検討する(ADR 候補)」で結ばれている。本 ADR はその「ADR 候補」を placeholder として開設し、Phase 1 計画時に忘れず参照されることを保証する tracking hook の役割を果たす。

2026-04-24 に Phase 1 kick-off のため本 ADR の 3 選択肢を再評価し、**選択肢 1 (canonical romaji を定義しない、現状維持)** を採用する判断を下した。本 ADR はその決定の最終版として「承認」ステータスに昇格する。

## 検討した選択肢 (Phase 1 で正式評価)

- **選択肢 1: canonical romaji を定義しない (現状維持)** — 多対一のまま。Phase 1 以降のかな漢字変換層は invariants を romaji 側に戻せない前提で設計。
- **選択肢 2: canonical romaji を定義し partial invertibility を確立** — 各異体ペアから 1 つを正規形として選択 (例: `ji` / `tsu` / `shi` / `chi` を採用)。property test に `canonicalize_then_invert` を追加。
- **選択肢 3: canonical romaji を configuration で選択可能にする** — ユーザ / ライブラリ利用者が正規形を選ぶ。複雑性が高い。

## 決定

**選択肢 1 (現状維持 / canonical romaji を定義しない)** を採用する。

Phase 1 以降、romaji 変換層は多対一マッピング (`ji` / `zi` → じ、`tu` / `tsu` → つ、`si` / `shi` → し、`ti` / `chi` → ち など) を維持する。spec §11.3 のプロパティテスト集合から `canonicalize_then_invert` 系の不変条件は引き続き除外する。

Phase 1 の Kana→Kanji 変換層は入力がひらがな (既に `じ` / `つ` / `し` / `ち` などに正規化された形) であり、romaji 側の多対一性とは独立レイヤーのため、本決定が Phase 1 実装に与える影響はない。

## 影響

### Phase 0 rule table への影響

なし。現行の `crates/kotoha-core/src/romaji/rules.rs` (213 entries、207 rule keys) はそのまま維持する。

### Phase 1 以降への影響

- **多対一入力の継続**: romaji 入力は異体ペアの両方を受け付ける方針を維持する。例: `ji` / `zi` の両方が `じ` に変換される。
- **将来の入力 convention 拡張は本決定と両立**: user の future vision として、記号マクロ変換 (例: `xl` → `→`、`->` → `→` のような typography 記号入力) や追加 convention 対応が想定される。これらは Phase 0 rule table に新 key を追加するだけで実装可能で、canonical romaji 経由を必要としない。本 ADR の決定と両立する。
- **property test 集合**: spec §11.3 revision 2 の除外決定を維持する。`canonicalize_then_invert` 系の property は追加しない。

### 将来の選択肢 2 / 3 への migration path

選択肢 1 を採用しても、将来 canonical romaji が必要となる具体的 use case (例: kanji → reading の逆引き romaji 表示、ヘボン式 / 訓令式 configurability) が発生した場合は、以下のいずれかで対応する:

- **display layer で canonical 化**: input 側の多対一を維持したまま、display 時に canonical form を pick する形で実装し、input と display の責務を分離する (選択肢 1 の拡張として扱う)。
- **新 ADR で選択肢 2 / 3 を採用**: Phase 1 以降で必要性が実証された時点で、本 ADR を参照しながら新 ADR (番号は当時の canonical scheme に従う) を作成して判断を更新する。本 ADR は 2026-04-24 時点の判断記録として残す。

## 参照

- `docs/superpowers/specs/2026-04-22-kotoha-phase-0-design.md` §11.3 (invariants 集合と canonical romaji の言及)
- `docs/ROADMAP.md` Phase 1 carry-over セクション
- ISSUE #16 (本 ADR 作成 tracking)
- PR #14 architecture review (発見経緯)
- Phase 1 kick-off review (2026-04-24): 本 ADR の 3 選択肢を再評価し、選択肢 1 採用を決定
