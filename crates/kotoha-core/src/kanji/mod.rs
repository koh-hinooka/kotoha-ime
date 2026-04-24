//! Kana-to-kanji conversion subsystem (Phase 1).
//!
//! This module is being populated incrementally in P1-1. Task P1-1-9 finalizes
//! the `pub use` surface. Until then, only `candidate` is declared.
//!
//! Spec: `docs/superpowers/specs/2026-04-24-kotoha-phase-1-design.md` §4-§8.

mod backend;
mod candidate;
mod error;

pub use backend::{BackendConfig, KanjiBackend};
pub use candidate::{Candidate, ConvertOptions};
pub use error::KanjiError;

#[cfg(feature = "mock-backend")]
mod mock;
#[cfg(feature = "mock-backend")]
pub use mock::MockBackend;
