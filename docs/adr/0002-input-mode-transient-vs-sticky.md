# 0002: Shift トリガ由来の Direct モードを Transient とする (Mozc 互換 / Karukan 非互換)

## ステータス

承認 (2026-04-23)

## コンテキスト

Kotoha は GNOME Wayland 向けの自作日本語 IME であり、参考実装 Karukan の Shift 挙動による不便を解消することが Phase 0 の主要動機である (spec §2 参照)。Karukan では Shift + 何らかのキーを押すと直接入力モード (ローマ字がそのまま出力されるモード) に入り、Enter 確定後もそのモードが持続する。ユーザが明示的にモード切替キーを押して戻さない限り、以降の入力もすべて英字として扱われる。この挙動は日本語入力体験を著しく損なっている。

一方、Mozc / Google 日本語入力の挙動は異なる。Mozc では Shift キー押下で一時的に直接入力モードへ入るが、Enter 確定で自動的にひらがなモードへ復帰する。これは入力中の 1 語ごとに「Shift 押下 → 英字入力 → Enter → ひらがな入力再開」という自然な流れを成立させる。

spec §8 はこの観察に基づき、`(mode, origin)` タプルで状態を表現する 3 状態機械を定義している:

- `(Hiragana, Sticky)` = 初期状態、ローマ字→かな変換
- `(Direct, Transient)` = Shift トリガで入った一時英字モード
- `(Direct, Sticky)` = 明示トグルで入った持続英字モード

`ModeOrigin::{Sticky, Transient}` は内部識別子であり、Transient origin の Direct モードのみが commit / cancel 契機で `(Hiragana, Sticky)` へ自動復帰する設計になっている。本 ADR はこの「Shift 由来 = Transient、明示トグル = Sticky」の 2 軸決定が、Phase 0 の入力モード管理レイヤーにおいて normative であることを記録する。

この判断は spec §8.5 で Karukan との差分として言及されているが、ADR としての記録は本 M4a (ISSUE #34, PR #35) で行う。

## 検討した選択肢

### 選択肢 1: Transient-by-default (採用候補、Mozc 互換)

- 利点:
  - Mozc / Google 日本語入力の挙動と揃うため、ユーザが既存の IME から移行する際の学習コストが最小になる。
  - 1 語ごとにモード復帰が自動化され、「明示的にモードを戻し忘れて次の日本語入力が英字になってしまう」事故を防ぐ。
  - Shift キーの「一時的な大文字化」セマンティクスが日本語入力体験において自然に機能する。
- 欠点:
  - ユーザが連続して英字を入力したい場合 (例: URL 入力中にひらがな IME のまま) は明示トグル (`toggle_mode`) による Sticky Direct への昇格が必要であり、Transient 中に追加の操作が必要になる。
  - Transient → Sticky 昇格経路 (`toggle_mode` が Transient Direct に対して呼ばれた場合の挙動) の仕様を別途決定する必要がある。

### 選択肢 2: Sticky-by-default (Karukan 互換)

- 利点:
  - 参考実装 Karukan と挙動が揃うため、Karukan のユーザが違和感なく移行できる。
  - `ModeOrigin` enum が不要になり、状態機械が 2 状態 (`Hiragana` / `Direct`) に単純化される。
- 欠点:
  - Phase 0 の主要動機 (Karukan の Shift 挙動による不便の解消) そのものを放棄することになる。本プロジェクトの存在理由に反する。
  - 大多数の日本語入力ユーザ体験において、「1 語英字入力した後もひらがなに戻らない」挙動は明確に unfriendly であることが、spec §2 で既に論証されている。

### 選択肢 3: Karukan-literal (Karukan の実装をほぼそのまま移植)

- 利点:
  - 実装工数が最小。
  - Karukan の golden fixture を流用可能。
- 欠点:
  - 選択肢 2 の欠点をすべて継承する。
  - Kotoha 独自の設計判断 (モード管理の責務を `kotoha-core::input` に閉じ込め、キーマッピングは Phase 3 の IBus engine 層に分離する方針、spec §4 参照) に反する。Karukan はモード遷移ロジックを統合層にハードコードしており、その設計ミスそのものが本プロジェクトの回避対象である。

## 決定

**選択肢 1 (Transient-by-default)** を採用する。具体的には:

1. Shift キー相当 (ASCII 大文字入力) による Hiragana → Direct 遷移は常に `(Direct, Transient)` を生成する。
2. `(Direct, Transient)` は `commit()` / `cancel()` で `(Hiragana, Sticky)` に自動復帰する。
3. 明示トグル (`toggle_mode()` / `set_mode()`) による Direct 遷移は常に `(Direct, Sticky)` を生成し、自動復帰の対象外とする。
4. `(Direct, Transient)` に対して `toggle_mode()` が呼ばれた場合は `(Direct, Sticky)` に昇格する。本昇格挙動はフラグ `InputContext::allow_transient_to_sticky_promotion: bool` で切り替え可能とし、Phase 0 でのデフォルトは `false` とする(Phase 3 以降で設定 UI 化予定)。

本決定は spec §8 (モード管理仕様) の状態遷移表を normative 仕様として認定する。Phase 0 では本 ADR と spec §8 の間に齟齬がないこと (遷移表全セルが `(Hiragana, Sticky)` / `(Direct, Transient)` / `(Direct, Sticky)` の 3 状態で閉じていること) を golden test (M4c) で verification する。

## 影響

### 実装への影響 (M4b)

- `crates/kotoha-core/src/input/mode.rs` に `InputMode` (`pub`) と `ModeOrigin` (`pub(crate)`) の 2 enum を定義する。
- `crates/kotoha-core/src/input/context.rs` の `InputContext` 状態機械は、spec §8.4 擬似コード通りに `input_char` / `commit` / `cancel` / `toggle_mode` / `set_mode` / `reset` を実装する。
- `InputContext::allow_transient_to_sticky_promotion: bool` フィールドを追加し、Phase 0 デフォルトは `false` とする。

### テストへの影響 (M4c)

- `crates/kotoha-core/tests/fixtures/mode_cases.tsv` 70 ケースは本 ADR の 3 状態機械を外部観察可能な入出力対に展開したものとして整備する。
- `crates/kotoha-core/tests/fixtures/mode_cases_karukan_diff.tsv` 10 ケースは本 ADR の「Transient 自動復帰」が Karukan の「Sticky 持続」と異なる入力を並べ、Kotoha 側の normative 挙動を明示する。
- property test 4 条件 (Hiragana-Sticky 安定性 / Transient 必ず復帰 / Sticky-Direct 持続 / reset 冪等性) は本 ADR の 3 状態機械の不変条件を proptest で検証する。

### spec への影響

- spec §8.5 (line 406) の ADR 参照番号を `0001-input-mode-transient-vs-sticky.md` から `0002-input-mode-transient-vs-sticky.md` へ修正する(本 PR 内で同時 Edit)。0001 は PR #28 により非 ASCII retraction policy に既に割り当て済であるため。

### Phase 3 以降への影響

- Phase 3 の IBus engine 層は `InputContext` の公開 API (`input_char` / `commit` / `cancel` / `toggle_mode` / `set_mode`) をキーイベントにマッピングする。本 ADR で 3 状態機械が normative に固定されたため、Phase 3 着手時にモード遷移ロジックを再設計する必要はない。
- 設定 UI で `allow_transient_to_sticky_promotion` を切り替え可能にする場合、本 ADR の「Phase 0 デフォルトは `false`」が起点となる。ユーザが Karukan 互換を望む場合は `toggle_mode` を介した昇格ではなく、将来の設定項目 (例: `ShiftModePolicy::Sticky`) で対応する (本 ADR を再検討する別 ADR を要する)。

## 参照

- ISSUE #32 — 本 ADR が属する M4 tracking ISSUE。
- ISSUE #34 — 本 ADR を新設する M4a ISSUE (本 PR で close)。
- Spec §2 — Kotoha の背景と動機 (Karukan の Shift 挙動による不便を解消する)。
- Spec §8 — モード管理仕様 (3 状態機械、遷移表、擬似コード)。
- Spec §8.5 — Karukan との差分 (本 ADR への参照を含む)。
- PR #28 (ADR 0001 — 非 ASCII retraction policy) — 0001 番号を先に消費した先行 ADR。
- Karukan (`togatoga/karukan`) — 反面教師としての参照実装 (MIT/Apache-2.0)。
- Mozc — Transient 挙動の模範となる OSS IME。
