---
title: Kotoha Phase 2 設計書 (draft) — Dictionary and learning
date: 2026-04-25
status: draft
phase: 2
revision: 1
---

# Kotoha Phase 2 (Dictionary and learning) 設計書 — draft

本書は Phase 2 の設計書 **draft** である。本 draft の役割は ISSUE #85 時点で Phase 2 のスコープ / アーキテクチャ / open question を確定し、P2-A〜P2-D の各 milestone 実装を同一方針で進められる状態に整えることである。詳細パラメータ (SudachiDict core / full 選択、`BackendConfig` 新 variant 名、learning cache の永続化 format、Ranker 重みの最終値、Dictionary load timing、Phase 5 integration plan) は Phase 2 の P2-A kick-off 時点で empirical 確定する。

本書は ADR 0014 (Phase 2 dictionary layer architecture) を設計根拠とし、ADR 0011 (Backend trait design) / ADR 0012 (Feature flag design) / ADR 0013 (latency target) を前提として参照する。

## 目次

- [1. 背景と動機](#1-背景と動機)
- [2. スコープ](#2-スコープ)
- [3. アーキテクチャ](#3-アーキテクチャ)
- [4. Dictionary 設計](#4-dictionary-設計)
- [5. Learning cache 設計](#5-learning-cache-設計)
- [6. API と Trait 拡張](#6-api-と-trait-拡張)
- [7. テスト戦略](#7-テスト戦略)
- [8. 非スコープ (将来 phase)](#8-非スコープ-将来-phase)
- [9. Open questions](#9-open-questions)
- [10. 参照](#10-参照)

## 1. 背景と動機

### 1.1 Phase 1 14/15 baseline の位置付け

Phase 1 P1-2.5 follow-up (PR #76 / ISSUE #75 / merge commit `3eccaa1`) は、Gemma-2-2B-jpn-it Q5_K_M をバックエンドに Layer 3 smoke fixture 15 行を empirical 実測し、`14/15 PASS` を確定した。唯一 FAIL した row 3「あした → 明日」は、v5〜v12 の 8 世代 prompt iteration を実施しても Gemma-2-2B-jpn-it の ICL (In-context learning) 限界で解消できず、Phase 1 acceptance では 14/15 を受容 (ADR 0009 D5) + Phase 5 (ADR 0010) で task-specific fine-tune により根本解決する計画を確定した。

Phase 1 は P1-4 (PR #84, merge `95e7df9`) で close した。Phase 2 kick-off 時点の前提は「Gemma-2-2B-jpn-it 14/15 を Phase 1 baseline として維持し、Phase 2 では 14/15 を退行させない前提で辞書補完 + 学習を追加する」である。

### 1.2 Phase 5 完成までの 3〜6 ヶ月の品質底上げ需要

Phase 5 (ADR 0010) は data pipeline + training + evaluation で 3〜6 ヶ月の工数を見込む (ADR 0010 負の帰結)。Phase 5 完成を待つ間、Phase 3 IBus 統合 / Phase 4 fcitx5 統合 (Gemma baseline を推論器とした IME shipping) が進行する。IME として shipping する以上、row 3 類の synonym bias 以外にも、敬称 / 固有名詞の誤変換が顕在化する。これらは Gemma-2-2B-jpn-it の ICL だけでは補い切れず、Phase 5 完成を待たず Phase 2 で辞書補完層を提供する意義がある (ADR 0014 C2 参照)。

### 1.3 固有語の軽微問題の実例

Phase 1 の smoke fixture で扱わなかった語彙のうち、Phase 2 で dict 補完によって解消するべき実例は以下 3 類型である。具体 fixture は P2-A kick-off で golden テスト (§7.3) に組込む形で確定する。

- **敬称 (honorific)**: 「たなか さん」→「田中 さん」、「すずき さま」→「鈴木 様」。LLM 単体では「さん / 様」を一般名詞として解釈する誤変換が発生するため、敬称専用の token match 層を dict で担保する
- **固有名詞**: 人名 (「ひのおか」→「日野岡」)、地名 (「しんじゅく」→「新宿」)、組織名 (「ぐーぐる」→「Google」)。LLM 学習分布内に無い固有名詞は recall が極端に落ちるため、dict 補完で recall を底上げする
- **User 個別語彙**: 本人の名前 / 所属 / 業界固有語。LLM / System dict の双方に存在しない語彙は、User dict への明示登録のみで recall できる

### 1.4 Phase 2 で解決する具体目標

Phase 2 は以下 4 点を同時解決する。

- **G1**: Phase 1 14/15 baseline を退行させない (Phase 2 regression test で hard-gate)
- **G2**: 敬称 / 固有名詞を System dict 補完で recall 改善する
- **G3**: User 個別語彙を User dict への明示登録で覆える
- **G4**: Learning cache によるユーザ選択履歴の rerank 反映を実現する

詳細 KPI は P2-A kick-off で確定する。

## 2. スコープ

### 2.1 In-scope

Phase 2 の実装範囲は以下 5 項目とする。

1. **System dictionary**: SudachiDict (core variant または full variant, P2-A で empirical 選択) を runtime load する DictionaryBackend の実装
2. **User dictionary**: ユーザ個別語彙 entry の追加 / 削除 / 列挙 API と永続化 (TOML / JSONL / TSV のいずれか、P2-A で empirical 選択)
3. **Learning cache**: in-memory LRU + 起動時 load + shutdown save の 2 層構成。data model は `(kana_input, chosen_kanji, frequency, last_used_at)`
4. **Ranker**: Dictionary 候補と LLM 候補の merge / dedupe / rerank。初期重み dict 0.95 / LLM 1.0 + Learning cache hit bonus (P2-D で empirical tuning)
5. **BackendConfig 拡張**: ADR 0011 の `#[non_exhaustive]` 拡張点を使用し、Phase 2 用の新 variant を 1 つ追加する (variant 名は P2-A で確定)

詳細は P2-A kick-off で確定する。

### 2.2 Out-of-scope (後続 Phase に送る)

以下 4 項目は Phase 2 では扱わず、後続 Phase に送る。

1. **streaming IME input 統合**: preedit / commit / cancel の IME 状態機械との統合は Phase 3 (IBus) / Phase 4 (fcitx5) で行う。Phase 2 は CLI 経由の単発変換のみを対象とする
2. **personalization ML**: ユーザごとの微小 fine-tune layer / adapter は Phase 5 custom model (ADR 0010) + Phase 6 以降に送る。Phase 2 の learning cache はユーザ選択履歴の統計情報のみ保持する
3. **cross-device sync**: User dict / learning cache の複数機同期は Phase 6+ (UX polish) に送る。Phase 2 は単一 machine ローカルのみを対象とする
4. **GUI dict editor**: User dict を編集する GUI は Phase 6+ / Phase 7 に送る。Phase 2 は CLI サブコマンド (`kotoha-dict add / remove / list` の draft) のみを対象とする (P2-B §2.1 参照)

詳細は P2-A kick-off で再確認する。

## 3. アーキテクチャ

### 3.1 Dictionary layer

Dictionary layer は System dictionary + User dictionary の 2 層から構成する。

- **System dictionary**: SudachiDict (core variant 約 70MB または full variant 約 500MB) を runtime load する。default は SudachiDict-core を P2-A 初期 pilot で採用し、recall 不足が empirical に確認された場合 P2-D で full variant への変更を検討する
- **User dictionary**: ユーザ個別語彙 entry を保持する。entry schema は `(surface, reading, pos, score)` で、永続化 format は P2-A で確定する
- **merge 戦略**: System 辞書で hit した候補と User 辞書で hit した候補は Ranker で merge する。同一 surface が両方で hit した場合は User 側の score を優先する

詳細 entry schema / load timing は P2-A kick-off で確定する。

### 3.2 Learning cache

Learning cache は ユーザが選択した変換履歴を保持する。

- **data model**: `(kana_input, chosen_kanji, frequency, last_used_at)` のレコード
  - `kana_input`: 入力された hiragana 文字列
  - `chosen_kanji`: ユーザが確定した surface (漢字交じり)
  - `frequency`: 選択回数の累積 (u32)
  - `last_used_at`: 最終選択時刻 (UNIX epoch seconds)
- **persistence**: TOML or JSONL or TSV のいずれかを `~/.local/share/kotoha/learning.{toml|jsonl|tsv}` に配置する。P2-A で format を empirical 確定する。path は XDG Base Directory 仕様に従う
- **load/save timing**: プロセス起動時に全件 in-memory LRU に load、shutdown (SIGTERM / 正常終了) 時に全件 save する。Phase 3 IBus 統合では plugin life cycle hook に合わせて再設計する
- **eviction / pruning policy**: LRU 上限 (例: 10,000 entry) を超過した場合、`last_used_at` が最古の entry を evict する。上限値は P2-C で empirical 確定する

詳細は P2-A kick-off で確定する。

### 3.3 Ranker

Ranker は Dictionary 候補 (`Vec<Candidate>`) と LLM 候補 (`Vec<Candidate>`) を 1 つの top-k 結果に統合する。

- **merge**: 両 `Vec<Candidate>` を 1 本に連結する
- **dedupe**: 同一 surface の候補は score が高い方を残し、他方を除去する (ADR 0011 の `score_sort_dedupe` helper を流用する)
- **rerank**: 初期重み dict 0.95 / LLM 1.0 を `score *= weight` で掛け、Learning cache hit bonus を `score += bonus(frequency, last_used_at)` で加算する
- **truncate**: top_k (`ConvertOptions::top_k`) で上位のみ返す

初期重みと bonus 関数の最終値は P2-D で empirical tuning する (§7.3 golden fixture での pass rate を KPI とする)。

### 3.4 BackendConfig 拡張 (新 variant)

Phase 2 は ADR 0011 で確定した `BackendConfig` enum の `#[non_exhaustive]` 拡張点を使用し、新 variant を 1 つ追加する。P2-A kick-off で確定する 2 案は以下のとおり。

- **候補 1** (集約型): `BackendConfig::DictionaryAugmented { model_path, dict_config, learning_config }`
  - Dictionary 補完付き LLM backend を 1 variant に集約する
  - 実装コスト低、variant 数が最小
- **候補 2** (再帰 wrap 型): `BackendConfig::Hybrid { llm: Box<BackendConfig>, dict: DictionaryConfig, learning: LearningConfig }`
  - LLM backend を再帰的に wrap する
  - Phase 5 `KotohaNative` backend との組合せにも自動対応する
  - 実装コスト中、Phase 5 integration が構造化される

Phase 1 の `BackendConfig::LlamaCpp { model_path, prompt_template }` と `BackendConfig::Mock` は変更しない。詳細は P2-A kick-off で確定する。

## 4. Dictionary 設計

### 4.1 SudachiDict core vs full の trade-off

SudachiDict は core (約 70MB) と full (約 500MB) の 2 variant を提供する。Phase 2 の選択は P2-A 初期 pilot で empirical 確定するが、現時点で整理した trade-off は以下のとおり。

| 項目 | SudachiDict-core | SudachiDict-full |
|---|---|---|
| size | 約 70MB | 約 500MB |
| 収録語彙数 | 約 76 万 entries (lemma + 活用形含む、lemma 単位では約 20 万) | 約 90 万 entries |
| 固有名詞 recall | 中 | 高 |
| 配布ライセンス | Apache-2.0 | Apache-2.0 |
| IME 常駐 footprint | 許容 | Phase 1 の Gemma-2-2B-jpn-it Q5_K_M 1.92GB + SudachiDict-full 約 500MB で合算約 2.4GB に膨張 |

Phase 2 default は core を採用する方針とし、recall が P2-D の golden fixture で不足と判定された場合のみ full への切替を検討する。

詳細は P2-A kick-off で確定する。

### 4.2 Kotoha 独自語彙の merge

SudachiDict には含まれない Kotoha 固有の語彙 (例: 敬称「さん」「様」の専用 entry、IME 業務ドメイン語彙、Phase 1 smoke fixture 語彙) を薄い補完レイヤーとして SudachiDict 上に merge する。

- 補完 entry の保存先: `crates/kotoha-core/resources/kotoha-dict.tsv` (case 依存、P2-A で path 確定)
- schema: SudachiDict と同一の `(surface, reading, pos, score)` を採用
- merge timing: プロセス起動時、SudachiDict load 後に同一 in-memory 構造へ読込

詳細 schema と load timing は P2-A kick-off で確定する。

### 4.3 bundling vs runtime download

SudachiDict の配布方法は 2 案ある。P2-A kick-off で empirical 確定する。

- **bundling**: Cargo crate として辞書 data を同梱する案。初回 install が単純だが、crate size が膨張する
- **runtime download**: 初回起動時に HuggingFace / GitHub Releases から download する案。crate size は軽量だが、オフライン環境での初回起動が失敗する

Phase 2 default は MEMORY.md の「Model management future vision」方針 (Phase 6 UX で AutoDownload、Phase 1〜5 は manual placement) に揃え、manual placement 運用を採る。具体 path は `KOTOHA_SUDACHI_DICT_PATH` 環境変数で指定する (Phase 1 の `KOTOHA_LLAMA_MODEL_PATH` と同様の運用)。

詳細は P2-A kick-off で確定する。

### 4.4 dict update policy

SudachiDict の upstream update (概ね年 2 回) への追従方針を P2-A kick-off で確定する。現時点の方針候補は以下のとおり。

- **固定版**: Phase 2 kick-off 時点の version を pin し、Phase 2 closure まで変更しない
- **quarterly sync**: Phase 2 期間中、upstream release の 3 ヶ月遅れで version bump する
- **issue-triggered**: User 報告の recall 不足で upstream を確認し、必要時に bump する

Phase 2 default は「固定版」を採る。更新が必要となった場合は別 ADR / ISSUE を起票する。

## 5. Learning cache 設計

### 5.1 data model

レコード単位は `(kana_input, chosen_kanji, frequency, last_used_at)` とする。

- **kana_input**: 入力された hiragana 文字列 (`String`)
- **chosen_kanji**: ユーザが確定した surface (漢字交じり, `String`)
- **frequency**: 選択回数の累積 (`u32`)
- **last_used_at**: 最終選択時刻 (UNIX epoch seconds, `i64`)

Phase 2 では上記 4 フィールドに限定する。Phase 5 custom model の personalization で追加フィールド (context embedding, ユーザ profile ID 等) が必要となった場合は schema migration を行う (Phase 5 spec §7 と相互参照)。

### 5.2 persistence

永続化 format の候補は 3 案とする。P2-A で empirical 確定する。

| 候補 | 利点 | 欠点 |
|---|---|---|
| TOML | 人手で読み書き可能、parse library 成熟 | レコード数増で file size 膨張 |
| JSONL | 1 行 1 record で append 書込みが簡単 | parse cost がレコード数 N に比例 |
| TSV | 最小 format、parse 最速 | escape 規則の手作業 define が必要 |

default 案は「起動時 load + shutdown save の full rewrite」で運用できる TSV を第一候補とし、Phase 5 personalization で append 書込みが必要となった時点で JSONL への migration を検討する。

### 5.3 load/save timing

- **load**: プロセス起動時、`~/.local/share/kotoha/learning.{ext}` が存在すれば全件 in-memory LRU に load する。path 未存在の場合は空の LRU で起動する
- **save**: shutdown 時 (SIGTERM / 正常終了 / drop) に全件 save する。Phase 3 IBus 統合では plugin の life cycle hook (session 終了) で save する
- **autosave 閾値**: Phase 2 では autosave を行わない。shutdown save のみに限定する。Phase 5 以降で必要となった場合に ADR を起票する

詳細は P2-A kick-off で確定する。

### 5.4 eviction / pruning policy

Learning cache の in-memory LRU 上限を 10,000 entry とする (暫定値)。

- **eviction**: LRU 上限超過時、`last_used_at` が最古の entry を evict する
- **pruning**: `frequency == 1` かつ `last_used_at` が 30 日以上前の entry は load 時に除外する (noise 低減)

上限値と pruning 閾値は P2-C で empirical 確定する。

## 6. API と Trait 拡張

### 6.1 既存 Backend trait を壊さず新 variant で対応する

ADR 0011 で確定した `KanjiBackend` trait (method 2 本: `convert` / `model_id`) は変更しない。Phase 2 は以下を新規追加する。

- 新 struct: Dictionary 補完付き backend を実装する 1 つ以上の struct (命名は P2-A で確定、候補: `HybridBackend` / `DictionaryAugmentedBackend`)
- 新 `BackendConfig` variant: §3.4 の候補 1 / 候補 2 から P2-A で選択した 1 つ
- `load_backend` factory の新 arm: 新 variant を dispatch する arm を追加 (ADR 0011 D3 と同一パターン)
- 新 feature flag: ADR 0012 の方針に従い `dict` / `learning` 等の optional dependency を隔離する feature flag を追加 (命名は P2-A で確定)

既存 `LlamaCppBackend` / `MockBackend` / `BackendConfig::LlamaCpp` / `BackendConfig::Mock` は変更しない。Phase 1 14/15 baseline の退行を避けるため、Phase 2 regression test (§7.4) を追加する。

詳細は P2-A kick-off で確定する。

### 6.2 ConvertOptions 拡張の検討

Phase 2 で新規に必要となる option を整理する。2 案ある。

- **案 1** (既存拡張): 既存の `ConvertOptions` 型に `use_dictionary: bool` / `use_learning_cache: bool` 等の field を追加する
- **案 2** (別型分離): `ConvertOptions` は Phase 1 のまま維持し、別型 `Phase2ConvertOptions { base: ConvertOptions, use_dictionary: bool, ... }` を新設する

default 案は「案 1 の既存拡張」とする。P2-A kick-off 着手前に `crates/kotoha-core/src/kanji/` 配下の `ConvertOptions` 定義で `#[non_exhaustive]` の有無を確認する。付いていなければ P2-A 最初の commit で属性を追加する (ADR 0006 方針との整合上必要)。新 field は `Default::default()` の boolean default を `true` として「Phase 2 機能が default で on」の挙動を与える。

詳細は P2-A kick-off で確定する。

### 6.3 Dictionary 単体 crate を分離するか kotoha-core に含めるか

Dictionary layer の配置方針は 2 案ある。

- **案 1** (kotoha-core 内配置): `crates/kotoha-core/src/dict/` に module として配置する。Phase 2 の実装量が Phase 1 の `kanji/` module 程度に留まる場合は案 1 で十分
- **案 2** (kotoha-dict 別 crate): `crates/kotoha-dict/` を新規追加する。SudachiDict 依存が `kotoha-core` の build を重くする場合、別 crate に分離して feature flag で隔離する

default 案は「案 1 の kotoha-core 内配置 + feature flag `dict` で隔離」とする。Phase 2 の実装量が案 1 の想定を超えた場合、P2-D で案 2 への migration を検討する。

詳細は P2-A kick-off で確定する。

## 7. テスト戦略

### 7.1 unit test

以下 3 component を `#[cfg(test)]` 単位で unit test する。

- **Dictionary lookup**: System dict / User dict の lookup が期待 entry を返すこと。merge 優先 (User > System) が動作すること
- **Learning cache**: LRU eviction が正常動作すること。frequency 加算 / last_used_at 更新が正確であること。persistence (load / save round-trip) が破損しないこと
- **Ranker**: Dictionary 候補と LLM 候補の merge / dedupe / rerank が §3.3 の契約どおり動作すること。初期重み 0.95 / 1.0 の score 計算が正確であること

Phase 1 の unit test 方針 (`crates/kotoha-core/src/kanji/backend.rs` 末尾の `mod tests`) を踏襲する。

### 7.2 integration test

hybrid backend の end-to-end を `crates/kotoha-core/tests/` 配下で integration test する。

- MockBackend を LLM として固定し、Dictionary 候補 / Learning cache 候補 / Ranker の 3 層が end-to-end で統合動作すること
- `load_backend` factory の新 arm が Phase 2 新 variant を正しく dispatch すること

Layer 2 (GGUF なし) で完結し、`llama-cpp` feature 非依存で動作する設計とする (ADR 0012 D5 `default = []` 厳守方針)。

### 7.3 golden fixture

Phase 2 固有の golden fixture を以下 4 カテゴリ、30+ cases を最小として P2-A で整備する。

- **敬称**: 「たなか さん → 田中 さん」「すずき さま → 鈴木 様」等、10+ cases
- **固有名詞 (人名)**: 「ひのおか → 日野岡」「やまだ たろう → 山田 太郎」等、10+ cases
- **固有名詞 (地名 / 組織名)**: 「しんじゅく → 新宿」「ぐーぐる → Google」等、5+ cases
- **外来語**: 「こんぴゅーた → コンピュータ」「いんたーねっと → インターネット」等、5+ cases

fixture 形式は Phase 1 の `crates/kotoha-core/tests/fixtures/kanji_llama_cpp_smoke.tsv` と同一 TSV schema を採用する。

詳細 case 数と category 比率は P2-A kick-off で確定する。

### 7.4 regression (Phase 1 14/15 退行防止)

Phase 1 の Layer 3 smoke fixture 15 行を Phase 2 でも継続実行する。Phase 2 の新規変更 (Dictionary 補完 / Learning cache / Ranker) で Phase 1 14/15 baseline が退行しないことを hard-gate する。

- 実行経路: `crates/kotoha-core/tests/kanji_llama_cpp_smoke.rs` + Phase 2 新 backend variant での再実行
- 期待結果: row 3 を除く 14 行が Phase 2 の新 backend でも PASS すること
- gate 方針: Phase 2 の PR merge 条件に「Phase 1 smoke 14/15 が Phase 2 backend で maintain されていること」を含める (ADR 0013 の latency gate 無し方針とは独立に、quality gate は設ける)

詳細は P2-A kick-off で確定する。

## 8. 非スコープ (将来 phase)

Phase 2 のスコープから以下を明示的に除外する。

### 8.1 streaming IME input

preedit / commit / cancel / 候補選択 UI との IME 状態機械統合は Phase 3 (IBus) / Phase 4 (fcitx5) で行う。Phase 2 は CLI 経由 (`kotoha-kanji` binary) の単発変換のみを対象とする。

### 8.2 personalization ML

ユーザごとの微小 fine-tune layer / adapter / embedding は Phase 5 custom model (ADR 0010) + Phase 6 以降に送る。Phase 2 の learning cache はユーザ選択履歴の統計情報 (`frequency` / `last_used_at`) のみ保持し、ML 的な personalization は行わない。

### 8.3 cross-device sync

User dict / learning cache の複数機同期は Phase 6+ (UX polish) に送る。Phase 2 は単一 machine ローカルのみを対象とする。

### 8.4 GUI dict editor

User dict を編集する GUI は Phase 6+ / Phase 7 に送る。Phase 2 は CLI サブコマンド (`kotoha-dict add / remove / list`) の draft のみを対象とする (P2-B 範囲)。

## 9. Open questions

P2-A kick-off 時点で解消する 6 点を以下に列挙する。

| # | Question | 解消 milestone |
|---|---|---|
| Q1 | SudachiDict-core (70MB) と full (500MB) のどちらを Phase 2 default とするか | P2-A kick-off 初週 (pilot recall 比較) |
| Q2 | `BackendConfig` 新 variant 名を `DictionaryAugmented` / `Hybrid` のどちらにするか | P2-A kick-off で確定 |
| Q3 | Learning cache 永続化 format を TOML / JSONL / TSV のどれにするか | P2-A kick-off で確定 |
| Q4 | Ranker 重みの defaults (dict 0.95 / LLM 1.0) を P2-D 完了時点でどう empirical tuning するか | P2-D で golden fixture の pass rate で確定 |
| Q5 | Dictionary load を compile-time 埋込 / runtime load のどちらにするか | P2-A kick-off で確定 (default は runtime load) |
| Q6 | Phase 5 integration plan — Phase 5 `KotohaNative` custom model が Phase 2 の Dictionary / Learning 層を継承するか、別 backend として独立構築するか | Phase 5 kick-off で確定 (Phase 2 の時点では「継承可能な設計を保つ」方針を D6 で定める) |

Q6 は Phase 5 spec §9 Q6 と相互参照する。Phase 2 で Dictionary / Learning cache を backend-agnostic な module として配置しておくことで、Phase 5 custom model が Phase 2 の同一 module を使える構造を担保する。

詳細は P2-A kick-off で確定する。

## 10. 参照

- ADR 0014 (Phase 2 方針決定): `docs/adr/0014-phase-2-dictionary-layer-architecture.md`
- ADR 0011 (Backend trait 拡張点): `docs/adr/0011-kanji-backend-trait-design.md`
- ADR 0012 (feature flag 方針): `docs/adr/0012-feature-flag-design-for-llama-cpp.md`
- ADR 0013 (latency policy): `docs/adr/0013-phase-1-latency-target.md`
- ADR 0010 (Phase 5 custom model 方針): `docs/adr/0010-kotoha-custom-romaji-base-model.md`
- ADR 0009 (Phase 1 default model): `docs/adr/0009-phase-1-default-model-selection.md`
- Phase 1 設計書 (14/15 baseline の根拠): `docs/superpowers/specs/2026-04-24-kotoha-phase-1-design.md`
- Phase 5 設計書 (stub): `docs/superpowers/specs/2026-04-25-kotoha-phase-5-custom-model.md`
- Phase 1 P1-2.5 follow-up 実装ログ (row 3 ICL 限界の empirical 記録): `docs/wbs/2026-04-24-feature-75-prompt-optimization-15-row-fixture.md`
- Phase 1 close 記録: `docs/wbs/2026-04-25-docs-83-p1-4-phase1-wrap.md`
- ROADMAP restructure 反映先: `docs/ROADMAP.md` Phase 一覧 + Phase 2 マイルストーン分割節
- SudachiDict upstream: <https://github.com/WorksApplications/SudachiDict>
- Sudachi (形態素解析器本体): <https://github.com/WorksApplications/Sudachi>

本 draft の詳細化は Phase 2 P2-A kick-off で実施する。
