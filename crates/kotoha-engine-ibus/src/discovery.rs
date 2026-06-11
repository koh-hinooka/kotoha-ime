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
//! 文字列 parse は純関数([`parse_address_file`] / [`display_number`])に分離し、
//! env / filesystem 入力は [`discover_ibus_address`] / `address_file_path` に
//! 集約する(L1 unit test 可能性、spec §10.1)。
//!
//! 詳細仕様: `docs/specs/_uncategorized/p3-b-ibus-listener.md` §7.1、ADR 0021
//! Amendment 2026-06-11。

use std::path::PathBuf;

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
    #[error("HOME not set; cannot resolve XDG config dir for IBus address file")]
    HomeUnset,
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
    if let Ok(addr) = std::env::var(KOTOHA_IBUS_ADDRESS_ENV) {
        if !addr.is_empty() {
            tracing::info!(
                source = KOTOHA_IBUS_ADDRESS_ENV,
                "IBus address from env override"
            );
            return Ok(addr);
        }
    }
    if let Ok(addr) = std::env::var(IBUS_ADDRESS_ENV) {
        if !addr.is_empty() {
            tracing::info!(source = IBUS_ADDRESS_ENV, "IBus address from env override");
            return Ok(addr);
        }
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

/// address file の path を導出する:
/// `$XDG_CONFIG_HOME/ibus/bus/<machine-id>-unix-<display>`。
///
/// - `<machine-id>`: `/var/lib/dbus/machine-id`(fallback `/etc/machine-id`)、trim 済
/// - `unix`: local display の hostname 固定値(`ibus_get_address()` 互換)
/// - `<display>`: env `DISPLAY` の display 番号([`display_number`])
fn address_file_path() -> Result<PathBuf, DiscoveryError> {
    let machine_id = std::fs::read_to_string("/var/lib/dbus/machine-id")
        .or_else(|_| std::fs::read_to_string("/etc/machine-id"))
        .map_err(DiscoveryError::MachineId)?;
    let display = std::env::var("DISPLAY").map_err(|_| DiscoveryError::DisplayUnset)?;
    let config_dir = std::env::var("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|_| {
            std::env::var("HOME")
                .map(|h| PathBuf::from(h).join(".config"))
                .map_err(|_| DiscoveryError::HomeUnset)
        })?;
    let file_name = format!("{}-unix-{}", machine_id.trim(), display_number(&display));
    Ok(config_dir.join("ibus").join("bus").join(file_name))
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
fn display_number(display: &str) -> &str {
    let trimmed = display.strip_prefix(':').unwrap_or(display);
    trimmed.split('.').next().unwrap_or(trimmed)
}

#[cfg(test)]
mod tests {
    //! Spec: `docs/specs/_uncategorized/p3-b-ibus-listener.md` §10.1
    //! L1 unit tests。env / filesystem 非依存の純関数のみを対象とする
    //! (`discover_ibus_address` の env 経路は L2 handshake test で
    //! `KOTOHA_IBUS_ADDRESS` 注入により exercise される)。

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
    fn display_number_strips_colon_and_screen_suffix() {
        assert_eq!(display_number(":0"), "0");
        assert_eq!(display_number(":0.0"), "0");
        assert_eq!(display_number(":12"), "12");
    }
}
