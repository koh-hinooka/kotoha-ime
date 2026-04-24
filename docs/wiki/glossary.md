# Kotoha Glossary — Ubiquitous Language

本ファイルは Kotoha プロジェクト (GNOME Wayland ネイティブに動作する自作日本語 IME) で使用するドメイン固有語彙を集約する。Phase 0 時点では stub 状態であったが、Phase 0 (romaji 変換 / 入力モード) + Phase 1 (kana→kanji 変換基盤 + LLM 推論) + Phase 5 foundation (custom model 方針 + P5-A data pipeline PoC) を経て累積したドメイン用語を、P1-4 wrap の一環として一括整備した実用版に置き換えた。

本ファイルの目的は 2 点である。第 1 に、specs / plans / ADR / コード / commit message / PR 本文 / WBS の全ドキュメント層で用語を一意に保ち、同一概念に対する別名 (例: 「モデル切替キー」と「input mode toggle key」) の並立を防ぐことである。第 2 に、コード上の identifier (型名・モジュール名・feature flag 名) とドメイン語彙の対応を明示することで、実装者が「この概念はコード上ではどの型で表現されているか」を即時に辿れるようにすることである。

以後、Kotoha の specs / plans / ADR / コードで新規ドメイン用語を導入する際は、本 glossary.md にも同時に追記する運用を確立する (global rule `~/.claude/rules/glossary-consistency.md` 準拠)。運用詳細は末尾の「更新規約」節に定める。

## 目次

- [1. 日本語入力の基礎用語](#1-日本語入力の基礎用語)
- [2. 入力モードとキーイベント](#2-入力モードとキーイベント)
- [3. ローマ字変換](#3-ローマ字変換)
- [4. かな→漢字変換の基盤](#4-かな漢字変換の基盤)
- [5. LLM 推論とプロンプト](#5-llm-推論とプロンプト)
- [6. テスト・検証](#6-テスト検証)
- [7. Phase 5 (custom model) 用語](#7-phase-5-custom-model-用語)
- [8. 参考実装・関連エコシステム](#8-参考実装関連エコシステム)
- [9. ツール・環境](#9-ツール環境)
- [更新規約](#更新規約)

## 1. 日本語入力の基礎用語

### ローマ字 (Romaji)

- **定義**: 日本語の音節を Latin アルファベット列で表記した形式。Kotoha の入力層は romaji を第一入力境界として受け取る。
- **初出**: Phase 0 design spec §5 (`docs/superpowers/specs/2026-01-XX-kotoha-phase-0-design.md` に相当する Phase 0 設計文書)
- **対応する identifier**: `crates/kotoha-core/src/romaji`
- **備考**: 日本語音韻のうち半分程度に Hepburn / Kunrei / waapuro の 3 方式の異体表記が存在する点を取り扱うため、Kotoha は canonical romaji を採用しない方針を取った (ADR 0008)。

### かな (Kana)

- **定義**: ひらがな (Hiragana) とカタカナ (Katakana) の総称。日本語の音節文字 (syllabary)。
- **初出**: Phase 0 design spec §5
- **対応する identifier**: Phase 0 では romaji→hiragana 変換の出力として、Phase 1 では kana→kanji 変換の入力として扱う。
- **備考**: 外来語表記は通常カタカナを用いるが、Phase 0 / Phase 1 時点では romaji→hiragana の一方向変換のみを実装し、カタカナ化は backend / 変換候補側の責務として切り分けている。

### ひらがな (Hiragana)

- **定義**: 日本語音節文字の 1 体系。主に和語・助詞・活用語尾の表記に使用する。
- **初出**: Phase 0 design spec §5
- **対応する identifier**: `InputMode::Hiragana` variant がひらがな入力モードを表す。
- **備考**: Phase 0 の入力モード enum の 2 値のうち 1 つ。もう 1 つは `InputMode::Direct` (ASCII 直接入力)。

### カタカナ (Katakana)

- **定義**: 日本語音節文字の 1 体系。主に外来語・擬音語・強調表記に使用する。
- **初出**: Phase 0 design spec §5
- **対応する identifier**: Phase 0 / Phase 1 時点ではカタカナ専用の入力モードを設けず、ひらがな→カタカナ変換は kanji backend 側で行う (ADR 0009 D4 参照)。
- **備考**: ADR 0009 D4 にて「hiragana→katakana preprocessing を backend 側で行わない」と決定したため、入力層ではひらがな統一で扱う。

### 促音 (Sokuon)

- **定義**: 「っ」「ッ」で表記される重子音 (geminated consonant)。例: きっぷ (切符) → `kippu`。
- **初出**: Phase 0 design spec §5
- **対応する identifier**: `crates/kotoha-core/src/romaji` の trie で「子音 2 連続 → 「っ」+ 子音 1」の規則として扱う。
- **備考**: 例外として「nn」→「ん」は撥音 (Hatsuon) として扱い、促音規則の対象外とする。

### 拗音 (Yōon)

- **定義**: 「ゃ」「ゅ」「ょ」の小文字かなと直前の -i 段かなが結合して 1 音節を形成する日本語音韻。例: しゃ (sha / sya) / きょ (kyo) / ちゅ (chu / tyu)。
- **初出**: Phase 0 design spec §5
- **対応する identifier**: `crates/kotoha-core/src/romaji` の trie に Hepburn / Kunrei 両方の綴りを収録する。
- **備考**: 拗音の綴りは Hepburn (sha / shu / sho) と Kunrei (sya / syu / syo) で異なり、Kotoha は両者を trie に並行して収録する。

### 撥音 (Hatsuon)

- **定義**: 「ん」「ン」で表記される音節鼻音 (moraic nasal)。例: しんぶん (新聞) → `shinbun`。
- **初出**: Phase 0 design spec §5
- **対応する identifier**: `crates/kotoha-core/src/romaji` の trie で「n」単独 + 「nn」の 2 綴りで扱う。
- **備考**: 「nn」は入力確定時に「ん」に畳み込むため、促音「っ」との区別 (「kanna」は「かんな」であり「かっな」ではない) を trie ルールで明示する。

### 長音 (Chōon)

- **定義**: 長母音を表す日本語音韻。カタカナでは「ー」記号、ひらがなでは母音の連続 (例: こうえん、えいが) で表記する。ローマ字では母音 2 重 (例: `koohii` = コーヒー)。
- **初出**: Phase 0 design spec §5
- **対応する identifier**: `crates/kotoha-core/src/romaji` の trie で母音 2 連続を長音として扱う規則。
- **備考**: ローマ字表記で `o+u` と `o+o` の区別が曖昧になる (例: おう / おお) が、Phase 0 は綴り通りに扱い canonical 化しない (ADR 0008 方針)。

## 2. 入力モードとキーイベント

### InputMode (入力モード)

- **定義**: Kotoha が保持する入力変換方式の状態。現時点では `InputMode::Hiragana` (ひらがな入力 → romaji→kana 変換を行う) と `InputMode::Direct` (ASCII 直接入力 → 変換を行わない) の 2 値列挙。
- **初出**: Phase 0 design spec §9
- **対応する identifier**: `crates/kotoha-core/src/input/mode.rs` (`InputMode` enum)
- **備考**: 2 値列挙は Phase 0 〜 Phase 4 の最小構成である。カタカナ入力モード / 全角英数モード等の追加可否は Phase 3 IBus 統合以降で再検討する。

### Transient モード

- **定義**: Shift trigger (Shift + 英字キー) 由来で一時的に `InputMode::Direct` に遷移し、Enter 確定後に `InputMode::Hiragana` に自動復帰する挙動方針。
- **初出**: ADR 0002 (`docs/adr/0002-input-mode-toggle-policy.md`)
- **対応する identifier**: `crates/kotoha-core/src/input/mode.rs` の遷移ロジック
- **備考**: Kotoha が採用する方針。Karukan が採用する Sticky モード (モード切替キーで明示指示されたモードが持続) とは対照的であり、ADR 0002 で明示的に区別した。

### Sticky モード

- **定義**: モード切替キーで明示指示された入力モードが、次に再度切替キーが押下されるまで持続する挙動方針。
- **初出**: ADR 0002
- **対応する identifier**: Kotoha では実装されない (非採用)。
- **備考**: Kotoha では Sticky を採用せず、Transient モード (Shift trigger 由来、Enter 復帰) を唯一の短期モード切替手段とする。Karukan 実装との差分として ADR 0002 に記録した。

### Shift trigger (Shift 由来起動)

- **定義**: Shift キーを押下しながら英字キーを押すことで、現在 `InputMode::Hiragana` であっても `InputMode::Direct` に一時遷移する契機。
- **初出**: ADR 0002 / ADR 0003 (`docs/adr/0003-shift-trigger-direct-mode.md` 相当)
- **対応する identifier**: `crates/kotoha-core/src/input` のキーイベント処理
- **備考**: 大文字英字 (例: URL、変数名) 入力時に ASCII 直接入力へスムーズに切替える UX を目的とする。Enter 確定で Transient 復帰する設計のため、Shift trigger は Sticky 的な永続状態を引き起こさない。

### Preedit (プリエディット)

- **定義**: ユーザが入力中で未確定のテキストを表すバッファ。IME が UI 層に preedit 文字列を渡し、アプリケーション側が下線付き等で仮表示する。
- **初出**: Phase 0 spec §10.4
- **対応する identifier**: Phase 0 CLI では未実装。Phase 3 IBus 統合で `ibus-engine` API を経由して実装する。
- **備考**: Phase 0 〜 Phase 2 は CLI テスト driver のみで、preedit UI は Phase 3 で初めて対象になる。

## 3. ローマ字変換

### Romaji trie

- **定義**: ローマ字文字列から kana 列への変換を最長一致で行う trie データ構造。`OnceLock` による 1 回だけの初期化で静的に保持する。
- **初出**: ADR 0005 (`docs/adr/0005-romaji-trie-design.md` 相当) / `crates/kotoha-core/src/romaji`
- **対応する identifier**: `crates/kotoha-core/src/romaji` module
- **備考**: trie 構築は起動時 1 回で済むため、変換コストは入力文字数に比例する低コスト処理である。Hepburn / Kunrei / waapuro の異体表記を同一 trie に並置する。

### Canonical romaji

- **定義**: ローマ字の異体表記を 1 つの標準表記に正規化した形式 (例: `si` と `shi` を内部的に `shi` に統一する等)。
- **初出**: ADR 0008 (`docs/adr/0008-canonical-romaji-deferral.md` 相当)
- **対応する identifier**: Kotoha では実装されない (非採用)。
- **備考**: Kotoha は部分可逆性 (ユーザが打ったキーストロークを復元可能に保つ) を優先し canonical romaji を採用しない。Phase 5 custom model 学習データ生成時には 3 方式 (Hepburn / Kunrei / waapuro) を同時生成してデータ多様性を確保する (Phase 5 spec §4.2 / P5-A PoC)。

### Hepburn / Kunrei / waapuro (3 方式)

- **定義**: ローマ字の 3 大異体表記方式。Hepburn は外来向け (し = `shi`, ち = `chi`, つ = `tsu`)、Kunrei は学校教育準拠 (し = `si`, ち = `ti`, つ = `tu`)、waapuro は PC 入力慣習 (し = `shi`, ち = `chi`, つ = `tsu`, 小書き文字の前置 `x` / `l` 等)。
- **初出**: Phase 5 spec §4.2 / `tools/p5a-data-pipeline/src/kotoha_p5a/romaji.py`
- **対応する identifier**: `tools/p5a-data-pipeline/src/kotoha_p5a/romaji.py` の 3 方式並行生成関数
- **備考**: P5-A PoC では 1 つの kana sequence から 3 方式の romaji を同時生成して学習データ多様性を確保する。Phase 0 入力層の trie は Hepburn + Kunrei + waapuro の綴りを並置するため、ユーザ側で方式を意識する必要はない。

## 4. かな→漢字変換の基盤

### Kana→Kanji conversion (かな漢字変換)

- **定義**: ひらがな文字列を漢字交じり日本語 surface (例: 「きょうは」→「今日は」) に変換するタスク。Phase 1 の中核機能。
- **初出**: Phase 1 design spec (`docs/superpowers/specs/2026-04-24-kotoha-phase-1-design.md`)
- **対応する identifier**: `crates/kotoha-core/src/kanji` module
- **備考**: Phase 1 では LLM (Gemma-2-2B-jpn-it) による生成方式で実装。Phase 2 では辞書 + 学習キャッシュを追加、Phase 5 では Kotoha 専用 romaji-base model (task-specific fine-tune) による置換を計画。

### Backend trait

- **定義**: kanji backend の抽象 interface。method は `convert(input, options) -> Result<Vec<Candidate>, KanjiError>` と `model_id() -> &str`。
- **初出**: ADR 0011 (`docs/adr/0011-kanji-backend-trait-design.md`) / `crates/kotoha-core/src/kanji/backend.rs`
- **対応する identifier**: `KanjiBackend` trait (`crates/kotoha-core/src/kanji/backend.rs`)
- **備考**: `Box<dyn KanjiBackend>` による dynamic dispatch を採用し、enum dispatch よりテスト時の mock 差替えやすさを優先した (ADR 0011 D4)。

### BackendConfig

- **定義**: kanji backend 選択のための `#[non_exhaustive]` 属性付き enum。Phase 1 時点の variant は `Mock` と `LlamaCpp`。Phase 5 で `KotohaNative` variant を追加予定。
- **初出**: Phase 1 P1-1 (実装) / ADR 0011 (trait 設計) / ADR 0012 (feature flag 設計)
- **対応する identifier**: `BackendConfig` enum (`crates/kotoha-core/src/kanji/backend.rs`)
- **備考**: `#[non_exhaustive]` により、後続 Phase での variant 追加が breaking change にならない保証を持つ。
- `#[non_exhaustive]` の適用方針は ADR 0006 (non-exhaustive on streaming enums) に準拠する

### MockBackend

- **定義**: 決定論的な fixture-based backend。テスト用に GGUF model を load せず事前定義の入力→出力 map で応答する。
- **初出**: Phase 1 P1-1 / `crates/kotoha-core/src/kanji/mock.rs`
- **対応する identifier**: `MockBackend` struct (`crates/kotoha-core/src/kanji/mock.rs`)
- **備考**: Layer 1 (unit) / Layer 2 (integration, GGUF なし) のテスト層で使用。`llama-cpp` feature 非依存で常に build 可能。

### LlamaCppBackend

- **定義**: llama.cpp ライブラリ経由で GGUF model を実行する本番 kanji backend。Phase 1 の default backend。
- **初出**: P1-2.5 refactor (PR #74) / `crates/kotoha-core/src/kanji/llama_cpp.rs`
- **対応する identifier**: `LlamaCppBackend` struct (`crates/kotoha-core/src/kanji/llama_cpp.rs`)
- **備考**: P1-2 時点では `ZenzBackend` 名で実装されたが、P1-2.5 で Zenz 固有名を汎用名に rename し、他の GGUF モデル (Gemma-2-2B-jpn-it, 将来の互換 LLM) を `PromptTemplate` 差替で切替可能にした。

### Candidate

- **定義**: 1 つの変換候補を表す構造体。`surface: String` (漢字交じり表示文字列) に加え、将来の score / metadata 拡張余地を持つ。
- **初出**: Phase 1 P1-1 / `crates/kotoha-core/src/kanji`
- **対応する identifier**: `Candidate` struct (`crates/kotoha-core/src/kanji/mod.rs`)
- **備考**: Phase 1 では surface のみ使用。Phase 2 で score / source (辞書 or LLM) フィールドを追加予定。

## 5. LLM 推論とプロンプト

### PromptTemplate

- **定義**: 推論時の prompt 構築戦略を表す enum。variant は `Gemma2InstructChat` / `Qwen2Chat` / `Custom { system, user_wrapper: (String, String), assistant_prefix }` の 3 種 (`user_wrapper` は prefix / suffix のペアで、実装は `crates/kotoha-core/src/kanji/backend.rs` を参照)。
- **初出**: ADR 0011 / PR #74 (`docs/adr/0011-kanji-backend-trait-design.md`)
- **対応する identifier**: `PromptTemplate` enum (`crates/kotoha-core/src/kanji/llama_cpp.rs`)
- **備考**: Phase 1 は最初に `apply_chat_template` 経路で `Gemma2InstructChat` variant を試みたが conversational echo が発生したため、PR #76 で plain-text completion 経路に pivot した。variant 自体は将来の dispatch 拡張余地として実装に保持されており、現行 Phase 1 の prompt 構築は plain-text completion 経路を使用する。`Qwen2Chat` は将来の model 差替を想定した placeholder。

### GGUF

- **定義**: llama.cpp が消費する量子化済み model binary format。ggml 系列の後継で、tensor + metadata + tokenizer 情報を単一ファイルに格納する。
- **初出**: Phase 1 P1-2 / ADR 0009
- **対応する identifier**: `KOTOHA_LLAMA_MODEL_PATH` 環境変数で GGUF ファイルパスを指定する。
- **備考**: Kotoha は HuggingFace Hub から配布される GGUF ファイルを手動配置する運用を Phase 1 で採用 (将来 Phase 6 UX で AutoDownload UI を実装予定、MEMORY.md 参照)。

### Q5_K_M

- **定義**: llama.cpp の量子化方式の 1 つ。5-bit K-quantization の medium 精度 variant。Phase 1 default 量子化。
- **初出**: ADR 0009 / P1-2-9 WBS
- **対応する identifier**: default GGUF ファイル名に含まれる量子化識別子 (例: `gemma-2-2b-jpn-it-Q5_K_M.gguf`)。
- **備考**: Q4 (4-bit) より精度が高く、Q8 (8-bit) よりファイルサイズが小さい中庸の選択。ADR 0009 で Gemma-2-2B-jpn-it の Q5_K_M が default と決定された。

### Gemma-2-2B-jpn-it

- **定義**: Google DeepMind が公開した日本語 instruction-tuned 20 億パラメータ model。Phase 1 の default kanji backend model。
- **初出**: ADR 0009 (`docs/adr/0009-phase-1-default-model-selection.md`)
- **対応する identifier**: HuggingFace `google/gemma-2-2b-jpn-it` / GGUF 変換版を `KOTOHA_LLAMA_MODEL_PATH` に指定
- **備考**: ICL (In-context learning) による few-shot 制御で kana→kanji タスクに適用する。row 3「あした → 明日」の既知失敗は Phase 5 custom model で根本解消予定 (ADR 0009 D5 / ADR 0010)。

### ICL (In-context learning)

- **定義**: prompt 内に few-shot 例を注入することで、モデル重みの更新なしに task 適応させる手法。
- **初出**: PR #76 WBS
- **対応する identifier**: Phase 1 では `LlamaCppBackend::convert` の prompt 構築時に few-shot 例を埋め込む。
- **備考**: Phase 1 row 3「あした → 翌日」が「明日」に改善しない根本原因は、instruction-tuned Gemma-2-2B が「意味的に等価な言い換え」を優先する ICL 挙動にあると特定済。Phase 5 では task-specific fine-tune で ICL 依存を排除する。

### Few-shot prompting

- **定義**: prompt に N 組の input/output ペアを提示してから本番入力を与える手法。N=0 を zero-shot、N≥1 を few-shot と呼ぶ。
- **初出**: Phase 1 spec §6 / PR #76 WBS
- **対応する identifier**: `crates/kotoha-core/src/kanji/llama_cpp.rs` の prompt 構築箇所
- **備考**: Phase 1 は v12 prompt で 13 組の positive 例 + 3 組の negative 例 (anti-example) を採用。PR #76 で確定した構成。

### Greedy decoding

- **定義**: 各 token 選択で確率最大 (argmax) を常に選ぶ決定論的サンプリング方式。`temperature=0` + `seed=0` 相当。
- **初出**: Phase 1 spec §6
- **対応する identifier**: `crates/kotoha-core/src/kanji/llama_cpp.rs` の sampler 設定
- **備考**: Phase 1 の baseline 推論方式。Layer 3 smoke の再現性確保のため、非 greedy (top_p 等) は Phase 2+ の評価対象として保留する。

### Chat template

- **定義**: GGUF metadata `tokenizer.chat_template` に埋め込まれた、会話形式 prompt を組み立てるための Jinja2 風 template 文字列。`apply_chat_template` で展開する。
- **初出**: PR #74 / PR #76
- **対応する identifier**: llama.cpp の `apply_chat_template` API (Phase 1 現在は未使用)
- **備考**: Phase 1 は chat template を試した後、plain-text completion へ pivot 済 (PR #76)。理由は Gemma-2 の chat template が system role を持たない制約を持ち、Kotoha の強化 directive を injection しづらかったためである。

### Plain-text completion

- **定義**: chat template を使わず、強化 directive + few-shot 例 + 入力プロンプトを単一の plain-text 文字列として構築して推論する方式。
- **初出**: PR #76 (Phase 1 P1-2.5 follow-up)
- **対応する identifier**: `crates/kotoha-core/src/kanji/llama_cpp.rs` の現行実装
- **備考**: chat template pivot 後の Phase 1 default 方式。row 3 を除く 14/15 行で期待出力に合致する水準に達した。

### BOS / EOS tokens

- **定義**: 推論シーケンスの begin-of-stream / end-of-stream を表す特殊 token。Gemma-2 の場合は `<bos>` (id 2) / `<eos>` (id 1) 相当。
- **初出**: `crates/kotoha-core/src/kanji/llama_cpp.rs`
- **対応する identifier**: llama.cpp の `AddBos::Always` + EOS token id 判定
- **備考**: Phase 1 は `AddBos::Always` を採用し、EOS を生成停止条件に使用する。Phase 5 custom model では PUA tokens を追加の構造 marker として併用予定 (ADR 0010 D4)。

## 6. テスト・検証

### Layer 3 smoke (Layer 3 スモーク)

- **定義**: 実 GGUF model を load して end-to-end 動作を確認する opt-in テスト層。`crates/kotoha-core/tests/kanji_llama_cpp_smoke.rs` に配置する。
- **初出**: P1-2.5 plan (`docs/superpowers/plans/2026-04-24-kotoha-phase-1-p1-2-5.md`) / `crates/kotoha-core/tests/kanji_llama_cpp_smoke.rs`
- **対応する identifier**: `crates/kotoha-core/tests/kanji_llama_cpp_smoke.rs` + `#[cfg(feature = "llama-cpp-smoke")]` gate
- **備考**: Layer 1 (unit) / Layer 2 (integration, GGUF なし) / Layer 3 (GGUF あり) / Layer 4 (CLI E2E) の 4 層テスト戦略のうち、Layer 3 のみが実モデルを必要とする。

### Opt-in smoke (オプトイン・スモーク)

- **定義**: 環境変数 (Phase 1 では `KOTOHA_LLAMA_MODEL_PATH`) が設定されている場合のみ実行され、未設定時には SKIPPED として exit 0 で抜けるテストパターン。
- **初出**: Phase 1 spec §8.3
- **対応する identifier**: `kanji_llama_cpp_smoke.rs` の env var check 分岐
- **備考**: lefthook pre-push で build / clippy / test を全走するが、Layer 3 smoke は GGUF ファイルを必要とするため opt-in とし、CI / contributor 環境で強制しない方針を採用した。

### KOTOHA_LLAMA_MODEL_PATH

- **定義**: Gemma-2-2B-jpn-it (または互換 GGUF) のモデルファイルへの絶対パスを指定する環境変数。Layer 3 smoke の opt-in 条件。
- **初出**: PR #74
- **対応する identifier**: `std::env::var("KOTOHA_LLAMA_MODEL_PATH")`
- **備考**: `kotoha-cli::bin::kotoha-kanji` は CLI フラグ `--model` でも同等の指定が可能 (`scripts/phase1-smoke.sh` で使用)。

### Row 3

- **定義**: Layer 3 fixture の 3 行目 `あした → 明日`。Gemma-2-2B-jpn-it が「翌日」と出力し続ける Phase 1 の既知失敗ケース。
- **初出**: PR #76 WBS
- **対応する identifier**: `crates/kotoha-core/tests/fixtures/kanji_llama_cpp_smoke.tsv` の 3 行目
- **備考**: Phase 1 acceptance では 14/15 PASS として Phase 5 へ deferral する決定を ADR 0009 D5 + ADR 0010 で明文化した。Phase 5 で task-specific fine-tune により根本解消する計画。

### Cold load / Warm inference

- **定義**: cold load は model ファイルの初回読込 + context 構築の所要時間 (~10.6 秒)。warm inference は 2 回目以降の推論の所要時間 (~3 秒 / ケース)。
- **初出**: ADR 0013 (`docs/adr/0013-phase-1-latency-target.md`)
- **対応する identifier**: `LlamaCppBackend::new` 時点が cold、`convert` 呼出しが warm
- **備考**: Phase 1 の spec §8.3 当初 target (30 秒以下) に対し empirical ~52 秒 (cold + 14 warm) で超過したため、ADR 0013 で target を `cold ≤ 15s / warm ≤ 5s/case` に relax した。

### assert_equal / assert_contains / assert_summary

- **定義**: `scripts/lib/assert.sh` に配置する shell smoke 共通ライブラリの assertion API 3 種。exit code を返し、呼出し側で集約する。
- **初出**: Phase 0 M6 plan (`scripts/lib/assert.sh`)
- **対応する identifier**: `scripts/lib/assert.sh` (Phase 0 配備) / `scripts/phase1-smoke.sh` から呼出
- **備考**: Phase 1 の Layer 4 E2E smoke でもそのまま再利用する。`assert_equal` は完全一致、`assert_contains` は部分一致、`assert_summary` はテスト総括出力を行う。

## 7. Phase 5 (custom model) 用語

### Custom romaji-base model (Kotoha 専用 romaji-base モデル)

- **定義**: Phase 5 で Kotoha が自作する、romaji keystroke を直接入力として受け取る kana→kanji 変換専用モデル。汎用 instruction model ではない。
- **初出**: ADR 0010 (`docs/adr/0010-kotoha-custom-romaji-base-model.md`)
- **対応する identifier**: `BackendConfig::KotohaNative` variant (Phase 5 で追加予定) / Phase 5 spec
- **備考**: Phase 1 の Gemma-2-2B-jpn-it は汎用 instruction model で ICL に依存するため row 3 のような誤変換が発生する。Phase 5 ではタスク専用学習により ICL 依存を排除する。

### Task-specific fine-tune (タスク専用 fine-tune)

- **定義**: 特定の narrow task (Kotoha の場合は romaji→kana→kanji 変換) に限定して学習を行う fine-tune 方針。汎用 instruction-tuned model との対比概念。
- **初出**: ADR 0010
- **対応する identifier**: Phase 5 B1 / B3 の学習方式 (Phase 5 spec §5.1 / §5.3)
- **備考**: Phase 5 では B1 (scratch training) / B2 (LoRA) / B3 (distillation) の 3 候補を検討。Kotoha は B3 (distillation) を default 推奨とする (ADR 0010)。

### Scratch training (スクラッチ学習)

- **定義**: ランダム初期化された weights から学習を開始する方式。Phase 5 B1 候補。
- **初出**: ADR 0010 / Phase 5 spec §5.1
- **対応する identifier**: Phase 5 B1 (候補)
- **備考**: 学習データ量と計算資源が潤沢でない場合には B3 (distillation) より劣る可能性がある。B1 は比較対照として保持する。

### LoRA (Low-Rank Adaptation)

- **定義**: 既存の pre-trained model に低ランク adapter matrix を追加して fine-tune する手法。adapter のみを学習し base model weights は凍結する。
- **初出**: Phase 5 spec §5.2
- **対応する identifier**: Phase 5 B2 (候補、主選から除外)
- **備考**: 推論時にも base model を load する必要があるため、ファイルサイズ目標 (≤ 200MB) を満たしにくい。Phase 5 では B2 を主選から除外した。

### Distillation (蒸留)

- **定義**: 大型 teacher model の出力分布を小型 student model に転移する学習手法。Kotoha は Gemma-4-31B などの大型 teacher から 90M〜180M student model を作る計画。
- **初出**: ADR 0010 / Phase 5 spec §5.3
- **対応する identifier**: Phase 5 B3 (default 推奨)
- **備考**: student は inference 時に teacher を必要とせず、`≤ 200MB` / `p50 ≤ 30ms` target を満たしやすい設計となる (ADR 0010 D6 / Phase 5 spec §6.4)。

### Teacher model / Student model

- **定義**: 蒸留の 2 役割。teacher は大型で高精度、student は軽量で低遅延。student は teacher の出力分布 (soft label) を学習目標とする。
- **初出**: Phase 5 spec §5.3
- **対応する identifier**: Phase 5 B3 の概念
- **備考**: Kotoha の候補は teacher = Gemma-4-31B 程度、student = 90M〜180M 程度。Phase 5 kick-off で teacher の最終選定を行う。

### PUA tokens (Private Use Area tokens)

- **定義**: Unicode Private Use Area (`U+E000` 〜 `U+F8FF`) のコードポイントを学習時に特殊 token として割り当て、構造 marker として使う手法。例: `\u{ee00}` = `<romaji>`, `\u{ee01}` = `<out>`, `\u{ee02}` = `<ctx>`, `\u{ee03}` = `<eos>`。
- **初出**: Karukan 参照 / ADR 0010 D4
- **対応する identifier**: Phase 5 custom model の tokenizer (外部 tokenizer 経由)
- **備考**: Karukan の jinen-v1-small が PUA tokens を採用しており、Kotoha も Phase 5 で継承する。llama.cpp の内蔵 tokenizer が PUA を特殊 token として扱えないため、外部 tokenizer を併用する。

### External tokenizer (外部 tokenizer)

- **定義**: llama.cpp 内蔵の tokenizer をバイパスし、HuggingFace `tokenizers` crate 経由で `tokenizer.json` を独立にロードして token id 変換を行う手法。
- **初出**: Karukan 由来 / ADR 0010 D5
- **対応する identifier**: Phase 5 custom model inference 層の設計
- **備考**: PUA tokens や独自 pre-tokenizer を採用する場合、llama.cpp 内蔵 tokenizer の制約を回避する目的で採用する。Zenz 系 model が llama.cpp に採用できなかった理由 (ADR 0009) と同じ根本原因への対処。

### Edit distance (編集距離) / Levenshtein distance

- **定義**: 2 つの文字列間の単一文字編集操作 (substitute / insert / delete) の最小回数。距離 `d(s1, s2) = n` は n 回の編集で s1 から s2 に変換できることを意味する。
- **初出**: Phase 5 spec §4.3 / `tools/p5a-data-pipeline/src/kotoha_p5a/typo.py`
- **対応する identifier**: P5-A PoC の typo 注入関数 (距離 1〜3 の誤入力をランダムに生成)
- **備考**: Phase 5 学習データ augmentation で編集距離 1〜3 の範囲の typo を注入してロバスト性を得る。距離 4 以上は意味が崩壊するため除外する。

### QWERTY adjacency (QWERTY 隣接)

- **定義**: US QWERTY キーボード配列上で物理的に隣接するキーの集合。例: `s` の隣接は `a`, `d`, `w`, `e`, `x`, `z`。
- **初出**: P5-A PoC `tools/p5a-data-pipeline/src/kotoha_p5a/typo.py`
- **対応する identifier**: `kotoha_p5a.typo` module 内の隣接テーブル
- **備考**: typo 注入の substitute 操作で「押し間違え」を模擬する際、隣接キーの中から置換候補を選ぶ。insert 操作でも使用する。

### Typo injection (typo 注入)

- **定義**: 学習データの romaji 入力に編集距離 1〜3 の操作 (substitute / transpose / delete / insert) を確率的に加え、誤入力に耐性のあるモデルを得る data augmentation 手法。
- **初出**: Phase 5 spec §4.3 / P5-A PoC
- **対応する identifier**: `tools/p5a-data-pipeline/src/kotoha_p5a/typo.py`
- **備考**: P5-A PoC では 1 行の kana sequence から {0, 1, 2, 3} 距離の typo 変異を同時生成する。

### Partial-input (部分入力)

- **定義**: romaji keystroke stream の途中 prefix。確定前の未完成文字列を意味する。例: `kyou` の途中段階として `k`, `ky`, `kyo` が partial-input となる。
- **初出**: ADR 0010 Context / Phase 5 spec §4.4
- **対応する identifier**: Phase 5 モデルの入力形式 (partial-input から候補列を返す設計)
- **備考**: Phase 5 モデルは「確定済 romaji」だけでなく「入力途中 romaji」を入力として、候補を逐次生成する設計目標を持つ。preedit UI との親和性を担保する。

### Noisy romaji (ノイジーロマ字)

- **定義**: typo 注入後の romaji 文字列。元 romaji に編集距離 >0 の変異が加わった状態。
- **初出**: `tools/p5a-data-pipeline/src/kotoha_p5a/__main__.py` (TSV 出力の column 名)
- **対応する identifier**: P5-A PoC の TSV schema の `noisy_romaji` column
- **備考**: P5-A PoC は `{kana, romaji_hepburn, romaji_kunrei, romaji_waapuro, noisy_romaji, surface}` 型の TSV を出力する。noisy_romaji は学習入力 (augmented input) として使う。

### Bias sampling (偏り補正 sampling)

- **定義**: コーパス頻度の偏り (例: 助詞「の」の過剰頻度) を学習データ抽出段階で補正する sampling 手法。
- **初出**: Phase 5 spec §3.1 (非 scope) / P5-A PoC README
- **対応する identifier**: 未実装 (Phase 5 kick-off で詳細化)
- **備考**: P5-A PoC の scope 外。Phase 5 kick-off 時に sampling 比率と対象語彙を確定する。

### Train / validation / test split

- **定義**: 学習データを 80% / 10% / 10% 等の比率で 3 分割し、過学習検出 (validation) と最終性能測定 (test) を独立に行う運用。
- **初出**: Phase 5 spec §3.1 (非 scope) / P5-A PoC README
- **対応する identifier**: 未実装 (Phase 5 kick-off で詳細化)
- **備考**: P5-A PoC の scope 外。Phase 5 kick-off 時に比率と分割基準を確定する。

## 8. 参考実装・関連エコシステム

### Karukan (`togatoga/karukan`)

- **定義**: Rust で実装された Linux 向け日本語 IME。fcitx5 addon として動作する。Kotoha の主要な参考実装。
- **初出**: Phase 0 design spec §2 / ADR 0002
- **対応する identifier**: 参考実装 (https://github.com/togatoga/karukan)
- **備考**: Kotoha は入力モードの Sticky 挙動を採用しない (ADR 0002) / jinen-v1-small を使用しない / 参考実装としての設計方針を一部継承 (PUA tokens、external tokenizer) する関係。

### jinen-v1-small / jinen-v1-xsmall

- **定義**: Karukan が採用する kana→kanji 専用 fine-tune GPT-2 model。jinen-v1-small は 90M パラメータ、jinen-v1-xsmall は 26M パラメータ。両方とも Q5_K_M 量子化済。
- **初出**: ADR 0010 Context
- **対応する identifier**: HuggingFace repo `togatogah/jinen-v1-small.gguf` / `togatogah/jinen-v1-xsmall.gguf`
- **備考**: Kotoha Phase 1 では採用せず Gemma-2-2B-jpn-it を選んだ (ADR 0009)。Phase 5 custom model の size target (90M〜180M) は jinen のサイズ帯を参考にしている。

### Zenz / Zenz-v2.5

- **定義**: azooKey ecosystem の kana→kanji 専用 GPT-2 model family。Phase 1 候補の 1 つとして検討された。
- **初出**: Phase 1 P1-2-9 WBS / ADR 0009
- **対応する identifier**: HuggingFace `Miwa-Keita/zenz-v2-gguf` 等
- **備考**: `gpt2-small-japanese-char` pre-tokenizer が llama.cpp で未サポートのため採用不可と判断した (ADR 0009 A2 / P1-2-9 WBS)。Phase 2+ での外部 tokenizer 併用による再評価候補として保持する。

### Zenzai

- **定義**: azooKey の neural 変換 engine 名。Zenz family の model を使用する。
- **初出**: Phase 1 P1-2-9 WBS
- **対応する identifier**: azooKey 内部実装
- **備考**: Kotoha は Zenzai を直接統合しないが、Zenz family の model を将来評価する際の文脈として参照する。

### azooKey

- **定義**: Swift 実装の multi-platform 日本語 IME ecosystem。macOS / iOS をカバーし、Windows 向け myime 派生がある。
- **初出**: Phase 1 P1-2-9 WBS
- **対応する identifier**: https://github.com/ensan-hcl/azooKey (参考)
- **備考**: Kotoha とは target platform (GNOME Wayland) と言語 (Rust) が異なるが、Zenz model 経由で neural 変換設計の参考になる。

### sudachipy / SudachiDict

- **定義**: sudachipy は Python 実装の日本語形態素解析器、SudachiDict は対応辞書。Apache-2.0 ライセンス。
- **初出**: `tools/p5a-data-pipeline/pyproject.toml`
- **対応する identifier**: P5-A PoC (`tools/p5a-data-pipeline/`) の依存
- **備考**: P5-A PoC で日本語コーパスの形態素分解に採用。ライセンス互換性 (Kotoha 本体の想定 OSS ライセンスと整合) も採用理由の 1 つ。

### MeCab / UniDic / fugashi

- **定義**: MeCab は C++ 製形態素解析器、UniDic は対応辞書、fugashi は MeCab の Python binding。
- **初出**: Phase 5 spec §3.1
- **対応する identifier**: Kotoha では採用しない
- **備考**: fugashi が GPL ライセンスであるため Kotoha の想定 OSS ライセンス方針と衝突し、P5-A PoC では sudachipy を採用した。MeCab 本体 / UniDic は選択肢から除外した。

## 9. ツール・環境

### lefthook

- **定義**: Git hook マネージャ。pre-commit / pre-push hook をプロジェクト横断で統一管理する。Kotoha では pre-commit (fmt / lint / doc-naming) と pre-push (build / clippy / test) の 2 層 gate として採用している。
- **初出**: Phase 0 setup / `lefthook.yml`
- **対応する identifier**: `lefthook.yml` + `scripts/pre-commit-doc-naming.sh`
- **備考**: global rule (modern-toolchain.md) で pre-commit / husky より lefthook を優先する方針に従う。

### lefthook pre-push gate

- **定義**: `git push` 実行時に workspace 全体の build / clippy (-D warnings 必須) / test を強制する安全弁。失敗時は push を block する。
- **初出**: Phase 1 spec §8.3 / global CLAUDE.md
- **対応する identifier**: `lefthook.yml` の `pre-push` 節
- **備考**: Layer 3 smoke (GGUF を必要とする) は opt-in のため pre-push には含めない。pre-push の速度を保ちつつ、実モデル検証を separate に保つ設計。

### uv

- **定義**: Python 3.12+ 向けの高速 package / project manager。依存管理と virtualenv を統合する。
- **初出**: `tools/p5a-data-pipeline/pyproject.toml`
- **対応する identifier**: P5-A PoC の dependency manager
- **備考**: global rule (modern-toolchain.md) で pip / Poetry より uv を優先する方針に従う。

### shellcheck / shfmt

- **定義**: shellcheck は shell script の静的解析ツール、shfmt は shell script の formatter。
- **初出**: `~/.claude/rules/file-shell.md` rule / `scripts/phase1-smoke.sh`
- **対応する identifier**: `scripts/phase*-smoke.sh` に適用される lint / format
- **備考**: Kotoha の smoke スクリプト (`scripts/phase*-smoke.sh`) は shellcheck PASS + shfmt 整形済を前提とする。

## 更新規約

新規ドメイン用語を specs / plans / ADR / コード / commit message / PR / WBS のいずれかに導入する際は、本 glossary.md にも同時に entry を追記する (global rule `~/.claude/rules/glossary-consistency.md` 準拠)。

運用手順は以下の 4 点である。

1. **entry 書式**: 新規 entry は `### 用語名 (English name / 識別子)` を heading とし、「定義」「初出」「対応する identifier」「備考」の 4 項目を必ず埋める。該当が無い項目には「なし」または「非採用」等の明示値を置く。
2. **カテゴリ配置**: 既存 9 カテゴリのいずれかに配置する。既存カテゴリに収まらない場合のみ、10 番目以降のカテゴリを新設する (その際は目次も同時に更新する)。
3. **同時 commit 原則**: 用語を初出する spec / plan / ADR / コードの commit と同一 commit 内で本 glossary.md を更新する。別 commit に分離しない。分離すると「用語が導入されたが glossary が追従していない」期間が生じる。
4. **非採用 / 廃止の扱い**: Kotoha が明示的に非採用とする用語 (例: canonical romaji、Sticky モード) も、参考実装との対比として本 glossary に残す。「対応する identifier」に「Kotoha では実装されない (非採用)」と明記して識別可能にする。
