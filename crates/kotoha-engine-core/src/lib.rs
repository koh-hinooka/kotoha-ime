//! `kotoha-engine-core`: Kotoha IME engine domain core (host-agnostic).
//!
//! 本 crate は Phase 3-A spec §3.1 で凍結された engine domain core layer に対応する。
//! IBus / fcitx5 / 将来の input-method protocol を含む host adapter から
//! 直接依存される一方、本 crate は host 層の identifier(`ibus` / `zbus` /
//! `fcitx5` 等)に依存しない。Hexagonal Architecture の core 配置である。
//!
//! 本 crate の主要 trait / types:
//!
//! - [`ranker::Ranker`] — 候補生成 trait(P2-D で `HybridRanker` として impl)
//! - [`ranker::ConversionContext`] / [`ranker::ConversionMode`] — Ranker 入力 context
//! - [`ranker::CandidateUpdate`] — 候補差分通知 enum
//! - [`cancel::CancellationToken`] — cancel signal trait
//! - [`cancel::StdCancellationToken`] — std::sync ベース impl
//!
//! 本 crate は Phase 3-A engine 本体(`KotohaEngine` 状態機械、`IMEEngine` /
//! `IMEHostBridge` trait、`RankerWorker`)を含まない。それらは Phase 3-A 本番
//! 実装段階で本 crate に追加される。

pub mod cancel;
pub mod ranker;

pub use cancel::{CancellationToken, StdCancellationToken};
pub use ranker::{
    CandidateUpdate, ConversionContext, ConversionMode, Ranker, RankerError, RankerOutput,
};
