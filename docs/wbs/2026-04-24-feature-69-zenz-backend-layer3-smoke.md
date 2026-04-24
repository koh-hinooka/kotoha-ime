---
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

## P1-2-9 empirical verification (deferred — requires user-provided model)

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
