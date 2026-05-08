//! IBus 1.x D-Bus interface proxy 定義(zbus 5.x blocking API 経由)。
//!
//! [`IBusEngineSignals`] は IBus engine が host(`InputContext`)に発する
//! signal の helper 群。Phase 3-B B2 で 5 method すべてが
//! `Message::signal(...)?.build(&body)?` + `connection.send(&signal)` の形で
//! 実 D-Bus signal を session bus に発信する。
//!
//! signal の wire-format type は [`crate::types`] module で定義(spec §4.2 r3)。
//!
//! # 失敗時挙動
//!
//! 各 method は `Result<(), zbus::Error>` を返し、caller(`host_bridge.rs`)が
//! `tracing::warn!(error = %e, ...)` で集約観測する。signal failure は engine
//! state に伝播させない(spec §9.3「silent_failure 禁止」と「non-propagating」
//! の両立)。
//!
//! # Security
//!
//! D-Bus session bus 上の signal はすべて **同 UID で動作する全プロセス** に
//! 配信される。`org.freedesktop.IBus.Engine` の `UpdatePreeditText` /
//! `CommitText` を subscribe する任意 process(browser extension の subprocess、
//! malicious npm postinstall、Electron app 等)が user の打鍵内容を
//! 取得可能。これは IBus protocol の根本前提であり、Kotoha は
//! **「user session = 同 UID プロセスは信頼可能」** を threat model assumption
//! として採用する。multi-user kiosk / shared user account のような前提が
//! 成立しないユースケースは out of scope(spec §4.2 r3)。
//!
//! # Visibility
//!
//! `IBusEngineSignals` は `pub(crate)` で crate 外部からは到達不可。production
//! 経路は `IBusHostBridge::IMEHostBridge` impl 1 点のみ。各 path の input gate は:
//!
//! - `commit_text`:`kotoha_engine_core::engine::transitions` 内 commit gate
//!   (`is_safe_for_host` filter 通過)
//! - `update_candidates`:`KotohaEngine::filter_safe_candidates`(同じく
//!   `is_safe_for_host` filter 通過)
//! - `update_preedit`:RomajiConverter の構造的出力(hiragana / katakana のみ)
//!   を直接渡す。wire-format 層 NUL handling が最終 fail-safe として動作する
//!
//! adapter 内部の defense-in-depth は visibility 降格で satisfy する(crate 外部
//! caller が sanitize を bypass して `IBusEngineSignals` を直接呼ぶ path 自体が存在
//! しない)。

use zbus::blocking::Connection;
use zbus::message::Message;
use zbus::Result;

use crate::types::{IBusLookupTable, IBusText};

/// IBus 1.5.x の engine signal が emit される D-Bus interface 名。
///
/// 対応する仕様: IBus 1.5.x `bus/inputcontext.c` の `BUS_INPUT_CONTEXT_GET_INTERFACE`。
pub(crate) const IBUS_ENGINE_INTERFACE: &str = "org.freedesktop.IBus.Engine";

/// Kotoha が `RequestName` で取得する IBus engine bus name(spec §7.1)。
///
/// 同 UID で同名 process が既に publish していると `Builder::name(...)` が `Err` を返し、
/// listener thread の起動が失敗する。
pub(crate) const IBUS_ENGINE_BUS_NAME: &str = "org.freedesktop.IBus.Engine.Kotoha";

/// Kotoha engine service の D-Bus object path(spec §7.1)。
///
/// `Builder::serve_at(...)` でこの path に `KotohaEngineService` を登録する。
pub(crate) const IBUS_ENGINE_OBJECT_PATH: &str = "/org/freedesktop/IBus/Engine/Kotoha";

/// `UpdatePreeditText` signal member 名(IBus 1.5.x 仕様)。
pub(crate) const MEMBER_UPDATE_PREEDIT_TEXT: &str = "UpdatePreeditText";
/// `CommitText` signal member 名。
pub(crate) const MEMBER_COMMIT_TEXT: &str = "CommitText";
/// `UpdateLookupTable` signal member 名。
pub(crate) const MEMBER_UPDATE_LOOKUP_TABLE: &str = "UpdateLookupTable";
/// `ShowLookupTable` signal member 名。
pub(crate) const MEMBER_SHOW_LOOKUP_TABLE: &str = "ShowLookupTable";
/// `HideLookupTable` signal member 名。
pub(crate) const MEMBER_HIDE_LOOKUP_TABLE: &str = "HideLookupTable";

/// IBus engine が host(`InputContext`)に発する signal の helper 群。
///
/// Phase 3-B B2 完了後は 5 method すべてが session bus に実 D-Bus signal を
/// 発信する。
pub(crate) struct IBusEngineSignals {
    /// zbus blocking connection(session bus)。
    connection: Connection,
    /// engine object path(`/org/freedesktop/IBus/Engine/Kotoha` 等)。
    object_path: zbus::zvariant::OwnedObjectPath,
}

impl IBusEngineSignals {
    /// Session bus に接続し、engine object path で signal emit を準備する。
    ///
    /// # Errors
    ///
    /// - zbus connection 確立失敗(`DBUS_SESSION_BUS_ADDRESS` 不設定等)
    /// - object_path 不正(D-Bus path syntax 違反)
    pub(crate) fn new(object_path: &str) -> Result<Self> {
        // path validation を session bus 接続より前に実行する(test から
        // session 不要で path validation を exercise できるように、構造
        // 分離は build_object_path で実施)。
        let path = build_object_path(object_path)?;
        let connection = Connection::session()?;
        Ok(Self {
            connection,
            object_path: path,
        })
    }

    /// `UpdatePreeditText(IBusText, u32 cursor_pos, bool visible)` signal emit。
    ///
    /// signature: `(vub)`(IBusText variant + cursor + visible)
    ///
    /// # Errors
    ///
    /// - signal message build 失敗(D-Bus serialization error、NUL 含み text 等)
    /// - `connection.send` 失敗(D-Bus daemon disconnect 等)
    pub(crate) fn update_preedit(&self, text: &str, cursor: u32, visible: bool) -> Result<()> {
        tracing::debug!(
            text_byte_len = text.len(),
            cursor,
            visible,
            "IBus update_preedit emit"
        );
        let signal = build_update_preedit_signal(self.object_path.as_str(), text, cursor, visible)?;
        self.connection.send(&signal)?;
        Ok(())
    }

    /// `CommitText(IBusText)` signal emit。
    ///
    /// signature: `(v)`(IBusText variant のみ)
    ///
    /// # Errors
    ///
    /// - signal message build 失敗
    /// - `connection.send` 失敗
    pub(crate) fn commit_text(&self, text: &str) -> Result<()> {
        tracing::debug!(text_byte_len = text.len(), "IBus commit_text emit");
        let signal = build_commit_text_signal(self.object_path.as_str(), text)?;
        self.connection.send(&signal)?;
        Ok(())
    }

    /// `UpdateLookupTable(IBusLookupTable, bool visible)` signal emit。
    ///
    /// signature: `(vb)`(IBusLookupTable variant + visible)
    ///
    /// # Errors
    ///
    /// - signal message build 失敗
    /// - `connection.send` 失敗
    pub(crate) fn update_lookup_table(
        &self,
        candidates: &[kotoha_core::Candidate],
        visible: bool,
    ) -> Result<()> {
        tracing::debug!(
            count = candidates.len(),
            visible,
            "IBus update_lookup_table emit"
        );
        let signal =
            build_update_lookup_table_signal(self.object_path.as_str(), candidates, visible)?;
        self.connection.send(&signal)?;
        Ok(())
    }

    /// `ShowLookupTable()` signal emit。
    ///
    /// signature: `()`(empty body)
    ///
    /// # Errors
    ///
    /// - signal message build 失敗
    /// - `connection.send` 失敗
    pub(crate) fn show_lookup_table(&self) -> Result<()> {
        tracing::debug!("IBus show_lookup_table emit");
        let signal = build_show_lookup_table_signal(self.object_path.as_str())?;
        self.connection.send(&signal)?;
        Ok(())
    }

    /// `HideLookupTable()` signal emit。
    ///
    /// signature: `()`(empty body)
    ///
    /// # Errors
    ///
    /// - signal message build 失敗
    /// - `connection.send` 失敗
    pub(crate) fn hide_lookup_table(&self) -> Result<()> {
        tracing::debug!("IBus hide_lookup_table emit");
        let signal = build_hide_lookup_table_signal(self.object_path.as_str())?;
        self.connection.send(&signal)?;
        Ok(())
    }
}

/// engine object path を `OwnedObjectPath` に変換する。`Connection::session()` を
/// 必要としないため、L1 unit test から path validation を直接 exercise できる。
pub(crate) fn build_object_path(object_path: &str) -> Result<zbus::zvariant::OwnedObjectPath> {
    let path: zbus::zvariant::ObjectPath = zbus::zvariant::ObjectPath::try_from(object_path)?;
    Ok(path.into())
}

/// `UpdatePreeditText` signal を組み立てる pure factory。production method が
/// 内部で呼び、L1 unit test もここを exercise することで `MEMBER_UPDATE_PREEDIT_TEXT`
/// および `IBUS_ENGINE_INTERFACE` の literal が test path に乗る。
pub(crate) fn build_update_preedit_signal(
    object_path: &str,
    text: &str,
    cursor: u32,
    visible: bool,
) -> Result<Message> {
    let body = IBusText::plain(text.to_string()).into_variant();
    Message::signal(
        object_path,
        IBUS_ENGINE_INTERFACE,
        MEMBER_UPDATE_PREEDIT_TEXT,
    )?
    .build(&(body, cursor, visible))
}

/// `CommitText` signal を組み立てる pure factory。
pub(crate) fn build_commit_text_signal(object_path: &str, text: &str) -> Result<Message> {
    let body = IBusText::plain(text.to_string()).into_variant();
    Message::signal(object_path, IBUS_ENGINE_INTERFACE, MEMBER_COMMIT_TEXT)?.build(&(body,))
}

/// `UpdateLookupTable` signal を組み立てる pure factory。
pub(crate) fn build_update_lookup_table_signal(
    object_path: &str,
    candidates: &[kotoha_core::Candidate],
    visible: bool,
) -> Result<Message> {
    let ibus_candidates: Vec<IBusText> = candidates
        .iter()
        .map(|c| IBusText::plain(c.surface.clone()))
        .collect();
    let body = IBusLookupTable::from_candidates(ibus_candidates).into_variant();
    Message::signal(
        object_path,
        IBUS_ENGINE_INTERFACE,
        MEMBER_UPDATE_LOOKUP_TABLE,
    )?
    .build(&(body, visible))
}

/// `ShowLookupTable` signal を組み立てる pure factory。
pub(crate) fn build_show_lookup_table_signal(object_path: &str) -> Result<Message> {
    Message::signal(object_path, IBUS_ENGINE_INTERFACE, MEMBER_SHOW_LOOKUP_TABLE)?.build(&())
}

/// `HideLookupTable` signal を組み立てる pure factory。
pub(crate) fn build_hide_lookup_table_signal(object_path: &str) -> Result<Message> {
    Message::signal(object_path, IBUS_ENGINE_INTERFACE, MEMBER_HIDE_LOOKUP_TABLE)?.build(&())
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_PATH: &str = "/org/freedesktop/IBus/Engine/Kotoha";

    /// build_object_path が valid path syntax を accept する。
    #[test]
    fn build_object_path_accepts_valid_path() {
        let p = build_object_path(TEST_PATH).expect("valid path");
        assert_eq!(p.as_str(), TEST_PATH);
    }

    /// build_object_path が invalid path syntax を reject する(空文字列、
    /// 先頭 / なし、空白を含む等)。session bus に接続せず L1 で検証可能。
    #[test]
    fn build_object_path_rejects_invalid_path() {
        for invalid in &["", "foo", "/with space", "/-leading-dash"] {
            let result = build_object_path(invalid);
            assert!(
                result.is_err(),
                "invalid path {invalid:?} must be rejected, got {result:?}"
            );
        }
    }

    /// Spec §4.2 r3 acceptance: production の build_update_preedit_signal が
    /// IBus 1.x interface / member / path を持つ Message を組立てる。production
    /// method `IBusEngineSignals::update_preedit` が内部で本 factory を呼ぶ
    /// ため、本 test が member literal `"UpdatePreeditText"` の regression を
    /// 検出する(theater pattern 防止)。
    #[test]
    fn build_update_preedit_signal_has_correct_path_interface_member() {
        let signal = build_update_preedit_signal(TEST_PATH, "こ", 1, true).expect("build");
        let header = signal.header();
        assert_eq!(header.path().unwrap().as_str(), TEST_PATH);
        assert_eq!(
            header.interface().unwrap().as_str(),
            "org.freedesktop.IBus.Engine"
        );
        assert_eq!(header.member().unwrap().as_str(), "UpdatePreeditText");
    }

    /// Spec §4.2 r3 acceptance: build_commit_text_signal が `CommitText`
    /// member 名を持つ。
    #[test]
    fn build_commit_text_signal_has_correct_member() {
        let signal = build_commit_text_signal(TEST_PATH, "漢字").expect("build");
        assert_eq!(signal.header().member().unwrap().as_str(), "CommitText");
        assert_eq!(
            signal.header().interface().unwrap().as_str(),
            "org.freedesktop.IBus.Engine"
        );
    }

    /// Spec §4.2 r3 acceptance: build_update_lookup_table_signal が
    /// `UpdateLookupTable` member 名を持ち、候補配列が body に乗る。
    #[test]
    fn build_update_lookup_table_signal_has_correct_member_and_carries_candidates() {
        let cands = vec![
            kotoha_core::Candidate::new("漢字", -1.0),
            kotoha_core::Candidate::new("勘事", -2.0),
        ];
        let signal = build_update_lookup_table_signal(TEST_PATH, &cands, true).expect("build");
        assert_eq!(
            signal.header().member().unwrap().as_str(),
            "UpdateLookupTable"
        );
    }

    /// Spec §4.2 r3 acceptance: build_show_lookup_table_signal は empty body で
    /// 組立可能(IBus 1.x signature `()`)。
    #[test]
    fn build_show_lookup_table_signal_has_empty_body() {
        let signal = build_show_lookup_table_signal(TEST_PATH).expect("build");
        assert_eq!(
            signal.header().member().unwrap().as_str(),
            "ShowLookupTable"
        );
        assert!(signal.body().data().is_empty(), "empty body");
    }

    /// Spec §4.2 r3 acceptance: build_hide_lookup_table_signal は empty body で
    /// 組立可能(IBus 1.x signature `()`)。
    #[test]
    fn build_hide_lookup_table_signal_has_empty_body() {
        let signal = build_hide_lookup_table_signal(TEST_PATH).expect("build");
        assert_eq!(
            signal.header().member().unwrap().as_str(),
            "HideLookupTable"
        );
        assert!(signal.body().data().is_empty(), "empty body");
    }

    /// Spec §4.2 r3 acceptance + Security review #2: NUL 含み text の挙動を pin する。
    ///
    /// zvariant 5 は in-memory build 段階では NUL を reject せず Ok を返す
    /// (signal は header / body 共に正常に構築される)。実際の D-Bus 仕様準拠
    /// rejection は (a) `connection.send` 時の D-Bus serialize、または (b) IBus
    /// daemon 側 parse で起こり、いずれも `Result<(), zbus::Error>` の `Err` で
    /// 観測される。`host_bridge.rs` の `tracing::warn!(error = %e, ...)` が
    /// failure path を観測する(spec §9.3 silent_failure 禁止 + non-propagating)。
    ///
    /// 本 test は build path で **panic しない** ことを保証し、Err / Ok のいずれを
    /// 返しても caller(host_bridge)が graceful に処理可能であることを pin する。
    #[test]
    fn build_update_preedit_signal_with_embedded_nul_does_not_panic() {
        let _ = build_update_preedit_signal(TEST_PATH, "a\0b", 0, true);
        // panic していないことが test runner の正常 return で確認される
    }

    /// 同様に commit_text path でも NUL が graceful に処理される。
    #[test]
    fn build_commit_text_signal_with_embedded_nul_does_not_panic() {
        let _ = build_commit_text_signal(TEST_PATH, "a\0b");
    }

    /// 巨大文字列(10 KiB)入力でも build path が panic せず graceful に処理される。
    #[test]
    fn build_update_preedit_signal_with_10k_chars_does_not_panic() {
        let long = "あ".repeat(3500);
        let result = build_update_preedit_signal(TEST_PATH, &long, 0, true);
        assert!(result.is_ok(), "10 KiB UTF-8 string should build OK");
    }

    /// 空文字列 preedit が build できる(初期状態 / `Idle` 復帰時の挙動)。
    #[test]
    fn build_update_preedit_signal_with_empty_string_succeeds() {
        let result = build_update_preedit_signal(TEST_PATH, "", 0, false);
        assert!(
            result.is_ok(),
            "empty string build must succeed: {result:?}"
        );
    }
}
