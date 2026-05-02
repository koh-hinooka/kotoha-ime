//! `Ranker` trait + `RankerError` + `RankerOutput`.
//!
//! Phase 3-A spec §4.3 で凍結された候補生成 trait。P2-D で `HybridRanker` として
//! 実装される。本 module は trait + types のみで、impl は `hybrid` sub-module
//! (P2-D Milestone 2 以降で追加される)。

mod context;
mod update;

pub use context::{ConversionContext, ConversionMode};
pub use update::CandidateUpdate;

use std::sync::mpsc;
use std::sync::Arc;

use crate::cancel::CancellationToken;

/// Ranker は `kana` 入力に対して候補一覧を生成し、`sink` channel 経由で
/// engine に逐次 push する。Phase 3-A spec §4.3 で凍結された contract。
///
/// # 動作モデル
///
/// - `rank()` は **同期に return**(channel 送信のみ)、heavy lifting は背後
///   thread / async runtime に dispatch される(impl 内部の自由)。
/// - `cancel.is_cancelled() == true` を検出したら以降の `sink.send()` を停止する。
/// - backend 個別の cancel propagation:
///   - SudachiDict / UserVocab / LearningCache: μs オーダーで完結のため cancel
///     check は不要(完了時に request_id mismatch なら engine 側で discard)。
///   - LLM: 10 token 毎に `cancel.is_cancelled()` を check、true なら inference 中断。
///
/// # Thread safety
///
/// `Send + Sync` を要求(engine の `RankerWorker` thread と engine 主 thread の
/// 双方から `Arc<dyn Ranker>` で参照される)。
pub trait Ranker: Send + Sync {
    /// 候補生成の起動。同期 return、heavy work は impl 内 thread に dispatch。
    ///
    /// # Errors
    ///
    /// - [`RankerError::Busy`]: 直前の rank 呼び出しが完了前で受け付け不可
    /// - [`RankerError::Internal`]: impl 内部の不可避エラー
    fn rank(
        &self,
        kana: &str,
        ctx: &ConversionContext,
        cancel: Arc<dyn CancellationToken>,
        sink: mpsc::Sender<RankerOutput>,
    ) -> Result<(), RankerError>;
}

/// Ranker から engine への通知 message。
#[derive(Debug, Clone)]
pub struct RankerOutput {
    /// engine 主 thread 側で active_request.id と mismatch なら discard する識別子。
    /// engine が `RankRequest` 発行時に採番した値を Ranker 内部で保持し送出する
    /// (Phase 3-A spec §7.5)。
    pub request_id: u64,
    /// 差分通知本体。
    pub update: CandidateUpdate,
}

/// Ranker error 列挙。
#[derive(thiserror::Error, Debug)]
#[non_exhaustive]
pub enum RankerError {
    #[error("ranker is busy")]
    Busy,
    #[error("ranker internal error: {0}")]
    Internal(String),
}
