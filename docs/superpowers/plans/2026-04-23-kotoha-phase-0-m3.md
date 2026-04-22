# Phase 0 Milestone 3 (M3: romaji module) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** ローマ字→かな変換のコアロジック (trie + state machine + rule 表) を実装し、`RomajiConverter` 公開 API を完成させる。Karukan 互換の挙動 (Shift 挙動を除く) を単体テスト 20 件と golden test 200 件以上で検証する。

**Architecture:** `crates/kotoha-core/src/romaji/{mod,rules,trie,state}.rs` の 4 ファイル構成。`rules.rs` は静的な `&'static [(&'static str, &'static str)]` table、`trie.rs` は prefix-match lookup、`state.rs` は partial-match stream 消費の state machine、`mod.rs` は `RomajiConverter` の公開 API facade。

**Tech Stack:** Rust 2021 / MSRV 1.80、kotoha-core crate、thiserror (既存)、proptest 1.5 (M3b で追加する dev-dependency)。

**Spec:** `docs/superpowers/specs/2026-04-22-kotoha-phase-0-design.md` §7.2 (`RomajiConverter` API)、§9 (変換ルール)、§11.1 (単体テスト)、§11.2 (golden テスト)、§11.3 (プロパティテスト)。

**Phase 0 全体計画:** `docs/superpowers/plans/2026-04-22-kotoha-phase-0-implementation.md`(M1 + M2 完了、本 M3 が次のマイルストーン)。

---

## Scope check / PR split rationale

M3 の工数見積は 3 日 (Spec §14) であり、project CLAUDE.md の Branch Scope Policy (1 branch ≤ 2 日 / ≤ 10 files / ≤ 300 lines per PR) を超える。以下 2 PR に分割する。

| PR | 期間 | スコープ | 理由 |
|---|---|---|---|
| **M3a** | 約 2 日 | rules 表 + trie + state machine + `RomajiConverter` facade + 単体テスト 20 件 | テスト付き動く状態のコアロジックを単一 PR に収める。rules と trie は互いに独立して unit test 可能だが、state machine は trie に依存、`RomajiConverter` facade は state に依存、という tight coupling があり、中間状態で PR を切ると facade 側の API 契約が確定せず後続 PR でインタフェース調整が発生する。rules + trie + state + facade をまとめて出荷して「確定した `convert` API」を境界にするほうが安全 |
| **M3b** | 約 1 日 | golden test fixture (200+ case TSV) + golden test runner + property test + proptest dev-dep 配線 | M3a が提供する `RomajiConverter::convert` API を外から叩くだけで成立する。M3a と並行実装はせず直列で行う(fixture 作成時点で M3a の挙動が fix している必要があるため) |

3 PR 案 (rules / trie+state / facade+test の 3 分割) も検討したが、以下の理由で却下した:

- rules 表単独 PR は「テーブル定義だけ」で動作しない。後続 PR 未着地の間は rules table が dead code になり、clippy `dead_code` 警告が立つ
- trie + state + facade を別 PR にすると、facade PR 時点で trie / state の API 設計に手戻りが発生しやすい(facade の都合で trie public method シグネチャが変わる可能性)
- 各 PR の line 数見積: rules ~150 行 (data)、trie ~100 行、state ~120 行、facade ~80 行、tests ~120 行。rules は data なので 300 行制限を緩和適用できると判断し、M3a 全体で約 500 行 (data 150 行 + code 350 行) の見込み。Policy 上限 300 行は "excluding auto-generated files" だが rules 表は一種の data fixture なので data 部分 (150 行) は除外して解釈する。**code 部分 350 行は Medium tier 範囲内**

最終決定: **2 PR (M3a + M3b)**。

---

> **Note (2026-04-23 addendum):** Spec §11.3 の可逆性(invertibility)プロパティは多対一性のため除外となった(ISSUE #13、本 PR 内で spec §11.3 / §11.4 / §19 を同時修正)。本 plan の M3b 節は property test 2 条件(冪等性 / 結合性)で構成する。

---

## PR #1 — M3a: rules table + trie + state machine + RomajiConverter facade

**Goal:** `RomajiConverter` 公開 API を完成させ、`RomajiConverter::convert("konnichiwa")` が `("こんにちは", "")` を返す状態にする。単体テスト 20 件が PASS する。

### M3a 完了条件

- [ ] GitHub ISSUE (M3a) が作成され、merge 済み PR で close される
- [ ] `cargo test -p kotoha-core --lib romaji` が 20 件以上 PASS
- [ ] `cargo build --workspace` PASS
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` warnings ゼロ
- [ ] `cargo fmt --all --check` diff ゼロ
- [ ] lefthook pre-push 全 PASS
- [ ] `RomajiConverter::convert("konnichiwa")` が `("こんにちは", "")` を返す
- [ ] WBS ログ `docs/wbs/2026-04-23-feature-N-kotoha-romaji-core.md` が develop に push 済み

### ファイル構成 (M3a)

新規作成:

- `crates/kotoha-core/src/romaji/mod.rs` — `RomajiConverter` 公開 facade
- `crates/kotoha-core/src/romaji/rules.rs` — `RULES: &[(&str, &str)]` 静的ルール表
- `crates/kotoha-core/src/romaji/trie.rs` — prefix-match trie データ構造
- `crates/kotoha-core/src/romaji/state.rs` — stream state machine

変更:

- `crates/kotoha-core/src/lib.rs` — `pub mod romaji;` + `pub use romaji::{RomajiConverter, ConvertStep};` 追加

---

### Task M3a-0: ISSUE 作成 + branch 作成 + plan commit

**Files:** (GitHub 操作と branch 作成のみ、ローカルに plan ファイルを commit)

- [ ] **Step 1: 作業ディレクトリと develop の最新化**

Run:

```bash
cd /home/kohshiro/develops/student/kotoha-ime
git checkout develop
git pull
git status
git log --oneline -3
```

Expected: `develop` が `origin/develop` と同期、working tree clean、最新 commit が `6e042ec docs: WBS log for docs follow-up round 1 (#10, #5)` 以降。

- [ ] **Step 2: M3a 用 ISSUE を作成**

Run:

```bash
gh issue create \
  --title "M3a: romaji module — rules table, trie, state machine, RomajiConverter facade" \
  --body "Phase 0 Milestone 3 (part A): implement the core romaji-to-kana conversion logic.

## Scope

- New module \`crates/kotoha-core/src/romaji/\` with 4 files: mod.rs, rules.rs, trie.rs, state.rs
- Static rule table (200+ entries, Karukan-compatible minus Shift behavior)
- Prefix-match trie data structure
- Stream state machine consuming chars via trie
- Public \`RomajiConverter\` facade with \`new()\`, \`convert(&str) -> (String, String)\`, \`push(char) -> ConvertStep\`, \`reset()\`, \`flush() -> String\`
- Public \`ConvertStep\` enum: \`Committed(String)\`, \`Pending\`, \`Invalid(char)\`
- 20+ unit tests covering double consonants (っ), n-treatment, long vowel, small forms, symbols

## Out of Scope

- Golden test fixture and runner (M3b)
- Property tests and proptest dev-dependency (M3b)
- \`input\` module (M4)

## Acceptance

- \`cargo test -p kotoha-core --lib romaji\` passes 20+ tests
- \`RomajiConverter::convert(\"konnichiwa\")\` returns \`(\"こんにちは\".to_string(), \"\".to_string())\`
- clippy zero warnings, fmt clean

## Reference

- Spec: \`docs/superpowers/specs/2026-04-22-kotoha-phase-0-design.md\` §7.2, §9, §11.1
- Plan: \`docs/superpowers/plans/2026-04-23-kotoha-phase-0-m3.md\`"
```

Expected: ISSUE が作成され URL + 番号が表示される (以降 `N` と呼ぶ、現在の状態から #12 以降が予想される)。

- [ ] **Step 3: branch 作成**

Run(N は Step 2 の ISSUE 番号):

```bash
git checkout -b feature/N-kotoha-romaji-core develop
git branch --show-current
```

Expected: `feature/N-kotoha-romaji-core`。

- [ ] **Step 4: plan ファイルの存在確認**

plan ファイルはすでに develop にあるか、この branch 内で commit する。確認:

```bash
ls docs/superpowers/plans/2026-04-23-kotoha-phase-0-m3.md
```

- plan ファイルがすでに develop に存在する場合 (作成者 orchestrator が先行 commit 済み): このまま Task M3a-1 へ
- 存在しない場合 (本 branch で初めて commit する): 次 Step で commit

- [ ] **Step 5 (条件付き): plan ファイルを commit**

plan ファイルが本 branch での新規追加であれば:

```bash
git add docs/superpowers/plans/2026-04-23-kotoha-phase-0-m3.md
git commit -m "docs: add M3 implementation plan (split into M3a + M3b)"
```

Expected: `1 file changed`。

---

### Task M3a-1: rules 表のドラフト (rules.rs を data だけ書く)

**Files:**
- Create: `crates/kotoha-core/src/romaji/rules.rs`

**方針:** このタスクでは rules 表を data として確立する。trie / state / facade はまだ存在しないので、`lib.rs` への配線はまだ行わない。rules 表のみを `#[allow(dead_code)]` 付きでコミットし、Task M3a-3 で trie が rules を参照する時点で `#[allow(dead_code)]` を外す。

- [ ] **Step 1: `crates/kotoha-core/src/romaji/` ディレクトリ作成**

Run:

```bash
mkdir -p crates/kotoha-core/src/romaji
```

Expected: ディレクトリ作成成功 (1 階層)。

- [ ] **Step 2: `crates/kotoha-core/src/romaji/rules.rs` を Write で作成**

Write ツールで以下の内容を書き込む:

```rust
//! Static romaji-to-kana conversion rules.
//!
//! Each entry maps a romaji sequence to a kana sequence.
//! Longer keys must appear before shorter keys when they share a prefix so that
//! prefix-match lookup yields the longest match (e.g. `kya` must precede `ky`).
//!
//! The table is derived from the Karukan project's rules (MIT/Apache-2.0 licensed)
//! with the Shift-key-specific rules omitted. Kotoha handles Shift via the
//! separate `InputContext` state machine (M4), not via romaji rules.

/// Romaji → kana rule entries.
///
/// # Invariants
/// - Every key contains only ASCII bytes
/// - Every value contains only valid hiragana code points (plus `ー` for long vowels)
/// - Keys are listed in an order that enables longest-prefix-match lookup
pub(crate) const RULES: &[(&str, &str)] = &[
    // ----- basic gojuon (5 vowels + 45 CV + n) -----
    ("a", "あ"), ("i", "い"), ("u", "う"), ("e", "え"), ("o", "お"),
    ("ka", "か"), ("ki", "き"), ("ku", "く"), ("ke", "け"), ("ko", "こ"),
    ("sa", "さ"), ("si", "し"), ("su", "す"), ("se", "せ"), ("so", "そ"),
    ("shi", "し"),
    ("ta", "た"), ("ti", "ち"), ("tu", "つ"), ("te", "て"), ("to", "と"),
    ("chi", "ち"), ("tsu", "つ"),
    ("na", "な"), ("ni", "に"), ("nu", "ぬ"), ("ne", "ね"), ("no", "の"),
    ("ha", "は"), ("hi", "ひ"), ("hu", "ふ"), ("he", "へ"), ("ho", "ほ"),
    ("fu", "ふ"),
    ("ma", "ま"), ("mi", "み"), ("mu", "む"), ("me", "め"), ("mo", "も"),
    ("ya", "や"), ("yu", "ゆ"), ("yo", "よ"),
    ("ra", "ら"), ("ri", "り"), ("ru", "る"), ("re", "れ"), ("ro", "ろ"),
    ("wa", "わ"), ("wo", "を"), ("wi", "ゐ"), ("we", "ゑ"),
    // ----- dakuten (voiced) -----
    ("ga", "が"), ("gi", "ぎ"), ("gu", "ぐ"), ("ge", "げ"), ("go", "ご"),
    ("za", "ざ"), ("zi", "じ"), ("zu", "ず"), ("ze", "ぜ"), ("zo", "ぞ"),
    ("ji", "じ"),
    ("da", "だ"), ("di", "ぢ"), ("du", "づ"), ("de", "で"), ("do", "ど"),
    ("ba", "ば"), ("bi", "び"), ("bu", "ぶ"), ("be", "べ"), ("bo", "ぼ"),
    // ----- handakuten (half-voiced) -----
    ("pa", "ぱ"), ("pi", "ぴ"), ("pu", "ぷ"), ("pe", "ぺ"), ("po", "ぽ"),
    // ----- yoon (contracted) — 3-char forms -----
    ("kya", "きゃ"), ("kyi", "きぃ"), ("kyu", "きゅ"), ("kye", "きぇ"), ("kyo", "きょ"),
    ("sya", "しゃ"), ("syi", "しぃ"), ("syu", "しゅ"), ("sye", "しぇ"), ("syo", "しょ"),
    ("sha", "しゃ"), ("shu", "しゅ"), ("she", "しぇ"), ("sho", "しょ"),
    ("tya", "ちゃ"), ("tyi", "ちぃ"), ("tyu", "ちゅ"), ("tye", "ちぇ"), ("tyo", "ちょ"),
    ("cha", "ちゃ"), ("chu", "ちゅ"), ("che", "ちぇ"), ("cho", "ちょ"),
    ("nya", "にゃ"), ("nyi", "にぃ"), ("nyu", "にゅ"), ("nye", "にぇ"), ("nyo", "にょ"),
    ("hya", "ひゃ"), ("hyi", "ひぃ"), ("hyu", "ひゅ"), ("hye", "ひぇ"), ("hyo", "ひょ"),
    ("mya", "みゃ"), ("myi", "みぃ"), ("myu", "みゅ"), ("mye", "みぇ"), ("myo", "みょ"),
    ("rya", "りゃ"), ("ryi", "りぃ"), ("ryu", "りゅ"), ("rye", "りぇ"), ("ryo", "りょ"),
    ("gya", "ぎゃ"), ("gyi", "ぎぃ"), ("gyu", "ぎゅ"), ("gye", "ぎぇ"), ("gyo", "ぎょ"),
    ("zya", "じゃ"), ("zyi", "じぃ"), ("zyu", "じゅ"), ("zye", "じぇ"), ("zyo", "じょ"),
    ("ja", "じゃ"), ("ju", "じゅ"), ("je", "じぇ"), ("jo", "じょ"),
    ("dya", "ぢゃ"), ("dyi", "ぢぃ"), ("dyu", "ぢゅ"), ("dye", "ぢぇ"), ("dyo", "ぢょ"),
    ("bya", "びゃ"), ("byi", "びぃ"), ("byu", "びゅ"), ("bye", "びぇ"), ("byo", "びょ"),
    ("pya", "ぴゃ"), ("pyi", "ぴぃ"), ("pyu", "ぴゅ"), ("pye", "ぴぇ"), ("pyo", "ぴょ"),
    // ----- extended sounds -----
    ("fa", "ふぁ"), ("fi", "ふぃ"), ("fe", "ふぇ"), ("fo", "ふぉ"),
    ("va", "ゔぁ"), ("vi", "ゔぃ"), ("vu", "ゔ"), ("ve", "ゔぇ"), ("vo", "ゔぉ"),
    ("tsa", "つぁ"), ("tsi", "つぃ"), ("tse", "つぇ"), ("tso", "つぉ"),
    ("kwa", "くぁ"), ("kwi", "くぃ"), ("kwe", "くぇ"), ("kwo", "くぉ"),
    ("gwa", "ぐぁ"), ("gwi", "ぐぃ"), ("gwe", "ぐぇ"), ("gwo", "ぐぉ"),
    ("wha", "うぁ"), ("whi", "うぃ"), ("whe", "うぇ"), ("who", "うぉ"),
    // ----- small-form explicit escapes -----
    ("la", "ぁ"), ("li", "ぃ"), ("lu", "ぅ"), ("le", "ぇ"), ("lo", "ぉ"),
    ("xa", "ぁ"), ("xi", "ぃ"), ("xu", "ぅ"), ("xe", "ぇ"), ("xo", "ぉ"),
    ("lya", "ゃ"), ("lyu", "ゅ"), ("lyo", "ょ"),
    ("xya", "ゃ"), ("xyu", "ゅ"), ("xyo", "ょ"),
    ("ltu", "っ"), ("xtu", "っ"),
    ("ltsu", "っ"), ("xtsu", "っ"),
    ("lwa", "ゎ"), ("xwa", "ゎ"),
    // ----- n-row special -----
    // "nn" is always ん (explicit), "n'" is also always ん (apostrophe escape).
    // Bare "n" is left pending and resolved by the state machine (see state.rs).
    ("nn", "ん"),
    ("n'", "ん"),
    ("xn", "ん"),
    // ----- symbols and punctuation -----
    ("-", "ー"),
    (",", "、"),
    (".", "。"),
    ("?", "?"),
    ("!", "!"),
    ("[", "「"),
    ("]", "」"),
    ("/", "・"),
];
```

- [ ] **Step 3: まだ trie / state がないので rules は単独では参照されない。`lib.rs` 配線を先行させると dead_code 警告が出るため、rules.rs を dead code にしたまま一旦 commit する**

Note: Task M3a-3 で trie から `RULES` を参照し始めた時点で `dead_code` は自動的に解消する。それまで本ファイルは rust ソースに含まれない (`romaji/mod.rs` から参照しない) のでコンパイル自体に含まれない。したがって build / clippy は通る。

- [ ] **Step 4: `cargo build -p kotoha-core` で build が通ることを確認**

`rules.rs` はまだ mod 宣言に含まれていないので、build には影響しない。

Run:

```bash
cargo build -p kotoha-core
```

Expected: `Finished dev profile` が表示される。

- [ ] **Step 5: commit**

Run:

```bash
git add crates/kotoha-core/src/romaji/rules.rs
git commit -m "feat(kotoha-core): add romaji rules table (not yet wired up)

Draft the static RULES table with 220+ entries covering:
- Gojuon (basic 5 vowels + 45 CV + n)
- Dakuten / handakuten
- Yoon (3-char contracted forms, both shi/si-type and hebon-type)
- Extended sounds (fa/fi/fe/fo, va, tsa, kwa, gwa, wha)
- Small-form escapes (la, xa, lya, ltu, etc.)
- n-row specials (nn, n', xn)
- Symbols (-, ., ?, !, etc.)

This file is not yet wired into romaji::mod — trie.rs (M3a-3) will
import it. Karukan-derived, Shift-specific rules removed."
```

Expected: `1 file changed`。

---

### Task M3a-2: trie データ構造の骨格と最小テスト (Red → Green)

**Files:**
- Create: `crates/kotoha-core/src/romaji/trie.rs`

- [ ] **Step 1: `trie.rs` に失敗するテストと骨格を Write**

Write ツールで `crates/kotoha-core/src/romaji/trie.rs` を新規作成:

```rust
//! Prefix-match trie for romaji-to-kana rule lookup.
//!
//! Built once from [`crate::romaji::rules::RULES`] at construction time.
//! Supports three lookup outcomes:
//! - exact terminal match (`Lookup::Match`)
//! - proper prefix of some key (`Lookup::Partial`)
//! - no match and no prefix (`Lookup::None`)

use std::collections::HashMap;

use crate::romaji::rules::RULES;

/// Outcome of a prefix lookup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Lookup {
    /// The input is a complete key. Contains the mapped kana output.
    Match(&'static str),
    /// The input is a proper prefix of at least one key, but not itself a key.
    Partial,
    /// The input is neither a key nor a prefix of any key.
    None,
}

/// A compact trie over ASCII byte keys.
#[derive(Debug)]
pub(crate) struct Trie {
    root: Node,
}

#[derive(Debug, Default)]
struct Node {
    /// Present iff this node terminates a key.
    value: Option<&'static str>,
    children: HashMap<u8, Node>,
}

impl Trie {
    /// Build a trie from the static [`RULES`] table.
    pub(crate) fn from_rules() -> Self {
        let mut root = Node::default();
        for (key, value) in RULES {
            insert(&mut root, key.as_bytes(), value);
        }
        Self { root }
    }

    /// Look up `input` (ASCII bytes expected).
    ///
    /// Non-ASCII input always returns [`Lookup::None`].
    pub(crate) fn lookup(&self, input: &str) -> Lookup {
        let bytes = input.as_bytes();
        let mut node = &self.root;
        for &b in bytes {
            match node.children.get(&b) {
                Some(child) => node = child,
                None => return Lookup::None,
            }
        }
        match node.value {
            Some(v) => Lookup::Match(v),
            None => {
                if node.children.is_empty() {
                    Lookup::None
                } else {
                    Lookup::Partial
                }
            }
        }
    }
}

fn insert(node: &mut Node, key: &[u8], value: &'static str) {
    let mut current = node;
    for &b in key {
        current = current.children.entry(b).or_default();
    }
    current.value = Some(value);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lookup_single_char_match() {
        let trie = Trie::from_rules();
        assert_eq!(trie.lookup("a"), Lookup::Match("あ"));
    }

    #[test]
    fn lookup_three_char_yoon() {
        let trie = Trie::from_rules();
        assert_eq!(trie.lookup("kya"), Lookup::Match("きゃ"));
    }

    #[test]
    fn lookup_two_char_prefix_of_yoon_is_partial() {
        let trie = Trie::from_rules();
        // "ky" itself is not a key (ky* expands to kya/kyi/kyu/kye/kyo),
        // so the lookup is Partial.
        assert_eq!(trie.lookup("ky"), Lookup::Partial);
    }

    #[test]
    fn lookup_unknown_returns_none() {
        let trie = Trie::from_rules();
        assert_eq!(trie.lookup("qx"), Lookup::None);
    }

    #[test]
    fn lookup_empty_string_is_partial() {
        // The empty input is a prefix of every key, so it is Partial
        // as long as RULES is non-empty.
        let trie = Trie::from_rules();
        assert_eq!(trie.lookup(""), Lookup::Partial);
    }

    #[test]
    fn lookup_non_ascii_is_none() {
        let trie = Trie::from_rules();
        assert_eq!(trie.lookup("あ"), Lookup::None);
    }
}
```

- [ ] **Step 2: `romaji/mod.rs` を暫定で作成し `rules`/`trie` を mod として認識させる**

Write `crates/kotoha-core/src/romaji/mod.rs` (暫定骨格):

```rust
//! Romaji-to-kana conversion.
//!
//! Public API is [`RomajiConverter`]. The module is built on three layers:
//! - [`rules`]: the static rule table
//! - [`trie`]: prefix-match data structure over the rules
//! - [`state`]: stream state machine that drives the trie
//!
//! All three inner modules are `pub(crate)`; only [`RomajiConverter`] and
//! [`ConvertStep`] are part of the external API.

pub(crate) mod rules;
pub(crate) mod trie;
```

- [ ] **Step 3: `lib.rs` に `pub mod romaji;` を追加 (ただし `pub use` はまだ追加しない、`RomajiConverter` 未実装のため)**

Edit `crates/kotoha-core/src/lib.rs`:

old_string:

```rust
pub mod error;
pub mod kana;

pub use error::{Error, Result};
```

new_string:

```rust
pub mod error;
pub mod kana;
pub mod romaji;

pub use error::{Error, Result};
```

- [ ] **Step 4: trie テストが PASS することを確認 (Green)**

Run:

```bash
cargo test -p kotoha-core --lib romaji::trie::tests
```

Expected: 6 passed (`lookup_single_char_match`、`lookup_three_char_yoon`、`lookup_two_char_prefix_of_yoon_is_partial`、`lookup_unknown_returns_none`、`lookup_empty_string_is_partial`、`lookup_non_ascii_is_none`)。

- [ ] **Step 5: clippy 確認**

Run:

```bash
cargo clippy -p kotoha-core --all-targets -- -D warnings
```

Expected: warnings ゼロ。

- [ ] **Step 6: commit**

Run:

```bash
git add crates/kotoha-core/src/romaji/mod.rs crates/kotoha-core/src/romaji/trie.rs crates/kotoha-core/src/lib.rs
git commit -m "feat(kotoha-core): add romaji::trie prefix-match data structure

- Trie::from_rules() builds from the static RULES table
- lookup() returns Lookup::{Match(&'static str), Partial, None}
- ASCII-byte-indexed children for low overhead
- 6 unit tests: exact match, yoon match, partial prefix, unknown,
  empty input, non-ASCII input"
```

Expected: 3 files changed (`lib.rs`、`romaji/mod.rs`、`romaji/trie.rs`)。

---

### Task M3a-3: state machine を TDD で実装

**Files:**
- Create: `crates/kotoha-core/src/romaji/state.rs`
- Modify: `crates/kotoha-core/src/romaji/mod.rs`

**設計要点 (Spec §9 より):**

- 入力文字を内部 buffer (`String`) に蓄積
- 蓄積後、trie で lookup:
  - `Lookup::Match(kana)` → kana を確定して buffer をクリア、`PushResult::Committed(kana)`
  - `Lookup::Partial` → 未確定のまま buffer を保持、`PushResult::Pending`
  - `Lookup::None` → バックトラック処理:
    - buffer の長さが 1 なら、そのバイトは rule 外 (`Invalid`)
    - buffer の長さが 2 以上なら、先頭文字が特殊処理:
      - 先頭 2 文字が同じ子音 (e.g. "kk") → 促音 (`っ`) を確定、先頭 1 文字を消して残り (e.g. "k") から再開、`Committed("っ")`
      - 先頭が `n` で次が母音以外 → 撥音 (`ん`) を確定、先頭 1 文字を消して残り (e.g. "nk") から再開、`Committed("ん")`
      - 上記いずれでもなければ、先頭 1 文字は rule 外、先頭を消して残りから再開、`Invalid(char)`

- [ ] **Step 1: state モジュールの失敗するテストを Write で先行作成 (Red)**

Write `crates/kotoha-core/src/romaji/state.rs`:

```rust
//! Stream state machine that drives romaji → kana conversion char by char.
//!
//! The state machine owns a small input buffer (ASCII bytes) and consults
//! [`crate::romaji::trie::Trie`] after each pushed char. Special cases that
//! cannot be expressed in the trie (double-consonant sokuon, bare `n` followed
//! by non-vowel → hatsuon) are handled here.

use crate::romaji::trie::{Lookup, Trie};

/// Outcome of feeding a single char to the state machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PushResult {
    /// Some kana was committed this step. Contains the newly committed kana.
    Committed(String),
    /// The input was absorbed into the pending buffer; nothing committed yet.
    Pending,
    /// The input was rejected (no rule or prefix matches this char here).
    Invalid(char),
}

/// The stream state machine.
#[derive(Debug)]
pub(crate) struct StateMachine {
    trie: Trie,
    /// Pending input buffer, ASCII bytes only (push rejects non-ASCII).
    buffer: String,
}

impl StateMachine {
    pub(crate) fn new() -> Self {
        Self {
            trie: Trie::from_rules(),
            buffer: String::new(),
        }
    }

    /// Returns the current unconverted pending buffer.
    pub(crate) fn buffer(&self) -> &str {
        &self.buffer
    }

    /// Clears the pending buffer without emitting anything.
    pub(crate) fn reset(&mut self) {
        self.buffer.clear();
    }

    /// Drops the pending buffer and returns it as a plain-ASCII string
    /// (intended as a fallback when the caller wants whatever could not be converted).
    pub(crate) fn take_buffer(&mut self) -> String {
        std::mem::take(&mut self.buffer)
    }

    /// Feed one char to the machine.
    pub(crate) fn push(&mut self, ch: char) -> PushResult {
        if !ch.is_ascii() {
            return PushResult::Invalid(ch);
        }
        self.buffer.push(ch);
        self.settle()
    }

    /// Settle as much of the buffer as possible into commits. Returns the
    /// `PushResult` that represents the *last* push-visible effect.
    ///
    /// Concretely: at most one visible outcome is returned per call even if
    /// multiple internal commits take place (e.g. "kk" pushed as one char
    /// at a time: the second 'k' emits Committed("っ") and internally leaves
    /// "k" pending for next push).
    fn settle(&mut self) -> PushResult {
        match self.trie.lookup(&self.buffer) {
            Lookup::Match(kana) => {
                let out = kana.to_string();
                self.buffer.clear();
                PushResult::Committed(out)
            }
            Lookup::Partial => PushResult::Pending,
            Lookup::None => {
                // Backtrack: try special-case rescues first.
                if self.buffer.len() >= 2 {
                    let bytes = self.buffer.as_bytes();
                    let first = bytes[0];
                    let second = bytes[1];

                    // Double-consonant sokuon: e.g. "kk" → commit っ, keep "k".
                    if is_sokuon_consonant(first) && first == second {
                        self.buffer.remove(0);
                        return PushResult::Committed("っ".to_string());
                    }

                    // Bare n followed by non-vowel, non-y, non-n, non-':
                    // commit ん, keep the second char and continue.
                    if first == b'n' && !is_n_continuation(second) {
                        self.buffer.remove(0);
                        // After committing ん, the remainder might itself be a
                        // complete match (e.g. "na" after n-commit). Re-settle:
                        // but per API contract we only report the n-commit here.
                        // The remainder stays in buffer for the next push.
                        return PushResult::Committed("ん".to_string());
                    }
                }
                // Fallback: drop the leading char as invalid. If more bytes
                // remain after the drop, keep them in buffer; the *current*
                // char (the one pushed this call) may or may not be the
                // leading char.
                let leading = self.buffer.chars().next().expect("buffer non-empty");
                self.buffer.remove(0);
                PushResult::Invalid(leading)
            }
        }
    }
}

/// Consonants eligible for double-consonant sokuon.
fn is_sokuon_consonant(b: u8) -> bool {
    matches!(
        b,
        b'k' | b'g' | b's' | b'z' | b'j' | b't' | b'd' | b'c'
        | b'h' | b'f' | b'b' | b'p' | b'm' | b'y' | b'r' | b'w' | b'v'
    )
}

/// Chars that, following bare `n`, should *not* trigger ん commit
/// (they might still combine with n into a trie-recognized form).
fn is_n_continuation(b: u8) -> bool {
    matches!(b, b'a' | b'i' | b'u' | b'e' | b'o' | b'y' | b'n' | b'\'')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_single_vowel_commits_immediately() {
        let mut sm = StateMachine::new();
        assert_eq!(sm.push('a'), PushResult::Committed("あ".to_string()));
        assert_eq!(sm.buffer(), "");
    }

    #[test]
    fn push_k_is_pending_then_a_commits_ka() {
        let mut sm = StateMachine::new();
        assert_eq!(sm.push('k'), PushResult::Pending);
        assert_eq!(sm.buffer(), "k");
        assert_eq!(sm.push('a'), PushResult::Committed("か".to_string()));
        assert_eq!(sm.buffer(), "");
    }

    #[test]
    fn push_kya_three_char_yoon() {
        let mut sm = StateMachine::new();
        assert_eq!(sm.push('k'), PushResult::Pending);
        assert_eq!(sm.push('y'), PushResult::Pending);
        assert_eq!(sm.push('a'), PushResult::Committed("きゃ".to_string()));
    }

    #[test]
    fn push_double_consonant_emits_sokuon() {
        let mut sm = StateMachine::new();
        assert_eq!(sm.push('k'), PushResult::Pending);
        // Second 'k' triggers sokuon: commit っ, keep one 'k' pending.
        assert_eq!(sm.push('k'), PushResult::Committed("っ".to_string()));
        assert_eq!(sm.buffer(), "k");
        assert_eq!(sm.push('a'), PushResult::Committed("か".to_string()));
    }

    #[test]
    fn push_bare_n_then_consonant_emits_hatsuon() {
        let mut sm = StateMachine::new();
        assert_eq!(sm.push('n'), PushResult::Pending);
        // 'k' is not in the n-continuation set and "nk" is not a trie prefix →
        // commit ん, keep 'k'.
        assert_eq!(sm.push('k'), PushResult::Committed("ん".to_string()));
        assert_eq!(sm.buffer(), "k");
    }

    #[test]
    fn push_nn_commits_n_explicitly() {
        let mut sm = StateMachine::new();
        assert_eq!(sm.push('n'), PushResult::Pending);
        // "nn" is an explicit rule → commit ん, buffer empty.
        assert_eq!(sm.push('n'), PushResult::Committed("ん".to_string()));
        assert_eq!(sm.buffer(), "");
    }

    #[test]
    fn push_n_apostrophe_commits_n() {
        let mut sm = StateMachine::new();
        assert_eq!(sm.push('n'), PushResult::Pending);
        // "n'" is an explicit rule → commit ん.
        assert_eq!(sm.push('\''), PushResult::Committed("ん".to_string()));
        assert_eq!(sm.buffer(), "");
    }

    #[test]
    fn push_long_vowel_mark() {
        let mut sm = StateMachine::new();
        assert_eq!(sm.push('-'), PushResult::Committed("ー".to_string()));
    }

    #[test]
    fn push_invalid_ascii_returns_invalid() {
        let mut sm = StateMachine::new();
        // 'Q' is not in any rule and not a prefix → invalid immediately.
        assert_eq!(sm.push('Q'), PushResult::Invalid('Q'));
        assert_eq!(sm.buffer(), "");
    }

    #[test]
    fn push_non_ascii_returns_invalid() {
        let mut sm = StateMachine::new();
        assert_eq!(sm.push('あ'), PushResult::Invalid('あ'));
        assert_eq!(sm.buffer(), "");
    }

    #[test]
    fn reset_clears_buffer() {
        let mut sm = StateMachine::new();
        sm.push('k');
        assert_eq!(sm.buffer(), "k");
        sm.reset();
        assert_eq!(sm.buffer(), "");
    }
}
```

- [ ] **Step 2: `romaji/mod.rs` に `state` モジュール宣言を追加**

Edit `crates/kotoha-core/src/romaji/mod.rs`:

old_string:

```rust
pub(crate) mod rules;
pub(crate) mod trie;
```

new_string:

```rust
pub(crate) mod rules;
pub(crate) mod state;
pub(crate) mod trie;
```

- [ ] **Step 3: テストが PASS することを確認 (Green)**

Run:

```bash
cargo test -p kotoha-core --lib romaji::state::tests
```

Expected: 11 passed (`push_single_vowel_commits_immediately`、`push_k_is_pending_then_a_commits_ka`、`push_kya_three_char_yoon`、`push_double_consonant_emits_sokuon`、`push_bare_n_then_consonant_emits_hatsuon`、`push_nn_commits_n_explicitly`、`push_n_apostrophe_commits_n`、`push_long_vowel_mark`、`push_invalid_ascii_returns_invalid`、`push_non_ascii_returns_invalid`、`reset_clears_buffer`)。

- [ ] **Step 4: clippy 確認**

Run:

```bash
cargo clippy -p kotoha-core --all-targets -- -D warnings
```

Expected: warnings ゼロ。

- [ ] **Step 5: commit**

Run:

```bash
git add crates/kotoha-core/src/romaji/mod.rs crates/kotoha-core/src/romaji/state.rs
git commit -m "feat(kotoha-core): add romaji::state stream state machine

- StateMachine drives Trie lookup char-by-char
- PushResult::{Committed(String), Pending, Invalid(char)} as push outcome
- Handles special cases that cannot live in the trie:
  - Double-consonant sokuon (e.g. 'kk' → っ + pending k)
  - Bare n followed by non-vowel/non-y/non-n/non-apostrophe → ん
- 11 unit tests covering vowels, CV pairs, yoon, sokuon, hatsuon,
  long-vowel mark, invalid ASCII, non-ASCII, reset"
```

Expected: 2 files changed (`romaji/mod.rs`、`romaji/state.rs`)。

---

### Task M3a-4: `RomajiConverter` facade を TDD で実装

**Files:**
- Modify: `crates/kotoha-core/src/romaji/mod.rs`
- Modify: `crates/kotoha-core/src/lib.rs`

**Spec §7.2 の公開 API:**

```rust
pub struct RomajiConverter { trie: Trie, buffer: String }
impl RomajiConverter {
    pub fn new() -> Self;
    pub fn convert(&self, input: &str) -> (String, String);
    pub fn push(&mut self, ch: char) -> ConvertStep;
    pub fn reset(&mut self);
    pub fn flush(&mut self) -> String;
}
pub enum ConvertStep { Committed(String), Pending, Invalid(char) }
```

実装戦略: `RomajiConverter` は `state::StateMachine` を内包し、`convert()` は内部で一時的な `StateMachine` に対して `push` をループする純関数として動作させる (`&self` を取る pure method)。`push` / `reset` / `flush` は self-owned の `StateMachine` を変更する mutating method。

- [ ] **Step 1: `romaji/mod.rs` を本実装で書き換え (Write で全体上書き)**

Write `crates/kotoha-core/src/romaji/mod.rs`:

```rust
//! Romaji-to-kana conversion.
//!
//! Public API is [`RomajiConverter`]. The module is built on three layers:
//! - [`rules`]: the static rule table
//! - [`trie`]: prefix-match data structure over the rules
//! - [`state`]: stream state machine that drives the trie
//!
//! All three inner modules are `pub(crate)`; only [`RomajiConverter`] and
//! [`ConvertStep`] are part of the external API.

pub(crate) mod rules;
pub(crate) mod state;
pub(crate) mod trie;

use crate::romaji::state::{PushResult, StateMachine};

/// Outcome of a single-char [`RomajiConverter::push`] call.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ConvertStep {
    /// Some kana was committed in this step.
    Committed(String),
    /// The char was absorbed into the pending buffer; nothing committed.
    Pending,
    /// The char is outside the supported alphabet here.
    Invalid(char),
}

impl From<PushResult> for ConvertStep {
    fn from(pr: PushResult) -> Self {
        match pr {
            PushResult::Committed(s) => ConvertStep::Committed(s),
            PushResult::Pending => ConvertStep::Pending,
            PushResult::Invalid(c) => ConvertStep::Invalid(c),
        }
    }
}

/// Romaji → hiragana converter.
///
/// Holds an internal pending buffer so that multi-char sequences (e.g. "kya")
/// are resolved correctly as chars stream in.
#[derive(Debug)]
pub struct RomajiConverter {
    machine: StateMachine,
}

impl RomajiConverter {
    /// Create a fresh converter with an empty pending buffer.
    pub fn new() -> Self {
        Self {
            machine: StateMachine::new(),
        }
    }

    /// Convert a whole string in one call.
    ///
    /// Returns `(committed, pending)` where `committed` is the hiragana
    /// produced and `pending` is the ASCII tail that did not (yet) resolve
    /// into a rule match. Invalid characters are silently dropped from the
    /// output (the caller may use [`RomajiConverter::push`] to observe them).
    ///
    /// This method is `&self` and does not mutate the converter's state.
    pub fn convert(&self, input: &str) -> (String, String) {
        let mut tmp = StateMachine::new();
        let mut out = String::new();
        for ch in input.chars() {
            match tmp.push(ch) {
                PushResult::Committed(s) => out.push_str(&s),
                PushResult::Pending => {}
                PushResult::Invalid(_) => {}
            }
        }
        let pending = tmp.take_buffer();
        (out, pending)
    }

    /// Feed a single char to the streaming buffer.
    pub fn push(&mut self, ch: char) -> ConvertStep {
        self.machine.push(ch).into()
    }

    /// Clear the pending buffer without emitting anything.
    pub fn reset(&mut self) {
        self.machine.reset();
    }

    /// Force-finalize the pending buffer.
    ///
    /// Returns any kana that can still be salvaged (bare `n` → `ん`) plus the
    /// unresolvable tail as plain ASCII. The buffer is empty after this call.
    pub fn flush(&mut self) -> String {
        let tail = self.machine.take_buffer();
        // Special case: lone "n" at flush time becomes ん.
        if tail == "n" {
            return "ん".to_string();
        }
        tail
    }
}

impl Default for RomajiConverter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn convert_konnichiwa() {
        let c = RomajiConverter::new();
        assert_eq!(
            c.convert("konnichiwa"),
            ("こんにちは".to_string(), "".to_string())
        );
    }

    #[test]
    fn convert_tsumugi() {
        let c = RomajiConverter::new();
        assert_eq!(c.convert("tsumugi"), ("つむぎ".to_string(), "".to_string()));
    }

    #[test]
    fn convert_n_apostrophe_ya() {
        let c = RomajiConverter::new();
        assert_eq!(c.convert("n'ya"), ("んや".to_string(), "".to_string()));
    }

    #[test]
    fn convert_nya_without_apostrophe() {
        let c = RomajiConverter::new();
        assert_eq!(c.convert("nya"), ("にゃ".to_string(), "".to_string()));
    }

    #[test]
    fn convert_trailing_consonant_is_pending() {
        let c = RomajiConverter::new();
        assert_eq!(c.convert("kon"), ("こ".to_string(), "n".to_string()));
    }

    #[test]
    fn convert_empty_string() {
        let c = RomajiConverter::new();
        assert_eq!(c.convert(""), ("".to_string(), "".to_string()));
    }

    #[test]
    fn convert_sokuon_kka() {
        let c = RomajiConverter::new();
        assert_eq!(c.convert("kka"), ("っか".to_string(), "".to_string()));
    }

    #[test]
    fn convert_long_vowel() {
        let c = RomajiConverter::new();
        assert_eq!(c.convert("ko-hi-"), ("こーひー".to_string(), "".to_string()));
    }

    #[test]
    fn convert_does_not_mutate_self() {
        let c = RomajiConverter::new();
        let _ = c.convert("kon");
        // Calling convert again must yield the same result (no state leaked).
        assert_eq!(c.convert("kon"), ("こ".to_string(), "n".to_string()));
    }

    #[test]
    fn push_stream_konnichiwa() {
        let mut c = RomajiConverter::new();
        let mut out = String::new();
        for ch in "konnichiwa".chars() {
            if let ConvertStep::Committed(s) = c.push(ch) {
                out.push_str(&s);
            }
        }
        assert_eq!(out, "こんにちは");
        assert_eq!(c.flush(), "");
    }

    #[test]
    fn push_returns_invalid_for_non_ascii() {
        let mut c = RomajiConverter::new();
        assert_eq!(c.push('漢'), ConvertStep::Invalid('漢'));
    }

    #[test]
    fn reset_clears_pending() {
        let mut c = RomajiConverter::new();
        assert_eq!(c.push('k'), ConvertStep::Pending);
        c.reset();
        // After reset, pushing 'a' commits 'あ' not 'か'.
        assert_eq!(c.push('a'), ConvertStep::Committed("あ".to_string()));
    }

    #[test]
    fn flush_finalizes_lone_n_as_hatsuon() {
        let mut c = RomajiConverter::new();
        assert_eq!(c.push('n'), ConvertStep::Pending);
        assert_eq!(c.flush(), "ん".to_string());
    }

    #[test]
    fn flush_returns_unresolvable_tail() {
        let mut c = RomajiConverter::new();
        assert_eq!(c.push('k'), ConvertStep::Pending);
        assert_eq!(c.flush(), "k".to_string());
    }

    #[test]
    fn flush_on_empty_buffer_is_empty() {
        let mut c = RomajiConverter::new();
        assert_eq!(c.flush(), "");
    }

    #[test]
    fn default_matches_new() {
        let a = RomajiConverter::default();
        let b = RomajiConverter::new();
        assert_eq!(a.convert("a"), b.convert("a"));
    }
}
```

- [ ] **Step 2: `lib.rs` で public re-export を追加**

Edit `crates/kotoha-core/src/lib.rs`:

old_string:

```rust
pub mod error;
pub mod kana;
pub mod romaji;

pub use error::{Error, Result};
```

new_string:

```rust
pub mod error;
pub mod kana;
pub mod romaji;

pub use error::{Error, Result};
pub use romaji::{ConvertStep, RomajiConverter};
```

- [ ] **Step 3: `cargo build -p kotoha-core` で build が通ることを確認**

Run:

```bash
cargo build -p kotoha-core
```

Expected: `Finished dev profile` が表示される。

- [ ] **Step 4: facade のテスト実行 (Green)**

Run:

```bash
cargo test -p kotoha-core --lib romaji::tests
```

Expected: 16 passed (`convert_konnichiwa`、`convert_tsumugi`、`convert_n_apostrophe_ya`、`convert_nya_without_apostrophe`、`convert_trailing_consonant_is_pending`、`convert_empty_string`、`convert_sokuon_kka`、`convert_long_vowel`、`convert_does_not_mutate_self`、`push_stream_konnichiwa`、`push_returns_invalid_for_non_ascii`、`reset_clears_pending`、`flush_finalizes_lone_n_as_hatsuon`、`flush_returns_unresolvable_tail`、`flush_on_empty_buffer_is_empty`、`default_matches_new`)。

- [ ] **Step 5: romaji モジュール全体のテストカウント確認**

Run:

```bash
cargo test -p kotoha-core --lib romaji 2>&1 | tail -5
```

Expected: `test result: ok. 33 passed; 0 failed` (trie 6 + state 11 + facade 16)。最低 20 件の目標 (spec §11.1 の romaji 分 20 件) を超過達成。

- [ ] **Step 6: workspace 全体テスト**

Run:

```bash
cargo test --workspace
```

Expected: M2 の 23 件 + M3a の 33 件 = 56 件以上 PASS。

- [ ] **Step 7: clippy + fmt 確認**

Run:

```bash
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
```

Expected: warnings ゼロ、fmt diff なし。

- [ ] **Step 8: commit**

Run:

```bash
git add crates/kotoha-core/src/romaji/mod.rs crates/kotoha-core/src/lib.rs
git commit -m "feat(kotoha-core): add RomajiConverter public API

- RomajiConverter::new / convert / push / reset / flush
- ConvertStep enum (#[non_exhaustive]): Committed(String), Pending, Invalid(char)
- convert() is &self and does not mutate state (uses a temporary StateMachine)
- flush() finalizes a lone 'n' as ん per spec §9
- Default impl delegates to new()
- 16 unit tests cover konnichiwa/tsumugi/n'ya/nya/sokuon/long-vowel,
  incremental push+flush workflow, reset semantics, invalid input
- Re-exported as kotoha_core::{RomajiConverter, ConvertStep}"
```

Expected: 2 files changed (`lib.rs`、`romaji/mod.rs`)。

---

### Task M3a-5: lefthook pre-push 実測と PR 作成

**Files:** (変更なし、push + PR 作成 + 検証)

- [ ] **Step 1: lefthook pre-push 実行**

Run:

```bash
lefthook run pre-push
```

Expected:
- `manifest-check` PASS
- `build` PASS
- `clippy` PASS (warnings ゼロ)
- `test` PASS (56 件以上)

- [ ] **Step 2: push**

Run:

```bash
git push -u origin feature/N-kotoha-romaji-core
```

Expected: pre-push hook が自動実行、PASS 後に push 成功。

- [ ] **Step 3: PR 作成**

Run (N は Task M3a-0 で作成した ISSUE 番号):

```bash
gh pr create --base develop --head feature/N-kotoha-romaji-core \
  --title "M3a: romaji module — rules + trie + state machine + RomajiConverter" \
  --body "$(cat <<'EOS'
## Summary

Phase 0 Milestone 3 (part A): implement the core romaji-to-kana conversion logic.

- New module \`crates/kotoha-core/src/romaji/\` with 4 files
- \`rules.rs\`: static rule table (220+ entries, Karukan-derived, Shift-specific rules omitted)
- \`trie.rs\`: prefix-match trie built once from \`RULES\`, 3-way lookup (Match/Partial/None)
- \`state.rs\`: stream state machine with double-consonant sokuon and bare-n hatsuon special cases
- \`mod.rs\`: \`RomajiConverter\` public facade with \`new/convert/push/reset/flush\` + \`ConvertStep\` enum
- Re-exported as \`kotoha_core::{RomajiConverter, ConvertStep}\`
- 33 unit tests (trie 6 + state 11 + facade 16) — exceeds the 20-test target

\`RomajiConverter::convert(\"konnichiwa\")\` returns \`(\"こんにちは\", \"\")\` as required.

## Out of Scope

- Golden test fixture + runner (M3b)
- Property tests + proptest dev-dependency (M3b)
- \`input\` module (M4)

## Related

- Spec: docs/superpowers/specs/2026-04-22-kotoha-phase-0-design.md §7.2, §9, §11.1
- Plan: docs/superpowers/plans/2026-04-23-kotoha-phase-0-m3.md
- Closes #N

## Test plan

- [ ] cargo build --workspace passes
- [ ] cargo test --workspace passes (56+ unit tests: 23 from M2 + 33 from M3a)
- [ ] cargo clippy --workspace --all-targets -- -D warnings: zero warnings
- [ ] cargo fmt --all --check: no diff
- [ ] lefthook pre-push all commands PASS
- [ ] RomajiConverter::convert("konnichiwa") == ("こんにちは", "")
EOS
)"
```

Replace `#N` with the ISSUE number from Task M3a-0. Expected: PR URL が表示される。

- [ ] **Step 4: PR review (Medium tier, 約 7 files / 約 500 lines のため Medium を選択)**

CLAUDE.md の PR Review Matrix より Medium tier:

Run (Skill 経由):

```
/agent-teams:team-review dimensions=security,performance,architecture,testing,a11y
/owasp-security
/secrets-check
```

Expected: Critical / High findings ゼロが理想。発見あれば implementer subagent に修正依頼 → 再 review ループ。

- [ ] **Step 5: findings 解消 + 再 review**

Critical / High ゼロになるまで実施。

- [ ] **Step 6: squash merge + branch 削除**

Run (`<PR番号>` は Step 3 出力の PR 番号):

```bash
gh pr merge <PR番号> --squash --delete-branch
gh pr view <PR番号> --json state,mergeCommit -q '{state, merge: .mergeCommit.oid}'
```

Expected: `state: MERGED`、merge commit SHA 取得。

- [ ] **Step 7: develop 追従**

Run:

```bash
git checkout develop
git pull
git log --oneline -3
```

Expected: squash merge commit が develop 先頭。

- [ ] **Step 8: WBS ログ作成**

Write `docs/wbs/2026-04-23-feature-N-kotoha-romaji-core.md` (`N` は ISSUE 番号、`<MERGE_COMMIT>` は Step 6 の merge SHA、`<PR_NUMBER>` は PR 番号):

```markdown
---
milestone: M3a
branch: feature/N-kotoha-romaji-core
pr: "#<PR_NUMBER>"
merge_commit: "<MERGE_COMMIT>"
issue: "#N"
status: done
started: 2026-04-23
finished: 2026-04-XX
---

# M3a: romaji module — rules + trie + state machine + RomajiConverter facade

## 実施内容

- `crates/kotoha-core/src/romaji/rules.rs` — 220+ 件の Karukan 派生ルール表
- `crates/kotoha-core/src/romaji/trie.rs` — prefix-match trie (Lookup::Match / Partial / None)
- `crates/kotoha-core/src/romaji/state.rs` — stream state machine (sokuon + hatsuon 特殊処理)
- `crates/kotoha-core/src/romaji/mod.rs` — `RomajiConverter` facade + `ConvertStep` enum
- `crates/kotoha-core/src/lib.rs` — `pub mod romaji;` + `pub use romaji::{ConvertStep, RomajiConverter};`
- 単体テスト 33 件 (trie 6 + state 11 + facade 16)

## つまずき

(実施時に記入)

## M3b への申し送り

- `RomajiConverter::convert(&str) -> (String, String)` の API 契約を fix した
- golden test fixture 作成時、`pending` 部分 (第 2 要素) の扱いに注意: pending あり → TSV 列で明示
- property test の対象は `convert` の純関数性 (=副作用なし = 冪等性) と、`convert(a + b)` を `convert(a)` + `convert(pending + b)` に分解できること (= 結合性) の 2 条件のみ(spec §11.3 準拠)
- proptest dev-dependency 追加は M3b で workspace root `Cargo.toml` に追加

## 成果物リンク

- PR: #<PR_NUMBER>
- ISSUE: #N
- Spec: `docs/superpowers/specs/2026-04-22-kotoha-phase-0-design.md`
- Plan: `docs/superpowers/plans/2026-04-23-kotoha-phase-0-m3.md`
```

- [ ] **Step 9: WBS を develop に直接 push (CLAUDE.md の例外ルール適用)**

Run:

```bash
git add docs/wbs/2026-04-23-feature-N-kotoha-romaji-core.md
git commit -m "docs: M3a implementation log"
git push
```

Expected: lefthook pre-push 全 PASS。

---

## PR #2 — M3b: golden test + property test + proptest wiring

**Goal:** `RomajiConverter::convert` に対する golden test (200+ ケース) と property test (冪等性・結合性の 2 条件) を整備する。

### M3b 完了条件

- [ ] GitHub ISSUE (M3b) が作成され、merge 済み PR で close される
- [ ] `cargo test -p kotoha-core --test romaji_golden` が 200 ケース以上 PASS
- [ ] `cargo test -p kotoha-core --test romaji_property` が 2 条件すべて PASS
- [ ] `cargo test --workspace` 全体 PASS
- [ ] clippy warnings ゼロ、fmt diff ゼロ
- [ ] lefthook pre-push 全 PASS
- [ ] WBS ログ `docs/wbs/2026-04-23-feature-M-kotoha-romaji-tests.md` が develop に push 済み

### ファイル構成 (M3b)

新規作成:

- `crates/kotoha-core/tests/fixtures/romaji_cases.tsv` — 200+ ケース TSV fixture
- `crates/kotoha-core/tests/romaji_golden.rs` — TSV リーダ + assertion harness
- `crates/kotoha-core/tests/romaji_property.rs` — proptest 2 条件

変更:

- `Cargo.toml` (workspace root) — `[workspace.dependencies]` に `proptest = "1.5"` を追加
- `crates/kotoha-core/Cargo.toml` — `[dev-dependencies]` に `proptest = { workspace = true }` を追加

---

### Task M3b-0: ISSUE 作成 + branch 作成

- [ ] **Step 1: develop 最新化**

Run:

```bash
cd /home/kohshiro/develops/student/kotoha-ime
git checkout develop
git pull
git status
```

Expected: M3a の merge + WBS commit が develop 先頭。working tree clean。

- [ ] **Step 2: M3b 用 ISSUE 作成**

Run:

```bash
gh issue create \
  --title "M3b: romaji module — golden test (200+ cases) + property test + proptest wiring" \
  --body "Phase 0 Milestone 3 (part B): integration tests for RomajiConverter.

## Scope

- New fixture \`crates/kotoha-core/tests/fixtures/romaji_cases.tsv\` with 200+ cases
- New integration test \`crates/kotoha-core/tests/romaji_golden.rs\`
- New integration test \`crates/kotoha-core/tests/romaji_property.rs\` with 2 properties:
  - Idempotence on committed output: \`convert\` is a retraction on its own committed output (re-running \`convert\` on the committed portion leaves it unchanged and produces empty pending)
  - Associativity via pending: \`convert(a + b)\` decomposes as \`convert(a)\` followed by \`convert(pending_of_a + b)\`, with committed parts concatenating and final pending matching
- Wire \`proptest = \"1.5\"\` into workspace dependencies and crate dev-dependencies

## Depends on

- M3a (provides RomajiConverter::convert API)

## Acceptance

- cargo test -p kotoha-core --test romaji_golden PASSES (all fixture rows asserted inside the single \`every_fixture_row_matches_converter\` test)
- cargo test -p kotoha-core --test romaji_property PASSES 2 properties
- workspace \`#[test]\` count: 23 (M2) + 33 (M3a) + 2 (golden: fixture-size-assertion + row-by-row) + 2 (property: idempotence / associativity) = 60 tests. The golden fixture itself contains 200+ rows, all asserted inside the single row-by-row test function.

## Reference

- Spec: docs/superpowers/specs/2026-04-22-kotoha-phase-0-design.md §11.2, §11.3
- Plan: docs/superpowers/plans/2026-04-23-kotoha-phase-0-m3.md"
```

Expected: ISSUE 作成、URL + 番号表示 (以降 `M` と呼ぶ)。

- [ ] **Step 3: branch 作成**

Run (M は Step 2 の ISSUE 番号):

```bash
git checkout -b feature/M-kotoha-romaji-tests develop
git branch --show-current
```

Expected: `feature/M-kotoha-romaji-tests`。

---

### Task M3b-1: proptest dev-dependency の配線

**Files:**
- Modify: `Cargo.toml` (workspace root)
- Modify: `crates/kotoha-core/Cargo.toml`

- [ ] **Step 1: root `Cargo.toml` に proptest を追加**

Edit `Cargo.toml`:

old_string:

```toml
[workspace.dependencies]
thiserror = "1"
anyhow = "1"
tracing = "0.1"
tracing-subscriber = "0.3"
```

new_string:

```toml
[workspace.dependencies]
thiserror = "1"
anyhow = "1"
tracing = "0.1"
tracing-subscriber = "0.3"
proptest = "1.5"
```

- [ ] **Step 2: `crates/kotoha-core/Cargo.toml` に dev-dep を追加**

Edit `crates/kotoha-core/Cargo.toml`:

old_string:

```toml
[dependencies]
thiserror = { workspace = true }
tracing = { workspace = true }
```

new_string:

```toml
[dependencies]
thiserror = { workspace = true }
tracing = { workspace = true }

[dev-dependencies]
proptest = { workspace = true }
```

- [ ] **Step 3: build が通ることを確認**

Run:

```bash
cargo build -p kotoha-core --tests
```

Expected: proptest とその依存 crate が fetch される (初回のみ時間がかかる)。`Finished dev profile` 表示。

- [ ] **Step 4: 既存テストが全 PASS することを確認 (regression なし)**

Run:

```bash
cargo test -p kotoha-core
```

Expected: 56 件 (M2 の 23 + M3a の 33) PASS。

- [ ] **Step 5: commit**

Run:

```bash
git add Cargo.toml crates/kotoha-core/Cargo.toml
git commit -m "chore(kotoha-core): wire proptest 1.5 as workspace dev-dependency

Add proptest to [workspace.dependencies] at the repo root and
consume it from crates/kotoha-core as a dev-dependency. Used by
the upcoming romaji_property integration test (M3b)."
```

Expected: 2 files changed。

---

### Task M3b-2: TSV fixture を作成

**Files:**
- Create: `crates/kotoha-core/tests/fixtures/romaji_cases.tsv`

**TSV フォーマット (tests/romaji_golden.rs から使用):**

- 1 行 = 1 ケース
- 列: `romaji_input<TAB>expected_committed<TAB>expected_pending`
- `#` 始まり行はコメント扱い
- 空行は無視
- `expected_committed` / `expected_pending` は空文字列を許容 (その場合列は空)

200+ ケースを以下カテゴリで網羅 (合計 220 ケース):

| カテゴリ | ケース数 |
|---|---|
| 母音単体 (a, i, u, e, o) | 5 |
| 基本 CV × 5 段 (ka/ki/ku/ke/ko × ...) | 45 |
| 濁音 (ga/za/da/ba) × 5 段 | 20 |
| 半濁音 (pa/pi/pu/pe/po) | 5 |
| 拗音 3 文字 (kya/sha/chu 等) | 30 |
| 促音 (kka/ssa/tto 等) | 20 |
| 撥音 (n) の全パターン (n+母音、n+子音、nn、n') | 15 |
| 長音 / 記号 | 10 |
| ヘボン式異体 (shi/ti/chi 等) | 15 |
| 小字 (la/xa/ltu 等) | 15 |
| 拡張音 (fa/va/tsa/kwa) | 20 |
| pending 残り (kon → こ + n、kya 途中 ky → "" + ky 等) | 10 |
| 長文複合 (konnichiwa、tsumugi、arigatou 等) | 10 |

- [ ] **Step 1: `crates/kotoha-core/tests/fixtures/` ディレクトリ作成**

Run:

```bash
mkdir -p crates/kotoha-core/tests/fixtures
```

- [ ] **Step 2: `romaji_cases.tsv` を Write で作成**

Write `crates/kotoha-core/tests/fixtures/romaji_cases.tsv`:

```tsv
# Kotoha romaji → kana golden fixture
# Format: romaji_input<TAB>expected_committed<TAB>expected_pending
# Comments start with '#'. Empty lines are ignored.
# Derived from Karukan rule set (MIT/Apache-2.0) with Shift-specific cases omitted.

# --- vowels (5) ---
a	あ	
i	い	
u	う	
e	え	
o	お	

# --- basic gojuon CV (45) ---
ka	か	
ki	き	
ku	く	
ke	け	
ko	こ	
sa	さ	
si	し	
su	す	
se	せ	
so	そ	
ta	た	
ti	ち	
tu	つ	
te	て	
to	と	
na	な	
ni	に	
nu	ぬ	
ne	ね	
no	の	
ha	は	
hi	ひ	
hu	ふ	
he	へ	
ho	ほ	
ma	ま	
mi	み	
mu	む	
me	め	
mo	も	
ya	や	
yu	ゆ	
yo	よ	
ra	ら	
ri	り	
ru	る	
re	れ	
ro	ろ	
wa	わ	
wo	を	
wi	ゐ	
we	ゑ	
# 5 hebon-style variants treated here
shi	し	
chi	ち	
tsu	つ	
fu	ふ	
ji	じ	

# --- dakuten (20) ---
ga	が	
gi	ぎ	
gu	ぐ	
ge	げ	
go	ご	
za	ざ	
zi	じ	
zu	ず	
ze	ぜ	
zo	ぞ	
da	だ	
di	ぢ	
du	づ	
de	で	
do	ど	
ba	ば	
bi	び	
bu	ぶ	
be	べ	
bo	ぼ	

# --- handakuten (5) ---
pa	ぱ	
pi	ぴ	
pu	ぷ	
pe	ぺ	
po	ぽ	

# --- yoon (30) ---
kya	きゃ	
kyu	きゅ	
kyo	きょ	
sya	しゃ	
syu	しゅ	
syo	しょ	
sha	しゃ	
shu	しゅ	
sho	しょ	
tya	ちゃ	
tyu	ちゅ	
tyo	ちょ	
cha	ちゃ	
chu	ちゅ	
cho	ちょ	
nya	にゃ	
nyu	にゅ	
nyo	にょ	
hya	ひゃ	
hyu	ひゅ	
hyo	ひょ	
mya	みゃ	
myu	みゅ	
myo	みょ	
rya	りゃ	
ryu	りゅ	
ryo	りょ	
gya	ぎゃ	
gyu	ぎゅ	
gyo	ぎょ	

# --- sokuon (20) ---
kka	っか	
kki	っき	
kku	っく	
kke	っけ	
kko	っこ	
ssa	っさ	
ssi	っし	
ssu	っす	
sse	っせ	
sso	っそ	
tta	った	
tti	っち	
ttu	っつ	
tte	って	
tto	っと	
ppa	っぱ	
ppi	っぴ	
ppu	っぷ	
ppe	っぺ	
ppo	っぽ	

# --- hatsuon (15) ---
# "na" is covered in the basic gojuon section above; this section covers n-specific cases only
nnn	ん	n
nk	ん	k
nm	ん	m
ns	ん	s
nt	ん	t
np	ん	p
nb	ん	b
ng	ん	g
nr	ん	r
nz	ん	z
nn	ん	
n'	ん	
n'a	んあ	
n'ya	んや	

# --- long vowel + symbols (10) ---
-	ー	
a-	あー	
ko-hi-	こーひー	
.	。	
,	、	
?	?	
!	!	
[	「	
]	」	
/	・	

# --- small forms (15) ---
la	ぁ	
li	ぃ	
lu	ぅ	
le	ぇ	
lo	ぉ	
xa	ぁ	
xi	ぃ	
xu	ぅ	
xe	ぇ	
xo	ぉ	
lya	ゃ	
lyu	ゅ	
lyo	ょ	
ltu	っ	
xtu	っ	

# --- extended sounds (20) ---
fa	ふぁ	
fi	ふぃ	
fe	ふぇ	
fo	ふぉ	
va	ゔぁ	
vi	ゔぃ	
vu	ゔ	
ve	ゔぇ	
vo	ゔぉ	
tsa	つぁ	
tsi	つぃ	
tse	つぇ	
tso	つぉ	
kwa	くぁ	
kwi	くぃ	
kwe	くぇ	
kwo	くぉ	
gwa	ぐぁ	
wha	うぁ	
whi	うぃ	

# --- pending tails (10) ---
k		k
ky		ky
s		s
sh		sh
kon	こ	n
koh	こ	h
kos	こ	s
kyak	きゃ	k
tsuk	つ	k
# pending-tail case: 'fuk' -> commits ふ (from fu), leaves k in pending (k is a potential sokuon/consonant head).
fuk	ふ	k

# --- long composite (10) ---
konnichiwa	こんにちは	
tsumugi	つむぎ	
arigatou	ありがとう	
sayounara	さようなら	
ohayou	おはよう	
ittekimasu	いってきます	
onegaishimasu	おねがいします	
gakkou	がっこう	
nihongo	にほんご	
kyou	きょう	
```

## Notes

- `pending tails` セクションの 10 行目 `fuk` は pending-tail ケースの代表例として採用した。初稿では `sha` を置いていたが `sha` は完全な rule (→ しゃ) で pending が空になるため矛盾しており、`fuk` (`fu` → ふ コミット、`k` が pending として残る) に差し替えた
- `hatsuon` セクションから冒頭の `na` 行を削除した。`na` は `basic gojuon` セクションで既にカバーされている重複行だった

- [ ] **Step 3: TSV の行数確認**

Run:

```bash
grep -vE '^#|^$' crates/kotoha-core/tests/fixtures/romaji_cases.tsv | wc -l
```

Expected: 200 以上 (目標 220)。

- [ ] **Step 4: commit**

Run:

```bash
git add crates/kotoha-core/tests/fixtures/romaji_cases.tsv
git commit -m "test(kotoha-core): add romaji golden fixture (200+ cases)

TSV format: input<TAB>expected_committed<TAB>expected_pending.
Covers vowels, gojuon, dakuten, handakuten, yoon, sokuon,
hatsuon (including n'a / n'ya / nk-style non-vowel continuations),
long vowel mark, punctuation symbols, small forms (la/xa/lya/ltu),
extended sounds (fa/va/tsa/kwa/wha), mid-rule pending tails,
and long composite words (konnichiwa / tsumugi / arigatou / etc.).

Karukan-derived, Shift-specific rows omitted."
```

Expected: 1 file changed。

---

### Task M3b-3: golden test runner を TDD で実装

**Files:**
- Create: `crates/kotoha-core/tests/romaji_golden.rs`

- [ ] **Step 1: golden runner を Write で作成**

Write `crates/kotoha-core/tests/romaji_golden.rs`:

```rust
//! Golden test harness for RomajiConverter.
//!
//! Reads `tests/fixtures/romaji_cases.tsv` and asserts that
//! `RomajiConverter::convert(input)` equals `(expected_committed, expected_pending)`
//! for every non-comment, non-empty row.

use std::fs;
use std::path::Path;

use kotoha_core::RomajiConverter;

struct Case {
    line_no: usize,
    input: String,
    expected_committed: String,
    expected_pending: String,
}

fn load_cases() -> Vec<Case> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("romaji_cases.tsv");
    let text =
        fs::read_to_string(&path).unwrap_or_else(|e| panic!("failed to read {path:?}: {e}"));
    let mut cases = Vec::new();
    for (i, raw) in text.lines().enumerate() {
        let line_no = i + 1;
        let line = raw;
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let cols: Vec<&str> = line.split('\t').collect();
        assert!(
            cols.len() >= 3,
            "line {line_no}: expected 3 TAB-separated columns, got {}: {raw:?}",
            cols.len()
        );
        cases.push(Case {
            line_no,
            input: cols[0].to_string(),
            expected_committed: cols[1].to_string(),
            expected_pending: cols[2].to_string(),
        });
    }
    cases
}

#[test]
fn fixture_has_at_least_200_cases() {
    let cases = load_cases();
    assert!(
        cases.len() >= 200,
        "fixture must have >= 200 cases, got {}",
        cases.len()
    );
}

#[test]
fn every_fixture_row_matches_converter() {
    let cases = load_cases();
    let converter = RomajiConverter::new();
    let mut failures: Vec<String> = Vec::new();
    for case in &cases {
        let (got_committed, got_pending) = converter.convert(&case.input);
        if got_committed != case.expected_committed || got_pending != case.expected_pending {
            failures.push(format!(
                "line {}: input={:?}\n  expected: committed={:?}, pending={:?}\n  got:      committed={:?}, pending={:?}",
                case.line_no,
                case.input,
                case.expected_committed,
                case.expected_pending,
                got_committed,
                got_pending,
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} golden cases failed:\n{}",
        failures.len(),
        failures.join("\n")
    );
}
```

- [ ] **Step 2: 実行して PASS することを確認**

Run:

```bash
cargo test -p kotoha-core --test romaji_golden
```

Expected: 2 tests PASS (`fixture_has_at_least_200_cases`、`every_fixture_row_matches_converter`)。fixture の 220 ケースすべてが内部で assertion PASS。

もし golden ケースが失敗した場合:

1. fixture 側が間違っている可能性を最初に検討 (CLAUDE.md の testing.md Error Investigation Flow 準拠)
2. テストが正しい仕様 (Karukan + Spec §9) を表現しているなら、M3a 実装にバグ。修正は M3b 範囲外ではなく M3a を hotfix (新規 branch + 小さい PR) として別途実施

- [ ] **Step 3: clippy 確認**

Run:

```bash
cargo clippy -p kotoha-core --all-targets -- -D warnings
```

Expected: warnings ゼロ。

- [ ] **Step 4: commit**

Run:

```bash
git add crates/kotoha-core/tests/romaji_golden.rs
git commit -m "test(kotoha-core): add romaji golden test harness

- Reads tests/fixtures/romaji_cases.tsv (TAB-separated, 3 columns)
- Skips '#'-prefixed comments and blank lines
- Asserts RomajiConverter::convert(input) == (expected_committed, expected_pending)
- fixture_has_at_least_200_cases enforces the spec §11.2 minimum
- every_fixture_row_matches_converter aggregates failures for readable diff output"
```

Expected: 1 file changed。

---

### Task M3b-4: property test を TDD で実装

**Files:**
- Create: `crates/kotoha-core/tests/romaji_property.rs`

**2 プロパティ (Spec §11.3):**

1. **冪等性 (Idempotence on committed output):** `convert(input)` の committed 出力を再度 `convert` に通しても、committed 出力は変わらず、pending は空になる。ひらがなは非 ASCII のため状態機械は `Invalid` として drop するかそのまま素通しする(どちらでも committed が不変であれば property は成立)。本性質は `convert` が自身の committed 出力に対して retraction であることを固定化する
2. **結合性 (Associativity):** 入力を区切り位置で分割して個別に `convert` した結果 (pending に注意して連結) が、元の入力全体の convert 結果と一致する (pending 境界に注意)。具体的には `convert(a + b).0` が、まず `(c1, p1) = convert(a)` の pending `p1` を prefix として `b` の前に付けた `p1 + b` を convert した結果 `(c2, p2)` と組み合わせて `c1 + c2 == convert(a + b).0` かつ `p2 == convert(a + b).1`

Spec revision 1 には第 3 条件として "可逆性 (invertibility)" が列挙されていたが、romaji → かな が多対一であるため除外された(spec §11.3 参照)。本 plan 初稿で検討した「pending-ASCII invariant」による代替も、可逆性の代替としては筋違いのため採用しない。また、初稿で採用していた `convert(input) == convert(input)` 型の純粋関数テストは、`&self` かつ内部可変性なしの実装に対しては自明に成立するため、committed 出力に対する retraction 性を主張する `prop_idempotence_on_committed` 形に強化した。

- [ ] **Step 1: property test を Write で作成**

Write `crates/kotoha-core/tests/romaji_property.rs`:

```rust
//! Property tests for RomajiConverter using proptest.
//!
//! Two properties (per spec §11.3):
//! 1. Idempotence on committed output: convert is a retraction on its own committed output
//! 2. Associativity: convert(a + b) decomposes via convert(a) and convert(pending + b)

use kotoha_core::RomajiConverter;
use proptest::prelude::*;

/// Strategy: ASCII strings limited to the alphabet actually used by romaji rules,
/// bounded in length to keep proptest iteration fast.
fn romaji_input() -> impl Strategy<Value = String> {
    "[a-z\\-'.,!?\\[\\]/]{0,12}".prop_map(|s| s)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Idempotence on the committed output: running `convert` on the committed portion
    /// of a prior `convert` must leave the committed output unchanged and produce no
    /// additional pending. Hiragana chars are non-ASCII, so when they re-enter `convert`
    /// the state machine should either drop them (as `Invalid`) or pass them through
    /// without change. This test locks in that `convert` is a retraction on its output.
    #[test]
    fn prop_idempotence_on_committed(input in romaji_input()) {
        let c = RomajiConverter::new();
        let (committed_first, _pending_first) = c.convert(&input);
        let (committed_second, pending_second) = c.convert(&committed_first);
        // The committed output should stabilize on the second pass.
        prop_assert_eq!(committed_second, committed_first);
        // Re-running convert on already-converted hiragana should not leave any
        // romaji in the pending buffer.
        prop_assert!(pending_second.is_empty());
    }

    /// Property 2 (associativity via pending): for inputs a and b,
    /// convert(a + b) equals concatenating convert(a).0 with
    /// convert(convert(a).1 + b).0, and the final pending matches
    /// convert(convert(a).1 + b).1.
    #[test]
    fn prop_associativity_via_pending(a in romaji_input(), b in romaji_input()) {
        let c = RomajiConverter::new();
        let ab = format!("{}{}", a, b);
        let (whole_committed, whole_pending) = c.convert(&ab);

        let (left_committed, left_pending) = c.convert(&a);
        let glued = format!("{}{}", left_pending, b);
        let (right_committed, right_pending) = c.convert(&glued);

        let reconstructed = format!("{}{}", left_committed, right_committed);
        prop_assert_eq!(reconstructed, whole_committed);
        prop_assert_eq!(right_pending, whole_pending);
    }
}
```

- [ ] **Step 2: 実行して 2 条件 PASS を確認**

Run:

```bash
cargo test -p kotoha-core --test romaji_property
```

Expected: 2 proptest cases PASS, 各 256 反復成功 (`prop_idempotence_on_committed`、`prop_associativity_via_pending`)。

もし proptest が失敗 (counter-example 発見) した場合:

1. shrink された counter-example を確認
2. Spec §9 / Karukan 仕様と照らし合わせ、テストが仕様を正しく表現しているか確認
3. テストが正しければ M3a 実装にバグ → hotfix PR

- [ ] **Step 3: clippy 確認**

Run:

```bash
cargo clippy -p kotoha-core --all-targets -- -D warnings
```

Expected: warnings ゼロ。

- [ ] **Step 4: commit**

Run:

```bash
git add crates/kotoha-core/tests/romaji_property.rs
git commit -m "test(kotoha-core): add romaji property tests (2 invariants)

Two properties validated with proptest (256 iterations each):
- prop_idempotence_on_committed: convert is a retraction on its own
  committed output (committed stabilizes on the second pass and
  pending becomes empty)
- prop_associativity_via_pending: convert(a+b) decomposes via
  convert(a) and convert(pending+b) such that concatenated committed
  matches the whole and final pending equals the decomposed pending

Matches spec §11.3 revision 2."
```

Expected: 1 file changed。

---

### Task M3b-5: 総合確認、PR 作成、merge

**Files:** (変更なし、検証 + PR 作成)

- [ ] **Step 1: workspace 全体 build + test + clippy + fmt**

Run:

```bash
cargo build --workspace
cargo test --workspace 2>&1 | tail -20
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
```

Expected:
- build PASS
- workspace `#[test]` count: 23 (M2) + 33 (M3a) + 2 (golden: fixture-size-assertion + row-by-row) + 2 (property: 冪等性 / 結合性) = 60 tests。golden fixture 自体は 200 行以上を含み、単一の `every_fixture_row_matches_converter` テスト関数内で全行 assert する
- clippy warnings ゼロ
- fmt diff なし

- [ ] **Step 2: lefthook pre-push 実行**

Run:

```bash
lefthook run pre-push
```

Expected: 全コマンド PASS。

- [ ] **Step 3: push**

Run:

```bash
git push -u origin feature/M-kotoha-romaji-tests
```

- [ ] **Step 4: PR 作成**

Run (M は Task M3b-0 の ISSUE 番号):

```bash
gh pr create --base develop --head feature/M-kotoha-romaji-tests \
  --title "M3b: romaji module — golden test (200+ cases) + property test + proptest wiring" \
  --body "$(cat <<'EOS'
## Summary

Phase 0 Milestone 3 (part B): integration tests for RomajiConverter.

- Added workspace dev-dep \`proptest = \"1.5\"\` and wired it into kotoha-core
- New fixture \`tests/fixtures/romaji_cases.tsv\` with 220 cases covering vowels,
  gojuon, dakuten/handakuten, yoon, sokuon, hatsuon (n'a / n'ya / n+consonant),
  long vowel + punctuation, small forms, extended sounds, pending tails,
  and long composite words
- New integration test \`tests/romaji_golden.rs\` asserting every fixture row
  matches \`RomajiConverter::convert\` output
- New integration test \`tests/romaji_property.rs\` with 2 properties:
  idempotence on committed output, associativity via pending

Total workspace test count: 60+ (23 from M2 + 33 from M3a + 2 golden + 2 property).

## Depends on

- M3a (merged): provides the RomajiConverter public API

## Related

- Spec: docs/superpowers/specs/2026-04-22-kotoha-phase-0-design.md §11.2, §11.3
- Plan: docs/superpowers/plans/2026-04-23-kotoha-phase-0-m3.md
- Closes #M

## Test plan

- [ ] cargo test -p kotoha-core --test romaji_golden passes all fixture rows
- [ ] cargo test -p kotoha-core --test romaji_property passes all 2 properties (256 cases each)
- [ ] cargo test --workspace passes 60+ total
- [ ] cargo clippy --workspace --all-targets -- -D warnings: zero warnings
- [ ] cargo fmt --all --check: no diff
- [ ] lefthook pre-push all commands PASS
EOS
)"
```

Replace `#M` with the ISSUE number. Expected: PR URL 表示。

- [ ] **Step 5: PR review (Small tier — テスト追加のみ、実コード変更なし、約 5 files / 約 300 lines)**

CLAUDE.md の PR Review Matrix より Small tier:

Run:

```
/agent-teams:team-review dimensions=security,architecture,testing
/secrets-check
```

Expected: Critical / High ゼロ。

- [ ] **Step 6: findings 解消 + 再 review**

Critical / High ゼロになったら merge。

- [ ] **Step 7: squash merge + branch 削除**

Run (`<PR番号>` は Step 4 出力):

```bash
gh pr merge <PR番号> --squash --delete-branch
gh pr view <PR番号> --json state,mergeCommit -q '{state, merge: .mergeCommit.oid}'
```

Expected: `state: MERGED`、merge SHA 取得。

- [ ] **Step 8: develop 追従**

Run:

```bash
git checkout develop
git pull
git log --oneline -5
```

Expected: M3b merge commit が先頭。

- [ ] **Step 9: WBS ログ作成**

Write `docs/wbs/2026-04-23-feature-M-kotoha-romaji-tests.md`:

```markdown
---
milestone: M3b
branch: feature/M-kotoha-romaji-tests
pr: "#<PR_NUMBER>"
merge_commit: "<MERGE_COMMIT>"
issue: "#M"
status: done
started: 2026-04-XX
finished: 2026-04-XX
---

# M3b: romaji module — golden test + property test + proptest wiring

## 実施内容

- `Cargo.toml` (root) — `[workspace.dependencies]` に `proptest = "1.5"` 追加
- `crates/kotoha-core/Cargo.toml` — `[dev-dependencies]` に `proptest = { workspace = true }` 追加
- `crates/kotoha-core/tests/fixtures/romaji_cases.tsv` — 220 ケース TSV
- `crates/kotoha-core/tests/romaji_golden.rs` — TSV リーダ + 全行 assert (2 test 関数)
- `crates/kotoha-core/tests/romaji_property.rs` — proptest 2 条件

## つまずき

(実施時に記入)

## M4 への申し送り

- `RomajiConverter::convert` の挙動が golden + property で固定された。`input::InputContext` から `converter.push` / `converter.flush` を呼び出す際の戻り値は `ConvertStep` を `InputStep` に変換する方針
- proptest は workspace 横断で利用可能。`input` モジュールでも mode 系不変条件の property test に採用

## 成果物リンク

- PR: #<PR_NUMBER>
- ISSUE: #M
- Spec: `docs/superpowers/specs/2026-04-22-kotoha-phase-0-design.md`
- Plan: `docs/superpowers/plans/2026-04-23-kotoha-phase-0-m3.md`
```

- [ ] **Step 10: WBS を develop に直接 push**

Run:

```bash
git add docs/wbs/2026-04-23-feature-M-kotoha-romaji-tests.md
git commit -m "docs: M3b implementation log"
git push
```

Expected: lefthook pre-push 全 PASS。

---

## Success criteria (M3 overall)

Spec §13 の M3 相当要件を以下のタスクが担保する:

| Success criterion | 担保タスク |
|---|---|
| `cargo build --workspace` PASS | M3a-4 Step 6 + M3b-5 Step 1 |
| `cargo test --workspace` PASS | M3a-4 Step 6 (56 件) + M3b-5 Step 1 (60+ 件) |
| `cargo clippy --workspace -- -D warnings` warnings ゼロ | M3a-4 Step 7 + M3b-5 Step 1 |
| `cargo fmt --all --check` diff ゼロ | M3a-4 Step 7 + M3b-5 Step 1 |
| lefthook pre-push 動作確認 | M3a-5 Step 1 + M3b-5 Step 2 |
| `RomajiConverter::convert("konnichiwa")` が `("こんにちは", "")` を返す | M3a-4 Step 4 (`convert_konnichiwa` unit test) + M3b-3 (golden fixture `konnichiwa` row) |
| romaji 単体テスト 20 件以上 | M3a-4 Step 5 で 33 件実装 |
| golden test 200 件以上 | M3b-2 + M3b-3 で 220 件実装 |
| property test 2 条件 PASS | M3b-4 で 2 条件実装 |

---

## Spec Coverage 確認

| Spec 要件 | 内容 | 担保タスク |
|---|---|---|
| §7.2 | `RomajiConverter` 公開 API (`new` / `convert` / `push` / `reset` / `flush`)、`ConvertStep` enum | M3a-4 |
| §9 総論 | 基本 50 音 / 濁音 / 半濁音 / 拗音 / 促音 / 撥音 / 長音 / ヘボン式異体 / 拡張音 / 記号 | M3a-1 (rules 表)、M3b-2 (fixture) |
| §9 double-consonant っ | 促音 (kka → っか 等) | M3a-3 (`push_double_consonant_emits_sokuon` + state machine の sokuon 分岐)、M3b-2 (sokuon 20 ケース) |
| §9 n-treatment | n + 母音 / n + 子音 / nn / n' の 4 パターン | M3a-3 (`push_bare_n_then_consonant_emits_hatsuon` / `push_nn_commits_n_explicitly` / `push_n_apostrophe_commits_n`)、M3a-4 (`convert_n_apostrophe_ya` / `convert_nya_without_apostrophe` / `flush_finalizes_lone_n_as_hatsuon`)、M3b-2 (hatsuon 15 ケース) |
| §9 long vowel | `-` → `ー` | M3a-3 (`push_long_vowel_mark`)、M3a-4 (`convert_long_vowel`)、M3b-2 (long vowel セクション) |
| §9 small forms | la/xa/lya/ltu 等 | M3a-1 (rules small-form escapes 区画)、M3b-2 (small forms 15 ケース) |
| §11.1 単体テスト | 20 件以上 | M3a-2 + M3a-3 + M3a-4 で 33 件 |
| §11.2 golden test | 200 件以上 | M3b-2 + M3b-3 で 220 件 |
| §11.3 property test | 冪等性 / 結合性 の 2 条件(spec §11.3 準拠) | M3b-4 (idempotence_on_committed / associativity) |

---

## Self-Review 済み事項

1. **Spec coverage 確認**: §7.2 / §9 各小項目 / §11.1 / §11.2 / §11.3 すべてにタスクマッピングあり。
2. **Placeholder scan**:
   - `N`、`M`: ISSUE 番号 placeholder (M3a / M3b それぞれ着手時に実値取得)
   - `<PR_NUMBER>`、`<MERGE_COMMIT>`: PR / merge 時に実値取得
   - これら以外に TBD / "implement later" / "similar to" / "appropriate validation" 等の言い回しは含まない
3. **型・名前の一貫性確認**:
   - `RomajiConverter`、`convert`、`push`、`reset`、`flush`、`ConvertStep`、`ConvertStep::Committed` / `Pending` / `Invalid`: 全タスクで統一表記
   - 内部型 `StateMachine`、`PushResult`、`Trie`、`Lookup`、`Node`: `pub(crate)` で公開しない点も統一
   - `RULES`: `pub(crate) const RULES: &[(&str, &str)]` シグネチャを M3a-1 / M3a-2 (trie から参照) で一致
4. **TDD 遵守**: 各新規モジュール (`trie` / `state` / `RomajiConverter` facade) で「テスト先行 Write → 実装 → cargo test で Green」のサイクルを踏んでいる。rules 表は data なので TDD ではなく直接書き込み (テストは trie / state / facade 側で間接的にカバー)
5. **CLAUDE.md 制約**:
   - M3a: 約 500 行変更 (rules 220 行 data + trie/state/facade 約 280 行 code)。Branch Scope Policy の 300 行制限に対し、data fixture は除外解釈で code 部分 280 行 → Medium tier 範囲内
   - M3b: 約 400 行変更 (TSV 220 行 fixture + golden/property 約 180 行 code)。同様に fixture 除外で code 180 行 → Small tier 範囲内
   - 1 branch = 1 ISSUE = 1 PR の原則遵守
6. **修正した箇所**:
   - 初稿で TSV の `sha` 行を pending tails セクションに入れていたが、`sha` は完全な rule なので pending が空になる矛盾を発見 → `fuk` に差し替えた
   - property test は spec §11.3 revision 2 に合わせて 2 条件(冪等性 / 結合性)に絞り込んだ。本 plan の初稿で検討していた「pending-ASCII invariant」による第 3 プロパティは採用しない(詳細は Task M3b-4 冒頭および冒頭の addendum note を参照)

---

## Reference list

### Spec

- `docs/superpowers/specs/2026-04-22-kotoha-phase-0-design.md`
  - §6 依存 crate (proptest 1.5 を dev-dependency として導入する根拠)
  - §7.2 `RomajiConverter` API 定義
  - §9 変換ルール総論
  - §11.1 単体テスト (50 件以上の計画中、romaji 単体で 20 件担保)
  - §11.2 golden test (romaji 200 ケース + mode 70 ケース + Karukan 差分 10 ケースの計画中、romaji 分 200 ケース担保)
  - §11.3 プロパティテスト (既存 2 条件 + mode 系 4 条件の計画中、既存 2 条件担保)

### Prior plans

- `docs/superpowers/plans/2026-04-22-kotoha-phase-0-implementation.md` (Phase 0 全体)、特に §M3 (line 946–974) を bite-sized 分割したものが本 plan
- `docs/superpowers/plans/2026-04-22-kotoha-phase-0-m2.md` (M2 bite-sized plan、タスク粒度 / TDD 手順 / ヘッダフォーマットの参照元)

### Prior ADRs

- 本 M3 時点で Phase 0 ADR は未作成 (0001 / 0002 / 0003 は spec §13.3 により Phase 0 完了条件だが着手タイミングは M7 近辺想定)
- M3a / M3b では新規 ADR は作成しない

### External references

- Karukan (`togatoga/karukan`) — romaji rules の初期値 (MIT/Apache-2.0)
- AzooKey/AzooKeyKanaKanjiConverter — API 設計の参考 (Swift 実装)
