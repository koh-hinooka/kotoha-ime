//! Kotoha IME entry binary。
//!
//! Phase 3-A spec §3.3 全体図の DI sequence を実装する:
//!
//! 1. tracing init(env var `KOTOHA_LOG`、default `info`)
//! 2. host detect(P3-A: IBus 固定)
//! 3. DB 配置 path 解決(`kotoha-storage::path::resolve_data_dir`)
//! 4. `Database::open` で SQLite 接続
//! 5. `SqliteUserVocabStore` / `SqliteLearningCacheStore` 構築
//! 6. `MorphologicalEngine` を SudachiDict-core から構築
//!    (env var `KOTOHA_SYSTEM_DICT_PATH`、未設定時は `StubRanker` fallback)
//! 7. `HybridRanker::new(sudachi, user_vocab, learning)` を構築
//!    (LLM 統合は Phase 3-B B2+ で詳細化、現段階では dict-only)
//! 8. `IBusHostBridge::new(object_path)` で session bus 接続
//!    (失敗時は `StubHostBridge` fallback)
//! 9. `KotohaEngine::new(host, ranker, learning_writer)`
//! 10. `IBusEventDispatcher::new(engine)` で event loop 起動 stub
//!
//! 実 D-Bus event loop(`zbus::blocking::MessageStream` 経由の signal
//! receive + dispatch)は Phase 3-B B3 / spec §13 Open Q 9 で詳細化する。

use std::path::PathBuf;
use std::sync::mpsc;
use std::sync::Arc;

use anyhow::Context;
use tracing_subscriber::EnvFilter;

mod host_detect;

const ENGINE_OBJECT_PATH: &str = "/org/freedesktop/IBus/Engine/Kotoha";
const KOTOHA_LOG_ENV: &str = "KOTOHA_LOG";
const KOTOHA_SYSTEM_DICT_PATH_ENV: &str = "KOTOHA_SYSTEM_DICT_PATH";

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
    let user_vocab_store = Arc::new(kotoha_storage::user_vocab::SqliteUserVocabStore::new(
        db.clone(),
    ));
    let learning_store =
        Arc::new(kotoha_storage::learning_cache::SqliteLearningCacheStore::new(db.clone()));

    // 6 + 7. Ranker: SudachiDict-core を env var 経由で読み、HybridRanker を構築する。
    // dict path 未設定 / load 失敗 / その他事故では StubRanker fallback で起動を続行
    // (development / CI 環境での起動可能性を担保、production は env var 設定必須)。
    let ranker: Arc<dyn kotoha_engine_core::Ranker> = match build_hybrid_ranker(
        user_vocab_store.clone(),
        learning_store.clone(),
    ) {
        Ok(r) => {
            tracing::info!("HybridRanker constructed (dict-only path; LLM is Phase 3-B B2+)");
            r
        }
        Err(e) => {
            tracing::warn!(error = %e, "failed to construct HybridRanker; falling back to StubRanker");
            Arc::new(StubRanker)
        }
    };

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

    tracing::info!("kotoha engine wired up; event loop deferred to Phase 3-B B3");
    Ok(())
}

/// `HybridRanker` を構築する production helper。
///
/// SudachiDict-core を env var `KOTOHA_SYSTEM_DICT_PATH` から読み込み、
/// 失敗時は `Err` で fallback path に return する。LLM backend は Phase 3-B
/// B2+ で feature gate 付きで追加する。
fn build_hybrid_ranker(
    user_vocab: Arc<kotoha_storage::user_vocab::SqliteUserVocabStore>,
    learning_cache: Arc<kotoha_storage::learning_cache::SqliteLearningCacheStore>,
) -> anyhow::Result<Arc<dyn kotoha_engine_core::Ranker>> {
    let dict_path: PathBuf = std::env::var(KOTOHA_SYSTEM_DICT_PATH_ENV)
        .map(PathBuf::from)
        .with_context(|| {
            format!("env var {KOTOHA_SYSTEM_DICT_PATH_ENV} is required for HybridRanker")
        })?;
    let sudachi = kotoha_core::dict::load_morphological_engine(&dict_path)
        .with_context(|| format!("load SudachiDict from {}", dict_path.display()))?;
    let ranker = kotoha_engine_core::HybridRanker::new(sudachi, user_vocab, learning_cache);
    Ok(Arc::new(ranker))
}

/// 暫定 stub ranker(env var 未設定 / Sudachi load 失敗時の fallback)。
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
