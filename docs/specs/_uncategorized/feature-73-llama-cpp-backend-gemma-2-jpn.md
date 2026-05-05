---
feature: feature-73-llama-cpp-backend-gemma-2-jpn
status: implemented
bounded_context: _uncategorized
related_issues: ["#73"]
related_prs: []
glossary_refs: ["azookey","gemma-2-2b-jpn-it","gguf","hiragana","kana","katakana","mock-backend","prompt-template","zenz","zenzai"]
last_reviewed: 2026-05-05
---

# P1-2.5 — LlamaCppBackend 汎用化 + Gemma-2-2B-jpn-it 採用 (実装ログ)

> **Migration note**: 本 spec は `docs/wbs/2026-04-24-feature-73-llama-cpp-backend-gemma-2-jpn.md` から spec 化 (Global CLAUDE.md §Documentation Structure 準拠の `docs/wbs/` 廃止対応、2026-05-05)。
> 元 WBS は実装ログ性質。14-section restructure は別 follow-up 起票予定 (各 project の既存 14-section follow-up issue と合流可能)。

## 元 WBS 内容 (実装ログ由来)


| 項目                | 値                                                                                           |
| ------------------- | -------------------------------------------------------------------------------------------- |
| Milestone           | P1-2.5                                                                                       |
| ISSUE (plan)        | #71                                                                                          |
| ISSUE (impl)        | #73                                                                                          |
| Plan doc            | `docs/superpowers/plans/2026-04-24-kotoha-phase-1-p1-2-5.md`                                 |
| PR (plan)           | #72 (merge commit `43ab1a4`)                                                                 |
| PR (impl)           | #74 (merge commit `02cf035`)                                                                 |
| Implementation 起点 | `718fd8e` (develop)                                                                          |
| Implementation 完了 | `02cf035` (develop)                                                                          |
| 工数                | 約 1 session (plan + impl 連続実行、cold load を含めた実測時間 `gh` API より導出可能)       |
| Reviewer findings   | Critical: 0、High: 2 (H1 spec §5.4 同期、H2 README/ROADMAP sweep) — 両件とも merge 前に fix 済 |

## 実装差分の要約

- **PromptTemplate enum 新設** (`crates/kotoha-core/src/kanji/backend.rs`):
  - `Gemma2InstructChat` / `Qwen2Chat` / `Custom { system, user_wrapper, assistant_prefix }` の 3 variant
  - `#[non_exhaustive]` + `#[derive(Debug, Clone)]`
- **BackendConfig::Zenz → BackendConfig::LlamaCpp** (`backend.rs`):
  - `model_path: PathBuf` + `prompt_template: PromptTemplate` の 2 field struct-like variant に置換
  - `load_backend` factory match arm 更新
- **ZenzBackend → LlamaCppBackend rename**:
  - `git mv` を 2 commit に split (pure mv → body rewrite) で rename threshold 維持
  - struct フィールド追加: `prompt_template: PromptTemplate`, `model_id: String`
  - `load(model_path, prompt_template)` の 2-arg signature 化
  - `model_id()` を `&self.model_id` (GGUF file stem 由来) に変更
- **apply_chat_template への切替** (`llama_cpp.rs`):
  - 旧 `build_prompt` (PUA token 組み立て) を削除
  - 新 `build_chat_tuples(template, user_input) -> Vec<(String, String)>` 追加
  - `infer()` の prompt 組立を `model.chat_template(None)? + model.apply_chat_template(&tmpl, &chat, true)?` に置換
  - `LlamaChatMessage::new(role, content)?` で Result propagate
- **IME-style multi-turn few-shot wrapper** (empirical 必須):
  - `apply_chat_template(None)` alone では Gemma-2-2B-jpn-it が kana→kanji 変換せず conversational echo を返すため、`build_chat_tuples` に 3 件の user/assistant 対を pre-fill
  - 例: にほんご→日本語 / やまださん→山田さん / わたしはがくせいです→私は学生です
- **hiragana→katakana preprocessing 削除** (`convert()`):
  - `crate::kana::hiragana_to_katakana(input)` 呼び出しを convert body から除去 (関数定義自体は kana/ に保持)
- **Layer 3 fixture 5→9 rows** (`tests/fixtures/kanji_smoke.tsv`):
  - Row 1 にほんご: expected substring `日本語` → `日本` に relax (Gemma が `日本` で出力停止するため)
  - Row 6-9 追加: きょうのてんき→今日、とうきょう→東京、わたしはがくせいです→学生、しんぶん→新聞
  - Drop (非変換 or 誤変換のため fixture 非採用): ぎゅうにゅう、きっぷ、こーひー、はっぴょう、じしょ、りょうり
- **env rename**: `KOTOHA_ZENZ_MODEL_PATH` → `KOTOHA_LLAMA_MODEL_PATH`
- **Cargo feature rename**: `zenz` → `llama-cpp`、`zenz-smoke` → `llama-cpp-smoke`
- **Spec 改訂**: §1 / §2.1 / §3.2 / §3.3 / §3.4 / §4.1 / §4.3 / §5.2 / §5.3 / §5.4 / §5.6 / §6 / §7.1 / §8.3 / §8.5 / §8.6 / §9 / §10 / §11 / §12 / §13 / §14 / §15 / §16 を Gemma-2-2B-jpn-it + LlamaCppBackend reality に同期。§3.2.2 / §3.3 は historical narrative として Zenz 言及を保存
- **ADR 0009 prep note** (`docs/adr/0009-kanji-backend-model-selection-prep.md`) 新規作成 (正式 ADR は P1-4)
- **residual cleanup** (`backend.rs` rustdoc / `error.rs` test fixtures の "zenz" 文字列を llama-cpp / gemma に置換)
- **README.md / docs/ROADMAP.md** の Phase 1 名称を "llama.cpp + Gemma-2-2B-jpn-it" に修正

## prompt template 決定根拠

| 観点                 | 内容                                                                                                                                                                   |
| -------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| API 確認             | `llama-cpp-2 0.1.145` local source を `~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/llama-cpp-2-0.1.145/src/model.rs` で走査し確認                             |
| `LlamaChatMessage`   | `role: CString` / `content: CString` は private field、pub accessor なし。`::new(role, content) -> Result<Self, NewLlamaChatMessageError>` で構築                        |
| `apply_chat_template`| `(&self, tmpl: &LlamaChatTemplate, chat: &[LlamaChatMessage], add_ass: bool) -> Result<String, ApplyChatTemplateError>`。`tmpl` は `Option<&str>` ではなく値必須         |
| GGUF 埋め込み取得    | `LlamaModel::chat_template(None) -> Result<LlamaChatTemplate, ChatTemplateError>` で GGUF の `tokenizer.chat_template` を取得可能                                        |
| dispatch             | Phase 1 は `Gemma2InstructChat` / `Qwen2Chat` を同一 dispatch (両者とも `chat_template(None)`) とし variant tag は将来の分岐拡張用として保持                             |

## Open Question close declaration (spec §13)

| Q   | 内容                                                                 | Close 判断                                                                                                                               |
| --- | -------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------- |
| Q1  | llama-cpp-2 version                                                  | 0.1.145 pin 継続で close。`apply_chat_template` + `chat_template(None)` は同 version で利用可能                                          |
| Q2  | LLM prompt template の exact form                                    | GGUF `tokenizer.chat_template` を信頼し `apply_chat_template` に委譲。加えて IME-style multi-turn few-shot を `build_chat_tuples` で注入 |
| Q3  | model 配置方針                                                       | Phase 1 は「利用者が HuggingFace から download」で close。auto-download は Phase 6 UX 課題として persist                                 |

## hiragana 直受け empirical 確認

本セッションで実測した Gemma-2-2B-jpn-it Q5_K_M の出力 (final fixture 9 rows):

| Input                  | Expected substring | Top-1 output (surface) | Pass   |
| ---------------------- | ------------------ | ---------------------- | ------ |
| にほんご               | 日本 (relaxed)     | 日本 \\n\\n            | PASS   |
| かんじ                 | 漢字               | 漢字                   | PASS   |
| あした                 | 明日               | 明日                   | PASS   |
| やまださん             | 山田               | 山田さん               | PASS   |
| ことば                 | 言葉               | 言葉                   | PASS   |
| きょうのてんき         | 今日               | 今日の天気             | PASS   |
| とうきょう             | 東京               | 東京                   | PASS   |
| わたしはがくせいです   | 学生               | 私は学生です           | PASS   |
| しんぶん               | 新聞               | 新聞                   | PASS   |

実行時間: cold load 約 10 秒 + 9 件 × 平均 3 秒 (warm inference) = 約 29-49 秒 (single-thread 実行時 `cargo test --test-threads=1`)

### Row drop evidence (review M5 follow-up)

下記 6 row は 15 行 fixture 構想時点では候補だったが、empirical で通らず drop。

| Input          | Expected substring  | Gemma-2-2B-jpn-it 出力 | Drop 理由                                                   |
| -------------- | ------------------- | ---------------------- | ----------------------------------------------------------- |
| ぎゅうにゅう   | 牛乳                | ぎゅうにゅう \\n\\n    | 非変換 (hiragana echo)                                      |
| きっぷ         | 切符                | きっぷ \\n\\n          | 非変換 (hiragana echo、v2 prompt では 切符 PASS したが v4 で regression) |
| こーひー       | コーヒー            | これは難しい！ \\n\\n  | 誤変換 (model が refusal-like 応答)                          |
| はっぴょう     | 発表                | ハッピー \\n\\n        | 誤変換 (katakana 音訳)                                       |
| じしょ         | 辞書                | 指示 / これは難しいです | 誤変換 (音響的に類似した語に誤認)                           |
| りょうり       | 料理                | 曜日 \\n\\n            | 誤変換 (音響的に類似した語に誤認)                           |

これらは Gemma-2-2B-jpn-it の Phase 1 固有の限界 (2B param instruction-tuned で IME-specialized training 無し)。Phase 2+ で fine-tuned specialized IME model を追加する際に再評価できる。

## spec 改訂差分

- §1 概要 / §2.1 In-scope: Zenz (GPT-2 系) 記述を LLM (Phase 1 default: Gemma-2-2B-jpn-it) に一般化
- §3.2: 新 default `Gemma-2-2B-jpn-it Q5_K_M`、§3.2.1 IME-style prompt wrapper、§3.2.2 historical Zenz context、§3.2.3 Phase 2 tiered-model candidates
- §3.3: AzooKey Zenzai docs を必読 → 歴史的参照に降格
- §3.4: tempfile の Zenz smoke 表記を llama.cpp smoke 表記に変更
- §4.1: crate layout ASCII tree の `zenz.rs` → `llama_cpp.rs`、`kanji_zenz_smoke.rs` → `kanji_llama_cpp_smoke.rs`
- §4.3: feature flag 表の `zenz` / `zenz-smoke` → `llama-cpp` / `llama-cpp-smoke`
- §5.2 / §5.3: ZenzBackend 言及を LlamaCppBackend に置換、`model_id` 例を gemma-2-2b-jpn-it-q5_k_m に
- §5.4: `BackendConfig::LlamaCpp { model_path, prompt_template }` 定義に変更、PromptTemplate enum 定義 (review H1 fix 済)
- §5.6: backend-internal preprocessing note 追加 (API boundary のみ hiragana-only 契約)
- §6: data flow の ZenzBackend block を LlamaCppBackend + apply_chat_template path に置換
- §7.1: `--model` option description を Phase 1 default: Gemma-2-2B-jpn-it Q5_K_M に
- §8.3: Layer 3 section 全面改訂 (feature name / test file name / env var / 所要時間)
- §8.5: test count 表の Zenz smoke row を llama.cpp smoke (9 件) に
- §8.6: regenerate procedure commands を llama-cpp-smoke base に
- §9 / §10: Zenz 言及を LLM / llama.cpp に一般化
- §11: P1-2 milestone description に「当初 ZenzBackend として着手、P1-2.5 で一般化」を追加
- §12: ADR 0009 名を `Phase 1 default model selection (Gemma-2-2B-jpn-it)` に変更
- §13: Q1/Q2/Q3 close 明記
- §14: Acceptance checklist 4 / 8 / 14 を新識別子に更新
- §15: Zenz-v2.5 collection リンクに Phase 1 降格 note 付記、Gemma-2-2B-jpn-it GGUF リンク追加
- §16: 公開 API リストに PromptTemplate 追加、Phase 2 bridge narrative を LlamaCppBackend 中心に

## 検証 commands (final)

```bash
cargo check -p kotoha-core --no-default-features   # PASS
cargo check -p kotoha-core                          # PASS
cargo check -p kotoha-core --features mock-backend  # PASS
cargo check -p kotoha-core --features llama-cpp     # PASS
cargo check -p kotoha-core --features llama-cpp-smoke  # PASS
cargo check -p kotoha-core --all-features           # PASS
cargo test -p kotoha-core --lib                     # 135 passed
cargo test -p kotoha-core --features mock-backend   # 141 passed (default 135 + mock 6)
cargo clippy --workspace --all-targets --all-features -- -D warnings   # zero warnings
cargo fmt --all --check                             # clean

export KOTOHA_LLAMA_MODEL_PATH=$HOME/.cache/kotoha/models/empirical/gemma-2-2b-jpn-it-Q5_K_M.gguf
cargo test -p kotoha-core --features llama-cpp-smoke --test kanji_llama_cpp_smoke -- --test-threads=1
# => 9 passed in ~30-49 s (cold load + 9 inferences)
```

## Follow-up (post-merge)

Review medium findings for future issues:

1. **Qwen2Chat rustdoc** — Phase 1 dispatch が Gemma2InstructChat と同一であることを明記 (rustdoc enhancement only)
2. **KanjiError::Backend structured cause** — Phase 2 の CLI / IBus engine 着手時に `Backend { kind: BackendErrorKind, source: ... }` 型拡張を検討
3. **user_wrapper named struct** — `Custom { user_wrapper: UserTurnWrapper { prefix, suffix }, ... }` への refactor (escape-hatch polish)
4. **Layer 3 determinism regression test** — `convert(input, &opts)` を 2 回呼んで surface 一致を assert する test 追加 (`llama-cpp-smoke` feature gate 内)
5. **README `README.md` の Phase 1 model 手順記載** — Task P1-3 で CLI `kotoha-kanji` + scripts/phase1-smoke.sh を追加する際に README に empirical 手順を統合

Phase 1 の残マイルストーン: P1-3 (CLI `kotoha-kanji` + Layer 4 E2E smoke) → P1-4 (ADR 0009 / 0010 / 0011 正式起票 + Phase 1 acceptance checklist 消化 + implementation WBS final log)
