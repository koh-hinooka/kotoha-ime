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

/// `EventReactor::recv*` が返す OS 非依存の error 型。
///
/// `kotoha-engine-core` の port boundary では `crossbeam_channel::RecvError` /
/// `crossbeam_channel::RecvTimeoutError` を直接露出させない。Linux 実装は
/// crossbeam の error variant を本 enum へ map する責務を持つ
/// (`kotoha-engine-reactor-linux::LinuxReactor` impl 参照)。将来 macOS
/// (kqueue) / Windows (IOCP) 実装が追加されても、それぞれの native error を
/// 本 enum へ map することで trait 契約は不変に保たれる。
///
/// # Variants
///
/// - [`ReactorError::Disconnected`] — 全 Sender が drop され event 源が完全に
///   閉じた状態。`recv` / `recv_timeout` どちらも返しうる
/// - [`ReactorError::Timeout`] — `recv_timeout` の指定 duration 経過。`recv`
///   は本 variant を返さない(blocking 仕様のため)
#[derive(Debug, thiserror::Error, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ReactorError {
    /// 全 Sender drop による reactor 終了状態。
    #[error("reactor disconnected: all senders dropped")]
    Disconnected,
    /// `recv_timeout` の timeout 経過。
    #[error("reactor recv timed out")]
    Timeout,
}

/// engine-loop thread が単一 thread で multiplex する event 受信機構の port。
///
/// # Invariants
///
/// - 実装は `Send + 'static`(engine-loop thread に move される)
/// - `recv` は `Event::Shutdown` または全 Sender drop を観測したら以降
///   [`ReactorError::Disconnected`] を返してよい
///
/// # Examples
///
/// ```ignore
/// use kotoha_engine_core::reactor::{Event, EventReactor, ReactorError};
///
/// fn run<R: EventReactor>(reactor: &R) -> Result<(), ReactorError> {
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
    /// - [`ReactorError::Disconnected`] — 全 Sender が drop された場合
    ///
    /// 本 method は [`ReactorError::Timeout`] を返さない(blocking のため)。
    fn recv(&self) -> Result<Event, ReactorError>;

    /// timeout 付き受信。`coalescing window`(spec §7.3)等で利用する。
    ///
    /// # Errors
    ///
    /// - [`ReactorError::Timeout`] — 指定 duration 経過(event 不到達)
    /// - [`ReactorError::Disconnected`] — 全 Sender が drop された場合
    fn recv_timeout(&self, timeout: Duration) -> Result<Event, ReactorError>;
}
