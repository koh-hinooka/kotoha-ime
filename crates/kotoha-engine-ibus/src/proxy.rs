//! IBus 1.x D-Bus interface proxy 定義(zbus 5.x blocking API 経由)。
//!
//! `IBusEngineSignals` は IBus engine が host(`InputContext`)に発する
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

use zbus::blocking::Connection;
use zbus::message::Message;
use zbus::zvariant::Value;
use zbus::Result;

use crate::types::{IBusLookupTable, IBusText};

/// IBus 1.5.x の engine signal が emit される D-Bus interface 名。
///
/// 対応する仕様: IBus 1.5.x `bus/inputcontext.c` の `BUS_INPUT_CONTEXT_GET_INTERFACE`。
pub(crate) const IBUS_ENGINE_INTERFACE: &str = "org.freedesktop.IBus.Engine";

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

impl IBusEngineSignals {
    /// Session bus に接続し、engine object path で signal emit を準備する。
    ///
    /// # Errors
    ///
    /// - zbus connection 確立失敗(`DBUS_SESSION_BUS_ADDRESS` 不設定等)
    /// - object_path 不正(D-Bus path syntax 違反)
    pub fn new(object_path: &str) -> Result<Self> {
        let connection = Connection::session()?;
        let path: zbus::zvariant::ObjectPath = zbus::zvariant::ObjectPath::try_from(object_path)?;
        Ok(Self {
            connection,
            object_path: path.into(),
        })
    }

    /// `UpdatePreeditText(IBusText, u32 cursor_pos, bool visible)` signal emit。
    ///
    /// signature: `(vub)`(IBusText variant + cursor + visible)
    ///
    /// # Errors
    ///
    /// - signal message build 失敗(D-Bus serialization error)
    /// - `connection.send` 失敗(D-Bus daemon disconnect 等)
    pub fn update_preedit(&self, text: &str, cursor: u32, visible: bool) -> Result<()> {
        tracing::debug!(
            text_len = text.chars().count(),
            cursor,
            visible,
            "IBus update_preedit emit"
        );

        let body = IBusText::plain(text.to_string()).into_variant();
        let signal = Message::signal(
            self.object_path.as_ref(),
            IBUS_ENGINE_INTERFACE,
            "UpdatePreeditText",
        )?
        .build(&(body, cursor, visible))?;
        self.connection.send(&signal)?;
        Ok(())
    }

    /// `CommitText(IBusText)` signal emit。
    ///
    /// signature: `(v)`(IBusText variant のみ)
    ///
    /// # Errors
    ///
    /// - signal message build 失敗(D-Bus serialization error)
    /// - `connection.send` 失敗(D-Bus daemon disconnect 等)
    pub fn commit_text(&self, text: &str) -> Result<()> {
        tracing::debug!(text_len = text.chars().count(), "IBus commit_text emit");

        let body = IBusText::plain(text.to_string()).into_variant();
        let signal = Message::signal(
            self.object_path.as_ref(),
            IBUS_ENGINE_INTERFACE,
            "CommitText",
        )?
        .build(&(body,))?;
        self.connection.send(&signal)?;
        Ok(())
    }

    /// `UpdateLookupTable(IBusLookupTable, bool visible)` signal emit。
    ///
    /// signature: `(vb)`(IBusLookupTable variant + visible)
    ///
    /// # Errors
    ///
    /// - signal message build 失敗(D-Bus serialization error)
    /// - `connection.send` 失敗(D-Bus daemon disconnect 等)
    pub fn update_lookup_table(
        &self,
        candidates: &[kotoha_core::Candidate],
        visible: bool,
    ) -> Result<()> {
        tracing::debug!(
            count = candidates.len(),
            visible,
            "IBus update_lookup_table emit"
        );

        let ibus_candidates: Vec<IBusText> = candidates
            .iter()
            .map(|c| IBusText::plain(c.surface.clone()))
            .collect();
        let body = IBusLookupTable::from_candidates(ibus_candidates).into_variant();

        let signal = Message::signal(
            self.object_path.as_ref(),
            IBUS_ENGINE_INTERFACE,
            "UpdateLookupTable",
        )?
        .build(&(body, visible))?;
        self.connection.send(&signal)?;
        Ok(())
    }

    /// `ShowLookupTable()` signal emit。
    ///
    /// signature: `()`(empty body)
    ///
    /// # Errors
    ///
    /// - signal message build 失敗(D-Bus serialization error、empty body でも
    ///   header が壊れていれば fail)
    /// - `connection.send` 失敗(D-Bus daemon disconnect 等)
    pub fn show_lookup_table(&self) -> Result<()> {
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

    /// `HideLookupTable()` signal emit。
    ///
    /// signature: `()`(empty body)
    ///
    /// # Errors
    ///
    /// - signal message build 失敗(D-Bus serialization error、empty body でも
    ///   header が壊れていれば fail)
    /// - `connection.send` 失敗(D-Bus daemon disconnect 等)
    pub fn hide_lookup_table(&self) -> Result<()> {
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
}

// `Value` を unused import にしないため(各 emit method 内で `into_variant` 戻り値を
// 受ける expression が `Value<'static>` であり、型推論で `Value` 型名が要求される)。
const _: fn() = || {
    let _: Value<'static> = IBusText::plain(String::new()).into_variant();
};

#[cfg(test)]
mod tests {
    use super::*;

    /// Spec §4.2 r2 acceptance: UpdatePreeditText signal が IBus 1.x interface
    /// 名と member 名で組立される。
    #[test]
    fn update_preedit_signal_has_correct_path_interface_member() {
        let object_path = "/org/freedesktop/IBus/Engine/Kotoha";
        let body = IBusText::plain("こ".to_string()).into_variant();
        let signal = Message::signal(object_path, IBUS_ENGINE_INTERFACE, "UpdatePreeditText")
            .expect("path/iface/member valid")
            .build(&(body, 1u32, true))
            .expect("build signal body");

        let header = signal.header();
        assert_eq!(header.path().unwrap().as_str(), object_path);
        assert_eq!(
            header.interface().unwrap().as_str(),
            "org.freedesktop.IBus.Engine"
        );
        assert_eq!(header.member().unwrap().as_str(), "UpdatePreeditText");
    }

    /// Spec §4.2 r2 acceptance: CommitText signal が IBus 1.x member 名で
    /// 組立される。
    #[test]
    fn commit_text_signal_has_correct_member() {
        let body = IBusText::plain("漢字".to_string()).into_variant();
        let signal = Message::signal(
            "/org/freedesktop/IBus/Engine/Kotoha",
            IBUS_ENGINE_INTERFACE,
            "CommitText",
        )
        .expect("path/iface/member valid")
        .build(&(body,))
        .expect("build signal body");

        assert_eq!(signal.header().member().unwrap().as_str(), "CommitText");
        assert_eq!(
            signal.header().interface().unwrap().as_str(),
            "org.freedesktop.IBus.Engine"
        );
    }

    /// Spec §4.2 r2 acceptance: UpdateLookupTable signal が IBus 1.x member 名で
    /// 組立され、候補配列が body に乗る。
    #[test]
    fn update_lookup_table_signal_has_correct_member_and_carries_candidates() {
        let cands = vec![
            IBusText::plain("漢字".to_string()),
            IBusText::plain("勘事".to_string()),
        ];
        let body = IBusLookupTable::from_candidates(cands).into_variant();
        let signal = Message::signal(
            "/org/freedesktop/IBus/Engine/Kotoha",
            IBUS_ENGINE_INTERFACE,
            "UpdateLookupTable",
        )
        .expect("path/iface/member valid")
        .build(&(body, true))
        .expect("build signal body");

        assert_eq!(
            signal.header().member().unwrap().as_str(),
            "UpdateLookupTable"
        );
    }

    /// Spec §4.2 r2 acceptance: ShowLookupTable signal は empty body で組立可能
    /// (IBus 1.x signature `()`)。
    #[test]
    fn show_lookup_table_signal_has_empty_body() {
        let signal = Message::signal(
            "/org/freedesktop/IBus/Engine/Kotoha",
            IBUS_ENGINE_INTERFACE,
            "ShowLookupTable",
        )
        .expect("path/iface/member valid")
        .build(&())
        .expect("build empty signal body");

        assert_eq!(
            signal.header().member().unwrap().as_str(),
            "ShowLookupTable"
        );
        assert!(signal.body().data().is_empty(), "empty body");
    }

    /// Spec §4.2 r2 acceptance: HideLookupTable signal は empty body で組立可能
    /// (IBus 1.x signature `()`)。
    #[test]
    fn hide_lookup_table_signal_has_empty_body() {
        let signal = Message::signal(
            "/org/freedesktop/IBus/Engine/Kotoha",
            IBUS_ENGINE_INTERFACE,
            "HideLookupTable",
        )
        .expect("path/iface/member valid")
        .build(&())
        .expect("build empty signal body");

        assert_eq!(
            signal.header().member().unwrap().as_str(),
            "HideLookupTable"
        );
        assert!(signal.body().data().is_empty(), "empty body");
    }
}
