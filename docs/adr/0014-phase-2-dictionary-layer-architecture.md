# ADR 0014 — Phase 2 dictionary layer architecture (Sudachi-based hybrid)

- **Status**: Accepted (2026-04-25、方針として承認。詳細パラメータは P2-A kick-off 時に確定)
- **Date**: 2026-04-25
- **Deciders**: Kotoha Phase 2 maintainers
- **Related ISSUE**: #85 (Phase 2 foundation docs)
- **Related PRs**: #74 (P1-2.5 refactor, Backend trait 最終形)、#76 (P1-2.5 follow-up, 14/15 baseline)、#82 (P1-3 CLI)、#84 (P1-4 Phase 1 close)

## Context

Kotoha Phase 1 は P1-4 (PR #84, merge `95e7df9`) で close した。Phase 1 の結論として以下 4 点が確定している。

### C1. Phase 1 14/15 baseline の残存 row 3 問題

Phase 1 P1-2.5 follow-up (PR #76, merge `3eccaa1`) は Gemma-2-2B-jpn-it Q5_K_M をバックエンドに Layer 3 smoke fixture 15 行を empirical 実測し、`14/15 PASS` を達成した。唯一 FAIL した row 3「あした → 明日」は v5〜v12 の 8 世代 prompt iteration で解消不能と確定し、Phase 1 では Gemma-2-2B-jpn-it の ICL (In-context learning) 限界として受容した。Phase 5 (ADR 0010, Kotoha custom romaji-base model) で task-specific fine-tune により根本解消する方針が確定している。

### C2. Phase 5 完成までの 3〜6 ヶ月の品質底上げ需要

Phase 5 は data pipeline + training + evaluation で 3〜6 ヶ月の工数を見込む (ADR 0010 負の帰結)。Phase 5 完成を待つ間、Phase 3 IBus 統合 / Phase 4 fcitx5 統合 (Gemma baseline を推論器とした IME shipping) が進行する。IME として shipping する以上、row 3 類の synonym bias 以外にも、敬称 (「たなか さん」→「田中 さん」) や固有名詞 (人名 / 地名 / 組織名) の誤変換が顕在化する。これらは Gemma-2-2B-jpn-it の ICL だけでは補い切れず、Phase 5 完成を待たず Phase 2 で辞書補完層を提供する意義がある。

### C3. 固有名詞と敬称の dictionary 補完価値

固有名詞 (「鈴木」「山田」「新宿」) と敬称 (「さん」「様」「殿」) は、LLM の synonym bias よりも「学習分布内に存在するか否か」が recall を決める。SudachiDict (WorksApplications, Apache-2.0) は約 76 万 entries (lemma + 活用形含む、lemma 単位では約 20 万) の System 辞書を提供しており、Phase 2 で dictionary lookup 層を追加すれば、LLM 単体で recall が不足する語彙を構造的に補える。User dict を併設すれば、ユーザ個別語彙 (自分の名前 / 所属組織名 / 業界固有語) も明示登録で覆える。

### C4. SudachiDict Apache-2.0 の OSS 互換性

Kotoha は OSS として公開予定であり、依存辞書の再配布ライセンスが採用可否を支配する。SudachiDict は Apache-2.0 で公開されており、Kotoha 本体想定 OSS ライセンスと両立する。MeCab + UniDic を組合せる選択肢は、UniDic の配布条件が商用利用で制約を受ける運用形態があり、Phase 2 で採用すべきではない (`docs/wiki/glossary.md` §8 MeCab / UniDic / fugashi 参照)。Phase 5 P5-A PoC でも SudachiDict を採用済であり、Phase 2 で同一辞書を流用することで学習データと推論 runtime の語彙整合も取れる。

## Decision

本 ADR では以下 6 項目を決定する。

### D1. Sudachi hybrid architecture を Phase 2 の default とする

Phase 2 の default architecture を「LlamaCpp backend (Gemma-2-2B-jpn-it baseline) と Dictionary backend (Sudachi-based) を hybrid に組合せる二段構成」とする。LLM は汎用的な変換 recall を、Dictionary は固有名詞 / 敬称 / User 登録語彙の高精度 recall を、それぞれ担当する。両者の候補は後段 Ranker で merge / dedupe / rerank し、top-k を確定する。

hybrid 採用により Phase 1 の 14/15 baseline を退行させず、かつ固有名詞 / 敬称の recall を追加できる。

### D2. Dictionary を 2 層 (System + User) で構成する

Dictionary layer は以下 2 層から成る。

- **System dictionary**: SudachiDict (core variant または full variant) を runtime load する。Phase 2 の default recall 源として働く
- **User dictionary**: ユーザが明示登録する個別語彙 (人名 / 所属 / 業界語 / macro 展開) を保持する。永続化形式 (TOML / JSONL / plain-text tsv) は P2-A kick-off で確定する

2 層構成により、System 辞書の汎用 recall と User 辞書の個別 recall を独立に拡張できる。User 辞書の更新は System 辞書の再配布サイクルに束縛されない。

### D3. Learning cache を in-memory LRU + 永続化ペアで持つ

ユーザが選択した変換履歴を learning cache として保持する。データ構造は以下とする。

- **runtime 構造**: in-memory LRU cache。`(kana_input, chosen_kanji)` を key とし、`frequency` / `last_used_at` を value とする
- **永続化形式**: TOML or JSONL (P2-A kick-off で empirical 確定する。初期候補: `~/.local/share/kotoha/learning.tsv` に TSV で保存)
- **load / save timing**: プロセス起動時に全件 load、shutdown 時に全件 save。Phase 3 IBus 統合では plugin の life cycle hook に合わせて見直す

Learning cache は Ranker の入力 feature として使用し、User が繰返し選択する候補の rerank 係数を上げる。

### D4. BackendConfig に新 variant を 2 段階で追加する

Phase 2 は ADR 0011 で確定した `BackendConfig` enum の `#[non_exhaustive]` 拡張点を使用し、Phase 2 期間中に以下 2 variants を段階的に追加する。

- **P2-A**: `BackendConfig::Dictionary { config: DictionaryConfig }`(集約型、LLM を含まない pure Dictionary backend)
  - 形態素解析と vocabulary lookup の合成のみを担当する。LLM 推論を内包しないため、`llama-cpp` feature 非依存で常に build / 単体実行可能となる
  - P2-A の Layer 1 / Layer 2 テストはこの variant を介して MorphologicalEngine + VocabularyLookup の合成を end-to-end 検証する
- **P2-D**: `BackendConfig::Hybrid { llm: Box<BackendConfig>, dict: DictionaryConfig, learning: LearningConfig }`(再帰 wrap 型)
  - LLM backend を再帰的に wrap し、Dictionary 候補 / LLM 候補 / Learning cache hit を Ranker で統合する
  - `llm: Box<BackendConfig>` 形式により、Phase 5 で `BackendConfig::KotohaNative` が加入した際にも自動対応できる(再 wrap で同じ Hybrid variant を再利用可能)

本判断の根拠は P2-A spec §3.3 Q3(`docs/superpowers/specs/2026-04-25-kotoha-phase-2-p2-a-dictionary-foundation.md` §3.3 Q3)に記載する。集約型を P2-A、再帰 wrap 型を P2-D と分割する理由は、(a) P2-A 段階では LLM 統合を持ち込まず純粋な Dictionary backend として独立に検証したい、(b) Phase 5 KotohaNative の自動対応(Hybrid 側の責務)を Phase 2 段階で完結させ、Phase 5 で再設計を不要にする、の 2 点である。

Phase 1 の `BackendConfig::LlamaCpp { model_path, prompt_template }` と `BackendConfig::Mock` は変更しない。既存 variant の破壊を伴わない純粋な拡張として 2 variants を追加する。

### D5. Ranker 戦略を「dict 0.95 / LLM 1.0 初期重み」から開始する

Dictionary 候補と LLM 候補を merge する際の rerank 重みを、初期値として以下に設定する。

- **LLM 候補の base weight**: 1.0
- **Dictionary 候補の base weight**: 0.95
- **Learning cache hit bonus**: +frequency 依存の加算 (P2-D で empirical 確定)

dict < LLM とする理由は、Phase 1 の 14/15 baseline を前提に、LLM が出せる汎用語彙 recall の信頼度が dict 単発 hit より一般的に高いためである。ただし敬称や固有名詞の hit に対しては dict 側の信頼度が逆転するケースが多く、P2-D で empirical tuning する。

### D6. 既存 Backend trait を壊さず拡張する (ADR 0011 との整合)

本 ADR の全ての新 variant / 新構成は、ADR 0011 で確定した `KanjiBackend` trait を実装する新 struct として導入する。trait 本体 (`convert` / `model_id` method 2 本) は変更しない。`validate_input` と `score_sort_dedupe` の共有 helper も既存を流用する。

Phase 5 で `KotohaNative` backend が追加される際も、本 ADR の Dictionary + Learning 構成をそのまま継承できるよう、Ranker と Learning cache は backend-agnostic な形で kotoha-core crate 内に配置する (具体配置は P2-A kick-off で確定)。

### D7. P2-B で `kotoha-storage` crate を導入し SQLite 共用 DB を採用する

P2-B kick-off brainstorming (2026-04-25) は、Phase 2 spec §5.2 の旧 TSV / JSONL / TOML 第一候補を再評価し、UserVocab (P2-B) と LearningCache (P2-C) の永続化層に SQLite を採用する判断を確定した。具体は ADR 0015 (`docs/adr/0015-kotoha-storage-sqlite-adoption.md`) に分離して記載する。本 D7 節は本 ADR 0014 と ADR 0015 の整合関係のみを明示する。

- **Decision**: ADR 0015 を参照、UserVocab + LearningCache は単一 DB ファイル `kotoha.db` の `user_vocab` table + `learning_cache` table に格納する。SQLite library は `rusqlite + bundled` を採用し、新 crate `crates/kotoha-storage/` を導入して `rusqlite` の C 依存を 1 crate に閉じ込める
- **Rationale**: ACID + WAL によるアプリレベル lock 不要、`PRAGMA user_version` + `include_str!` 同梱 SQL による正式 versioned migration、Phase 5 personalization で field 追加 (例: `context_embedding`) が `ALTER TABLE` 1 文で完結する。desktop standalone IME 方針 (ADR 0010 / Phase 5 spec) と整合し、PostgreSQL / Docker 採用 (Docker daemon 起動を IME runtime の前提条件にする案) は ADR 0015 で却下した
- **Consequences**:
  - **`BackendConfig` には新 variant を追加せず**、`DictionaryConfig.user_vocab_db_path: Option<PathBuf>` field 追加で対応する。D4 改訂版の「P2-A 集約型 + P2-D 再帰 wrap 型」の 2 variants 計画は本 D7 で変更しない (UserVocab は P2-A `BackendConfig::Dictionary { config }` の `DictionaryConfig` 拡張として組込まれ、Hybrid variant は P2-D で追加される)
  - **`kotoha-storage` 新 crate は `kotoha-core` に逆依存しない**: Clean Architecture「Interface 依存」/ SOLID DIP に整合させ、`UserVocabStore` trait は `kotoha-storage` 側に置く
  - **Phase 2 spec §5.2 の旧 TSV / JSONL / TOML 比較は archived**: 本 P2-B docs PR で parent-spec §5.2 を SQLite 採用根拠に後付け改訂し、§9 Open questions Q3 を「P2-B kickoff brainstorming (2026-04-25、ADR 0015)」で解消フラグ付きに更新する

詳細な比較表 (TSV / CSV / JSONL / TOML / JSON / SQLite の 6 軸比較)、PostgreSQL / Docker 案の不採用根拠、desktop runtime 採用実績 (Firefox places.sqlite / Chrome cookies / iOS Photos.sqlite 等) は ADR 0015 に記載する。

## Consequences

### 正の帰結

- **Phase 5 完成を待たずに固有名詞 / 敬称 failure を解消できる**: Phase 5 (ADR 0010) の 3〜6 ヶ月工数を待たず、Phase 2 で IME として実用水準の recall を提供できる
- **User dict で個人化が可能になる**: 自分の名前 / 所属 / 業界語を明示登録でき、Gemma baseline の学習分布に依存しない recall が構造的に確保できる
- **Learning cache の data path が確立される**: Phase 5 custom model の personalization (Phase 6 以降) でも同じ data path を流用できる。Phase 2 の learning.tsv format が Phase 5 / Phase 6 の data interface として機能する
- **Phase 5 との互換性が設計段階で確保される**: D6 により Phase 5 `KotohaNative` backend が追加された際も、Dictionary + Learning 構成を再利用可能である (Phase 2 spec §9 Q6 で継承可否を判断する)
- **ADR 0011 / 0012 の拡張点が実地で検証される**: `#[non_exhaustive]` enum と feature flag 方針が Phase 2 の新 variant 追加で動作することを empirical に確認できる

### 負の帰結

- **2 layer (LLM + Dictionary) の保守コストが追加される**: Phase 1 の LLM 単層と比較し、Dictionary 初期化 / User dict 永続化 / Learning cache sync の 3 path を Phase 2 で追加実装する必要がある
- **Rerank tuning が P2-D に集中する**: D5 の初期重み (dict 0.95 / LLM 1.0) は empirical tuning を前提とし、P2-D で 100〜200 件規模の fixture で evaluation する工数が要る。D5 の値は暫定であり、P2-D の結果で再確定する
- **Sudachi binary の 70〜500MB footprint が常駐サイズに上乗せされる**: SudachiDict-core は約 70MB、full は約 500MB。Phase 1 の Gemma-2-2B-jpn-it Q5_K_M (1.92GB) と合算すると、core 採用で約 2.0GB、full 採用で約 2.4GB の常駐 footprint になる。Phase 2 default は core、full は opt-in に限定する方針。Phase 5 custom model (ADR 0010 D6, ≤ 200MB) への移行で total size は改善見込みだが、Phase 2 単体では footprint 増が避けられない

### Phase 2 spec との整合

本 ADR の 6 決定は Phase 2 spec (`docs/superpowers/specs/2026-04-25-kotoha-phase-2-design.md`) §3 アーキテクチャ / §4 Dictionary 設計 / §5 Learning cache 設計 / §6 API と Trait 拡張 の 4 節と相互参照する。spec の各節末尾には「詳細は P2-A kick-off で確定する」の stub disclaimer を付し、ADR と spec が同期的に P2-A で確定する構造とする。

## Alternatives considered

以下 4 案を検討し、いずれも棄却した。

### A. Dictionary only (LLM 無し)

**Rejected.** Phase 1 の 14/15 baseline は LLM が担保しており、Dictionary 単独では汎用語彙 recall が大幅に低下する。Gemma-2-2B-jpn-it が正しく変換する一般語 (動詞活用 / 助詞接続 / 慣用句) を dict 単独で recall するには、辞書 size を UniDic full 級まで拡大しつつ N-gram 統計モデルを加える必要があり、Phase 2 の工数目安 (P2-A〜D 合計 3〜6 週) を大幅に超過する。

### B. LLM only 継続 (Phase 2 を skip)

**Rejected.** Phase 5 完成までの 3〜6 ヶ月を Gemma baseline のまま shipping すると、固有名詞 / 敬称 / User 個別語彙の recall が改善されない。IME として実用水準に届かず、Phase 3 IBus 統合 / Phase 4 fcitx5 統合の shipping 品質が底上げされない。C2 で論じた「Phase 5 完成を待たない品質底上げ」目的と両立しない。

### C. 3rd party mozc-rs 呼出し

**Rejected.** mozc-rs (Google 日本語入力のエンジンを Rust からバインディングする crate) は活発度が低く、Kotoha の最小依存方針 (Cargo workspace で全 crate version を pin する運用) と整合しにくい。また mozc の辞書 / 変換エンジン全体を取込むと Kotoha の Backend trait 設計 (ADR 0011) との interface 不整合が発生し、Phase 5 `KotohaNative` backend への継承パス (D6) も断ち切られる。Dictionary 補完は Kotoha 内製で構築し、必要な辞書 data のみ SudachiDict から流用する方針を採る。

### D. 独自 dict format only (SudachiDict を採用しない)

**Rejected.** SudachiDict は Apache-2.0 / 約 76 万 entries (lemma + 活用形含む) / 形態素解析済 / Phase 5 P5-A PoC で採用実績あり、の 4 条件を満たす。独自 dict format を Phase 2 で新設すると、(a) 約 76 万 entries 規模の初期辞書作成工数が別途発生し、(b) Phase 5 data pipeline (ADR 0010 Phase 5 spec §3.1) との辞書整合が取りにくくなる。Kotoha 独自語彙は SudachiDict の上に薄い補完レイヤーとして merge するのが合理的である (Phase 2 spec §4.2 参照)。

## Related documents

- 実装前提: ADR 0009 (`docs/adr/0009-phase-1-default-model-selection.md`) — Gemma-2-2B-jpn-it default 採用
- Backend trait 拡張点: ADR 0011 (`docs/adr/0011-kanji-backend-trait-design.md`) — `BackendConfig` `#[non_exhaustive]` enum + `load_backend` factory
- Feature flag 方針: ADR 0012 (`docs/adr/0012-feature-flag-design-for-llama-cpp.md`) — optional dependency 隔離
- Latency policy: ADR 0013 (`docs/adr/0013-phase-1-latency-target.md`) — Phase 2 での hard-gate 無し方針
- Phase 5 継承関係: ADR 0010 (`docs/adr/0010-kotoha-custom-romaji-base-model.md`) — Phase 5 `KotohaNative` backend、Phase 5 spec §3.3 と本 ADR D6 / Phase 2 spec §9 Q6 で整合
- Phase 2 設計書: `docs/superpowers/specs/2026-04-25-kotoha-phase-2-design.md` (本 ADR と同一 PR で起票)
- Phase 1 14/15 empirical record: `docs/wbs/2026-04-24-feature-75-prompt-optimization-15-row-fixture.md` (PR #76 merge `3eccaa1`)
- Phase 1 close 記録: `docs/wbs/2026-04-25-docs-83-p1-4-phase1-wrap.md` (PR #84 merge `95e7df9`)
- ROADMAP restructure 反映先: `docs/ROADMAP.md` Phase 一覧テーブル + Phase 2 マイルストーン分割節
- 参考辞書: SudachiDict (Apache-2.0, <https://github.com/WorksApplications/SudachiDict>)

## Note: 本 ADR の status について

本 ADR は方針として Accepted とするが、確定状況は以下のとおり項目別に異なる。

- **D4 (BackendConfig 新 variant 構成)**: P2-A 設計時点で「P2-A: 集約型 `Dictionary { config }`、P2-D: 再帰 wrap 型 `Hybrid { llm, dict, learning }`」の 2 段階追加方針として確定済 (本 ADR 上記 D4 改訂、および P2-A spec §3.3 Q3 に根拠を記載)
- **D5 (Ranker 重み)**: 初期値 dict 0.95 / LLM 1.0 を暫定値とし、P2-D の golden fixture (530 cases) で empirical tuning する。Layer 3 の 530-case fixture と golden test runner、`phase2-dict-smoke.sh` は P2-A 範囲外として ISSUE #92 で別 PR にて実装する
- **D3 (learning cache 永続化 format)**: P2-C 着手時に empirical 確定する
- **D2 (SudachiDict core/full)**: P2-A 初期 pilot で recall を測定し core/full を確定する。Phase 2 default は core を採用し、recall 不足が P2-D で確認された場合のみ full への切替を検討する

P2-D 完了時点で本 ADR に「確定パラメータ」節を追記し、Phase 2 closure で内容を最終化する。
