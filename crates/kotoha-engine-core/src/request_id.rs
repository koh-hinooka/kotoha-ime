//! `RequestId` — engine ⇄ ranker-worker 経路で使う request 識別子の newtype。
//!
//! 旧 bare `u64` は user ID / candidate index などの他の数値と type system 上
//! 区別できず、誤って swap してもコンパイルが通る surface だった。本 newtype
//! で「これは ranker request の識別子である」という意図を型に昇格させる。
//!
//! # 採番セマンティクス
//!
//! - 単調増加(`u64::wrapping_add(1)`)。`u64::MAX` で 0 へ折り返す。実機の
//!   IME 用途では 1 keystroke / 1 採番として 64 bit 空間を消費し切るには
//!   およそ 10^11 年要するため、wrap 衝突の現実的リスクは無い
//! - `KotohaEngine` の `request_id_seed` は engine-loop thread が単独所有する
//!   ため、`&mut self` 経路で `next()` を呼ぶ
//! - 将来 atomic seed が必要になった場合のために `next_from_atomic` を提供する
//!   が、現状の単一 thread モデルでは使用しない

use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};

/// engine が発行する ranker request の単調増加 ID。
///
/// # Invariants
///
/// - `RequestId(0)` は `Default::default()` の値であり、未採番状態を示す
///   sentinel として使ってよい(spec §7.5)。最初の採番は `next()` で
///   `RequestId(1)` から始まる
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RequestId(u64);

impl RequestId {
    /// 未採番状態を示す sentinel(`RequestId(0)`)。
    pub const ZERO: Self = Self(0);

    /// raw `u64` から構築する(test fixture / migration path 用)。
    ///
    /// production code では `KotohaEngine::next_request_id()` 経由で取得する
    /// のが原則であり、本 constructor は test と外部 ID の取り込みに限定する。
    #[must_use]
    pub const fn new(raw: u64) -> Self {
        Self(raw)
    }

    /// 内部 `u64` を取り出す(log / metric / FFI 境界で必要時のみ)。
    #[must_use]
    pub const fn as_u64(self) -> u64 {
        self.0
    }

    /// 次 ID を返す(`u64::wrapping_add(1)`)。
    ///
    /// 単一 thread 採番(`KotohaEngine::next_request_id`)で利用する。
    /// 複数 thread から共有する seed に対しては `next_from_atomic` を使うこと。
    #[must_use]
    pub const fn next(self) -> Self {
        Self(self.0.wrapping_add(1))
    }

    /// `AtomicU64` を seed として次 ID を採番する(thread-safe)。
    ///
    /// 戻り値は採番後の値 (= `seed.fetch_add(1) + 1`)。`Ordering::Relaxed`
    /// で十分(因果関係は engine-loop 側 channel send/recv で確立される)。
    #[must_use]
    pub fn next_from_atomic(seed: &AtomicU64) -> Self {
        Self(seed.fetch_add(1, Ordering::Relaxed).wrapping_add(1))
    }
}

impl fmt::Display for RequestId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<RequestId> for u64 {
    fn from(id: RequestId) -> Self {
        id.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_zero() {
        assert_eq!(RequestId::default(), RequestId::ZERO);
        assert_eq!(RequestId::default().as_u64(), 0);
    }

    #[test]
    fn next_is_monotonic_with_wrapping() {
        let mut id = RequestId::ZERO;
        id = id.next();
        assert_eq!(id.as_u64(), 1);
        id = id.next();
        assert_eq!(id.as_u64(), 2);

        let max = RequestId::new(u64::MAX);
        assert_eq!(max.next(), RequestId::ZERO);
    }

    #[test]
    fn next_from_atomic_returns_post_increment_value() {
        let seed = AtomicU64::new(0);
        let first = RequestId::next_from_atomic(&seed);
        let second = RequestId::next_from_atomic(&seed);
        assert_eq!(first.as_u64(), 1);
        assert_eq!(second.as_u64(), 2);
    }

    #[test]
    fn display_renders_inner_u64() {
        assert_eq!(format!("{}", RequestId::new(42)), "42");
    }

    #[test]
    fn into_u64_round_trip() {
        let id = RequestId::new(7);
        let raw: u64 = id.into();
        assert_eq!(raw, 7);
    }
}
