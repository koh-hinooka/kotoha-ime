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
//!    (env var `KOTOHA_SYSTEM_DICT_PATH` 必須、`KOTOHA_ALLOW_STUB=1` 時のみ
//!    `StubRanker` fallback 許可)
//! 7. `HybridRanker::new(sudachi, user_vocab, learning)` を構築
//!    (LLM 統合は Phase 3-B B2+ で詳細化、現段階では dict-only)
//! 8. `IBusHostBridge::new(object_path)` で session bus 接続
//!    (`KOTOHA_ALLOW_STUB=1` 時のみ `StubHostBridge` fallback 許可)
//! 9. `KotohaEngine::new(host, ranker, learning_writer)` (Result)
//! 10. `IBusEventDispatcher::new(engine)` で event loop 起動 stub
//!
//! 実 D-Bus event loop(`zbus::blocking::MessageStream` 経由の signal
//! receive + dispatch)は Phase 3-B B3 / spec §13 Open Q 9 で詳細化する。
//!
//! # Panic recovery (spec §9.1 row 5)
//!
//! `main` は `std::panic::catch_unwind` で `run()` 全体を包み、unwind
//! 観測時は `tracing::error!` + exit code 75 (EX_TEMPFAIL) で terminate
//! する。これにより systemd 等が restart loop に入れる。

use std::panic::{self, AssertUnwindSafe};
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::mpsc;
use std::sync::Arc;

use anyhow::Context;
use tracing_subscriber::EnvFilter;

mod host_detect;

const ENGINE_OBJECT_PATH: &str = "/org/freedesktop/IBus/Engine/Kotoha";
const KOTOHA_LOG_ENV: &str = "KOTOHA_LOG";
const KOTOHA_SYSTEM_DICT_PATH_ENV: &str = "KOTOHA_SYSTEM_DICT_PATH";
const KOTOHA_ALLOW_STUB_ENV: &str = "KOTOHA_ALLOW_STUB";

/// `EX_TEMPFAIL`(systemd / sysexits.h):再起動が妥当な一時失敗。
const EXIT_TEMPFAIL: u8 = 75;
/// 一般エラー。spec §9.1 で具体 exit code は未指定のため、anyhow 由来の
/// 起動失敗は通常の 1 を返す。
const EXIT_FAILURE: u8 = 1;

fn main() -> ExitCode {
    init_tracing();

    // spec §9.1 row 5: top-level catch_unwind。`AssertUnwindSafe` は
    // `run()` 内部で borrow / mutex を panic 越しに抱える設計でないことを
    // 引き受ける明示。
    let result = panic::catch_unwind(AssertUnwindSafe(run));

    match result {
        Ok(Ok(())) => ExitCode::SUCCESS,
        Ok(Err(e)) => {
            tracing::error!(error = ?e, "kotoha-bin failed");
            ExitCode::from(EXIT_FAILURE)
        }
        Err(panic_payload) => {
            let msg = panic_message(&panic_payload);
            tracing::error!(panic = msg, "kotoha-bin caught top-level panic");
            ExitCode::from(EXIT_TEMPFAIL)
        }
    }
}

/// `catch_unwind` payload から表示用 message を取り出す best-effort helper。
fn panic_message(payload: &Box<dyn std::any::Any + Send>) -> &str {
    if let Some(s) = payload.downcast_ref::<&'static str>() {
        s
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.as_str()
    } else {
        "(non-string panic payload)"
    }
}

fn run() -> anyhow::Result<()> {
    let host = host_detect::detect();
    tracing::info!(?host, "kotoha-bin starting");

    match host {
        host_detect::DetectedHost::IBus => run_ibus()?,
    }
    Ok(())
}

fn init_tracing() {
    // 未設定時は静かに `info` default。設定済 + parse 失敗時のみ eprintln! で
    // warning を出す(`try_from_env` は両者を `Err` で返すため、env::var で
    // 先に存在判定する)。
    let filter = match std::env::var(KOTOHA_LOG_ENV) {
        Ok(directive) => match EnvFilter::try_new(&directive) {
            Ok(f) => f,
            Err(e) => {
                eprintln!(
                    "warning: invalid {KOTOHA_LOG_ENV}={directive:?} ({e}); falling back to info"
                );
                EnvFilter::new("info")
            }
        },
        Err(_) => EnvFilter::new("info"),
    };
    tracing_subscriber::fmt().with_env_filter(filter).init();
}

/// `KOTOHA_ALLOW_STUB` env var が `1` / `true` / `yes` のいずれかを指す場合 true。
/// それ以外(未設定 / `0` 等)は false で、production 起動時は stub fallback を
/// 拒否(spec §9.3「空候補返却で終わる」を防ぐ)。
fn stub_fallback_allowed() -> bool {
    matches!(
        std::env::var(KOTOHA_ALLOW_STUB_ENV).as_deref(),
        Ok("1" | "true" | "yes")
    )
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
    let allow_stub = stub_fallback_allowed();
    let (ranker, ranker_backend): (Arc<dyn kotoha_engine_core::Ranker>, &'static str) =
        match build_hybrid_ranker(user_vocab_store.clone(), learning_store.clone()) {
            Ok(r) => {
                tracing::info!("HybridRanker constructed (dict-only path; LLM is Phase 3-B B2+)");
                (r, "HybridRanker")
            }
            Err(e) if allow_stub => {
                tracing::error!(
                    error = ?e,
                    "failed to construct HybridRanker; falling back to StubRanker (KOTOHA_ALLOW_STUB=1)"
                );
                (Arc::new(StubRanker), "StubRanker")
            }
            Err(e) => {
                return Err(e).with_context(|| {
                    format!(
                        "HybridRanker construction failed and {KOTOHA_ALLOW_STUB_ENV} is not set; \
                         set {KOTOHA_SYSTEM_DICT_PATH_ENV} or {KOTOHA_ALLOW_STUB_ENV}=1 for development"
                    )
                });
            }
        };

    // 8. host bridge: session bus 接続。
    let (host_bridge, host_bridge_backend): (
        Box<dyn kotoha_engine_core::IMEHostBridge>,
        &'static str,
    ) = match kotoha_engine_ibus::IBusHostBridge::new(ENGINE_OBJECT_PATH) {
        Ok(b) => (Box::new(b), "IBusHostBridge"),
        Err(e) if allow_stub => {
            tracing::error!(
                error = ?e,
                "IBus session bus connect failed; using StubHostBridge (KOTOHA_ALLOW_STUB=1)"
            );
            (Box::new(StubHostBridge), "StubHostBridge")
        }
        Err(e) => {
            return Err(anyhow::Error::new(e)).with_context(|| {
                format!("IBusHostBridge connect failed and {KOTOHA_ALLOW_STUB_ENV} is not set")
            });
        }
    };

    // 9. engine(spec §9.1 row 5: spawn 失敗は Result 経由 propagate)
    let engine = kotoha_engine_core::engine::KotohaEngine::new(host_bridge, ranker, learning_store)
        .context("spawn ranker worker thread")?;

    // 10. dispatcher (event loop stub)
    let _dispatcher = kotoha_engine_ibus::IBusEventDispatcher::new(engine);

    tracing::info!(
        ranker_backend,
        host_bridge_backend,
        "kotoha engine wired up; event loop deferred to Phase 3-B B3"
    );
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

/// 暫定 stub ranker(`KOTOHA_ALLOW_STUB=1` 時のみ fallback として利用)。
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

/// 暫定 stub host bridge(`KOTOHA_ALLOW_STUB=1` 時のみ fallback として利用)。
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
