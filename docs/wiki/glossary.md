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

### Phase 2 Dictionary layer (P2-A 以降)

以下 15 entry は ADR 0014 (`docs/adr/0014-phase-2-dictionary-layer-architecture.md`) / ADR 0015 (`docs/adr/0015-kotoha-storage-sqlite-adoption.md`) および Phase 2 spec / P2-A spec / P2-B spec / P2-C spec で初出した用語を集約する。実装 identifier は P2-A で確定済のもの、P2-B で確定したもの (kotoha-dict / kotoha-storage / kotoha.db / UserVocab / UserVocabStore の 5 entry。P2-C で `UserVocabStore` は `UserVocabReader` / `UserVocabWriter` の 2 trait に分割した)、P2-C で確定したもの (`LearningCacheReader` / `LearningCacheWriter` / `test-helpers` feature flag / `CapOverrideGuard` / `evict_to_cap` helper の 5 entry)、P2-D 以降で確定予定のものを含む。

### 形態素解析 (Morphological Analysis)

- **定義**: 入力テキストを形態素 (意味を持つ最小単位) に分割し、各形態素の表記 / 読み / 品詞を同定する処理。Phase 2 P2-A は本処理を `MorphologicalEngine` trait の抽象境界経由で `SudachiAdapter` 実装として提供する。
- **初出**: ADR 0014 C3 / Phase 2 P2-A spec §3.3 / §4.2
- **対応する identifier**: `MorphologicalEngine` trait + `SudachiAdapter` 実装 (`crates/kotoha-core/src/dict/morph/` 配下)
- **備考**: SudachiDict 採用の動機 (固有名詞 / 敬称 recall を構造的に補う) は形態素解析処理に依存する。Phase 5 以降で vibrato / lindera 等の形態素解析器への差し替えを検討する余地は MorphologicalEngine trait 抽象によって確保している。

### MorphologicalEngine

- **定義**: Phase 2 P2-A で導入した形態素解析 engine の抽象境界 trait。`tokenize(reading) -> Vec<EngineCandidate>` と `engine_id() -> &str` の 2 method を提供する。
- **初出**: Phase 2 P2-A spec §4.2
- **対応する identifier**: `MorphologicalEngine` trait (`crates/kotoha-core/src/dict/morph/`)
- **備考**: P2-A 段階では `SudachiAdapter` のみが本 trait を実装する。Phase 5 以降で vibrato / lindera 等の形態素解析器への差し替え余地を確保する目的で先出しした抽象 layer であり、`KanjiBackend` trait (ADR 0011) と同様に `Box<dyn MorphologicalEngine>` による dynamic dispatch を採用する。

### VocabularyLookup

- **定義**: Phase 2 P2-A で導入した user / custom vocabulary の lookup 抽象境界 trait。`lookup(reading) -> Vec<VocabEntry>` と `vocab_id() -> &str` の 2 method を提供する。
- **初出**: Phase 2 P2-A spec §4.2
- **対応する identifier**: `VocabularyLookup` trait (`crates/kotoha-core/src/dict/vocab/`)
- **備考**: P2-A 段階では `CustomVocab` (TSV reader) のみが本 trait を実装する。P2-B で追加予定の `UserVocab` (ユーザ個別語彙、追加 / 削除 / 列挙 API を持つ) が同 trait を実装する extension path を確保する目的で先出しした抽象 layer。

### SudachiDict

- **定義**: WorksApplications が提供する日本語形態素解析辞書 (core / small / full の 3 variant)。Apache-2.0 ライセンスで公開され、約 76 万 entries (lemma + 活用形含む、lemma 単位では約 20 万) 規模の core variant を Kotoha Phase 2 P2-A は採用する。
- **初出**: ADR 0014 C4 / Phase 2 spec §4.1
- **対応する identifier**: `KOTOHA_SYSTEM_DICT_PATH` 環境変数で `system_core.dic` の絶対パスを指定する (Phase 1 の `KOTOHA_LLAMA_MODEL_PATH` と同 pattern の manual placement 運用)
- **備考**: Kotoha Phase 2 P2-A が採用する正確な version は v20260116 (約 70MB、76 万 entries 規模)。core / full / small の 3 variant のうち、IME 常駐 footprint と recall の trade-off で core を default とする (Phase 2 spec §4.1)。`sudachipy / SudachiDict` (§8) 項は P5-A PoC 文脈での Python binding 採用を扱うが、本項は Phase 2 P2-A の Rust 実装 runtime での採用文脈に焦点を当てる。

### DictionaryBackend / DictionaryAugmented variant

- **定義**: Phase 2 で追加予定の `BackendConfig` 新 variant (正式名は P2-A kick-off で確定)。Sudachi-based dictionary lookup と LlamaCpp LLM 生成を hybrid 構成で組合せる backend を表す。LLM 単体では recall が不足する固有名詞 / 敬称 / User 登録語彙を dictionary 側で structural に補う目的で導入する。
- **初出**: ADR 0014 D1 / Phase 2 spec §3.3
- **対応する identifier**: `BackendConfig::DictionaryAugmented` (候補名、P2-A kick-off で実装確定)
- **備考**: 既存 `BackendConfig` は `#[non_exhaustive]` 属性を持つため、新 variant 追加は ADR 0006 方針に整合し breaking change にならない (既存 BackendConfig entry 備考参照)。Phase 5 で追加予定の `KotohaNative` variant とは直交しており、Phase 5 backend も同じ Dictionary layer を再利用できる設計 (ADR 0014 D6)。

### System dictionary (システム辞書)

- **定義**: Kotoha が配布する共通語彙辞書。SudachiDict-core (WorksApplications, Apache-2.0) を base とし、Kotoha 独自語彙 (敬称、IME 固有表記等) を薄い補完 layer として merge する。Phase 2 default の recall 源として働く。
- **初出**: ADR 0014 D2 / Phase 2 spec §4.2
- **対応する identifier**: System dictionary loader module (`crates/kotoha-core/src/dict/` 配下、P2-A kick-off で配置確定)
- **備考**: core variant (約 70MB) と full variant (約 500MB) の 2 種のうち、Phase 2 default は core を採用する (Phase 2 spec §4.1)。SudachiDict-core の収録 entry 数は約 76 万 (lemma + 活用形含む、lemma 単位では約 20 万)。full 切替は P2-D golden fixture で recall 不足と判定された場合に限定する。

### User dictionary (ユーザ辞書)

- **定義**: ユーザが明示登録する個別語彙 (人名 / 所属組織名 / 業界固有語 / macro 展開) を保持する辞書。System dictionary と分離し、永続化形式は SQLite 共用 DB `kotoha.db` の `user_vocab` table である (ADR 0015 / P2-B spec §5.1、2026-04-25 P2-B kick-off で確定)。
- **初出**: ADR 0014 D2 / Phase 2 spec §4.3 / P2-B spec §5.1
- **対応する identifier**: `kotoha_storage::user_vocab::SqliteUserVocabStore` (永続化層) + `kotoha_core::dict::user_vocab::UserVocab` (`VocabularyLookup` 実装層) の 2 component
- **備考**: System dictionary の再配布サイクルに束縛されず、ユーザ側で独立に更新可能である。Phase 6 UX (MEMORY.md 参照) で設定 UI からの追加 / 編集 / import / export を実装予定。Phase 2 P2-B では `kotoha-dict` CLI subcommand (`add` / `remove` / `list` / `show` の 4 本) で編集する。

### Learning cache (学習キャッシュ)

- **定義**: ユーザの変換候補選択履歴を永続化し、後続の rerank に利用する cache。永続化形式は SQLite 共用 DB `kotoha.db` の `learning_cache` table である (ADR 0015 / P2-B spec §5.2、2026-04-25 P2-B kick-off で確定)。runtime での in-memory LRU 形態を採るか SQLite 直読みのみとするかは P2-C kick-off で empirical 確定する。同一 kana 入力に対するユーザ選択の偏りを時系列で反映する目的で導入する。
- **初出**: ADR 0014 D3 / Phase 2 spec §5 / ADR 0015
- **対応する identifier**: `kotoha_storage::learning_cache::{LearningCacheReader, LearningCacheWriter}` trait (P2-B で `LearningCacheStore` 1 trait の skeleton として先出し、P2-C で arch-M-2 ISP split + 本実装に置換)。SQLite 実装は `kotoha_storage::learning_cache::sqlite::SqliteLearningCacheStore` が両 trait を impl する。
- **備考**: P2-B 段階では skeleton(stub)であったが、P2-C(PR #106)で UPSERT(`record_choice`)+ 自動 LRU eviction(行数上限超過時に `last_used_at ASC` 順で削除)+ lookup(`frequency DESC, last_used_at DESC` 順)の本実装に置換した。LRU 容量上限は `LEARNING_CACHE_MAX_ROWS` 定数(暫定 10,000 entry)で表現し、Phase 5 personalization での動的 cap 化を後段 Issue として残す。Phase 5 `KotohaNative` backend でも再利用可能な layer として設計する (ADR 0014 D6)。

### Ranker / Reranker

- **定義**: Dictionary 候補と LLM 候補を merge / dedupe / rerank して最終 top-k を決定する layer。初期重みは dict = 0.95 / LLM = 1.0 (P2-D での empirical tuning を前提とした暫定値)。学習キャッシュの履歴情報を rerank signal として追加で加味する。
- **初出**: ADR 0014 D5 / Phase 2 spec §3.3 / §6
- **対応する identifier**: Ranker module (`crates/kotoha-core/src/ranker/` 配下、P2-A kick-off で配置確定)
- **備考**: 初期重み (dict 0.95 / LLM 1.0) は ADR 0014 D5 の暫定値であり、P2-D で 100〜200 件規模の golden fixture に対して evaluation し再確定する (ADR 0014 Consequences 負の帰結 参照)。

### Candidate merge / dedupe

- **定義**: 辞書候補と LLM 候補を同一 kanji surface で統合 (merge) し、重複 entry を排除 (dedupe) する処理。score は「最大値採用」または「重み合算」のいずれかで決定し、選択方針は P2-D で empirical 確定する。
- **初出**: Phase 2 spec §3.3
- **対応する identifier**: Ranker module 内の merge / dedupe 関数 (P2-A kick-off で実装確定)
- **備考**: 同一 surface が Dictionary 側と LLM 側の両方から返る場合、source フィールド (Candidate 構造体に Phase 2 で追加予定) を保持して、後段の UX / debug に活用できる設計とする。

### kotoha-dict

- **定義**: Phase 2 P2-B で新規導入する CLI binary。`add` / `remove` / `list` / `show` の 4 subcommand で UserVocab を編集する。`update` / `import` / `export` / `init` は Phase 6+ で扱う。
- **初出**: Phase 2 P2-B spec §7
- **対応する identifier**: `crates/kotoha-cli/src/bin/dict.rs` + `crates/kotoha-cli/src/dict_cli.rs`(`--features dict-persist` で有効化)
- **備考**: ADR 0014 D7 / ADR 0015 で決定した SQLite 共用 DB `kotoha.db` に対する CRUD 経路として動作する。global flag `--data-dir` / `--quiet` / `--json` を共有する。`<READING>` 引数は P2-B spec §7.7 の auto-detect 仕様 (全 hiragana → そのまま、全 ASCII → RomajiConverter、混在 → reject) に従う。

### kotoha-storage

- **定義**: Phase 2 P2-B で新規導入する Rust crate。SQLite ベースの永続化層を提供し、`Database` 構造体 / Migration runner / `UserVocabReader` / `UserVocabWriter` / `LearningCacheReader` / `LearningCacheWriter` の 4 trait + 各実装(Sqlite + Mock)を含む。`rusqlite + bundled` を採用し、SQLite C library の依存を本 crate 内に閉じ込める。
- **初出**: ADR 0015 / Phase 2 P2-B spec §4
- **対応する identifier**: `crates/kotoha-storage/`(crate root)、`kotoha_storage::Database` / `kotoha_storage::user_vocab::{UserVocabReader, UserVocabWriter}` / `kotoha_storage::learning_cache::{LearningCacheReader, LearningCacheWriter}` 等
- **備考**: Clean Architecture「Interface 依存」/ SOLID DIP に整合させるため、`kotoha-core` に逆依存しない。`kotoha-core::dict::user_vocab::UserVocab` が `Box<dyn UserVocabReader>` を field 保持することで `kotoha-core` 単体 build は SQLite C library コンパイル不要となる。P2-B 着地時点では `UserVocabStore` / `LearningCacheStore` の 2 trait 構成であったが、P2-C(2026-04-26、PR #106)で arch-M-2 ISP split を適用し Reader/Writer の 4 trait 構成に置換した。

### kotoha.db

- **定義**: Kotoha Phase 2 が共用する単一 SQLite DB ファイル。`$KOTOHA_DATA_DIR` (未設定時は `$XDG_DATA_HOME/kotoha` または `~/.local/share/kotoha`) に配置し、`user_vocab` table (P2-B) と `learning_cache` table (P2-B で schema only、P2-C で本実装) を同梱する。WAL モード (`PRAGMA journal_mode = WAL`) で動作するため、隣接位置に `kotoha.db-wal` / `kotoha.db-shm` の 2 ファイルが補助生成される。
- **初出**: ADR 0015 / Phase 2 spec §5.2 / Phase 2 P2-B spec §5
- **対応する identifier**: `kotoha_storage::Database::open(path)` の path 引数、`kotoha_storage::path::resolve_data_dir` の戻り値 + `kotoha.db` 連結
- **備考**: WAL モード採用により Phase 3 IBus engine と `kotoha-dict` CLI の同時 open が独自 file lock 実装なしで安全動作する。debugging / inspection は distro の `sqlite3` CLI (例: `sqlite3 kotoha.db 'SELECT * FROM user_vocab'`) で行う。

### UserVocab

- **定義**: Phase 2 P2-B で導入する user-managed 語彙 lookup 実装。P2-A で先出しした `VocabularyLookup` trait の 2 つ目の実装 (`CustomVocab` に続く)で、`kotoha-storage` crate の SQLite `user_vocab` table を backing storage とする。CustomVocab とは Vec 順序 `[CustomVocab, UserVocab]` で score tie 時の優先順位が決まる(curated CustomVocab 由来 entry を default で勝ち残らせる)。
- **初出**: Phase 2 P2-B spec §3.7 / §6.7
- **対応する identifier**: `crates/kotoha-core/src/dict/user_vocab.rs` の `UserVocab` 構造体 (`impl VocabularyLookup`)
- **備考**: `UserVocab::lookup` は SQLite backend エラー時に空 Vec を返す (`unwrap_or_default()`) ことで、`VocabularyLookup::lookup` の sync signature を維持しつつ MorphologicalEngine 経路と CustomVocab 経路の recall を保護する。

### UserVocabReader / UserVocabWriter (ISP split)

- **定義**: `kotoha-storage` crate 内で定義する 2 trait。`UserVocabReader` は read-only(`find_by_id` / `find_by_reading` / `find_by_prefix` / `list_all` の 4 method)、`UserVocabWriter` は write-only(`insert` / `delete_by_id` / `delete_by_surface_reading` の 3 method)である。SOLID Interface Segregation Principle(arch-M-2)に従い、Phase 3 IBus engine などの read-only consumer に Writer 系 method を露出しない設計とする。SQLite 実装(`SqliteUserVocabStore`)と Mock 実装(`MockUserVocabStore`)が両 trait を impl する。
- **初出**: Phase 2 P2-B spec §6.1(`UserVocabStore` 1 trait として導入) / Phase 2 P2-C spec §3.2(arch-M-2 ISP split で 2 trait に分割)
- **対応する identifier**: `kotoha_storage::user_vocab::store::{UserVocabReader, UserVocabWriter}`
- **備考**: 両 trait とも `Send + Sync` 制約を持ち、`Box<dyn UserVocabReader>` / `Box<dyn UserVocabWriter>` で `kotoha-core::dict::user_vocab::UserVocab` 等の上位層に注入される。`MockUserVocabStore` は SQLite 不在環境での Layer 2 integration test を可能にする目的で同 crate 内に配置する(P2-A `MockEngine` / `MockVocab` と同 pattern)。履歴: P2-B(2026-04-25、PR #101)で `UserVocabStore` 1 trait として導入したが、P2-C(2026-04-26、PR #106)で arch-M-2 ISP split を適用し Reader / Writer の 2 trait に分割した。旧 `UserVocabStore` trait は P2-C で削除済。

### LearningCacheReader / LearningCacheWriter (ISP split)

- **定義**: `kotoha-storage` crate 内で定義する 2 trait。`LearningCacheReader` は read-only(`lookup(kana_input, limit) -> Vec<LearningCacheRecord>` の 1 method、`frequency DESC, last_used_at DESC` 順で返す)、`LearningCacheWriter` は write-only(`record_choice(kana_input, chosen_kanji)` UPSERT + 自動 LRU eviction、明示 eviction 用 `evict_lru(max_entries) -> usize` の 2 method)である。SOLID Interface Segregation Principle に従い、rerank consumer に Writer 系 method を露出しない設計とする。SQLite 実装(`SqliteLearningCacheStore`)が両 trait を impl する。
- **初出**: Phase 2 P2-C spec §3.1 / §6.1(P2-B 段階の `LearningCacheStore` skeleton を ISP split で 2 trait に分割)
- **対応する identifier**: `kotoha_storage::learning_cache::{LearningCacheReader, LearningCacheWriter}` trait + `kotoha_storage::learning_cache::sqlite::SqliteLearningCacheStore` 実装
- **備考**: 両 trait とも `Send + Sync` 制約を持ち、`Box<dyn LearningCacheReader>` / `Box<dyn LearningCacheWriter>` で上位層に注入される。`record_choice` 内部では `effective_max_rows()` を cap として `evict_to_cap` helper を呼出し自動 LRU eviction を行うため、通常の consumer は `evict_lru` を明示呼出しする必要は少ない。履歴: P2-B(2026-04-25、PR #101)で `LearningCacheStore` 1 trait の skeleton として先出ししたが、P2-C(2026-04-26、PR #106)で arch-M-2 ISP split + 本実装に置換した。旧 `LearningCacheStore` trait は P2-C で削除済。

### test-helpers feature flag

- **定義**: `kotoha-storage` crate の Cargo feature。test-only API(`CapOverrideGuard` / `LEARNING_CACHE_MAX_ROWS_TEST_OVERRIDE` / `CAP_OVERRIDE_LOCK` / `effective_max_rows`)を `#[cfg(any(test, feature = "test-helpers"))]` で gate し、production binary に test-only API が漏出しない設計を実現する。default では off であり、integration test crate(`crates/kotoha-storage/tests/learning_cache_*`)が `[[test]] required-features = ["test-helpers"]` で要求する。
- **初出**: Phase 2 P2-C spec §4.5 / §4.6(commit `42f5cd2` で導入)
- **対応する identifier**: `crates/kotoha-storage/Cargo.toml` の `[features]` 節 `test-helpers = []` + 各 test-only 定義の `#[cfg(any(test, feature = "test-helpers"))]` gate
- **備考**: lefthook pre-push の test step が `--features kotoha-storage/test-helpers` を強制することで、production-only build と test build の両方を CI で機械検証する。本 feature gate により「test-only RAII guard が production API に漏出する」という arch-M-2 / sec-M finding を構造的に防止する。

### CapOverrideGuard

- **定義**: `kotoha-storage` crate の test-only RAII guard。`#[cfg(any(test, feature = "test-helpers"))]` で gate される。`new(cap)` は test override 値設定 + `CAP_OVERRIDE_LOCK` 取得を同時に行い、`lock_only()` は override 値変更なしに `CAP_OVERRIDE_LOCK` のみを取得する(parallel 実行される他 test の `record_choice` 内 auto eviction を小さな cap で発火させない用途)。Drop 時に override 値を 0 に戻す。
- **初出**: Phase 2 P2-C spec §4.5(commit `1af6d6b` で `LEARNING_CACHE_MAX_ROWS` cap const と同時導入、`eef7cc9` で `lock_only` API 追加、`42f5cd2` で test-helpers feature gate 化)
- **対応する identifier**: `kotoha_storage::learning_cache::sqlite::CapOverrideGuard` 構造体(`crates/kotoha-storage/src/learning_cache/sqlite.rs`)
- **備考**: `kotoha_storage::user_vocab::sqlite::QuotaOverrideGuard`(P2-B 由来)と同 pattern であり、static `LEARNING_CACHE_MAX_ROWS_TEST_OVERRIDE: AtomicUsize` の値を test scope で一時的に上書きする。process 全体の static を変更する性質上、test 間の race を `CAP_OVERRIDE_LOCK: Mutex<()>` で serialize する。

### evict_to_cap helper

- **定義**: `kotoha-storage` crate の `pub(crate)` 内部関数。signature は `pub(crate) fn evict_to_cap(conn: &Connection, cap: usize) -> Result<usize, StorageError>`。`COUNT(*)` で `learning_cache` table の行数を取得し、`cap` を超過していれば超過分の entry を `last_used_at ASC, id ASC` 順に DELETE し、削除件数を返す。
- **初出**: Phase 2 P2-C spec §4.6(commit `fd124b7` で抽出)
- **対応する identifier**: `kotoha_storage::learning_cache::sqlite::evict_to_cap`(`crates/kotoha-storage/src/learning_cache/sqlite.rs`)
- **備考**: `record_choice` の自動 eviction(commit `fd124b7`)と `evict_lru` の明示 eviction(commit `3c8391d`)で共通化された helper。tie-breaker `id ASC` を併用することで `last_used_at` が同値の場合でも決定論的な削除順序を保証する。

### Phase 3 IBus engine 用語 (P3-A 以降)

以下は Phase 3-A spec(`docs/superpowers/specs/2026-05-02-p3-a-ibus-engine-design.md`、ISSUE #116)で初出するドメイン用語である。Phase 3-A 本番実装段階での crate / type identifier は同 spec §4 で凍結する。

### Hexagonal Architecture (Ports and Adapters)

- **定義**: domain core が外部世界(IME host / DB / UI 等)と「port」と呼ばれる trait 経由でのみ会話するアーキテクチャ pattern。core の依存方向は内向き(adapter → core)で、IBus / fcitx5 / 将来の input-method protocol 等は adapter crate の追加 / 差し替えだけで吸収できる。Driving port(host → core を呼ぶ側、`IMEEngine`)と Driven port(core → host を呼ぶ側、`IMEHostBridge`)の 2 方向を分離する。
- **初出**: Phase 3-A spec §3.2(2026-05-02、ISSUE #116)
- **対応する identifier**: `kotoha-engine-core` crate(domain)+ `kotoha-engine-ibus` crate(adapter)+ `kotoha-bin` crate(entry / DI)
- **備考**: Dependency Injection(DI)と orthogonal だが補完的に組み合わさる。Phase 3-A は `Box<dyn IMEHostBridge>` / `Box<dyn Ranker>` を `KotohaEngine::new` の引数で注入する構造的 DI を採用する。Phase 4 fcitx5 adapter は `kotoha-engine-fcitx5` 新 crate として追加される予定で、`kotoha-engine-core` は無変更で extend される。

### IMEEngine / IMEHostBridge (Phase 3 trait pair)

- **定義**: Phase 3-A で導入する Hexagonal port 2 trait。`IMEEngine` は driving port(IME host adapter が engine の `process_key_event` / `focus_in` / `focus_out` / `enable` / `disable` / `reset` を呼び出す側)、`IMEHostBridge` は driven port(engine が host adapter の `update_preedit` / `commit_text` / `update_candidates` / `show_candidate_window` / `hide_candidate_window` を呼び出す側)である。両 trait の組み合わせで IBus / fcitx5 / 将来の host との会話 layer を完全に adapter 側に局所化する。
- **初出**: Phase 3-A spec §4.1 / §4.2(2026-05-02)
- **対応する identifier**: `kotoha_engine_core::IMEEngine` trait + `kotoha_engine_core::IMEHostBridge` trait
- **備考**: `IMEEngine` は単一 thread から呼ばれる前提で `Send + !Sync`、`IMEHostBridge` は engine 主 thread と RankerWorker thread の双方から呼ばれるため `Send + Sync` を要求する。

### Live 変換 (Live conversion mode)

- **定義**: typing 中の毎 keystroke で Ranker を invoke し、preedit kana に対する候補を逐次表示する変換 mode。Phase 3-A から first-class で実装する設計判断は、Live 変換が IME 全体で最も処理負荷が高いため後付け実装を避け、初期から budget を測定可能にすること、および backspace 時の romaji-level 同期 reset などの周辺 logic を最初から扱うため。
- **初出**: Phase 3-A spec §5.2 / §6.1(2026-05-02、Q5 (a) 採択)
- **対応する identifier**: `kotoha_engine_core::ConversionMode::Live`(`crates/kotoha-engine-core/src/lib.rs`、Phase 3-A 本番実装で確定)
- **備考**: Live 変換時の Ranker latency budget は dict only fast path で keystroke あたり < 10ms。LLM backend は best-effort で投げる(typing 中は cancel propagation を頻発する想定、§13 Open Q 7)。Phase 5 custom model の partial-input + beam search が来ると Live 変換が本格高度化する。

### Commit 変換 (Commit conversion mode)

- **定義**: space 確定後の `CommitConverting` 状態で行う候補確定変換 mode。Live 変換と同じ Ranker trait を共有するが、coalescing window が拡張(30ms)され LLM 結果まで待機する。p99 100ms 以内に候補ウィンドウ表示完了を targeted。
- **初出**: Phase 3-A spec §5.2 / §6.1(2026-05-02)
- **対応する identifier**: `kotoha_engine_core::ConversionMode::Commit`
- **備考**: 日野岡さんの最重視要件「短い単語 + space + 文脈認識変換」は Commit 変換 path で実現される。`ConversionContext::commit_history` 経由で直前 200 chars(暫定値、§13 Open Q 1)の context を Ranker に渡す。

### Coalescing window (候補 update 統合時間窓)

- **定義**: RankerWorker が複数 backend(SudachiDict / UserVocab / LearningCache / LLM)からの partial 候補を bundling し、1 回の `IMEHostBridge::update_candidates` 呼び出しに統合するための時間窓。Live 変換時 5-10ms、Commit 変換時 30ms と動的に切り替える。
- **初出**: Phase 3-A spec §7.3(2026-05-02)
- **対応する identifier**: `kotoha_engine_core::RankerWorker` 内部 logic(Phase 3-A 本番実装で確定)
- **備考**: window 値の最適値は §13 Open Q 2 で empirical 確定する。IBus `update_lookup_table` は仕様上全置換のため、coalescing で呼び出し回数を減らすことで描画 flicker を最小化する。

### ConversionContext (Ranker 入力 context)

- **定義**: Ranker `rank()` の引数として渡される struct。`commit_history: Vec<String>`(focus session 内の直前 N 文字 commit 履歴)、`time_since_last_commit: Duration`、`mode: ConversionMode`(Live / Commit)を含む。Phase 5 で `partial_input: Option<String>` / `typo_distance: u32` field が non-breaking で追加される予定。
- **初出**: Phase 3-A spec §4.3(2026-05-02)
- **対応する identifier**: `kotoha_engine_core::ConversionContext`
- **備考**: KotohaEngine 内部 state の `commit_history: VecDeque<String>` の snapshot を `Vec<String>` として clone して context に詰める設計(VecDeque は engine 内 pop_front 効率性、Vec は context 不変性と clone の単純さ)。

### StdCancellationToken (std::sync ベース cancel token impl)

- **定義**: Phase 3-A spec §4.4 で凍結された `CancellationToken` trait の Phase 3-A 初期 impl。`Arc<AtomicBool>` + `Mutex<Vec<Waker>>` で cancel signal の永続化と async future 待機を実装する。`Mutex` poison は `unwrap_or_else(PoisonError::into_inner)` で recover する(PR #111 規約と整合)。
- **初出**: P2-D Milestone 1(2026-05-02、ISSUE #120 / branch `feature/120-p2d-trait-skeleton`)
- **対応する identifier**: `kotoha_engine_core::cancel::StdCancellationToken`(`crates/kotoha-engine-core/src/cancel/std_token.rs`)
- **備考**: tokio runtime 導入は Phase 5 / 6 で再評価。それまで Phase 3-A は std::sync ベースで運用。同期 thread block-wait API は現状未提供、将来追加時に Condvar 復活と `wait_blocking()` method を同時導入する。

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

### Mixed JP/EN input (JP/EN 混在入力)

- **定義**: 日本語と英語が 1 行内で混在する romaji 入力ストリーム。例: `tuginocommitwoshuuseisitepull requestwodasite` はユーザー意図として「次のcommitを修正してpull requestを出して」を表し、`commit` / `pull request` は英字列のまま出力することが期待される。
- **初出**: ADR 0010 C4 / D8 (Phase 5 primary goal に mixed JP/EN 要件追加) / Phase 5 spec §1.5 / §3.5
- **対応する identifier**: Phase 5 custom model が context で判定する (explicit API は持たない予定、Phase 5 実装時に確定)
- **備考**: Phase 0 ADR 0002 が扱う Shift-triggered Transient Direct モードとは異なり、本要件は「行内に EN span が挟まる」ケースを対象とする。Phase 5 custom model の language-context detection (§3.5.1) が primary 対応、Tier 2 fallback として `InputMode::Latin` 新設案 (ADR 0010 Alternative E、現時点 Rejected) を保持する。

### Language-context detection (言語文脈検出)

- **定義**: Phase 5 custom model が input romaji stream 中で JP (かな / 漢字変換対象) と EN (英字出力対象) の boundary を context で判定する内部機構。decoder の hidden state に「現在 EN span 中か JP 変換対象か」の状態を保持する。
- **初出**: ADR 0010 D8 (Phase 5 primary goal) / Phase 5 spec §3.5.1
- **対応する identifier**: Phase 5 custom model 内部 (Phase 5 decoder の hidden state、explicit API は持たない予定)
- **備考**: 明示的 classifier head を decoder に追加する案ではなく、decoder hidden state の暗黙学習で担わせる方針を default とする (明示 classifier は fallback)。P5-A kick-off で empirical 妥当性を検証する。

### Context-aware space (文脈依存 space)

- **定義**: space 文字を単一意味の変換 trigger として扱わず、「EN 文脈では word separator、JP 文脈では区切り記号 (変換 boundary hint)」の 2 義として context で解釈する model-level semantics。
- **初出**: ADR 0010 D8 / Phase 5 spec §3.5.2
- **対応する identifier**: Phase 5 training data + Phase 5 decoder で学習 (explicit API なし、Phase 5 実装時に確定)
- **備考**: Phase 0 / Phase 1 の IME input layer は space = 単純な区切りとして扱ったが、Phase 5 では model が context で space の扱いを切り替える。space event の「IME layer 前処理 vs 生のまま decoder に渡す」の選択は Phase 5 kick-off で empirical 確定する (spec §3.5.2)。

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
