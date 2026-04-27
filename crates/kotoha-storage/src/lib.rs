//! `kotoha-storage`: SQLite-based persistence layer for Kotoha (Phase 2 P2-B).
//!
//! Spec: `docs/superpowers/specs/2026-04-25-p2-b-user-dictionary-design.md`.
//! ADR: `docs/adr/0015-kotoha-storage-sqlite-adoption.md`.
//!
//! この crate は永続化詳細層であり、`kotoha-core` の domain layer に依存しない
//! (spec §4.2 Clean Architecture DIP)。`UserVocabReader` / `UserVocabWriter` /
//! `LearningCacheReader` / `LearningCacheWriter` trait は本 crate 側に置き、
//! `kotoha-core::dict::user_vocab::UserVocab` が `Box<dyn UserVocabReader>` を
//! field に保持することで DIP + ISP を成立させる(arch-M-2、P2-C-A / P2-C-B)。

pub mod database;
pub mod error;
pub mod learning_cache;
pub mod migrations;
pub mod path;
pub mod user_vocab;
pub mod validation;
