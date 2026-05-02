//! engine 内部型 — `RankRequest` / `EngineEvent`。
//!
//! Phase 3-A spec §7.1 で定義された worker 経路の plumbing 型。
//! crate 外には export しない(`pub(crate)`)。

use std::sync::Arc;

use crate::cancel::{CancellationToken, StdCancellationToken};
use crate::ranker::{CandidateUpdate, ConversionContext, Ranker};

/// engine 主 thread から worker thread へ送る 1 RankRequest。
pub(crate) struct RankRequest {
    pub(crate) request_id: u64,
    pub(crate) kana: String,
    pub(crate) ctx: ConversionContext,
    pub(crate) cancel_token: Arc<StdCancellationToken>,
    pub(crate) ranker: Arc<dyn Ranker>,
}

impl RankRequest {
    /// `cancel_token` を `Arc<dyn CancellationToken>` に widen する。
    pub(crate) fn cancel_dyn(&self) -> Arc<dyn CancellationToken> {
        self.cancel_token.clone()
    }
}

/// worker thread から engine 主 thread への通知 message。
#[derive(Debug)]
pub(crate) enum EngineEvent {
    Candidates {
        request_id: u64,
        update: CandidateUpdate,
    },
    /// worker 内部で異常検出時(Ranker::rank が `Err`)、engine が tracing で残す。
    WorkerError { request_id: u64, error: String },
}
