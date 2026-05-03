//! `Ranker` trait + `RankerError` + `RankerOutput`.
//!
//! Phase 3-A spec §4.3 で凍結された候補生成 trait。P2-D で `HybridRanker` として
//! 実装され、Phase 3-B B0h-b (ISSUE #149 / #155) で `kotoha-ranker-hybrid` 別
//! crate へ切り出された。本 module は domain core layer に属する trait と
//! 値型のみを公開する(impl は外部 crate に依存して提供される)。

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
///     check は不要。stale response の discard は engine 主 thread 側 `request_id`
///     照合(spec §7.5)で行う。Ranker 側は engine の `request_id` を知らない設計。
///   - LLM: 10 token 毎に `cancel.is_cancelled()` を check、true なら inference 中断。
///
/// # Thread safety
///
/// `Send + Sync` を要求(engine の `RankerWorker` thread と engine 主 thread の
/// 双方から `Arc<dyn Ranker>` で参照される)。
///
/// # Per-request channel invariant (Phase 3-B B0e)
///
/// `sink` は **request 毎に新規作成された** mpsc channel である(`RankerWorker`
/// が `worker_loop` 各 iteration で `(tx_ranker, rx_ranker) = mpsc::channel()`
/// を生成し、本 `sink` を Ranker に渡す)。本 channel に届く output はすべて
/// current request のものとして worker が受け取る。Ranker 側は出力を識別する
/// id を持たない(以前の `RankerOutput.request_id` field は撤去済、I10)。
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
///
/// # Phase 3-B B0e (ISSUE #140 / Important 10)
///
/// `request_id` field は撤去済。`Ranker::rank` の trait signature が engine
/// 側 `request_id` を引数で受けない設計のため、Ranker impl 側で stamp しても
/// engine 側で意味のある照合は不可能だった(B5 で worker レベルの id 照合を
/// 撤去した時点で `request_id` field は dead surface 化)。
/// Stale response の discard は engine 主 thread 側の `RankRequest` ベース
/// id 照合 + per-request channel 不変条件で十分に成立する(spec §7.5)。
#[derive(Debug, Clone)]
pub struct RankerOutput {
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
