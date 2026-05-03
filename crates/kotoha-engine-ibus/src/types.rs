// Phase 3-B B2 段階的実装:Task 3-7 で proxy.rs から各 type の `into_variant`
// が呼ばれる。Task 1(本 module 単独)時点では未参照のため一時的に許容する。
// `IBusAttribute` は B5 で attribute 付き preedit 実装時まで未使用。
// Task 8 で proxy 配線完了後、本 attribute を撤去する。
#![allow(dead_code)]

//! IBus 1.x wire-format types(Phase 3-B B2 で導入)。
//!
//! 各 struct は IBus 1.5.x の `IBusSerializable` 互換シリアライゼーションに
//! 準拠する。具体的 serialize 順序は `src/ibusserializable.c::ibus_serializable_serialize_object`
//! と `src/ibustext.c::ibus_text_serialize` を参照(spec §4.2 r2)。
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
//! - `Type` derive を使うと `attrs: OwnedValue` field の signature 解決経路で同じ
//!   問題が再現する。
//! - `Value::Structure(...)` を手で組む方式は冗長だが、zvariant 5 の制約と整合し、
//!   IBusSerializable の variant ラップを 1:1 で表現できる。
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
/// 付き preedit を出すときに helper を生やす。
///
/// signature: `(s a{sv} u u u u)`
#[derive(Debug, Clone)]
pub struct IBusAttribute {
    pub type_: u32,
    pub value: u32,
    pub start_index: u32,
    pub end_index: u32,
}

impl IBusAttribute {
    /// IBusSerializable type-name(class lookup 用、IBus daemon 側で `g_type_from_name`)。
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
///
/// signature: `(s a{sv} av)`
#[derive(Debug, Clone, Default)]
pub struct IBusAttrList {
    pub attributes: Vec<Value<'static>>,
}

impl IBusAttrList {
    /// IBusSerializable type-name。
    pub const NAME: &'static str = "IBusAttrList";

    /// 空の attribute list(下線・色 attribute なし)を生成する。
    pub fn empty() -> Self {
        Self {
            attributes: Vec::new(),
        }
    }

    /// D-Bus variant value に変換する(signature `(sa{sv}av)`)。
    pub fn into_variant(self) -> Value<'static> {
        Value::Structure(Structure::from((
            String::from(Self::NAME),
            empty_attachments(),
            self.attributes,
        )))
    }
}

/// IBus 1.x `IBusText`(preedit / commit / candidate text の wire format)。
///
/// signature: `(s a{sv} s v)`
#[derive(Debug, Clone)]
pub struct IBusText {
    pub text: String,
    pub attrs: Value<'static>,
}

impl IBusText {
    /// IBusSerializable type-name。
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
    /// - `attrs` field は空 `IBusAttrList` の variant value
    pub fn plain(text: String) -> Self {
        Self {
            text,
            attrs: IBusAttrList::empty().into_variant(),
        }
    }

    /// D-Bus variant value に変換する(signature `(sa{sv}sv)`)。
    pub fn into_variant(self) -> Value<'static> {
        Value::Structure(Structure::from((
            String::from(Self::NAME),
            empty_attachments(),
            self.text,
            self.attrs,
        )))
    }
}

/// IBus 1.x `IBusLookupTable`(候補 list の wire format)。
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
    pub candidates: Vec<Value<'static>>,
    pub labels: Vec<Value<'static>>,
}

impl IBusLookupTable {
    /// IBusSerializable type-name。
    pub const NAME: &'static str = "IBusLookupTable";

    /// 候補 list から `IBusLookupTable` を生成する。
    ///
    /// IBus 1.5.x default に揃えた値:
    /// - `page_size = 5`(IBus default、後で empirical 調整)
    /// - `cursor_pos = 0`(highlight 位置は engine 側で track、ここでは固定)
    /// - `cursor_visible = true`
    /// - `round = true`
    /// - `orientation = 1`(`IBUS_ORIENTATION_VERTICAL`)
    /// - `labels = []`(IBus が default 1/2/3... を表示)
    pub fn from_candidates(candidates: Vec<IBusText>) -> Self {
        Self {
            page_size: 5,
            cursor_pos: 0,
            cursor_visible: true,
            round: true,
            orientation: 1,
            candidates: candidates.into_iter().map(IBusText::into_variant).collect(),
            labels: Vec::new(),
        }
    }

    /// D-Bus variant value に変換する(signature `(sa{sv}uubbiavav)`)。
    pub fn into_variant(self) -> Value<'static> {
        Value::Structure(Structure::from((
            String::from(Self::NAME),
            empty_attachments(),
            self.page_size,
            self.cursor_pos,
            self.cursor_visible,
            self.round,
            self.orientation,
            self.candidates,
            self.labels,
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Spec §4.2 r2 acceptance: IBusAttribute の wire-format signature
    /// が IBus 1.x `(sa{sv}uuuu)` と一致する。
    #[test]
    fn ibus_attribute_signature_matches_ibus_1x_spec() {
        let attr = IBusAttribute {
            type_: 0,
            value: 0,
            start_index: 0,
            end_index: 0,
        }
        .into_variant();
        assert_eq!(attr.value_signature().to_string(), "(sa{sv}uuuu)");
    }

    /// Spec §4.2 r2 acceptance: IBusAttrList の wire-format signature
    /// が IBus 1.x `(sa{sv}av)` と一致する。
    #[test]
    fn ibus_attr_list_signature_matches_ibus_1x_spec() {
        let v = IBusAttrList::empty().into_variant();
        assert_eq!(v.value_signature().to_string(), "(sa{sv}av)");
    }

    /// Spec §4.2 r2 acceptance: IBusText の wire-format signature
    /// が IBus 1.x `(sa{sv}sv)` と一致する。
    #[test]
    fn ibus_text_signature_matches_ibus_1x_spec() {
        let v = IBusText::plain("テスト".to_string()).into_variant();
        assert_eq!(v.value_signature().to_string(), "(sa{sv}sv)");
    }

    /// Spec §4.2 r2 acceptance: IBusLookupTable の wire-format signature
    /// が IBus 1.x `(sa{sv}uubbiavav)` と一致する。
    #[test]
    fn ibus_lookup_table_signature_matches_ibus_1x_spec() {
        let v = IBusLookupTable::from_candidates(vec![]).into_variant();
        assert_eq!(v.value_signature().to_string(), "(sa{sv}uubbiavav)");
    }

    /// Spec §4.2 r2 acceptance: plain text が type-name と本文を含む
    /// Structure として serialize される。
    #[test]
    fn ibus_text_plain_serializes_with_type_name_and_text() {
        let v = IBusText::plain("こんにちは".to_string()).into_variant();
        let s = format!("{v:?}");
        assert!(s.contains("\"IBusText\""), "type-name in debug: {s}");
        assert!(s.contains("\"こんにちは\""), "text in debug: {s}");
    }

    /// Spec §4.2 r2 acceptance: 候補配列が IBusLookupTable 経由で
    /// type-name と各候補 surface を含む Structure として serialize される。
    #[test]
    fn ibus_lookup_table_with_candidates_carries_each_surface() {
        let cands = vec![
            IBusText::plain("こんにちは".to_string()),
            IBusText::plain("今日は".to_string()),
        ];
        let v = IBusLookupTable::from_candidates(cands).into_variant();
        let s = format!("{v:?}");
        assert!(s.contains("\"IBusLookupTable\""), "type-name: {s}");
        assert!(s.contains("\"こんにちは\""), "candidate 1: {s}");
        assert!(s.contains("\"今日は\""), "candidate 2: {s}");
    }

    /// Spec §4.2 r2 acceptance: 候補配列が空でも Structure として
    /// 構築可能(D-Bus serialize は試さず、structural validity のみ確認)。
    #[test]
    fn ibus_lookup_table_empty_candidates_is_constructible() {
        let v = IBusLookupTable::from_candidates(vec![]).into_variant();
        // value_signature() が成立していれば Structure は valid
        assert_eq!(v.value_signature().to_string(), "(sa{sv}uubbiavav)");
    }
}
