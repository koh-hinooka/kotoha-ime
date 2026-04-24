# ADR 0009 — Phase 1 default model selection (Gemma-2-2B-jpn-it)

- **Status**: **Accepted** (2026-04-25, Phase 1 P1-4 で正式化)
- **Date**: 2026-04-25
- **Deciders**: Kotoha Phase 1 maintainers
- **Supersedes**: 本 ADR が prep note `0009-kanji-backend-model-selection-prep.md` を置き換える (file rename 済、git history は `git log --follow` で追跡可能)

## Context

Kotoha Phase 1 (Kana→Kanji conversion) の default 推論モデルを確定する決定である。Phase 1 kick-off 時点の spec §3.2 では `Zenz-v2.5-medium` (Miwa-Keita 配下) を default 候補として記述していたが、P1-2 着手後の empirical verification で 2 段階の pivot を実施した。本 ADR は pivot 後の最終状態 (Gemma-2-2B-jpn-it Q5_K_M を Phase 1 default に採用する決定) を Accepted として記録する。

### C1. Zenz family の load 失敗 (P1-2-9 empirical verification, WBS commit `718fd8e`)

Miwa-Keita 配下の Zenz GGUF (`v1` / `v2` / `v2.5-medium [gated]` / `v3.1-small` / `v3.1-xsmall`) はいずれも `tokenizer.ggml.pre = "gpt2-small-japanese-char"` を使用する。llama-cpp-2 0.1.145 (bundled llama.cpp commit `e21cdc11`) および upstream llama.cpp master の pre-tokenizer allow-list には `gpt2-small-japanese-char` が登録されておらず、Kotoha が Zenz family を load しようとすると architectural blocker として load 失敗する。llama-cpp-2 の version bump 内での解消見込みは無い (upstream 追従が必要)。

### C2. 3-way empirical 比較 (P1-2-9)

Zenz 系の代替として 3 モデルを Layer 3 smoke 5 cases で比較した結果、以下となった。

| モデル | 5/5 PASS | 敬称 `やまださん → 山田さん` | Size |
|---|---|---|---|
| Qwen2.5-1.5B-Instruct Q5_K_M | 4/5 | FAIL (「山田さん」不生成) | 1.29 GB |
| Gemma-2-2B-jpn-it Q5_K_M | **5/5** | **PASS** | 1.92 GB |
| Gemma-3-1B-it Q5_K_M | 2/5 | hallucination 発生 | 0.85 GB |

Gemma-2-2B-jpn-it が 5/5 を達成した唯一のモデルであり、Phase 1 default 候補として選定した。

### C3. Backend の汎用化 (P1-2.5 refactor, PR #74 merge `02cf035`)

Zenz 専用だった `ZenzBackend` を llama.cpp-family 汎用の `LlamaCppBackend` に rename / 拡張し、`PromptTemplate` enum (`Gemma2InstructChat` / `Qwen2Chat` / `Custom`) で prompt 構築を dispatch する構造に refactor した。`BackendConfig::Zenz` を `BackendConfig::LlamaCpp { model_path, prompt_template }` に置換し、Phase 2+ で新 chat-family model を追加する際の拡張点を確立した。

### C4. Prompt 方式の pivot (P1-2.5 follow-up, PR #76 merge `3eccaa1`)

P1-2.5 本体 (PR #74) では `llama-cpp-2 0.1.145::apply_chat_template` に `tokenizer.chat_template` (GGUF 埋め込み) を渡す chat-turn 経路を採用し、Layer 3 fixture を 9 行で 9/9 PASS に抑えた。後続の P1-2.5 follow-up (PR #76) で 15 行 fixture に復元する際、chat-turn 経路は Gemma-2-2B-jpn-it の conversational echo を誘発し 8/15 で頭打ちとなったため、**plain-text completion + v12 few-shot prompt** 方式に pivot した。最終 prompt の構成は (a) 厳格 directive、(b) 13 pair positive few-shot、(c) 3 pair negative 対照、(d) `入力: / 出力: ` query suffix の 4 要素である。

### C5. row 3「あした → 明日」の ICL 限界

v12 prompt + plain-text completion の最終測定は 14/15 PASS であり、row 3「あした」のみが「翌日」を出力し続けた。v5〜v12 の 8 世代で positive few-shot 隣接配置、negative example 埋込、直接禁止指示のいずれも row 3 を解消できなかった。同種の negative example「ぎゅうにゅう → 牛乳 (not ミルク)」「りょうり → 料理 (not クッキング)」は v12 で解消しており、row 3 のみが解消しない原因は Gemma-2-2B-jpn-it の 2B parameter instruction-tuning で形成された pretrain 語彙バイアス (「あした ↔ 翌日」の意味的隣接性が強固) が prompt 信号強度を上回っているためと推定する。詳細は WBS `docs/wbs/2026-04-24-feature-75-prompt-optimization-15-row-fixture.md` に記録した。

## Decision

本 ADR では以下 6 項目を決定する。

### D1. Phase 1 default model を Gemma-2-2B-jpn-it Q5_K_M とする

- HuggingFace repo: `bartowski/gemma-2-2b-jpn-it-GGUF`
- ファイル名: `gemma-2-2b-jpn-it-Q5_K_M.gguf`
- Size: 約 1.92 GB
- License: Gemma License (再配布可、attribution 必須)
- Tokenizer: SentencePiece (Gemma 2 family)
- 採用根拠: C2 の 3-way empirical 比較で 5/5 を達成した唯一のモデル

### D2. Backend を `LlamaCppBackend` として llama.cpp-family 汎用に定義する

`ZenzBackend` を `LlamaCppBackend` に rename し、`BackendConfig::LlamaCpp { model_path, prompt_template: PromptTemplate }` を公開 API とする。Prompt 構築は `PromptTemplate` enum (`Gemma2InstructChat` / `Qwen2Chat` / `Custom`) で dispatch する。詳細な設計根拠は ADR 0011 (kanji backend trait design) に記録する。

### D3. Prompt 構築を plain-text completion + v12 few-shot に固定する

P1-2.5 follow-up (PR #76) で plain-text completion + v12 few-shot prompt に pivot し、14/15 PASS で accept した。`apply_chat_template` 経路は Phase 1 では採用しない。v12 prompt の 4 要素 ((a) directive、(b) 13-pair positive、(c) 3-pair negative、(d) query suffix) を `build_prompt` の実装真実とする。

### D4. hiragana→katakana preprocessing を backend で行わない

API 境界 (spec §5.6) は hiragana-only 契約のまま維持する。backend の内部処理で kana casing 変換を行う必要はない (Phase 1 default の Gemma-2-2B-jpn-it は hiragana を直接受理する)。将来の Zenz 再評価時には backend-internal に preprocessing を持つことを許容する (spec §5.6 の Note on backend-internal preprocessing 参照)。

### D5. row 3「あした → 翌日」を既知制約として受容する

row 3 の 1 行は Phase 1 acceptance 基準から外す。14/15 を Phase 1 Layer 3 smoke の pass 条件とする。row 3 の根本解決は ADR 0010 (Phase 5 Kotoha custom romaji-base model) に委ねる。row 3 は fixture からは削除せず、test 失敗として発現させたまま保持する (将来の baseline 比較のため)。

### D6. Zenz family を Phase 2+ の再評価候補として保持する

upstream llama.cpp が `gpt2-small-japanese-char` pre-tokenizer を allow-list に追加した時点で、Phase 2+ の tiered-model 比較対象として Zenz family を再評価する。Phase 1 scope では採用しない。

## Consequences

### 正の帰結

- Phase 1 Layer 3 smoke 14/15 を達成し、Phase 1 acceptance 基準 (spec §14) を満たす
- `PromptTemplate` enum と `BackendConfig` enum の `#[non_exhaustive]` 設計により、Phase 2+ で新 chat-family model (Phi-4 / Llama-3 等) を追加する際に breaking change を避けられる
- Gemma License 配下ではあるが、Kotoha repo に model を同梱せず「利用者が HuggingFace から download」する方式のため再配布義務は発生しない
- plain-text completion への pivot により、GGUF に `tokenizer.chat_template` が含まれない model でも `PromptTemplate::Custom` escape hatch 経由で load 可能となった

### 負の帰結

- row 3「あした → 翌日」は本 ADR の decision 下では解消不能である。Phase 5 (ADR 0010) で task-specific fine-tune により根本解決する計画であり、Phase 1 〜 Phase 4 の期間は 14/15 baseline で運用する
- Gemma License は Apache 2.0 に比べて制約が多く、Kotoha の将来 OSS 再配布時に model 再配布には毎回 Gemma License の attribution 手続きが必要になる
- Gemma-2-2B-jpn-it の size は 1.92 GB で IME 常駐用途としては大きく、Phase 3 IBus 統合時のメモリ占有が懸念される。本 ADR の decision では解消しない (Phase 5 の軽量モデル `≤ 200MB` で解決する)
- Cold load 約 10.6s + warm inference 約 3s/case × 14 行 = 約 52s の総時間は spec §8.3 の 30s target を超過する。latency target の緩和判断は ADR 0013 で別個に記録する

## Alternatives considered

### A. upstream llama.cpp の pre-tokenizer allow-list に `gpt2-small-japanese-char` が追加されるまで待機し Zenz 系を使う

**Rejected.** Phase 1 schedule は open-ended な upstream 依存を許容しない。本 ADR の決定時点 (2026-04-25) で upstream にも追加予定は立っていない。Phase 2+ で状況が変化した場合は新 ADR で再評価する。

### B. forked llama-cpp-2 で `gpt2-small-japanese-char` pre-tokenizer を実装する

**Rejected.** llama.cpp bindgen surface の churn 追従コストが、Phase 1 scale の benefit を上回る。メンテナンス工数が Phase 1 の残予算を圧迫する。

### C. Candle (pure-Rust 推論 framework) で Zenz 系を動かす

**Rejected.** Candle の GGUF tokenizer (character-level + byte-level BPE) 対応が 2026-04 時点で未成熟であり、Kotoha の Phase 1 schedule に乗せられない。

### D. 生ひらがな入力を instruction wrapper 無しで Gemma-2-2B-jpn-it に渡す

**Rejected.** P1-2.5-8 の empirical 実測で 1/15 pass rate となった (モデルが入力 echo + emoji を返し、kana→kanji 変換を行わなかった)。v12 few-shot + strict directive は不可避である。

### E. Gemma-4-31B 系への upgrade

**Rejected.** 31B parameter の GGUF は 13-18 GB であり、Phase 1 の size budget (1.92 GB、IME 常駐用途) を 6-9 倍超過する。IME 常駐不可能。Phase 5 distillation の teacher 候補としては別途検討する (ADR 0010 D3 参照)。

## Related documents

- 実装 crate: `crates/kotoha-core/src/kanji/llama_cpp.rs` / `crates/kotoha-core/src/kanji/backend.rs`
- Phase 1 spec: `docs/superpowers/specs/2026-04-24-kotoha-phase-1-design.md` §3.2 / §3.2.1 / §3.2.2 / §3.2.3 / §5.4 / §5.6 / §14
- Plan: `docs/superpowers/plans/2026-04-24-kotoha-phase-1-p1-2-5.md`
- Empirical (model selection, 5-case): `docs/wbs/2026-04-24-feature-69-zenz-backend-layer3-smoke.md` (commit `718fd8e`)
- Empirical (refactor + chat-turn, 9-case): `docs/wbs/2026-04-24-feature-73-llama-cpp-backend-gemma-2-jpn.md` (PR #74 merge `02cf035`)
- Empirical (plain-text completion + v12, 15-case): `docs/wbs/2026-04-24-feature-75-prompt-optimization-15-row-fixture.md` (PR #76 merge `3eccaa1`)
- CLI 実装: PR #82 merge `f9a820c` (P1-3 で kotoha-kanji CLI + phase1-smoke.sh)
- Phase 5 根本解決方針: ADR 0010 (`docs/adr/0010-kotoha-custom-romaji-base-model.md`)
- backend trait 設計: ADR 0011 (`docs/adr/0011-kanji-backend-trait-design.md`)
- feature flag 設計: ADR 0012 (`docs/adr/0012-feature-flag-design-for-llama-cpp.md`)
- latency target 緩和: ADR 0013 (`docs/adr/0013-phase-1-latency-target.md`)
- `#[non_exhaustive]` 方針: ADR 0006 (`docs/adr/0006-non-exhaustive-on-streaming-enums.md`)
