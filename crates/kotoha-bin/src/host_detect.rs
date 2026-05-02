//! Host detection logic — Phase 3-A 初期は IBus 固定。
//!
//! Phase 4 fcitx5 adapter 増設時に env var(`XDG_CURRENT_DESKTOP` /
//! `IM_MODULE` 等)+ D-Bus name 確認で分岐する(spec §3.2 / §13 Open Q 8)。

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetectedHost {
    IBus,
    // Phase 4: Fcitx5,
}

/// 現在 host を検出する。Phase 3-A 初期は常に `IBus` を返す。
pub fn detect() -> DetectedHost {
    DetectedHost::IBus
}
