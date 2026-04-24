# ADR 0013 — Phase 1 latency target (relax spec §8.3 30s goal)

- **Status**: Accepted (2026-04-25)
- **Date**: 2026-04-25
- **Deciders**: Kotoha Phase 1 maintainers

## Context

Kotoha Phase 1 spec §8.3 (Layer 3 llama.cpp smoke) は、実推論を伴う smoke test の所要時間の目標として「約 30 秒」を記述してきた。当初の見積もりは「5 cases × 平均 6 秒 = 30 秒」という Phase 1 早期 planning 時点の naïve estimate であり、empirical data が揃う前の暫定値であった。

P1-2.5 の empirical verification (WBS `docs/wbs/2026-04-24-feature-73-llama-cpp-backend-gemma-2-jpn.md`) で、実 model (Gemma-2-2B-jpn-it Q5_K_M、1.92 GB) を使った smoke の所要時間内訳が以下と判明した。

- **Cold load**: 約 10.6 秒 (GGUF mmap + tokenizer 初期化 + KV cache 確保)
- **Warm inference**: 平均約 3 秒 / case
- **9 行 fixture 時代 (P1-2.5 本体)**: 約 30-49 秒 (`cargo test --test-threads=1` での実測レンジ)

続く P1-2.5 follow-up (PR #76、WBS `docs/wbs/2026-04-24-feature-75-prompt-optimization-15-row-fixture.md`) で fixture を 15 行に復元した際、14/15 PASS を達成した実測の概算は以下である。

- **Cold load**: 約 10.6 秒 (fixture 変更で不変)
- **Warm inference × 14 行** (row 3 skip 相当、実際には FAIL として 1 回は推論): 約 3 秒 × 14 = 約 42 秒
- **合計**: 約 52 秒

すなわち、empirical 実測値は spec §8.3 当初の 30 秒 target を超過している。当該 target は Phase 1 の lefthook pre-push に smoke を含めない方針 (spec §8.3 ならびに ADR 0012 D4) のもとでは CI blocking ではないが、spec 上の未達成項目として残ると Phase 1 acceptance 判定を曖昧にする。

加えて、Phase 1 default model (Gemma-2-2B-jpn-it Q5_K_M、2B parameter、1.92 GB) の 3 秒 / case という warm inference は、IME のリアルタイム入力応答 (目標 p50 ≤ 30ms、Phase 5 spec §6.4) には桁違いに遅い。Phase 3 IBus 統合前に model 差替え (Phase 5 の Kotoha 専用 90〜180M parameter モデル、ADR 0010 参照) が必要である。

本 ADR は「Phase 1 の smoke latency を hard-gate しない」という consensus を記録する。

## Decision

本 ADR では以下 4 項目を決定する。

### D1. Phase 1 latency target を empirical 値ベースに書き直す

- 旧 target (spec §8.3 原文): 「約 30 秒 (5 cases 想定)」
- 新 target: 「opt-in smoke として実行可能で、cold load ~15 秒以内 / warm inference ≤ 5 秒/case を目安」とする
- 新 target は絶対値の上限ではなく目安であり、連続実行の合計時間を hard-gate しない
- 目安設定の根拠: Phase 1 default model (Gemma-2-2B-jpn-it Q5_K_M) の empirical 実測 (cold ~10.6s / warm ~3s/case) に対し、環境差 (CPU コア数 / メモリ速度 / 並列 thread 数) を見込んだ margin を付与した値である

### D2. lefthook pre-push に smoke を含めない方針 (spec §8.3、ADR 0012 D4) を維持する

本 ADR では smoke の実行契機を変更しない。smoke は `cargo test --features llama-cpp-smoke` の手動実行 + `KOTOHA_LLAMA_MODEL_PATH` 設定が前提であり、pre-push / CI の自動実行には組み込まない。目的は「pre-push cycle を数秒以内に保ち、開発速度を犠牲にしない」である。

### D3. Phase 2+ で latency optimization が必要になった時点で新 ADR を起こす

本 ADR は Phase 1 の consensus record であり、Phase 2 以降の latency 要求は別途議論する。Phase 3 (IBus 統合) 着手時点で「IME ユーザから見た入力→候補表示の応答時間」を測定し、目標値 (例: p50 ≤ 30ms) に届かない場合は新 ADR (0014+ の番号で) を起こして対応方針を決める。想定される対応は model 差替え (Phase 5 の軽量モデル) / KV cache 再利用 / mmap 戦略見直し等である。

### D4. 本 ADR は Phase 1 での「latency を受容し、妥協ではなく deferment として記録する」 consensus record とする

Phase 1 acceptance 判定時に「latency 不達成で未完了」と記載されるリスクを取り除く。Phase 1 は 14/15 PASS + latency 受容の 2 点で close する。

## Consequences

### 正の帰結

- Phase 1 Layer 3 smoke が「時間がかかっても pass」で close 可能となり、P1-4 (Phase 1 最終 wrap) を blocking しない
- Phase 5 (Kotoha custom 90M〜180M model、Q5_K_M で ≤ 200MB 目標、ADR 0010 D6) 完了時点で、model size 比が約 10 分の 1 になることにより inference 速度も大幅改善が期待される。本 ADR の relaxation はこの改善を取り込むまでの bridge である
- spec §8.3 の実測レンジと実装実態の乖離が解消され、contributor が spec を信頼して読める状態になる

### 負の帰結

- 現在の Gemma-2-2B-jpn-it baseline では IME のリアルタイム応答性 (目標 p50 ≤ 30ms、Phase 5 spec §6.4) には到達不能であり、Phase 3 IBus 統合の前に model 差替えが必要となる。本 ADR は「差替えまで Phase 1〜Phase 4 は baseline で動作させる」ことを暗黙に認めるが、Phase 3 着手時の追加 ADR 起票 (D3) を忘れないよう本 ADR を参照点として残す
- latency 劣化の regression を検出する CI gate が存在しないため、Phase 2+ の refactor で不意に latency が倍化しても気づかない可能性がある。対策として WBS への smoke 実測値記録を継続するが、恒久的 regression gate は Phase 3 以降で別途検討する
- smoke の実行時間が 1 分前後になると、contributor が smoke 実行を敬遠する risk があり、Layer 3 の empirical カバレッジが低下する可能性がある。README / CONTRIBUTING に「smoke は手動 opt-in、PR 投稿前に最低 1 回は実行を推奨」という運用規則を明示することで緩和する (Phase 1 P1-4 完了後の別 ISSUE として個別に検討対象となる)

## Alternatives considered

以下 3 案を検討し、いずれも棄却した。

### A. 30 秒厳守を維持し、Gemma-2-2B-jpn-it を軽量モデル (jinen-v1 or Gemma-3-1B) に差替える

**Rejected.** ADR 0009 (Phase 1 default model selection) C2 に記録した 3-way empirical 比較では、Gemma-2-2B-jpn-it が 14/15 を達成する唯一のモデルである。Gemma-3-1B-it は 2/5 (hallucination 発生)、Qwen2.5-1.5B-Instruct は 4/5 (敬称失敗) であり、軽量モデルへの差替えで latency を短縮しても conversion quality が落ちて Phase 1 acceptance (14/15 PASS) を失う。quality と latency を同時に満たすモデルは Phase 1 の empirical 範囲には存在しなかった。

### B. latency target を完全撤廃し制約を設けない

**Rejected.** Phase 5+ で再議論する際の起点を失う。本 ADR は「Phase 1 は latency を hard-gate しないが、Phase 2+ で新 ADR を起こして具体値を設定する」という明示的な継続議論の宣言を含むため、完全撤廃は棄却する。ADR としての role は「将来の議論への bridge を残す」である。

### C. 5 cases のみを smoke scope に限定し、30 秒を維持する

**Rejected.** ISSUE #75 で 15 行 fixture を正式採用した (WBS `2026-04-24-feature-75-prompt-optimization-15-row-fixture.md`)。5 行に削減すると quality evaluation のサンプル数が 1/3 になり、Gemma-2-2B-jpn-it の Phase 1 ICL 限界 (row 3 等) の empirical record が薄くなる。quality coverage の後退を伴う latency 維持は品質低下として棄却する。

## Related documents

- Phase 1 spec §8.3 (Layer 3 llama.cpp smoke の latency 記述): `docs/superpowers/specs/2026-04-24-kotoha-phase-1-design.md`
- Empirical latency (9-row baseline): `docs/wbs/2026-04-24-feature-73-llama-cpp-backend-gemma-2-jpn.md`
- Empirical latency (14/15 final): `docs/wbs/2026-04-24-feature-75-prompt-optimization-15-row-fixture.md`
- Phase 1 default model 選定: ADR 0009 (`docs/adr/0009-phase-1-default-model-selection.md`)
- Phase 5 latency 目標 (p50 ≤ 30ms / p99 ≤ 100ms): ADR 0010 (`docs/adr/0010-kotoha-custom-romaji-base-model.md`) D6 および `docs/superpowers/specs/2026-04-25-kotoha-phase-5-custom-model.md` §6.4
- feature flag による smoke opt-in 設計: ADR 0012 (`docs/adr/0012-feature-flag-design-for-llama-cpp.md`)
