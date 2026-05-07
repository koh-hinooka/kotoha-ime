//! engine 内部型 — `RankRequest`。
//!
//! Phase 3-A spec §7.1 で定義された worker 経路の plumbing 型。crate 外には
//! export しない(`pub(crate)`)。
//!
//! Phase 3-B B0h-f + B3 (ADR 0020) で旧 `EngineEvent` enum は削除された。
//! worker → engine-loop 経路は `Sender<Event>::send(Event::WorkerOutput { .. })`
//! で直接送る形に統合され(`crate::reactor::Event` / `WorkerPayload` 参照)、
//! 中間 enum を経由しない。

use std::sync::Arc;

use crate::cancel::StdCancellationToken;
use crate::ranker::{ConversionContext, Ranker};
use crate::request_id::RequestId;

/// engine 主 thread から worker thread へ送る 1 RankRequest。
pub(crate) struct RankRequest {
    pub(crate) request_id: RequestId,
    pub(crate) kana: String,
    pub(crate) ctx: ConversionContext,
    pub(crate) cancel_token: Arc<StdCancellationToken>,
    pub(crate) ranker: Arc<dyn Ranker>,
}

impl RankRequest {
    /// `cancel_token` を `Arc<dyn CancellationToken>` に widen する。
    pub(crate) fn cancel_dyn(&self) -> Arc<dyn crate::cancel::CancellationToken> {
        self.cancel_token.clone()
    }
}
