# 0003: Phase 0 CLI では Shift トリガを ASCII 大文字入力で代替する

## ステータス

承認 (2026-04-24)

## コンテキスト

Kotoha の入力モード仕様 (spec §8) は、Hiragana モード中に Shift キーが押された場合に `(Direct, Transient)` へ遷移する挙動を normative に定義している (ADR 0002)。しかし Phase 0 の CLI `kotoha-romaji` は標準入力を **行単位の文字列** として受け取るため、物理 Shift キー押下という生のキーコードイベントを観測できない。

Phase 3 の IBus engine では `KeyPress` イベントから `Shift` modifier の状態を直接取得できるが、Phase 0 CLI は spec §10.4 で「キーコード模擬 (raw キーイベント流し込み) は Phase 0 では実装しない」と明記されている。このため Phase 0 CLI では Shift トリガを別の手段で表現する必要がある。一方、M4b で実装済みの `InputContext::input_char(ch)` は既に `ch.is_ascii_uppercase()` を Shift 相当として扱うロジックを持つ (spec §8.4 擬似コード line 352 参照)。

本 ADR は、Phase 0 CLI における「Shift トリガ = ASCII 大文字入力」という対応付けを Phase 0 限定の pragmatic 判断として記録し、Phase 3 IBus engine では物理キーコードイベントに置き換えることを明示する。

## 検討した選択肢

### 選択肢 1: ASCII 大文字を Shift トリガとして扱う (採用)

- 利点:
  - `InputContext::input_char` が既に採用しているロジックに CLI を揃えるだけで実装コストがゼロ。
  - パイプ入力 (`echo "HELLO" | kotoha-romaji`) で Shift の挙動を smoke test できるため、`scripts/phase0-smoke.sh` と組み合わせやすい。
  - spec §10.2 (大文字入力は Shift トリガ扱い) と整合する。
- 欠点:
  - 「Shift を押さずに大文字を入力」「Shift を押したが小文字が出る」といった物理キーと文字の不一致を CLI では表現できない。Phase 0 の smoke test 範囲ではこの限界は問題にならない。

### 選択肢 2: `--shift` / `--unshift` の明示引数で状態を渡す

- 利点: 大文字と Shift 状態を独立に制御でき、理論上は任意の組み合わせを検証できる。
- 欠点: spec §10 の「行単位で読み込み、行末を commit」という簡潔な CLI 設計から逸脱する。ユーザ操作の直感にも反し、pipe 駆動の smoke test が書きづらい。

### 選択肢 3: エスケープシーケンスで raw キーコードを模擬する

- 利点: Phase 3 の IBus engine と同じ抽象レベルで Shift を扱える。
- 欠点: spec §10.4 で「キーコード模擬は Phase 3 で追加」と明示的に defer されている。Phase 0 のスコープを超える。

## 決定

**選択肢 1 (ASCII 大文字を Shift トリガとして扱う)** を採用する。具体的には:

1. `kotoha-romaji` は stdin から受け取った行中の ASCII 大文字を、Shift が同時に押されたキー入力と等価に扱う。
2. 本対応付けは Phase 0 CLI 限定であり、`InputContext::input_char` の仕様そのものは「`is_ascii_uppercase()` が true のとき Hiragana → Transient Direct へ遷移」のまま変更しない (ADR 0002 の §8.4 擬似コードと同一)。
3. Phase 3 の IBus engine では本対応付けを使わず、物理 `KeyPress` イベントの `Shift` modifier 状態を直接観測する。

## 影響

### Phase 0 への影響

- `scripts/phase0-smoke.sh` の Shift ケース (例: `echo "HELLO" | kotoha-romaji` が `HELLO` を出力) が本 ADR の対応付けを前提として成立する (M6 PR #53 で実装済み)。
- CLI では「lowercase text with shift on」「uppercase text with shift off」を表現できない。Phase 0 の smoke test 要件 (spec §13.2) には含まれないため問題にならない。

### Phase 3 以降への影響

- Phase 3 の IBus engine 実装時に、本 ADR の対応付けを IBus の KeyPress ハンドラで置き換える。`InputContext::input_char(ch)` の呼び出しはそのままでよく、engine 層は「Shift modifier が立っている + 文字キー押下」を大文字 ASCII 文字に正規化して渡す責務を負う。
- 物理 Shift の状態を観測できるため、「Shift を押しながら小文字を打つ」ような不自然な入力経路は engine 層で吸収され、`InputContext` に影響しない。

## 参照

- ADR 0002 — 本 ADR が前提とする Transient-by-default モード仕様。
- Spec §8.4 — `InputContext::input_char` 擬似コード (line 352 で `is_ascii_uppercase` を Shift 相当として判定)。
- Spec §8.5 — Karukan との差分 (Shift 由来モードの扱い)。
- Spec §10.2 — Phase 0 CLI 動作仕様 (「大文字入力は Shift トリガ扱い」)。
- Spec §10.4 — Phase 0 で実装しないもの (キーコード模擬は Phase 3 で追加)。
- M6 PR #53 — `kotoha-romaji` CLI と `scripts/phase0-smoke.sh` の実装。
- ISSUE #32 — M4 (入力モード管理) tracking ISSUE。
