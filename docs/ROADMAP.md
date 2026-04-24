# Kotoha Roadmap

Kotoha プロジェクトの開発フェーズと、各フェーズの到達目標を記録する。

## Phase 一覧

ADR 0010 (`docs/adr/0010-kotoha-custom-romaji-base-model.md`) の決定により、2026-04-25 に旧 Phase 5 (Advanced features) を Phase 6 に、旧 Phase 6 (UX polish) を Phase 7 に後ろ倒しし、新 Phase 5「Kotoha custom romaji-base model」を挿入した。

| Phase | 名称 | 内容 | 状態 |
|---|---|---|---|
| 0 | Foundation | Cargo workspace + ローマ字→かな変換 + 入力モード管理 + CLI | 完了 |
| 1 | Kana→Kanji conversion | llama.cpp + Gemma-2-2B-jpn-it baseline によるかな→漢字変換 (P1-2.5 follow-up で Layer 3 smoke 14/15 達成) | 進行中 (P1-2.5 follow-up 完了) |
| 2 | Dictionary and learning | システム辞書 + ユーザ辞書 + 学習キャッシュ | 未着手 |
| 3 | IBus integration | IBus engine(GNOME Mutter 用) | 未着手 |
| 4 | fcitx5 integration | fcitx5 addon(KDE / wlroots 用) | 未着手 |
| 5 | **Kotoha custom romaji-base model** | raw romaji keystrokes を直接受理する Kotoha 専用 90〜180M parameter モデルの自作 (data pipeline + training + GGUF 推論統合 + evaluation)。row 3 類の ICL 限界 + typo robustness + partial-input 対応を同時解決する | 未着手 |
| 6 | Advanced features | タイポ訂正 + 文脈リランキング (Phase 5 の romaji-base モデルを技術基盤とする) | 未着手 |
| 7 | UX polish | 設定 UI + 辞書自動更新 + 同期 | 未着手 |

## Phase 0 マイルストーン

Phase 0 の詳細マイルストーン分割は `docs/superpowers/plans/2026-04-22-kotoha-phase-0-implementation.md` を参照。

## Phase 0 → Phase 1 への申し送り

Phase 0 完了時に Phase 1 へ引き継ぐ設計判断事項を記録する。

- **canonical romaji ADR 判定済み** (`docs/adr/0008-canonical-romaji-and-partial-invertibility.md`): 2026-04-24 の Phase 1 kick-off で選択肢 1 (canonical romaji を定義しない、現状維持) の採用を決定。本 ADR は「承認」ステータスに昇格済み。将来 canonical romaji が必要となる use case が発生した場合は、display layer で canonical 化するか、新 ADR を起こして選択肢 2 / 3 に移行する。

## Phase 1 → Phase 2 への申し送り

Phase 1 P1-2.5 follow-up (PR #76 / ISSUE #75 / merge commit `3eccaa1`) で、Gemma-2-2B-jpn-it Q5_K_M の Layer 3 smoke fixture 15 行のうち 14 行を PASS、1 行 (row 3「あした → 明日」) のみ FAIL (「翌日」出力) で close した。row 3 の FAIL は v5〜v12 の 8 世代 prompt iteration で解消不能であり、Gemma-2-2B-jpn-it の 2B parameter instruction-tuning における in-context learning (ICL) 限界として Phase 1 は 14/15 を受容した。根本解消は Phase 5「Kotoha custom romaji-base model」で task-specific fine-tune モデルにより達成する方針を ADR 0010 に記録した。Phase 2 (Dictionary and learning) 着手時には、Gemma baseline 14/15 を前提として Dictionary / 学習キャッシュ設計を進める。

## Phase 5 マイルストーン分割

Phase 5「Kotoha custom romaji-base model」は 4 milestone に分割する。詳細は `docs/superpowers/specs/2026-04-25-kotoha-phase-5-custom-model.md` および ADR 0010 を参照。各 milestone の exact スコープは Phase 4 完了時の Phase 5 kick-off で確定する。

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
- B2: LoRA fine-tune (Gemma-2-2B-jpn-it adapter、inference 時 base 必要のため候補から除外推奨)
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
- 旧 Phase 5 の「Shift 挙動設定」は、設計書 revision 2 の判断により Phase 0 に前倒し済み。新 Phase 6 の内容は「タイポ訂正 + 文脈リランキング」のみ
- 各 Phase の設計書は `docs/superpowers/specs/` に配置する
