# Kotoha Roadmap

Kotoha プロジェクトの開発フェーズと、各フェーズの到達目標を記録する。

> **規約**: 本 ROADMAP は **常に 1 つ以上の Active マイルストーン** を持つ (Active 0 状態は禁止)。詳細: `~/.claude/CLAUDE.md` §Milestone Specification

## Active マイルストーン

### v0.3.0 — Phase 3 IBus integration

**目標日**: 2026-08-31
**開始日**: 2026-05-02

#### 含まれる ISSUE と spec

| ISSUE | spec | 状態 |
|---|---|---|
| [#174](https://github.com/std-koh-hinooka/kotoha-ime/issues/174) docs 構造改修と Obsidian vault 連携 | `docs/specs/_uncategorized/*` (8 spec frontmatter 整備) | [x] |
| [#149](https://github.com/std-koh-hinooka/kotoha-ime/issues/149) P3-B B0h-f: I3 dispatch_rank_request async-ification | `docs/specs/_uncategorized/p3-a-ibus-engine.md` | [ ] |
| [#136](https://github.com/std-koh-hinooka/kotoha-ime/issues/136) P3-B B3 + B6: signal listener loop + L3 manual smoke | `docs/specs/_uncategorized/p3-a-ibus-engine.md` | [ ] |

<!--
更新ルール: ~/.claude/CLAUDE.md §post-merge follow-up checklist 参照
- 機能完了 PR merge 後: 該当行の `[ ]` を `[x]` に更新
- 仕様変更 (scope/粒度/遅延/前倒し/キャンセル): 該当行を直接編集
- ISSUE/spec 追加: 行追加 (PR で commit)
- 全行 [x] 達成: §Milestone Specification の完了条件 を実施し本セクションを「完了済」へ mv
-->

## Phase 一覧

ADR 0010 (`docs/adr/0010-kotoha-custom-romaji-base-model.md`) の決定により、2026-04-25 に旧 Phase 5 (Advanced features) を Phase 6 に、旧 Phase 6 (UX polish) を Phase 7 に後ろ倒しし、新 Phase 5「Kotoha custom romaji-base model」を挿入した。

| Phase | 名称 | 内容 | 状態 |
|---|---|---|---|
| 0 | Foundation | Cargo workspace + ローマ字→かな変換 + 入力モード管理 + CLI | 完了 |
| 1 | Kana→Kanji conversion | llama.cpp + Gemma-2-2B-jpn-it baseline によるかな→漢字変換 (P1-2.5 follow-up で Layer 3 smoke 14/15 達成) | 完了 (14/15 PASS, P1-4 で close 2026-04-25) |
| 2 | Dictionary and learning | システム辞書 + ユーザ辞書 + 学習キャッシュ + Hybrid Ranker | 完了 (P2-A〜P2-D 全 PR merge、最終 commit `be0fae9` 2026-05-02、test 416 PASS。follow-up backlog #92/#94/#100/#107 は OSS 公開前 triage) |
| 3 | IBus integration | IBus engine(GNOME Mutter 用) | **進行中** (P3-A draft + P3-B B0/B0g/B0h 大部分/B1/B2/B4/B5 完了。残 B0h-f I3 async dispatch + B3 signal listener loop + B6 L3 manual smoke) |
| 4 | fcitx5 integration | fcitx5 addon(KDE / wlroots 用) | 未着手 |
| 5 | **Kotoha custom romaji-base model** | raw romaji keystrokes を直接受理する Kotoha 専用 90〜180M parameter モデルの自作 (data pipeline + training + GGUF 推論統合 + evaluation)。row 3 類の ICL 限界 + typo robustness + partial-input 対応を同時解決する | 未着手 |
| 6 | Advanced features | タイポ訂正 + 文脈リランキング (Phase 5 の romaji-base モデルを技術基盤とする) | 未着手 |
| 7 | UX polish | 設定 UI + 辞書自動更新 + 同期 | 未着手 |

## Phase 0 マイルストーン

Phase 0 の詳細マイルストーン分割は `docs/plans/2026-04-22-kotoha-phase-0-implementation.md` を参照。

## Phase 0 → Phase 1 への申し送り

Phase 0 完了時に Phase 1 へ引き継ぐ設計判断事項を記録する。

- **canonical romaji ADR 判定済み** (`docs/adr/0008-canonical-romaji-and-partial-invertibility.md`): 2026-04-24 の Phase 1 kick-off で選択肢 1 (canonical romaji を定義しない、現状維持) の採用を決定。本 ADR は「承認」ステータスに昇格済み。将来 canonical romaji が必要となる use case が発生した場合は、display layer で canonical 化するか、新 ADR を起こして選択肢 2 / 3 に移行する。

## Phase 1 → Phase 2 への申し送り

Phase 1 P1-2.5 follow-up (PR #76 / ISSUE #75 / merge commit `3eccaa1`) で、Gemma-2-2B-jpn-it Q5_K_M の Layer 3 smoke fixture 15 行のうち 14 行を PASS、1 行 (row 3「あした → 明日」) のみ FAIL (「翌日」出力) で close した。row 3 の FAIL は v5〜v12 の 8 世代 prompt iteration で解消不能であり、Gemma-2-2B-jpn-it の 2B parameter instruction-tuning における in-context learning (ICL) 限界として Phase 1 は 14/15 を受容した。根本解消は Phase 5「Kotoha custom romaji-base model」で task-specific fine-tune モデルにより達成する方針を ADR 0010 に記録した。Phase 2 (Dictionary and learning) 着手時には、Gemma baseline 14/15 を前提として Dictionary / 学習キャッシュ設計を進める。

P1-4 (PR: 本 ISSUE #83) で ADR 0009 を正式化 (rename + Status: Accepted)、ADR 0011 (backend trait design) / ADR 0012 (feature flag design) / ADR 0013 (latency target relaxation) を新規起票、spec §14.1 で Phase 1 完了宣言を記録した。Phase 2 (Dictionary and learning) 着手時には、Gemma-2-2B-jpn-it 14/15 baseline + Backend trait の既存拡張点を前提として、辞書検索 + 学習キャッシュを追加する BackendConfig variant を設計する。

P1-4 (PR #84, merge `95e7df9`) での Phase 1 close 後、Phase 2 foundation docs (本 ISSUE #85) を整備した。ADR 0014 (dictionary layer architecture) と Phase 2 spec draft を起票し、P2-A kick-off で SudachiDict-core を実装対象として確定する。Phase 5 (ADR 0010 custom model) は Phase 2 の dictionary / learning cache / BackendConfig 拡張を継承して自作モデルに置換する予定 (Phase 5 spec §3.3 との整合を Phase 2 spec §9 Open Q6 で明示)。

## Phase 2 マイルストーン分割

Phase 2「Dictionary and learning」は 4 milestone に分割する。詳細は `docs/specs/_uncategorized/kotoha-phase-2.md` および ADR 0014 を参照。各 milestone の exact スコープは P2-A kick-off で確定する。

### P2-A: Dictionary layer (完了、PR #97 / #99)

- SudachiDict-core を runtime load する DictionaryBackend 実装
- Kotoha 独自語彙 (敬称、IME 固有) の merge
- Dictionary lookup の unit test / golden fixture (敬称 / 固有名詞 30+ cases)
- BackendConfig 新 variant 追加 (ADR 0011 の non_exhaustive 拡張方針)
- follow-up backlog: #92 (Layer 3 golden 530 cases) / #94 (P2-A 25+ findings triage) / #100 (10 hardening deferral)

### P2-B: User dictionary (完了、PR #98 / #102 / #104)

- User dict entry 追加 / 削除 / 列挙 / 詳細取得の API
- SQLite 永続化 (`kotoha-storage` 新 crate、共用 DB `kotoha.db`、ADR 0015 / P2-B spec `docs/specs/_uncategorized/p2-b-user-dictionary.md` 参照)
- CLI サブコマンド `kotoha-dict {add, remove, list, show}` の実装 (`update` / `import` / `export` / `init` は Phase 6+ で扱う、P2-B spec §3.8 参照)

### P2-C: Learning cache (完了、PR #106 + hardening #109/#111/#113/#115)

- in-memory + SQLite 永続(P2-B 同 DB に置き、同 Mutex<Connection> を共有)
- data model: (kana_input, chosen_kanji, frequency, last_used_at)
- 4 trait ISP split (`LearningCacheReader` / `LearningCacheWriter` / `UserVocabReader` / `UserVocabWriter`)
- v002 migration + test-helpers feature gate
- prerequisite hardening: bounded `usize as i64` (#109) / `Mutex` poison recovery (#111) / `MockLearningCacheStore` decoupling (#113) / `prepare_cached` perf (#115)
- follow-up backlog: #107 (Medium / Low review residuals)

### P2-D: Integration + rerank (完了、PR #117〜#127)

- hybrid backend (dict + LLM) の factory 実装(`kotoha-engine-core` crate に新 `HybridRanker` 配置、後に B0h-b で `kotoha-ranker-hybrid` crate に分離)
- 候補 merge / dedupe / rerank (初期重み dict=0.95 / LLM=1.0)
- regression test: Phase 1 14/15 が保たれること
- final test count baseline: 416 PASS / 0 FAIL

## Phase 3 マイルストーン分割

Phase 3「IBus integration」は P3-A(設計 + skeleton)と P3-B(production wiring + L3 manual smoke)の 2 段階に分割する。詳細は `docs/specs/_uncategorized/p3-a-ibus-engine.md` および `docs/wbs/2026-05-02-phase3a-implementation.md` を参照。Phase 1 / Phase 2 と異なり、P3-A は「draft 完成」段階で 2 度の包括レビューにより機能完成宣言を取り下げ、P3-B B0g(silent-failure / security / testing residuals)+ B0h(architectural rework)で resolved した経緯を持つ。

### P3-A: Design + skeleton (完了 draft、ISSUE #128 / PR #129〜#135)

- `IMEEngine` / `IMEHostBridge` / `Ranker` の port trait 定義(spec §4)
- `KotohaEngine` state machine(Idle / LiveConverting / CommitConverting / CandidatesShown、spec §5.2 表)
- `RankerWorker` background thread + dual coalescing windows(typing 5ms / commit 30ms)
- `IBusEventDispatcher` + `IBusHostBridge` skeleton(zbus 5 binding、proxy.rs は B0f まで silent stub、後に fail-loud 化、B2 で実 emit)
- `kotoha-bin` 起動エントリ
- ISSUE #128 は ADR 0016〜0018 候補の deferral と共に close(B0/B0f で機能完成宣言を取り下げ、B0g/B0h で resolved)

### P3-B: Production wiring + L3 manual smoke (進行中、ISSUE #136 tracking)

| Sub-milestone | 内容 | 状態 |
|---|---|---|
| B0 (#140) | 第 1 回包括レビュー Critical 4 + Important 9 消化 | 完了(PR #141〜#145、test 416 → 473) |
| B0f (#146) | 機能完成宣言取り下げ + proxy / event loop fail-loud 化 | 完了(PR #147 squash `9602dff`) |
| B0g-a/b/c (#148) | silent-failure / security / testing residuals 消化(panic_message / lookup_table / catch_unwind / sanitize / theater fix 等) | 完了(PR #150〜#152) |
| B0h-a (#153) | C3 hexagonal driven port inversion(`learning_port` を engine-core に新設、`kotoha-engine-adapter` crate 切出し) | 完了(PR #154 squash `5c37d65`) |
| B0h-b (#155) | I4 `HybridRanker` を `kotoha-ranker-hybrid` 別 crate へ切出し | 完了(PR #156 squash `3cc36a9`) |
| B0h-c-i/ii/iii (#161/#163/#165) | I1 SRP 分割:`PreeditBuffer` / `CandidateBuffer` / `WorkerChannel` / `LearningSink` 抽出 | 完了(PR #162/#164/#166) |
| B0h-d (#157) | I2 dispatcher `Arc<Mutex<dyn IMEEngine>>` 化(B3 multi-thread D-Bus listener の前提整備) | 完了(PR #158 squash `ba7dc6d`) |
| B0h-e (#159) | I5 stub fallback feature gate(release default で stub symbol 0 link、`KOTOHA_ALLOW_STUB=1` exit 1) | 完了(PR #160 squash `7b58cbc`) |
| B1 (#137) | kotoha-core 側 `Arc<dyn MorphologicalEngine + Send + Sync>` 返す production API + kotoha-bin の `StubRanker` を実 `HybridRanker` に置換 | 完了(PR #137) |
| B4 (#138) | `KeyModifiers` の IBus full mapping(`IBusModifierType` 全列挙)+ RELEASE event handling | 完了(PR #138) |
| B5 (#139) | KotohaEngine + 実 `HybridRanker` end-to-end integration test | 完了(PR #139) |
| **B2 (#170)** | IBus signal body marshalling(`proxy.rs` 5 method を実 D-Bus signal emit に置換、`IBusText` / `IBusLookupTable` wire-format 確定:`(sa{sv}sv)` / `(sa{sv}uubbiavav)`) | **完了**(PR #171 squash `4731c50`、test 536 / 547) |
| **B0h-f (#149 残)** | I3 `dispatch_rank_request` async-ification(`drain_events_blocking` 撤去 + wakeup channel) | **未着手**(spec §6.1 / §7 ADR 必須、B3 と密接で一体化推奨) |
| **B3 (#136 残)** | `IBusEventDispatcher` の `zbus::blocking::MessageStream` 経由 signal listener loop(`kotoha-bin::run_ibus()` の `anyhow::bail!` 置換) | **未着手**(B0h-f と一体化、event loop integration) |
| **B6 (#136 残)** | L3 manual smoke on GNOME Wayland(Firefox / GNOME Text Editor / VS Code で典型変換 10 件) | **未着手**(B2 / B3 完了後、`docs/wbs/` に追記) |

### Phase 3 受け入れ基準

- 実機(GNOME Wayland)で `cargo run --bin kotoha` 起動が IBus daemon に登録される
- typical 10 件の Layer 3 manual smoke が PASS
- coalescing window 値の empirical 確定(spec §13 Open Q 2)、必要なら ADR 追補
- 既存 baseline(現状 536 default / 547 test-helpers)を 0 regression
- Phase 1 14/15 regression が engine 経由でも保たれる

## Phase 5 マイルストーン分割

Phase 5「Kotoha custom romaji-base model」は 4 milestone に分割する。詳細は `docs/specs/_uncategorized/kotoha-phase-5-custom-model.md` および ADR 0010 を参照。各 milestone の exact スコープは Phase 4 完了時の Phase 5 kick-off で確定する。

### P5-A: Data pipeline (工数目安 2〜4 週)

Data pipeline は kana→kanji ペアの大規模コーパス構築を担う。以下を含む。

- コーパス source からの日本語テキスト抽出 (LLM-JP Corpus / CC-100 Japanese / Wikipedia JP のライセンス確認 + 抽出スクリプト)
- MeCab 形態素解析による kana→kanji ペア生成
- kana→romaji 拡張 (Hepburn / Kunrei / waapuro の 3 方式並立展開)
- typo 注入 (edit distance 1〜3、隣接キー置換 / 転倒 / 欠落 / 余剰)
- partial-input 生成 (ランダム truncate による prefix 集合)
- special tokens 設計確定 (`<ctx>` / `<romaji>` / `<out>` / `<eos>` + 拡張候補)
- データ scale 目標: 1M〜10M ペア (B1 scratch では 10M+、B3 distillation では 1M〜3M)

### P5-B: Training (工数目安 1〜2 週 + GPU 数日)

学習戦略候補 3 本の empirical 比較と default 決定を担う。

- B1: scratch training (GPT-2 Small 90M、random init、RTX 4090 で 1〜3 GPU-day)
- B2: LoRA fine-tune (Gemma-2-2B-jpn-it adapter、inference 時 base 必要のため候補から除外推奨。offline batch 用途での再検討余地あり。詳細は Phase 5 spec §5.2)
- B3: distillation (Gemma-4-31B-it 等を teacher、90〜180M student、数 GPU-day、**default 推奨**)
- 比較計画: B1 と B3 を同一評価 fixture で走らせ、row 3 解消率 / typo robustness / latency の 3 軸で判定する

### P5-C: Integration (工数目安 1〜2 週)

学習済みモデルの Kotoha 推論経路統合を担う。

- GGUF Q5_K_M 量子化 (目標 size ≤ 200MB)
- `BackendConfig::KotohaNative { model_path, tokenizer_path }` variant 追加 (Phase 1 `LlamaCpp` と並立)
- 外部 HuggingFace tokenizer (`tokenizers` crate) の統合
- partial-input 対応 beam search
- typo 対応 top-k 候補生成
- context 注入 (周辺テキスト)
- Sudachi 辞書 fallback (OOV 語彙、後段併用)

### P5-D: Evaluation and iteration (継続)

以下の評価 set を実装し、継続的に iterate する。

- Layer 3 smoke 15-row (Phase 1 fixture との regression 確認)
- 200-row regression set (Phase 5 専用に拡張)
- 10k-row golden set (BLEU / exact-match metric)
- typo robustness set (意図的 typo 100 件、edit distance 1〜3)
- partial-input set (prefix top-k 評価)
- latency SLA (p50 ≤ 30ms, p99 ≤ 100ms on CPU)

## 注記

- Phase 5 への restructure (ADR 0010) により、旧 Phase 5 (Advanced features) は Phase 6 へ、旧 Phase 6 (UX polish) は Phase 7 へ後ろ倒しされた
- 旧 Phase 5 の「Shift 挙動設定」は、Phase 0 設計書 (`docs/specs/_uncategorized/kotoha-phase-0.md`) revision 2 の判断により Phase 0 に前倒し済み。新 Phase 6 の内容は「タイポ訂正 + 文脈リランキング」のみ
- 各 Phase の設計書は `docs/specs/_uncategorized/` に配置する

## 完了済 (SemVer マッピング)

### v0.0.0 — Phase 0 Foundation (完了 2026-04-23)

| ISSUE | spec | 状態 |
|---|---|---|
| [#1](https://github.com/std-koh-hinooka/kotoha-ime/issues/1) Project setup + Phase 0 implementation | `docs/specs/_uncategorized/kotoha-phase-0.md` | [x] |

### v0.1.0 — Phase 1 Kana→Kanji conversion (完了 2026-04-25 P1-4 close)

| ISSUE | spec | 状態 |
|---|---|---|
| [#75](https://github.com/std-koh-hinooka/kotoha-ime/issues/75) P1-2.5 follow-up: Layer 3 smoke 14/15 達成 | `docs/specs/_uncategorized/kotoha-phase-1.md` | [x] |
| [#83](https://github.com/std-koh-hinooka/kotoha-ime/issues/83) P1-4 phase1 wrap (ADR 0011〜0013 起票) | `docs/specs/_uncategorized/kotoha-phase-1.md` | [x] |

### v0.2.0 — Phase 2 Dictionary and learning (完了 2026-05-02、test 416 PASS、`be0fae9`)

| ISSUE | spec | 状態 |
|---|---|---|
| [#85](https://github.com/std-koh-hinooka/kotoha-ime/issues/85) Phase 2 foundation docs | `docs/specs/_uncategorized/kotoha-phase-2.md` | [x] |
| P2-A〜P2-D 全 milestone | `docs/specs/_uncategorized/p2-a-dictionary-layer.md`、`p2-b-user-dictionary.md`、`p2-c-learning-cache.md` | [x] |

<!--
注: v0.0.0 / v0.1.0 / v0.2.0 の遡及 SemVer mapping waiver は ADR 0019 §D 参照。
v0.3.0 以降は §Milestone Specification 完了条件 (annotated tag + リリース ADR) を厳密適用する。
-->

## 改訂履歴

| 日付 | 改訂内容 |
|------|----------|
| 2026-05-05 | docs 構造移行 (PR #174): Active マイルストーン v0.3.0 table 化、完了済 v0.0.0/v0.1.0/v0.2.0 を SemVer マッピングで table 化、spec 参照を新パス (`docs/specs/_uncategorized/`) に更新 |
| 2026-05-04 | Phase 2 を「完了」に、Phase 3 を「進行中」に更新。Phase 3 マイルストーン分割 section を新設(P3-A draft + P3-B B0/B0g/B0h-a〜e/B1/B2/B4/B5 完了、B0h-f / B3 / B6 残)。Phase 2 各 milestone entry に merge PR 番号を追記(ISSUE #172 / PR 後続) |
| 2026-04-25 | ADR 0010 で旧 Phase 5/6 を 6/7 に後ろ倒し、新 Phase 5「Kotoha custom romaji-base model」を挿入(初版) |
