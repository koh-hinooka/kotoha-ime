//! `PreeditBuffer` — kana surface + Romaji 変換器の SRP-pure value 単位。
//!
//! Phase 3-B B0h-c-i (ISSUE #149 / #161) で `KotohaEngine` から抽出された 2 field
//! (`current_preedit: String` + `romaji: RomajiConverter`)を 1 sub-struct に
//! 集約する。本 sub-struct の責務は **「IBus に通知される pre-edit 表示文字列」と、
//! 続く typing で push される RomajiConverter pending state を 1 単位として保持し、
//! reset / pop / push / size 観測の API を提供する** こと。
//!
//! 本 PR (B0h-c-i) では既存呼び出し側との diff を最小化するため、`current` と
//! `romaji` は依然 `pub(crate)` で直接 access する。method 経由への抽象化は
//! B0h-c-ii / -iii の `transitions.rs` 全 free function method 化と同時に進める。

use kotoha_core::romaji::RomajiConverter;

/// Pre-edit kana surface(`current`)と RomajiConverter pending state(`romaji`)を
/// 1 単位として持つ value 型。
///
/// # Invariants
///
/// - `current` は IBus 側 `update_preedit` で表示される literal kana surface
///   (filter / sanitize は engine boundary で別途実施される。本 buffer 自体に
///   sanitize 責務は無い)。
/// - `romaji` の pending state は次 typing で `push` され、`reset_pending` で
///   破棄できる。`current` と `romaji` の同期は呼び出し側(`transitions.rs`)が
///   各 keystroke で維持する(本 PR では method 抽象化を行わない)。
pub(crate) struct PreeditBuffer {
    /// IBus に通知される pre-edit 表示文字列。確定済 kana が末尾追加され、
    /// commit / focus_out / reset / disable で `clear` される。
    pub(crate) current: String,
    /// 次 typing 用の Romaji 変換器 pending state。`reset_pending` で
    /// pending を破棄し、`push` で 1 char ずつ ConvertStep を返す。
    pub(crate) romaji: RomajiConverter,
}

impl PreeditBuffer {
    /// 空の buffer を構築する。
    ///
    /// # Postconditions
    ///
    /// - `current.is_empty()`
    /// - `romaji` pending は `RomajiConverter::new()` 状態
    pub(crate) fn new() -> Self {
        Self {
            current: String::new(),
            romaji: RomajiConverter::new(),
        }
    }

    /// `current` をクリアし、Romaji pending state を破棄する。
    ///
    /// `focus_out` / `reset` / `disable` / `handle_return` (commit 後) /
    /// `handle_escape` (LiveConverting / CommitConverting path) で呼ばれる。
    ///
    /// # Postconditions
    ///
    /// - `current.is_empty()`
    /// - `romaji` pending state cleared
    pub(crate) fn clear(&mut self) {
        self.current.clear();
        self.romaji.reset_pending();
    }
}
