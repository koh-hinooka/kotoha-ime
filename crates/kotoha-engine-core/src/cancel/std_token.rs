//! `StdCancellationToken` — std::sync ベースの `CancellationToken` impl。
//!
//! Phase 3-A spec §4.4 で凍結された自作 trait の Phase 3-A 初期 impl。
//! `Arc<AtomicBool>` + `Mutex<Vec<Waker>>` で cancel signal の永続化と async
//! future 待機を実装する。`Mutex` poison は kotoha-storage で確立した規約
//! (PR #111、`unwrap_or_else(PoisonError::into_inner)`)を流用する。
//!
//! 同期 thread block-wait は現状 trait に含まれない(`cancelled() -> Future` のみ)
//! ため `Condvar` は配置しない。将来 sync wait API を追加する場合に Condvar 復活と
//! `wait_blocking()` method を同時導入する。

use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::task::{Context, Poll, Waker};

use super::CancellationToken;

/// std::sync ベースの cancel token impl。`Arc` で clone 可能、複数 thread から共有できる。
#[derive(Clone)]
pub struct StdCancellationToken {
    inner: Arc<Inner>,
}

struct Inner {
    flag: AtomicBool,
    notify: Mutex<Vec<Waker>>,
}

impl StdCancellationToken {
    /// 新しい未 cancel な token を作成する。
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Inner {
                flag: AtomicBool::new(false),
                notify: Mutex::new(Vec::new()),
            }),
        }
    }
}

impl Default for StdCancellationToken {
    fn default() -> Self {
        Self::new()
    }
}

impl CancellationToken for StdCancellationToken {
    fn cancel(&self) {
        self.inner.flag.store(true, Ordering::SeqCst);
        // Future 経由で待機している waker を起こす
        let mut wakers = self
            .inner
            .notify
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        for waker in wakers.drain(..) {
            waker.wake();
        }
    }

    fn is_cancelled(&self) -> bool {
        self.inner.flag.load(Ordering::SeqCst)
    }

    fn cancelled<'a>(&'a self) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
        Box::pin(CancelledFuture {
            inner: self.inner.clone(),
        })
    }
}

struct CancelledFuture {
    inner: Arc<Inner>,
}

impl Future for CancelledFuture {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        if self.inner.flag.load(Ordering::SeqCst) {
            return Poll::Ready(());
        }
        let mut wakers = self
            .inner
            .notify
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        // double-check after taking the lock to avoid lost-wakeup
        if self.inner.flag.load(Ordering::SeqCst) {
            return Poll::Ready(());
        }
        wakers.push(cx.waker().clone());
        Poll::Pending
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;

    /// spec §4.4 不変条件: cancel() 前は is_cancelled() == false
    #[test]
    fn fresh_token_is_not_cancelled() {
        let t = StdCancellationToken::new();
        assert!(!t.is_cancelled());
    }

    /// spec §4.4 不変条件: cancel() 後は is_cancelled() == true (永続)
    #[test]
    fn cancel_persists() {
        let t = StdCancellationToken::new();
        t.cancel();
        assert!(t.is_cancelled());
        assert!(t.is_cancelled()); // 2 回目も true
    }

    /// spec §4.4 不変条件: cancel() は idempotent
    #[test]
    fn cancel_is_idempotent() {
        let t = StdCancellationToken::new();
        t.cancel();
        t.cancel(); // 2 回目以降 no-op
        assert!(t.is_cancelled());
    }

    /// 複数 thread から共有された clone は同一 cancel state を共有する
    #[test]
    fn clones_share_state() {
        let t1 = StdCancellationToken::new();
        let t2 = t1.clone();
        let handle = thread::spawn(move || {
            // 100ms 待ってから cancel(t1 thread が wait できる時間)
            thread::sleep(Duration::from_millis(100));
            t2.cancel();
        });
        // t1 で cancel を観測できる
        let start = std::time::Instant::now();
        while !t1.is_cancelled() {
            if start.elapsed() > Duration::from_secs(1) {
                panic!("cancel propagation timeout");
            }
            thread::sleep(Duration::from_millis(10));
        }
        handle.join().unwrap();
    }
}
