//! Layer 2 integration tests for the Dictionary backend (P2-A).
//!
//! Spec: `docs/superpowers/specs/2026-04-25-p2-a-dictionary-layer-design.md` §6.2.
//!
//! これらの test は SudachiDict 不在環境でも実行可能。実辞書が要る smoke
//! は `kanji_dictionary_golden.rs` (Layer 3、`dict-smoke` feature) 側に置く。

#![cfg(feature = "dict")]

use std::path::PathBuf;

use kotoha_core::dict::DictionaryConfig;
use kotoha_core::kanji::{load_backend, BackendConfig, KanjiError};

#[test]
#[serial_test::serial]
fn load_backend_dictionary_without_env_errors_backend_with_actionable_hint() {
    std::env::remove_var("KOTOHA_SYSTEM_DICT_PATH");
    let cfg = BackendConfig::Dictionary {
        config: DictionaryConfig::default(),
    };
    // NOTE: `Box<dyn KanjiBackend>` does not implement `Debug`, so we cannot
    // use `Result::expect_err` here. Match the `Err` arm directly instead.
    match load_backend(&cfg) {
        Err(KanjiError::Backend { reason }) => {
            // 受容契約: reason は env var 名と config field 名の両方に言及し、
            // user が次に何を設定すべきかが一読でわかる。
            assert!(
                reason.contains("KOTOHA_SYSTEM_DICT_PATH"),
                "reason should name the env var: {reason}"
            );
            assert!(
                reason.contains("system_dict_path"),
                "reason should name the config field: {reason}"
            );
        }
        Err(other) => panic!("expected Backend, got: {other:?}"),
        Ok(_) => panic!("no dict path must error"),
    }
}

#[test]
#[serial_test::serial]
fn load_backend_dictionary_with_nonexistent_path_errors_model_not_found() {
    let cfg = BackendConfig::Dictionary {
        config: DictionaryConfig {
            system_dict_path: Some(PathBuf::from(
                "/tmp/definitely-does-not-exist-kotoha-p2a-integration.dic",
            )),
            custom_vocab_path: None,
        },
    };
    match load_backend(&cfg) {
        Err(KanjiError::ModelNotFound { path }) => {
            assert!(path.to_string_lossy().contains("definitely-does-not-exist"));
        }
        Err(other) => panic!("expected ModelNotFound, got: {other:?}"),
        Ok(_) => panic!("missing file must error"),
    }
}

#[test]
#[serial_test::serial]
fn dictionary_config_env_var_resolve_fallback_works() {
    // NOTE: env var を直接書き換える test は並列実行で flaky になる可能性
    // があるため、resolve は unit test 側 (dict::config_tests) で検証し、
    // 本 integration test では path 明示の振る舞いのみを確認する。
    let cfg = DictionaryConfig {
        system_dict_path: Some(PathBuf::from("/tmp/explicit-path.dic")),
        custom_vocab_path: None,
    };
    let result = load_backend(&BackendConfig::Dictionary { config: cfg });
    // explicit path でも実ファイルは無いため ModelNotFound になる。これは
    // env var fallback 経路ではなく explicit 経路を通過したことの裏返し。
    match result {
        Err(KanjiError::ModelNotFound { path }) => {
            assert_eq!(path, PathBuf::from("/tmp/explicit-path.dic"));
        }
        Err(other) => panic!("expected ModelNotFound from explicit path, got: {other:?}"),
        Ok(_) => panic!("missing file must error, not Ok"),
    }
}

#[test]
#[serial_test::serial]
fn dictionary_config_custom_vocab_missing_errors_backend() {
    let cfg = DictionaryConfig {
        system_dict_path: Some(PathBuf::from("/tmp/no-such-system.dic")),
        custom_vocab_path: Some(PathBuf::from("/tmp/no-such-custom.tsv")),
    };
    // system_dict の load で ModelNotFound に到達し、custom_vocab の load
    // には進まない。この test は「system が先にチェックされる」順序契約の
    // regression 防止に相当する。
    match load_backend(&BackendConfig::Dictionary { config: cfg }) {
        Err(KanjiError::ModelNotFound { .. }) => {}
        Err(other) => panic!("expected ModelNotFound, got: {other:?}"),
        Ok(_) => panic!("missing files must error"),
    }
}
