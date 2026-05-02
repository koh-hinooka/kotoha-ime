//! Dictionary-based kanji conversion backend (Phase 2 P2-A).
//!
//! Spec: `docs/superpowers/specs/2026-04-25-p2-a-dictionary-layer-design.md`.
//!
//! この module は P2-A で追加された `KanjiBackend` の 2 つ目の実装
//! (`LlamaCppBackend` に続く)を提供する。SudachiDict-core を
//! `sudachi.rs` 経由で runtime load する Dictionary backend を中核とし、
//! `MorphologicalEngine` / `VocabularyLookup` 2 本の trait で engine 切替を
//! 抽象化する(Clean Architecture DIP、spec §4.2)。

use std::path::{Path, PathBuf};
use std::sync::Arc;

pub use self::backend::DictionaryBackend;
pub use self::engine::{EngineCandidate, MorphologicalEngine};

/// Production Send+Sync `MorphologicalEngine` を SudachiDict-core から構築する。
///
/// Phase 3-B B1(2026-05-03、ISSUE #136)で追加。kotoha-bin が `HybridRanker`
/// を構築する際の sudachi backend を生成する単一 entry point。
///
/// # Preconditions
///
/// - `system_dict_path` は SudachiDict-core の `system_core.dic` を指す
/// - 拡張子は `.dic`、size は SYSTEM_DICT_MAX_BYTES 以下(P2-A hardening 規約)
///
/// # Postconditions
///
/// - 戻り値は `Arc<dyn MorphologicalEngine + Send + Sync>`
///   (`HybridRanker::new` の sudachi backend 引数として直接利用可能)
///
/// # Errors
///
/// - [`KanjiError::ModelNotFound`] / [`KanjiError::Backend`] /
///   [`KanjiError::ModelLoadFailed`] — `SudachiAdapter::load` と同じ条件で発生する
pub fn load_morphological_engine(
    system_dict_path: &Path,
) -> Result<Arc<dyn MorphologicalEngine + Send + Sync>, crate::kanji::KanjiError> {
    let adapter = sudachi_adapter::SudachiAdapter::load(system_dict_path)?;
    Ok(Arc::new(adapter))
}
#[cfg(feature = "dict-persist")]
pub use self::user_vocab::UserVocab;
pub use self::vocab::{VocabEntry, VocabularyLookup};

/// Engine 非依存の Dictionary backend 設定(spec §3.4 Q4)。
///
/// 将来 engine を vibrato / lindera に置換しても config 互換性を保つため、
/// field 名に engine 名を含めない(`sudachi_dict_path` ではなく `system_dict_path`)。
///
/// # Invariants
///
/// - `system_dict_path` が `None` の場合、`DictionaryBackend::load` は
///   `KOTOHA_SYSTEM_DICT_PATH` 環境変数を読みに行く
/// - `custom_vocab_path` が `None` の場合、backend は custom vocab を持たない
/// - `user_vocab_db_path` が `None` の場合、backend は user vocab(SQLite)を持たない
#[derive(Debug, Clone, Default)]
pub struct DictionaryConfig {
    /// 形態素解析用 system dictionary file のパス(SudachiDict-core `system_core.dic`)。
    pub system_dict_path: Option<PathBuf>,
    /// Custom vocabulary TSV file のパス(`kotoha-dict.tsv`)。
    pub custom_vocab_path: Option<PathBuf>,
    /// SQLite User dictionary DB のパス(P2-B、spec §8.1)。
    /// `None` の場合 UserVocab を構築しない。
    pub user_vocab_db_path: Option<PathBuf>,
}

/// 明示 path、次点で env var、どちらも無ければ `None` を返す解決ヘルパ。
///
/// `DictionaryBackend::load` は本関数で `system_dict_path` を解決する。
/// `env_value` 引数は test 容易性のため `std::env::var` の結果を呼び出し側が
/// 注入する契約にする(spec §3.4 / §5.1)。
pub(crate) fn resolve_dict_path(
    explicit: Option<&Path>,
    env_value: Option<String>,
) -> Option<PathBuf> {
    if let Some(p) = explicit {
        return Some(p.to_path_buf());
    }
    env_value.map(PathBuf::from)
}

pub(crate) mod backend;
pub(crate) mod custom_vocab;
pub(crate) mod engine;
pub(crate) mod sudachi_adapter;
#[cfg(feature = "dict-persist")]
pub(crate) mod user_vocab;
pub(crate) mod vocab;

#[cfg(test)]
mod config_tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn dictionary_config_default_is_none_paths() {
        let cfg = DictionaryConfig::default();
        assert!(cfg.system_dict_path.is_none());
        assert!(cfg.custom_vocab_path.is_none());
    }

    #[test]
    fn dictionary_config_clone_preserves_fields() {
        let cfg = DictionaryConfig {
            system_dict_path: Some(PathBuf::from("/tmp/system_core.dic")),
            custom_vocab_path: Some(PathBuf::from("/tmp/kotoha-dict.tsv")),
            user_vocab_db_path: None,
        };
        let cloned = cfg.clone();
        assert_eq!(cloned.system_dict_path, cfg.system_dict_path);
        assert_eq!(cloned.custom_vocab_path, cfg.custom_vocab_path);
        assert_eq!(cloned.user_vocab_db_path, cfg.user_vocab_db_path);
    }

    #[test]
    fn dictionary_config_debug_contains_field_names() {
        let cfg = DictionaryConfig {
            system_dict_path: Some(PathBuf::from("/tmp/system_core.dic")),
            custom_vocab_path: None,
            user_vocab_db_path: None,
        };
        let msg = format!("{cfg:?}");
        assert!(
            msg.contains("system_dict_path"),
            "Debug must expose field names: {msg}"
        );
    }

    #[test]
    fn dictionary_config_default_user_vocab_db_path_is_none() {
        let cfg = DictionaryConfig::default();
        assert!(cfg.user_vocab_db_path.is_none());
    }

    #[test]
    fn dictionary_config_user_vocab_db_path_field_works() {
        let cfg = DictionaryConfig {
            system_dict_path: Some(PathBuf::from("/tmp/system.dic")),
            custom_vocab_path: None,
            user_vocab_db_path: Some(PathBuf::from("/tmp/kotoha.db")),
        };
        assert_eq!(
            cfg.user_vocab_db_path,
            Some(PathBuf::from("/tmp/kotoha.db"))
        );
    }

    #[test]
    fn resolve_dict_path_prefers_explicit_over_env() {
        let explicit = PathBuf::from("/tmp/explicit.dic");
        let resolved = resolve_dict_path(Some(&explicit), None);
        assert_eq!(resolved, Some(PathBuf::from("/tmp/explicit.dic")));
    }

    #[test]
    fn resolve_dict_path_falls_back_to_env_var() {
        let resolved = resolve_dict_path(None, Some("/tmp/env.dic".to_string()));
        assert_eq!(resolved, Some(PathBuf::from("/tmp/env.dic")));
    }

    #[test]
    fn resolve_dict_path_returns_none_when_both_absent() {
        let resolved = resolve_dict_path(None, None);
        assert!(resolved.is_none());
    }
}
