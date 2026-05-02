//! IBus 1.x D-Bus interface proxy 定義(zbus 5.x blocking API 経由)。
//!
//! IBus engine が呼び出す host 側 method は `org.freedesktop.IBus.Engine`
//! interface の subclass virtual method として送出される。実用的に Rust 側で
//! IBus engine subclass を定義することは難しいため、本 module では engine
//! object path を保持した上で zbus `Connection` に signal を emit する形を
//! 取る。signal body の `IBusText` / `IBusLookupTable` 詳細 marshalling は
//! Phase 3-A 実装段階の L3 manual smoke で empirical に詳細化する
//! (spec §13 Open Q 9)。

use zbus::blocking::Connection;
use zbus::Result;

/// IBus engine が host(`InputContext`)に発する signal の helper 群。
///
/// 本 PR (M5) では `Connection::session()` で session bus に接続し、object
/// path を保持するところまでを実装する。signal body 構築 + emit は実装段階で
/// `connection.send_signal` 越しに詳細化する(spec §13 Open Q 9)。
pub struct IBusEngineSignals {
    /// zbus blocking connection(session bus)。
    #[allow(dead_code)]
    connection: Connection,
    /// engine object path(`/org/freedesktop/IBus/Engine/Kotoha` 等)。
    #[allow(dead_code)]
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
    /// IBus 規約上 `IBusText` は `(s a(uuv) v)` 互換の D-Bus type で、
    /// attribute list と attachment dict を持つ struct。M5 段階では string-only の
    /// 簡略 marshalling とし、attribute は Phase 3-A 実装段階で詳細化する
    /// (spec §13 Open Q 9)。
    pub fn update_preedit(&self, text: &str, cursor: u32, visible: bool) -> Result<()> {
        tracing::trace!(text, cursor, visible, "IBus update_preedit (zbus)");
        Ok(())
    }

    /// `CommitText(IBusText)` signal emit。
    pub fn commit_text(&self, text: &str) -> Result<()> {
        tracing::trace!(text, "IBus commit_text (zbus)");
        Ok(())
    }

    /// `UpdateLookupTable(IBusLookupTable, bool visible)` signal emit。
    pub fn update_lookup_table(
        &self,
        candidates: &[kotoha_core::Candidate],
        visible: bool,
    ) -> Result<()> {
        tracing::trace!(
            count = candidates.len(),
            visible,
            "IBus update_lookup_table (zbus)"
        );
        Ok(())
    }

    /// `ShowLookupTable` signal emit。
    pub fn show_lookup_table(&self) -> Result<()> {
        tracing::trace!("IBus show_lookup_table (zbus)");
        Ok(())
    }

    /// `HideLookupTable` signal emit。
    pub fn hide_lookup_table(&self) -> Result<()> {
        tracing::trace!("IBus hide_lookup_table (zbus)");
        Ok(())
    }
}
