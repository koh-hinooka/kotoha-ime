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
//! - [`listener::run`] — D-Bus method call → `Event::IBusKey` / `Event::IBusReset`
//!   decode + bridge channel forward(B0h-f + B3 / ADR 0020、driving listener thread)
//! - `keysym` — IBus keysym → `KeyEvent` 変換 helper(M5 で追加)
//! - `discovery` / `factory` — IBus private bus address discovery と
//!   `org.freedesktop.IBus.Factory` service(#208 / ADR 0021 Amendment)
//!
//! # Boundary 原則
//!
//! 本 crate は IBus 固有の D-Bus interface に依存して良いが、`kotoha-engine-core`
//! は host 非依存のため、core 側に IBus 識別子を漏らさない(spec §3.1 / Adaptive
//! boundary-first 原則)。

// Phase 3-B B0h-f rev3 (ADR 0020) review fix:旧 `dispatcher::IBusEventDispatcher`
// (`Arc<Mutex<dyn IMEEngine>>` ベース)は本 PR で完全削除された。listener.rs が
// keysym decode 経路を継承する。
pub(crate) mod discovery;
pub(crate) mod factory;
pub mod host_bridge;
pub mod keysym;
pub mod listener;
pub mod lookup_table;
pub(crate) mod proxy;
pub(crate) mod service;
pub(crate) mod types;

/// `crates/kotoha-engine-ibus/src/types.rs` の wire-format type を doc test
/// から参照するための test-only export。production binary には影響しない。
#[doc(hidden)]
pub mod types_test_export {
    pub use crate::types::{IBusAttrList, IBusAttribute, IBusLookupTable, IBusText};
}

pub use host_bridge::IBusHostBridge;
pub use lookup_table::LookupTable;
