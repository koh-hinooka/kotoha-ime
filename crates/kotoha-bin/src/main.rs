//! Kotoha IME entry binary。
//!
//! Phase 3-A spec §3.3 全体図の DI sequence を実装する:
//!
//! 1. tracing init(env var `KOTOHA_LOG`、default `info`)
//! 2. host detect(P3-A: IBus 固定)
//! 3. DB 配置 path 解決(`kotoha-storage::path::resolve_data_dir`)
//! 4. `Database::open` で SQLite 接続
//! 5. `SqliteUserVocabStore` / `SqliteLearningCacheStore` 構築
//! 6. `MorphologicalEngine` / LLM backend 構築(本 PR は Stub で wiring 確立)
//! 7. `HybridRanker::new(...).with_llm(llm)`
//! 8. `IBusHostBridge::new(object_path)` で session bus 接続
//! 9. `KotohaEngine::new(host, ranker, learning_writer)`
//! 10. `IBusEventDispatcher::new(engine)` で event loop 起動 stub
//!
//! 実 D-Bus event loop(`zbus::blocking::MessageStream` 経由の signal
//! receive + dispatch)は spec §13 Open Q 9 通り Phase 3-A 実装段階で
//! empirical に詳細化する。本 binary は **DI sequence + tracing log で
//! 起動確認可能な状態** を成果物とする。

use std::sync::mpsc;
use std::sync::Arc;

use anyhow::Context;
use tracing_subscriber::EnvFilter;

mod host_detect;

const ENGINE_OBJECT_PATH: &str = "/org/freedesktop/IBus/Engine/Kotoha";
const KOTOHA_LOG_ENV: &str = "KOTOHA_LOG";

fn main() -> anyhow::Result<()> {
    init_tracing();
    let host = host_detect::detect();
    tracing::info!(?host, "kotoha-bin starting");

    match host {
        host_detect::DetectedHost::IBus => run_ibus()?,
    }
    Ok(())
}

fn init_tracing() {
    let filter = EnvFilter::try_from_env(KOTOHA_LOG_ENV).unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(filter).init();
}

fn run_ibus() -> anyhow::Result<()> {
    // 3. DB 配置 path
    let db_path = kotoha_storage::path::resolve_data_dir().context("resolve kotoha.db path")?;
    tracing::info!(path = %db_path.display(), "opening database");

    // 4. DB open
    let db = kotoha_storage::database::Database::open(&db_path).context("open kotoha.db")?;

    // 5. stores
    let _user_vocab_store = Arc::new(kotoha_storage::user_vocab::SqliteUserVocabStore::new(
        db.clone(),
    ));
    let learning_store =
        Arc::new(kotoha_storage::learning_cache::SqliteLearningCacheStore::new(db.clone()));

    // 6 + 7. Ranker (P3-A M6 では DI sequence 確立に focus、
    // production の MorphologicalEngine / LLM 統合は L3 manual smoke 段階で
    // 詰める。本段階では `StubRanker` を暫定で構築)。
    let ranker: Arc<dyn kotoha_engine_core::Ranker> = Arc::new(StubRanker);

    // 8. host bridge: session bus 接続。失敗時は stub に fallback して engine
    // 自体は起動可能にする(L3 manual smoke 段階で実 IBus 接続を検証)。
    let host_bridge: Box<dyn kotoha_engine_core::IMEHostBridge> =
        match kotoha_engine_ibus::IBusHostBridge::new(ENGINE_OBJECT_PATH) {
            Ok(b) => Box::new(b),
            Err(e) => {
                tracing::warn!(error = %e, "IBus session bus connect failed; using stub host_bridge");
                Box::new(StubHostBridge)
            }
        };

    // 9. engine
    let engine = kotoha_engine_core::engine::KotohaEngine::new(host_bridge, ranker, learning_store);

    // 10. dispatcher (event loop stub)
    let _dispatcher = kotoha_engine_ibus::IBusEventDispatcher::new(engine);

    tracing::info!("kotoha engine wired up; event loop deferred to Phase 3-A L3 manual smoke");
    Ok(())
}

/// 暫定 stub ranker(P3-A M6 段階)。
///
/// Phase 3-A 実装段階で実 `HybridRanker::new(SudachiAdapter, ...)` に置換される。
/// 本 stub は `rank()` 内で何も送らず、空候補で完結する Ranker として動く。
struct StubRanker;

impl kotoha_engine_core::Ranker for StubRanker {
    fn rank(
        &self,
        _kana: &str,
        _ctx: &kotoha_engine_core::ConversionContext,
        _cancel: Arc<dyn kotoha_engine_core::CancellationToken>,
        _sink: mpsc::Sender<kotoha_engine_core::RankerOutput>,
    ) -> Result<(), kotoha_engine_core::RankerError> {
        Ok(())
    }
}

/// 暫定 stub host bridge(session bus 接続失敗時の fallback)。
struct StubHostBridge;

impl kotoha_engine_core::IMEHostBridge for StubHostBridge {
    fn update_preedit(&self, text: &str, cursor: usize, visible: bool) {
        tracing::trace!(text, cursor, visible, "stub update_preedit");
    }
    fn commit_text(&self, text: &str) {
        tracing::trace!(text, "stub commit_text");
    }
    fn update_candidates(&self, _update: kotoha_engine_core::CandidateUpdate) {
        tracing::trace!("stub update_candidates");
    }
    fn show_candidate_window(&self) {
        tracing::trace!("stub show_candidate_window");
    }
    fn hide_candidate_window(&self) {
        tracing::trace!("stub hide_candidate_window");
    }
}
