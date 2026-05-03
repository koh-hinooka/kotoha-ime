//! `CandidateBuffer` — ranker から到着した候補列 + 現在 highlight 位置の SRP-pure
//! value 単位。
//!
//! Phase 3-B B0h-c-i (ISSUE #149 / #161) で `KotohaEngine` から抽出された 2 field
//! (`candidates: Vec<Candidate>` + `highlight_idx: usize`)を 1 sub-struct に
//! 集約する。本 sub-struct の責務は **「engine 内部に保持する候補列(filter 適用後)
//! と、navigation で動かす highlight 位置を 1 単位として保持し、clear / index 維持
//! の API を提供する」** こと。
//!
//! 本 PR (B0h-c-i) では既存呼び出し側との diff を最小化するため、`items` と
//! `highlight` は依然 `pub(crate)` で直接 access する。method 経由への抽象化は
//! B0h-c-ii / -iii で `transitions.rs` の free function を sub-struct method 化
//! する際に併せて進める。

use kotoha_core::Candidate;

/// 候補列(`items`)と現在 highlight 位置(`highlight`)を 1 単位として持つ
/// value 型。
///
/// # Invariants
///
/// - `items.is_empty()` ⇒ `highlight == 0`(空 buffer の highlight は 0 に
///   固定する。`Remove` 経路は `truncate_highlight_to_len` で維持する)。
/// - `!items.is_empty()` ⇒ `highlight < items.len()`(navigation / Remove で
///   保たれる)。
pub(crate) struct CandidateBuffer {
    /// engine 内部に保持する候補列(filter / sanitize 適用後)。
    /// `Replace` で全置換、`Append` で末尾追加、`Remove` で範囲 drain、
    /// `Clear` で空にする。
    pub(crate) items: Vec<Candidate>,
    /// 現在 highlight 中の候補 index(0-based)。空 buffer の場合は 0、
    /// それ以外では `0 <= highlight < items.len()`。
    pub(crate) highlight: usize,
}

impl CandidateBuffer {
    /// 空の buffer を構築する。
    ///
    /// # Postconditions
    ///
    /// - `items.is_empty()`
    /// - `highlight == 0`
    pub(crate) fn new() -> Self {
        Self {
            items: Vec::new(),
            highlight: 0,
        }
    }

    /// `items` を空にし、`highlight` を 0 にリセットする。
    ///
    /// `focus_out` / `reset` / `disable` / `handle_return` (commit 後) /
    /// `handle_escape` (CandidatesShown path) / `handle_typing` の
    /// CandidatesShown re-entry / `dispatch_rank_request` の prior-clear /
    /// `apply_candidate_update::Clear` 等で呼ばれる。
    ///
    /// # Postconditions
    ///
    /// - `items.is_empty()`
    /// - `highlight == 0`
    pub(crate) fn clear(&mut self) {
        self.items.clear();
        self.highlight = 0;
    }
}
