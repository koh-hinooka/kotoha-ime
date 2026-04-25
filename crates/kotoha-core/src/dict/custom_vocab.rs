//! `CustomVocab`: `VocabularyLookup` の TSV reader 実装。
//!
//! Spec: `docs/superpowers/specs/2026-04-25-p2-a-dictionary-layer-design.md` §5.2.

use std::collections::HashMap;
use std::ffi::OsStr;
use std::fs;
use std::io;
use std::path::Path;

use crate::dict::vocab::{VocabEntry, VocabularyLookup};
use crate::kanji::KanjiError;

/// Custom vocab TSV の最大許容サイズ(bytes)。
/// 64 MiB を超える file は CWE-400 / 資源枯渇防止のため reject する
/// (P2-A hardening item 2)。
const CUSTOM_VOCAB_MAX_BYTES: u64 = 64 * 1024 * 1024;

/// Custom vocabulary source backed by a TSV file.
///
/// Schema: `surface<TAB>reading<TAB>pos<TAB>score`。
/// 先頭 `#` の行および空行は comment として skip する。`score` は f32 として
/// 解釈し、parse 失敗時は malformed line として reject する(spec §5.2)。
///
/// # Invariants
///
/// - `entries` key は hiragana reading、value は同 reading を持つ entry 配列
/// - `vocab_id` は `"custom(<source_label>)"` 形式
#[derive(Debug, Clone)]
pub struct CustomVocab {
    entries: HashMap<String, Vec<VocabEntry>>,
    vocab_id: String,
}

impl CustomVocab {
    /// Loads a custom vocab from a TSV file on disk.
    ///
    /// # Preconditions
    ///
    /// - 拡張子は `.tsv` でなければならない(P2-A hardening item 2)
    /// - canonicalize 後のサイズは [`CUSTOM_VOCAB_MAX_BYTES`] 以下でなければならない
    ///
    /// # Errors
    ///
    /// - [`KanjiError::ModelNotFound`] when the canonicalize step fails with NotFound.
    /// - [`KanjiError::Backend`] when extension is not `.tsv`, the file exceeds
    ///   the size cap, or any other I/O / parse error occurs.
    pub fn load(path: &Path) -> Result<Self, KanjiError> {
        // ========================================
        // Path traversal hardening (P2-A item 2)
        // ========================================
        let canonical = fs::canonicalize(path).map_err(|e| match e.kind() {
            io::ErrorKind::NotFound => KanjiError::ModelNotFound {
                path: path.to_path_buf(),
            },
            _ => KanjiError::Backend {
                reason: format!(
                    "failed to canonicalize custom vocab path {}: {e:?}",
                    path.display()
                ),
            },
        })?;

        if canonical.extension().and_then(OsStr::to_str) != Some("tsv") {
            return Err(KanjiError::Backend {
                reason: format!(
                    "custom vocab must have `.tsv` extension, got {}",
                    canonical.display()
                ),
            });
        }

        let size = fs::metadata(&canonical)
            .map_err(|e| KanjiError::Backend {
                reason: format!("failed to read metadata for {}: {e:?}", canonical.display()),
            })?
            .len();
        if size > CUSTOM_VOCAB_MAX_BYTES {
            return Err(KanjiError::Backend {
                reason: format!(
                    "custom vocab at {} exceeds size cap: {size} > {CUSTOM_VOCAB_MAX_BYTES} bytes",
                    canonical.display()
                ),
            });
        }

        let content = fs::read_to_string(&canonical).map_err(|e| KanjiError::Backend {
            reason: format!("custom vocab load from {}: {e:?}", canonical.display()),
        })?;
        let label = canonical
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();
        Self::parse(&content, &label)
    }

    /// Parses a TSV string in-memory. Used directly by unit tests; production
    /// callers go through [`CustomVocab::load`] which reads the TSV from disk
    /// before delegating to `parse`.
    ///
    /// # Errors
    ///
    /// - [`KanjiError::Backend`] when any non-comment line has a field count
    ///   other than 4 or a non-parsable score.
    #[cfg(test)]
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(content: &str) -> Result<Self, KanjiError> {
        Self::parse(content, "inline")
    }

    fn parse(content: &str, source_label: &str) -> Result<Self, KanjiError> {
        let mut entries: HashMap<String, Vec<VocabEntry>> = HashMap::new();
        for (lineno, raw) in content.lines().enumerate() {
            let line = raw.trim_end_matches('\r');
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let parts: Vec<&str> = line.split('\t').collect();
            if parts.len() != 4 {
                return Err(KanjiError::Backend {
                    reason: format!(
                        "malformed TSV line {} in {:?}: expected 4 fields, got {}",
                        lineno + 1,
                        source_label,
                        parts.len()
                    ),
                });
            }
            let score: f32 = parts[3].parse().map_err(|e| KanjiError::Backend {
                reason: format!(
                    "malformed score on line {} in {:?}: {e}",
                    lineno + 1,
                    source_label
                ),
            })?;
            // NaN / +inf / -inf を弾く。`score_sort_dedupe` の sort 比較は
            // `partial_cmp` で NaN を `Ordering::Equal` に丸めるため、
            // NaN を含むと dedupe / 順序が非決定的になる。決定性確保のため
            // load 時点で reject する(spec §5.7 と整合)。
            if !score.is_finite() {
                return Err(KanjiError::Backend {
                    reason: format!(
                        "non-finite score on line {} in {:?}: {score}",
                        lineno + 1,
                        source_label
                    ),
                });
            }
            let entry = VocabEntry {
                surface: parts[0].to_string(),
                reading: parts[1].to_string(),
                pos: parts[2].to_string(),
                score,
            };
            entries
                .entry(entry.reading.clone())
                .or_default()
                .push(entry);
        }
        // 同 reading 内を score 降順に保持しておく(lookup で再 sort しないため)
        for vs in entries.values_mut() {
            vs.sort_by(|a, b| {
                b.score
                    .partial_cmp(&a.score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
        }
        Ok(Self {
            entries,
            vocab_id: format!("custom({source_label})"),
        })
    }
}

impl VocabularyLookup for CustomVocab {
    fn lookup(&self, reading: &str) -> Vec<VocabEntry> {
        self.entries.get(reading).cloned().unwrap_or_default()
    }

    fn vocab_id(&self) -> &str {
        &self.vocab_id
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EMPTY_TSV: &str = "# comment only\n# no entries\n";
    const SINGLE_ENTRY_TSV: &str = "漢字\tかんじ\t名詞\t0.85\n";
    const MULTI_ENTRY_TSV: &str = "\
# header comment
漢字\tかんじ\t名詞\t0.85
感じ\tかんじ\t動詞\t0.45
";
    // surface / reading / pos / score のいずれかが欠けた行は reject 対象。
    const MALFORMED_TSV: &str = "漢字\tかんじ\t名詞\n";

    #[test]
    fn custom_vocab_empty_tsv_yields_no_entries() {
        let vocab = CustomVocab::from_str(EMPTY_TSV).expect("empty TSV must load");
        assert!(vocab.lookup("かんじ").is_empty());
    }

    #[test]
    fn custom_vocab_single_entry_can_be_looked_up() {
        let vocab = CustomVocab::from_str(SINGLE_ENTRY_TSV).expect("single TSV must load");
        let result = vocab.lookup("かんじ");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].surface, "漢字");
        assert_eq!(result[0].pos, "名詞");
    }

    #[test]
    fn custom_vocab_skips_comment_lines() {
        let vocab = CustomVocab::from_str(MULTI_ENTRY_TSV).expect("multi TSV must load");
        let result = vocab.lookup("かんじ");
        assert_eq!(result.len(), 2, "2 non-comment entries expected");
    }

    #[test]
    fn custom_vocab_rejects_malformed_line() {
        let err = CustomVocab::from_str(MALFORMED_TSV).expect_err("malformed TSV must be rejected");
        // 受容契約: backend 層の KanjiError::Backend へ折り畳み、reason に "malformed" を含める
        match err {
            crate::kanji::KanjiError::Backend { reason } => {
                assert!(
                    reason.contains("malformed") || reason.contains("fields"),
                    "error reason should mention malformed line: {reason}"
                );
            }
            other => panic!("unexpected error variant: {other:?}"),
        }
    }

    #[test]
    fn custom_vocab_returns_empty_for_unknown_reading() {
        let vocab = CustomVocab::from_str(SINGLE_ENTRY_TSV).expect("single TSV must load");
        assert!(vocab.lookup("みず").is_empty());
    }

    #[test]
    fn custom_vocab_vocab_id_contains_source_label() {
        let vocab = CustomVocab::from_str(EMPTY_TSV).expect("empty TSV must load");
        assert!(
            vocab.vocab_id().contains("custom"),
            "vocab_id should mention 'custom': {}",
            vocab.vocab_id()
        );
    }

    #[test]
    fn custom_vocab_rejects_nan_score() {
        let nan_tsv = "漢字\tかんじ\t名詞\tnan\n";
        let err = CustomVocab::from_str(nan_tsv).expect_err("nan score must be rejected");
        match err {
            crate::kanji::KanjiError::Backend { reason } => {
                assert!(
                    reason.contains("non-finite") || reason.contains("malformed"),
                    "error reason should mention non-finite or malformed: {reason}"
                );
            }
            other => panic!("unexpected error variant: {other:?}"),
        }
    }

    #[test]
    fn custom_vocab_rejects_inf_score() {
        let inf_tsv = "漢字\tかんじ\t名詞\tinf\n";
        let err = CustomVocab::from_str(inf_tsv).expect_err("inf score must be rejected");
        assert!(matches!(err, crate::kanji::KanjiError::Backend { .. }));
    }

    // ======================================================================
    // Path traversal hardening (P2-A item 2)
    //
    // tempfile crate を導入せず std のみで unique path を組み立てる。
    // PID + nanoseconds の組み合わせで test 並列実行時の衝突を避ける。
    // ======================================================================

    fn unique_tmp_path(suffix: &str) -> std::path::PathBuf {
        use std::time::{SystemTime, UNIX_EPOCH};
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.subsec_nanos())
            .unwrap_or(0);
        std::env::temp_dir().join(format!(
            "kotoha-p2a-test-{}-{}{}",
            std::process::id(),
            nanos,
            suffix,
        ))
    }

    #[test]
    fn custom_vocab_load_rejects_non_tsv_extension() {
        let path = unique_tmp_path("-nottsv.txt");
        std::fs::write(&path, SINGLE_ENTRY_TSV).expect("write tmp file");
        let err = CustomVocab::load(&path).expect_err("non-tsv extension must reject");
        let _ = std::fs::remove_file(&path);
        match err {
            KanjiError::Backend { reason } => {
                assert!(
                    reason.contains(".tsv"),
                    "error must mention required extension: {reason}"
                );
            }
            other => panic!("expected Backend, got: {other:?}"),
        }
    }

    #[test]
    fn custom_vocab_load_rejects_oversized_file() {
        // 64 MiB cap を 1 byte 超える sparse file を ftruncate-style で作成する。
        // Linux の seek + write で疎なファイルになるため実 disk は数 KB で済む。
        let path = unique_tmp_path("-large.tsv");
        let f = std::fs::File::create(&path).expect("create tmp tsv");
        f.set_len(super::CUSTOM_VOCAB_MAX_BYTES + 1)
            .expect("set len above cap");
        drop(f);
        let err = CustomVocab::load(&path).expect_err("oversized file must reject");
        let _ = std::fs::remove_file(&path);
        match err {
            KanjiError::Backend { reason } => {
                assert!(
                    reason.contains("size cap"),
                    "error must mention size cap: {reason}"
                );
            }
            other => panic!("expected Backend, got: {other:?}"),
        }
    }

    #[test]
    fn custom_vocab_load_round_trips_real_file_and_rejects_missing() {
        // 1) 実 file を `.tsv` で配置し、canonicalize -> read_to_string -> parse の
        //    happy path が通ることを確認する(canonicalize 動作の statement
        //    coverage を兼ねる、P2-A hardening item 2)。
        let valid_path = unique_tmp_path("-canon.tsv");
        std::fs::write(&valid_path, SINGLE_ENTRY_TSV).expect("write tmp tsv");
        let vocab = CustomVocab::load(&valid_path).expect("canonicalize + load must succeed");
        let _ = std::fs::remove_file(&valid_path);
        assert_eq!(vocab.lookup("かんじ").len(), 1);

        // 2) 存在しない path に対しては canonicalize が NotFound を返し、
        //    KanjiError::ModelNotFound に折り畳まれる(item 6 整合)。
        let missing = unique_tmp_path("-missing.tsv");
        let err = CustomVocab::load(&missing).expect_err("missing file must error");
        match err {
            KanjiError::ModelNotFound { path: p } => {
                assert!(p.to_string_lossy().contains("missing.tsv"));
            }
            other => panic!("expected ModelNotFound, got: {other:?}"),
        }
    }
}
