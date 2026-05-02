//! Cancellation token abstraction for in-flight `RankRequest` propagation.
//!
//! Phase 3-A spec §4.4 で凍結された自作 trait。tokio_util::sync::CancellationToken と
//! 同形式の interface だが、tokio runtime に直接依存しない。Phase 3-A 初期は
//! [`StdCancellationToken`](std_token::StdCancellationToken) を std::sync ベース impl
//! として同梱、Phase 5/6 で tokio runtime を導入する場合は別 crate で
//! `TokioCancellationToken` adapter を実装し差し替え可能とする。

use std::future::Future;
use std::pin::Pin;

mod std_token;
pub use std_token::StdCancellationToken;

/// In-flight `RankRequest` の cancel signal を伝搬する抽象境界。
///
/// # Implementor 要件
///
/// - `cancel()` は idempotent(複数回呼ばれても 2 回目以降は no-op で副作用なし)
/// - `is_cancelled()` は `cancel()` 呼び出し後 `true` を永続的に返す
/// - `cancelled()` は `cancel()` 呼び出し後即時 ready な future を返す
/// - `Send + Sync` を要求(複数 thread から `is_cancelled()` を listen される)
pub trait CancellationToken: Send + Sync {
    /// Cancel 状態に遷移させる。
    fn cancel(&self);

    /// 現在の cancel 状態を返す。
    fn is_cancelled(&self) -> bool;

    /// Cancel 待機 future を返す。`cancel()` 後即時 ready。
    fn cancelled<'a>(&'a self) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>>;
}
