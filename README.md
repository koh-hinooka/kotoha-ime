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
| 0 | Cargo workspace + ローマ字→かな変換 + CLI | 完了 |
| 1 | かな→漢字変換 (Zenz + llama.cpp) | 未着手 |
| 2 | システム辞書 + ユーザ辞書 + 学習 | 未着手 |
| 3 | IBus engine (GNOME) | 未着手 |
| 4 | fcitx5 addon (KDE/wlroots) | 未着手 |
| 5 | タイポ訂正, Shift 挙動設定, 文脈リランキング | 未着手 |
| 6 | 設定 UI, 辞書自動更新, 同期 | 未着手 |

詳細は `docs/superpowers/specs/` の各設計書を参照。

## kotoha-romaji CLI 使用例

Phase 0 で実装した `kotoha-romaji` は、ローマ字→かな変換と Shift トリガによる Direct モード遷移を動作確認するための CLI である。標準入力から行単位で読み込み、行末を commit として扱う (詳細は `docs/adr/0004-cli-line-based-commit.md` と spec §10 を参照)。

### ビルド

```bash
cargo build -p kotoha-cli
# バイナリは target/debug/kotoha-romaji に生成される
```

### 基本的な使い方

以下の例は `target/debug/kotoha-romaji` を `kotoha-romaji` として PATH 上で実行できる前提で記述する。

1. **基本: ローマ字→ひらがな変換**

    ```bash
    $ echo "konnnichiha" | kotoha-romaji
    こんにちは
    ```

2. **Shift トリガ: 大文字入力で Transient Direct モード**

    ```bash
    $ echo "HELLO" | kotoha-romaji
    HELLO
    ```

3. **混在: 大文字始まりの行の後に Enter で自動復帰**

    ```bash
    $ printf "Konnichiwa\nkonnnichiha\n" | kotoha-romaji
    Konnichiwa
    こんにちは
    ```

4. **Sticky Direct モード (`--mode direct` で明示起動)**

    ```bash
    $ printf "hello\nworld\n" | kotoha-romaji --mode direct
    hello
    world
    ```

5. **モード表示 (デバッグ用、`--show-mode`)**

    ```bash
    $ printf "Hi\nkon\n" | kotoha-romaji --show-mode
    Hi [H]
    こん [H]
    ```

6. **Sticky Direct + モード表示**

    ```bash
    $ echo "hi" | kotoha-romaji --mode direct --show-mode
    hi [D]
    ```

### スモークテスト

Phase 0 の canonical smoke test として `scripts/phase0-smoke.sh` を提供する。上記 6 例を含む spec §13.2 の assertion を一括検証する。

```bash
./scripts/phase0-smoke.sh
```

## 開発環境セットアップ

本プロジェクトはローカルの Git hook(lefthook)で品質ゲートを担保する。CI/CD システムは導入していないため、**clone 後は必ず `lefthook install` を実行する**。

```bash
git clone <repo-url>
cd kotoha-ime
lefthook install
```

`lefthook install` を忘れると pre-commit / pre-push の検査がスキップされ、フォーマット違反・文書命名規則違反・ビルド破壊を含むコミットが走ってしまうため注意。

## ライセンス

本プロジェクトは [MIT License](./LICENSE-MIT) または [Apache License 2.0](./LICENSE-APACHE) のデュアルライセンスで配布される。利用者はいずれか一方を選択できる。

## 参考実装

- [togatoga/karukan](https://github.com/togatoga/karukan) — Rust 製 Linux 日本語 IME。本プロジェクトの参考実装
- [azooKey/AzooKeyKanaKanjiConverter](https://github.com/azooKey/AzooKeyKanaKanjiConverter) — Swift 製かな漢字変換ライブラリ
