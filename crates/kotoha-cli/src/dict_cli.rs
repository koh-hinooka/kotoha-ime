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

/// `<READING>` 引数の auto-detect normalize(spec §7.7)。
///
/// # Preconditions
/// - `reading` は非空であること。
///
/// # Postconditions
/// - 返却される文字列は hiragana のみ(U+3040..=U+309F)に長音記号 `ー` (U+30FC) /
///   中黒 `・` (U+30FB) を加えた集合からのみ構成される。
///
/// # Errors
/// - `reading` が空文字列の場合。
/// - hiragana・ASCII の混在、カタカナ・漢字を含む場合。
/// - ASCII を `RomajiConverter::convert` に投じた結果、pending residue が残る場合。
///
/// # Behavior
/// - 全 hiragana(U+3040..=U+309F + U+30FC + U+30FB) → そのまま返す。
/// - 全 ASCII(U+0020..=U+007E) → `RomajiConverter::convert` で変換し、pending が空でかつ
///   committed が hiragana-only である場合のみ受理する。
/// - それ以外(混在・カタカナ・漢字) → `Err`。
pub fn normalize_reading(reading: &str) -> Result<String, String> {
    if reading.is_empty() {
        return Err("READING must not be empty".to_string());
    }
    let all_hiragana = reading.chars().all(|c| {
        let cp = c as u32;
        (0x3040..=0x309F).contains(&cp) || cp == 0x30FC || cp == 0x30FB
    });
    if all_hiragana {
        return Ok(reading.to_string());
    }
    let all_ascii = reading.chars().all(|c| {
        let cp = c as u32;
        (0x0020..=0x007E).contains(&cp)
    });
    if all_ascii {
        let converter = kotoha_core::romaji::RomajiConverter::new();
        let (committed, pending) = converter.convert(reading);
        // 変換後の pending が空でなければ ASCII tail が残っている → conversion 不完全。
        if !pending.is_empty() {
            return Err(format!(
                "READING (ASCII) conversion left residue '{pending}' — must convert to pure hiragana"
            ));
        }
        // committed が hiragana-only であることを念のため確認する。
        let still_ok = committed.chars().all(|c| {
            let cp = c as u32;
            (0x3040..=0x309F).contains(&cp) || cp == 0x30FC || cp == 0x30FB
        });
        if !still_ok {
            return Err("READING (ASCII) failed to convert to pure hiragana".to_string());
        }
        return Ok(committed);
    }
    Err("READING must be all hiragana or all ASCII romaji".to_string())
}

use kotoha_storage::error::StorageError;
use kotoha_storage::user_vocab::store::{UserVocabRecord, UserVocabStore};

/// `kotoha-dict add` 実装。
///
/// # Errors
///
/// Exit code を文字列で返す(`Result<exit_code, String>`)。
pub fn run_add(store: &dyn UserVocabStore, args: &AddArgs, quiet: bool) -> Result<i32, String> {
    let reading = match normalize_reading(&args.reading) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: {}", e);
            return Ok(EXIT_INPUT);
        }
    };
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let record = UserVocabRecord {
        id: None,
        surface: args.surface.clone(),
        reading,
        pos: args.pos.clone(),
        score: args.score,
        created_at: now,
        updated_at: now,
    };
    match store.insert(record) {
        Ok(id) => {
            if !quiet {
                println!(
                    "added: id={id} surface=\"{}\" reading=\"{}\" pos=\"{}\" score={}",
                    args.surface, args.reading, args.pos, args.score
                );
            }
            Ok(EXIT_OK)
        }
        Err(StorageError::DuplicateEntry { surface, reading }) => {
            eprintln!("error: duplicate entry: surface={surface} reading={reading}");
            Ok(EXIT_DUPLICATE)
        }
        Err(StorageError::InvalidField { name, reason }) => {
            eprintln!("error: invalid {name}: {reason}");
            Ok(EXIT_INPUT)
        }
        Err(e) => {
            eprintln!("error: {e}");
            Ok(EXIT_INTERNAL)
        }
    }
}

/// `kotoha-dict remove` 実装。
///
/// # Errors
///
/// Exit code を文字列で返す(`Result<exit_code, String>`)。
pub fn run_remove(
    store: &dyn UserVocabStore,
    args: &RemoveArgs,
    quiet: bool,
) -> Result<i32, String> {
    let result = match (args.id, &args.surface, &args.reading) {
        (Some(id), None, None) => store.delete_by_id(id).map(|()| (id, None)),
        (None, Some(s), Some(r)) => {
            let normalized = match normalize_reading(r) {
                Ok(n) => n,
                Err(e) => {
                    eprintln!("error: {e}");
                    return Ok(EXIT_INPUT);
                }
            };
            store
                .delete_by_surface_reading(s, &normalized)
                .map(|()| (0, Some(format!("{s}/{normalized}"))))
        }
        _ => {
            eprintln!("error: specify either <ID> or both --surface and --reading");
            return Ok(EXIT_INPUT);
        }
    };
    match result {
        Ok((id, label)) => {
            if !quiet {
                match label {
                    Some(l) => println!("removed: {l}"),
                    None => println!("removed: id={id}"),
                }
            }
            Ok(EXIT_OK)
        }
        Err(StorageError::NotFound) => {
            eprintln!("error: entry not found");
            Ok(EXIT_NOT_FOUND)
        }
        Err(StorageError::InvalidField { name, reason }) => {
            eprintln!("error: invalid {name}: {reason}");
            Ok(EXIT_INPUT)
        }
        Err(e) => {
            eprintln!("error: {e}");
            Ok(EXIT_INTERNAL)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_reading_passes_through_pure_hiragana() {
        let result = normalize_reading("ひのおか").expect("ok");
        assert_eq!(result, "ひのおか");
    }

    #[test]
    fn normalize_reading_passes_through_long_sound() {
        let result = normalize_reading("こーひー").expect("ok");
        assert_eq!(result, "こーひー");
    }

    #[test]
    fn normalize_reading_converts_pure_ascii_to_hiragana() {
        let result = normalize_reading("hinooka").expect("ok");
        assert_eq!(result, "ひのおか");
    }

    #[test]
    fn normalize_reading_rejects_mixed_hiragana_ascii() {
        let err = normalize_reading("ひのoka").unwrap_err();
        assert!(err.contains("hiragana") || err.contains("ASCII"));
    }

    #[test]
    fn normalize_reading_rejects_katakana() {
        let err = normalize_reading("カタカナ").unwrap_err();
        assert!(err.contains("hiragana") || err.contains("ASCII"));
    }

    #[test]
    fn normalize_reading_rejects_kanji() {
        let err = normalize_reading("漢字").unwrap_err();
        assert!(err.contains("hiragana") || err.contains("ASCII"));
    }

    #[test]
    fn normalize_reading_rejects_empty() {
        let err = normalize_reading("").unwrap_err();
        assert!(!err.is_empty());
    }
}
