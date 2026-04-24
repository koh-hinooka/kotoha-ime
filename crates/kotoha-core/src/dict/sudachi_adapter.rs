//! `SudachiAdapter`: `MorphologicalEngine` の sudachi.rs 実装。
//!
//! Spec: `docs/superpowers/specs/2026-04-25-p2-a-dictionary-layer-design.md` §3.4, §4.1, §5.1.

use std::path::{Path, PathBuf};

use sudachi::analysis::stateful_tokenizer::StatefulTokenizer;
use sudachi::analysis::Mode;
use sudachi::config::Config;
use sudachi::dic::dictionary::JapaneseDictionary;

use crate::dict::engine::{EngineCandidate, MorphologicalEngine};
use crate::kanji::KanjiError;

/// sudachi.rs の engine_id() で返す固定 label。Layer 3 golden test から
/// 同一文字列でアサートするため、module スコープの `pub(crate)` const として
/// 固定する。
pub(crate) const SUDACHI_ENGINE_ID_LABEL: &str = "sudachi-0.6.11+sudachidict-core:v20260116";

/// sudachi.rs runtime ラッパー。`MorphologicalEngine` 実装。
///
/// SudachiDict-core (`system_core.dic`) を runtime load し、Mode::C で
/// tokenize する。score は `head_word_length` の負値を proxy として用いる
/// (Task 14 で診断 → 必要なら切替、spec §3.4 注釈)。
///
/// # Invariants
///
/// - `dict` は `load` 成功後に valid な SudachiDict instance を保持する
/// - `system_dict_path` は load 時の path を保持し、Backend variant で error
///   message に埋め込む用途で使用する
#[allow(dead_code)] // Consumed by `DictionaryBackend` in Task 8 (P2-A).
pub(crate) struct SudachiAdapter {
    dict: JapaneseDictionary,
    system_dict_path: PathBuf,
}

impl std::fmt::Debug for SudachiAdapter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // `JapaneseDictionary` does not implement `Debug`, so format only the
        // load-time path. Sufficient for test failure messages.
        f.debug_struct("SudachiAdapter")
            .field("system_dict_path", &self.system_dict_path)
            .finish_non_exhaustive()
    }
}

impl SudachiAdapter {
    /// SudachiDict-core file を disk から load する。
    ///
    /// # Preconditions
    ///
    /// - `system_dict_path` は SudachiDict-core の `system_core.dic` を指す
    ///
    /// # Errors
    ///
    /// - [`KanjiError::ModelNotFound`] — path に file が存在しない場合
    /// - [`KanjiError::ModelLoadFailed`] — sudachi.rs 側で config / dict の
    ///   読み込みに失敗した場合
    #[allow(dead_code)] // Consumed by `DictionaryBackend` in Task 8 (P2-A).
    pub(crate) fn load(system_dict_path: &Path) -> Result<Self, KanjiError> {
        if !system_dict_path.exists() {
            return Err(KanjiError::ModelNotFound {
                path: system_dict_path.to_path_buf(),
            });
        }
        let config =
            Config::new(None, None, Some(system_dict_path.to_path_buf())).map_err(|e| {
                KanjiError::ModelLoadFailed {
                    source: Box::new(e),
                }
            })?;
        let dict =
            JapaneseDictionary::from_cfg(&config).map_err(|e| KanjiError::ModelLoadFailed {
                source: Box::new(e),
            })?;
        Ok(Self {
            dict,
            system_dict_path: system_dict_path.to_path_buf(),
        })
    }
}

impl MorphologicalEngine for SudachiAdapter {
    fn tokenize(&self, reading: &str) -> Result<Vec<EngineCandidate>, KanjiError> {
        if reading.is_empty() {
            return Ok(Vec::new());
        }
        let mut tokenizer: StatefulTokenizer<&JapaneseDictionary> =
            StatefulTokenizer::create(&self.dict, false, Mode::C);
        tokenizer.reset().push_str(reading);
        tokenizer.do_tokenize().map_err(|e| KanjiError::Backend {
            reason: format!(
                "sudachi tokenize failed for input {:?} using dict {}: {e}",
                reading,
                self.system_dict_path.display()
            ),
        })?;
        let morphemes = tokenizer
            .into_morpheme_list()
            .map_err(|e| KanjiError::Backend {
                reason: format!("sudachi morpheme collection failed: {e}"),
            })?;
        let mut out = Vec::with_capacity(morphemes.len());
        for m in morphemes.iter() {
            let surface = m.surface().to_string();
            let reading_form = m.reading_form().to_string();
            let cost = m.get_word_info().head_word_length() as f32;
            out.push(EngineCandidate {
                surface,
                reading: reading_form,
                score: -cost,
            });
        }
        Ok(out)
    }

    fn engine_id(&self) -> &str {
        SUDACHI_ENGINE_ID_LABEL
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kanji::KanjiError;
    use std::path::Path;

    #[test]
    fn sudachi_adapter_load_missing_file_errors() {
        let err = SudachiAdapter::load(Path::new("/tmp/definitely-does-not-exist-kotoha-p2a.dic"))
            .expect_err("load must error when the path does not exist");
        match err {
            KanjiError::ModelNotFound { path } => {
                assert!(path.to_string_lossy().contains("definitely-does-not-exist"));
            }
            other => panic!("expected ModelNotFound, got: {other:?}"),
        }
    }

    #[test]
    fn sudachi_engine_id_label_constant_is_stable() {
        assert!(SUDACHI_ENGINE_ID_LABEL.contains("sudachi"));
        assert!(SUDACHI_ENGINE_ID_LABEL.contains("0.6"));
    }
}
