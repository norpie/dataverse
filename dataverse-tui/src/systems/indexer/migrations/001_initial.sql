-- Indexer database schema
-- Tracks metadata sync status for each environment

-- Environment sync state
CREATE TABLE environment_sync (
    env_id INTEGER PRIMARY KEY,
    status TEXT NOT NULL,  -- 'idle', 'syncing', 'paused', 'error'
    last_sync_at INTEGER,  -- unix timestamp, NULL if never synced
    last_error TEXT,       -- error message from last failed sync
    entities_count INTEGER DEFAULT 0,
    global_optionsets_count INTEGER DEFAULT 0,
    total_attributes_count INTEGER DEFAULT 0
);

-- Sync history log
CREATE TABLE sync_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    env_id INTEGER NOT NULL,
    started_at INTEGER NOT NULL,
    completed_at INTEGER,     -- NULL if still running or failed
    status TEXT NOT NULL,     -- 'success', 'failed', 'cancelled'
    error TEXT,               -- error message if failed
    entities_fetched INTEGER DEFAULT 0,
    optionsets_fetched INTEGER DEFAULT 0
);

-- Index for querying recent logs per environment
CREATE INDEX idx_sync_log_env_id ON sync_log(env_id, started_at DESC);

-- Settings key-value store (bincode serialized values)
CREATE TABLE settings (
    key TEXT PRIMARY KEY,
    value BLOB NOT NULL
);
