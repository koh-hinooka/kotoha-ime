# 0001: 非 ASCII 入力に対する `StateMachine::push` の取り扱い

## ステータス

承認 (2026-04-23)

## コンテキスト

`RomajiConverter::convert` は ASCII ローマ字列をかな文字列に変換する関数である。M3b (PR #24) の property test 実装中、次の観察により「非 ASCII 入力が `StateMachine::push` に到達した場合の挙動」が normatively pin されていないことが判明した。

- `convert("[")` は `(`"「", "")` を返す (ASCII `[` → 全角 `「`)。
- `convert("「")` は `("", "")` を返す (`StateMachine::push` の先頭ガード `if !ch.is_ascii()` により `PushResult::Invalid('「')` が返り、buffer は変化しない)。
- したがって「`convert(convert(x).committed).committed == convert(x).committed`」という強い retraction 性は成立しない (`convert(convert("[").committed)` の committed は `""` であり、元の `"「"` とは等しくない)。

M3b plan 初稿の property docstring は「state machine は `drop` または `pass-through` のいずれかを選ぶべき」と両論併記していたが、実装は `drop` を採用している。property test `prop_idempotence_on_committed` は strict equality を要求できないため、弱化した形 (`pending_second.is_empty()` のみ) で実装された経緯がある。

本 ADR は「どちらを normative とするか」を Phase 0 の範囲で確定する。

## 検討した選択肢

### 選択肢 1: drop as Invalid (現行挙動)

- 利点:
  - 「ASCII ローマ字 → かな」という本 converter の型的契約に沿う (境界入力は拒否)。
  - upstream で誤ってかなが convert 側に流入した場合、症状が見える形で現れる (fail-fast)。
  - Mozc / Google IME / MS-IME の慣習と一致する (これらの IME ではかなを romaji converter に再投入しない)。
  - 実装が単純で、state machine の推論が容易。
- 欠点:
  - `convert` の strict retraction 性が成立しない (committed が drop によって空になる)。property test は pending 側の invariant のみに制限される。
  - 将来 `convert` を再帰的に適用するユースケースが出てきた場合、想定外の情報損失になりうる。

### 選択肢 2: pass-through unchanged

- 利点:
  - `convert(convert(x).committed).committed == convert(x).committed` の strict retraction が成立し、property test を数学的にきれいな形で記述できる。
  - `convert` が「部分関数」ではなく「全域関数」として振る舞うため、呼び出し側がガードを意識しなくて済む。
- 欠点:
  - 「ASCII ローマ字 → かな」という契約を弱め、非 ASCII が transparent に通過するため境界混入バグを隠蔽しうる。
  - 実装に新しい分岐が必要 (非 ASCII を buffer に入れず直接 committed として emit する経路)。state machine の invariant 「buffer is ASCII-only」が影響を受けないが、`PushResult` に新 variant (`PassThrough(char)` 等) を追加する必要が出る。
  - 既存 `PushResult::Invalid(char)` を使って上位層に「想定外入力あり」の signal を送っている契約が崩れる。
  - 実際の IME 用途では発生しないケース (ASCII ローマ字 stream の中にかなが混入するのは異常系) のために実装負荷を払うことになる。

### 選択肢 3: 上位層で非 ASCII を事前フィルタ

- 利点:
  - `StateMachine::push` は純粋 ASCII 契約のまま保たれる。
  - 非 ASCII を見た段階で呼び出し側がエラー処理を選べる (drop / passthrough / reject)。
- 欠点:
  - `RomajiConverter::convert` / `push` / `flush` のいずれもが明示的なフィルタ層を挟む必要があり、公開 API に複雑性を追加する。
  - Phase 0 の対象ユースケース (CLI `kotoha-romaji`) ではフィルタの必要性が発生しない。

## 決定

**選択肢 1 (drop as Invalid)** を採用する。現行の `StateMachine::push` ガードはそのまま維持し、spec §9.2 で normative 挙動として明記する。

property test `prop_idempotence_on_committed` は弱化形 (`pending_second.is_empty()`) のままとし、strict retraction は要求しない。本プロパティが実際に保証するのは「かな出力を再度 `convert` に通しても新たな romaji pending は生成されない」という retraction 性の本質部分であり、数学的 retraction (全変数での strict equality) は本 IME の契約外であると整理する。

## 影響

### 実装への影響

- コード変更なし。`crates/kotoha-core/src/romaji/state.rs::push` の `if !ch.is_ascii() { return PushResult::Invalid(ch); }` ガード (state.rs:99-101) を normative として保持する。
- `RomajiConverter::convert` / `flush` の挙動変更なし。

### spec への影響

- `docs/superpowers/specs/2026-04-22-kotoha-phase-0-design.md` に §9.2 を新設し、非 ASCII 入力の扱いを記述する (本 PR で追加)。

### property test への影響

- `crates/kotoha-core/tests/romaji_property.rs` の `prop_idempotence_on_committed` は現行の弱化形で確定。
- 戦略 `romaji_input` は本 ADR 確定により non-ASCII 出力を含む入力 (句読点など plan 原型の `[a-z\-'.,!?\[\]/]{0,12}`) への拡大が可能となる (非 ASCII 再入力時に drop されることが normative に pin されたため、弱化 idempotence property は成立する)。strategy broaden は follow-up で実施予定 (ISSUE #26 と合流)。

### 将来の Phase 1 以降への影響

- かな漢字変換層を後段に追加する際、romaji 層 → かな層 → 漢字層 の単方向データフローを維持する (かな漢字変換層の出力が再度 romaji 層に戻ることはない設計を前提とする)。
- 設定オプションなどで「かなを直接 committed に pass-through する」動作が必要になった場合は、本 ADR を再検討し、選択肢 2 または 3 への switch を別 ADR で議論する。

## 参照

- ISSUE #22 — 本 ADR で close。
- PR #24 (M3b) — 本 ADR が必要となった契機。
- PR #27 (#23 hotfix) — `convert` 終端 buffer 正規化 (本 ADR とは独立だが、兄弟的な state machine 改善)。
- spec §9.2 (本 PR で新設) — normative 挙動の記述。
- `crates/kotoha-core/src/romaji/state.rs:98-101` — 現行実装。
