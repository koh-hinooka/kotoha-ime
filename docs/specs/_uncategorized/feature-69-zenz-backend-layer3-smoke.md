---
feature: feature-69-zenz-backend-layer3-smoke
status: deprecated
deprecated_reason: "Phase E migration で旧 docs/wbs/ から spec 化した実装ログ性質の文書。Global CLAUDE.md §Development Flow legacy spec 取扱いルール (実装ログ性質 → status: deprecated、本文 14-section restructure 不要) に基づき deprecated 扱い。git history は参照点として保持 (2026-05-06)。"
bounded_context: _uncategorized
related_issues: ["#69"]
related_prs: []
glossary_refs: ["azookey","backend-trait","candidate","chat-template","distillation","gemma-2-2b-jpn-it","gguf","greedy-decoding","hiragana","kana","katakana","layer-3-smoke","lefthook","mock-backend","pua-tokens","uv","zenz","zenzai"]
last_reviewed: 2026-05-06
---

# P1-2: ZenzBackend via llama-cpp-2 + Layer 3 smoke

> **Migration note**: 本 spec は `docs/wbs/2026-04-24-feature-69-zenz-backend-layer3-smoke.md` から spec 化 (Global CLAUDE.md §Documentation Structure 準拠の `docs/wbs/` 廃止対応、2026-05-05)。
> 元 WBS は実装ログ性質。14-section restructure は別 follow-up 起票予定 (各 project の既存 14-section follow-up issue と合流可能)。

## 元 WBS 内容 (実装ログ由来)

milestone: P1-2
branch: feature/69-zenz-backend-layer3-smoke
pr: "#70"
merge_commit: "49ed56e"
issue: "#69"
status: done
started: 2026-04-24
finished: 2026-04-24
---

# P1-2: ZenzBackend via llama-cpp-2 + Layer 3 smoke

## 実施内容

P1-1 で skeleton 配線した `ZenzBackend` を llama-cpp-2 経由で本実装し、Layer 3 smoke test harness を追加した。squashed commit `49ed56e` には以下 7 commit が含まれる:

- `b6c96e7` — `llama-cpp-2 = "0.1"` を workspace root `[workspace.dependencies]` に追加し、`crates/kotoha-core/Cargo.toml` の `[dependencies]` に `llama-cpp-2 = { workspace = true, optional = true }` を追加。同時に feature flag `zenz` を `[]` から `["dep:llama-cpp-2"]` に書き換えた。
- `86f4723` — `crates/kotoha-core/src/kanji/zenz.rs` の `ZenzBackend` struct を P1-1 の `_placeholder: ()` から `{ model: LlamaModel, model_path: PathBuf }` に rewrite。`LlamaModel` が Debug を派生していないため `impl fmt::Debug for ZenzBackend` を manual に実装した。
- `f905e28` — `ZenzBackend::load` を本実装に置換。`OnceLock<LlamaBackend>` で `LlamaBackend::init()` を process-wide singleton 化し、`LlamaModel::load_from_file` で gguf を読む。P1-1 skeleton の `Err(KanjiError::Backend {..})` 返却は削除した。
- `b8dce07` — `model_id()` を `"zenz-v2.5-medium"` 固定返却に実装(spec §5.3)。
- `41c5e50` — `convert()` を本実装に置換: `validate_input` → `hiragana_to_katakana` → `build_prompt` (PUA separator U+EE00..U+EE02 配置) → llama-cpp-2 で greedy sampling → `detokenize_bytes` (UTF-8 lossy) → `score_sort_dedupe` のパイプラインで `Vec<Candidate>` を返す。
- `ff413d4` — `crates/kotoha-core/tests/kanji_zenz_smoke.rs` (Layer 3 test skeleton) + `crates/kotoha-core/tests/fixtures/kanji_smoke.tsv` (5 row fixture: にほんご / かんじ / あした / やまだ / ことば → 日本語 / 漢字 / 明日 / 山田 / 言葉) を追加。`KOTOHA_ZENZ_MODEL_PATH` が未設定なら `SKIPPED` を println! して exit 0 する `cfg(feature = "zenz-smoke")` gated test。
- `4c9fb0f` — `Cargo.lock` refresh (llama-cpp-2 transitive deps を pin)。

## Prompt format 解析ログ (Q2 解決)

1 次情報 (AzooKey/Miwa-Keita 公式 docs) から zenz-v3 系の prompt format を確定した。詳細は本 PR 作業ブランチ上の scratch file `docs/wbs/p1-2-prompt-format-analysis.md` (統合後削除) を出典とする。

- 出典: AzooKey zenzai.md (<https://github.com/azooKey/AzooKeyKanaKanjiConverter/blob/main/Docs/zenzai.md>) + Miwa-Keita `zenz-v2.5-dataset` README (<https://huggingface.co/datasets/Miwa-Keita/zenz-v2.5-dataset>)。
- 特殊 token 配置: Private Use Area (U+EE00〜U+EE06)。BOS 明示 token は docs 未記載 → P1-2-5 実装時に GGUF tokenizer metadata で empirical 確認する方針に倒した(P1-2-9 empirical verification で実施予定)。
- EOS token: `</s>` (llama 系標準、zenz-v1/v2/v3 共通)。
- zenz-v3 prompt の一般形: `<context><input_katakana><output></s>` (docs 明記、推奨形式)。
- 世代別差分: v1 は `<input_katakana><output></s>` (context なし)、v2 は `<input_katakana><context><output></s>` (context 後置)、v3 は `<context><input_katakana><output></s>` (context 前置、推奨)。
- 入力: **カタカナ** であり、Kotoha が保持する **ひらがな** buffer は `ZenzBackend::convert` 冒頭で `crate::kana::hiragana_to_katakana` を通す契約とした。
- Kotoha 実装は Phase 1 smoke では `inferenceLimit = 1` 相当の single-pass greedy decoding を採用する。AzooKey docs 側の `inferenceLimit` は reranking ループの反復回数であり、max new tokens とは別概念である点を確認した。
- Tokenizer 内部方式 / sampling parameters (temperature/top-K/top-P) / score aggregation は AzooKey docs 未記載。Kotoha 側で empirical 確定とし、temperature == 0.0 + seed == Some(0) を greedy sampling に mapping する方針を採用した。

## Zenz 入力 format 検証 (user-directed 1b research)

ユーザ指摘により、AzooKey docs は「Swift IME 側の Zenz 消費者の記述」であり「Zenz 著者による 1 次情報」ではない可能性を再確認した。Miwa-Keita が HuggingFace 上で公開している dataset/model card を独立確認した結果、1 次情報で katakana 入力が明示されていることを確定した。

- 決定的 verbatim quote (`Miwa-Keita/zenz-v2.5-dataset` README、訓練データの field 定義):

  ```text
  "input": str, 入力のカタカナ文字列（記号、数字、空白などが含まれることがあります）
  ```

- 著者 Miwa-Keita 本人が `input` field を **「入力のカタカナ文字列」** と宣言しており、訓練時の contract として確定。推論時に hiragana を直接与えると out-of-distribution 入力となり変換品質が劣化する。
- hiragana 版 model の存否: zenz-v1〜v3.1 の全 variants において hiragana 入力を想定した training が行われた形跡は無い。`<input_hiragana>` special token への言及も著者文書中に皆無。
- 結論 (A): **公式文書が katakana を明示している** → `hiragana_to_katakana` 前処理は必須。AzooKey docs 側の `<input_katakana>` 記述は Swift IME 側の独自 convention ではなく、Zenz 本体の訓練時契約をそのまま反映していることが確定。
- 実装方針への影響: P1-2 の元の実装計画を変更する必要は無かったが、hiragana → katakana 変換の根拠が「AzooKey convention」から「Zenz 著者本人の訓練 contract」へと格上げされ、より強い authoritative backing を得た。
- spec §5.6 / §6 に footnote 追加は P1-4 docs bundle に繰り延べた。

## llama-cpp-2 version pin rationale (Q1 解決)

- 採用 version: `"0.1"` (caret) を workspace root で pin。検証時点の最新 stable は `0.1.145` (crates.io `max_stable_version` / 2026-04-22 リリース)。
- 確認手段: Context7 MCP + crates.io JSON API (`/api/v1/crates/llama-cpp-2`)。crates.io HTML は JS レンダリングで WebFetch 不可のため JSON API を権威ソースとした。
- 採用理由:
  - 最新 stable かつ upstream llama.cpp への追随が活発(直近 1 ヶ月で 0.1.141 → 0.1.145 の 5 リリース、2026-03-29〜2026-04-22)。
  - 総 DL 417,703 / 直近 90 日 DL 180,274 と Rust 圏 llama.cpp バインディングのデファクト指標を満たす。
  - repository: `github.com/utilityai/llama-cpp-rs` (Utility AI 公式 wrapper)、Context7 Source Reputation: High / Benchmark Score 60.65 / 42 snippets。
  - メンテナが「strict semver よりも upstream llama.cpp 追随を優先」と明言、0.1.x 系継続運用。
- License: `MIT OR Apache-2.0` (dual license)。Kotoha (OSS 予定) との互換性 OK。
- MSRV: llama-cpp-2 は `rust_version` 未宣言。Kotoha MSRV 1.80 と互換性は `cargo build --workspace` (Layer 3 smoke 用 `cargo check` 含む) で empirical に確認した(全 feature 組合せ PASS)。
- 代替候補の不採用理由再確認:
  - `llama_cpp` (edgenai/llama_cpp-rs): 最新 0.3.2 / 2024-04-29、2 年近く更新なしで停滞。spec §3.1 の「非活発」判定継続。
  - `mistral.rs` (mistralrs): 最新 0.8.1 / 2026-04-02、活発だが MSRV 1.88 宣言で Kotoha の 1.80 と **非互換**。明示拒否理由が追加された。
  - `Candle` (candle-core): 最新 0.10.2 / 2026-04-01、pure-Rust tensor framework。AzooKey 公式 docs が llama.cpp prompt format を前提としているため、Candle を採用すると zenz-v3 prompt format 実装を自前再実装する必要が生じ P1-2 smoke コストが増大する。spec §3.1 の判断継続。
- semver 注意: メンテナ方針により 0.1.x patch bump でも API 微小 breaking 混入の可能性あり。P1-2 では `"0.1.145"` 時点を基準とし、以降の bump は PR レビューで差分確認する運用とする。
- Bundled llama.cpp: `llama-cpp-sys-2` が upstream C++ を static link で bundling。ビルド時に C++ toolchain (clang / cmake) 要求。開発環境では既に導入済のため Phase 1 では ADR 化せず。

## Deterministic output (Q5 解決)

- `ConvertOptions { temperature: 0.0, seed: Some(0) }` → llama-cpp-2 の `LlamaSampler::greedy()` にマップし、argmax 選択で completely deterministic を達成する方針とした。
- `temperature > 0.0` は Phase 1 では `KanjiError::Backend { reason: "temperature>0 not supported in Phase 1 (greedy only)" }` で明示的に拒否する。Phase 2+ で proper sampling + pinned-seed RNG を追加する。
- `seed` parameter の greedy 下での挙動: 内部的には no-op となる(argmax は乱数を使わない)。この挙動は Phase 2 の sampling 導入時に seed 値を活用する前提で、Phase 1 では「将来互換のためのフィールド予約」として温存する(Low finding として記録)。

## Cold start / warm cache latency 測定 (deferred)

Zenz-v2.5-medium GGUF model が実装セッション内で未入手のため、empirical な latency 測定は deferred とした。spec §8.3 の target は「first load on CPU で 10〜30 秒」。Phase 1 follow-up で `KOTOHA_ZENZ_MODEL_PATH` を設定した上で以下を測定し本 section に追記する:

1. cold start: `ZenzBackend::load` 初回呼び出し (LlamaBackend::init + LlamaModel::load_from_file の総時間)
2. warm cache: 同一 process 内 2 回目の `load` (OS page cache heated 後)
3. ファイルサイズ: zenz-v2.5-medium gguf の実測 (150MB 程度の想定 vs 実測)

## つまずき

1. **`OnceLock<LlamaBackend>` singleton の採用 (plan deviation #1)**: llama-cpp-2 の `LlamaBackend::init()` は process-wide で 1 回のみ呼ばねばならず(upstream llama.cpp の制約)、Plan pseudocode では singleton 方式を明示していなかった。`once_cell` は既に std `OnceLock` (Rust 1.70+) でカバーされるため追加依存なしで実装できた。
2. **`impl fmt::Debug for ZenzBackend` の手動実装 (plan deviation #2)**: llama-cpp-2 0.1.145 の `LlamaModel` は Debug を派生しないため、`ZenzBackend` に `#[derive(Debug)]` を適用できない。`model_path: PathBuf` のみを表示する Debug impl を手動で書いた。
3. **Layer 3 test の `ConvertOptions` 構築 (plan deviation #3)**: P1-1 と同じく `#[non_exhaustive]` cross-crate 制約 (E0639) により struct-literal 構築不可。`let mut opts = ConvertOptions::default(); opts.top_k = 5; …` + `#[allow(clippy::field_reassign_with_default)]` の idiom を採用。
4. **Detokenization は `token_to_piece_bytes` + `String::from_utf8_lossy` (plan deviation #4)**: UTF-8 境界の中途半端な token が sampling 途中で現れる可能性への保険。`encoding_rs` を直接依存に追加せず、llama-cpp-2 経由の transitive 依存のみで済ませた。`from_utf8_lossy` による silent substitution は Low finding として記録されたが、本実装では許容範囲として採用。
5. **ユーザ指摘 1b research (AzooKey = Zenz 著者ではない点の再確認)**: 作業中に user 指摘により「AzooKey docs は 2 次情報」との可能性を検証。Miwa-Keita の dataset card を 1 次情報として確認し、katakana 入力 contract を authoritative に裏取りした。実装方針そのものは変更せず維持したが、根拠が格段に強化された。

## Regression 検証

- `cargo test --workspace` (default features): 160 PASS (P1-1 baseline と同値、regression なし)。
- `cargo test --workspace --features mock-backend`: 171 PASS (+11 = Layer 2 integration 5 + MockBackend unit 6、P1-1 と同値)。
- `cargo test --workspace --features zenz`: 160 PASS (default と同値。`zenz` unit test は `#[cfg(test)]` inside `zenz.rs` にあり feature gate の組合せで `lib` target に登録される挙動の実装詳細)。
- `cargo test --workspace --features zenz-smoke`: 165 PASS (+5 Layer 3、`KOTOHA_ZENZ_MODEL_PATH` 未設定のため全 5 件が SKIPPED println! + PASS + exit 0)。
- `cargo test --workspace --all-features`: 176 PASS。
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`: warnings ゼロ。
- `cargo fmt --all --check`: diff ゼロ。
- lefthook pre-push (manifest-check / build / clippy / test): PASS。

## Review 結果

- `secrets-check`: CLEAN (AWS / GH / GL tokens / private keys / DB conn strings / .env 参照すべてゼロ)。
- `agent-teams:team-reviewer` (architecture + testing + security combined): **MERGE_NOW** 判定。Critical 0 / High 0 / Medium 2 / Low 4。
  - Medium findings (P1-4 / Phase 2 繰延):
    - `OnceLock<LlamaBackend>` 初期化失敗時の race condition (Phase 2 concurrency 対応時に再検討、Phase 1 の single-thread 前提では影響なし)
    - `max_new_tokens` 選定根拠の comment drift (記述微小な不整合、trivial)
  - Low findings (P1-4 繰延):
    - `seed` が greedy 下で no-op になる旨の doc comment 不足
    - `score` 値が現状 `0.0` placeholder の旨の doc comment 不足
    - `from_utf8_lossy` 適用時の silent substitution に `tracing::warn!` を入れる案
    - Layer 3 test helper の `.expect(...)` メッセージの改善

## P1-2-9 empirical verification 実施結果 (2026-04-24、Option A pivot — zenz-v3.1-small-gguf)

本 section は Option A (gated spec default `zenz-v2.5-medium-gguf` を諦め、公開 repo の `Miwa-Keita/zenz-v3.1-small-gguf` で empirical 検証) を実施した結果である。**結論: v3.1-small は Kotoha の現行実装にとって drop-in 代替として使用不可**。詳細は以下。

### Download / 検体同定

| 項目 | 値 |
|------|-----|
| Repo | `Miwa-Keita/zenz-v3.1-small-gguf` (公開、auth 不要) |
| File | `ggml-model-Q5_K_M.gguf` |
| 入手経路 | `curl -L https://huggingface.co/Miwa-Keita/zenz-v3.1-small-gguf/resolve/main/ggml-model-Q5_K_M.gguf` |
| size | 74 MiB (file) / 70.26 MiB (model weights、GGUF ヘッダ込み) |
| GGUF magic | `G G U F` 4 bytes 先頭で確認 OK |
| SHA-256 | `4de930c06bef8c263aa1aa40684af206db4ce1b96375b3b8ed0ea508e0b14f6c` |
| local path | `$HOME/.cache/kotoha/models/zenz-v3.1-small-gguf/ggml-model-Q5_K_M.gguf` |

### なぜ v3.1-small を pivot 先に選んだか

spec §3.2 default の `Miwa-Keita/zenz-v2.5-medium-gguf` は HuggingFace API が HTTP 401 を返す gated repository であり、本セッション内での実施不能。公開 Zenz 系 GGUF の中で最も新しく (2025-08-30 更新)、直近 30 日 DL 5194 と community 採用が最も厚い `zenz-v3.1-small-gguf` を選んだ。v3 系の prompt format `<context><input_katakana>{input}<output></s>` は Kotoha の `build_prompt` 実装と一致する想定であった (しかし後述の通り、v3.1-small では成立しないことが判明した)。

### Layer 3 smoke test outcome (全 5 件 FAIL)

```bash
export KOTOHA_ZENZ_MODEL_PATH="$HOME/.cache/kotoha/models/zenz-v3.1-small-gguf/ggml-model-Q5_K_M.gguf"
cargo test -p kotoha-core --features zenz-smoke --test kanji_zenz_smoke -- --nocapture --test-threads=1
```

結果: **0 passed / 5 failed** (all tests panic at `ZenzBackend::load`)。

全 5 件ともに同一の panic:

```
llama_model_load: error loading model: error loading model vocabulary:
  unknown pre-tokenizer type: 'gpt2-small-japanese-char'
llama_model_load_from_file_impl: failed to load model
panicked at crates/kotoha-core/tests/kanji_zenz_smoke.rs:92:
  Zenz backend must load from the pinned fixture path:
  ModelLoadFailed { source: NullResult }
```

5 件の内訳は機械的に同じ理由なので、top-1 substring 検証は一切行えなかった (load 段で全件中断)。

### モデル構造の解析 (なぜ load に失敗したか)

GGUF metadata を dump すると、本 model は `zenz-v2` 系とは **アーキテクチャが異なる** ことが判明した:

| GGUF KV field | 値 | 備考 |
|---------------|------|------|
| `general.architecture` | `gpt2` | **llama 系ではない**。llama.cpp 本体の GPT-2 サポート (static subset) が必要 |
| `general.name` | `Gpt2 Small Japanese Char` | Character-level 日本語 GPT-2 (京大 NLP) をベースに fine-tune |
| `general.organization` | `Ku Nlp` | 京都大学 NLP (`ku-nlp/gpt2-small-japanese-char`) 派生 |
| `general.finetune` | `japanese-char` | |
| `general.size_label` | `small` | |
| `general.version` | `v3.1` | |
| `gpt2.block_count` | 12 | |
| `gpt2.context_length` | 1024 | |
| `gpt2.embedding_length` | 768 | |
| `tokenizer.ggml.model` | `gpt2` | GPT-2 byte-level BPE |
| `tokenizer.ggml.pre` | `gpt2-small-japanese-char` | **これが llama.cpp 0.1.145 同梱版に未登録のため load 不能** |
| tokens (total) | 6000 | zenz-v2.5 系の vocab 規模と桁違いに小さい |

直接の load 失敗原因は `tokenizer.ggml.pre = "gpt2-small-japanese-char"` の pre-tokenizer identifier が llama-cpp-2 0.1.145 が bundling する llama.cpp 内の pre-tokenizer allow-list に登録されていないこと。これは upstream llama.cpp 側で明示的 allow-list 方式を採用しており、未登録 pre-tokenizer は load 時に reject される仕様である (security 前提)。回避には llama.cpp upstream patch の PR 待ち or 自前 patch が必要となり、Phase 1 scope 外である。

### PUA token verification (致命的な不整合)

より重要な論点として、**v3.1-small は Kotoha `build_prompt` が前提とする PUA token を一切持たない**。

```python
# gguf python reader で全 6000 token を走査した結果
PUA (U+E000..U+F8FF) token 数: 0
"katakana" / "context" / "input" / "output" 含有 token 数: 0
先頭 30 token: [UNK], [PAD], <s>, </s>, !, ", #, ..., : (ASCII 記号)
vocab: byte-level BPE (UTF-8 byte fragments of hiragana/kanji)
```

一方 Kotoha `build_prompt` は spec §5.6 に準拠して:

- U+EE00 = `<context>` (placeholder)
- U+EE01 = `<input_katakana>` (placeholder)
- U+EE02 = `<output>` (placeholder)

を prompt 中に挿入する。v3.1-small はこれらの special token を vocab に持たないため、仮に load に成功していたとしても prompt format が model の訓練時契約と合致せず、出力は完全に garbage になる。

これは v3.1-small が zenz-v2 系とは **別モデル系統** であることを示している。v3 系 README (AzooKey docs) の `<context><input_katakana><output></s>` format はおそらく同名の別 variant (`zenz-v3-xsmall`、`zenz-v3-small` など) ないし author の命名方針変更を意味しており、`v3.1-small` は別物の可能性が高い。

### Latency 測定

model load 自体が 0.03 秒以内に失敗するため、**cold start / warm cache latency は測定不能** (load が成立しないため意味をなさない)。

cargo test 全体の壁時計は 0.37 秒 (build はすでに完了していた prior cache を再利用)。

### Fixture TSV 更新の要否

**不要** (現 fixture で対応できる model が無いため、fixture 行の実測値への修正は意味がない)。v2.5-medium (gated) が入手できた時点で改めて empirical 実施する。

### ADR 0009 (P1-4) への入力 — 決定的な含意

本 empirical 結果は ADR 0009「Phase 1 default Zenz model の選定」の判断材料を大幅に塗り替えた:

1. **選択肢 2 (`zenz-v3.1-small-gguf` に差し替え) は却下**:
   - llama.cpp 0.1.145 が `gpt2-small-japanese-char` pre-tokenizer を認識しないため、load 不能。
   - 仮に patched llama.cpp で load 通しても、PUA special token を持たないため `build_prompt` format が一切効かない。
   - architecture が `gpt2` (not `llama`) で、推論 pass 自体が別経路を必要とする可能性。
2. **選択肢 1 (spec default `zenz-v2.5-medium-gguf` を維持) が優位**:
   - gated ではあるが HF 上に存在する(HTTP 401 は未 auth / access 未承認の挙動であり、model 自体は生きている)。
   - v2.5 系は v3.1 と別系統 (v2.5 = llama 系、v3.1 = gpt2 系) のため、Kotoha の現行 prompt format 前提は v2.5 系固有である可能性が高い。
3. **選択肢 3 (CLI `--model` 必須 + README に両論併記) が次善**:
   - v2.5-medium gated ユーザには v2.5 系、public なユーザには現時点では該当なし。
   - 将来 `zenz-v2-gguf` (`Miwa-Keita/zenz-v2-gguf`, 公開) を第二候補として empirical 検証する手がある。
4. **新規論点**: Kotoha の `build_prompt` 実装は「v2.5-medium 訓練時の special token layout」に強く依存している可能性が高い。ADR 0009 では「どの Zenz variant を sustain 対象とするか」を単なる default 問題ではなく「prompt format ABI を何にロックするか」として議論すべき。

### 申し送り

- P1-2-9 empirical 本番 (v2.5-medium) は spec default を変更しない限り user の HF auth + gated access request が前提となる。
- もし public な代替を探す場合、次の empirical 候補は `Miwa-Keita/zenz-v2-gguf` (`zenz-v2-Q5_K_M.gguf`)。v2 系は v3 系と tokenizer architecture が異なる可能性があり、再度 `gpt2` vs `llama` の判定から始める必要がある。
- llama.cpp upstream の pre-tokenizer allow-list に `gpt2-small-japanese-char` を追加する PR は本プロジェクト scope 外。もし Zenz 著者自身が upstream に patch を投げていれば llama-cpp-2 の version bump で解決する可能性はあるが、現時点 (0.1.145) では未対応。

## P1-2-9 empirical verification (deferred — 当初 WBS、未実施時の記述)

Zenz-v2.5-medium GGUF が実装セッション内で未入手のため、Phase 1 follow-up として user が実施する。本セッションで確認した環境制約と代替手段を以下に記す。

### 環境前提 (2026-04-24 時点の調査結果)

- `huggingface-cli` は本プロジェクト開発環境に未 install。
- `uv` / `uvx` は install 済み (CLAUDE.md modern-toolchain.md 推奨 Python tool)。`uvx --from huggingface_hub huggingface-cli ...` で一時実行できる。
- `curl` / `wget` は利用可能。HuggingFace の file URL (`https://huggingface.co/<repo>/resolve/main/<file>`) を直接 download することも可能。

### Zenz GGUF model の入手先: 公開 repo と gated repo の区別

spec §3.2 の default である **`Miwa-Keita/zenz-v2.5-medium-gguf` は gated repository** (HuggingFace API が HTTP 401 "Invalid username or password" を返す)。download には HuggingFace account の login + 当該 gated access の approval が必要である。

非 gated の公開 GGUF 代替 (本セッションで `curl` + HuggingFace API により files section を確認済み):

| Repo | 主要 GGUF file | quantization |
|------|----------------|-------------|
| `Miwa-Keita/zenz-v1` | `ggml-model-Q8_0.gguf` | Q8_0 |
| `Miwa-Keita/zenz-v2-gguf` | `zenz-v2-Q5_K_M.gguf` | Q5_K_M |
| `Miwa-Keita/zenz-v3-small-gguf` | (`curl ... siblings` で要確認) | — |
| `Miwa-Keita/zenz-v3.1-small-gguf` | `ggml-model-Q5_K_M.gguf` | Q5_K_M |
| `Miwa-Keita/zenz-v3.1-xsmall-gguf` | (`curl ... siblings` で要確認) | — |

### download 手順 (3 option — user の環境と model 選択に応じて選ぶ)

**Option A — 公開 GGUF を `curl` で直接 download (auth 不要、最短)**

例: `zenz-v3.1-small-gguf` を `$HOME/.cache/kotoha/models/` に配置する場合:

```bash
mkdir -p "$HOME/.cache/kotoha/models"
curl -L -o "$HOME/.cache/kotoha/models/zenz-v3.1-small-Q5_K_M.gguf" \
  "https://huggingface.co/Miwa-Keita/zenz-v3.1-small-gguf/resolve/main/ggml-model-Q5_K_M.gguf"
export KOTOHA_ZENZ_MODEL_PATH="$HOME/.cache/kotoha/models/zenz-v3.1-small-Q5_K_M.gguf"
```

別候補として `Miwa-Keita/zenz-v2-gguf` の `zenz-v2-Q5_K_M.gguf` も同じ URL pattern で取得可能。

**Option B — `uvx` 経由で `huggingface-cli` を使う (複数 file / snapshot 単位の取得に便利)**

```bash
uvx --from huggingface_hub huggingface-cli download \
  Miwa-Keita/zenz-v3.1-small-gguf \
  --local-dir "$HOME/.cache/kotoha/models/zenz-v3.1-small-gguf"
export KOTOHA_ZENZ_MODEL_PATH="$HOME/.cache/kotoha/models/zenz-v3.1-small-gguf/ggml-model-Q5_K_M.gguf"
```

**Option C — spec default (`zenz-v2.5-medium-gguf`) を使いたい場合 (auth 必要)**

1. <https://huggingface.co/Miwa-Keita/zenz-v2.5-medium-gguf> にアクセスし、gated access を request (HuggingFace account が前提)。
2. approval 後に HuggingFace token を発行 (<https://huggingface.co/settings/tokens>)。
3. `uvx --from huggingface_hub huggingface-cli login` で token を登録。
4. `uvx --from huggingface_hub huggingface-cli download Miwa-Keita/zenz-v2.5-medium-gguf --local-dir "$HOME/.cache/kotoha/models/zenz-v2.5-medium-gguf"` で取得。

### Layer 3 smoke 実行と fixture 調整

上記 Option のいずれかで `KOTOHA_ZENZ_MODEL_PATH` を設定した後:

```bash
cargo test -p kotoha-core --features zenz-smoke --test kanji_zenz_smoke -- --nocapture
```

- 全 5 件 PASS が理想。FAIL した場合、top-1 出力と `tests/fixtures/kanji_smoke.tsv` の `expected_substring` が不一致。
- 不一致時は実測値で TSV を更新し、`test(kanji): adjust Layer 3 fixture for actual <model-id> output` commit で反映する。
- `build_prompt` の PUA separator codepoint (U+EE00..U+EE02) を GGUF tokenizer の `added_tokens` metadata と照合。相違があれば `zenz.rs` 側の `CONTEXT` / `INPUT` / `OUTPUT` 定数を実値で置換して `fix(kanji): align build_prompt PUA tokens with Zenz GGUF added_tokens` として commit する。

### latency 測定手順

```bash
cargo test -p kotoha-core --features zenz-smoke --test kanji_zenz_smoke -- --nocapture --test-threads=1
```

`--test-threads=1` で逐次実行し、1 件目の load (cold start) と 2 件目以降 (warm cache) の実行時間差を観測する。測定値は spec §8.3 target (10〜30 秒 / CPU inference) に照らして本 WBS の「Cold start / warm cache latency 測定」section に追記する。

### ADR 0009 (P1-4 起票予定) への input

gated default が実使用上の障壁となる場合、ADR 0009 で Phase 1 default を以下のいずれかに切替える選択肢を比較検討する:

- **選択肢 1 — 現状維持**: spec §3.2 の `zenz-v2.5-medium-gguf` を default として継続。ユーザに HF login + gated access 許可を求める。
- **選択肢 2 — 公開代替に差し替え**: default を `zenz-v3.1-small-gguf` (Q5_K_M、公開) に変更。quality 低下の影響評価が必要。
- **選択肢 3 — 両対応**: CLI の `--model` option で明示指定を必須とし、README に「spec default: v2.5-medium (gated)、quick start: v3.1-small (公開)」を併記。

判断材料として、P1-2-9 を v3.1-small で empirical に走らせた結果 (品質 / latency) を ADR 0009 に記載する。

## P1-3 への申し送り

- `ZenzBackend` は llama-cpp-2 経由で fully functional となった。`kotoha-kanji` binary (P1-3) は `load_backend(&BackendConfig::Zenz { model_path })` を直接呼べる。
- CLI には `--model <PATH>` option を追加する必要あり (spec §7.1)。`ZenzBackend` 側の追加変更は不要。
- `process_line` pure function (spec §7.5) は `validate_input` + `backend.convert` + `score_sort_dedupe` の既存 helper を再利用する。
- Layer 4 E2E smoke (`scripts/phase1-smoke.sh`) は `kotoha-romaji | kotoha-kanji` pipe を driver とし、fixture は Layer 3 と共通化できる見込み。

## P1-4 への申し送り (plan refinement bundle)

P1-1 + P1-2 の review で surface した findings と plan bug を P1-4 docs バンドルで解消する:

- P1-1 review 繰延項目: ARCH-3 (lib.rs re-export scope 判断)、TEST-1 (score 降順 fixture 強化)、TEST-2 (stable_for_equal_scores 3+ 要素化)、SEC-2 (validate_input error message に U+コードポイント併記)。
- P1-2 review Medium: `OnceLock` race on init failure (Phase 2 concurrency tracking issue を別 file)、`max_new_tokens` rationale comment drift 修正。
- P1-2 review Low: `seed` no-op の doc comment、`score 0.0` placeholder の doc comment、`from_utf8_lossy` に `tracing::warn!`、Layer 3 test helper `.expect` message 改善。
- Plan bugs (P1-1 実装中に surface): P1-1-8 `.expect_err` → `match` pattern、P1-1-10 struct-literal → `default + field mutation` idiom (両者とも P1-1 WBS で既記録)。
- ADR 0009 (Zenz model version policy): P1-2 pinning + Miwa-Keita dataset README authoritative source を参照。
- ADR 0010 (Kanji backend trait design): P1-1 trait + P1-2 ZenzBackend を concrete implementation として参照。
- ADR 0011 (Feature flag design for Zenz): `[workspace.dependencies]` + `optional = true` + `dep:llama-cpp-2` pattern を参照。
- spec §5.6 / §6 に hiragana-input contract と zenz backend 直前の katakana 変換の footnote を追記 (1b research の反映)。

## 成果物リンク

- PR: <https://github.com/std-koh-hinooka/kotoha-ime/pull/70>
- ISSUE: <https://github.com/std-koh-hinooka/kotoha-ime/issues/69>
- Merge commit: `49ed56e`
- Plan PR: <https://github.com/std-koh-hinooka/kotoha-ime/pull/68> (#67)
- Spec: `docs/superpowers/specs/2026-04-24-kotoha-phase-1-design.md`
- Overall plan: `docs/superpowers/plans/2026-04-24-kotoha-phase-1-implementation.md`
- Detailed plan: `docs/superpowers/plans/2026-04-24-kotoha-phase-1-p1-2.md`
- Dataset authoritative source: <https://huggingface.co/Miwa-Keita/zenz-v2.5-dataset>
- llama-cpp-2 crate: <https://crates.io/crates/llama-cpp-2>
- AzooKey Zenzai reference: <https://github.com/azooKey/AzooKeyKanaKanjiConverter/blob/main/Docs/zenzai.md>

## P1-2-9 empirical verification 実施結果 (2026-04-24、Option A 第二候補 — zenz-v2-gguf)

v3.1-small 失敗 (pre-tokenizer 非対応) を受け、より古い世代 `Miwa-Keita/zenz-v2-gguf` (2024-08-04 release) で再検証した。仮説は「v2 系は `gpt2-small-japanese-char` pre-tokenizer 以前の tokenizer を使用しているので llama-cpp-2 0.1.145 と compat な可能性がある」であったが、検証結果は **仮説の棄却** である。

### Model file

- 配置: `$HOME/.cache/kotoha/models/zenz-v2-gguf/zenz-v2-Q5_K_M.gguf`
- サイズ: 72 MiB (on-disk) / GGUF 内 `file size = 68.76 MiB (6.07 BPW)`
- 出典: <https://huggingface.co/Miwa-Keita/zenz-v2-gguf>
- GGUF magic 確認: yes (`GGUF` ASCII先頭4バイト確認済み)

### GGUF metadata

- `general.architecture`: `gpt2`
- `general.name`: `zenz-v2`
- `tokenizer.ggml.model`: `gpt2`
- `tokenizer.ggml.pre`: **`gpt2-small-japanese-char`**(v3.1-small と同一の pre-tokenizer)
- 総 token 数: 6000(vocab size。reader が 12005 parts を返すのは tokens + token\_types 合算。`tokens field` arr 長は 6000)
- PUA-containing tokens (U+E000..U+F8FF): **0 個**
- Bracket special tokens (`<` で始まり `>` で終わる token): 2 個のみ (`<s>` id=10, `</s>` id=12)
- BOS id=1, EOS id=2, PAD id=1, `add_bos_token=false`

### Layer 3 smoke 結果

KOTOHA_ZENZ_MODEL_PATH を設定し `cargo test -p kotoha-core --features zenz-smoke --test kanji_zenz_smoke -- --nocapture --test-threads=1` を実行:

- zenz_smoke_1_nihongo: **FAIL** (`ModelLoadFailed { source: NullResult }`)
- zenz_smoke_2_kanji: **FAIL** (同上)
- zenz_smoke_3_ashita: **FAIL** (同上)
- zenz_smoke_4_yamada_san: **FAIL** (同上)
- zenz_smoke_5_kotoba: **FAIL** (同上)
- 合計: **0/5 PASS**

llama.cpp stderr:

```text
llama_model_load: error loading model: error loading model vocabulary: unknown pre-tokenizer type: 'gpt2-small-japanese-char'
llama_model_load_from_file_impl: failed to load model
```

v3.1-small と **完全に同一の失敗モード**。load 段階で vocab が reject されるため prompt format や PUA token 整合性までは到達しない。

### 考察

1. **仮説棄却**: `Miwa-Keita/zenz-v2-gguf` (2024-08-04) も `Miwa-Keita/zenz-v3.1-small-gguf` と同じく `gpt2-small-japanese-char` pre-tokenizer で GGUF 化されている。時期が古い v2 世代でも base model は既に `ku-nlp/gpt2-small-japanese-char` 系であった、あるいは少なくとも GGUF converter 側で同じ pre-tokenizer 名が付与されている。llama-cpp-2 0.1.145 (bundled llama.cpp) の allow-list にこの pre-tokenizer が入っていないため、**Miwa-Keita 配布の全 Zenz GGUF が現行 llama-cpp-2 ではロード不可**と判断する。
2. **build_prompt 仮定への影響**: v2 の vocab には PUA codepoint token が 0 個、bracket special token も `<s>` / `</s>` のみ。現在の `ZenzBackend::build_prompt` が前提としている PUA delimiter (spec §5.6 / ADR 0009 初版) は v2 では文字通りには存在しない。ロードに到達しないため実証できないが、**現行 build_prompt 実装が Miwa-Keita 世代全般に適用できない可能性が高い** ことを示唆する (PUA codepoint は v3.x 以降の convention の可能性)。P1-4 の ADR 0009 更新で、PUA 方式が v3.x 固有か v2 系まで遡れるかの明示が必要。
3. **Phase 1 での結論**: Option A(Miwa-Keita 配布の GGUF を直接ロード)は、**zenz-v1 / v2 / v2-5 / v3 / v3.1-small のいずれの GGUF も同じ pre-tokenizer 壁に阻まれる高い蓋然性** がある。v1 を追加で試行するコストは低いが、期待値は限りなく低い。Phase 1 の model sourcing 判断は Option B / Option C (自前 convert、または llama-cpp-2 upgrade 待ち) へ pivot することを ADR 0009 に記録すべきである。
4. **ADR 0009 input**: (a) v2 / v3.1-small 両方で同じ pre-tokenizer エラーが確定、(b) llama-cpp-2 0.1.145 は `gpt2-small-japanese-char` を受け付けない、(c) PUA token は少なくとも v2 世代では vocab に存在しない — の 3 点を Decision Record に反映し、Phase 1 の default model sourcing を Option A から外す根拠とする。

### v1 追検証について

計画上「v2 が駄目なら v1 を軽く確認」とあったが、上記考察 3 のとおり v1 まで同じ convention である蓋然性が高く、**追加ダウンロード + 再テストのコストに見合う情報利得は低い**と判断し、本 WBS 段階では実施しない。実施を希望する場合は別 ISSUE として立ててから進める。

## P1-2-9 (empirical verification, v2 attempt) 結論

- Option A 第一候補 (v3.1-small) / 第二候補 (v2) ともに **現行 llama-cpp-2 0.1.145 ではロード不可**。
- Phase 1 の zenz backend を実運用可能にするには **Option B (自前 HF model から GGUF 再変換し pre-tokenizer を llama-compat に調整) または Option C (llama-cpp-2 が `gpt2-small-japanese-char` を support する version へ bump)** が必須。
- 本 empirical verification 自体のタスクは「Option A 不成立を確定させる」ことが deliverable として完了した。ADR 0009 更新と次手段選定は P1-4 docs bundle に繰延する。

## P1-2-9 model selection pivot (2026-04-24、Zenz 系全 blocker を受けた再設計)

### 背景

zenz-v3.1-small-gguf / zenz-v2-gguf の empirical verification が `tokenizer.ggml.pre = gpt2-small-japanese-char` という llama.cpp 未登録 pre-tokenizer により load 段階で abort した。upstream llama.cpp master にも該当文字列は未登録のため、llama-cpp-2 version bump でも解消不可と判明 (ggerganov/llama.cpp `src/llama-vocab.cpp` 直接確認)。Miwa-Keita 著者の spec default `zenz-v2.5-medium-gguf` は別途 gated + 認証失敗 (401/404)。→ Phase 1 backend を Zenz 系以外の日本語対応 LLM へ pivot する判断となった。

本 section は scratch 3 files (`p1-2-9-alternative-models-research.md` / `p1-2-9-gemma-investigation.md` / `p1-2-9-empirical-3way-comparison.md`、計 451 行) の調査結果を consolidate したものである。元の scratch は本 commit で削除した。

### 代替候補調査 (Category A: kana→kanji 専用モデル、Zenz 以外)

HuggingFace 網羅検索 (`/api/models?search=kana+kanji`, `search=japanese+ime`, `search=kana` 各 limit 20) の結果:

- `fujie/kana_kanji_20240307` — safetensors only、GGUF なし。
- `mradermacher/kanji-2-kana-gemma3-1b-GGUF` — 方向が逆 (漢字→かなの ruby 用途)。
- `yuuki14202028/gpt2-kanakanji` — safetensors only、同じ gpt2-japanese 系罠の懸念。

→ 公開されている kana→kanji specialized open model は事実上 Zenz family のみ。非 Zenz 路線は必然的に汎用 Japanese-capable small LLM + prompt engineering になる。

### 代替候補調査 (Category B: 汎用 Japanese small LLM)

| 順位 | Model | arch | pre | size (Q5_K_M) | license |
|---|---|---|---|---|---|
| 1 | Qwen2.5-1.5B-Instruct | qwen2 | qwen2 | 1.29 GB | Apache 2.0 |
| 2 | Gemma-2-2B-jpn-it (Google 日本語 native FT、2024-10) | gemma2 | default | 1.92 GB | Gemma |
| 3 | Gemma-3-1B-it | gemma3 | default | 0.85 GB | Gemma |

### Gemma family 調査 (ユーザ指摘、google/collections 起点)

Gemma 1 / Gemma 2 (2b/9b/27b + jpn FT) / Gemma 3 (1b/4b/12b/27b + 270m) / Gemma 3n (mobile MatFormer) / Gemma 4 (E2B/E4B/31B/26B-A4B、2026-04-10 リリース、license を Apache 2.0 に変更) の 5 世代を確認。llama-cpp-2 0.1.145 の bundle llama.cpp commit は `e21cdc11` (2026-04-13) で `LLM_ARCH_GEMMA4` + `LLAMA_VOCAB_PRE_TYPE_GEMMA4` 対応済と直接確認。Gemma 4 E2B (Q4_K_M 3.11 GB) は Phase 1 2GB budget 超過のため Phase 2 検討扱い。

### 3-way head-to-head empirical 比較 (llama-cpp-python via uvx)

Test cases (Layer 3 fixture と同一): にほんご/かんじ/あした/やまださん/ことば → 日本語/漢字/明日/山田/言葉。

実行環境: llama-cpp-python 0.3.20 (uvx 経由、初回 source build) / AMD Zen3 (`-march=znver3`、ggml CPU backend、OpenMP) / `n_ctx=2048` `n_batch=256` `n_threads=4` `temperature=0.0` `max_tokens=32` `seed=0` / Chat template は llama-cpp-python が GGUF 埋め込み template を自動適用。Prompt は system + few-shot 2 例 (`わたし→私`, `ありがとう→有難う`) + target 入力で 3 モデル同一。

| Model | Pass | Avg latency | Cold load | License | Size |
|---|---|---|---|---|---|
| **Gemma-2-2B-jpn-it Q5_K_M** | **5/5** | 4,105 ms | 10.6 s | Gemma | 1.92 GB |
| Qwen2.5-1.5B-Instruct Q5_K_M | 3/5 | 3,264 ms | 1.3 s | Apache 2.0 | 1.29 GB |
| Gemma-3-1B-it Q5_K_M | 2/5 | 3,984 ms | 3.7 s | Gemma | 0.85 GB |

Per-case top-1:

- にほんご: Gemma-2-jpn=日本語 PASS / Qwen=日本語 PASS / Gemma-3=日本語 PASS
- かんじ: Gemma-2-jpn=漢字 PASS / Qwen=カンジ FAIL (katakana 化) / Gemma-3=感謝します FAIL (hallucination)
- あした: Gemma-2-jpn=明日 PASS / Qwen=明日 PASS / Gemma-3=します FAIL (hallucination)
- やまださん: Gemma-2-jpn=山田さん PASS (敬称保持) / Qwen=やまださん FAIL (変換せず) / Gemma-3=又楽ます FAIL (hallucination)
- ことば: Gemma-2-jpn=言葉 PASS / Qwen=言葉 PASS / Gemma-3=言葉 PASS

### 勝者: Gemma-2-2B-jpn-it (Q5_K_M、1.92 GB)

- narrow task (kana→kanji) に対する日本語 native SFT の優位性が empirical に確認された。
- 敬称「さん」を自然に保持 (IME として正しい挙動)。
- Cold load 10.6s + 5 × 4.1s ≈ 31s → spec §8.3 target (10〜30s) に fit。
- Gemma license は商用 attribution-based 利用可。OSS 公開時は license ファイル同梱と Gemma Terms of Use (<https://ai.google.dev/gemma/terms>) への compliance が必要。ADR 0009 で明文化する。
- community GGUF 採用候補: `bartowski/gemma-2-2b-jpn-it-GGUF` (または grapevine-AI / MCZK の imatrix 版)。いずれも Google 公式 `google/gemma-2-2b-jpn-it` の重みを量子化した再配布。

### Phase 2 migration 参考候補 (ユーザ指摘の 31B 系 2 repo)

いずれも Phase 1 採用外 (31B dense ≈ 13〜18 GB、2GB budget の 6〜9 倍)。Phase 2 の tiered model 設計時に候補化する。

- **`Jackrong/Gemopus-4-31B-it-GGUF`** (2026-04-15、Apache 2.0、5119 DL/month): `google/gemma-4-31B-it` の community SFT 派生。"Gemopus" = Gemma + Opus の命名のみで Claude 関連性は無し。philosophy は "stability first" (Gemma 4 native reasoning order を保持、英語 answer quality / structure / clarity / consistency 改善に focus)、Claude-style CoT distillation を明示的に拒否。Unsloth + post-fix gradient accumulation で訓練。Quantization は BF16 / Q3_K_M / Q4_K_M / Q5_K_M / Q5_K_S / Q6_K / Q8_0 + mmproj.gguf の 7 variant。tags は gguf / gemma / gemma4 / instruction-tuned / reasoning / alignment / text-generation、言語は en/zh/ko (ja は frontmatter には記載されるが tags には無し → JP 能力は base 継承のみで SFT 方向性は英語 reasoning)。**Phase 1 判定: 不適合** (31B ≥ 13 GB even at Q3_K_M、Phase 1 budget の 6〜9 倍超過、FT 方向が英語寄りで narrow JP task への寄与は期待薄)。
- **`batiai/gemma-4-31B-it-GGUF`** (2026-04-18、2057 DL/month): `google/gemma-4-31B-it` の純粋 GGUF quantization (FT なし、base-only)。作者 BatiAI は商用 AI 企業 (macOS 向け BatiFlow product あり)。focus は macOS Apple Silicon Metal on-device inference (Ollama 経由)。Quantization は IQ3_M / IQ4_XS (imatrix) / Q4_K_M / Q6_K + mmproj-BF16 / mmproj-Q6_K の 4 quant + 2 mmproj。tags は imatrix / apple-silicon / ollama / multimodal / vision / on-device。license tag は `other` / `license_name: gemma` / link は <https://ai.google.dev/gemma/terms> を記載するが、**upstream Google `google/gemma-4-31B-it` API は現時点で `apache-2.0` を返すため batiai の frontmatter は pre-release の stale metadata である可能性が高い** (Phase 2 採用検討時に再確認要)。**Phase 1 判定: 不適合** (size reality は Gemopus と同じ)、ただし base-only quantization として cleaner で、Phase 2 の tiered migration で GPU/Metal acceleration 追加時の候補として記録に残す。

両 repo とも multimodal (vision) 対応だが Phase 1 は text-only のため mmproj は不要。

### ADR 0009 (P1-4 で起票) への input 集約

- Phase 1 default: **Gemma-2-2B-jpn-it Q5_K_M** (empirical 5/5 PASS、spec §8.3 latency target fit)。
- Phase 2 migration candidates (GPU/Metal acceleration 前提): Gemma 4 E2B (Apache 2.0)、batiai/gemma-4-31B-it (純粋 quantization、Metal 最適化)。Jackrong/Gemopus は英語寄り SFT のため narrow JP task には不向きとして候補外。
- license 方針: Apache 2.0 (Qwen / Gemma 4) を優先、Gemma license (Gemma 2 / 3) は attribution + propagation 条件で OSS 互換と判断するが終局的には ADR 0009 で Gemma Terms of Use の全条項 review を実施する。
- Zenz family: architectural blocker (gpt2-small-japanese-char) により Phase 1/2 ともに直接採用不可。Phase 3+ で upstream llama.cpp への pre-tokenizer 追加 PR または Kotoha 側での自前 pre-tokenizer 実装を検討する別 issue として分離する。

### 本セッションでの handoff

Phase 1 P1-2 実装 (ZenzBackend + Layer 3) は architectural には動くが empirical には未検証。新たに判明した事実:

- `ZenzBackend` という名前は backend-specific すぎる → `LlamaCppBackend` 汎用化が必要。
- `build_prompt` の PUA codepoint 実装は Gemma-2-2B-jpn-it では不要 (chat template で置換)。
- hiragana→katakana 前処理は Gemma では不要 (native JP で hiragana 直受け可)。

これらを反映する **P1-2.5 refactor milestone** を次セッションで起票する。内容:

1. `superpowers:writing-plans` skill で `docs/superpowers/plans/2026-04-XX-kotoha-phase-1-p1-2-5.md` を起票。
2. ISSUE + branch: `feature/NN-llama-cpp-backend-gemma-2-jpn`。
3. Rename: `ZenzBackend` → `LlamaCppBackend`、`BackendConfig::Zenz { model_path }` → `BackendConfig::LlamaCpp { model_path, prompt_template: PromptTemplate }`。
4. `PromptTemplate` enum (Gemma2InstructChat / Qwen2Chat / Custom の 3 variant)。
5. `infer` を llama-cpp-2 の chat template API (`apply_chat_template`) 利用に改修。
6. hiragana→katakana 前処理を削除 (Gemma template では不要)。
7. Layer 3 fixture を 15〜20 cases に拡張 (文単位も含める)。
8. Spec 改訂 (中程度): §3.2 default 差替、§3.3 AzooKey を historical reference に格下げ、§5.6/§6 katakana 前処理を "backend 内部依存" に抽象化。
9. ADR 0009 draft (P1-4 正式起票の先行メモ)。

scope 見積: Medium tier (10 files / 500 LOC、spec 改訂込)、所要 1 day。既存 PR #70 は revert せず forward refactor (Zenz attempt を architectural blocker で不採用とした経緯は commit 本文で保持)。

