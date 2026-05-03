//! `kotoha-engine-ibus`: IBus 1.x protocol adapter for Kotoha engine.
//!
//! 本 crate は Phase 3-A spec §3.1 の adapter layer に対応する。
//! [`kotoha_engine_core::IMEHostBridge`] を [`IBusHostBridge`] で impl し、
//! IBus engine 側の D-Bus signal/method を `kotoha_engine_core::IMEEngine`
//! method 呼び出しに変換する dispatcher を提供する。
//!
//! # Module 構成
//!
//! - [`host_bridge::IBusHostBridge`] — `IMEHostBridge` の IBus 実装(driven port)
//! - [`lookup_table::LookupTable`] — `Mutex<Vec<Candidate>>` 内部 buffer + IBus mapping
//! - `dispatcher::IBusEventDispatcher` — D-Bus event 受信 + `IMEEngine` 呼び出し(M5 で追加)
//! - `keysym` — IBus keysym → `KeyEvent` 変換 helper(M5 で追加)
//!
//! # Boundary 原則
//!
//! 本 crate は IBus 固有の D-Bus interface に依存して良いが、`kotoha-engine-core`
//! は host 非依存のため、core 側に IBus 識別子を漏らさない(spec §3.1 / Adaptive
//! boundary-first 原則)。

pub mod dispatcher;
pub mod host_bridge;
pub mod keysym;
pub mod lookup_table;
pub(crate) mod proxy;
pub(crate) mod types;

/// `crates/kotoha-engine-ibus/src/types.rs` の wire-format type を doc test
/// から参照するための test-only export。production binary には影響しない。
#[doc(hidden)]
pub mod types_test_export {
    pub use crate::types::{IBusAttrList, IBusAttribute, IBusLookupTable, IBusText};
}

pub use dispatcher::IBusEventDispatcher;
pub use host_bridge::IBusHostBridge;
pub use lookup_table::LookupTable;
