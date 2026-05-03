//! 状態遷移 helper — spec §5.2 状態遷移 table を実装する。
//!
//! [`KotohaEngine::process_key_event`](super::KotohaEngine::process_key_event) から
//! 呼ばれ、現在 state + keysym で各 path に分岐する。

use kotoha_core::romaji::ConvertStep;
use kotoha_core::Candidate;

use super::{EngineState, KotohaEngine};
use crate::key_event::{KeyEvent, KeyEventResult};
use crate::ranker::{CandidateUpdate, ConversionMode};

/// X11 keysym 定数(IBus event で渡される値、spec §13 Open Q 8 で完全 mapping は実装段階)。
pub mod keysyms {
    pub const BACKSPACE: u32 = 0xff08;
    pub const RETURN: u32 = 0xff0d;
    pub const ESCAPE: u32 = 0xff1b;
    pub const SPACE: u32 = 0x0020;
    pub const TAB: u32 = 0xff09;
    pub const UP: u32 = 0xff52;
    pub const DOWN: u32 = 0xff54;
    pub const LEFT: u32 = 0xff51;
    pub const RIGHT: u32 = 0xff53;
}

/// `KotohaEngine::process_key_event` の本体 dispatch。
pub(super) fn dispatch_key(engine: &mut KotohaEngine, key: KeyEvent) -> KeyEventResult {
    match key.keysym {
        keysyms::BACKSPACE => handle_backspace(engine),
        keysyms::RETURN => handle_return(engine),
        keysyms::ESCAPE => handle_escape(engine),
        keysyms::SPACE => handle_space(engine),
        keysyms::UP | keysyms::DOWN | keysyms::LEFT | keysyms::RIGHT | keysyms::TAB => {
            handle_navigation(engine, key.keysym)
        }
        _ => handle_typing(engine, key),
    }
}

/// 通常文字入力 path(spec §5.2: Idle / Live / CandidatesShown + 通常 char)。
fn handle_typing(engine: &mut KotohaEngine, key: KeyEvent) -> KeyEventResult {
    // ASCII printable 範囲のみ Romaji に渡す。spec §13 Open Q 8 で他文字
    // 処理は実装段階対応。
    let ch = match char::from_u32(key.keysym) {
        Some(c) if c.is_ascii_graphic() => c,
        _ => return KeyEventResult::Forwarded,
    };

    // CandidatesShown で typing 再開 → hide_candidate_window + LiveConverting へ
    if engine.state == EngineState::CandidatesShown {
        engine.host.hide_candidate_window();
        engine.candidates.clear();
        engine.highlight_idx = 0;
    }

    // RomajiConverter::push でストリーミング 1 char convert(state を蓄積)。
    // push 後に normalize_pending で sokuon/hatsuon edge を確定させる。
    let mut committed = String::new();
    if let ConvertStep::Committed(s) = engine.romaji.push(ch) {
        committed.push_str(&s);
    }
    committed.push_str(&engine.romaji.normalize_pending());
    if !committed.is_empty() {
        engine.current_preedit.push_str(&committed);
    }

    // preedit 更新
    let cursor = engine.current_preedit.chars().count();
    engine.host.update_preedit(
        &engine.current_preedit,
        cursor,
        !engine.current_preedit.is_empty(),
    );

    // 状態遷移 → LiveConverting(preedit 非空時のみ)
    if !engine.current_preedit.is_empty() {
        engine.cancel_active();
        engine.dispatch_rank_request(ConversionMode::Live);
        // B0g #148 / I16:dispatch_rank_request が worker channel disconnect を
        // 観測すると `enabled = false` + `state = Idle` に degrade する。本 path
        // では state を LiveConverting に上書きせず、IME-disabled 状態を維持する
        // (上書きすると次 keystroke で再度 dispatch を試み ERROR log flood に
        // 戻る)。
        if !engine.enabled {
            return KeyEventResult::Consumed;
        }
        if !engine.candidates.is_empty() {
            engine
                .host
                .update_candidates(CandidateUpdate::Replace(engine.candidates.clone()));
            engine.host.show_candidate_window();
        }
        engine.state = EngineState::LiveConverting;
    } else {
        // pending romaji のみ(kana 出力なし)、状態は Idle 維持
        engine.state = EngineState::Idle;
    }

    KeyEventResult::Consumed
}

/// Backspace path(spec §6.2)。
fn handle_backspace(engine: &mut KotohaEngine) -> KeyEventResult {
    match engine.state {
        EngineState::Idle => KeyEventResult::Forwarded,
        EngineState::LiveConverting
        | EngineState::CommitConverting
        | EngineState::CandidatesShown => {
            // CandidatesShown → 候補 window hide
            if engine.state == EngineState::CandidatesShown {
                engine.host.hide_candidate_window();
                engine.candidates.clear();
                engine.highlight_idx = 0;
            }

            // spec §6.2: kana 末尾 1 char pop + romaji pending reset
            engine.current_preedit.pop();
            engine.romaji.reset_pending();

            let cursor = engine.current_preedit.chars().count();
            engine.host.update_preedit(
                &engine.current_preedit,
                cursor,
                !engine.current_preedit.is_empty(),
            );

            engine.cancel_active();

            if engine.current_preedit.is_empty() {
                engine.host.hide_candidate_window();
                engine.state = EngineState::Idle;
            } else {
                engine.dispatch_rank_request(ConversionMode::Live);
                if !engine.candidates.is_empty() {
                    engine
                        .host
                        .update_candidates(CandidateUpdate::Replace(engine.candidates.clone()));
                    engine.host.show_candidate_window();
                }
                engine.state = EngineState::LiveConverting;
            }
            KeyEventResult::Consumed
        }
    }
}

/// Space path — commit-mode 開始(spec §5.2 row 4 + row 7)。
///
/// spec §5.2 strict: `LiveConverting + space → CommitConverting`(候補未到着の中間状態)、
/// `CommitConverting + RankerOutput → CandidatesShown`(候補到着で遷移)の 2 段。
///
/// Phase 3-B B0d (ISSUE #140 / Critical 4 async path 修正):
/// 1. state を即 `CommitConverting` に遷移し、`show_candidate_window` で
///    user に「変換中」フィードバックを出す(内容は到着待ち)。
/// 2. `dispatch_rank_request` 内 blocking で第 1 batch を待つ(同期 Mock 経路 + 速い
///    実 Ranker は ここで populate される)。
/// 3. 候補が間に合えば即 `CandidatesShown` に追加遷移。間に合わない場合は
///    `CommitConverting` で抜け、後続 `process_key_event` 先頭の
///    [`KotohaEngine::drain_pending_events`] が候補到着時に `CandidatesShown` 遷移を行う。
fn handle_space(engine: &mut KotohaEngine) -> KeyEventResult {
    match engine.state {
        EngineState::Idle => KeyEventResult::Forwarded,
        EngineState::LiveConverting | EngineState::CommitConverting => {
            engine.cancel_active();
            // spec §5.2 row 4: 即 CommitConverting + show_candidate_window
            engine.candidates.clear();
            engine.highlight_idx = 0;
            engine.state = EngineState::CommitConverting;
            engine.host.show_candidate_window();

            engine.dispatch_rank_request(ConversionMode::Commit);

            // 候補が dispatch 内 drain で間に合っていれば row 7 遷移を即適用。
            // 間に合っていなければ後続 keystroke の drain_pending_events で対応。
            if !engine.candidates.is_empty() {
                engine
                    .host
                    .update_candidates(CandidateUpdate::Replace(engine.candidates.clone()));
                engine.state = EngineState::CandidatesShown;
            }
            KeyEventResult::Consumed
        }
        EngineState::CandidatesShown => {
            // CandidatesShown で space は次候補 navigation(IBus 慣行)
            handle_navigation(engine, keysyms::DOWN)
        }
    }
}

/// Return / Enter — commit 確定(spec §6.3)。
fn handle_return(engine: &mut KotohaEngine) -> KeyEventResult {
    if engine.state != EngineState::CandidatesShown || engine.candidates.is_empty() {
        return KeyEventResult::Forwarded;
    }
    let selected: Candidate = engine.candidates[engine.highlight_idx].clone();
    let kana_at_request = engine.current_preedit.clone();

    // B0g-b #148 / 第 2 回 review I8: host 出力の trust boundary で sanitization。
    // 改ざん辞書 / 悪意ある LLM 出力 / Phase 5 custom model から来た候補が ANSI
    // escape / NUL byte / RTL override 等を含む場合、application 側(terminal /
    // chat client / git editor 等)に注入されないよう commit を skip する。
    //
    // self-review F2:本 path に到達するということは `apply_candidate_update`
    // の filter を通り抜けて engine.candidates に保持されている surface が
    // unsafe ということ。理論上は到達不可能(filter があるため)だが、防御
    // 深度として残し、到達した場合は engine state を Idle に戻して UI ロック
    // を防ぐ(候補ウィンドウ閉鎖 + preedit clear + state Idle)。`debug_assert`
    // で dev / CI build では必ず観測する。
    if !crate::sanitize::is_safe_for_host(&selected.surface) {
        tracing::error!(
            surface_len = selected.surface.chars().count(),
            "candidate surface contains unsafe control / bidi / escape characters; \
             skipping commit_text and resetting engine state to avoid UI lock"
        );
        debug_assert!(
            false,
            "unsafe surface reached handle_return; apply_candidate_update filter bypass?"
        );
        // F2 cleanup:UI 状態を Idle に戻して user が次操作できるようにする。
        engine.host.hide_candidate_window();
        engine.host.update_preedit("", 0, false);
        engine.current_preedit.clear();
        engine.romaji.reset_pending();
        engine.candidates.clear();
        engine.highlight_idx = 0;
        engine.cancel_active();
        engine.state = EngineState::Idle;
        return KeyEventResult::Consumed;
    }

    engine.host.commit_text(&selected.surface);
    if let Err(e) = engine
        .learning_writer
        .record_choice(&kana_at_request, &selected.surface)
    {
        tracing::warn!(error = %e, "learning_cache record_choice failed; commit succeeded");
    }
    engine.commit_history.push(selected.surface.clone());
    engine.last_commit_at = std::time::Instant::now();

    engine.host.hide_candidate_window();
    engine.host.update_preedit("", 0, false);
    engine.current_preedit.clear();
    engine.romaji.reset_pending();
    engine.candidates.clear();
    engine.highlight_idx = 0;
    engine.cancel_active();
    engine.state = EngineState::Idle;
    KeyEventResult::Consumed
}

/// Escape — 候補閉 / preedit clear(spec §5.2)。
fn handle_escape(engine: &mut KotohaEngine) -> KeyEventResult {
    match engine.state {
        EngineState::Idle => KeyEventResult::Forwarded,
        EngineState::LiveConverting | EngineState::CommitConverting => {
            engine.cancel_active();
            engine.current_preedit.clear();
            engine.romaji.reset_pending();
            engine.candidates.clear();
            engine.highlight_idx = 0;
            engine.host.update_preedit("", 0, false);
            engine.host.hide_candidate_window();
            engine.state = EngineState::Idle;
            KeyEventResult::Consumed
        }
        EngineState::CandidatesShown => {
            // spec §5.2: CandidatesShown で Esc は候補閉、preedit kana 維持で Live 復帰
            engine.host.hide_candidate_window();
            engine.candidates.clear();
            engine.highlight_idx = 0;
            engine.cancel_active();
            if engine.current_preedit.is_empty() {
                engine.state = EngineState::Idle;
            } else {
                engine.dispatch_rank_request(ConversionMode::Live);
                if !engine.candidates.is_empty() {
                    engine
                        .host
                        .update_candidates(CandidateUpdate::Replace(engine.candidates.clone()));
                    engine.host.show_candidate_window();
                }
                engine.state = EngineState::LiveConverting;
            }
            KeyEventResult::Consumed
        }
    }
}

/// 候補 navigation(↑↓←→ / Tab、CandidatesShown で highlight 移動)。
fn handle_navigation(engine: &mut KotohaEngine, keysym: u32) -> KeyEventResult {
    if engine.state != EngineState::CandidatesShown || engine.candidates.is_empty() {
        return KeyEventResult::Forwarded;
    }
    let n = engine.candidates.len();
    match keysym {
        keysyms::DOWN | keysyms::TAB | keysyms::RIGHT => {
            engine.highlight_idx = (engine.highlight_idx + 1) % n;
        }
        keysyms::UP | keysyms::LEFT => {
            engine.highlight_idx = (engine.highlight_idx + n - 1) % n;
        }
        _ => return KeyEventResult::Forwarded,
    }
    engine
        .host
        .update_candidates(CandidateUpdate::Replace(engine.candidates.clone()));
    KeyEventResult::Consumed
}
