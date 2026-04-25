-- v001_initial: user_vocab + learning_cache schema
-- spec: docs/superpowers/specs/2026-04-25-p2-b-user-dictionary-design.md §5.1 / §5.2

CREATE TABLE user_vocab (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    surface     TEXT    NOT NULL,
    reading     TEXT    NOT NULL,
    pos         TEXT    NOT NULL DEFAULT '名詞-固有名詞-一般',
    score       REAL    NOT NULL DEFAULT 1.0,
    created_at  INTEGER NOT NULL,
    updated_at  INTEGER NOT NULL,
    UNIQUE(surface, reading)
);

CREATE INDEX idx_user_vocab_reading ON user_vocab(reading);

CREATE TABLE learning_cache (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    kana_input   TEXT    NOT NULL,
    chosen_kanji TEXT    NOT NULL,
    frequency    INTEGER NOT NULL DEFAULT 1,
    last_used_at INTEGER NOT NULL,
    UNIQUE(kana_input, chosen_kanji)
);

CREATE INDEX idx_learning_cache_kana ON learning_cache(kana_input);
