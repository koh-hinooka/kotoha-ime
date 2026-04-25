//! Layer 3 integration test for `kotoha-dict` CLI (spec §10.3 / §10.3.1)。
//!
//! `KOTOHA_DATA_DIR` env var 操作は process-global のため `#[serial]` 必須。

#![cfg(feature = "dict-persist")]

use assert_cmd::Command;
use serial_test::serial;
use tempfile::tempdir;

fn cli() -> Command {
    Command::cargo_bin("kotoha-dict").expect("kotoha-dict binary built")
}

#[test]
#[serial]
fn add_subcommand_succeeds_with_pure_hiragana_reading() {
    let tmp = tempdir().unwrap();
    cli()
        .env("KOTOHA_DATA_DIR", tmp.path())
        .args(["add", "日野岡", "ひのおか"])
        .assert()
        .success();
}

#[test]
#[serial]
fn add_subcommand_rejects_mixed_reading() {
    let tmp = tempdir().unwrap();
    cli()
        .env("KOTOHA_DATA_DIR", tmp.path())
        .args(["add", "日野岡", "ひのoka"])
        .assert()
        .code(2);
}

#[test]
#[serial]
fn add_subcommand_returns_3_on_duplicate() {
    let tmp = tempdir().unwrap();
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
    let tmp = tempdir().unwrap();
    cli()
        .env("KOTOHA_DATA_DIR", tmp.path())
        .args(["add", "X", "あ", "--score", "nan"])
        .assert()
        .code(2);
}
