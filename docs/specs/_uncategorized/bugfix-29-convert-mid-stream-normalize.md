---
feature: bugfix-29-convert-mid-stream-normalize
status: implemented
bounded_context: _uncategorized
related_issues: ["#29"]
related_prs: []
glossary_refs: []
last_reviewed: 2026-05-05
---

# #29 hotfix: convert() mid-stream buffer 正規化

> **Migration note**: 本 spec は `docs/wbs/2026-04-23-bugfix-29-convert-mid-stream-normalize.md` から spec 化 (Global CLAUDE.md §Documentation Structure 準拠の `docs/wbs/` 廃止対応、2026-05-05)。
> 元 WBS は実装ログ性質。14-section restructure は別 follow-up 起票予定 (各 project の既存 14-section follow-up issue と合流可能)。

## 元 WBS 内容 (実装ログ由来)

milestone: hotfix
branch: bugfix/29-convert-mid-stream-normalize
pr: "#30"
merge_commit: "99d8358"
issue: "#29"
status: done
started: 2026-04-23
finished: 2026-04-23
---

# #29 hotfix: convert() mid-stream buffer 正規化

## 背景

PR #31 (M3b property test broaden) の proptest を full strategy `[a-z\-'.,!?\[\]/]{0,12}` で走らせた際、minimal counter-example `a="b!"`, `b="a"` が判明。これは #22 (非 ASCII retraction) と #23 (EOF buffer normalization) のいずれとも異なる第 3 のバグクラス:

- `convert("b!a")` は `("あ", "")` を返すが、本来 `("!あ", "")` であるべき。
- `convert("b!") + convert("a")` の split 評価は reconstructed `"!あ"` となり whole `"あ"` と不一致。

## 根本原因

PR #27 (#23 hotfix) では `StateMachine::normalize()` を `convert` の for-loop **後** に 1 度だけ呼んでいた。EOF residue は安定化されるが、mid-stream で settle が Invalid-drop 後に残したバッファ (例: `"!"`) は、次の push (例: `'a'`) で再び Invalid として drop され、commit されない。

トレース (修正前):
1. push 'b': buffer="b" Partial.
2. push '!': settle("b!")=None, drop 'b', buffer="!" Invalid('b')。`!` は完全な rule だが buffer に stranded。
3. push 'a': settle("!a")=None, drop '!', buffer="a" Invalid('!')。**ここで `!` が失われる**。
4. EOF normalize: buffer="a" Match "あ", commit。

## 実施内容

### 修正

`crates/kotoha-core/src/romaji/mod.rs` の `convert` 関数内の for-loop で、各 push 後に `self.machine.normalize()` を呼ぶ。salvage された kana は `out` に append。EOF normalize は defense-in-depth として保持 (冗長)。

### テスト

TDD で実施:
1. Red: 2 つの regression test を先行追加 (`convert_commits_punctuation_after_partial_invalid_transition` と `convert_associative_across_partial_punctuation_split`)、実装前に 2 件 FAIL を確認。
2. Green: for-loop 内 normalize を追加、全 76 lib tests PASS。
3. no-regression: golden 207 rows PASS、property (narrow strategy) PASS。

### コミット

1 commit: `789de88` — fix + 2 regression tests。

## つまずき

- 本バグは M3b PR #24 merge 時点では気づいておらず、PR #26 (property broaden) の proptest で初めて露出した。proptest の shrinking が `b!a` という minimal counter-example まで絞り込んでくれたため、原因特定は迅速。
- #22 / #23 の ADR / hotfix が先に入っていたため、このバグが第 3 の独立クラスであると切り分けられた。property test を先に broaden していたら 3 つのバグが同時露出してデバッグが困難だった可能性があり、#22/#23 → #29 → #26 の順序は正解だった。

## レビュー

Architecture + testing 合同 review。Critical / Important なし、Minor 3 件:
1. EOF normalize を `debug_assert!` 化してループ不変条件を明示化 (defense-in-depth の意図を自己文書化) → deferred。
2. prefix 文字を `b` 以外 (`k`, `s`, `t`, `n`) にも広げた coverage → PR #31 の property test broaden で自動的にカバー、deferred。
3. 3-way split test → PR #31 で自動的にカバー、deferred。

全 Minor を deferred として merge。PR #31 merge 後、これらの coverage は property test により自動的に取得されている。

## M4 への申し送り

- `convert` の for-loop 内 normalize により、mid-stream での partial-invalid transition が正しく commit 経路に乗るようになった。
- ただし **streaming API (`push` 直接 + `flush`) は未修正**: `push` 自体は依然として per-call に 1 つの `PushResult` を返す契約のため、mid-stream の salvage は caller (例: `InputContext`) 側で `push` 後に別途 `normalize` 相当を呼ぶか、push/settle の契約を変更する必要がある。M4 の `InputContext` 実装時にこの設計判断が必要。
- 候補案: (a) `StateMachine` に `normalize_and_take` メソッドを追加し caller から呼ぶ、(b) `push` を loop 化して複数 commit を `Vec<PushResult>` として返す、(c) `InputContext` 側で push 後に毎回 normalize 呼び出し。現時点では (c) が最小変更。

## 成果物リンク

- PR: https://github.com/std-koh-hinooka/kotoha-ime/pull/30
- ISSUE: https://github.com/std-koh-hinooka/kotoha-ime/issues/29 (CLOSED)
- 関連 PR: #27 (#23 hotfix — EOF normalize) / #31 (#26 property broaden — 本修正により unblock)
