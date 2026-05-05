---
feature: perf-19-trie-oncelock-cache
status: implemented
bounded_context: _uncategorized
related_issues: ["#19"]
related_prs: []
glossary_refs: []
last_reviewed: 2026-05-05
---

# 2026-04-23 perf/19-trie-oncelock-cache 実装ログ

> **Migration note**: 本 spec は `docs/wbs/2026-04-23-perf-19-trie-oncelock-cache.md` から spec 化 (Global CLAUDE.md §Documentation Structure 準拠の `docs/wbs/` 廃止対応、2026-05-05)。
> 元 WBS は実装ログ性質。14-section restructure は別 follow-up 起票予定 (各 project の既存 14-section follow-up issue と合流可能)。

## 元 WBS 内容 (実装ログ由来)


## 対象

- ISSUE: [#19 — perf: cache `romaji::Trie` in `OnceLock` to eliminate per-call rebuild](https://github.com/std-koh-hinooka/kotoha-ime/issues/19)
- PR: [#49 — perf(kotoha-core): cache Trie in OnceLock to eliminate per-call rebuild (#19)](https://github.com/std-koh-hinooka/kotoha-ime/pull/49)
- merge commit: `fb3c6fa`
- branch: `perf/19-trie-oncelock-cache` (merge 後削除)

## 実施内容

- `crates/kotoha-core/src/romaji/state.rs` に module-private な `fn global_trie() -> &'static Trie` を追加し、`static TRIE: OnceLock<Trie>` で process-wide に `Trie` を cache 化した。
- `StateMachine.trie` フィールドを `Trie` owned から `&'static Trie` 参照に変更し、`StateMachine::new()` は `Trie::from_rules()` を直接呼ばずに `global_trie()` を通じて cache 済み `Trie` を参照するようにした。
- `crates/kotoha-core/src/romaji/trie.rs` は非変更。同 file 内の constructor テストは引き続き `Trie::from_rules()` を直接呼び出し、constructor 自体の動作を検証する用途のため変更不要と判断した。
- `crates/kotoha-core/src/romaji/mod.rs` は非変更。`RomajiConverter::new()` と `::convert()` は内部で `StateMachine::new()` を経由するため、cache 効果を自動的に享受する。public API は完全に不変。
- cache 動作の regression 固定化のため、以下 2 つの pointer-equality テストを追加:
  - `global_trie_returns_same_instance_across_calls` — `std::ptr::from_ref` で連続 2 回の `global_trie()` 戻り値が同一アドレスであることを確認。
  - `state_machine_new_shares_trie_with_global_cache` — 2 つの `StateMachine::new()` インスタンスと `global_trie()` の戻り値が全て同一 `Trie` を参照することを確認。
- `docs/adr/0005-romaji-trie-over-hashmap.md` の「影響」>「パフォーマンスへの影響」節に、ISSUE #19 解消を記録する 1 行を追加した。per-call rebuild が deferred であった旨の元記述は残し、解消日付 (2026-04-23) を追記する形式を採用して ADR の履歴性を保った。

## 検証結果

pre-push hook と main-agent spot-check の両方で以下 5 コマンドが全て green:

| コマンド | 結果 |
|---------|------|
| `cargo build --workspace` | clean |
| `cargo test --workspace` | 102 lib + integration + doc tests 全て pass (baseline 51 + 2 新規 cache-coverage test で 53 に増加) |
| `cargo test -p kotoha-core --lib romaji` | 53 passed (baseline 51 + 2) |
| `cargo clippy --workspace --all-targets -- -D warnings` | warnings ゼロ |
| `cargo fmt --all --check` | diff なし |

## レビュー結果

Medium tier 5 dimensions (security / performance / architecture / testing / API-ergonomics) + `owasp-security` + `secrets-check` を実施。

- Critical: 0, High: 0, Medium: 0
- Low: 2 (将来的な multi-thread race 用 stress test の追加、function-local `static` の可視性トレードオフ — いずれも非ブロッキング)
- Informational: 3 (性能影響ゼロ確認、OWASP CLEAN、API-ergonomics CLEAN)
- `owasp-security`: CLEAN
- `secrets-check`: CLEAN

Low finding はサイズキャップ遵守のため本 PR では fix せず、必要に応じ follow-up ISSUE で対処する方針。

## つまずき

なし。pre-discovered facts が正確で、`Trie` の `Send + Sync` 性 (`HashMap<u8, Node>` + `Option<&'static str>`) と `Trie::from_rules()` の panic-free 性が事前分析の想定通りであった。`std::ptr::from_ref` は Rust 1.76+ で stable のため、`rust-version = 1.80` 環境で問題なく使用できた。

## パフォーマンス観察

定量計測は本 ISSUE のスコープ外としたが、`cargo test --workspace` の実行時間は baseline と誤差範囲内で有意差なし (いずれのスイートも 0.00-0.15s 域)。207 entries の rule 表では per-call rebuild コストが μs オーダーであり、本 cache 化の目的は Phase 3 IBus engine のキーストローク頻度 hot path での allocator 圧削減であるため、現段階の改善は test suite 時間には反映されない。実測ベンチマークは Phase 3 で IBus engine を実装する段階で別途計測する。

## 成果物リンク

- ISSUE: https://github.com/std-koh-hinooka/kotoha-ime/issues/19
- PR: https://github.com/std-koh-hinooka/kotoha-ime/pull/49
- merge commit: [fb3c6fa](https://github.com/std-koh-hinooka/kotoha-ime/commit/fb3c6fa)
- 関連 ADR: `docs/adr/0005-romaji-trie-over-hashmap.md`
