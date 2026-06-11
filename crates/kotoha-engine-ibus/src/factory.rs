//! `KotohaFactoryService` — IBus engine factory(#208 / spec §4.5)。
//!
//! ibus-daemon の engine 生成 protocol は、component が
//! `org.freedesktop.IBus.Factory` interface を `/org/freedesktop/IBus/Factory`
//! で serve していることを必須とする。daemon は `CreateEngine(engine_name)` を
//! call し、返却された object path に対して engine method を発行する。
//!
//! 本実装は静的単一 path 採択(spec §7.3)のため、`CreateEngine` は常に
//! 既 serve 済の [`crate::proxy::IBUS_ENGINE_OBJECT_PATH`] を返す。
//!
//! # Panic-free 規約(spec §9.1)
//!
//! zbus 5 は `#[interface]` handler 内の panic を catch **しない**(handler panic
//! は dispatch task を黙殺し、以後の全 method dispatch が無 log で停止する)。
//! このため handler 本体に panic site を置くことは禁止で、engine path は
//! `listener::build_connection` が事前 validate した値を field 注入する。
//!
//! 詳細仕様: `docs/specs/_uncategorized/p3-b-ibus-listener.md` §4.5 / §7.3 / §9.1、
//! ADR 0021 Amendment 2026-06-11。

use zbus::interface;

/// component XML `<engines>` に列挙した engine 名(`<name>kotoha</name>`)。
/// daemon はこの名前で `CreateEngine` を call する。
pub(crate) const ENGINE_NAME: &str = "kotoha";

/// IBus engine factory。
///
/// `engine_path` は `listener::build_connection` が事前 validate した
/// [`crate::proxy::IBUS_ENGINE_OBJECT_PATH`] を保持する(handler 内 panic site
/// 排除、module doc 参照)。engine object は connection build 時に `serve_at`
/// 済みのため、`CreateEngine` は path の返却のみを行う。
pub(crate) struct KotohaFactoryService {
    /// 事前 validate 済の静的 engine object path(spec §7.3)。
    pub(crate) engine_path: zbus::zvariant::OwnedObjectPath,
}

#[interface(name = "org.freedesktop.IBus.Factory")]
impl KotohaFactoryService {
    /// `CreateEngine(s engine_name) -> o`(IBus 1.5.x `ibusfactory.c` 仕様)。
    ///
    /// `engine_name == "kotoha"` のとき静的 engine path を返す。未知名は
    /// `org.freedesktop.DBus.Error.InvalidArgs` で reject する(spec §9.1)。
    fn create_engine(
        &self,
        engine_name: &str,
    ) -> zbus::fdo::Result<zbus::zvariant::OwnedObjectPath> {
        if engine_name != ENGINE_NAME {
            // engine_name は bus 上の任意 peer が指定できる文字列。log forging
            // 防止のため制御文字を escape して観測する(#207 と同型の log
            // injection 対策)。
            tracing::warn!(
                error_id = "factory.create_engine.unknown_engine",
                engine_name = %engine_name.escape_debug(),
                "CreateEngine called with unknown engine name"
            );
            return Err(zbus::fdo::Error::InvalidArgs(format!(
                "unknown engine name: {}",
                engine_name.escape_debug()
            )));
        }
        tracing::info!(engine_name, path = %self.engine_path, "CreateEngine");
        Ok(self.engine_path.clone())
    }
}

#[cfg(test)]
mod tests {
    //! Spec: `docs/specs/_uncategorized/p3-b-ibus-listener.md` §10.1
    //! L1 unit tests。dbus を使わず `#[interface]` の inherent method を直接呼ぶ。

    use super::*;
    use crate::proxy::IBUS_ENGINE_OBJECT_PATH;

    fn setup() -> KotohaFactoryService {
        KotohaFactoryService {
            engine_path: zbus::zvariant::ObjectPath::try_from(IBUS_ENGINE_OBJECT_PATH)
                .expect("valid path")
                .into(),
        }
    }

    #[test]
    fn create_engine_returns_static_engine_path_for_kotoha() {
        let factory = setup();
        let path = factory
            .create_engine(ENGINE_NAME)
            .expect("known engine name must succeed");
        assert_eq!(path.as_str(), IBUS_ENGINE_OBJECT_PATH);
    }

    #[test]
    fn create_engine_rejects_unknown_engine_name() {
        let factory = setup();
        let err = factory
            .create_engine("anthy")
            .expect_err("unknown engine name must be rejected");
        assert!(
            matches!(err, zbus::fdo::Error::InvalidArgs(_)),
            "expected InvalidArgs, got: {err:?}"
        );
    }

    #[test]
    fn create_engine_escapes_control_characters_in_rejection() {
        // log forging 対策: 制御文字入り engine_name は escape されて返る
        let factory = setup();
        let err = factory
            .create_engine("evil\nname")
            .expect_err("unknown engine name must be rejected");
        let zbus::fdo::Error::InvalidArgs(msg) = err else {
            panic!("expected InvalidArgs");
        };
        assert!(
            msg.contains("evil\\nname"),
            "control characters must be escaped in the error message: {msg}"
        );
    }
}
