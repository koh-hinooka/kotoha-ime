//! IBus 1.x wire-format types(Phase 3-B B2 で導入)。
//!
//! 各 struct は IBus 1.5.x の `IBusSerializable` 互換シリアライゼーションに
//! 準拠する。具体的 serialize 順序は `src/ibusserializable.c::ibus_serializable_serialize_object`
//! と `src/ibustext.c::ibus_text_serialize` を参照(spec §4.2 r3)。
//!
//! IBusSerializable 親クラスが先頭 2 field(type-name string + attachments dict)を書き、
//! 子クラスが `g_variant_builder_add` で残り field を append する。結果として:
//!
//! - `IBusAttribute`: `(s a{sv} u u u u)` — name / attachments / type / value / start / end
//! - `IBusAttrList`:  `(s a{sv} av)` — name / attachments / attributes(variant array)
//! - `IBusText`:      `(s a{sv} s v)` — name / attachments / text / attrs
//! - `IBusLookupTable`: `(s a{sv} u u b b i av av)` — name / attachments / page_size /
//!   cursor_pos / cursor_visible / round / orientation / candidates / labels
//!
//! # 設計判断:Value<'static> ベース表現
//!
//! Phase 3-B B2 では `#[derive(Type, Serialize)]` ではなく、各 type を data struct
//! として持ち `into_variant() -> Value<'static>` で D-Bus value に変換する form を
//! 採用する。理由:
//!
//! - zbus 5 の `OwnedValue::try_from(Value)` は variant signature `v` のみを受ける
//!   ため、Structure を直接 OwnedValue 化できない。
//! - `Value::Structure(...)` を手で組む方式は冗長だが、zvariant 5 の制約と整合し、
//!   IBusSerializable の variant ラップを 1:1 で表現できる。
//! - 内部表現は `attrs: IBusAttrList` / `candidates: Vec<IBusText>` のように
//!   原型を保持し、wire 化は `into_variant()` で 1 段だけ行う。これで型情報を
//!   構築 path 全体で保持できる。
//!
//! # Boundary
//!
//! 本 module は engine-core / kotoha-core の domain type を一切参照しない。
//! `IBusEngineSignals`(`proxy.rs`)が呼び出し側で `kotoha_core::Candidate`
//! → `IBusText` 変換を行う。

use std::collections::HashMap;
use zbus::zvariant::{Structure, Value};

/// IBusSerializable 派生型が共通で持つ "type-name + attachments" を Value::Structure
/// の先頭 2 field として組み立てるための内部 helper。
fn empty_attachments() -> HashMap<String, Value<'static>> {
    HashMap::new()
}

/// IBus 1.x `IBusAttribute`(下線・色 etc 属性)。
///
/// Phase 3-B B2 では未使用(plain text のみ送信)。Phase 5 で attribute
/// 付き preedit を出すときに helper を生やすため `#[allow(dead_code)]` で
/// 残置する。`type_` を `enum AttributeKind` に昇格し field を `pub(crate)` 化
/// する作業も Phase 5 と同時に実施する(B5 でなく Phase 5、`docs/wbs/...` で追跡)。
///
/// signature: `(s a{sv} u u u u)`
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct IBusAttribute {
    pub type_: u32,
    pub value: u32,
    pub start_index: u32,
    pub end_index: u32,
}

#[allow(dead_code)]
impl IBusAttribute {
    /// IBusSerializable type-name(class lookup 用、IBus daemon 側で `g_type_from_name`)。
    ///
    /// ```
    /// use kotoha_engine_ibus::types_test_export::IBusAttribute;
    /// assert_eq!(IBusAttribute::NAME, "IBusAttribute");
    /// ```
    pub const NAME: &'static str = "IBusAttribute";

    /// D-Bus variant value に変換する(signature `(sa{sv}uuuu)`)。
    pub fn into_variant(self) -> Value<'static> {
        Value::Structure(Structure::from((
            String::from(Self::NAME),
            empty_attachments(),
            self.type_,
            self.value,
            self.start_index,
            self.end_index,
        )))
    }
}

/// IBus 1.x `IBusAttrList`(`IBusAttribute` の variant array)。
///
/// Phase 3-B B2 では `attributes: vec![]` のみ使用(下線・色 attribute なし)。
/// 内部表現は `Vec<IBusAttribute>` で type-information を保持する。
///
/// signature: `(s a{sv} av)`
#[derive(Debug, Clone)]
pub struct IBusAttrList {
    pub attributes: Vec<IBusAttribute>,
}

impl IBusAttrList {
    /// IBusSerializable type-name。
    ///
    /// ```
    /// use kotoha_engine_ibus::types_test_export::IBusAttrList;
    /// assert_eq!(IBusAttrList::NAME, "IBusAttrList");
    /// ```
    pub const NAME: &'static str = "IBusAttrList";

    /// 空の attribute list(下線・色 attribute なし)を生成する。
    pub fn empty() -> Self {
        Self {
            attributes: Vec::new(),
        }
    }

    /// D-Bus variant value に変換する(signature `(sa{sv}av)`)。
    pub fn into_variant(self) -> Value<'static> {
        let attrs: Vec<Value<'static>> = self
            .attributes
            .into_iter()
            .map(IBusAttribute::into_variant)
            .collect();
        Value::Structure(Structure::from((
            String::from(Self::NAME),
            empty_attachments(),
            attrs,
        )))
    }
}

/// IBus 1.x `IBusText`(preedit / commit / candidate text の wire format)。
///
/// 内部表現は `attrs: IBusAttrList` で type-information を保持。
///
/// signature: `(s a{sv} s v)`
#[derive(Debug, Clone)]
pub struct IBusText {
    pub text: String,
    pub attrs: IBusAttrList,
}

impl IBusText {
    /// IBusSerializable type-name。
    ///
    /// ```
    /// use kotoha_engine_ibus::types_test_export::IBusText;
    /// assert_eq!(IBusText::NAME, "IBusText");
    /// ```
    pub const NAME: &'static str = "IBusText";

    /// attribute 無しの plain text を生成する。
    ///
    /// # Preconditions
    ///
    /// - `text` は UTF-8 文字列(`String` 型保証)
    ///
    /// # Postconditions
    ///
    /// - `text` field が引数 `text` と等しい
    /// - `attrs` field は空 `IBusAttrList`
    pub fn plain(text: String) -> Self {
        Self {
            text,
            attrs: IBusAttrList::empty(),
        }
    }

    /// D-Bus variant value に変換する(signature `(sa{sv}sv)`)。
    pub fn into_variant(self) -> Value<'static> {
        Value::Structure(Structure::from((
            String::from(Self::NAME),
            empty_attachments(),
            self.text,
            self.attrs.into_variant(),
        )))
    }
}

/// IBus 1.x `IBusLookupTable`(候補 list の wire format)。
///
/// 内部表現は `candidates: Vec<IBusText>` / `labels: Vec<IBusText>` で
/// type-information を保持。
///
/// signature: `(s a{sv} u u b b i av av)`
///
/// IBus 1.5.x `src/ibuslookuptable.c::ibus_lookup_table_serialize` の
/// field 順序に準拠:
///   page_size / cursor_pos / cursor_visible / round / orientation /
///   candidates(variant array of IBusText)/ labels(variant array of IBusText)
#[derive(Debug, Clone)]
pub struct IBusLookupTable {
    pub page_size: u32,
    pub cursor_pos: u32,
    pub cursor_visible: bool,
    pub round: bool,
    pub orientation: i32,
    pub candidates: Vec<IBusText>,
    pub labels: Vec<IBusText>,
}

/// `IBusLookupTable::from_candidates` が hardcode する IBus 1.5.x default。
/// 値の意味は `IBusLookupTable` の field 単位 doc を参照。
pub const DEFAULT_PAGE_SIZE: u32 = 5;
/// `IBusLookupTable::from_candidates` が hardcode する初期 cursor 位置。
pub const DEFAULT_CURSOR_POS: u32 = 0;
/// IBus 1.x `IBUS_ORIENTATION_HORIZONTAL`。
#[allow(dead_code)]
pub const ORIENTATION_HORIZONTAL: i32 = 0;
/// IBus 1.x `IBUS_ORIENTATION_VERTICAL`。Phase 3-B B2 default。
pub const ORIENTATION_VERTICAL: i32 = 1;
/// IBus 1.x `IBUS_ORIENTATION_SYSTEM`。
#[allow(dead_code)]
pub const ORIENTATION_SYSTEM: i32 = 2;

impl IBusLookupTable {
    /// IBusSerializable type-name。
    ///
    /// ```
    /// use kotoha_engine_ibus::types_test_export::IBusLookupTable;
    /// assert_eq!(IBusLookupTable::NAME, "IBusLookupTable");
    /// ```
    pub const NAME: &'static str = "IBusLookupTable";

    /// 候補 list から `IBusLookupTable` を生成する。
    ///
    /// IBus 1.5.x default に揃えた値:
    /// - `page_size = DEFAULT_PAGE_SIZE`(5、後で empirical 調整)
    /// - `cursor_pos = DEFAULT_CURSOR_POS`(0、highlight 位置は engine 側で track)
    /// - `cursor_visible = true`
    /// - `round = true`
    /// - `orientation = ORIENTATION_VERTICAL`(1)
    /// - `labels = []`(IBus が default 1/2/3... を表示)
    pub fn from_candidates(candidates: Vec<IBusText>) -> Self {
        Self {
            page_size: DEFAULT_PAGE_SIZE,
            cursor_pos: DEFAULT_CURSOR_POS,
            cursor_visible: true,
            round: true,
            orientation: ORIENTATION_VERTICAL,
            candidates,
            labels: Vec::new(),
        }
    }

    /// D-Bus variant value に変換する(signature `(sa{sv}uubbiavav)`)。
    pub fn into_variant(self) -> Value<'static> {
        let cands: Vec<Value<'static>> = self
            .candidates
            .into_iter()
            .map(IBusText::into_variant)
            .collect();
        let labels: Vec<Value<'static>> = self
            .labels
            .into_iter()
            .map(IBusText::into_variant)
            .collect();
        Value::Structure(Structure::from((
            String::from(Self::NAME),
            empty_attachments(),
            self.page_size,
            self.cursor_pos,
            self.cursor_visible,
            self.round,
            self.orientation,
            cands,
            labels,
        )))
    }
}

/// `Value` から expected `Signature` を構築する test-only helper。
/// signature 比較を文字列ではなく型レベルで行うため(zvariant 5 の
/// Display format 変更に test を依存させない)。
#[cfg(test)]
fn sig(s: &str) -> zbus::zvariant::Signature {
    zbus::zvariant::Signature::try_from(s).expect("test signature literal must be valid")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Spec §4.2 r3 acceptance: IBusAttribute の wire-format signature が
    /// IBus 1.x `(sa{sv}uuuu)` と一致する(`Signature` 型比較)。
    #[test]
    fn ibus_attribute_signature_matches_ibus_1x_spec() {
        let v = IBusAttribute {
            type_: 1,
            value: 0xFF0000,
            start_index: 0,
            end_index: 3,
        }
        .into_variant();
        assert_eq!(v.value_signature(), &sig("(sa{sv}uuuu)"));
    }

    /// Spec §4.2 r3 acceptance: IBusAttrList の wire-format signature が
    /// IBus 1.x `(sa{sv}av)` と一致する。
    #[test]
    fn ibus_attr_list_signature_matches_ibus_1x_spec() {
        let v = IBusAttrList::empty().into_variant();
        assert_eq!(v.value_signature(), &sig("(sa{sv}av)"));
    }

    /// Spec §4.2 r3 acceptance: IBusText の wire-format signature が
    /// IBus 1.x `(sa{sv}sv)` と一致する。
    #[test]
    fn ibus_text_signature_matches_ibus_1x_spec() {
        let v = IBusText::plain("テスト".to_string()).into_variant();
        assert_eq!(v.value_signature(), &sig("(sa{sv}sv)"));
    }

    /// Spec §4.2 r3 acceptance: IBusLookupTable の wire-format signature が
    /// IBus 1.x `(sa{sv}uubbiavav)` と一致する。
    #[test]
    fn ibus_lookup_table_signature_matches_ibus_1x_spec() {
        let v = IBusLookupTable::from_candidates(vec![]).into_variant();
        assert_eq!(v.value_signature(), &sig("(sa{sv}uubbiavav)"));
    }

    /// `Value::Structure` から field を取り出して field 順序を厳密に assert する
    /// 共通 helper。lifetime は引数の Value に紐付くため、各 field を `try_clone`
    /// しつつ borrow 関係を保つ。
    fn structure_fields<'a>(v: &'a Value<'a>) -> Vec<Value<'a>> {
        match v {
            Value::Structure(s) => s.fields().iter().map(|f| f.try_clone().unwrap()).collect(),
            other => panic!("expected Value::Structure, got {other:?}"),
        }
    }

    /// Spec §4.2 r3 acceptance: IBusText が parent type-name + attachments + text +
    /// attrs の **正しい順序** で組まれている(structural assertion、Debug substring
    /// では検出できない field-swap regression を捕捉)。
    #[test]
    fn ibus_text_plain_has_correct_field_order_and_values() {
        let v = IBusText::plain("こんにちは".to_string()).into_variant();
        let fields = structure_fields(&v);
        assert_eq!(fields.len(), 4, "(s a{{sv}} s v) は 4 field");
        // field 0: type-name "IBusText" (s)
        let name = String::try_from(fields[0].try_clone().unwrap()).expect("field 0: String");
        assert_eq!(name, "IBusText");
        // field 1: attachments a{sv} (空)
        assert_eq!(fields[1].value_signature(), &sig("a{sv}"));
        // field 2: text (s)
        let text = String::try_from(fields[2].try_clone().unwrap()).expect("field 2: String");
        assert_eq!(text, "こんにちは");
        // field 3: attrs は variant wrap('v')、unwrap した中身が IBusAttrList の
        // signature `(sa{sv}av)` を持つ。zvariant 5 は Tuple の Value 型 field を
        // 自動的に Variant でラップする(IBus 1.x の `(sa{sv}sv)` 期待と一致)。
        match &fields[3] {
            Value::Value(inner) => {
                assert_eq!(inner.value_signature(), &sig("(sa{sv}av)"));
            }
            other => panic!("field 3 must be Variant wrapping IBusAttrList, got {other:?}"),
        }
    }

    /// Spec §4.2 r3 acceptance: IBusLookupTable が parent + scalar 5 field +
    /// candidates + labels の **正しい順序** で組まれている。
    #[test]
    fn ibus_lookup_table_from_candidates_has_correct_field_order_and_defaults() {
        let cands = vec![
            IBusText::plain("こんにちは".to_string()),
            IBusText::plain("今日は".to_string()),
        ];
        let v = IBusLookupTable::from_candidates(cands).into_variant();
        let fields = structure_fields(&v);
        assert_eq!(fields.len(), 9, "(s a{{sv}} u u b b i av av) は 9 field");
        // field 0: type-name
        let name = String::try_from(fields[0].try_clone().unwrap()).expect("field 0: String");
        assert_eq!(name, "IBusLookupTable");
        // field 1: attachments
        assert_eq!(fields[1].value_signature(), &sig("a{sv}"));
        // field 2-6: hardcoded defaults
        assert_eq!(
            u32::try_from(fields[2].try_clone().unwrap()).unwrap(),
            DEFAULT_PAGE_SIZE
        );
        assert_eq!(
            u32::try_from(fields[3].try_clone().unwrap()).unwrap(),
            DEFAULT_CURSOR_POS
        );
        assert!(
            bool::try_from(fields[4].try_clone().unwrap()).unwrap(),
            "cursor_visible default true"
        );
        assert!(
            bool::try_from(fields[5].try_clone().unwrap()).unwrap(),
            "round default true"
        );
        assert_eq!(
            i32::try_from(fields[6].try_clone().unwrap()).unwrap(),
            ORIENTATION_VERTICAL
        );
        // field 7: candidates av — Array of variants. zvariant 5 では Array<Variant> の
        // 各 element は in-memory では生 Value(Structure)のまま保持され、wire serialize
        // 時に 'v' タグが付く非対称設計(Tuple の Value field は in-memory で Value::Value
        // ラップ、Array の element は wire 時のみラップ、と挙動が異なる)。
        assert_eq!(fields[7].value_signature(), &sig("av"));
        if let Value::Array(a) = &fields[7] {
            assert_eq!(a.len(), 2, "2 candidates");
            // candidates[0] is IBusText struct (in-memory shape)
            let cand0: Value<'_> = a.get(0).unwrap().unwrap();
            assert_eq!(cand0.value_signature(), &sig("(sa{sv}sv)"));
        } else {
            panic!("field 7 must be Array");
        }
        // field 8: labels av (空)
        assert_eq!(fields[8].value_signature(), &sig("av"));
        if let Value::Array(a) = &fields[8] {
            assert_eq!(a.len(), 0, "labels empty by default");
        } else {
            panic!("field 8 must be Array");
        }
    }

    /// Spec §4.2 r3 acceptance: 候補配列が空でも正しい signature と field 数で構築可能。
    #[test]
    fn ibus_lookup_table_empty_candidates_keeps_field_count() {
        let v = IBusLookupTable::from_candidates(vec![]).into_variant();
        let fields = structure_fields(&v);
        assert_eq!(fields.len(), 9);
        assert_eq!(v.value_signature(), &sig("(sa{sv}uubbiavav)"));
    }

    // ---- Boundary inputs (Test review Medium #4) ----

    /// 空文字列 preedit が type-name + 空 text の signature を持つことを確認。
    #[test]
    fn ibus_text_plain_empty_string_preserves_signature() {
        let v = IBusText::plain(String::new()).into_variant();
        assert_eq!(v.value_signature(), &sig("(sa{sv}sv)"));
        let fields = structure_fields(&v);
        let text = String::try_from(fields[2].try_clone().unwrap()).unwrap();
        assert!(text.is_empty());
    }

    /// SMP code point(emoji)を含む text が UTF-8 として正しく載る。
    #[test]
    fn ibus_text_plain_with_smp_emoji_preserves_text() {
        let v = IBusText::plain("お祝い 🎉🎊".to_string()).into_variant();
        let fields = structure_fields(&v);
        let text = String::try_from(fields[2].try_clone().unwrap()).unwrap();
        assert_eq!(text, "お祝い 🎉🎊");
    }

    /// 10 KiB 程度の長い preedit でも構造が壊れない。
    #[test]
    fn ibus_text_plain_with_10k_chars_preserves_signature() {
        let long = "あ".repeat(3500); // ~10.5 KiB UTF-8
        let v = IBusText::plain(long).into_variant();
        assert_eq!(v.value_signature(), &sig("(sa{sv}sv)"));
    }

    /// embedded NUL を持つ text の挙動を pin する:`into_variant` 自体は構造を組むだけで
    /// panic しない。NUL の D-Bus 仕様違反は signal emit 時 zvariant エラーで surface し
    /// `host_bridge::tracing::warn!` で観測される(spec §9.3 non-propagating)。
    /// 本 test は in-memory Structure 構築が NUL でも graceful であることを保証する。
    #[test]
    fn ibus_text_plain_with_embedded_nul_constructs_without_panic() {
        let v = IBusText::plain("a\0b".to_string()).into_variant();
        assert_eq!(v.value_signature(), &sig("(sa{sv}sv)"));
        // NUL 含み text が field 2 に保持される(serialize 時の rejection は
        // proxy.rs テストで pin)
        let fields = structure_fields(&v);
        let text = String::try_from(fields[2].try_clone().unwrap()).unwrap();
        assert_eq!(text, "a\0b");
    }

    /// 50 候補 lookup table が signature と field 数を保つ。
    #[test]
    fn ibus_lookup_table_with_50_candidates_keeps_structure() {
        let cands: Vec<IBusText> = (0..50)
            .map(|i| IBusText::plain(format!("候補{i}")))
            .collect();
        let v = IBusLookupTable::from_candidates(cands).into_variant();
        assert_eq!(v.value_signature(), &sig("(sa{sv}uubbiavav)"));
        let fields = structure_fields(&v);
        match &fields[7] {
            Value::Array(a) => assert_eq!(a.len(), 50),
            other => panic!("expected Array, got {other:?}"),
        }
    }

    /// Spec §4.2 r3 acceptance + Testing review Low: field 4 (`cursor_visible`)
    /// と field 5 (`round`) は同 `bool` 型のため `from_candidates` default
    /// (双方 true)では swap regression を検出できない。distinguishing 値で
    /// 検出する追加 test。
    #[test]
    fn ibus_lookup_table_field_4_5_distinguish_cursor_visible_from_round() {
        let mut t = IBusLookupTable::from_candidates(vec![]);
        t.cursor_visible = true;
        t.round = false;
        let v = t.into_variant();
        let fields = structure_fields(&v);
        assert!(
            bool::try_from(fields[4].try_clone().unwrap()).unwrap(),
            "field 4 must be cursor_visible=true"
        );
        assert!(
            !bool::try_from(fields[5].try_clone().unwrap()).unwrap(),
            "field 5 must be round=false"
        );
    }
}
