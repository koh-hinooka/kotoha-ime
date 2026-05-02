//! `IBusHostBridge` — `IMEHostBridge` の IBus 1.x 実装。
//!
//! Phase 3-A spec §4.2 / §3.3 全体図に対応する driven port adapter。
//! 本 PR (M4) では D-Bus call は `tracing::trace!` で stub し、
//! `Mutex<Vec<Candidate>>` 内部 buffer の coalesce logic を確定させる。
//! M5 で zbus `Proxy` 経由で IBus engine interface に結線する。

use kotoha_engine_core::{CandidateUpdate, IMEHostBridge};

use crate::lookup_table::LookupTable;

/// IBus 1.x host(`org.freedesktop.IBus.Engine` interface)への呼び出し adapter。
///
/// # Construction
///
/// M4 段階では `LookupTable` + `tracing::trace!` stub のみ。
/// M5 で zbus `Connection` / object path を field 追加する。
///
/// # Thread safety
///
/// `Send + Sync`(spec §4.2)。`LookupTable` 内部 `Mutex` で coalesce、
/// `tracing` macro は thread-safe。
#[derive(Debug, Default)]
pub struct IBusHostBridge {
    lookup_table: LookupTable,
}

impl IBusHostBridge {
    pub fn new() -> Self {
        Self {
            lookup_table: LookupTable::new(),
        }
    }
}

impl IMEHostBridge for IBusHostBridge {
    fn update_preedit(&self, text: &str, cursor: usize, visible: bool) {
        // M5 で zbus.Proxy::call("UpdatePreeditText", ...) に置換。
        tracing::trace!(text, cursor, visible, "IBus update_preedit (stub)");
    }

    fn commit_text(&self, text: &str) {
        tracing::trace!(text, "IBus commit_text (stub)");
    }

    fn update_candidates(&self, update: CandidateUpdate) {
        let merged = self.lookup_table.apply(update);
        tracing::trace!(
            count = merged.len(),
            "IBus update_lookup_table (stub, coalesced)"
        );
    }

    fn show_candidate_window(&self) {
        tracing::trace!("IBus show_lookup_table (stub)");
    }

    fn hide_candidate_window(&self) {
        tracing::trace!("IBus hide_lookup_table (stub)");
    }
}
