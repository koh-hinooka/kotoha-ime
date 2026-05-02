//! Phase 1 14/15 regression test through `HybridRanker`.
//!
//! Phase 3-A spec §10.1 Regression / §10.4 で要求される Phase 1 baseline 維持
//! 確認。本 test は、LLM 統合済み HybridRanker を経由しても Phase 1 で達成済の
//! 14/15 fixture PASS が維持されることを確認するための **frame** を提供する。
//!
//! # Feature gate
//!
//! - `llama-cpp-smoke` feature **有効** 時:`mod smoke` を compile し、
//!   `LlamaCppBackend` + Phase 1 既存 fixture(`crates/kotoha-core/tests/
//!   kanji_llama_cpp_smoke.rs` の 14/15 セット)を Ranker 経由で実行する
//!   smoke test を提供する。本 test の **本実装 placeholder** は P2-D 後続
//!   ISSUE で完成させる(Plan 2026-05-02-feature-120-p2d-hybrid-ranker.md
//!   §M3 Adaptation 4)。M3 段階では smoke variant は **structure-only**
//!   (実 fixture loader を持たない)に留め、`llama-cpp-smoke` を有効にした
//!   開発環境で compile できることのみ保証する。
//!
//! - `llama-cpp-smoke` feature **無効** 時(default):lefthook pre-push が
//!   この場合に該当する。compile-only sanity test 1 件を提供し、本ファイル
//!   全体が default features で compile error を起こさないことを観測する。

#[cfg(feature = "llama-cpp-smoke")]
mod smoke {
    //! Live LLM smoke variant — `llama-cpp-smoke` feature 必須。
    //!
    //! 本 module は M3 で **structure-only**。実 fixture(Phase 1 14/15)を
    //! `HybridRanker` 経由で run する完全 impl は Phase 3-A 本番 / P2-D 後続
    //! ISSUE で追加する。M3 段階では下記項目のみ保証する:
    //!
    //! 1. `llama-cpp-smoke` feature 有効時に compile が通ること
    //! 2. `LlamaCppBackend` を Arc<dyn KanjiBackend + Send + Sync> として
    //!    HybridRanker::with_llm() に渡せる型が成立すること(現実の load 呼び出しは
    //!    GGUF model file path を要するため smoke test 本体では行わない)

    use std::sync::Arc;

    use kotoha_core::kanji::{KanjiBackend, LlamaCppBackend};

    /// `LlamaCppBackend` が `KanjiBackend + Send + Sync` の trait object として
    /// `HybridRanker::with_llm` に渡せる型であることを compile time で確認する。
    /// 実 model load + inference は GGUF file が必要なので本 test では行わず、
    /// Phase 3-A 本番(別 ISSUE)で fixture-driven smoke を追加する。
    #[allow(dead_code)]
    fn _llama_cpp_backend_is_compatible_with_hybrid_ranker_llm_slot(
        backend: LlamaCppBackend,
    ) -> Arc<dyn KanjiBackend + Send + Sync> {
        Arc::new(backend)
    }
}

/// `llama-cpp-smoke` 無効時の compile-only sanity。本 test は assertion を
/// 持たず、ファイル全体が default features で compile することのみ観測する。
#[test]
#[cfg(not(feature = "llama-cpp-smoke"))]
fn regression_test_compiles_without_llama_smoke_feature() {
    // Intentional empty body. The presence of this test under default features
    // ensures the file is exercised by `cargo test` without requiring the
    // optional `llama-cpp-smoke` toolchain (llama.cpp + GGUF model).
}
