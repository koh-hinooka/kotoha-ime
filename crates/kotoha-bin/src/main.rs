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
#[cfg(feature = "dev-stubs")]
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
    install_panic_hook();

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
            tracing::error!(panic = %msg, "kotoha-bin caught top-level panic");
            ExitCode::from(EXIT_TEMPFAIL)
        }
    }
}

/// `catch_unwind` payload から表示用 message を best-effort で抽出する。
///
/// B0g #148 / 第 2 回 review C4: 旧 impl は `&'static str` / `String` のみ
/// downcast していたため、`panic_any(anyhow::Error)` 等の payload が
/// `(non-string panic payload)` で消失していた。本版では:
///
/// - `&'static str` / `String` を最優先で抽出
/// - `anyhow::Error` 経由の `panic_any` を debug 形式で展開
/// - いずれにも該当しない場合は payload type 名(`std::any::Any::type_id`
///   の Debug 表現)を含めて返す
///
/// 加えて、本関数で payload を取り損ねた場合でも `install_panic_hook` で
/// 登録された hook が **panic 発生時の location + backtrace を `tracing::error!`
/// に残す**(本関数の前段で観測される)ため、root cause 探索が継続可能。
fn panic_message(payload: &Box<dyn std::any::Any + Send>) -> String {
    if let Some(s) = payload.downcast_ref::<&'static str>() {
        return (*s).to_string();
    }
    if let Some(s) = payload.downcast_ref::<String>() {
        return s.clone();
    }
    if let Some(e) = payload.downcast_ref::<anyhow::Error>() {
        return format!("anyhow: {e:?}");
    }
    format!(
        "(non-string panic payload, type_id={:?})",
        (**payload).type_id()
    )
}

/// process global panic hook。`tracing` が初期化された後に呼ぶこと。
///
/// B0g #148 / C4: hook を登録することで、`catch_unwind` の payload type に
/// 依存せず **panic location + payload 表示** が確実に `tracing::error!` に
/// 流れる。`catch_unwind` 経由の `tracing::error!(panic = ..., ...)` と
/// 重複するが、hook の方が source file / line number を持つため debug 価値
/// が高い。
fn install_panic_hook() {
    // Default hook も呼んで stderr 上の human-readable backtrace を残す。
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        // tracing への構造化転送(systemd journal / log aggregator 想定)
        //
        // self-review #1:同一 panic に対して本 hook と `catch_unwind` 経路の
        // 両方で `tracing::error!` が出る(unwind 開始前 / 後で 2 回)。後段
        // 集計で重複扱いするため `source = "panic_hook"` で識別子を付与する。
        // catch_unwind 経路側は別 message なので grep で区別可能だが、本 field
        // を併用すると alert duplication 抑制が容易。
        tracing::error!(
            source = "panic_hook",
            thread = ?std::thread::current().name(),
            location = ?info.location(),
            payload = %info,
            "panic hook captured panic"
        );
        // stderr へ default の human-readable trace も流す
        default_hook(info);
    }));
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
///
/// B0h-e (#149 / #159):`dev-stubs` feature が OFF の場合(release default)は
/// 常に `false` を返す。env var が誤設定されても fallback path は compile されず、
/// production binary は stub に到達不能となる(spec §11 凍結)。
#[cfg(feature = "dev-stubs")]
fn stub_fallback_allowed() -> bool {
    matches!(
        std::env::var(KOTOHA_ALLOW_STUB_ENV).as_deref(),
        Ok("1" | "true" | "yes")
    )
}

#[cfg(not(feature = "dev-stubs"))]
fn stub_fallback_allowed() -> bool {
    // production / release default: stub fallback path は compile されない。
    // `KOTOHA_ALLOW_STUB` 環境変数の値は無視される。
    false
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
    //   adapter wrapper 経由で kotoha-engine-core の domain port
    //   (`UserVocabLookup` / `LearningLookup` / `LearningRecorder`)に変換する
    //   (B0h-a / C3 hexagonal driven port 反転、ISSUE #149 / #153)。
    let user_vocab_port = kotoha_engine_adapter::arc_sqlite_user_vocab(user_vocab_store.clone());
    let (learning_recorder, learning_lookup) =
        kotoha_engine_adapter::arc_sqlite_learning_cache(learning_store.clone());
    let allow_stub = stub_fallback_allowed();
    // `dev-stubs` OFF では下記 `Err(e) if allow_stub =>` arm 自体が cfg で
    // strip され、guard expression も含めて消えるため `allow_stub` は未参照
    // 変数となる。`stub_fallback_allowed()` の env-var 読み(将来の logging
    // hook 余地を含む)は production / dev で同 path を保持したいので変数自体
    // は維持し、`let _` で discard して `-D warnings` clippy を黙らせる。
    let _ = allow_stub;
    let (ranker, ranker_backend): (Arc<dyn kotoha_engine_core::Ranker>, &'static str) =
        match build_hybrid_ranker(user_vocab_port.clone(), learning_lookup.clone()) {
            Ok(r) => {
                tracing::info!("HybridRanker constructed (dict-only path; LLM is Phase 3-B B2+)");
                (r, "HybridRanker")
            }
            #[cfg(feature = "dev-stubs")]
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
        #[cfg(feature = "dev-stubs")]
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

    // 9. reactor (Phase 3-B B0h-f + B3 / ADR 0020):4-thread topology の core。
    //    main は `ReactorHandles` を保持し、bridge_tx / worker_tx を各 thread に
    //    move、shutdown_tx を SIGTERM/SIGINT handler 経由で発火させる。
    let kotoha_engine_reactor_linux::ReactorHandles {
        reactor,
        bridge_tx: _bridge_tx,
        worker_tx,
        shutdown_tx: _shutdown_tx,
    } = kotoha_engine_reactor_linux::start();

    // 10. engine(spec §9.1 row 5: spawn 失敗は Result 経由 propagate)
    //     adapter 経由で `Arc<dyn LearningRecorder>` を engine に注入し、
    //     worker thread が生成する `Event::WorkerOutput` の送信先 (`worker_tx`) を渡す。
    let engine = kotoha_engine_core::engine::KotohaEngine::new(
        host_bridge,
        ranker,
        learning_recorder,
        worker_tx,
    )
    .context("spawn ranker worker thread")?;

    // 11. dispatcher / engine-loop / dbus-listener の thread 起動は Phase D / E で
    //     完成させる(本 commit は Phase C scope のため、B3 listener / engine_loop
    //     の wiring は未実施)。fail-loud で起動失敗を user に通知する。
    let _ = engine; // engine は thread 移動前は drop すると worker が exit するため hold。
    let _ = reactor;

    tracing::error!(
        ranker_backend,
        host_bridge_backend,
        "kotoha engine + reactor wired up but the IBus event loop is not yet implemented \
         (Phase 3-B B3 / ISSUE #136); refusing to silently exit"
    );
    anyhow::bail!(
        "IBus event loop not yet implemented (tracked in Phase 3-B B3 / ISSUE #136); \
         kotoha-bin cannot serve as an IME yet"
    );
}

/// `HybridRanker` を構築する production helper。
///
/// SudachiDict-core を env var `KOTOHA_SYSTEM_DICT_PATH` から読み込み、
/// 失敗時は `Err` で fallback path に return する。LLM backend は Phase 3-B
/// B2+ で feature gate 付きで追加する。
fn build_hybrid_ranker(
    user_vocab: Arc<dyn kotoha_engine_core::learning_port::UserVocabLookup>,
    learning_cache: Arc<dyn kotoha_engine_core::learning_port::LearningLookup>,
) -> anyhow::Result<Arc<dyn kotoha_engine_core::Ranker>> {
    let dict_path: PathBuf = std::env::var(KOTOHA_SYSTEM_DICT_PATH_ENV)
        .map(PathBuf::from)
        .with_context(|| {
            format!("env var {KOTOHA_SYSTEM_DICT_PATH_ENV} is required for HybridRanker")
        })?;
    let sudachi = kotoha_core::dict::load_morphological_engine(&dict_path)
        .with_context(|| format!("load SudachiDict from {}", dict_path.display()))?;
    let ranker = kotoha_ranker_hybrid::HybridRanker::new(sudachi, user_vocab, learning_cache);
    Ok(Arc::new(ranker))
}

/// 暫定 stub ranker(`KOTOHA_ALLOW_STUB=1` 時のみ fallback として利用)。
///
/// B0h-e (#149 / #159):`dev-stubs` feature が OFF(default / release)では
/// 本 struct と impl が compile されず、release binary には link されない。
#[cfg(feature = "dev-stubs")]
struct StubRanker;

#[cfg(feature = "dev-stubs")]
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
///
/// B0g-b #148 / I6: `text` 引数は user 入力(password 含む可能性)。`tracing::trace!`
/// に流すと `KOTOHA_LOG=trace` 設定時に systemd journal / log aggregator へ
/// 平文流出する。dev fallback path とはいえ user の手元で trace を有効化する
/// ケース(debug session 中の log tail 等)を考慮し、`text_len` のみ記録する。
///
/// B0h-e (#149 / #159):`dev-stubs` feature gate を追加。`StubRanker` と同方針。
#[cfg(feature = "dev-stubs")]
struct StubHostBridge;

#[cfg(feature = "dev-stubs")]
impl kotoha_engine_core::IMEHostBridge for StubHostBridge {
    fn update_preedit(&self, text: &str, cursor: usize, visible: bool) {
        tracing::trace!(
            text_len = text.chars().count(),
            cursor,
            visible,
            "stub update_preedit"
        );
    }
    fn commit_text(&self, text: &str) {
        tracing::trace!(text_len = text.chars().count(), "stub commit_text");
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
