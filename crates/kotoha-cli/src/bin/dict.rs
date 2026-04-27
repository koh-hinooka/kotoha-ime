//! `kotoha-dict` binary entry point (P2-B、spec §7.1)。

#![cfg(feature = "dict-persist")]

use clap::Parser;

use kotoha_cli::dict_cli::{run_add, run_list, run_remove, run_show, Cli, Command, EXIT_INTERNAL};

fn main() {
    let cli = Cli::parse();

    // DB path 解決: --data-dir > KOTOHA_DATA_DIR > XDG > HOME(spec §5.4)
    if let Some(dir) = &cli.data_dir {
        std::env::set_var("KOTOHA_DATA_DIR", dir);
    }
    let db_path = match kotoha_storage::path::resolve_data_dir() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("error: failed to resolve data dir: {e}");
            std::process::exit(EXIT_INTERNAL);
        }
    };
    let db = match kotoha_storage::database::Database::open(&db_path) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("error: failed to open DB at {}: {e}", db_path.display());
            std::process::exit(EXIT_INTERNAL);
        }
    };
    let reader = db.user_vocab_reader();
    let writer = db.user_vocab_writer();

    let exit_code = match &cli.command {
        Command::Add(args) => run_add(writer.as_ref(), args, cli.quiet).unwrap_or(EXIT_INTERNAL),
        Command::Remove(args) => {
            run_remove(writer.as_ref(), args, cli.quiet).unwrap_or(EXIT_INTERNAL)
        }
        Command::List(args) => run_list(reader.as_ref(), args, cli.json).unwrap_or(EXIT_INTERNAL),
        Command::Show(args) => run_show(reader.as_ref(), args, cli.json).unwrap_or(EXIT_INTERNAL),
    };
    std::process::exit(exit_code);
}
