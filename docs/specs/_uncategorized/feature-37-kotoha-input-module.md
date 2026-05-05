---
feature: feature-37-kotoha-input-module
status: implemented
bounded_context: _uncategorized
related_issues: ["#37"]
related_prs: []
glossary_refs: []
last_reviewed: 2026-05-05
---

# M4b: input module — InputMode / ModeOrigin / InputContext / InputStep

> **Migration note**: 本 spec は `docs/wbs/2026-04-23-feature-37-kotoha-input-module.md` から spec 化 (Global CLAUDE.md §Documentation Structure 準拠の `docs/wbs/` 廃止対応、2026-05-05)。
> 元 WBS は実装ログ性質。14-section restructure は別 follow-up 起票予定 (各 project の既存 14-section follow-up issue と合流可能)。

## 元 WBS 内容 (実装ログ由来)


## 実施内容

- `crates/kotoha-core/src/input/mode.rs` — `InputMode` (pub) + `ModeOrigin` (pub(crate)) enum 定義、3 件の単体テスト
- `crates/kotoha-core/src/input/context.rs` — `InputContext` 状態機械 + `InputStep` enum、8 公開メソッド、21 件の単体テスト
- `crates/kotoha-core/src/input/mod.rs` — module 配線と pub re-export
- `crates/kotoha-core/src/lib.rs` — `pub mod input;` + `pub use input::{InputContext, InputMode, InputStep};`
- `crates/kotoha-core/src/romaji/mod.rs` — `RomajiConverter::normalize_pending` 公開メソッド追加 (streaming path の mid-stream normalize ギャップ対応)
- M4b 計 24 件の input unit test (21 context + 3 mode) + 1 件の新規 doctest (normalize_pending)

## コミット履歴 (TDD 構成 4 件 + owasp 低リスク指摘の fix 1 件)

| SHA | 種別 | 概要 |
|-----|------|------|
| `2682a7a` | feat | `input::mode` enum 定義 (InputMode pub, ModeOrigin pub(crate)) |
| `742083c` | feat | `RomajiConverter::normalize_pending` 公開メソッド追加 |
| `a86ee35` | feat | `input::context::InputContext` + `InputStep` 実装 |
| `1b8da89` | feat | crate root から `InputContext` / `InputStep` を re-export |
| `833eb77` | fix | `toggle_mode` Hiragana→Direct 遷移時に converter buffer を clear (owasp L-1) |

## レビュー結果

### owasp-security (Step 事前実施分)

| 深刻度 | 件数 |
|--------|------|
| Critical | 0 |
| High | 0 |
| Medium | 0 |
| Low | 2 |

- **L-1**: `toggle_mode` Hiragana→Direct で converter pending buffer を clear していない (set_mode と非対称)。
  - 対応: 本 PR 内で 1 行 fix として追加 commit (`833eb77`)。
- **L-2**: `InputContext.direct_buffer` に長さ上限がない。
  - 対応: Phase 0 では外部呼び出し経路が存在しないため非 blocker。Phase 3 IBus engine 着手時に対応するよう、follow-up ISSUE を起票した (#39)。

### agent-teams:team-review (Medium tier, 5 次元)

`CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS=1` フラグが未設定のため、parallel teammate spawn は利用不可。main agent が 5 次元 (security / performance / architecture / testing / accessibility-as-API-ergonomics) のチェックリストを順次適用した。

| 深刻度 | 件数 |
|--------|------|
| Critical | 0 |
| High | 0 |
| Medium | 0 |
| Low | 4 |

- **L-3** [architecture / API ergonomics]: `InputContext::preedit()` は Hiragana mode 時に空文字列を返す暫定実装。code 内 doc で Phase 3 対応を明示している。対応: acknowledge (Phase 3 で `RomajiConverter::pending(&self) -> &str` を追加する計画が既に本ログ末尾の「M4c への申し送り」に記載済)。
- **L-4** [performance]: `preedit()` が Direct mode 時に `direct_buffer` を clone する。Phase 0 の現状では hot path が存在せず非 blocker。対応: acknowledge (L-2 と併せて Phase 3 IBus engine 着手時に見直し)。
- **L-5** [testing]: `(Direct, Sticky)` での大文字 input を直接検証する test ケースが未整備 (plan では M4c の mode golden / property test に委譲)。対応: acknowledge (M4c で整備)。
- **L-6** [API ergonomics]: `commit()` は空 buffer と実空 commit を区別しない戻り値。spec §8.4 と整合。対応: acknowledge (spec 準拠)。

すべて Low のため本 PR 内での追加修正は不要と判断した。

### secrets-check

CLEAN。機械ツール (gitleaks / trufflehog) は環境に未 install のため、AWS/GitHub token パターン + 高 entropy 文字列の手動 grep を実施。Rust 識別子以外の hit は 0 件。PR 内に `.env` / 鍵 / 証明書系ファイルの追加もなし。

## main agent 検証コマンド (Step 6)

`cargo` 命令はメモリ制約に従い逐次実行。全 5 コマンドが exit 0。

- `cargo build --workspace` → PASS (`Finished dev profile ... in 0.03s`)
- `cargo test --workspace` → PASS (100 unit + 2 golden + 5 property + 2 doc = 109 件全 pass)
- `cargo test -p kotoha-core --lib input` → PASS (24 件、plan 契約 ≥20 を満たす)
- `cargo clippy --workspace --all-targets -- -D warnings` → PASS (warnings ゼロ)
- `cargo fmt --all --check` → PASS (diff ゼロ)

## つまずき

- owasp L-1 指摘の `toggle_mode` 非対称性は 1 行 fix で解消可能。`set_mode` 側の clear ロジック (L241) と同じパターンを転用した。invariant コメントを付けて意図を明示した。
- team-review tool は `CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS=1` が未 set のため parallel 実行不可。main agent が 5 次元を逐次評価する fallback 運用となった。finding 結果は全 Low であり、判定の質への影響は無いと判断。

## M4c への申し送り

- `RomajiConverter::normalize_pending` は pub 化済。M4c の streaming path 対応で再利用可能。
- `InputContext` の公開 API と挙動は本 PR で fix した。M4c の mode golden TSV runner は `InputContext::new()` → `input_char` (各 char について) → `\n` 到達時 `commit()` のループで fixture の `input` 列を処理し、`expected_output` 列は commit の戻り値 + 行末改行の連結と一致させる方針。
- 4 条件の property test は `InputContext` の 3 状態機械の不変条件 (Hiragana-Sticky 安定性 / Transient 必ず復帰 / Sticky-Direct 持続 / reset 冪等性) を proptest で検証する。
- `allow_transient_to_sticky_promotion` フラグの property test は M4c で flag=false/true の 2 面展開を入れる予定 (ただし 4 条件の property 内訳には含めない、属人的 sanity check にとどめる)。
- `InputContext::preedit()` は Hiragana mode 時に空文字列を返す暫定実装。Phase 3 着手時に `RomajiConverter::pending(&self) -> &str` を追加する follow-up ISSUE を起票する (本 PR では起票せず、Phase 3 着手時に起票する方針。L-3 参照)。
- `crates/kotoha-core/tests/mode_golden.rs` / `mode_property.rs` は M4c で新規追加する。

## 成果物リンク

- ISSUE: [#37](https://github.com/std-koh-hinooka/kotoha-ime/issues/37)
- PR: [#38](https://github.com/std-koh-hinooka/kotoha-ime/pull/38) (merge commit: `c34b3c5`)
- Follow-up ISSUE: [#39](https://github.com/std-koh-hinooka/kotoha-ime/issues/39) (direct_buffer 上限、Phase 3 で対応)
- ADR: `docs/adr/0002-input-mode-transient-vs-sticky.md`
- Spec: `docs/superpowers/specs/2026-04-22-kotoha-phase-0-design.md` §7.3-§7.6, §8
- Plan: `docs/superpowers/plans/2026-04-23-kotoha-phase-0-m4.md` (Tasks M4b-1 〜 M4b-8)
