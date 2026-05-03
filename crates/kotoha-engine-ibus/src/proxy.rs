//! IBus 1.x D-Bus interface proxy 定義(zbus 5.x blocking API 経由)。
//!
//! # 現状(Phase 3-B B0f 時点)
//!
//! signal body marshalling と emit 機構は **Phase 3-B B2 / B3 で実装する**。
//! B0f までは 5 method すべて [`zbus::Error::Failure`] を返し fail-loud で
//! 動く(spec §9.3「silent_failure 禁止」を守るため)。host_bridge 側は
//! `Err` を `tracing::warn!` で観測する経路を既に持つ
//! (`crates/kotoha-engine-ibus/src/host_bridge.rs`)。
//!
//! 旧版(B0e まで)は `Ok(())` を返して TRACE log 1 行のみ残す **silent
//! no-op stub** で、production 起動成功 INFO log と組合わさって user に
//! 「IBus 通信が動いていない」を伝えない構造的 silent failure を作っていた。
//! 包括 review (2026-05-03) で Critical 1 として検出した(ISSUE #146)。
//!
//! 実 signal emit は Phase 3-B B3 で `connection.send_signal` 越しに
//! `IBusText` / `IBusLookupTable` の D-Bus marshalling を含めて詳細化する
//! (spec §13 Open Q 9)。

use zbus::blocking::Connection;
use zbus::Result;

/// 5 method すべてに共通する「未実装である」error message。
///
/// caller(`host_bridge.rs`)は本 message を tracing::warn 経由で観測する。
/// Phase 3-B B3 で実 signal emit に置き換わると本 message は廃止される。
const NOT_YET_IMPLEMENTED: &str =
    "IBus signal emit is not yet implemented; tracked in Phase 3-B B3";

/// IBus engine が host(`InputContext`)に発する signal の helper 群。
///
/// # 現状
///
/// session bus 接続と object path 保持までは本 struct 内で完結するが、
/// 各 signal emit は B0f 段階で fail-loud(`Err` を返す)。実 emit は
/// Phase 3-B B3 で実装される。
pub struct IBusEngineSignals {
    /// zbus blocking connection(session bus)。Phase 3-B B3 で
    /// `connection.send_signal(...)` 越しに使用される。
    #[allow(dead_code)]
    connection: Connection,
    /// engine object path(`/org/freedesktop/IBus/Engine/Kotoha` 等)。
    /// Phase 3-B B3 で signal の `path` field に使用される。
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
    /// # Errors
    ///
    /// 現状は常に [`zbus::Error::Failure`] を返す(Phase 3-B B3 まで未実装)。
    pub fn update_preedit(&self, text: &str, cursor: u32, visible: bool) -> Result<()> {
        tracing::warn!(
            text_len = text.chars().count(),
            cursor,
            visible,
            "IBus update_preedit not yet wired to D-Bus (Phase 3-B B3)"
        );
        Err(zbus::Error::Failure(NOT_YET_IMPLEMENTED.to_string()))
    }

    /// `CommitText(IBusText)` signal emit。
    ///
    /// # Errors
    ///
    /// 現状は常に [`zbus::Error::Failure`] を返す(Phase 3-B B3 まで未実装)。
    pub fn commit_text(&self, text: &str) -> Result<()> {
        tracing::warn!(
            text_len = text.chars().count(),
            "IBus commit_text not yet wired to D-Bus (Phase 3-B B3)"
        );
        Err(zbus::Error::Failure(NOT_YET_IMPLEMENTED.to_string()))
    }

    /// `UpdateLookupTable(IBusLookupTable, bool visible)` signal emit。
    ///
    /// # Errors
    ///
    /// 現状は常に [`zbus::Error::Failure`] を返す(Phase 3-B B3 まで未実装)。
    pub fn update_lookup_table(
        &self,
        candidates: &[kotoha_core::Candidate],
        visible: bool,
    ) -> Result<()> {
        tracing::warn!(
            count = candidates.len(),
            visible,
            "IBus update_lookup_table not yet wired to D-Bus (Phase 3-B B3)"
        );
        Err(zbus::Error::Failure(NOT_YET_IMPLEMENTED.to_string()))
    }

    /// `ShowLookupTable` signal emit。
    ///
    /// # Errors
    ///
    /// 現状は常に [`zbus::Error::Failure`] を返す(Phase 3-B B3 まで未実装)。
    pub fn show_lookup_table(&self) -> Result<()> {
        tracing::warn!("IBus show_lookup_table not yet wired to D-Bus (Phase 3-B B3)");
        Err(zbus::Error::Failure(NOT_YET_IMPLEMENTED.to_string()))
    }

    /// `HideLookupTable` signal emit。
    ///
    /// # Errors
    ///
    /// 現状は常に [`zbus::Error::Failure`] を返す(Phase 3-B B3 まで未実装)。
    pub fn hide_lookup_table(&self) -> Result<()> {
        tracing::warn!("IBus hide_lookup_table not yet wired to D-Bus (Phase 3-B B3)");
        Err(zbus::Error::Failure(NOT_YET_IMPLEMENTED.to_string()))
    }
}
