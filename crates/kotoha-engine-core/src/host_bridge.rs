//! `IMEHostBridge` driven port — engine から host(adapter)への呼び出し API。
//!
//! Phase 3-A spec §4.2 で凍結。`Send + Sync` を要求するのは engine 主 thread と
//! `RankerWorker` thread の双方から `Box<dyn IMEHostBridge>` を経由して呼ばれる
//! ため(spec §4.2 Send + Sync 節)。

use crate::CandidateUpdate;

/// engine から IME host(adapter)への呼び出し API。
///
/// # Implementor 要件
///
/// - `Send + Sync` を満たす(spec §4.2、複数 thread 利用)
/// - 内部 D-Bus call の thread-safety は impl 側責務(IBus 1.x adapter は
///   `Mutex<Vec<Candidate>>` 内部 buffer で coalesce)
pub trait IMEHostBridge: Send + Sync {
    /// preedit text を更新する。
    ///
    /// # Postconditions
    ///
    /// - host の preedit display が `text` / `cursor` / `visible` で書き換わる
    /// - `visible == false` の場合は preedit 非表示扱い(空文字列でも host 側で
    ///   表示無し化)
    fn update_preedit(&self, text: &str, cursor: usize, visible: bool);

    /// `text` を application に確定送信する。
    ///
    /// # Postconditions
    ///
    /// - host が application(focused window)に `text` を文字列として注入する
    fn commit_text(&self, text: &str);

    /// 候補 list の差分を反映する。
    ///
    /// # Postconditions
    ///
    /// - IBus 1.x adapter は `Replace` / `Append` / `Remove` / `Clear` を
    ///   internal buffer mutate + `update_lookup_table` 1 回で集約する
    ///   (spec §4.2 mapping 表)
    fn update_candidates(&self, update: CandidateUpdate);

    /// 候補 window を表示する。
    fn show_candidate_window(&self);

    /// 候補 window を非表示にする。
    fn hide_candidate_window(&self);
}
