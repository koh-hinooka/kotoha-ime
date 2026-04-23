---
title: Kotoha Phase 0 設計書 — ローマ字→かな変換コアと入力モード管理 + CLI
date: 2026-04-22
status: draft
phase: 0
revision: 2
---

# Kotoha Phase 0 設計書

## 1. 概要

本文書は Kotoha プロジェクトの Phase 0 に関する設計仕様である。Phase 0 のゴールは Rust workspace を立ち上げ、ローマ字→ひらがな変換のコアライブラリ、入力モード管理の状態機械、および動作確認用 CLI ツールを作成することである。

2026-04-22 初版では入力モード管理を Phase 5 に置いていたが、参考実装 Karukan の Shift キー挙動に起因する不便(Shift 押下で直接入力モードになった後、ユーザが明示的に戻さない限りひらがな変換モードに復帰しない)を解決することが本プロジェクトの主要動機の 1 つであったため、Phase 0 のスコープに入力モード管理を前倒しで組み込む改訂版として本文書を再発行する。

## 2. 背景と動機

Kotoha は GNOME Wayland 環境で動作する自作日本語 IME を目指すプロジェクトである。既存の選択肢 (Mozc, Karukan) を評価した結果、以下の理由で完全自作を選択した:

- Karukan の Shift キー挙動などのハードコードされた設計選択に手を入れたい(**最大の動機**)
- fcitx5 の GNOME Wayland 統合の癖から解放されたい
- 自作することで将来の機能拡張 (文脈理解、タイポ訂正) を自由に追加可能

特に 1 点目の「Karukan の Shift キー挙動」に関して、使い手が経験している具体的な不便は以下のとおりである:

- Shift + 何らかのキーを押すと直接入力モード(ローマ字がそのまま出力されるモード)に入る
- このモードに入ると Enter 確定後も直接入力モードが持続し、ひらがな変換モードに自動復帰しない
- 毎回ユーザ自身が明示的にモード切替キーを押してひらがなモードに戻す必要があり、日本語入力体験を著しく損なっている

これは Mozc / Google 日本語入力の挙動とも異なる。Mozc では Shift によって入る直接入力モードは Enter 確定で自動的にひらがなモードへ復帰する。Kotoha はこの Mozc 式の挙動を再現することを Phase 0 の達成目標に加える。

Phase 0 ではまず最も基礎となる「ローマ字入力をひらがなに変換する」部分と「入力モード管理の状態機械」を独立したライブラリとして実装する。ニューラル変換や IME フレームワーク統合には手を出さない。

## 3. ゴール

1. Cargo workspace の土台を作成する
2. `kotoha-core` crate にローマ字→ひらがな変換機能を実装する
3. `kotoha-core` crate に入力モード管理 (`InputContext`) を実装する
4. `kotoha-cli` crate に動作確認用 CLI (`kotoha-romaji`) を実装する
5. Karukan との挙動差分を定量把握できる golden test を整備する
6. lefthook によるローカル品質ゲートを確立する

## 4. 非ゴール (Phase 1 以降で扱う)

- かな→漢字変換
- システム辞書 / ユーザ辞書
- 学習キャッシュ
- Zenz 等のニューラルモデル推論
- IBus / fcitx5 統合
- **キーコード → モード遷移のマッピング設定** (Phase 3 の IBus engine 層で扱う)
- HuggingFace モデルダウンロード
- 文脈考慮変換

Phase 0 で実装するのは純粋なドメインロジック(ローマ字→かな変換とモード状態機械)のみであり、実機のキー入力からモード遷移を駆動する層は Phase 3 以降に委ねる。この責務分離こそが Karukan の「統合層にモード遷移ロジックをハードコードする」設計ミスを避ける鍵である。

## 5. アーキテクチャ

### 5.1 全体像 (Phase 6 完了時の想定)

```
┌──────────────────────────────────────────────────────────────┐
│              kotoha-core (Rust ライブラリ)                     │
│   - ローマ字→かな変換 ← Phase 0                              │
│   - 入力モード管理 (InputContext) ← Phase 0                   │
│   - かな→漢字変換 (Zenz + llama.cpp) ← Phase 1               │
│   - システム辞書 (Sudachi / UT / 独自) ← Phase 2              │
│   - ユーザ辞書 ← Phase 2                                      │
│   - 学習キャッシュ ← Phase 2                                  │
│   - 候補管理 ← Phase 2                                        │
│   - タイポ訂正 ← Phase 5                                      │
│   - 文脈リランキング ← Phase 5                                │
└──────────────┬──────────────────────────┬────────────────────┘
               │                          │
   ┌───────────▼────────────┐ ┌───────────▼─────────────┐
   │ kotoha-ibus            │ │ kotoha-fcitx5           │
   │ Rust + zbus            │ │ Rust + C++ FFI          │
   │ ← Phase 3              │ │ ← Phase 4               │
   │ GNOME Mutter 用         │ │ KDE / wlroots 用         │
   └─────────────────────────┘ └─────────────────────────┘
```

### 5.2 Phase 0 で実装する範囲

Phase 0 では以下のディレクトリ構成のうち、`kotoha-core` と `kotoha-cli` の 2 つの crate および関連ドキュメントのみを実装する:

```
kotoha-ime/
├── Cargo.toml                     # workspace
├── crates/
│   ├── kotoha-core/
│   │   ├── Cargo.toml
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── error.rs
│   │   │   ├── kana/
│   │   │   │   ├── mod.rs
│   │   │   │   ├── hiragana.rs
│   │   │   │   └── katakana.rs
│   │   │   ├── romaji/
│   │   │   │   ├── mod.rs
│   │   │   │   ├── rules.rs
│   │   │   │   ├── trie.rs
│   │   │   │   └── state.rs
│   │   │   └── input/                            # ← 新規
│   │   │       ├── mod.rs
│   │   │       ├── mode.rs                       # InputMode, ModeOrigin
│   │   │       └── context.rs                    # InputContext 状態機械
│   │   └── tests/
│   │       ├── romaji_golden.rs
│   │       ├── mode_golden.rs                    # ← 新規
│   │       └── fixtures/
│   │           ├── romaji_cases.tsv
│   │           ├── mode_cases.tsv                # ← 新規
│   │           └── mode_cases_karukan_diff.tsv   # ← 新規
│   └── kotoha-cli/
│       ├── Cargo.toml
│       └── src/
│           └── bin/
│               └── romaji.rs
├── docs/
│   ├── superpowers/{specs,plans}/
│   ├── wiki/
│   ├── adr/                                      # 0001, 0002, 0003 を Phase 0 で作成
│   ├── wbs/
│   └── ROADMAP.md
├── scripts/
│   └── phase0-smoke.sh                           # CLI 動作確認の smoke テスト
├── lefthook.yml
├── CLAUDE.md
├── LICENSE-MIT
├── LICENSE-APACHE
└── README.md
```

## 6. 依存 crate (Phase 0 のみ)

| crate | バージョン目安 | 用途 |
|---|---|---|
| `thiserror` | ~1.0 | Error 型定義 |
| `anyhow` | ~1.0 | アプリ層のエラー伝播 (CLI 側) |
| `tracing` | ~0.1 | 構造化ログ |
| `tracing-subscriber` | ~0.3 | ログ出力フォーマット |
| `clap` (derive) | ~4.5 | CLI パーサ (kotoha-cli のみ) |
| `proptest` | ~1.5 | プロパティテスト (dev-dependencies) |

Phase 0 では重量級 crate (llama-cpp-2, tokenizers, hf-hub, zbus 等) は導入しない。バージョンは実装時の最新 stable を使用し、Cargo.toml に明記する。

## 7. API スケッチ

### 7.1 kotoha-core 公開 API

```rust
// crates/kotoha-core/src/lib.rs
pub mod error;
pub mod kana;
pub mod romaji;
pub mod input;

pub use error::{Error, Result};
pub use input::{InputContext, InputMode, InputStep};
pub use romaji::{RomajiConverter, ConvertStep};
```

### 7.2 RomajiConverter (下位レイヤ、元の設計を維持)

```rust
// crates/kotoha-core/src/romaji/mod.rs
pub struct RomajiConverter {
    trie: Trie,
    buffer: String,
}

impl RomajiConverter {
    pub fn new() -> Self;

    /// 文字列をまとめて変換する。
    /// 戻り値は (確定したひらがな, まだ変換できていない未確定文字列)。
    pub fn convert(&self, input: &str) -> (String, String);

    /// 1 文字ずつ食わせるストリーム変換
    pub fn push(&mut self, ch: char) -> ConvertStep;

    /// 内部バッファをクリアする
    pub fn reset(&mut self);

    /// 未確定バッファを強制確定してひらがな化して返す (flush)
    pub fn flush(&mut self) -> String;
}

pub enum ConvertStep {
    Committed(String),
    Pending,
    Invalid(char),
}
```

### 7.3 InputMode

```rust
// crates/kotoha-core/src/input/mode.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum InputMode {
    /// ローマ字 → かな変換モード (デフォルト)
    Hiragana,
    /// 英字直接入力モード
    Direct,
}
```

### 7.4 ModeOrigin (内部用)

```rust
// crates/kotoha-core/src/input/mode.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ModeOrigin {
    /// 初期状態、または明示トグルによる Sticky モード
    Sticky,
    /// Shift トリガによる Transient モード
    /// (未確定バッファが空になったらデフォルトに戻る)
    Transient,
}
```

### 7.5 InputContext

```rust
// crates/kotoha-core/src/input/context.rs
pub struct InputContext {
    mode: InputMode,
    origin: ModeOrigin,
    converter: RomajiConverter,   // Hiragana モード時の未確定バッファを内包
    direct_buffer: String,        // Direct モード時の英字バッファ
    allow_transient_to_sticky_promotion: bool,  // Phase 3 以降で設定化
}

impl InputContext {
    pub fn new() -> Self;

    pub fn mode(&self) -> InputMode;

    /// 現在の未確定バッファ (プリエディット表示用)
    pub fn preedit(&self) -> String;

    /// 1 文字入力。
    /// - 小文字 / 数字 / 記号 → 現在モードのルールで処理
    /// - 大文字 (ASCII uppercase) → Shift トリガ扱い。
    ///   Hiragana モードから呼ばれたら (Direct, Transient) へ遷移し、
    ///   その大文字を direct_buffer に追加する。
    pub fn input_char(&mut self, ch: char) -> InputStep;

    /// Enter 相当。未確定バッファを確定して返す。
    /// Transient モードだった場合は (Hiragana, Sticky) に自動復帰。
    pub fn commit(&mut self) -> String;

    /// Escape 相当。未確定バッファを破棄。Transient なら Hiragana に復帰。
    pub fn cancel(&mut self);

    /// 明示的モードトグル (将来 Phase 3 で半角/全角キー等にマッピング)
    /// - Hiragana/Sticky ⇄ Direct/Sticky
    /// - Direct/Transient → Direct/Sticky への昇格
    pub fn toggle_mode(&mut self);

    /// 明示的モード設定。常に Sticky origin になる。
    pub fn set_mode(&mut self, mode: InputMode);

    /// 全状態リセット (バッファクリア + Hiragana / Sticky へ)
    pub fn reset(&mut self);
}
```

### 7.6 InputStep

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum InputStep {
    /// バッファに追加された(未確定のまま)
    Preedit,
    /// この入力でかな / 英字の一部が確定した (例 "n" + "a" → "な" が確定)
    Committed(String),
    /// ルール外の文字
    Invalid(char),
}
```

### 7.7 Kana ユーティリティ

```rust
// crates/kotoha-core/src/kana/mod.rs
pub fn is_hiragana(ch: char) -> bool;
pub fn is_katakana(ch: char) -> bool;
pub fn hiragana_to_katakana(s: &str) -> String;
pub fn katakana_to_hiragana(s: &str) -> String;
```

### 7.8 Error 型

```rust
// crates/kotoha-core/src/error.rs
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    #[error("invalid character: {0:?}")]
    InvalidCharacter(char),

    #[error("internal state inconsistency")]
    InvalidState,
}

pub type Result<T> = std::result::Result<T, Error>;
```

## 8. モード管理仕様

### 8.1 状態定義

状態は `(mode, origin)` のタプルで表現する。取り得る組み合わせは 3 つのみ:

| 状態 | 意味 |
|---|---|
| `(Hiragana, Sticky)` | **初期状態**。ローマ字→かな変換 |
| `(Direct, Transient)` | Shift トリガで入った一時英字モード |
| `(Direct, Sticky)` | 明示トグルで入った持続英字モード |

`(Hiragana, Transient)` は定義上あり得ない(Transient は Direct への遷移のみ記述する)。

### 8.2 遷移表

| 現状態 | 小文字 a-z | 大文字 A-Z | 数字 / 記号 | Enter (commit) | Esc (cancel) | toggle_mode |
|---|---|---|---|---|---|---|
| `(Hiragana, Sticky)` | ローマ字→かな変換 | `(Direct, Transient)` へ、大文字英字として追加 | ルール表通り (- → ー 等) | バッファ確定 → 現状態維持 | バッファ破棄 → 現状態維持 | `(Direct, Sticky)` へ |
| `(Direct, Transient)` | 小文字英字として追加 (Shift なくても英字) | 大文字英字として追加 | そのまま追加 | バッファ確定 → `(Hiragana, Sticky)` に自動復帰 | バッファ破棄 → 自動復帰 | `(Direct, Sticky)` に昇格 |
| `(Direct, Sticky)` | 小文字英字として追加 | 大文字英字として追加 | そのまま追加 | バッファ確定 → 現状態維持 (戻らない) | バッファ破棄 → 現状態維持 | `(Hiragana, Sticky)` へ |

### 8.3 重要なルール

1. **Shift (大文字) は常に Direct を駆動する**
   - ひらがなモード中でも例外なく `(Direct, Transient)` に遷移する。これが「Shift 押下時は英字入力が決定的」の保証
2. **Transient → Hiragana は "バッファ空" で自動復帰**
   - Enter 確定 / Esc キャンセルが契機。Shift を離しただけでは戻らない
3. **Sticky Direct 中は Shift 不要**
   - 小文字も全部英字として追加。大小は Shift の物理状態(= 入力文字の大小)に従うだけ
4. **Transient → Sticky は昇格のみ**
   - Transient Direct 中に `toggle_mode()` を呼ぶと、`(Direct, Sticky)` に昇格する(以降は確定しても戻らない)

### 8.4 擬似コード

```rust
fn input_char(&mut self, ch: char) -> InputStep {
    let is_shift = ch.is_ascii_uppercase();

    // ひらがなモード中に大文字 → Transient Direct へ遷移
    if self.mode == InputMode::Hiragana && is_shift {
        self.mode = InputMode::Direct;
        self.origin = ModeOrigin::Transient;
    }

    match self.mode {
        InputMode::Hiragana => self.converter.push(ch),
        InputMode::Direct => {
            self.direct_buffer.push(ch);
            InputStep::Preedit
        }
    }
}

fn commit(&mut self) -> String {
    let result = match self.mode {
        InputMode::Hiragana => self.converter.flush(),
        InputMode::Direct => std::mem::take(&mut self.direct_buffer),
    };

    // Transient だった場合のみ Hiragana に自動復帰
    if self.origin == ModeOrigin::Transient {
        self.mode = InputMode::Hiragana;
        self.origin = ModeOrigin::Sticky;
    }
    result
}

fn toggle_mode(&mut self) {
    self.mode = match self.mode {
        InputMode::Hiragana => InputMode::Direct,
        InputMode::Direct => {
            if self.origin == ModeOrigin::Transient
                && self.allow_transient_to_sticky_promotion
            {
                // Transient → Sticky への昇格
                self.origin = ModeOrigin::Sticky;
                return;
            }
            InputMode::Hiragana
        }
    };
    self.origin = ModeOrigin::Sticky;
}
```

### 8.5 Karukan との差分

- **Karukan**: Shift 由来のモードは Sticky 相当で、ユーザが明示的に戻さない限り持続 → 使い手の期待を裏切る
- **Kotoha**: Shift 由来は Transient。Enter 確定で `(Hiragana, Sticky)` に自動復帰 → 使い手が "1 確定ごとに意識せずモードが戻る" 体験

この判断は `docs/adr/0001-input-mode-transient-vs-sticky.md` に記録する。

## 9. 変換ルール

Phase 0 でカバーするローマ字変換規則:

- 基本 50 音 (a i u e o, ka ki ku ke ko, ...)
- 濁音 (ga gi gu ge go, za zi zu ze zo, ...)
- 半濁音 (pa pi pu pe po)
- 拗音 (kya kyu kyo, sha shu sho, ...)
- 促音 (kka → っか, ssa → っさ, ...)
- 撥音 (n, nn, n')
- 長音 (- → ー)
- ヘボン式の異体 (shi/si, tsu/tu, chi/ti, fu/hu, ji/zi, ...)
- 拡張音 (va vi ve vo, kwa, gwa, tsa tse tso, ...)
- 記号 (-, !, ?, ., ...)

具体的なルール表は `src/romaji/rules.rs` に `const RULES: &[(&str, &str)]` の形で宣言する。初期ルールセットは Karukan の `karukan-engine/src/romaji/rules.rs` から移植する (MIT/Apache-2.0 ライセンス下で可能)。

### 9.1 ローマ字入力 convention 注記

本 IME の rule table は Karukan 互換の 1:1 打鍵 → かな mapping を採用する。particle の「は」は `ha` キー、「を」は `wo` キー、「へ」は `he` キーで入力する(Mozc / Google IME / MS-IME と同じ typewriter-style convention)。

したがって、挨拶「こんにちは」を入力するには `konnnichiha` と打鍵する(`nn` で ん の hatsuon を 1 度挟み、`ha` で particle の は を出す)。literal な `konnichiwa` は rule-faithful に `こんいちわ` と変換される(`nn` rule で ん が 2 文字消費、`wa` で わ がそのまま出る)。

particle の位置を見て `wa` → `は` に変換する heuristic は、かな漢字変換層(Phase 1 以降)の責務とする。Phase 0 の `RomajiConverter` は heuristic-free な pure rule transformer とする。

### 9.2 非 ASCII 入力の取り扱い

`StateMachine::push` は先頭ガードにより非 ASCII 文字 (`ch.is_ascii() == false`) を受け取ると、内部バッファを変更せず `PushResult::Invalid(ch)` を返す。`RomajiConverter::convert` / `push` / `flush` はこの挙動を継承する。

この決定の normative 根拠と代替選択肢は `docs/adr/0001-non-ascii-retraction-policy.md` を参照のこと。要約:

- `RomajiConverter::convert` は「ASCII ローマ字 → かな」の変換関数であり、非 ASCII 入力は契約外である。
- かな出力を再度 `convert` に通しても、非 ASCII 文字はすべて drop されるため、新たな romaji pending は生成されない (property test `prop_idempotence_on_committed` の弱化形不変条件)。
- 強い retraction `convert(convert(x).committed).committed == convert(x).committed` は契約外とし、上位層 (かな漢字変換層) でもこの再入力を前提とした設計は避ける。

## 10. CLI 仕様

Phase 0 の CLI `kotoha-romaji` は動作確認とデバッグが目的である。モード切替ロジック全体の網羅的検証は golden test / unit test に委ね、CLI は最小限の入出力ツールに徹する。

### 10.1 コマンド

```
kotoha-romaji [OPTIONS]

OPTIONS:
    -m, --mode <MODE>    初期モード [default: hiragana]
                         可能な値: hiragana | direct
    --show-mode          各行出力の末尾に現在モードを [H] / [D] で表示
    -h, --help
    -V, --version
```

### 10.2 動作仕様

- 標準入力から **行単位** で読み込み、**行末を Enter (commit)** として扱う
- 大文字入力は Shift トリガ扱いで Transient Direct に遷移
- 行処理後、commit による **Transient → Hiragana の自動復帰** が発生
- 確定文字列を 1 行 1 出力で標準出力へ書き出す

### 10.3 使用例

```bash
# 基本: ひらがな変換
$ echo "konnnichiha" | kotoha-romaji
こんにちは

# Shift トリガ: 大文字入力で Transient Direct
$ echo "HELLO" | kotoha-romaji
HELLO

# 混在: Shift 後 Enter で自動復帰
$ printf "Konnichiwa\nkonnnichiha\n" | kotoha-romaji
Konnichiwa
こんにちは

# Sticky Direct (明示トグル相当、--mode direct 起点)
$ printf "hello\nworld\n" | kotoha-romaji --mode direct
hello
world

# モード表示 (デバッグ用)
$ printf "Hi\nkon\n" | kotoha-romaji --show-mode
Hi [H]
こん [H]

$ echo "hi" | kotoha-romaji --mode direct --show-mode
hi [D]
```

### 10.4 Phase 0 では実装しないもの

- インタラクティブ REPL (`:toggle` 等の対話コマンド)
- 行途中の明示トグル
- キーコード模擬 (raw キーイベント流し込み)

これらは Phase 3 (IBus engine) 以降で必要になったら追加する。

### 10.5 終了コード

| コード | 意味 |
|---|---|
| 0 | 正常終了 |
| 1 | 不正な入力(読み込み失敗、不正な `--mode` 値) |
| 2 | 内部エラー(想定外の `Error::InvalidState` 等) |

## 11. テスト戦略

### 11.1 単体テスト (50 件以上)

既存 30 件 + `InputContext` 関連 20 件追加。カバー対象:

| カテゴリ | ケース数 | 内容 |
|---|---|---|
| 初期状態 | 2 | `new()` で `(Hiragana, Sticky)`、`mode()` / `preedit()` の初期値 |
| Hiragana モードの `input_char` | 3 | 小文字・記号・ルール外文字 |
| Shift トリガ (Transient) | 4 | 大文字入力で遷移、Transient 中の小文字追加、混在入力、`preedit()` の確認 |
| 明示トグル (Sticky) | 3 | `toggle_mode()` の往復、`set_mode()` の明示指定 |
| Transient → Sticky 昇格 | 2 | Transient 中の `toggle_mode()` で Sticky 化 |
| `commit` / `cancel` | 4 | 各状態からの復帰挙動 |
| `reset` | 2 | バッファクリア + 初期状態復帰 |

### 11.2 Golden テスト

**既存**: `tests/fixtures/romaji_cases.tsv` (200 ケース) を維持。

**新規**: `tests/fixtures/mode_cases.tsv` (70 ケース)

#### TSV フォーマット

```
# columns: initial_mode TAB input TAB expected_output TAB final_mode
# input / expected_output 内の改行は \n でエスケープ
hiragana	konnnichiha	こんにちは	hiragana
hiragana	HELLO	HELLO	hiragana
hiragana	Konnichiwa\nkonnnichiha	Konnichiwa\nこんにちは	hiragana
direct	hello	hello	direct
direct	hello\nworld	hello\nworld	direct
```

- 1 行 = 1 テストケース
- `input` 内の `\n` = `commit()` 相当(行 Enter)
- `final_mode` は最終行処理後のモード

#### カバレッジ目標 (70 ケース)

| カテゴリ | ケース数 |
|---|---|
| Sticky Hiragana 基本 | 10 |
| Shift トリガ単発 | 15 |
| Transient 自動復帰 | 10 |
| Sticky Direct 持続 | 10 |
| トグル遷移 | 10 |
| Transient → Sticky 昇格 | 5 |
| エッジケース (空行、単文字、長文、記号混在) | 10 |

#### Karukan 差分 TSV

`tests/fixtures/mode_cases_karukan_diff.tsv` (10 ケース)

Karukan の既存挙動と Kotoha で結果が異なる入力パターンを並べ、コメントで「なぜ Kotoha はこう振る舞うか」を記載する。将来 Karukan 挙動を再現するオプションを足したくなった時の参考にもなる。

### 11.3 プロパティテスト (6 条件)

既存 2 条件(冪等性 / 結合性)に加えて、モード系の不変条件を `proptest` で検証する。

なお、revision 1 では可逆性(invertibility)も "既存" プロパティとして列挙していたが、romaji → かな は多対一(`ji`/`zi` → じ、`tu`/`tsu` → つ、`si`/`shi` → し、`ti`/`chi` → ち 等)であり strict な round-trip invertibility は数学的に定義不能なため、Phase 0 のプロパティ集合からは除外する。canonical romaji を定義して部分的に可逆性を回復する拡張は Phase 1 以降で検討する(ADR 候補)。

| 不変条件 | 意味 |
|---|---|
| Hiragana/Sticky の安定性 | `(Hiragana, Sticky)` 状態で任意入力 + `commit` を繰り返しても状態は `(Hiragana, Sticky)` のまま |
| Transient の必ず復帰 | `(Direct, Transient)` からは任意文字列 + `commit` 後、必ず `(Hiragana, Sticky)` に戻る |
| Sticky Direct の持続 | `(Direct, Sticky)` で任意入力 + `commit` を繰り返しても `(Direct, Sticky)` を維持 |
| `reset` の冪等性 | `reset()` を任意回呼んでも最終状態は `(Hiragana, Sticky)` + バッファ空 |

### 11.4 テスト総数と実行時間見積もり

| 種別 | 拡張後 | 実行時間目安 |
|---|---|---|
| 単体テスト | 50 件以上 | < 1 秒 |
| Golden テスト | 270 ケース以上 + Karukan 差分 10 ケース | < 2 秒 |
| プロパティテスト | 6 条件 | < 5 秒 (proptest デフォルト 256 反復) |

`cargo test --workspace` 全体で 10 秒以内に収まる想定。lefthook pre-push での実行負荷は許容範囲。

## 12. 品質ゲート

### 12.1 ローカル (lefthook)

pre-commit:

- `cargo fmt --all --check`
- ドキュメントファイル名の命名規則チェック (`scripts/pre-commit-doc-naming.sh`)

pre-push:

- `cargo build --workspace`
- `cargo clippy --workspace -- -D warnings`
- `cargo test --workspace`

### 12.2 CI

Kotoha はローカル実行のデスクトップアプリケーション(日本語 IME)であり、クラウドへのデプロイは発生しない。したがって CI/CD システムは導入せず、品質ゲートはローカル lefthook のみで担保する。配布は `.deb` / `.rpm` / Flatpak / AUR 等のパッケージングで行う(Phase 5 以降で詳細化)。

global CLAUDE.md の CI/CD Policy(GitHub Actions 禁止 + GCP Cloud Build Trigger で Deploy)は、クラウドデプロイを前提とする他プロジェクト向け方針である。Kotoha はローカル実行アプリのため GCP は関与しない(言及があれば削除対象)。

## 13. 成功基準 (Phase 0 完了条件)

### 13.1 ビルド・テスト系 (lefthook 自動検証)

- [ ] `cargo build --workspace` PASS
- [ ] `cargo test --workspace` PASS (golden test 270 ケース以上 + Karukan 差分 10 ケース含む)
- [ ] `cargo clippy --workspace -- -D warnings` 警告ゼロ
- [ ] `cargo fmt --all --check` OK
- [ ] lefthook pre-commit / pre-push 動作確認

### 13.2 CLI 手動確認

ローマ字→かな変換 (既存):

- [ ] `echo "konnnichiha" | kotoha-romaji` → `こんにちは`
- [ ] `echo "tsumugi" | kotoha-romaji` → `つむぎ`
- [ ] `echo "n'ya" | kotoha-romaji` → `んや`
- [ ] `echo "nya" | kotoha-romaji` → `にゃ`

モード管理 (新規):

- [ ] **Shift トリガ**: `echo "HELLO" | kotoha-romaji` → `HELLO`
- [ ] **Transient 自動復帰**: `printf "Ko\nkonnnichiha\n" | kotoha-romaji` が 1 行目 `Ko`、2 行目 `こんにちは`
- [ ] **Sticky Direct 持続**: `printf "hello\nworld\n" | kotoha-romaji --mode direct` が両行とも英字
- [ ] **混在**: `printf "Konnichiwa\nkonnnichiha\n" | kotoha-romaji` が `Konnichiwa\nこんにちは`
- [ ] **モード表示**: `echo "Hi" | kotoha-romaji --show-mode` が `Hi [H]`
- [ ] **Sticky モード表示**: `echo "hi" | kotoha-romaji --mode direct --show-mode` が `hi [D]`

上記 CLI 手動確認を一括実行する `scripts/phase0-smoke.sh` を用意する。WBS には実行結果をログ化する。

### 13.3 ドキュメント成果物

- [ ] `docs/adr/0001-input-mode-transient-vs-sticky.md` (Transient/Sticky 2 軸モデルの根拠)
- [ ] `docs/adr/0002-shift-via-uppercase-char.md` (Shift = 大文字表現の限界と採用根拠)
- [ ] `docs/adr/0003-cli-line-based-commit.md` (行単位 commit 抽象化の根拠)
- [ ] 本設計書末尾に付録 "Phase 3 想定インタフェース" 記載済み
- [ ] `docs/ROADMAP.md` に Phase 1〜6 の骨組み記載 (Phase 5 から「Shift 挙動設定」除去済み)
- [ ] `docs/wbs/2026-04-XX-feature-NN-kotoha-phase-0.md` に実装ログ

### 13.4 テスト成果物

- [ ] `tests/fixtures/mode_cases.tsv` 70 ケース以上
- [ ] `tests/fixtures/mode_cases_karukan_diff.tsv` 10 ケース以上
- [ ] `InputContext` 単体テスト 20 件以上
- [ ] プロパティテスト 4 条件追加(モード系不変条件、総数は §11.3 参照: 既存 2 + モード系 4 = 6 条件)

## 14. 工数目安

| 作業 | 目安 |
|---|---|
| プロジェクト初期化 (workspace, templates, lefthook) | 0.5 日 |
| GitHub repo 作成 + 初期 push | 0.2 日 |
| `kotoha-core` スケルトン + error モジュール | 0.3 日 |
| `kana` モジュール | 0.5 日 |
| `romaji` モジュール (trie + state machine) | 2 日 |
| ローマ字ルール表 + Karukan から fixture 移植 | 1 日 |
| **`input` モジュール (新規)** | **1.5〜2 日** |
| golden test + property test (mode_cases + Karukan 差分 + property 拡張含む) | 1.5 日 |
| `kotoha-cli` (`kotoha-romaji` コマンド、mode オプション含む) | 0.8 日 |
| **ADR 作成 (0001, 0002, 0003)** | **0.3 日** |
| ドキュメント整備 (ROADMAP, ARCHITECTURE, scripts/phase0-smoke.sh 等) | 0.7 日 |
| 合計 | **約 9〜10 日** |

実際の着手は週末中心になる想定のため、暦上は 3〜4 週間を見込む。

## 15. リスクと対応

| # | リスク | 影響 | 対応 |
|---|---|---|---|
| 1 | ローマ字変換の規則が Mozc/Karukan と微妙に異なる | 後続フェーズとの突き合わせが困難に | Karukan の規則を初期値とし、差分を ADR に記録 |
| 2 | proptest の不変条件選定を誤る | false negative / positive | Phase 1 実装時に必要に応じて見直す |
| 3 | lefthook の pre-push が重すぎて開発速度低下 | 開発フィードバック遅延 | 必要に応じて short モードでのテスト並列化 |
| 4 | `--no-verify` で hook を無効化する誘惑 | 品質ゲート無力化 | CLAUDE.md に禁止事項として明記、例外は record |
| 5 | 状態遷移の組み合わせ爆発で遷移漏れ | バグ温床、稀遷移で一貫性崩壊 | 設計書の状態遷移表を仕様の "正本" として扱い、golden test と property test で網羅。`input/context.rs` のコメントから本設計書の遷移表を参照 |
| 6 | Phase 3 IBus 統合時に `InputContext` API が IBus のキーイベント形と噛み合わない | Phase 3 で API 破壊変更が必要に | 本設計書末尾に付録 "Phase 3 想定インタフェース" を記載。Phase 0 着手前に 1 度レビュー |
| 7 | Mozc / Google 日本語入力との挙動差分(境界ケース) | ユーザの体験期待とのギャップ | Phase 0 時点では Mozc 互換を保証しない。golden test で "Kotoha の定義" を正本とし、Mozc との差分測定は Phase 1 以降(評価サーバ整備後)に回す |
| 8 | Transient → Sticky 昇格ルールが実使用で不評 | 使い手が想定外の持続化を経験 | `InputContext::allow_transient_to_sticky_promotion: bool` を設定として用意。Phase 0 では `true` 固定、Phase 3 以降で設定 UI 化 |

## 16. Phase 1 への橋渡し

Phase 0 完了時点で以下が Phase 1 の前提として利用可能になる:

- 確立された Cargo workspace
- `kotoha-core::romaji::RomajiConverter` の安定 API
- **`kotoha-core::input::InputContext` の安定 API** (モード管理の基礎)
- `kotoha-core::kana` ユーティリティ
- Golden test の仕組み (かな→漢字評価にも流用可能)
- lefthook による品質ゲート

Phase 1 では `kotoha-core` に以下を追加する:

- `kanji/` モジュール (llama-cpp-2 経由で Zenz GGUF 推論)
- `kanji/backend.rs` と `kanji/hf_download.rs`
- `InputContext` の Hiragana モードで「確定後のかな → 漢字候補生成」をつなぐインタフェース
- CLI に `kotoha-kanji` サブコマンド追加

## 17. 付録: Phase 3 想定インタフェース

リスク #6 (IBus 統合時の API ミスマッチ) への予防策として、Phase 3 の IBus engine 層が `InputContext` をどう呼び出すかの想定を先出しで記録する。Phase 0 ではこれを実装せず、API 設計の妥当性検証に用いる。

### 17.1 IBus キーイベント → InputContext 呼び出しの典型マッピング

```
IBusKeyEvent { keycode: KEY_a,        state: 0         } → input_char('a')
IBusKeyEvent { keycode: KEY_a,        state: SHIFT     } → input_char('A')
IBusKeyEvent { keycode: KEY_Return,   state: 0         } → commit() の結果をコミット
IBusKeyEvent { keycode: KEY_Escape,   state: 0         } → cancel()
IBusKeyEvent { keycode: KEY_zenkaku,  state: 0         } → toggle_mode()
IBusKeyEvent { keycode: KEY_muhenkan, state: 0         } → (ユーザ設定次第で toggle_mode)
```

### 17.2 IBus preedit 表示更新

各キーイベント処理後、`InputContext::preedit()` の返り値を IBus の preedit 文字列として更新する。モード表示は IBus status area に `mode()` の返り値をマップして表示する(Phase 3 で仕様を確定)。

### 17.3 Phase 3 への申し送り

- `InputContext` API に `KeyEvent` / `InputAction` の抽象型は持たせない方針 (YAGNI)。IBus engine 層側で IBus キーイベント → `input_char` / `commit` / `cancel` / `toggle_mode` への変換を実装する
- この変換層こそが Karukan における「ハードコード」に相当する部分。Kotoha では設定可能なキーマップテーブルで駆動する(Phase 3 で詳細設計)

## 18. 参考

- Karukan (togatoga/karukan) — Phase 0 の設計上、romaji rules の初期値と Shift 挙動の反面教師として参照
- AzooKey/AzooKeyKanaKanjiConverter — Swift 実装だが API 設計の参考
- Mozc — 変換評価セットの参考、Shift 挙動の模範

## 19. 変更履歴

- **2026-04-22 (revision 1, 初稿)**: 初稿作成。Phase 0 のスコープは「Cargo workspace + ローマ字→かな変換コア + CLI」のみ。Shift 挙動の設定は Phase 5 に配置
- **2026-04-22 (revision 2, 更新版)**: 入力モード管理を Phase 0 に前倒し追加。理由は Karukan の Shift 挙動(Enter 確定後もモード自動復帰しない)が本プロジェクトの主要動機であり、Phase 0 の API 設計に最初から組み込むべきと判断したため。主な変更:
  - `input` モジュール (`InputMode` / `ModeOrigin` / `InputContext` / `InputStep`) を `kotoha-core` 公開 API に追加
  - モード管理仕様 (§8) を新設。状態遷移は `(Hiragana, Sticky)` / `(Direct, Transient)` / `(Direct, Sticky)` の 3 状態機械
  - CLI 仕様 (§10) を新設。`--mode` / `--show-mode` オプション、行単位 commit 抽象化
  - Golden テストを 200 → 270 ケースに拡張、Karukan 差分 10 ケースを新設
  - プロパティテストを 2 → 6 条件に拡張(revision 1 の "可逆性" は多対一性のため除外、詳細は §11.3)
  - ADR 3 件 (0001, 0002, 0003) の作成を Phase 0 完了条件に追加
  - 付録 "Phase 3 想定インタフェース" を新設 (リスク #6 対応)
  - Phase 5 から「Shift 挙動設定」を除去
  - 工数目安 6〜7 日 → 9〜10 日 (暦上 2〜3 週 → 3〜4 週)
