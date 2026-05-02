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
//! - [`ime_engine::IMEEngine`] — driving port(host → engine)
//! - [`host_bridge::IMEHostBridge`] — driven port(engine → host)
//! - [`key_event::KeyEvent`] / [`key_event::KeyEventResult`] / [`key_event::KeyModifiers`]
//!
//! 本 crate は Phase 3-A engine 本体(`KotohaEngine` 状態機械、`RankerWorker`)を
//! Milestone 2 / 3 で追加する。

pub mod cancel;
pub mod host_bridge;
pub mod ime_engine;
pub mod key_event;
pub mod ranker;

#[cfg(feature = "test-helpers")]
pub mod testing;

pub use cancel::{CancellationToken, StdCancellationToken};
pub use host_bridge::IMEHostBridge;
pub use ime_engine::IMEEngine;
pub use key_event::{KeyEvent, KeyEventResult, KeyModifiers};
pub use ranker::{
    CandidateUpdate, ConversionContext, ConversionMode, HybridRanker, Ranker, RankerError,
    RankerOutput,
};
