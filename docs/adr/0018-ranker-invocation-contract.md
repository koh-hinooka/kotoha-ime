# ADR 0018: Ranker invocation contract

## Status

Accepted (2026-05-02)

## Context

Phase 3-A engine が `Ranker::rank()` を呼ぶ contract と、その背景の
`RankerWorker` thread / coalescing / cancel propagation 規約を確定する必要があった。

## Decision

- **同期 `rank()` + 内部 thread dispatch**: `Ranker::rank()` は同期に return し、
  heavy lifting は impl 内部で thread / async に dispatch する。engine 主 thread は
  blocking しない。`mpsc::Sender<RankerOutput>` は impl 側に move される。
- **Coalescing window**: typing 中(`Live`)は 7ms primary、space 後(`Commit`)は
  30ms primary + 150ms secondary の 2 段 window で候補を集約する。値の細部は
  Phase 3-A 実装段階で empirical 調整する(spec §13 Open Q 2、ADR 改訂不要)。
- **request_id mismatch discard**: engine 採番の単調増加 64-bit ID で stale
  response を識別し、worker レベル(fast path)+ engine 主 thread レベル
  (safety net)の両側で discard する(layered defense、spec §7.5)。
- **Cancel propagation**: `CancellationToken` を `Arc<dyn>` で 5 trigger
  (focus_out / preedit edit / 連続 space / Esc / `IMEEngine::reset`)から fire し、
  backend 別 latency は SudachiDict 即 / UserVocab 即 / LearningCache 即 / LLM
  10 token check を許容する(spec §8.2)。

## Consequences

- `HybridRanker` impl はその内部で更に下位 thread / async を起動する自由度を持つ
  (P2-D で `std::thread::spawn` ベースの parallel backend dispatch を採用)。
- coalescing 値は実装段階で empirical に確定(spec §13 Open Q 2)、本 ADR は
  「2 段 window 構造」を凍結し、定数値の調整は ADR 改訂不要とする。
- LLM cancel 漏れ token は計算 cost が無駄になるが UX 影響なしとして許容する。
- Phase 3-A engine の coalescing window 細部は `LIVE_WINDOW` / `COMMIT_WINDOW`
  / `COMMIT_SECOND_WINDOW` constants(`crates/kotoha-engine-core/src/engine/worker.rs`)
  を tweak することで調整可能。

## References

- Phase 3-A spec `docs/superpowers/specs/2026-05-02-p3-a-ibus-engine-design.md` §4.3 / §7 / §8
- ADR 0011: KanjiBackend の sync API 前例(下位 async 持ち込みなし)
- ADR 0017: 2 trait port + async runtime 非依存(本 ADR は §7 / §8 規約を補完)
