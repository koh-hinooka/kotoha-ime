-- v002_learning_cache_index: learning_cache テーブルへの last_used_at index 追加
-- spec: docs/superpowers/specs/2026-04-26-p2-c-learning-cache-design.md §4.1
--
-- 目的:
--   evict_lru(ORDER BY last_used_at ASC) および
--   lookup(ORDER BY frequency DESC, last_used_at DESC) における
--   last_used_at 列のソートを index seek で高速化する。
--
-- 適用条件:
--   本 migration は forward-only(P2-B §3.6 決定)。
--   既存 v001 DB を持つ環境では Database::open() が PRAGMA user_version = 1 を検出し、
--   v002 を自動 apply する。新規 DB では v001 と v002 を順に apply する。

CREATE INDEX idx_learning_cache_last_used ON learning_cache(last_used_at);
