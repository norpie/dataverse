-- Queue items table
CREATE TABLE IF NOT EXISTS queue_items (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    priority INTEGER NOT NULL DEFAULT 0,
    status TEXT NOT NULL DEFAULT 'ready',
    payload BLOB NOT NULL,
    env_id INTEGER NOT NULL,
    account_id INTEGER NOT NULL,
    source TEXT NOT NULL,
    description TEXT NOT NULL,
    created_at TEXT NOT NULL
);

-- Index for getting next ready item (priority DESC, created_at ASC)
CREATE INDEX IF NOT EXISTS idx_queue_items_ready
    ON queue_items (status, priority DESC, created_at ASC)
    WHERE status = 'ready';

-- Index for filtering by status
CREATE INDEX IF NOT EXISTS idx_queue_items_status
    ON queue_items (status);

-- Index for filtering by source
CREATE INDEX IF NOT EXISTS idx_queue_items_source
    ON queue_items (source);

-- Index for environment availability updates
CREATE INDEX IF NOT EXISTS idx_queue_items_env
    ON queue_items (env_id, status);

-- Execution history table
CREATE TABLE IF NOT EXISTS execution_history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    item_id INTEGER NOT NULL,
    started_at TEXT NOT NULL,
    completed_at TEXT NOT NULL,
    duration_ms INTEGER NOT NULL,
    status TEXT NOT NULL,
    error TEXT,
    success_count INTEGER NOT NULL DEFAULT 0,
    failure_count INTEGER NOT NULL DEFAULT 0
);

-- Index for getting executions by item
CREATE INDEX IF NOT EXISTS idx_execution_history_item
    ON execution_history (item_id, started_at DESC);

-- Index for cleanup by date
CREATE INDEX IF NOT EXISTS idx_execution_history_date
    ON execution_history (completed_at);

-- Queue settings table (concurrency, max_failures, etc.)
CREATE TABLE IF NOT EXISTS queue_settings (
    key TEXT PRIMARY KEY,
    value BLOB NOT NULL
);
