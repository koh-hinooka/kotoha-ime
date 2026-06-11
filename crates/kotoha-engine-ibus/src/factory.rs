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
//! 詳細仕様: `docs/specs/_uncategorized/p3-b-ibus-listener.md` §4.5 / §7.3、
//! ADR 0021 Amendment 2026-06-11。

use zbus::interface;

use crate::proxy::IBUS_ENGINE_OBJECT_PATH;

/// component XML `<engines>` に列挙した engine 名(`<name>kotoha</name>`)。
/// daemon はこの名前で `CreateEngine` を call する。
pub(crate) const ENGINE_NAME: &str = "kotoha";

/// IBus engine factory。state を持たない(engine object は connection build 時に
/// `serve_at` 済みで、本 service は path の返却のみを行う)。
pub(crate) struct KotohaFactoryService;

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
            tracing::warn!(
                error_id = "factory.create_engine.unknown_engine",
                engine_name,
                "CreateEngine called with unknown engine name"
            );
            return Err(zbus::fdo::Error::InvalidArgs(format!(
                "unknown engine name: {engine_name}"
            )));
        }
        tracing::info!(engine_name, path = IBUS_ENGINE_OBJECT_PATH, "CreateEngine");
        let path = zbus::zvariant::ObjectPath::try_from(IBUS_ENGINE_OBJECT_PATH)
            .expect("IBUS_ENGINE_OBJECT_PATH is a valid D-Bus object path");
        Ok(path.into())
    }
}

#[cfg(test)]
mod tests {
    //! Spec: `docs/specs/_uncategorized/p3-b-ibus-listener.md` §10.1
    //! L1 unit tests。dbus を使わず `#[interface]` の inherent method を直接呼ぶ。

    use super::*;

    #[test]
    fn create_engine_returns_static_engine_path_for_kotoha() {
        let factory = KotohaFactoryService;
        let path = factory
            .create_engine(ENGINE_NAME)
            .expect("known engine name must succeed");
        assert_eq!(path.as_str(), IBUS_ENGINE_OBJECT_PATH);
    }

    #[test]
    fn create_engine_rejects_unknown_engine_name() {
        let factory = KotohaFactoryService;
        let err = factory
            .create_engine("anthy")
            .expect_err("unknown engine name must be rejected");
        assert!(
            matches!(err, zbus::fdo::Error::InvalidArgs(_)),
            "expected InvalidArgs, got: {err:?}"
        );
    }
}
