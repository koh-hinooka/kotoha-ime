# ADR 0010 — Phase 5 として Kotoha 専用 romaji-base かな→漢字モデルを自作する

- **Status**: Accepted (Phase 5 方針として承認。詳細パラメータは Phase 4 完了時の Phase 5 kick-off で確定)
- **Date**: 2026-04-25
- **Deciders**: Kotoha Phase 1 maintainers
- **Related ISSUE**: #77 (Phase 5 foundation docs)
- **Related PRs**: #76 (P1-2.5 follow-up — 14/15 empirical basis)

## Context

Phase 1 P1-2.5 follow-up (PR #76 / ISSUE #75 / merge commit `3eccaa1`) の empirical verification で、以下 2 つの事実が確定した。

### C1. Gemma-2-2B-jpn-it Q5_K_M の in-context learning (ICL) 限界

Layer 3 smoke fixture 15 行の通算 pass rate は `14/15` であり、row 3「あした → 明日」のみが「翌日」を生成し、v5〜v12 の 8 世代に渡る prompt iteration で解消不能であった。試行した手法と結果を再掲する。

| 手法 | 結果 |
|---|---|
| positive few-shot 13 pair に「あした → 明日」を含める | row 3 は「翌日」を出力 |
| few-shot 末尾 (query 直前) に「あした → 明日」を配置 (隣接 bias 強化) | row 3 は「翌日」を出力 |
| directive 内に「『あした』は『明日』であり『翌日』ではありません」を埋込 | row 3 は「翌日」を出力 |
| 「翻訳・類義語置換・言い換え禁止」を directive 冒頭に明示 | row 3 は「翌日」を出力 |

同種の negative example「ぎゅうにゅう → 牛乳 (not ミルク)」「りょうり → 料理 (not クッキング)」は v12 で解消した。row 3 のみ解消しない原因は、Gemma-2-2B-jpn-it 事前学習分布において「あした ↔ 翌日」の語彙意味的距離が他の synonym 対に比べ極めて小さく、2B parameter instruction-tuning で学習した pretrain 語彙バイアスが、prompt の 13 pair few-shot + 否定例 + 直接指示の信号強度を上回っているためと推定する。PR #76 WBS の「Gemma-2-2B-jpn-it の ICL 限界に関する分析」節に詳細を記録した。

### C2. Karukan 調査で判明した成功要因

Karukan (Linux 向け日本語 IME) は `jinen-v1-small` (90M parameter、GPT-2 base、Q5_K_M 量子化、約 80MB) を採用し、本問題を起こさない。Karukan を Kotoha と比較して観察した成功要因は以下 4 点である。

1. **タスク専用 fine-tune モデル**: `jinen-v1-small` は kana→kanji 変換タスクに特化して fine-tune された GPT-2 系 decoder-only モデルである。汎用 instruction-following LLM ではない。「あした → 明日」は学習分布の内側に位置するため、ICL に依存しない。
2. **学習済み PUA (Private Use Area) special tokens による構造指示**: tokenizer は `\u{ee02}` を `CONTEXT` として、`\u{ee00}` を `INPUT_START` として、`\u{ee01}` を `OUTPUT_START` として fine-tuning 時に学習している。推論時の prompt は `\u{ee02}今日は\u{ee00}コンニチハ\u{ee01}` の形式で構造的に渡す。Prompt injection 耐性と構造安定性の両方に寄与する。
3. **外部 HuggingFace tokenizer の使用**: Karukan は Rust の `tokenizers` crate で `tokenizer.json` を直接 load し、llama.cpp の内蔵 tokenizer を完全バイパスする。P1-2-9 で Zenz GGUF が `gpt2-small-japanese-char` pre-tokenizer の allow-list 不対応により llama.cpp load 失敗した問題は、この方式で回避可能である。
4. **context 情報の受理**: 推論関数 `build_jinen_prompt(katakana, context)` は周辺テキストを `CONTEXT` token 後に注入する。Phase 5 の Kotoha モデルも同様の context 注入をサポートする。

### C3. Karukan にも残る前処理層起因の不便

ユーザーが Karukan を利用中に観察した以下の現象は、モデルを大きくしても解消しない。これらは全て「モデル到達前の決定論的 romaji→katakana 変換層」の限界である。

- 「s」単打: fcitx5 の前処理層が「未確定 prefix」として保留し、モデルに届かない
- 「si」入力: Hepburn 規則の前処理層が「し」に変換せず、`si` のまま残留する
- 「s<backspace>」: 前処理層の状態機械が中間状態で破綻する
- 「まうｓ」(「ます」の typo): 前処理層は typo を解釈できず、モデルが「ます」に復元する機会を奪う

### C4. プログラマ / 技術ライター use case における JP + EN 混在入力要件

Kotoha の想定利用者はプログラマおよび技術ライターが主要セグメントである。このセグメントが日常入力する文面 (コミットメッセージの編集、GitHub issue / PR 本文、技術 blog、コード内コメント) では、日本語と英語が 1 行内で混在する形が常態化している。具体例として、romaji 入力 `tuginocommitwoshuuseisitepull requestwodasite` はユーザー意図として「次のcommitを修正してpull requestを出して」を表し、以下 3 条件を同時に満たす変換を期待する。(a) `commit` / `pull request` は英字列のまま出力する、(b) 2 語の間の空白 (`pull request` の中間) は word separator として保持され変換 trigger 扱いしない、(c) `tugi no` / `wo shuusei site` / `wo dasite` の JP 部分は既存の kana→kanji 経路を通って漢字仮名混じり表現に変換される。

Phase 0 ADR 0002 は「Shift 起動で `InputMode::Direct` に一時遷移し Enter 確定で `InputMode::Hiragana` に自動復帰する Transient モード」を決定したが、当該 ADR が扱うのは「行末に短い英字列が入る」ケースであり、「行内に EN span が挟まる」ケースは扱っていない。従って C4 は ADR 0002 を前提として別問題と位置付ける。なお本要件は Phase 1 row 3 (C1) と同系統の LLM ICL 限界事例である。汎用 instruction-tuned LLM の prompt 指示では「context 依存の言語判定」を安定させるのは困難であり、Phase 5 task-specific fine-tune で学習分布内に mixed JP/EN 文面を含めることで根本解決する方が構造的に正しい。

## Decision

Kotoha は Phase 5 として **Kotoha 専用 romaji-base かな→漢字変換モデル** を自作する。Decision の構成は 7 項目とする。

### D1. Phase 5 として romaji-base 専用モデルを自作する

Phase 5 を「Kotoha custom romaji-base model」と位置付け、kana→kanji 変換専用に fine-tune した 90M〜180M parameter decoder-only transformer を Kotoha 固有に構築する。Phase 5 の完了条件は「Phase 1 の 14/15 baseline を維持しつつ、row 3 相当の ICL 限界問題 + typo robustness + partial-input 対応を smoke / golden / typo robustness / latency SLA の 4 評価で PASS すること」とする。

### D2. 入力境界を raw romaji keystrokes 直接とする

従来 (Phase 1 baseline) の「fcitx5 前処理で確定した katakana/hiragana 列をモデルに渡す」方式を Phase 5 では採用しない。Phase 5 のモデルは **raw romaji keystrokes (ASCII 列)** を直接受理し、自身で romaji→kana 的な読み解きを行う。この境界変更により、以下が possible になる。

- 部分入力 `s` / `sh` / `si` を文脈で統合解釈する
- typo `maus` / `まうｓ` をモデル側で「ます」と認識する fine-tune が可能になる
- `backspace` を含む編集履歴を special token で表現できる
- 半角/全角モード切替などの境界問題が根本解消する

### D3. 学習戦略は Phase 5 kick-off で empirical 比較して確定する

学習戦略候補を 3 本洗い出した上で、default recommendation を **B3 distillation** とする。最終確定は Phase 5 kick-off 時点の empirical 比較で行う。

| 候補 | 手法 | 推奨度 |
|---|---|---|
| **B1** | scratch training (GPT-2 Small アーキテクチャ 90M を random 初期化から学習) | 候補 |
| **B2** | LoRA fine-tune (Gemma-2-2B-jpn-it の adapter 学習) | 除外推奨 |
| **B3** | distillation (Gemma-4-31B-it 等を teacher、90〜180M student に転移学習) | **default** |

B2 を除外推奨とする理由: inference 時に base model (2.6B) の load が必要で、IME 常駐用途の size budget (Phase 1 現状 1.92GB、目標 ≤ 200MB) を満たさない。

### D4. Special tokens の役割を確定する

Phase 5 kick-off 時点で Karukan 踏襲 (PUA 領域 `\u{ee00}-\u{ee0F}`) か新設 (別領域 / 別 id) かを empirical 判断するが、役割は事前に固定する。最低限学習させる special tokens の役割は以下 4 種。

- `<ctx>` — 周辺テキスト (context) の開始
- `<romaji>` — romaji 入力の開始
- `<out>` — モデル出力の開始
- `<eos>` — 生成終了

Phase 5 の拡張 special tokens 候補として `<edit>` (編集履歴), `<partial>` (未確定 prefix), `<typo-hint>` (typo tolerance 強調) を検討する。

PUA id は Phase 5 kick-off で empirical 確定する (Karukan 踏襲 `\u{ee02}=<ctx>` / `\u{ee00}=<romaji>` / `\u{ee01}=<out>` / `\u{ee03}=<eos>` を初期候補とする。詳細と追加 token は [Phase 5 spec §4.5](../superpowers/specs/2026-04-25-kotoha-phase-5-custom-model.md) を参照)。

### D5. 外部 HuggingFace tokenizer を必須とする

Kotoha の推論コードは Rust の `tokenizers` crate で `tokenizer.json` を直接 load し、llama.cpp 内蔵 tokenizer はバイパスする。これにより以下を達成する。

- Zenz family が遭遇した `gpt2-small-japanese-char` pre-tokenizer 不対応問題の構造的回避
- special tokens の ID 管理を tokenizer 側に一元化
- Phase 6 以降の pre-tokenizer 拡張 (編集履歴 / partial-input) を llama.cpp upstream から独立して実装可能にする

### D6. 推論は GGUF Q5_K_M 量子化で IME 常駐とする

推論パス構成は以下とする。

- モデル size 目標: 90M〜180M parameter、GGUF Q5_K_M で **≤ 200MB**
- latency 目標: CPU 推論で **p50 ≤ 30ms、p99 ≤ 100ms**
- 量子化精度候補: Q5_K_M を default、Q4_K_M / Q8_0 を empirical 比較 (Phase 5 kick-off)
- 実装統合: `BackendConfig::KotohaNative { model_path, tokenizer_path }` variant を Phase 5 で追加 (Phase 1 の `BackendConfig::LlamaCpp` と並立)

### D7. ROADMAP restructure

Phase 5 の位置付け変更に伴い、既存 ROADMAP を以下のように restructure する。

| 旧 Phase | 新 Phase | 名称 |
|---|---|---|
| 0 | 0 | Foundation |
| 1 | 1 | Kana→Kanji conversion (Gemma-2-2B-jpn-it baseline, 14/15) |
| 2 | 2 | Dictionary and learning |
| 3 | 3 | IBus integration |
| 4 | 4 | fcitx5 integration |
| — | **5 (new)** | **Kotoha custom romaji-base model** |
| 5 | 6 | Advanced features (typo correction + context reranking) |
| 6 | 7 | UX polish |

Phase 3 (IBus) と Phase 4 (fcitx5) は Gemma baseline で先行リリースし、IME shipping を遅延させない。Phase 5 (custom model) が Phase 6 (旧 Phase 5、Advanced features) の技術基盤を提供する。

### D8. Phase 5 custom model の primary goal に mixed JP/EN 自動判定を含める

Context C4 を受け、Phase 5 custom model は kana→kanji 変換に加えて以下 3 機能を primary goal として学習する。

1. **Language-context detection**: モデルは input romaji stream を prefix から逐次 consume し、各位置で「現在 EN span 中か JP 変換対象か」を decoder の hidden state に保持する。boundary の推定には空白 / punctuation / 語彙 plausibility を利用する。本機能は明示的 classifier ではなく暗黙学習で実現する方針とし、P5-A kick-off で empirical に妥当性を検証する。
2. **Context-aware space handling**: モデルは space 文字を変換 trigger ではなく「EN 文脈では word separator、JP 文脈では区切り記号」として解釈する。training data 側で space を含む mixed sentence を十分 sampling することで学習分布に含める。
3. **Mixed output generation**: 単一推論 pass で JP 部は kanji / kana surface、EN 部は ASCII 文字列として混在 decode する。special tokens ({ctx}, {romaji}, {out}, {eos}) に加えて {en-span} / {jp-span} の明示 marker を導入するか、暗黙で通すかは P5-A kick-off で empirical 判断する。

training data は以下 2 源から構成する mixed コーパスで補強する (詳細は Phase 5 spec §4.7 を参照)。

- Programming context: GitHub issues / commit messages / OSS README / 技術 blog (Zenn / Qiita 等) / 技術書 — Apache-2.0 / MIT 相当の再配布可能ライセンスのみを採用する
- 一般 loanword context: Wikipedia JP の technology / IT / business / culture カテゴリ (CC BY-SA 3.0) および青空文庫 (Public Domain)

データ比率は P5-A kick-off で empirical 確定する。初期案は「JP-only 60% / mixed (jp+en) 30% / EN-only 10%」とする。

## Consequences

### 正の帰結

- **row 3 類の ICL 限界問題を根本解決する**: task-specific fine-tune により、「あした → 明日」のような pretrain bias 起因の synonym 誤変換は学習分布内で解消する
- **typo tolerance の native 実現**: 「まうｓ → ます」「maus → ます」の意図的 typo 注入データで fine-tune することで、モデル側で typo 吸収が可能になる
- **partial-input の native 対応**: `s` / `sh` / `si` の部分入力から top-k 候補を即時算出でき、IME の逐次候補表示が実装可能になる
- **Phase 6 (旧 Phase 5) の技術基盤になる**: typo correction + context reranking は romaji-base モデルの上でこそ自然に実装できる
- **Size の大幅削減**: Gemma-2-2B-jpn-it の 2.6B / 1.92GB から 90〜180M / ≤ 200MB へ縮小する。IME 常駐用途に適合する
- **プログラマ / 技術ライター use case の IME 体験が根本改善する**: C4 で示した `tuginocommitwoshuuseisitepull requestwodasite` のような mixed JP/EN 入力が Tier 1 (モデル自動判定) で成立するため、ユーザは明示的モード切替を意識せずに混在文章を打鍵できる。
- **Phase 5 完成後、Tier 2 / Tier 3 の明示 mode 切替 UX は不要になる**: Phase 5 モデルが context 判定を担うため、Alternatives E が想定する `InputMode::Latin` sticky mode を実装する必要が無くなる。結果として Phase 0 ADR 0002 が決定した 2 値 InputMode (Hiragana / Direct) を Phase 5 完了後も維持できる。
- **row 3 類と mixed JP/EN の両方を単一モデルで解決する (投資集約)**: C1 の synonym bias と C4 の言語判定は別症状だが、いずれも「task-specific fine-tune で学習分布を制御する」ことで同時に解決可能である。Phase 5 の 1 本の training run で 2 要件を同時に満たせる。

### 負の帰結

- **学習インフラ整備の工数**: GPU 環境、data pipeline (LLM-JP / CC-100 / Wikipedia JP 抽出 + MeCab 形態素解析 + romaji 拡張 + typo 注入)、eval harness (smoke / regression / golden / typo / latency) の全セットアップが必要であり、Phase 5 前半の工数が膨らむ
- **学習データ品質が最終性能を支配する**: romaji 拡張規則 (Hepburn / Kunrei / waapuro) と typo 注入規則 (edit distance 1-3) の設計品質が、Phase 5 完了時の accuracy を決める
- **B3 distillation は teacher の Japanese quality に下限を制約される**: Gemma-4-31B-it 等の teacher が row 3 相当の synonym bias を持つ場合、student にも波及する可能性があり、evaluation で確認が必要
- **Phase 6 完遂までの schedule が延びる**: Phase 5 の data pipeline + training + evaluation で 3〜6 ヶ月程度を見込む。Phase 6 (旧 Phase 5) の着手時期は Phase 5 完了に連動する
- Phase 5 は手動 training を前提とし、CI-driven training pipeline の自動化は Phase 6 以降へ延期する (spec §7 参照)。
- **training data の scope が Wikipedia JP 単独から多 source へ拡大する**: D8 で定義した programming context + 一般 loanword context の 2 追加源 (GitHub issues / OSS README / 技術 blog / Wikipedia JP IT カテゴリ / 青空文庫) を P5-A corpus に取り込む必要があり、ライセンス確認と抽出スクリプトの整備工数が増える。
- **model complexity が増す (multi-language vocabulary + context span 管理)**: 単一言語 (JP) を想定した vocabulary から JP + EN 両対応の vocabulary へ拡張する必要があり、tokenizer 設計と special tokens 設計 (D4 + D8 で言及した `{en-span}` / `{jp-span}` marker) の empirical 検証工数が増える。
- **評価 fixture に mixed ケースを追加する必要がある**: Phase 5 spec §6 の smoke / regression / golden / typo robustness の 4 fixture に加え、「row 3 系 synonym bias + commit / pull request 等 programming context + 一般 loanword context」の 3 カテゴリを mixed fixture として新設する。測定項目 (EN 単語の英字出力 F1 / JP span の変換精度 / space handling 正答率) も追加する。

## Alternatives considered

以下 6 案を検討し、いずれも棄却または deferral とした。

### A. jinen-v1-small (Karukan と同一モデル) 流用

**Rejected.** Karukan で観察した前処理層起因の不便 (`s` / `sh` / `si` / backspace / typo) は jinen-v1-small を流用しても解消しない。romaji-base 境界への移行には再学習が必須であり、jinen 流用は fine-tune データ設計の自由度を奪う割に利得が少ない。

### B. Gemma-2-2B-jpn-it の LoRA fine-tune

**Rejected.** LoRA adapter 本体は小さいが、inference 時に base model (2.6B / 1.92GB) の同時 load が必要であり、IME 常駐用途の size budget を満たさない。

### C. Gemma-4-31B-it の Japanese specialization

**Rejected.** 31B parameter の GGUF は 18GB 前後であり、Phase 1 の size budget (1.92GB) の 9 倍を超える。IME 常駐不可能。Phase 5 の teacher 候補としては検討対象となる (D3 参照)。

### D. row 3 類を長期既知制約として受容し、Phase 5 を旧 Phase 5 (Advanced features) 実装に使う

**Rejected.** C1 で実証したように、汎用 instruction-following LLM の ICL では row 3 類の synonym bias を prompt だけで覆せない。Phase 6 (旧 Phase 5) の typo correction / context reranking を実装しても、根本の convert layer が Gemma のままでは「あした → 翌日」問題は残存する。

### E. Tier 2 採用 (Shift → EN sticky mode) + Phase 0 ADR 0002 の InputMode 拡張

Phase 5 custom model で Tier 1 (D8 の language-context detection) が empirical に達成困難と判明した場合の fallback として、新 variant `InputMode::Latin` を Phase 0 input state machine に追加する案である。ADR 0002 が決定した「Transient vs Sticky」の判断を拡張し、Shift 起動で Latin に入り明示的に Hiragana へ戻るまで EN 入力を継続する Sticky 挙動を採用する。行内 EN span (C4) と行末 Transient (ADR 0002) の両要件を共存させる必要から、Direct (Phase 0 で実装済、行末 Transient 専用) と Latin (新規、行内 Sticky 専用) は別モードとして分離する。

**Status**: Tier 1 (Phase 5 model auto-detect) の empirical 検証後に判断する。Tier 1 成功なら本 Alternative E は不要、Tier 1 失敗なら本 Alternative E を ADR 0015 で正式採用する。**現時点では Rejected**、Phase 5 model の学習可能性を優先検証する。

### F. Dictionary (Phase 2) のみで英語 loanword を辞書 lookup 解決

Phase 2 (ADR 0014) の Sudachi-based dictionary に英語 loanword entry を追加し、`commit` / `pull request` 等を辞書 hit のみで EN 出力する案である。

**Rejected.** 文脈判定が辞書単体では困難である。具体的には「commit」という romaji 入力は動詞の「コミット」(カタカナ) と区別できず、context 情報を持たない lookup では誤変換リスクが大きい。また「次の review を pull する」のような multi-word 英語表現は phrase-level の学習が必要であり、辞書 entry 量が組合せ爆発する。Phase 5 の context-aware model が本問題の正攻法である。ただし Phase 2 dictionary に高頻度 loanword (例: `api` / `commit` / `issue` / `pr` などプログラマ頻出 100 語規模) を限定的に登録することで、Phase 5 完成前の UX を部分改善する検討余地は残す (別 ISSUE で扱う)。

## Related documents

- Parent empirical record: `docs/wbs/2026-04-24-feature-75-prompt-optimization-15-row-fixture.md` (PR #76 merge commit `3eccaa1`)
- Phase 1 design spec: `docs/superpowers/specs/2026-04-24-kotoha-phase-1-design.md` §3.2 / §3.3 / §6 の Phase 1 acceptance 14/15 記述
- Phase 5 design spec (stub): `docs/superpowers/specs/2026-04-25-kotoha-phase-5-custom-model.md`
- ROADMAP restructure 反映先: `docs/ROADMAP.md` Phase 一覧テーブル + Phase 5 マイルストーン分割節
- 先行 ADR 0009 (Gemma-2-2B-jpn-it pivot prep note): `docs/adr/0009-kanji-backend-model-selection-prep.md`
- Phase 0 ADR 0002 (input mode Transient vs Sticky、Alternative E `InputMode::Latin` の基盤): `docs/adr/0002-input-mode-transient-vs-sticky.md`
- Phase 5 spec の mixed JP/EN 対応節: `docs/superpowers/specs/2026-04-25-kotoha-phase-5-custom-model.md` §1.5 / §3.5 / §4.7 / §8 Q7 (本 ADR C4 / D8 の spec 側対応)

## Note: Phase 3 呼称訂正

PR #76 で既に develop に merge 済みの WBS (`docs/wbs/2026-04-24-feature-75-prompt-optimization-15-row-fixture.md`) 内には「Phase 3 への引き継ぎ」「Phase 3 deferral」等の記述が残存しているが、本 ADR の ROADMAP restructure 確定により、これら参照先は Phase 5 に訂正されるべきである。ISSUE #77 のスコープ外として、別 follow-up commit で訂正する。
