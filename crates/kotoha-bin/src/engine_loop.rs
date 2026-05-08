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
/// 戻り値が `anyhow::Result` のままなのは、main thread の `join().map_err`
/// chain と `?` で合流させるため。本関数自身は内部で error を伝播しない。
pub(crate) fn run<R: EventReactor>(mut engine: KotohaEngine, reactor: R) -> anyhow::Result<()> {
    loop {
        match reactor.recv() {
            Ok(Event::IBusKey { event, respond }) => {
                let result = engine.process_key_event(event);
                // listener が timeout した場合 send Err は ignore
                // (= keystroke は app に forward 済、spec §5.2 / p3-b-ibus-listener.md)
                let _ = respond.send(result);
            }
            Ok(Event::IBusReset(kind)) => match kind {
                IBusResetKind::FocusOut => engine.focus_out(),
                IBusResetKind::Reset => engine.reset(),
                // Phase 3-B B0h-f rev3 (ADR 0020) review fix: Disable は
                // `engine.disable()` (`enabled = false`) が正しい mapping。
                // 旧コードは reset() に倒していたが、IBus daemon 側 disable
                // signal は「IME を OFF にする」意味で、reset と semantics が違う。
                IBusResetKind::Disable => engine.disable(),
                // `IBusResetKind` は `#[non_exhaustive]`。将来 variant 追加時は
                // 個別 mapping を本 match で必ず追加すること。安全側 fallback として
                // disable() に倒し、誤動作よりも IME OFF を選ぶ。
                _ => {
                    tracing::warn!(?kind, "unknown IBusResetKind, falling back to disable()");
                    engine.disable();
                }
            },
            Ok(Event::WorkerOutput {
                request_id,
                payload,
            }) => {
                engine.apply_candidate_update(request_id, payload);
            }
            Ok(Event::Shutdown) => {
                tracing::info!("engine-loop received Event::Shutdown, exiting");
                break Ok(());
            }
            // `Event` は `#[non_exhaustive]`。将来 variant 追加 (Phase 5
            // notification 等)時に本 arm が拾った場合は **error** log + 続行で、
            // metric / log filter 経路で必ず拾える severity に揃える(spec §9.3
            // silent failure 禁止規約)。
            Ok(other) => {
                tracing::error!(
                    error_id = "engine_loop.unhandled_event_variant",
                    ?other,
                    "engine-loop received unknown Event variant; this is a spec drift signal \
                     (Event is #[non_exhaustive] and a new variant landed without engine_loop \
                     update)"
                );
            }
            Err(_) => {
                // 全 Sender drop = 自然な shutdown(ReactorHandles drop / 各 thread exit)。
                tracing::info!("engine-loop reactor disconnected, exiting");
                break Ok(());
            }
        }
    }
}
