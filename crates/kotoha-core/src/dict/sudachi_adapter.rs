//! `SudachiAdapter`: `MorphologicalEngine` の sudachi.rs 実装。
//!
//! Spec: `docs/superpowers/specs/2026-04-25-p2-a-dictionary-layer-design.md` §3.4, §4.1, §5.1.

use std::ffi::OsStr;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use sudachi::analysis::stateful_tokenizer::StatefulTokenizer;
use sudachi::analysis::Mode;
use sudachi::config::Config;
use sudachi::dic::dictionary::JapaneseDictionary;

use crate::dict::engine::{EngineCandidate, MorphologicalEngine};
use crate::kanji::KanjiError;

/// SudachiDict-core を runtime load する際の最大許容サイズ(bytes)。
/// 200 MiB は v20260116 release の実サイズ(約 70 MB)に対し将来の dictionary 拡張を
/// 見越した上限。これを超える file が指定された場合は CWE-400 / 資源枯渇防止のため
/// load を reject する(P2-A hardening item 2)。
const SYSTEM_DICT_MAX_BYTES: u64 = 200 * 1024 * 1024;

/// sudachi.rs の engine_id() で返す固定 label。Layer 3 golden test から
/// 同一文字列でアサートするため、module スコープの `pub(crate)` const として
/// 固定する。
///
/// # 同期義務
///
/// `Cargo.toml` `[workspace.dependencies]` の `sudachi` git rev を更新したら、
/// 以下を **必ず** 同期せよ(P2-A hardening item 7):
/// - `sudachi-X.Y.Z` 部分を新 sudachi.rs release tag に揃える
/// - `sudachidict-core:vYYYYMMDD` 部分を `README.md` 配置手順で参照している
///   SudachiDict-core release tag に揃える
///
/// build-time 自動化は YAGNI として見送り、PR review チェックリストで担保する。
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
    /// - 拡張子は `.dic` でなければならない(P2-A hardening item 2、extension allowlist)
    /// - canonicalize 後のサイズは [`SYSTEM_DICT_MAX_BYTES`] 以下でなければならない
    ///
    /// # Postconditions
    ///
    /// - sudachi.rs の config search は default location に fallback しない。
    ///   `Config::new_embedded()` で sudachi 同梱の baseline config を読み込み、
    ///   `with_system_dic()` で system dictionary だけを caller 提供 path で
    ///   override する(P2-A hardening item 1、CWE-426 対策)。
    ///
    /// # Errors
    ///
    /// - [`KanjiError::ModelNotFound`] — path に file が存在しない場合 / canonicalize 失敗時
    /// - [`KanjiError::Backend`] — 拡張子が `.dic` でない / size cap 超過 / I/O error
    /// - [`KanjiError::ModelLoadFailed`] — sudachi.rs 側で config / dict の
    ///   読み込みに失敗した場合(NotFound 以外の I/O error も含む)
    pub(crate) fn load(system_dict_path: &Path) -> Result<Self, KanjiError> {
        // ========================================
        // Path traversal hardening (P2-A item 2): symlink resolve + extension allowlist + size cap
        // ========================================
        // canonicalize は symlink を解決し、`..` 等を正規化する。NotFound 系の
        // I/O error は ModelNotFound に折り畳み(item 6 と整合: TOCTOU を避けるため
        // exists() の早期 check は廃する)、それ以外は Backend として通知する。
        let canonical = fs::canonicalize(system_dict_path).map_err(|e| match e.kind() {
            io::ErrorKind::NotFound => KanjiError::ModelNotFound {
                path: system_dict_path.to_path_buf(),
            },
            _ => KanjiError::Backend {
                reason: format!(
                    "failed to canonicalize system dict path {}: {e:?}",
                    system_dict_path.display()
                ),
            },
        })?;

        // Extension allowlist: SudachiDict は `.dic` 形式のみを受理する。
        if canonical.extension().and_then(OsStr::to_str) != Some("dic") {
            return Err(KanjiError::Backend {
                reason: format!(
                    "system dict must have `.dic` extension, got {}",
                    canonical.display()
                ),
            });
        }

        // Size cap: 異常に大きな file の load を防ぐ(CWE-400 / 資源枯渇)。
        let size = fs::metadata(&canonical)
            .map_err(|e| KanjiError::Backend {
                reason: format!("failed to read metadata for {}: {e:?}", canonical.display()),
            })?
            .len();
        if size > SYSTEM_DICT_MAX_BYTES {
            return Err(KanjiError::Backend {
                reason: format!(
                    "system dict at {} exceeds size cap: {size} > {SYSTEM_DICT_MAX_BYTES} bytes",
                    canonical.display()
                ),
            });
        }

        // ========================================
        // Config 構築 (P2-A item 1, CWE-426): search path を回避
        // ========================================
        // `Config::new_embedded()` は sudachi.rs に同梱された baseline config を
        // bytes から読む。`Config::new(None, ..)` のように on-disk default config を
        // search する経路を通らないため、`SUDACHI_CONFIG_PATH` 等の外部入力で
        // 任意 config を pin できない。system dictionary のみを caller の正規化済み
        // path で override する。
        let config = Config::new_embedded()
            .map(|c| c.with_system_dic(canonical.clone()))
            .map_err(|e| KanjiError::ModelLoadFailed {
                source: Box::new(e),
            })?;
        let dict = JapaneseDictionary::from_cfg(&config).map_err(|e| {
            // sudachi.rs 内部で I/O NotFound が起きるケース(canonicalize と
            // from_cfg の間で file を消されたなど TOCTOU race)は ModelNotFound に
            // 折り畳む。それ以外は ModelLoadFailed のまま伝播する。
            if io_error_is_not_found(&e) {
                KanjiError::ModelNotFound {
                    path: canonical.clone(),
                }
            } else {
                KanjiError::ModelLoadFailed {
                    source: Box::new(e),
                }
            }
        })?;
        Ok(Self {
            dict,
            system_dict_path: canonical,
        })
    }
}

/// `SudachiError` 連鎖に `io::Error` の NotFound が含まれるかを判定する。
///
/// `SudachiError::Io { cause, .. }` か、`source()` chain を辿って `io::Error::kind()`
/// が `NotFound` の場合に `true` を返す。`from_cfg` が file 欠損を I/O error として
/// 投げるケースを ModelNotFound に折り畳むために用いる(P2-A hardening item 6)。
fn io_error_is_not_found(err: &sudachi::error::SudachiError) -> bool {
    let mut current: &dyn std::error::Error = err;
    loop {
        if let Some(io_err) = current.downcast_ref::<io::Error>() {
            return io_err.kind() == io::ErrorKind::NotFound;
        }
        match current.source() {
            Some(next) => current = next,
            None => return false,
        }
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
            // `head_word_length` は morpheme 表記の文字数で、SudachiDict 仕様上
            // 実用範囲は 100 を大きく超えない(u16 / u32 のいずれの返り値型でも
            // f32 の有効精度 24 bit を逸脱しない)。EngineCandidate::score は
            // 順序付け proxy として用いる相対値であり、絶対値の精度は不要。
            // f64 への migrate は score の broad な API 影響を伴うため見送る
            // (P2-A hardening item 5)。
            #[allow(clippy::cast_precision_loss)]
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

    // NOTE: Extension allowlist の動作 verification は default features で動く
    // `custom_vocab::tests::custom_vocab_load_rejects_non_tsv_extension` 側で
    // 検証する。両 adapter で同一 logic を使うため重複 test を避ける
    // (P2-A hardening item 2)。

    #[test]
    fn sudachi_engine_id_label_constant_is_stable() {
        assert!(SUDACHI_ENGINE_ID_LABEL.contains("sudachi"));
        assert!(SUDACHI_ENGINE_ID_LABEL.contains("0.6"));
    }
}
