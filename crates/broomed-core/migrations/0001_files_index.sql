-- 0001_files_index — broomed-core SQLite schema (spec §8, §43)
-- Standalone SQLite file for the Rust core; does NOT conflict with Alembic/Postgres web storage.
PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS files (
    id TEXT PRIMARY KEY,
    path TEXT NOT NULL UNIQUE,
    canonical_path TEXT,
    filename TEXT NOT NULL,
    extension TEXT,
    mime_type TEXT,
    size INTEGER,
    created_at TEXT,
    modified_at TEXT,
    accessed_at TEXT,
    inode TEXT,
    hash TEXT,
    hash_algorithm TEXT,
    parent_directory TEXT,
    is_hidden INTEGER DEFAULT 0,
    is_symlink INTEGER DEFAULT 0,
    is_deleted INTEGER DEFAULT 0,
    scan_version INTEGER DEFAULT 1,
    updated_at TEXT
);
CREATE INDEX IF NOT EXISTS idx_files_hash ON files(hash);
CREATE INDEX IF NOT EXISTS idx_files_parent_directory ON files(parent_directory);
CREATE INDEX IF NOT EXISTS idx_files_mime_type ON files(mime_type);

CREATE TABLE IF NOT EXISTS file_metadata (
    file_id TEXT NOT NULL REFERENCES files(id) ON DELETE CASCADE,
    data TEXT NOT NULL,
    PRIMARY KEY (file_id)
);

CREATE TABLE IF NOT EXISTS file_embeddings (
    file_id TEXT NOT NULL REFERENCES files(id) ON DELETE CASCADE,
    model TEXT NOT NULL,
    vec BLOB NOT NULL,
    version INTEGER DEFAULT 1,
    PRIMARY KEY (file_id, model)
);

CREATE TABLE IF NOT EXISTS file_categories (
    file_id TEXT NOT NULL REFERENCES files(id) ON DELETE CASCADE,
    category TEXT NOT NULL,
    subcategory TEXT,
    confidence REAL,
    reason TEXT,
    provider TEXT,
    created_at TEXT
);

CREATE TABLE IF NOT EXISTS operations (
    id TEXT PRIMARY KEY,
    source TEXT NOT NULL,
    destination TEXT NOT NULL,
    operation_type TEXT NOT NULL,
    reason TEXT,
    confidence REAL,
    reversible INTEGER DEFAULT 1,
    status TEXT NOT NULL,
    created_at TEXT
);

CREATE TABLE IF NOT EXISTS operation_items (
    operation_id TEXT NOT NULL REFERENCES operations(id) ON DELETE CASCADE,
    seq INTEGER NOT NULL,
    source TEXT,
    destination TEXT,
    status TEXT,
    PRIMARY KEY (operation_id, seq)
);

CREATE TABLE IF NOT EXISTS ai_requests (
    id TEXT PRIMARY KEY,
    provider TEXT,
    model TEXT,
    task_type TEXT,
    prompt_version INTEGER,
    tokens INTEGER,
    cost REAL,
    created_at TEXT
);

CREATE TABLE IF NOT EXISTS directories (
    path TEXT PRIMARY KEY,
    watched INTEGER DEFAULT 0,
    excluded INTEGER DEFAULT 0
);

CREATE TABLE IF NOT EXISTS settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    version INTEGER DEFAULT 1
);
