PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS saved (
    article_id TEXT PRIMARY KEY NOT NULL,
    saved_at TEXT NOT NULL,
    note TEXT,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS saved_tags (
    article_id TEXT NOT NULL,
    tag TEXT NOT NULL,
    PRIMARY KEY (article_id, tag)
);

CREATE TABLE IF NOT EXISTS tags (
    tag TEXT PRIMARY KEY NOT NULL
);

CREATE TABLE IF NOT EXISTS sessions (
    token_hash TEXT PRIMARY KEY NOT NULL,
    created_at TEXT NOT NULL,
    expires_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS settings (
    key TEXT PRIMARY KEY NOT NULL,
    value TEXT NOT NULL
);
