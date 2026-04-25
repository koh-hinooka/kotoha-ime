//! `StorageError` enum: kotoha-storage 全層の error 型(spec §9.4)。

use std::path::PathBuf;

/// 永続化層の error。
///
/// # Variants
///
/// - [`StorageError::InvalidField`]: validation 違反(spec §9.5 reason 列挙)
/// - [`StorageError::InvalidPath`]: path resolution / blacklist 違反(spec §5.4)
/// - [`StorageError::DuplicateEntry`]: UNIQUE(surface, reading) 違反
/// - [`StorageError::NotFound`]: delete / show 対象 entry が不在
/// - [`StorageError::Sqlite`]: rusqlite backend 失敗
/// - [`StorageError::Io`]: filesystem IO 失敗
/// - [`StorageError::Migration`]: migration apply 失敗
/// - [`StorageError::HomeDirNotFound`]: $HOME / $XDG_DATA_HOME 解決不能
#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("invalid field {name}: {reason}")]
    InvalidField { name: String, reason: String },
    #[error("invalid path {path:?}: {reason}")]
    InvalidPath { path: PathBuf, reason: String },
    #[error("duplicate entry: surface={surface} reading={reading}")]
    DuplicateEntry { surface: String, reading: String },
    #[error("entry not found")]
    NotFound,
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("migration error: {0}")]
    Migration(String),
    #[error("home directory not found: $HOME / $XDG_DATA_HOME / KOTOHA_DATA_DIR all unset")]
    HomeDirNotFound,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_field_display_includes_name_and_reason() {
        let err = StorageError::InvalidField {
            name: "reading".to_string(),
            reason: "non-hiragana reading".to_string(),
        };
        let msg = format!("{err}");
        assert!(msg.contains("reading"));
        assert!(msg.contains("non-hiragana reading"));
    }

    #[test]
    fn invalid_path_display_includes_path_and_reason() {
        let err = StorageError::InvalidPath {
            path: PathBuf::from("/etc/kotoha"),
            reason: "system path blacklisted".to_string(),
        };
        let msg = format!("{err}");
        assert!(msg.contains("/etc/kotoha"));
        assert!(msg.contains("blacklisted"));
    }

    #[test]
    fn duplicate_entry_display_includes_surface_reading() {
        let err = StorageError::DuplicateEntry {
            surface: "日野岡".to_string(),
            reading: "ひのおか".to_string(),
        };
        let msg = format!("{err}");
        assert!(msg.contains("日野岡"));
        assert!(msg.contains("ひのおか"));
    }

    #[test]
    fn not_found_display_is_stable_string() {
        let err = StorageError::NotFound;
        assert_eq!(format!("{err}"), "entry not found");
    }

    #[test]
    fn sqlite_error_from_conversion_works() {
        let sqlite_err = rusqlite::Error::QueryReturnedNoRows;
        let storage_err: StorageError = sqlite_err.into();
        assert!(matches!(storage_err, StorageError::Sqlite(_)));
    }

    #[test]
    fn io_error_from_conversion_works() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "no such file");
        let storage_err: StorageError = io_err.into();
        assert!(matches!(storage_err, StorageError::Io(_)));
    }

    #[test]
    fn home_dir_not_found_display_is_stable() {
        let err = StorageError::HomeDirNotFound;
        let msg = format!("{err}");
        assert!(msg.contains("home directory not found"));
    }
}
