//! `Event` sum 型と関連 enum 定義。
//!
//! 本 module は engine-loop thread が `EventReactor::recv()` で受け取る全 event
//! source(D-Bus / worker / shutdown / 将来の notification)を単一 sum 型に集約する。
//! 詳細は spec `docs/specs/_uncategorized/p3-a-ibus-engine.md` §7.1 / ADR 0020 §採択 Q4。

use crate::key_event::KeyEvent;
use crate::ranker::CandidateUpdate;
use crate::request_id::RequestId;

/// engine-loop thread が単一 thread で multiplex する全 event source の sum 型。
///
/// # Invariants
///
/// - 全 variant は `Send + 'static`(crossbeam channel 越しに送るため)
/// - `#[non_exhaustive]` で将来の variant 追加に備える(Phase 5 notification 等)
///
/// # 設計根拠
///
/// 旧 `engine::event::EngineEvent`(`Candidates` / `WorkerError`)は本 enum の
/// `WorkerOutput` variant に統合される(`From<EngineEvent> for Event` 経由で
/// worker → engine-loop 経路で変換)。`IBusKey` / `IBusReset` は B3 listener
/// thread からの新規 source、`Shutdown` は main thread からの終端 sentinel。
#[non_exhaustive]
#[derive(Debug)]
pub enum Event {
    /// IBus session bus で受信した key event。
    /// `kotoha-dbus-listener` thread が `Sender<Event>::send` で engine-loop に届ける。
    IBusKey(KeyEvent),

    /// IBus focus_out / reset / disable 系の signal。
    IBusReset(IBusResetKind),

    /// ranker-worker thread が生成した候補 update + worker error。
    /// 旧 `EngineEvent::Candidates` / `EngineEvent::WorkerError` を統合した形。
    WorkerOutput {
        request_id: RequestId,
        payload: WorkerPayload,
    },

    /// 全 thread に shutdown を通知する sentinel。
    ///
    /// # Postconditions
    ///
    /// - engine-loop は本 variant を観測した直後に loop から `Ok(())` で抜ける
    /// - 全 `Sender<Event>` が drop されても `Receiver::recv()` が
    ///   `Err(Disconnected)` を返すため、shutdown 経路は二重に冗長化されている
    Shutdown,
}

/// IBus からの focus_out / reset / disable 系 signal の種別。
///
/// `Reset` と `Disable` は engine 側の処理が同一だが、log と将来の振り分け余地
/// のため variant を分けて保持する。
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IBusResetKind {
    FocusOut,
    Reset,
    Disable,
}

/// `Event::WorkerOutput` の payload。
///
/// 旧 `engine::event::EngineEvent::Candidates` / `WorkerError` を統合した形。
#[non_exhaustive]
#[derive(Debug)]
pub enum WorkerPayload {
    /// ranker が生成した候補差分。
    Candidates(CandidateUpdate),

    /// worker 内部 panic / 致命 error 検出時に engine が再生成判断するための signal。
    Error(String),
}
