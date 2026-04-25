//! `kotoha-dict` — P2-B user dictionary CLI binary entry point.
//!
//! 本 binary は spec §7 に従い、user dictionary の add / remove / list /
//! show 操作を SQLite-backed `kotoha-storage` 経由で提供する。
//!
//! D2 時点では scaffold のみで、command dispatch および backend 結線は
//! D8 で実装する(P2-B plan §D8)。
//!
//! Spec: `docs/superpowers/specs/2026-04-25-p2-b-user-dictionary-design.md` §7。

fn main() {
    // D8 で `kotoha_cli::dict_cli::Cli::parse()` 経由の dispatch に置換する。
    eprintln!("kotoha-dict: not yet implemented (D2 scaffold; D8 wires up dispatch)");
    std::process::exit(1);
}
