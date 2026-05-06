//! engine-loop thread の main loop。
//!
//! ADR 0020 §採択 Q4 の 4-thread topology のうち `kotoha-engine-loop` thread を担う。
//! `KotohaEngine` を `move` で単独所有し、lock を一切使わずに状態を更新する。
//!
//! # Architecture
//!
//! ```text
//!   [dbus-listener thread]   [ranker-worker thread]
//!         │                          │
//!         │ Event::IBusKey/Reset     │ Event::WorkerOutput
//!         ▼                          ▼
//!         (bridge channel)    (worker channel)
//!                       \    /
//!                        ▼ ▼
//!               EventReactor::recv()
//!                          │
//!                          ▼
//!                   engine-loop thread
//!                  (本 module の `run`)
//! ```

use kotoha_engine_core::engine::KotohaEngine;
use kotoha_engine_core::ime_engine::IMEEngine;
use kotoha_engine_core::reactor::{Event, EventReactor, IBusResetKind};

/// engine-loop の main loop。本関数は engine-loop thread の `spawn` 時に呼ばれる。
///
/// # Preconditions
///
/// - `engine` は本関数が単独所有する(他 thread からの参照は不在)
/// - `reactor` の `EventReactor::recv()` は engine-loop thread からのみ呼ばれる
///
/// # Postconditions
///
/// - `Event::Shutdown` を観測する、または全 `Sender<Event>` が drop されると
///   `Ok(())` で return する
///
/// # Errors
///
/// - `KotohaEngine::apply_candidate_update` が将来 `KotohaEngineError` を
///   非 placeholder variant で返した場合に伝播する。現状は placeholder のみで
///   実 variant は無いため `Err` 経路は到達しないが、API 表面を保つために
///   `anyhow::Result` で受け取る。
pub(crate) fn run<R: EventReactor>(mut engine: KotohaEngine, reactor: R) -> anyhow::Result<()> {
    loop {
        match reactor.recv() {
            Ok(Event::IBusKey(key)) => {
                let _result = engine.process_key_event(key);
            }
            Ok(Event::IBusReset(kind)) => match kind {
                IBusResetKind::FocusOut => engine.focus_out(),
                IBusResetKind::Reset | IBusResetKind::Disable => engine.reset(),
                // `IBusResetKind` は `#[non_exhaustive]`。将来 variant が増えた
                // 際は `engine.reset()` 相当の保守的な処理に倒す。production で
                // 未知 variant が観測されたら observability 経路に拾って後追いする。
                _ => {
                    tracing::warn!(?kind, "unknown IBusResetKind, falling back to reset()");
                    engine.reset();
                }
            },
            Ok(Event::WorkerOutput {
                request_id,
                payload,
            }) => {
                if let Err(e) = engine.apply_candidate_update(request_id, payload) {
                    return Err(anyhow::Error::new(e));
                }
            }
            Ok(Event::Shutdown) => {
                tracing::info!("engine-loop received Event::Shutdown, exiting");
                break Ok(());
            }
            // `Event` は `#[non_exhaustive]`。将来 variant 追加 (Phase 5
            // notification 等)で本 arm が拾った場合は warn log + 無視で継続する。
            Ok(other) => {
                tracing::warn!(?other, "engine-loop ignoring unknown Event variant");
            }
            Err(_) => {
                // 全 Sender drop = 自然な shutdown(ReactorHandles drop / 各 thread exit)。
                tracing::info!("engine-loop reactor disconnected, exiting");
                break Ok(());
            }
        }
    }
}
