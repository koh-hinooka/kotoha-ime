//! `IMEEngine` driving port — host(adapter)から engine への呼び出し API。
//!
//! Phase 3-A spec §4.1 で凍結。`Send` のみ要求(`Sync` は不要、event loop は
//! 単一 thread から engine を呼ぶ前提、内部並行性は `RankerWorker` thread と
//! channel で扱う、spec §4.1 Send + ! Sync 節)。

use crate::key_event::{KeyEvent, KeyEventResult};

/// IME host(adapter)から engine に呼び出される API。
///
/// # Lifecycle
///
/// 1. host が `enable()` を呼ぶ → engine は active 状態
/// 2. `focus_in()` でフォーカス取得通知
/// 3. `process_key_event()` で keystroke を渡す(0 回以上)
/// 4. `focus_out()` でフォーカス喪失通知
/// 5. `disable()` で engine inactive へ
///
/// # Invariants
///
/// - `enable()` 前 / `disable()` 後の `process_key_event()` は no-op + `Forwarded` 返却
/// - `focus_out()` 後は engine 状態が必ず Idle(active_request cancel + preedit clear)
pub trait IMEEngine: Send {
    /// 1 keystroke を処理し、消費 / forward を返す。
    ///
    /// # Preconditions
    ///
    /// - engine が `enable()` 後 / `disable()` 前
    /// - `focus_in()` 後 / `focus_out()` 前
    ///
    /// # Postconditions
    ///
    /// - `Consumed` を返した場合、[`crate::host_bridge::IMEHostBridge::update_preedit`] /
    ///   `update_candidates` / `commit_text` のいずれかが engine 内部で
    ///   呼び出された(0 回以上)
    /// - panic-free を保証(panic は top-level catch_unwind で reset 経由、
    ///   spec §9.1)
    fn process_key_event(&mut self, key: KeyEvent) -> KeyEventResult;

    /// application フォーカス取得通知(host から)。
    fn focus_in(&mut self);

    /// application フォーカス喪失通知(host から)。
    ///
    /// # Postconditions
    ///
    /// - active_request の cancel_token を fire
    /// - preedit / commit_history / 候補 window を clear
    /// - engine 状態 → Idle
    fn focus_out(&mut self);

    /// engine 内部状態を強制 reset(host からの強制 reset、`focus_out` と同等)。
    fn reset(&mut self);

    /// engine を active 状態に遷移させる(host が IME を有効化)。
    fn enable(&mut self);

    /// engine を inactive 状態に遷移させる(host が IME を無効化)。
    fn disable(&mut self);
}
