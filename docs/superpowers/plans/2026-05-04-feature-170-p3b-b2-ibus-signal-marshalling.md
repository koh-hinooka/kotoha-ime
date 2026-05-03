# Phase 3-B B2 (IBus signal body marshalling) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.
>
> **Test-After 方式**: 本プロジェクトは Spec-Driven Test-After (CLAUDE.md 参照)。各 task は「実装 → spec acceptance を test に落とす → run」の順で進める。failing test を先に書く TDD は使わない。

**Goal:** `crates/kotoha-engine-ibus/src/proxy.rs` の 5 method を `Err(zbus::Error::Failure(NOT_YET_IMPLEMENTED))` の fail-loud stub から実 D-Bus signal emit に置換し、IBus 1.x 仕様の `IBusText` / `IBusLookupTable` wire-format marshalling を実装する。

**Architecture:** 新 file `types.rs` に `#[derive(zbus::zvariant::Type, serde::Serialize)]` で IBus 1.x serializable struct(`IBusText` / `IBusAttribute` / `IBusLookupTable`)を定義。proxy.rs の各 method は `Message::signal(path, interface, member)?.build(&body)?` で signal を組立て `connection.send(&signal)` で session bus に発信。失敗は `Result<(), zbus::Error>` を caller(`host_bridge.rs`)に propagate、host_bridge は `tracing::warn!` で集約観測(engine state には影響なし)。

**Tech Stack:** Rust 2021 edition, rust-version 1.80, zbus 5.x (workspace pinned), serde, tracing.

---

## 関連 reference (engineer 必読)

- **Spec**: `docs/superpowers/specs/2026-05-02-p3-a-ibus-engine-design.md` §4.2 (revision r2), §13 Open Q 9
- **ISSUE**: #170 (this branch's tracking)
- **Parent ISSUE**: #136 (Phase 3-B tracking)
- **Branch**: `feature/170-p3b-b2-ibus-signal-marshalling` (already created from develop HEAD `41f2687`)
- **既存ファイル(変更対象)**:
  - `crates/kotoha-engine-ibus/src/proxy.rs` (現状 130 行、全 method `Err(...)` stub)
  - `crates/kotoha-engine-ibus/src/lib.rs` (module export 追加のみ)
- **既存ファイル(変更しない)**:
  - `crates/kotoha-engine-ibus/src/host_bridge.rs` (現状の `tracing::warn` 観測 path を維持)
  - `crates/kotoha-engine-ibus/src/lookup_table.rs` (Replace/Append/Remove/Clear coalescing 動作)
- **新規ファイル**:
  - `crates/kotoha-engine-ibus/src/types.rs` (IBus 1.x wire-format types)
- **IBus 1.x 仕様参照**:
  - https://github.com/ibus/ibus/wiki/IBusEngine
  - IBus source `src/ibustext.c` / `src/ibuslookuptable.c` の `_serialize` / `_deserialize` 関数
- **zbus 5.x API form**:
  - `Message::signal(path, interface, member)?.build(&body)?` で signal を組立て
  - `connection.send(&signal)` で session bus に発信(blocking 版)

## Test baseline (絶対に regress させない)

- `cargo test --workspace` = **504 PASS / 0 FAIL** (default features)
- `cargo test --workspace --features kotoha-storage/test-helpers,kotoha-ranker-hybrid/test-helpers` = **515 PASS / 0 FAIL**
- `KOTOHA_ALLOW_STUB=1 ./target/debug/kotoha; echo "exit=$?"` = **exit=1**
- `nm target/release/kotoha | grep -ci stub` = **0**

各 task の最後にこれら baseline を確認する step を含める。

## IBus 1.x 仕様の D-Bus signature まとめ(初期推定)

以下は IBus 1.5.x の慣用的な serialization から導出した初期推定。Task 1 で IBus source(`src/ibustext.c` / `src/ibuslookuptable.c` の `_serialize` 関数)を直接参照して訂正する。**signature が間違っていると IBus daemon が parse failure し B6 manual smoke で reject される** ため、L1 unit test (round-trip) + IBus source 参照で boundary を厚く取る。

| Signal name | Signal body signature | 内訳 |
|---|---|---|
| `UpdatePreeditText` | `(vub)` | (`IBusText` variant, u32 cursor, bool visible) |
| `CommitText` | `(v)` | (`IBusText` variant) |
| `UpdateLookupTable` | `(vb)` | (`IBusLookupTable` variant, bool visible) |
| `ShowLookupTable` | `()` | empty body |
| `HideLookupTable` | `()` | empty body |

| Type | D-Bus signature | Field 順序 |
|---|---|---|
| `IBusAttribute` | `(sa{sv}uuuu)` | name (固定 "IBusAttribute") / attachments (空 dict) / type u32 / value u32 / start_index u32 / end_index u32 |
| `IBusAttrList` | `(sa{sv}av)` | name (固定 "IBusAttrList") / attachments (空 dict) / attributes (variant array of IBusAttribute) |
| `IBusText` | `(sa{sv}sv)` | name (固定 "IBusText") / attachments (空 dict) / text String / attrs (variant of IBusAttrList) |
| `IBusLookupTable` | `(sa{sv}uubbiavav)` | name (固定 "IBusLookupTable") / attachments / page_size u32 / cursor_pos u32 / cursor_visible bool / round bool / orientation i32 / candidates (variant array of IBusText) / labels (variant array of IBusText) |

**Signal interface name**: `org.freedesktop.IBus.Engine`

---

## Task 0: spec update を branch 上で commit

**Files:**
- Modify (既に edit 済、未 commit): `docs/superpowers/specs/2026-05-02-p3-a-ibus-engine-design.md`

このセッション開始時に既に spec の §4.2 末尾(B2 wire format 実装方針)+ §13 Open Q 9 closure + 改訂履歴 r2 を edit 済。差分が working tree に残っているので、まず branch 上で commit する。

- [ ] **Step 1: 差分確認**

```bash
git status -sb
```

Expected:
```
## feature/170-p3b-b2-ibus-signal-marshalling
 M docs/superpowers/specs/2026-05-02-p3-a-ibus-engine-design.md
```

- [ ] **Step 2: 差分 review**

```bash
git diff docs/superpowers/specs/2026-05-02-p3-a-ibus-engine-design.md
```

期待: §4.2 に新 subsection "Phase 3-B B2 での wire format 実装方針"、§13 Open Q 9 に "closure" 注記、改訂履歴 r2 行が追加されている。

- [ ] **Step 3: commit**

```bash
git add docs/superpowers/specs/2026-05-02-p3-a-ibus-engine-design.md
git commit -m "$(cat <<'EOF'
docs(spec): add §4.2 B2 wire-format outline and close §13 Open Q 9 (#170)

Spec revision r2 prepares Phase 3-A spec for the B2 implementation:

- §4.2 — adds a "Phase 3-B B2 での wire format 実装方針" subsection
  spelling out the proxy.rs replacement plan, types.rs introduction,
  failure observability via host_bridge tracing::warn, and the L1 unit
  test strategy (zvariant round-trip, no Connection mock).
- §13 Open Q 9 — closes the "best-effort single-thread" assumption.
  After B0h-d's Arc<Mutex<dyn IMEEngine>>, the dispatcher and engine
  are both thread-safe, and the adapter buffer's Mutex<Vec<Candidate>>
  remains correct.
- 改訂履歴 r2 entry added (2026-05-04).
EOF
)"
```

Expected: commit succeeds, lefthook gitleaks passes (no secrets in spec).

- [ ] **Step 4: 確認**

```bash
git log --oneline -3
```

Expected: 直前 commit が "docs(spec): add §4.2 B2 wire-format outline ..." であること。

---

## Task 1: `types.rs` 新設 + IBus 1.x wire-format types を定義

**Files:**
- Create: `crates/kotoha-engine-ibus/src/types.rs`
- Modify: `crates/kotoha-engine-ibus/src/lib.rs:21-25` (module 宣言追加)

`#[derive(zbus::zvariant::Type, serde::Serialize)]` で IBus 1.x の `IBusAttribute` / `IBusAttrList` / `IBusText` / `IBusLookupTable` を定義する。Phase 3-B B2 では下線・色等の attribute は使わない(plain text のみ)ので `IBusAttrList::empty()` を default、`IBusText::plain(text)` を helper として用意する。

**実装段階の重要事項**: 上記 signature 表は初期推定。実装前に IBus 1.5.x source の以下を直接参照して signature を確定する:
- `src/ibustext.c` の `ibus_text_serialize`
- `src/ibusattribute.c` の `ibus_attribute_serialize`
- `src/ibusattrlist.c` の `ibus_attr_list_serialize`
- `src/ibuslookuptable.c` の `ibus_lookup_table_serialize`

ibus は git: `https://github.com/ibus/ibus`(branch: 1.5.x)。distro shipping の `/usr/include/ibus-1.0/ibustext.h` / `ibus-1.0/ibusserializable.h` もチェック対象。

- [ ] **Step 1: cargo に dependency 追加(必要なら)**

`zbus` は既に `workspace = true` で参照済。`serde` は `kotoha-engine-ibus/Cargo.toml` に未追加なので確認。

```bash
grep -E "^(serde|zbus)" crates/kotoha-engine-ibus/Cargo.toml
```

Expected: `zbus = { workspace = true }` 行が存在、`serde` は無し。

```bash
grep -A1 "^\[workspace.dependencies\]" Cargo.toml | head -20
grep -E "^serde" Cargo.toml | head -5
```

Workspace に `serde` が pin されているか確認。なければ workspace に追加する必要があるが、現状 workspace で `zbus = "5"` が pin 済 → zbus 5 が re-export する `zvariant::Type` derive を使えば `serde` も transitively 利用可能。明示 dep として `serde = { version = "1", features = ["derive"] }` を `kotoha-engine-ibus/Cargo.toml` に追加する。workspace 側にもあれば良し、無ければ workspace に追加する。

- [ ] **Step 2: workspace `Cargo.toml` に serde を追加(無ければ)**

`Cargo.toml`(workspace root)の `[workspace.dependencies]` に以下を確認/追加:

```toml
serde = { version = "1", features = ["derive"] }
```

既に存在していれば skip。

- [ ] **Step 3: `crates/kotoha-engine-ibus/Cargo.toml` に serde 依存を追加**

`[dependencies]` に追加:

```toml
serde = { workspace = true }
```

(zbus 経由で transitively 使えるが、明示 dep の方が clippy `disallowed-types` や `unused-crate-dependencies` で警告されない)

- [ ] **Step 4: `crates/kotoha-engine-ibus/src/types.rs` を新規作成**

```rust
//! IBus 1.x wire-format types(Phase 3-B B2 で導入)。
//!
//! 各 struct は IBus 1.5.x の `IBusSerializable` 互換シリアライゼーションに
//! 準拠する。具体的 signature は IBus source(`src/ibustext.c`,
//! `src/ibuslookuptable.c`)の `_serialize` 関数を参照(spec §4.2 r2)。
//!
//! # Boundary
//!
//! 本 module は engine-core / kotoha-core の domain type を一切参照しない。
//! `IBusEngineSignals` (proxy.rs) が呼び出し側で `kotoha_core::Candidate`
//! → `IBusText` 変換を行う。

use serde::Serialize;
use std::collections::HashMap;
use zbus::zvariant::{Type, Value};

/// IBus 1.x `IBusAttribute`(下線・色 etc 属性)。
///
/// Phase 3-B B2 では未使用(plain text のみ)。
/// signature: `(sa{sv}uuuu)`
#[derive(Debug, Clone, Serialize, Type)]
pub struct IBusAttribute<'a> {
    pub name: &'static str,
    pub attachments: HashMap<String, Value<'a>>,
    pub type_: u32,
    pub value: u32,
    pub start_index: u32,
    pub end_index: u32,
}

impl<'a> IBusAttribute<'a> {
    pub const NAME: &'static str = "IBusAttribute";
}

/// IBus 1.x `IBusAttrList`(IBusAttribute の variant array)。
///
/// Phase 3-B B2 では `attributes: vec![]` のみ使用。
/// signature: `(sa{sv}av)`
#[derive(Debug, Clone, Serialize, Type)]
pub struct IBusAttrList<'a> {
    pub name: &'static str,
    pub attachments: HashMap<String, Value<'a>>,
    pub attributes: Vec<Value<'a>>,
}

impl<'a> IBusAttrList<'a> {
    pub const NAME: &'static str = "IBusAttrList";

    /// 空の attribute list(下線・色 attribute なし)。
    pub fn empty() -> Self {
        Self {
            name: Self::NAME,
            attachments: HashMap::new(),
            attributes: Vec::new(),
        }
    }
}

/// IBus 1.x `IBusText`(preedit / commit text の wire format)。
///
/// signature: `(sa{sv}sv)`
#[derive(Debug, Clone, Serialize, Type)]
pub struct IBusText<'a> {
    pub name: &'static str,
    pub attachments: HashMap<String, Value<'a>>,
    pub text: String,
    pub attrs: Value<'a>,
}

impl<'a> IBusText<'a> {
    pub const NAME: &'static str = "IBusText";

    /// attribute 無しの plain text を生成する。
    ///
    /// # Preconditions
    /// - `text` は UTF-8 文字列(`String` 型保証)
    pub fn plain(text: String) -> Self {
        Self {
            name: Self::NAME,
            attachments: HashMap::new(),
            text,
            attrs: Value::from(IBusAttrList::empty()),
        }
    }
}

/// IBus 1.x `IBusLookupTable`(候補 list の wire format)。
///
/// signature: `(sa{sv}uubbiavav)`
#[derive(Debug, Clone, Serialize, Type)]
pub struct IBusLookupTable<'a> {
    pub name: &'static str,
    pub attachments: HashMap<String, Value<'a>>,
    pub page_size: u32,
    pub cursor_pos: u32,
    pub cursor_visible: bool,
    pub round: bool,
    pub orientation: i32,
    pub candidates: Vec<Value<'a>>,
    pub labels: Vec<Value<'a>>,
}

impl<'a> IBusLookupTable<'a> {
    pub const NAME: &'static str = "IBusLookupTable";

    /// 候補 list から `IBusLookupTable` を生成する。
    ///
    /// page_size = 5(IBus default、後で empirical 調整)
    /// cursor_pos = 0(highlight 位置は engine 側で track、ここでは固定)
    /// cursor_visible = true / round = true / orientation = 1 (vertical)
    /// labels は空(IBus が default 1/2/3... を表示)
    pub fn from_candidates(candidates: Vec<IBusText<'a>>) -> Self {
        Self {
            name: Self::NAME,
            attachments: HashMap::new(),
            page_size: 5,
            cursor_pos: 0,
            cursor_visible: true,
            round: true,
            orientation: 1,
            candidates: candidates.into_iter().map(Value::from).collect(),
            labels: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zbus::zvariant::serialized::{Context, Format};
    use zbus::zvariant::{to_bytes, Endian};

    fn ctx() -> Context {
        Context::new(Format::DBus, Endian::Little, 0)
    }

    #[test]
    fn ibus_text_signature_matches_ibus_1x_spec() {
        assert_eq!(IBusText::SIGNATURE.to_string(), "(sa{sv}sv)");
    }

    #[test]
    fn ibus_attr_list_signature_matches_ibus_1x_spec() {
        assert_eq!(IBusAttrList::SIGNATURE.to_string(), "(sa{sv}av)");
    }

    #[test]
    fn ibus_lookup_table_signature_matches_ibus_1x_spec() {
        assert_eq!(IBusLookupTable::SIGNATURE.to_string(), "(sa{sv}uubbiavav)");
    }

    #[test]
    fn ibus_text_plain_round_trips_through_dbus_format() {
        let text = IBusText::plain("こんにちは".to_string());
        let encoded = to_bytes(ctx(), &text).expect("serialize");
        // round-trip: deserialize as a structured Value
        let value: Value = encoded.deserialize().expect("deserialize").0;
        // structural assertion via Display(Debug)
        let s = format!("{value:?}");
        assert!(s.contains("\"IBusText\""), "name field present");
        assert!(s.contains("\"こんにちは\""), "text field present");
    }

    #[test]
    fn ibus_lookup_table_with_candidates_round_trips() {
        let cands = vec![
            IBusText::plain("こんにちは".to_string()),
            IBusText::plain("今日は".to_string()),
        ];
        let table = IBusLookupTable::from_candidates(cands);
        let encoded = to_bytes(ctx(), &table).expect("serialize");
        let value: Value = encoded.deserialize().expect("deserialize").0;
        let s = format!("{value:?}");
        assert!(s.contains("\"IBusLookupTable\""));
        assert!(s.contains("\"こんにちは\""));
        assert!(s.contains("\"今日は\""));
    }

    #[test]
    fn ibus_lookup_table_empty_candidates_serializes() {
        let table = IBusLookupTable::from_candidates(vec![]);
        let encoded = to_bytes(ctx(), &table).expect("serialize");
        // empty candidates should still serialize
        assert!(!encoded.bytes().is_empty());
    }
}
```

- [ ] **Step 5: `lib.rs` に module 宣言を追加**

修正対象: `crates/kotoha-engine-ibus/src/lib.rs:21-25`

```rust
pub mod dispatcher;
pub mod host_bridge;
pub mod keysym;
pub mod lookup_table;
pub mod proxy;
pub(crate) mod types;
```

`types` module は B2 内部実装の詳細なので `pub(crate)` で十分。下流 crate には export しない。

- [ ] **Step 6: cargo build で types.rs が compile することを確認**

```bash
cargo build -p kotoha-engine-ibus 2>&1 | tail -10
```

Expected: 0 error。warning は `IBusAttribute` の `pub const NAME` が unused でも OK(B2 後の Phase 5 で使用予定)。

ビルドエラーが出る場合の主な原因と対処:
- `Value::from(IBusAttrList::empty())` で `From<IBusAttrList<'_>>` impl がない → `IBusAttrList` を `OwnedValue` 型 derive(`#[derive(Type, Serialize, Value)]`)経由にする。zbus 5 で `Value::new` は object-like 値受領可。代替: `Value::Structure(...)` を手で組む。
- lifetime issue → `'a` を `'static` に固定し helper では `attachments: HashMap::new()` を使う(Value の lifetime 不要のケースあり)。
  代替実装: `attachments` 型を `HashMap<String, OwnedValue>` にして lifetime を消す。

実装時に lifetime + Value::from の組み立てが煩雑であれば、以下の simpler form に切り替える:

```rust
// 簡易版: Value を持たず serde で直接 dispatch する pattern
#[derive(Debug, Clone, Serialize, Type)]
pub struct IBusText {
    pub name: String,
    pub attachments: HashMap<String, OwnedValue>,
    pub text: String,
    pub attrs: OwnedValue,
}
```

`Value` vs `OwnedValue` の選択は zbus 5 の API form 次第。compile 通る方を採用する。

- [ ] **Step 7: cargo test で wire format unit test が通ることを確認**

```bash
cargo test -p kotoha-engine-ibus types:: 2>&1 | tail -10
```

Expected: 5 tests passed (`ibus_text_signature_matches_ibus_1x_spec`, `ibus_attr_list_signature_matches_ibus_1x_spec`, `ibus_lookup_table_signature_matches_ibus_1x_spec`, `ibus_text_plain_round_trips_through_dbus_format`, `ibus_lookup_table_with_candidates_round_trips`, `ibus_lookup_table_empty_candidates_serializes` の 6 つ)。

signature assertion が fail する場合: IBus 1.5.x source(`ibustext.c` 等)の `_serialize` 関数で actual signature を確認、struct 定義を訂正。signature が異なる例:
- `IBusLookupTable.orientation` が IBus 仕様で `i32` ではなく `i32`(signed)→ `i32` のまま
- `cursor_visible` / `round` が `gboolean`(IBus internal は int だが D-Bus では bool 仕様)→ `bool` のまま

- [ ] **Step 8: clippy 確認**

```bash
cargo clippy -p kotoha-engine-ibus --all-targets -- -D warnings 2>&1 | tail -10
```

Expected: 0 error, 0 warning。

- [ ] **Step 9: commit**

```bash
git add crates/kotoha-engine-ibus/src/types.rs crates/kotoha-engine-ibus/src/lib.rs crates/kotoha-engine-ibus/Cargo.toml Cargo.toml
git commit -m "$(cat <<'EOF'
feat(ibus): add IBus 1.x wire-format types module (#170)

Introduces crates/kotoha-engine-ibus/src/types.rs with IBusAttribute /
IBusAttrList / IBusText / IBusLookupTable, all #[derive(Type, Serialize)]
matching IBus 1.5.x serializable signatures (spec §4.2 r2).

Includes 6 L1 unit tests:
- 3 signature assertions ((sa{sv}sv) / (sa{sv}av) / (sa{sv}uubbiavav))
- 2 round-trip (encode → decode → field structure assert)
- 1 empty-candidates serialization

No proxy.rs changes yet — just the type definitions. Wired into emit
path in subsequent commits per the task plan in
docs/superpowers/plans/2026-05-04-feature-170-p3b-b2-ibus-signal-marshalling.md.
EOF
)"
```

---

## Task 2: signal interface name 定数 + path helper

**Files:**
- Modify: `crates/kotoha-engine-ibus/src/proxy.rs:1-30` (file header + const 追加)

D-Bus signal の interface name `org.freedesktop.IBus.Engine` を `pub(crate)` const として proxy.rs 冒頭に定義する(magic string を散らばらせない)。

- [ ] **Step 1: proxy.rs 冒頭に const 追加**

`crates/kotoha-engine-ibus/src/proxy.rs:27-28` の `NOT_YET_IMPLEMENTED` const の **直前** に追加(NOT_YET_IMPLEMENTED は Task 8 で削除する)。

```rust
/// IBus 1.5.x の engine signal が emit される D-Bus interface 名。
///
/// 対応する仕様: IBus 1.5.x `bus/inputcontext.c` の `BUS_INPUT_CONTEXT_GET_INTERFACE`。
pub(crate) const IBUS_ENGINE_INTERFACE: &str = "org.freedesktop.IBus.Engine";
```

- [ ] **Step 2: cargo build / clippy 確認**

```bash
cargo build -p kotoha-engine-ibus 2>&1 | tail -5
cargo clippy -p kotoha-engine-ibus --all-targets -- -D warnings 2>&1 | tail -5
```

Expected: dead_code warning が出る可能性あり(まだ使用箇所無し)。出る場合は `#[allow(dead_code)]` を一時付与し、Task 3 で参照されたら除去する。

- [ ] **Step 3: commit**

```bash
git add crates/kotoha-engine-ibus/src/proxy.rs
git commit -m "feat(ibus): add IBUS_ENGINE_INTERFACE constant for signal emit (#170)"
```

---

## Task 3: `proxy::update_preedit` を実 D-Bus signal emit に置換

**Files:**
- Modify: `crates/kotoha-engine-ibus/src/proxy.rs:64-77`

`Err(NOT_YET_IMPLEMENTED)` を `Message::signal(...).build(&body)?` + `connection.send(&signal)` に置換。`tracing::warn!` を `tracing::debug!` に降格(成功 path の per-signal trace)。

- [ ] **Step 1: 実装を置換**

`update_preedit` method 全体を以下に置換:

```rust
/// `UpdatePreeditText(IBusText, u32 cursor_pos, bool visible)` signal emit。
///
/// signature: `(vub)` (IBusText variant + cursor + visible)
///
/// # Errors
///
/// - signal message build 失敗(D-Bus serialization error)
/// - connection.send 失敗(D-Bus daemon disconnect 等)
pub fn update_preedit(&self, text: &str, cursor: u32, visible: bool) -> Result<()> {
    use crate::types::IBusText;
    use zbus::message::Message;
    use zbus::zvariant::Value;

    tracing::debug!(
        text_len = text.chars().count(),
        cursor,
        visible,
        "IBus update_preedit emit"
    );

    let ibus_text = IBusText::plain(text.to_string());
    let signal = Message::signal(
        self.object_path.as_ref(),
        IBUS_ENGINE_INTERFACE,
        "UpdatePreeditText",
    )?
    .build(&(Value::from(ibus_text), cursor, visible))?;
    self.connection.send(&signal)?;
    Ok(())
}
```

`Message::signal` の引数:
- 第 1 引数: object path(`zbus::zvariant::ObjectPath` を取る変種があるが、`AsRef<str>` でも受ける版がある)。compile error なら `.as_ref()` を `.into()` に変えるなど zbus 5 の API form に合わせる。
- `body` は tuple `&(arg1, arg2, ...)`。signature `(vub)` には `(Value, u32, bool)` を渡す。

- [ ] **Step 2: cargo build**

```bash
cargo build -p kotoha-engine-ibus 2>&1 | tail -10
```

Expected: 0 error。`#[allow(dead_code)]` を `connection` / `object_path` field から外せる(Task 8 でまとめて clean)。

- [ ] **Step 3: spec acceptance に基づく test 追加**

`crates/kotoha-engine-ibus/src/proxy.rs` 末尾に `#[cfg(test)] mod tests { ... }` を新設(まだ無ければ)。テスト方針:Connection mock を使わず、Message::signal の build まで実行して signal の path/interface/member を assert する pure unit test を書く。Connection 不要の form。

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use zbus::message::Message;
    use zbus::zvariant::Value;

    use crate::types::IBusText;

    /// Spec §4.2 r2 acceptance: UpdatePreeditText signal は IBus 1.x interface 名と member 名を持つ。
    #[test]
    fn update_preedit_signal_has_correct_path_interface_member() {
        let object_path = "/org/freedesktop/IBus/Engine/Kotoha";
        let text = IBusText::plain("こ".to_string());
        let signal = Message::signal(
            object_path,
            IBUS_ENGINE_INTERFACE,
            "UpdatePreeditText",
        )
        .expect("path/iface/member valid")
        .build(&(Value::from(text), 1u32, true))
        .expect("build signal body");

        let header = signal.header();
        assert_eq!(header.path().unwrap().as_str(), object_path);
        assert_eq!(header.interface().unwrap().as_str(), "org.freedesktop.IBus.Engine");
        assert_eq!(header.member().unwrap().as_str(), "UpdatePreeditText");
    }
}
```

- [ ] **Step 4: cargo test**

```bash
cargo test -p kotoha-engine-ibus proxy::tests::update_preedit_signal_has_correct_path_interface_member 2>&1 | tail -5
```

Expected: 1 test passed。

API form に合わせて assert の `.as_str()` form を `&str` 比較に変える等、compile 通る形に微調整する。

- [ ] **Step 5: clippy + 全 test 確認**

```bash
cargo clippy -p kotoha-engine-ibus --all-targets -- -D warnings 2>&1 | tail -5
cargo test -p kotoha-engine-ibus 2>&1 | tail -10
```

Expected: 0 warning, 0 fail。

- [ ] **Step 6: commit**

```bash
git add crates/kotoha-engine-ibus/src/proxy.rs
git commit -m "feat(ibus): wire UpdatePreeditText signal emit (#170)"
```

---

## Task 4: `proxy::commit_text` を実 D-Bus signal emit に置換

**Files:**
- Modify: `crates/kotoha-engine-ibus/src/proxy.rs:79-90`

signature `(v)`(IBusText variant only)。

- [ ] **Step 1: 実装を置換**

```rust
/// `CommitText(IBusText)` signal emit。
///
/// signature: `(v)` (IBusText variant)
///
/// # Errors
///
/// - signal message build 失敗(D-Bus serialization error)
/// - connection.send 失敗(D-Bus daemon disconnect 等)
pub fn commit_text(&self, text: &str) -> Result<()> {
    use crate::types::IBusText;
    use zbus::message::Message;
    use zbus::zvariant::Value;

    tracing::debug!(text_len = text.chars().count(), "IBus commit_text emit");

    let ibus_text = IBusText::plain(text.to_string());
    let signal = Message::signal(
        self.object_path.as_ref(),
        IBUS_ENGINE_INTERFACE,
        "CommitText",
    )?
    .build(&(Value::from(ibus_text),))?;
    self.connection.send(&signal)?;
    Ok(())
}
```

注意: tuple body の syntax は `&(Value::from(ibus_text),)` のように **trailing comma 必須**(1要素 tuple)。

- [ ] **Step 2: spec acceptance test 追加**

`mod tests` 内に追加:

```rust
#[test]
fn commit_text_signal_has_correct_member() {
    let signal = Message::signal(
        "/org/freedesktop/IBus/Engine/Kotoha",
        IBUS_ENGINE_INTERFACE,
        "CommitText",
    )
    .expect("path/iface/member valid")
    .build(&(Value::from(IBusText::plain("漢字".to_string())),))
    .expect("build signal body");

    assert_eq!(signal.header().member().unwrap().as_str(), "CommitText");
    assert_eq!(
        signal.header().interface().unwrap().as_str(),
        "org.freedesktop.IBus.Engine"
    );
}
```

- [ ] **Step 3: build / clippy / test**

```bash
cargo build -p kotoha-engine-ibus 2>&1 | tail -3
cargo clippy -p kotoha-engine-ibus --all-targets -- -D warnings 2>&1 | tail -3
cargo test -p kotoha-engine-ibus proxy:: 2>&1 | tail -5
```

Expected: 0 error, 0 warning, all proxy:: tests pass.

- [ ] **Step 4: commit**

```bash
git add crates/kotoha-engine-ibus/src/proxy.rs
git commit -m "feat(ibus): wire CommitText signal emit (#170)"
```

---

## Task 5: `proxy::update_lookup_table` を実 D-Bus signal emit に置換

**Files:**
- Modify: `crates/kotoha-engine-ibus/src/proxy.rs:92-108`

signature `(vb)`(IBusLookupTable variant + visible)。`Vec<Candidate>` → `Vec<IBusText>` 変換は proxy 内で行う(types.rs に kotoha-core 依存を持ち込まない設計、Task 1 boundary 原則通り)。

- [ ] **Step 1: 実装を置換**

```rust
/// `UpdateLookupTable(IBusLookupTable, bool visible)` signal emit。
///
/// signature: `(vb)` (IBusLookupTable variant + visible)
///
/// # Errors
///
/// - signal message build 失敗(D-Bus serialization error)
/// - connection.send 失敗(D-Bus daemon disconnect 等)
pub fn update_lookup_table(
    &self,
    candidates: &[kotoha_core::Candidate],
    visible: bool,
) -> Result<()> {
    use crate::types::{IBusLookupTable, IBusText};
    use zbus::message::Message;
    use zbus::zvariant::Value;

    tracing::debug!(count = candidates.len(), visible, "IBus update_lookup_table emit");

    let ibus_candidates: Vec<IBusText> = candidates
        .iter()
        .map(|c| IBusText::plain(c.surface.clone()))
        .collect();
    let table = IBusLookupTable::from_candidates(ibus_candidates);

    let signal = Message::signal(
        self.object_path.as_ref(),
        IBUS_ENGINE_INTERFACE,
        "UpdateLookupTable",
    )?
    .build(&(Value::from(table), visible))?;
    self.connection.send(&signal)?;
    Ok(())
}
```

`kotoha_core::Candidate.surface` の field 名は実装段階で確認する(`crates/kotoha-core/src/lib.rs` 等で `pub struct Candidate { pub surface: String, ... }` の form)。

- [ ] **Step 2: Candidate field 名の確認**

```bash
grep -A3 "pub struct Candidate" crates/kotoha-core/src/lib.rs | head -10
```

Expected: `surface: String` 相当の field が見える。違う名前(`text` 等)なら実装の `c.surface` を訂正。

- [ ] **Step 3: spec acceptance test 追加**

`mod tests` 内に追加:

```rust
#[test]
fn update_lookup_table_signal_has_correct_member() {
    use crate::types::{IBusLookupTable, IBusText};

    let cands = vec![
        IBusText::plain("漢字".to_string()),
        IBusText::plain("勘事".to_string()),
    ];
    let table = IBusLookupTable::from_candidates(cands);
    let signal = Message::signal(
        "/org/freedesktop/IBus/Engine/Kotoha",
        IBUS_ENGINE_INTERFACE,
        "UpdateLookupTable",
    )
    .expect("path/iface/member valid")
    .build(&(Value::from(table), true))
    .expect("build signal body");

    assert_eq!(signal.header().member().unwrap().as_str(), "UpdateLookupTable");
}
```

- [ ] **Step 4: build / clippy / test**

```bash
cargo build -p kotoha-engine-ibus 2>&1 | tail -3
cargo clippy -p kotoha-engine-ibus --all-targets -- -D warnings 2>&1 | tail -3
cargo test -p kotoha-engine-ibus proxy:: 2>&1 | tail -5
```

Expected: 0 error, 0 warning, all proxy:: tests pass.

- [ ] **Step 5: commit**

```bash
git add crates/kotoha-engine-ibus/src/proxy.rs
git commit -m "feat(ibus): wire UpdateLookupTable signal emit (#170)"
```

---

## Task 6: `proxy::show_lookup_table` を実 D-Bus signal emit に置換

**Files:**
- Modify: `crates/kotoha-engine-ibus/src/proxy.rs:110-118`

signature `()`(empty body)。

- [ ] **Step 1: 実装を置換**

```rust
/// `ShowLookupTable()` signal emit。
///
/// signature: `()` (empty body)
///
/// # Errors
///
/// - signal message build 失敗(D-Bus serialization error)
/// - connection.send 失敗(D-Bus daemon disconnect 等)
pub fn show_lookup_table(&self) -> Result<()> {
    use zbus::message::Message;

    tracing::debug!("IBus show_lookup_table emit");

    let signal = Message::signal(
        self.object_path.as_ref(),
        IBUS_ENGINE_INTERFACE,
        "ShowLookupTable",
    )?
    .build(&())?;
    self.connection.send(&signal)?;
    Ok(())
}
```

`build(&())` は empty tuple = empty body。

- [ ] **Step 2: spec acceptance test 追加**

```rust
#[test]
fn show_lookup_table_signal_has_empty_body_and_correct_member() {
    let signal = Message::signal(
        "/org/freedesktop/IBus/Engine/Kotoha",
        IBUS_ENGINE_INTERFACE,
        "ShowLookupTable",
    )
    .expect("path/iface/member valid")
    .build(&())
    .expect("build empty signal body");

    assert_eq!(signal.header().member().unwrap().as_str(), "ShowLookupTable");
    // empty body は body bytes が 0 byte であることを assert
    assert_eq!(signal.body().data().len(), 0);
}
```

`signal.body().data()` の API form は zbus 5 で異なる可能性あり。compile 通る form(`signal.body().bytes()` 等)に切替。

- [ ] **Step 3: build / clippy / test**

```bash
cargo build -p kotoha-engine-ibus 2>&1 | tail -3
cargo clippy -p kotoha-engine-ibus --all-targets -- -D warnings 2>&1 | tail -3
cargo test -p kotoha-engine-ibus proxy:: 2>&1 | tail -5
```

Expected: 0 error, 0 warning, all proxy:: tests pass.

- [ ] **Step 4: commit**

```bash
git add crates/kotoha-engine-ibus/src/proxy.rs
git commit -m "feat(ibus): wire ShowLookupTable signal emit (#170)"
```

---

## Task 7: `proxy::hide_lookup_table` を実 D-Bus signal emit に置換

**Files:**
- Modify: `crates/kotoha-engine-ibus/src/proxy.rs:120-128`

signature `()`(empty body、Task 6 と同 form)。

- [ ] **Step 1: 実装を置換**

```rust
/// `HideLookupTable()` signal emit。
///
/// signature: `()` (empty body)
///
/// # Errors
///
/// - signal message build 失敗(D-Bus serialization error)
/// - connection.send 失敗(D-Bus daemon disconnect 等)
pub fn hide_lookup_table(&self) -> Result<()> {
    use zbus::message::Message;

    tracing::debug!("IBus hide_lookup_table emit");

    let signal = Message::signal(
        self.object_path.as_ref(),
        IBUS_ENGINE_INTERFACE,
        "HideLookupTable",
    )?
    .build(&())?;
    self.connection.send(&signal)?;
    Ok(())
}
```

- [ ] **Step 2: spec acceptance test 追加**

```rust
#[test]
fn hide_lookup_table_signal_has_empty_body_and_correct_member() {
    let signal = Message::signal(
        "/org/freedesktop/IBus/Engine/Kotoha",
        IBUS_ENGINE_INTERFACE,
        "HideLookupTable",
    )
    .expect("path/iface/member valid")
    .build(&())
    .expect("build empty signal body");

    assert_eq!(signal.header().member().unwrap().as_str(), "HideLookupTable");
    assert_eq!(signal.body().data().len(), 0);
}
```

- [ ] **Step 3: build / clippy / test**

```bash
cargo build -p kotoha-engine-ibus 2>&1 | tail -3
cargo clippy -p kotoha-engine-ibus --all-targets -- -D warnings 2>&1 | tail -3
cargo test -p kotoha-engine-ibus proxy:: 2>&1 | tail -5
```

Expected: 0 error, 0 warning, all proxy:: tests pass.

- [ ] **Step 4: commit**

```bash
git add crates/kotoha-engine-ibus/src/proxy.rs
git commit -m "feat(ibus): wire HideLookupTable signal emit (#170)"
```

---

## Task 8: cleanup(stub の残骸を削除)

**Files:**
- Modify: `crates/kotoha-engine-ibus/src/proxy.rs:1-46` (header rustdoc + dead_code attribute + NOT_YET_IMPLEMENTED const)

Task 3-7 で 5 method すべてが `connection` と `object_path` を使用するようになるため、`#[allow(dead_code)]` を撤去する。`NOT_YET_IMPLEMENTED` const も使用箇所が無くなるため削除する。crate header rustdoc も Phase 3-B B2 完了状態に update する。

- [ ] **Step 1: file header rustdoc を update**

`crates/kotoha-engine-ibus/src/proxy.rs:1-18` を以下に置換:

```rust
//! IBus 1.x D-Bus interface proxy 定義(zbus 5.x blocking API 経由)。
//!
//! `IBusEngineSignals` は IBus engine が host(`InputContext`)に対して発する
//! signal の helper 群。Phase 3-B B2 で 5 method すべてが
//! `Message::signal(...)?.build(&body)?` + `connection.send(&signal)` の形で
//! 実 D-Bus signal を session bus に発信する。
//!
//! signal の wire-format type は [`crate::types`] module で定義(spec §4.2 r2)。
//!
//! # 失敗時挙動
//!
//! 各 method は `Result<(), zbus::Error>` を返し、caller(`host_bridge.rs`)が
//! `tracing::warn!(error = %e, ...)` で集約観測する。signal failure は engine
//! state に伝播させない(spec §9.3「silent_failure 禁止」と「non-propagating」
//! の両立)。
```

- [ ] **Step 2: `NOT_YET_IMPLEMENTED` const 行を削除**

`crates/kotoha-engine-ibus/src/proxy.rs:23-28` の以下を削除:

```rust
/// 5 method すべてに共通する「未実装である」error message。
///
/// caller(`host_bridge.rs`)は本 message を tracing::warn 経由で観測する。
/// Phase 3-B B3 で実 signal emit に置き換わると本 message は廃止される。
const NOT_YET_IMPLEMENTED: &str =
    "IBus signal emit is not yet implemented; tracked in Phase 3-B B3";
```

- [ ] **Step 3: `IBusEngineSignals` struct field の `#[allow(dead_code)]` を撤去**

`crates/kotoha-engine-ibus/src/proxy.rs:37-46` を以下に変更:

```rust
/// IBus engine が host(`InputContext`)に発する signal の helper 群。
///
/// Phase 3-B B2 完了後は 5 method すべてが session bus に実 D-Bus signal を
/// 発信する。
pub struct IBusEngineSignals {
    /// zbus blocking connection(session bus)。
    connection: Connection,
    /// engine object path(`/org/freedesktop/IBus/Engine/Kotoha` 等)。
    object_path: zbus::zvariant::OwnedObjectPath,
}
```

- [ ] **Step 4: cargo build + clippy + test**

```bash
cargo build -p kotoha-engine-ibus 2>&1 | tail -3
cargo clippy -p kotoha-engine-ibus --all-targets -- -D warnings 2>&1 | tail -3
cargo test -p kotoha-engine-ibus 2>&1 | tail -10
```

Expected: 0 error, 0 warning, 全 test pass(types::tests + proxy::tests + 既存 dispatcher::tests / lookup_table::tests / keysym::tests)。

- [ ] **Step 5: commit**

```bash
git add crates/kotoha-engine-ibus/src/proxy.rs
git commit -m "refactor(ibus): remove fail-loud stub residuals from proxy.rs (#170)

After Tasks 3-7 wired all 5 methods to real D-Bus signal emit:
- Update file header rustdoc to reflect B2 完了 state.
- Drop the NOT_YET_IMPLEMENTED const (no callers remain).
- Drop #[allow(dead_code)] from connection / object_path; both fields
  are now read by every method.

Behaviorally unchanged from Task 7."
```

---

## Task 9: Workspace-wide verification + WBS update

**Files:**
- Modify: `docs/wbs/2026-05-02-phase3a-implementation.md` (B2 完了記録)

このセッション末で全 baseline を確認し、WBS log を update して merge 準備。

- [ ] **Step 1: cargo fmt --all**

```bash
cargo fmt --all
```

Expected: 出力なし(既に format 済)。

- [ ] **Step 2: cargo clippy workspace-wide**

```bash
cargo clippy --workspace --all-targets -- -D warnings 2>&1 | tail -10
```

Expected: 0 warning, 0 error。

- [ ] **Step 3: cargo test default features**

```bash
cargo test --workspace 2>&1 | grep -E "test result:|FAILED" | tail -40
```

Expected: 全 result が `ok`、追加された 6+5 = 11 tests 分だけ test count が増加(504 → 515 程度)。0 FAIL。

新 baseline の正確な数を記録(commit message / WBS で参照)。

- [ ] **Step 4: cargo test test-helpers features**

```bash
cargo test --workspace --features kotoha-storage/test-helpers,kotoha-ranker-hybrid/test-helpers 2>&1 | grep -E "test result:|FAILED" | tail -40
```

Expected: 全 result が `ok`、+11 tests 増(515 → 526 程度)。0 FAIL。

- [ ] **Step 5: KOTOHA_ALLOW_STUB exit code 確認**

```bash
KOTOHA_ALLOW_STUB=1 ./target/debug/kotoha; echo "exit=$?"
```

Expected: `exit=1`(B0h-e gate intact)。

- [ ] **Step 6: release stub symbol 確認**

```bash
cargo build --release -p kotoha-bin 2>&1 | tail -3
nm target/release/kotoha 2>/dev/null | grep -ci stub
```

Expected: 0(B0h-e release gate intact)。

- [ ] **Step 7: WBS update**

`docs/wbs/2026-05-02-phase3a-implementation.md` 末尾(or 適切な section)に以下を追記:

```markdown
## Phase 3-B B2(2026-05-04)— IBus signal body marshalling 完了

| 項目 | 値 |
|---|---|
| ISSUE | #170 |
| Branch | feature/170-p3b-b2-ibus-signal-marshalling |
| 主要 commit | (PR merge 後の squash hash を後追記) |
| Test count(default)| 504 → <new>(+ <delta>) |
| Test count(test-helpers)| 515 → <new>(+ <delta>) |
| KOTOHA_ALLOW_STUB exit | 1(B0h-e gate intact) |
| Release stub symbol 数 | 0(B0h-e release gate intact) |

### 主要変更
- 新 file `crates/kotoha-engine-ibus/src/types.rs`:`IBusAttribute` / `IBusAttrList` / `IBusText` / `IBusLookupTable` を `#[derive(zbus::zvariant::Type, serde::Serialize)]` で定義(IBus 1.5.x serializable signature 準拠)。
- `crates/kotoha-engine-ibus/src/proxy.rs` 5 method:`Err(zbus::Error::Failure(NOT_YET_IMPLEMENTED))` の fail-loud stub を `Message::signal(path, interface, member)?.build(&body)?` + `connection.send(&signal)` の実 D-Bus signal emit に置換。`tracing::warn!` を `tracing::debug!` に降格(per-signal trace)。
- `crates/kotoha-engine-ibus/src/lib.rs`:`pub(crate) mod types;` を追加(crate 内部限定)。
- spec `docs/superpowers/specs/2026-05-02-p3-a-ibus-engine-design.md` r2:§4.2 末尾に B2 wire-format 実装方針 subsection 追加、§13 Open Q 9 を closure(B0h-d で thread-safe 化済)。

### 残タスク(B3 以降)
- B3 `kotoha-bin/src/main.rs::run_ibus()` の `zbus::blocking::MessageStream` 経由 signal listener loop。
- B0h-f I3 `dispatch_rank_request` async-ification(spec §6.1 / §7 ADR 必須)。
- B6 L3 manual smoke on GNOME Wayland。
```

- [ ] **Step 8: commit**

```bash
git add docs/wbs/2026-05-02-phase3a-implementation.md
git commit -m "docs(wbs): record Phase 3-B B2 completion (#170)"
```

---

## Task 10: Self-review + PR 作成 + 全体レビュー

**Files:** なし(meta 作業)

- [ ] **Step 1: pr-review-toolkit:silent-failure-hunter で self-review**

`Skill` tool 経由で `pr-review-toolkit:silent-failure-hunter` を invoke し、本 branch の diff を review してもらう。focus 項目:
- `tracing::debug!` 降格は silent failure を生まないか?(降格は ok、同等の WARN は host_bridge 側で集約する設計)
- `connection.send` 失敗時に engine state を壊さない invariant が維持されているか?
- `NOT_YET_IMPLEMENTED` const 削除で参照漏れがないか?

findings は inline fix し、commit 追加。

- [ ] **Step 2: PR 作成(commit-commands:commit-push-pr スキル)**

```bash
git push -u origin feature/170-p3b-b2-ibus-signal-marshalling
gh pr create --base develop \
  --title "P3-B B2: IBus signal body marshalling — replace fail-loud stubs (#170)" \
  --body "(template per CLAUDE.md commit policy)"
```

Body 構成:
- Summary(3-5 行)
- Test plan(下記 acceptance criteria 全項目を checkbox で)
- Reference(spec §4.2 r2、ISSUE #170、parent #136)

- [ ] **Step 3: 全体 review 並列起動**

PR Size 判定: ≤ 15 files / ≤ 600 lines → **Medium tier**(CLAUDE.md PR Review Matrix 参照)。

並列起動する review:
- `agent-teams:team-review`(security + architecture + testing + accessibility + performance、5 dim full)
- `secrets-check`(常に at PR creation)
- `owasp-security`(Medium tier 必須)
- `security-scanning:security-sast`(Medium tier add-on)

trigger-based add-ons は無し(UI なし、auth/crypto なし、DB schema なし、LLM なし)。

- [ ] **Step 4: findings 解消**

各 review の Critical / Important findings を inline fix し、commit 追加。Minor / Nit は別 ISSUE 起票で deferral 可能。

- [ ] **Step 5: re-review(必要なら)+ merge**

Critical / Important fix 後、必要なら該当 review を re-run。green になれば squash merge。

```bash
gh pr merge --squash --delete-branch
```

merge 後 develop に戻り、squash commit hash を WBS に追記する別 commit を develop に直接 push(CLAUDE.md WBS 直接 push 例外規約に基づく)。

---

## Spec coverage 自己 check

| Spec 要件(§4.2 r2 / §13 Open Q 9) | 対応 task |
|---|---|
| `IBusText` / `IBusAttribute` / `IBusLookupTable` を `#[derive(Type, Serialize)]` で定義 | Task 1 |
| 5 method の `Err(...)` stub を `connection.send_signal(...)` に置換 | Task 3-7 |
| signal interface `org.freedesktop.IBus.Engine` を const 化 | Task 2 |
| 失敗時 `Result<(), zbus::Error>` を caller に propagate(host_bridge は warn 観測) | Task 3-7 (host_bridge は変更なし、既存の warn path を維持) |
| L1 unit test:wire format round-trip | Task 1 (6 tests) + Task 3-7 (5 tests) |
| Connection mock 不使用、B6 で実機検証 | Task 1-7 全体方針 |
| `tracing::warn!` → `tracing::debug!` 降格 | Task 3-7 |
| spec 改訂履歴 r2 commit | Task 0 |
| §13 Open Q 9 closure(B0h-d で thread-safe 化済) | Task 0(spec 内記述) |

| ISSUE #170 acceptance criteria | 対応 task |
|---|---|
| 5 method すべて real signal emit に置換 | Task 3-7 |
| `cargo fmt --all` clean | Task 9 Step 1 |
| `cargo clippy --workspace --all-targets -- -D warnings` clean | Task 9 Step 2 |
| `cargo test --workspace` 0 regression vs 504 | Task 9 Step 3 |
| `cargo test --workspace --features ...test-helpers` 0 regression vs 515 | Task 9 Step 4 |
| 新 unit test ≥ 6 | Task 1 (6) + Task 3-7 (5) = **合計 11** |
| `KOTOHA_ALLOW_STUB=1` exit=1 | Task 9 Step 5 |
| `nm target/release/kotoha \| grep -ci stub` == 0 | Task 9 Step 6 |
| 失敗 path で engine state 改変なし | Task 3-7 host_bridge 変更なしで担保 |

---

## 補足:engineer 向け注意事項

1. **lefthook pre-commit / pre-push を skip しない**:`--no-verify` は CLAUDE.md で禁止。失敗したら原因を調べる(format / clippy / secret scan)。
2. **commit message は英語**:Kotoha プロジェクト例外規約。日本語は spec / plan / ADR / WBS / コード内コメントのみ。
3. **commit 単位は task 単位**:1 task = 1 commit。fixup は origin push 前のみ許可。
4. **D-Bus daemon が CI に無い前提で test を書く**:`Connection::session()` を使う test は CI で flake する → 全 test を Message build までに止める。
5. **IBus signature が間違っていたら**:Task 1 Step 7 の signature assertion が FAIL する。IBus source `src/ibustext.c` 等を直接確認して訂正、合わせて plan を update して同じ問題が再発しないようにする。
6. **lifetime 問題で詰まったら**:`'a` を `'static` に固定して `attachments: HashMap::new()` で済ませる、または `OwnedValue` 型を使って lifetime を消す。
