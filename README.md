# Kotoha (kotoha-ime)

日本語入力メソッド (IME) を Rust で自作するプロジェクト。GNOME Wayland をプライマリターゲットとする。

## 目標

- GNOME Wayland ネイティブに動作する軽量で拡張可能な日本語 IME
- ニューラル変換 (Zenz 系 GPT-2 + llama.cpp) による高品質なかな漢字変換
- 設定可能な挙動 (Shift 動作、ホットキー、モードなど)
- タイポ訂正、文脈リランキング等のモダンな機能
- IBus engine として GNOME に統合、将来的に fcitx5 addon (KDE/wlroots 対応) も提供

## アーキテクチャ

Cargo workspace として以下の crate を持つ:

- `kotoha-core`: 変換エンジン本体 (ローマ字→かな, かな→漢字, 辞書, 学習)
- `kotoha-cli`: CLI ツール群 (辞書ビルド, 評価サーバ, ベンチ)
- `kotoha-ibus`: IBus engine (GNOME 用, Phase 3 で追加)
- `kotoha-fcitx5`: fcitx5 addon (KDE/wlroots 用, Phase 4 で追加)

## 開発フェーズ

| Phase | 内容 | 状態 |
|---|---|---|
| 0 | Cargo workspace + ローマ字→かな変換 + CLI | 設計完了、実装未着手 |
| 1 | かな→漢字変換 (Zenz + llama.cpp) | 未着手 |
| 2 | システム辞書 + ユーザ辞書 + 学習 | 未着手 |
| 3 | IBus engine (GNOME) | 未着手 |
| 4 | fcitx5 addon (KDE/wlroots) | 未着手 |
| 5 | タイポ訂正, Shift 挙動設定, 文脈リランキング | 未着手 |
| 6 | 設定 UI, 辞書自動更新, 同期 | 未着手 |

詳細は `docs/superpowers/specs/` の各設計書を参照。

## ライセンス

MIT OR Apache-2.0 のデュアルライセンス (ライセンスファイルは実装フェーズで追加)。

## 参考実装

- [togatoga/karukan](https://github.com/togatoga/karukan) — Rust 製 Linux 日本語 IME。本プロジェクトの参考実装
- [azooKey/AzooKeyKanaKanjiConverter](https://github.com/azooKey/AzooKeyKanaKanjiConverter) — Swift 製かな漢字変換ライブラリ
