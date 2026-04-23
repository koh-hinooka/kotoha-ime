# 0005: ローマ字 lookup に HashMap ではなくカスタム Trie を採用する

## ステータス

承認 (2026-04-23)

## コンテキスト

Kotoha のローマ字→かな変換層は M3a で 207 エントリのルール表を実装した (`crates/kotoha-core/src/romaji/rules.rs`)。`StateMachine::push(char)` は streaming 変換であり、各入力後に「pending buffer = 完全 rule 一致 / rule の proper prefix のみ / どちらでもない」の 3-way 判定を一度の lookup で決定する必要がある (spec §9.3 pending バッファの backtrack 規則)。採用データ構造は、この 3-way lookup と、ルール表 `&'static str` の寿命、将来的な OnceLock キャッシュ化 (ISSUE #19) を同時に満たす必要がある。

## 検討した選択肢

### 選択肢 1: `HashMap<&'static str, &'static str>` + 別途 prefix set

- 利点: 標準ライブラリのみで済み、実装工数が最小。
- 欠点: 完全一致と prefix 判定で 2 回 lookup が必要。prefix set を別管理すると 2 データ構造の整合性コストが発生し、ルール追加のたびに両方を更新する boilerplate が増える。

### 選択肢 2: カスタム Trie (採用)

- 利点: ノード 1 走査で `Match | Partial | None` を得られる (`Trie::lookup`)。`&'static str` からそのまま構築でき、ISSUE #19 の OnceLock キャッシュと互換。ルール追加は `RULES` 配列だけで完結し、Trie は `from_rules()` で自動構築される。
- 欠点: HashMap 案比で実装コードが 50 行ほど多い。ノード毎の HashMap により間接参照が増える。

### 選択肢 3: `phf` crate による compile-time perfect hash

- 利点: compile 時ハッシュ化によりランタイム構築コストがゼロ。
- 欠点: prefix 判定を別データ構造に分離せざるを得ず、3-way lookup の「1 回走査」性を満たせない。build-time code gen の複雑性も Phase 0 に不要。Phase 3 perf tuning 時に再評価候補として deferring する。

## 決定

**選択肢 2 (カスタム Trie)** を採用する。`crates/kotoha-core/src/romaji/trie.rs` に `Trie` と 3-way 分類 `Lookup::{Match, Partial, None}` を実装し、`StateMachine::settle` から単一の `trie.lookup(buffer)` 呼び出しで backtrack 判定を駆動する。

## 影響

### 実装への影響

- `StateMachine::push` / `normalize` は単一の `trie.lookup` 呼び出しで次状態を決定できる (spec §9.3 で normative 化)。
- Trie は `pub(crate)` に閉じる。将来データ構造を差し替えても `Lookup` 3-way インタフェースを維持すれば呼び出し側は無変更。

### パフォーマンスへの影響

- 初期実装は `StateMachine::new` ごとに Trie を per-call rebuild する。207 エントリでは実測問題なし。OnceLock キャッシュ化は ISSUE #19 で別途 tracking する。

### 将来への影響

- Phase 3 perf tuning で `phf` 再評価の余地がある。`Lookup::{Match, Partial, None}` インタフェースを維持すれば実装差し替え可能。

## 参照

- `crates/kotoha-core/src/romaji/trie.rs` — 本 ADR が決定した Trie 実装。
- `crates/kotoha-core/src/romaji/rules.rs` — 207 エントリのルール表。
- `crates/kotoha-core/src/romaji/state.rs` — Trie の呼び出し元 (`StateMachine::settle` / `normalize`)。
- `docs/superpowers/specs/2026-04-22-kotoha-phase-0-design.md` §9 / §9.3。
- ISSUE #19 — OnceLock キャッシュ最適化 (deferred)。
- ISSUE #20 — 本 ADR を新設する docs-only ISSUE。
- PR #18 architecture review (Low finding) — 本 ADR 新設の発端。
