# Kotoha Roadmap

Kotoha プロジェクトの開発フェーズと、各フェーズの到達目標を記録する。

## Phase 一覧

| Phase | 名称 | 内容 | 状態 |
|---|---|---|---|
| 0 | Foundation | Cargo workspace + ローマ字→かな変換 + 入力モード管理 + CLI | 実装中 |
| 1 | Kana→Kanji conversion | Zenz + llama.cpp によるかな→漢字変換 | 未着手 |
| 2 | Dictionary and learning | システム辞書 + ユーザ辞書 + 学習キャッシュ | 未着手 |
| 3 | IBus integration | IBus engine(GNOME Mutter 用) | 未着手 |
| 4 | fcitx5 integration | fcitx5 addon(KDE / wlroots 用) | 未着手 |
| 5 | Advanced features | タイポ訂正 + 文脈リランキング | 未着手 |
| 6 | UX polish | 設定 UI + 辞書自動更新 + 同期 | 未着手 |

## Phase 0 マイルストーン

Phase 0 の詳細マイルストーン分割は `docs/superpowers/plans/2026-04-22-kotoha-phase-0-implementation.md` を参照。

## 注記

- Phase 5 の「Shift 挙動設定」は、設計書 revision 2 の判断により Phase 0 に前倒しされた。Phase 5 は「タイポ訂正 + 文脈リランキング」のみ。
- 各 Phase の設計書は `docs/superpowers/specs/` に配置する。
