//! `MockReactor` — engine-loop unit test 用の `EventReactor` 実装。
//!
//! PR #113 の `MockLearningCacheStore` 流儀(`feedback_mock_owns_invariants.md`)に
//! 従い、SQLite / zbus / OS primitive に一切依存せず queue ベースで動く。
//!
//! 本 mock は `cargo test` 自動 cfg もしくは `feature = "test-helpers"` で
//! exposes される(`reactor::mod.rs` 参照)。

use std::collections::VecDeque;
use std::sync::Mutex;
use std::time::Duration;

use crate::reactor::{Event, EventReactor, ReactorError};

/// queue ベースの `EventReactor` mock。
///
/// # Invariants
///
/// - queue が空かつ `closed = false` で `recv()` を呼ぶことは usage error。
///   real reactor とは異なり blocking しない(deterministic test 担保のため)。
///   `debug_assert!` で検出し、release では `Err(ReactorError::Disconnected)` を返す。
/// - `close()` 後の `recv()` / `recv_timeout()` は `ReactorError::Disconnected` を返す。
pub struct MockReactor {
    inner: Mutex<MockState>,
}

struct MockState {
    queue: VecDeque<Event>,
    closed: bool,
}

impl MockReactor {
    /// 新規 mock を生成する。
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(MockState {
                queue: VecDeque::new(),
                closed: false,
            }),
        }
    }

    /// queue 末尾に event を追加する。
    pub fn push(&self, ev: Event) {
        let mut s = self.lock();
        s.queue.push_back(ev);
    }

    /// 全 producer が drop された状態を simulate する。
    pub fn close(&self) {
        let mut s = self.lock();
        s.closed = true;
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, MockState> {
        // PR #111 の Mutex poison cascade 対処と同方針:test 用 mock であり
        // poison 時は test 自体を fail させるべきだが、recover して継続する。
        match self.inner.lock() {
            Ok(g) => g,
            Err(e) => e.into_inner(),
        }
    }
}

impl Default for MockReactor {
    fn default() -> Self {
        Self::new()
    }
}

impl EventReactor for MockReactor {
    fn recv(&self) -> Result<Event, ReactorError> {
        let mut s = self.lock();
        if let Some(ev) = s.queue.pop_front() {
            return Ok(ev);
        }
        if s.closed {
            return Err(ReactorError::Disconnected);
        }
        debug_assert!(
            false,
            "MockReactor::recv called on empty queue without close(); \
             test should push events or close before recv"
        );
        Err(ReactorError::Disconnected)
    }

    fn recv_timeout(&self, _timeout: Duration) -> Result<Event, ReactorError> {
        let mut s = self.lock();
        if let Some(ev) = s.queue.pop_front() {
            Ok(ev)
        } else if s.closed {
            Err(ReactorError::Disconnected)
        } else {
            Err(ReactorError::Timeout)
        }
    }
}
