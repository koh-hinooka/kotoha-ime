//! `IBusHostBridge` — `IMEHostBridge` の IBus 1.x 実装(zbus binding 版)。
//!
//! Phase 3-A spec §4.2 / §3.3 全体図に対応する driven port adapter。
//! M5 で zbus `Connection` 経由で IBus engine interface に接続する。
//! signal body 詳細は spec §13 Open Q 9 通り Phase 3-A 実装段階で
//! empirical に詰める。

use kotoha_engine_core::{CandidateUpdate, IMEHostBridge};

use crate::lookup_table::LookupTable;
use crate::proxy::IBusEngineSignals;

/// IBus 1.x host(`org.freedesktop.IBus.Engine` interface)への呼び出し adapter。
///
/// # Construction
///
/// [`Self::new`] で session bus 接続 + engine object path を保持する。
///
/// # Thread safety
///
/// `Send + Sync`(spec §4.2)。`LookupTable` 内部 `Mutex` で coalesce、
/// zbus `Connection` は内部で `Arc` 共有のため複数 thread から呼び出し可能。
pub struct IBusHostBridge {
    lookup_table: LookupTable,
    signals: IBusEngineSignals,
}

impl IBusHostBridge {
    /// session bus + engine object path で adapter を構築する。
    ///
    /// # Errors
    ///
    /// - zbus connection 確立失敗(`DBUS_SESSION_BUS_ADDRESS` 不設定等)
    /// - object_path 不正(D-Bus path syntax 違反)
    pub fn new(object_path: &str) -> zbus::Result<Self> {
        Ok(Self {
            lookup_table: LookupTable::new(),
            signals: IBusEngineSignals::new(object_path)?,
        })
    }
}

impl IMEHostBridge for IBusHostBridge {
    fn update_preedit(&self, text: &str, cursor: usize, visible: bool) {
        if let Err(e) = self.signals.update_preedit(text, cursor as u32, visible) {
            tracing::warn!(error = %e, "IBus update_preedit failed");
        }
    }

    fn commit_text(&self, text: &str) {
        if let Err(e) = self.signals.commit_text(text) {
            tracing::warn!(error = %e, "IBus commit_text failed");
        }
    }

    /// IBus 1.x では候補 window の表示・非表示は `update_lookup_table` の
    /// `visible` 引数で制御する設計のため、`hide_lookup_table` を別途呼ばない。
    /// 内部 buffer が空になったときも `visible = false` を載せた
    /// `update_lookup_table` で 1 回の D-Bus signal に集約する。`Show/HideLookupTable`
    /// は engine の state transition(`hide_candidate_window` 等)が独立に呼ぶ。
    /// 将来 refactor で `visible` 制御を分離する場合は `LookupTable` の coalesce
    /// 設計と合わせて見直すこと。
    fn update_candidates(&self, update: CandidateUpdate) {
        let merged = self.lookup_table.apply(update);
        let visible = !merged.is_empty();
        if let Err(e) = self.signals.update_lookup_table(&merged, visible) {
            tracing::warn!(error = %e, "IBus update_lookup_table failed");
        }
    }

    fn show_candidate_window(&self) {
        if let Err(e) = self.signals.show_lookup_table() {
            tracing::warn!(error = %e, "IBus show_lookup_table failed");
        }
    }

    fn hide_candidate_window(&self) {
        if let Err(e) = self.signals.hide_lookup_table() {
            tracing::warn!(error = %e, "IBus hide_lookup_table failed");
        }
    }
}
