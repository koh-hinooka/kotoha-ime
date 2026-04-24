//! Layer 3 LlamaCpp smoke integration tests.
//!
//! Runs the real `LlamaCppBackend` (via llama-cpp-2) against a short fixture
//! of hiragana inputs and asserts that each top-1 candidate contains the
//! expected substring.
//!
//! # Enabling
//!
//! - Build-time: enable the `llama-cpp-smoke` feature (which implies
//!   `llama-cpp`).
//! - Runtime: set `KOTOHA_ZENZ_MODEL_PATH` to the absolute path of a
//!   GGUF model file (download via HuggingFace — see below). The env var
//!   name keeps its legacy `ZENZ` prefix for this commit; Task P1-2.5-13
//!   renames it.
//!
//! If the environment variable is unset, each test prints
//! `SKIPPED: KOTOHA_ZENZ_MODEL_PATH not set` and exits early, so the test
//! suite succeeds even when the model is missing. This follows spec §8.3
//! ("lefthook pre-push には含めない、opt-in で実行") without relying on
//! `#[ignore]`.
//!
//! # Model setup
//!
//! The default Phase 1 model is Zenz-v2.5-medium, hosted at
//! <https://huggingface.co/Miwa-Keita/zenz-v2.5-medium-gguf>. Download with:
//!
//! ```text
//! huggingface-cli download Miwa-Keita/zenz-v2.5-medium-gguf \
//!     --local-dir $HOME/.cache/kotoha/models
//! export KOTOHA_ZENZ_MODEL_PATH=$HOME/.cache/kotoha/models/<gguf-filename>
//! cargo test -p kotoha-core --features llama-cpp-smoke --test kanji_llama_cpp_smoke
//! ```
//!
//! Spec: `docs/superpowers/specs/2026-04-24-kotoha-phase-1-design.md` §3.2,
//! §8.3, §8.6.

#![cfg(feature = "llama-cpp-smoke")]

use std::path::PathBuf;

use kotoha_core::kanji::{load_backend, BackendConfig, ConvertOptions, PromptTemplate};

/// Returns the Zenz model path from `KOTOHA_ZENZ_MODEL_PATH`, or `None` with
/// a SKIP message on stdout if the env var is unset.
fn get_model_path_or_skip() -> Option<PathBuf> {
    match std::env::var("KOTOHA_ZENZ_MODEL_PATH") {
        Ok(p) => Some(PathBuf::from(p)),
        Err(_) => {
            println!("SKIPPED: KOTOHA_ZENZ_MODEL_PATH not set");
            None
        }
    }
}

/// Reads `tests/fixtures/kanji_smoke.tsv` and returns `(input, expected_substring)` pairs.
fn load_fixture() -> Vec<(String, String)> {
    let tsv = include_str!("fixtures/kanji_smoke.tsv");
    tsv.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            let mut parts = line.splitn(2, '\t');
            let input = parts
                .next()
                .expect("fixture line missing input")
                .to_string();
            let expected = parts
                .next()
                .expect("fixture line missing expected_substring")
                .to_string();
            (input, expected)
        })
        .collect()
}

/// Asserts a single fixture row against the LlamaCpp backend loaded from `model_path`.
///
/// `ConvertOptions` is `#[non_exhaustive]` (ADR 0006), so struct-literal
/// construction from outside the defining crate is forbidden. We therefore
/// build options via `Default` + field mutation, which triggers clippy's
/// `field_reassign_with_default` lint. We allow it with this rationale to
/// match the established pattern in `kanji_mock.rs`.
#[allow(clippy::field_reassign_with_default)]
fn run_fixture(model_path: PathBuf, row_index: usize) {
    let fixtures = load_fixture();
    assert!(
        row_index < fixtures.len(),
        "fixture index {row_index} is out of range (fixture has {} rows)",
        fixtures.len()
    );
    let (input, expected_substring) = &fixtures[row_index];

    let backend = load_backend(&BackendConfig::LlamaCpp {
        model_path: model_path.clone(),
        prompt_template: PromptTemplate::Gemma2InstructChat,
    })
    .expect("LlamaCpp backend must load from the pinned fixture path");

    let mut options = ConvertOptions::default();
    options.top_k = 5;
    // temperature / seed stay at defaults (0.0 / Some(0)) for deterministic output.

    let result = backend
        .convert(input, &options)
        .unwrap_or_else(|e| panic!("convert must succeed for input {input:?}: {e}"));

    assert!(
        !result.is_empty(),
        "convert must return at least one candidate for input {input:?}"
    );
    assert!(
        result[0].surface.contains(expected_substring.as_str()),
        "top-1 candidate {:?} must contain expected substring {:?} for input {:?}",
        result[0].surface,
        expected_substring,
        input
    );
}

#[test]
fn zenz_smoke_1_nihongo() {
    let Some(path) = get_model_path_or_skip() else {
        return;
    };
    run_fixture(path, 0);
}

#[test]
fn zenz_smoke_2_kanji() {
    let Some(path) = get_model_path_or_skip() else {
        return;
    };
    run_fixture(path, 1);
}

#[test]
fn zenz_smoke_3_ashita() {
    let Some(path) = get_model_path_or_skip() else {
        return;
    };
    run_fixture(path, 2);
}

#[test]
fn zenz_smoke_4_yamada_san() {
    let Some(path) = get_model_path_or_skip() else {
        return;
    };
    run_fixture(path, 3);
}

#[test]
fn zenz_smoke_5_kotoba() {
    let Some(path) = get_model_path_or_skip() else {
        return;
    };
    run_fixture(path, 4);
}
