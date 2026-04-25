//! `kotoha-dict` CLI 実装(spec §7、P2-B、`dict-persist` feature 下)。

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

/// kotoha-dict 全体の CLI。
#[derive(Debug, Parser)]
#[command(
    name = "kotoha-dict",
    about = "Kotoha User dictionary CLI (P2-B)",
    version
)]
pub struct Cli {
    /// DB 配置 dir を上書き(spec §7.1、KOTOHA_DATA_DIR と等価、CLI 引数優先)。
    #[arg(long, value_name = "PATH", global = true)]
    pub data_dir: Option<PathBuf>,

    /// 成功時の確認メッセージを抑止(spec §7.1)。
    #[arg(long, global = true)]
    pub quiet: bool,

    /// 出力を JSON 形式で返す(`list` / `show` 用、spec §7.1)。
    #[arg(long, global = true)]
    pub json: bool,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Add an entry to the user dictionary.
    Add(AddArgs),
    /// Remove an entry by id, or by surface+reading.
    Remove(RemoveArgs),
    /// List entries (text or JSON format).
    List(ListArgs),
    /// Show details for a single entry by id.
    Show(ShowArgs),
}

#[derive(Debug, Args)]
pub struct AddArgs {
    pub surface: String,
    pub reading: String,
    #[arg(long, default_value = "名詞-固有名詞-一般")]
    pub pos: String,
    #[arg(long, default_value_t = 1.0)]
    pub score: f32,
}

#[derive(Debug, Args)]
#[command(group = clap::ArgGroup::new("target").required(true).multiple(false))]
pub struct RemoveArgs {
    /// Remove by id.
    #[arg(group = "target")]
    pub id: Option<i64>,
    /// Remove by surface + reading(両指定 / 両未指定は exit 2、spec §7.3)。
    #[arg(long, requires = "reading", group = "target")]
    pub surface: Option<String>,
    #[arg(long, requires = "surface")]
    pub reading: Option<String>,
}

#[derive(Debug, Args)]
pub struct ListArgs {
    #[arg(long, value_enum, default_value_t = ListFormat::Text)]
    pub format: ListFormat,
    #[arg(long)]
    pub reading: Option<String>,
    #[arg(long, default_value_t = 100)]
    pub limit: usize,
    #[arg(long, default_value_t = 0)]
    pub offset: usize,
}

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum ListFormat {
    Text,
    Json,
}

#[derive(Debug, Args)]
pub struct ShowArgs {
    pub id: i64,
}

/// Exit code 体系(spec §7.6)。
pub const EXIT_OK: i32 = 0;
pub const EXIT_INTERNAL: i32 = 1;
pub const EXIT_INPUT: i32 = 2;
pub const EXIT_DUPLICATE: i32 = 3;
pub const EXIT_NOT_FOUND: i32 = 4;
