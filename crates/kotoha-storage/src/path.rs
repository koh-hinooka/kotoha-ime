//! DB 配置 path の resolve + 8-step containment 検証(spec §5.4)。

use std::env;
use std::fs;
use std::path::PathBuf;

use crate::error::StorageError;

/// System path blacklist。spec §5.4.1 step 3 / step 6 で使用。
const BLACKLIST: &[&str] = &[
    "/etc", "/var", "/tmp", "/proc", "/sys", "/dev", "/root", "/boot",
];

/// DB 配置 path を resolve / 検証する 8-step protocol(spec §5.4.1)。
///
/// # Postconditions
///
/// - 戻り値は absolute path で `kotoha.db` で終わる
/// - 戻り値の親 dir は実在 + canonicalize 済
/// - blacklist 配下に到達するすべての経路は reject 済
///
/// # Errors
///
/// - [`StorageError::InvalidPath`] when prefix containment / blacklist が違反
/// - [`StorageError::Io`] when create_dir_all / canonicalize が失敗
/// - [`StorageError::HomeDirNotFound`] when all of KOTOHA_DATA_DIR / XDG_DATA_HOME / HOME are unset
pub fn resolve_data_dir() -> Result<PathBuf, StorageError> {
    // step 1: target_dir 解決
    let (target_dir, allowed_prefix) = if let Ok(custom) = env::var("KOTOHA_DATA_DIR") {
        let p = PathBuf::from(&custom);
        (p.clone(), p)
    } else if let Ok(xdg) = env::var("XDG_DATA_HOME") {
        let prefix = PathBuf::from(&xdg);
        (prefix.join("kotoha"), prefix)
    } else if let Ok(home) = env::var("HOME") {
        let prefix = PathBuf::from(&home);
        (prefix.join(".local/share/kotoha"), prefix)
    } else {
        return Err(StorageError::HomeDirNotFound);
    };

    // step 2: prefix containment 検証(canonicalize 前)
    if !target_dir.starts_with(&allowed_prefix) {
        return Err(StorageError::InvalidPath {
            path: target_dir.clone(),
            reason: format!(
                "target_dir does not start with allowed prefix {}",
                allowed_prefix.display()
            ),
        });
    }

    // step 3: system path blacklist(canonicalize 前)
    if let Some(p) = BLACKLIST.iter().find(|p| target_dir.starts_with(p)) {
        return Err(StorageError::InvalidPath {
            path: target_dir.clone(),
            reason: format!("system path blacklisted (pre-canonicalize): {p}"),
        });
    }

    // step 4: dir 作成(canonicalize 前)
    fs::create_dir_all(&target_dir)?;

    // step 5: canonicalize
    let canonical = target_dir.canonicalize()?;

    // step 6: prefix containment + blacklist 再検証(canonicalize 後、defense-in-depth)
    if let Some(p) = BLACKLIST.iter().find(|p| canonical.starts_with(p)) {
        return Err(StorageError::InvalidPath {
            path: canonical.clone(),
            reason: format!("symlink resolved to blacklisted path: {p}"),
        });
    }

    // step 7: 絶対 path assert
    if !canonical.is_absolute() {
        return Err(StorageError::InvalidPath {
            path: canonical.clone(),
            reason: "not absolute after canonicalize".to_string(),
        });
    }

    let db_path = canonical.join("kotoha.db");

    // step 8: DB ファイル size sanity warn(100 MiB 超過時 stderr)
    if let Ok(meta) = fs::metadata(&db_path) {
        if meta.len() > 100 * 1024 * 1024 {
            eprintln!(
                "warning: kotoha.db size {} bytes exceeds 100 MiB threshold",
                meta.len()
            );
        }
    }

    Ok(db_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use std::env;
    use std::path::Path;

    // env var 操作は process-global のため #[serial] で順次実行する(spec §10.3.1)。

    fn save_env() -> (Option<String>, Option<String>, Option<String>) {
        let a = env::var("KOTOHA_DATA_DIR").ok();
        let b = env::var("XDG_DATA_HOME").ok();
        let c = env::var("HOME").ok();
        (a, b, c)
    }

    fn restore_env(saved: (Option<String>, Option<String>, Option<String>)) {
        for (k, v) in [
            ("KOTOHA_DATA_DIR", saved.0),
            ("XDG_DATA_HOME", saved.1),
            ("HOME", saved.2),
        ] {
            match v {
                Some(s) => env::set_var(k, s),
                None => env::remove_var(k),
            }
        }
    }

    /// Cargo の `target/` 配下に test 用 tempdir を作成する。
    ///
    /// `tempfile::tempdir()` は default で `/tmp` 配下に dir を作成するが、
    /// spec §5.4.1 step 3 の system path blacklist が `/tmp` を含むため、
    /// resolve_data_dir() の test では blacklist 衝突を避けて
    /// `$CARGO_MANIFEST_DIR/../../target/test-tmp/` 配下に作成する。
    fn make_tempdir() -> tempfile::TempDir {
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        // workspace root の target/ を base にする(crate dir = workspace/crates/kotoha-storage)
        let base = Path::new(manifest_dir)
            .parent()
            .and_then(Path::parent)
            .expect("workspace root must exist")
            .join("target")
            .join("test-tmp-path");
        std::fs::create_dir_all(&base).expect("create test-tmp dir");
        tempfile::Builder::new()
            .prefix("kotoha-path-test-")
            .tempdir_in(&base)
            .expect("tempdir_in must succeed")
    }

    #[test]
    #[serial]
    fn rejects_system_path_blacklist_etc() {
        let saved = save_env();
        env::set_var("KOTOHA_DATA_DIR", "/etc/kotoha");
        let result = resolve_data_dir();
        assert!(matches!(result, Err(StorageError::InvalidPath { .. })));
        restore_env(saved);
    }

    #[test]
    #[serial]
    fn rejects_system_path_blacklist_proc() {
        let saved = save_env();
        env::set_var("KOTOHA_DATA_DIR", "/proc/kotoha");
        let result = resolve_data_dir();
        assert!(matches!(result, Err(StorageError::InvalidPath { .. })));
        restore_env(saved);
    }

    #[test]
    #[serial]
    fn accepts_tempdir_path_when_explicit_kotoha_data_dir_set() {
        let saved = save_env();
        let tmp = make_tempdir();
        env::set_var("KOTOHA_DATA_DIR", tmp.path());
        let result = resolve_data_dir().expect("explicit tempdir must succeed");
        assert!(result.is_absolute());
        assert!(result.ends_with("kotoha.db"));
        restore_env(saved);
    }

    #[test]
    #[serial]
    fn falls_back_to_xdg_when_kotoha_data_dir_absent() {
        let saved = save_env();
        let tmp = make_tempdir();
        env::remove_var("KOTOHA_DATA_DIR");
        env::set_var("XDG_DATA_HOME", tmp.path());
        let result = resolve_data_dir().expect("XDG fallback must succeed");
        let canonical_tmp = tmp.path().canonicalize().expect("canonicalize tmp");
        assert!(result.starts_with(&canonical_tmp));
        restore_env(saved);
    }

    #[test]
    #[serial]
    fn falls_back_to_home_local_share_when_xdg_absent() {
        let saved = save_env();
        let tmp = make_tempdir();
        env::remove_var("KOTOHA_DATA_DIR");
        env::remove_var("XDG_DATA_HOME");
        env::set_var("HOME", tmp.path());
        let result = resolve_data_dir().expect("HOME fallback must succeed");
        let canonical_tmp = tmp.path().canonicalize().expect("canonicalize tmp");
        let expected_prefix = canonical_tmp.join(".local/share/kotoha");
        assert!(result.starts_with(&expected_prefix));
        restore_env(saved);
    }

    #[test]
    #[serial]
    fn returns_home_dir_not_found_when_all_envs_absent() {
        let saved = save_env();
        env::remove_var("KOTOHA_DATA_DIR");
        env::remove_var("XDG_DATA_HOME");
        env::remove_var("HOME");
        let result = resolve_data_dir();
        assert!(matches!(result, Err(StorageError::HomeDirNotFound)));
        restore_env(saved);
    }
}
