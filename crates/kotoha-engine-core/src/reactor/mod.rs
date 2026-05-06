//! `EventReactor` — multi-source event multiplexer の OS 非依存 port。
//!
//! 本 module は ADR 0020 §採択 Q3 の「engine-core 側 trait のみ」境界に対応する。
//! Linux 実装は `kotoha-engine-reactor-linux` crate に置く。本 module には
//! `select!` macro / OS 依存 primitive を含めない。

pub mod event;

#[cfg(any(test, feature = "test-helpers"))]
pub mod mock;

pub use event::{Event, IBusResetKind, WorkerPayload};

use std::time::Duration;

/// engine-loop thread が単一 thread で multiplex する event 受信機構の port。
///
/// # Invariants
///
/// - 実装は `Send + 'static`(engine-loop thread に move される)
/// - `recv` は `Event::Shutdown` または全 Sender drop を観測したら以降
///   `Err(crossbeam_channel::RecvError)` を返してよい
///
/// # Examples
///
/// ```ignore
/// use kotoha_engine_core::reactor::{Event, EventReactor};
///
/// fn run<R: EventReactor>(reactor: &R) -> Result<(), crossbeam_channel::RecvError> {
///     loop {
///         match reactor.recv()? {
///             Event::Shutdown => break Ok(()),
///             _other => { /* dispatch */ }
///         }
///     }
/// }
/// ```
pub trait EventReactor: Send + 'static {
    /// 単一 event を blocking 受信する。
    ///
    /// # Errors
    ///
    /// - 全 Sender が drop された場合 `crossbeam_channel::RecvError`
    fn recv(&self) -> Result<Event, crossbeam_channel::RecvError>;

    /// timeout 付き受信。`coalescing window`(spec §7.3)等で利用する。
    ///
    /// # Errors
    ///
    /// - timeout 経過: `crossbeam_channel::RecvTimeoutError::Timeout`
    /// - 全 Sender drop: `crossbeam_channel::RecvTimeoutError::Disconnected`
    fn recv_timeout(&self, timeout: Duration)
        -> Result<Event, crossbeam_channel::RecvTimeoutError>;
}
