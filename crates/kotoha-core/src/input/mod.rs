//! Input mode management module.
//!
//! Public API:
//! - [`InputMode`] — user-visible input mode
//! - `InputContext` — state machine (added by M4b-4)
//! - `InputStep` — per-char outcome enum (added by M4b-4)
//!
//! The crate-private [`mode::ModeOrigin`] is used by `context::InputContext`
//! to distinguish Sticky vs Transient origins per ADR 0002.

pub mod context;
pub mod mode;

pub use context::{InputContext, InputStep};
pub use mode::InputMode;
