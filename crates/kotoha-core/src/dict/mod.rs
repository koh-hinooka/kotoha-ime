//! Dictionary-based kanji conversion backend (Phase 2 P2-A).
//!
//! Spec: `docs/superpowers/specs/2026-04-25-p2-a-dictionary-layer-design.md`.
//!
//! この module は P2-A で追加された `KanjiBackend` の 2 つ目の実装
//! (`LlamaCppBackend` に続く)を提供する。SudachiDict-core を
//! `sudachi.rs` 経由で runtime load する Dictionary backend を中核とし、
//! `MorphologicalEngine` / `VocabularyLookup` 2 本の trait で engine 切替を
//! 抽象化する(Clean Architecture DIP、spec §4.2)。

pub(crate) mod backend;
pub(crate) mod custom_vocab;
pub(crate) mod engine;
pub(crate) mod sudachi_adapter;
pub(crate) mod vocab;
