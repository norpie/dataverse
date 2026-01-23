-- Saved queries initial schema

CREATE TABLE IF NOT EXISTS saved_queries (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    entity TEXT,
    data BLOB NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

-- Index for listing queries by name
CREATE INDEX IF NOT EXISTS idx_saved_queries_name
    ON saved_queries (name);

-- Index for filtering by entity
CREATE INDEX IF NOT EXISTS idx_saved_queries_entity
    ON saved_queries (entity);
