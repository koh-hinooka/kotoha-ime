//! Layer 2 integration tests for the kanji subsystem.
//!
//! Enabled by the `mock-backend` feature. Covers the five properties that
//! spec §5.3 (trait contract) and §5.7 (output guarantees) require every
//! `KanjiBackend` implementation to honor, verified through a `Box<dyn
//! KanjiBackend>` obtained from `load_backend(&BackendConfig::Mock)` rather
//! than a direct `MockBackend::new()` — this guarantees the factory path and
//! the trait object path are exercised end-to-end.
//!
//! Because `ConvertOptions` is `#[non_exhaustive]` per ADR 0006, an external
//! crate cannot use struct-literal syntax. This file therefore constructs
//! options via `ConvertOptions::default()` + field mutation, which is the
//! forward-compatible idiom for `#[non_exhaustive]` types.

#![cfg(feature = "mock-backend")]

use kotoha_core::kanji::{load_backend, BackendConfig, ConvertOptions};

#[test]
fn mock_backend_model_id_exposed_via_trait() {
    let backend = load_backend(&BackendConfig::Mock)
        .expect("Mock must construct when mock-backend feature is on");
    assert_eq!(backend.model_id(), "mock");
}

#[test]
fn mock_backend_returns_score_descending() {
    let backend = load_backend(&BackendConfig::Mock)
        .expect("Mock must construct when mock-backend feature is on");
    let mut opts = ConvertOptions::default();
    opts.top_k = 5;
    let out = backend
        .convert("にほんご", &opts)
        .expect("convert must succeed on known hiragana input");
    assert!(
        out.len() >= 2,
        "fixture should produce at least 2 candidates"
    );
    for i in 0..out.len() - 1 {
        assert!(
            out[i].score >= out[i + 1].score,
            "score at index {} ({}) must be >= score at index {} ({})",
            i,
            out[i].score,
            i + 1,
            out[i + 1].score
        );
    }
}

#[test]
fn mock_backend_respects_top_k() {
    let backend = load_backend(&BackendConfig::Mock)
        .expect("Mock must construct when mock-backend feature is on");
    let mut opts = ConvertOptions::default();
    opts.top_k = 1;
    let out = backend
        .convert("にほんご", &opts)
        .expect("convert must succeed");
    assert_eq!(
        out.len(),
        1,
        "top_k=1 must truncate output to a single candidate"
    );
}

#[test]
fn mock_backend_dedupes_identical_surface() {
    let backend = load_backend(&BackendConfig::Mock).expect("mock backend should load");
    let mut options = ConvertOptions::default();
    options.top_k = 5;

    // "にほん" fixture intentionally emits [(日本, 0.9), (日本, 0.3), (二本, 0.5)].
    // After score-desc sort + surface dedupe, the lower-scored "日本" (0.3) is
    // dropped; the output is [(日本, 0.9), (二本, 0.5)] — 2 candidates, no
    // duplicate surfaces.
    let result = backend
        .convert("にほん", &options)
        .expect("valid hiragana input should convert");

    assert_eq!(
        result.len(),
        2,
        "dedupe should collapse one duplicate surface"
    );
    assert_eq!(result[0].surface, "日本");
    assert!((result[0].score - 0.9).abs() < 1e-6);
    assert_eq!(result[1].surface, "二本");

    let mut surfaces: Vec<&str> = result.iter().map(|c| c.surface.as_str()).collect();
    surfaces.sort();
    surfaces.dedup();
    assert_eq!(
        surfaces.len(),
        result.len(),
        "no duplicate surfaces permitted in backend output (spec §5.7 bullet 2)",
    );
}

#[test]
fn mock_backend_top_k_zero_returns_empty() {
    let backend = load_backend(&BackendConfig::Mock)
        .expect("Mock must construct when mock-backend feature is on");
    let mut opts = ConvertOptions::default();
    opts.top_k = 0;
    let out = backend
        .convert("にほんご", &opts)
        .expect("top_k=0 must succeed with an empty Vec, not an error");
    assert!(
        out.is_empty(),
        "top_k=0 must yield an empty Vec; got {} candidates",
        out.len()
    );
}
