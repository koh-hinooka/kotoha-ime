//! Layer 3 integration test for `kotoha-dict` CLI (spec §10.3 / §10.3.1)。
//!
//! `KOTOHA_DATA_DIR` env var 操作は process-global のため `#[serial]` 必須。

#![cfg(feature = "dict-persist")]

use assert_cmd::Command;
use serial_test::serial;

fn cli() -> Command {
    Command::cargo_bin("kotoha-dict").expect("kotoha-dict binary built")
}

/// `tempfile::tempdir()` 代替: `/tmp` は kotoha-storage の resolve_data_dir
/// blacklist に含まれるため、workspace `target/test-tmp-cli/` 配下に作成する
/// (Phase A Task A3 と同じ pattern)。
fn safe_tempdir() -> tempfile::TempDir {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let base = std::path::Path::new(manifest_dir)
        .parent()
        .and_then(std::path::Path::parent)
        .expect("workspace root must exist")
        .join("target")
        .join("test-tmp-cli");
    std::fs::create_dir_all(&base).expect("create test-tmp-cli dir");
    tempfile::Builder::new()
        .prefix("kotoha-cli-test-")
        .tempdir_in(&base)
        .expect("tempdir_in must succeed")
}

#[test]
#[serial]
fn add_subcommand_succeeds_with_pure_hiragana_reading() {
    let tmp = safe_tempdir();
    cli()
        .env("KOTOHA_DATA_DIR", tmp.path())
        .args(["add", "日野岡", "ひのおか"])
        .assert()
        .success();
}

#[test]
#[serial]
fn add_subcommand_rejects_mixed_reading() {
    let tmp = safe_tempdir();
    cli()
        .env("KOTOHA_DATA_DIR", tmp.path())
        .args(["add", "日野岡", "ひのoka"])
        .assert()
        .code(2);
}

#[test]
#[serial]
fn add_subcommand_returns_3_on_duplicate() {
    let tmp = safe_tempdir();
    cli()
        .env("KOTOHA_DATA_DIR", tmp.path())
        .args(["add", "X", "あ"])
        .assert()
        .success();
    cli()
        .env("KOTOHA_DATA_DIR", tmp.path())
        .args(["add", "X", "あ"])
        .assert()
        .code(3);
}

#[test]
#[serial]
fn add_subcommand_returns_2_on_nan_score() {
    let tmp = safe_tempdir();
    cli()
        .env("KOTOHA_DATA_DIR", tmp.path())
        .args(["add", "X", "あ", "--score", "nan"])
        .assert()
        .code(2);
}

#[test]
#[serial]
fn remove_subcommand_by_id_succeeds() {
    let tmp = safe_tempdir();
    cli()
        .env("KOTOHA_DATA_DIR", tmp.path())
        .args(["add", "X", "あ"])
        .assert()
        .success();
    cli()
        .env("KOTOHA_DATA_DIR", tmp.path())
        .args(["remove", "1"])
        .assert()
        .success();
}

#[test]
#[serial]
fn remove_subcommand_by_id_returns_4_when_absent() {
    let tmp = safe_tempdir();
    cli()
        .env("KOTOHA_DATA_DIR", tmp.path())
        .args(["remove", "999"])
        .assert()
        .code(4);
}

#[test]
#[serial]
fn remove_subcommand_by_surface_reading_succeeds() {
    let tmp = safe_tempdir();
    cli()
        .env("KOTOHA_DATA_DIR", tmp.path())
        .args(["add", "日野岡", "ひのおか"])
        .assert()
        .success();
    cli()
        .env("KOTOHA_DATA_DIR", tmp.path())
        .args(["remove", "--surface", "日野岡", "--reading", "ひのおか"])
        .assert()
        .success();
}

#[test]
#[serial]
fn list_subcommand_text_format() {
    let tmp = safe_tempdir();
    cli()
        .env("KOTOHA_DATA_DIR", tmp.path())
        .args(["add", "日野岡", "ひのおか"])
        .assert()
        .success();
    let output = cli()
        .env("KOTOHA_DATA_DIR", tmp.path())
        .arg("list")
        .output()
        .expect("output");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("日野岡"));
}

#[test]
#[serial]
fn list_subcommand_json_format_parseable() {
    let tmp = safe_tempdir();
    cli()
        .env("KOTOHA_DATA_DIR", tmp.path())
        .args(["add", "日野岡", "ひのおか"])
        .assert()
        .success();
    let output = cli()
        .env("KOTOHA_DATA_DIR", tmp.path())
        .args(["list", "--format", "json"])
        .output()
        .expect("output");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.starts_with('['));
    assert!(stdout.trim_end().ends_with(']'));
    assert!(stdout.contains("日野岡"));
}

#[test]
#[serial]
fn list_subcommand_with_reading_prefix() {
    let tmp = safe_tempdir();
    cli()
        .env("KOTOHA_DATA_DIR", tmp.path())
        .args(["add", "日野岡", "ひのおか"])
        .assert()
        .success();
    cli()
        .env("KOTOHA_DATA_DIR", tmp.path())
        .args(["add", "別人", "べつじん"])
        .assert()
        .success();
    let output = cli()
        .env("KOTOHA_DATA_DIR", tmp.path())
        .args(["list", "--reading", "ひの"])
        .output()
        .expect("output");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("日野岡"));
    assert!(!stdout.contains("別人"));
}

#[test]
#[serial]
fn show_subcommand_returns_4_when_absent() {
    let tmp = safe_tempdir();
    cli()
        .env("KOTOHA_DATA_DIR", tmp.path())
        .args(["show", "999"])
        .assert()
        .code(4);
}

#[test]
#[serial]
fn show_subcommand_displays_entry() {
    let tmp = safe_tempdir();
    cli()
        .env("KOTOHA_DATA_DIR", tmp.path())
        .args(["add", "日野岡", "ひのおか"])
        .assert()
        .success();
    cli()
        .env("KOTOHA_DATA_DIR", tmp.path())
        .args(["show", "1"])
        .assert()
        .success();
}
