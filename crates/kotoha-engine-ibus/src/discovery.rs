//! IBus private bus address discovery(#208 / spec §7.1)。
//!
//! ibus-daemon は D-Bus session bus ではなく自前の private bus 上で component を
//! 待ち受ける。本 module は IBus client library の `ibus_get_address()` と同一
//! semantics で address を解決する:
//!
//! 1. env `KOTOHA_IBUS_ADDRESS`(test / 開発時 override、Kotoha 固有)
//! 2. env `IBUS_ADDRESS`(IBus エコシステム標準 override)
//! 3. address file `$XDG_CONFIG_HOME/ibus/bus/<machine-id>-unix-<display>` の
//!    `IBUS_ADDRESS=` 行(通常経路)
//!
//! 文字列 parse / path 合成は純関数([`parse_address_file`] / [`display_number`] /
//! [`resolve_config_dir`] / [`compose_address_file_path`])に分離して L1 unit test
//! 対象とし、env / filesystem 読み取りは [`discover_ibus_address`] /
//! `address_file_path` に集約する(spec §10.1)。env / file 読み取りを含む
//! 全段 fallback の結合動作は L2 handshake test(priority 1 注入)と
//! L3 manual smoke(priority 3 実機)で検証する。
//!
//! 詳細仕様: `docs/specs/_uncategorized/p3-b-ibus-listener.md` §7.1、ADR 0021
//! Amendment 2026-06-11。

use std::path::{Path, PathBuf};

/// test / 開発時に private bus address を直接指定する Kotoha 固有 override。
pub(crate) const KOTOHA_IBUS_ADDRESS_ENV: &str = "KOTOHA_IBUS_ADDRESS";

/// IBus エコシステム標準の address override(`ibus_get_address()` 互換)。
pub(crate) const IBUS_ADDRESS_ENV: &str = "IBUS_ADDRESS";

/// address discovery の失敗 reason(spec §9.1: いずれも listener 起動失敗 →
/// main thread に propagate して process exit)。
#[derive(Debug, thiserror::Error)]
pub(crate) enum DiscoveryError {
    #[error("machine-id not readable (/var/lib/dbus/machine-id, /etc/machine-id): {0}")]
    MachineId(std::io::Error),
    #[error("DISPLAY not set; cannot derive IBus address file name")]
    DisplayUnset,
    #[error(
        "DISPLAY value {display:?} is not a local display (expected `:N` or `:N.S`); \
         cannot derive IBus address file name"
    )]
    DisplayInvalid { display: String },
    #[error("neither XDG_CONFIG_HOME nor HOME is set; cannot resolve IBus address file dir")]
    ConfigDirUnset,
    #[error("IBus address file not readable: {path}: {source}")]
    AddressFile {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("IBUS_ADDRESS= line not found in address file: {path}")]
    AddressLineMissing { path: PathBuf },
}

/// IBus private bus address を 3 段 fallback で解決する(spec §7.1)。
///
/// # Errors
///
/// 3 source すべてで解決できない場合に [`DiscoveryError`] を返す。caller
/// (`listener::build_connection`)は Err を main thread に propagate する。
pub(crate) fn discover_ibus_address() -> Result<String, DiscoveryError> {
    if let Some(addr) = env_override(KOTOHA_IBUS_ADDRESS_ENV) {
        tracing::info!(
            source = KOTOHA_IBUS_ADDRESS_ENV,
            "IBus address from env override"
        );
        return Ok(addr);
    }
    if let Some(addr) = env_override(IBUS_ADDRESS_ENV) {
        tracing::info!(source = IBUS_ADDRESS_ENV, "IBus address from env override");
        return Ok(addr);
    }
    let path = address_file_path()?;
    let content = std::fs::read_to_string(&path).map_err(|source| DiscoveryError::AddressFile {
        path: path.clone(),
        source,
    })?;
    let addr = parse_address_file(&content)
        .ok_or(DiscoveryError::AddressLineMissing { path: path.clone() })?;
    tracing::info!(file = %path.display(), "IBus address from address file");
    Ok(addr.to_owned())
}

/// env override を読み、**有効値のときのみ** `Some` を返す。
///
/// silent-failure 禁止(spec §9.3): 「設定されているが空 / 非 UTF-8」の override は
/// 黙って fallback せず、warn で観測経路を残してから次 source に進む。
fn env_override(name: &str) -> Option<String> {
    match std::env::var(name) {
        Ok(value) if !value.is_empty() => Some(value),
        Ok(_) => {
            tracing::warn!(
                source = name,
                "env override is set but empty; ignoring and trying the next source"
            );
            None
        }
        Err(std::env::VarError::NotUnicode(_)) => {
            tracing::warn!(
                source = name,
                "env override is set but not valid UTF-8; ignoring and trying the next source"
            );
            None
        }
        Err(std::env::VarError::NotPresent) => None,
    }
}

/// address file の path を env / filesystem から導出する(純関数 3 つの結合)。
fn address_file_path() -> Result<PathBuf, DiscoveryError> {
    let machine_id = std::fs::read_to_string("/var/lib/dbus/machine-id")
        .or_else(|_| std::fs::read_to_string("/etc/machine-id"))
        .map_err(DiscoveryError::MachineId)?;
    let display = std::env::var("DISPLAY").map_err(|_| DiscoveryError::DisplayUnset)?;
    let display_num = display_number(&display).ok_or_else(|| DiscoveryError::DisplayInvalid {
        display: display.clone(),
    })?;
    let config_dir = resolve_config_dir(
        std::env::var("XDG_CONFIG_HOME").ok(),
        std::env::var("HOME").ok(),
    )?;
    Ok(compose_address_file_path(
        &config_dir,
        &machine_id,
        display_num,
    ))
}

/// config dir を解決する純関数: `XDG_CONFIG_HOME`(非空)→ `$HOME/.config` →
/// いずれも無ければ Err。
fn resolve_config_dir(
    xdg_config_home: Option<String>,
    home: Option<String>,
) -> Result<PathBuf, DiscoveryError> {
    if let Some(xdg) = xdg_config_home.filter(|v| !v.is_empty()) {
        return Ok(PathBuf::from(xdg));
    }
    home.filter(|v| !v.is_empty())
        .map(|h| PathBuf::from(h).join(".config"))
        .ok_or(DiscoveryError::ConfigDirUnset)
}

/// address file の full path を合成する純関数:
/// `<config_dir>/ibus/bus/<machine-id(trim 済)>-unix-<display_num>`。
///
/// - `<machine-id>`: 読み取り raw 値を受け取り、ここで trim する(末尾改行対策)
/// - `unix`: local display の hostname 固定値(`ibus_get_address()` 互換)
fn compose_address_file_path(config_dir: &Path, machine_id: &str, display_num: &str) -> PathBuf {
    let file_name = format!("{}-unix-{}", machine_id.trim(), display_num);
    config_dir.join("ibus").join("bus").join(file_name)
}

/// address file の本文から `IBUS_ADDRESS=` 行の値を取り出す純関数。
///
/// file 形式(ibus-daemon が生成):
///
/// ```text
/// # This file is created by ibus-daemon, please do not modify it
/// IBUS_ADDRESS=unix:abstract=/home/user/.cache/ibus/dbus-XXXX,guid=...
/// IBUS_DAEMON_PID=12345
/// ```
pub(crate) fn parse_address_file(content: &str) -> Option<&str> {
    content
        .lines()
        .find_map(|line| line.strip_prefix("IBUS_ADDRESS="))
        .filter(|v| !v.is_empty())
}

/// env `DISPLAY` 値から display 番号を取り出す純関数(`":0"` → `"0"`、
/// `":0.0"` → `"0"`)。
///
/// spec §7.1 の対象は local display(`:N` / `:N.S` 形式)のみ。hostname prefix
/// 付き(`localhost:0.0` 等)や数字以外を含む値は `None` で reject する
/// (file 名への `/` / `..` 混入も構造的に塞ぐ)。
fn display_number(display: &str) -> Option<&str> {
    let trimmed = display.strip_prefix(':')?;
    let num = trimmed.split('.').next().unwrap_or(trimmed);
    if !num.is_empty() && num.bytes().all(|b| b.is_ascii_digit()) {
        Some(num)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    //! Spec: `docs/specs/_uncategorized/p3-b-ibus-listener.md` §10.1
    //! L1 unit tests。env / filesystem 非依存の純関数のみを対象とする。env / file
    //! 読み取りを含む結合動作は L2 handshake test(`KOTOHA_IBUS_ADDRESS` 注入 =
    //! priority 1 経路)と L3 manual smoke(address file = priority 3 経路)で
    //! 検証する。

    use super::*;

    #[test]
    fn parse_address_file_extracts_ibus_address_line() {
        let content = "\
# This file is created by ibus-daemon, please do not modify it
IBUS_ADDRESS=unix:abstract=/home/user/.cache/ibus/dbus-abcd,guid=feedface
IBUS_DAEMON_PID=12345
";
        assert_eq!(
            parse_address_file(content),
            Some("unix:abstract=/home/user/.cache/ibus/dbus-abcd,guid=feedface")
        );
    }

    #[test]
    fn parse_address_file_returns_none_when_line_missing() {
        let content = "# comment only\nIBUS_DAEMON_PID=12345\n";
        assert_eq!(parse_address_file(content), None);
    }

    #[test]
    fn parse_address_file_returns_none_on_empty_input() {
        assert_eq!(parse_address_file(""), None);
        // 値が空の IBUS_ADDRESS= 行も不正として None
        assert_eq!(parse_address_file("IBUS_ADDRESS=\n"), None);
    }

    #[test]
    fn display_number_extracts_local_display_number() {
        assert_eq!(display_number(":0"), Some("0"));
        assert_eq!(display_number(":0.0"), Some("0"));
        assert_eq!(display_number(":12"), Some("12"));
    }

    #[test]
    fn display_number_rejects_non_local_or_malformed_display() {
        // hostname prefix 付き network display は spec §7.1 scope 外
        assert_eq!(display_number("localhost:0.0"), None);
        // 数字以外(path traversal 防止: `/` や `..` を file 名に混入させない)
        assert_eq!(display_number(":0/../../../../tmp/evil"), None);
        assert_eq!(display_number(""), None);
        assert_eq!(display_number(":"), None);
    }

    #[test]
    fn resolve_config_dir_prefers_xdg_then_home() {
        assert_eq!(
            resolve_config_dir(Some("/xdg".into()), Some("/home/u".into())).unwrap(),
            PathBuf::from("/xdg")
        );
        // XDG 未設定(または空)→ $HOME/.config
        assert_eq!(
            resolve_config_dir(None, Some("/home/u".into())).unwrap(),
            PathBuf::from("/home/u/.config")
        );
        assert_eq!(
            resolve_config_dir(Some(String::new()), Some("/home/u".into())).unwrap(),
            PathBuf::from("/home/u/.config")
        );
        assert!(matches!(
            resolve_config_dir(None, None),
            Err(DiscoveryError::ConfigDirUnset)
        ));
    }

    #[test]
    fn compose_address_file_path_trims_machine_id_and_assembles_full_path() {
        // machine-id file の raw 値は末尾改行を含む
        let path = compose_address_file_path(
            Path::new("/home/u/.config"),
            "0123456789abcdef0123456789abcdef\n",
            "0",
        );
        assert_eq!(
            path,
            PathBuf::from("/home/u/.config/ibus/bus/0123456789abcdef0123456789abcdef-unix-0")
        );
    }
}
