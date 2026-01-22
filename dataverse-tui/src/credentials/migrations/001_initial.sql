-- Credentials database initial schema

CREATE TABLE IF NOT EXISTS environments (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    url TEXT NOT NULL UNIQUE,
    display_name TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS accounts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    display_name TEXT NOT NULL,
    auth_type TEXT NOT NULL,
    client_id TEXT NOT NULL,
    tenant_id TEXT,
    client_secret TEXT,
    username TEXT,
    password TEXT
);

CREATE TABLE IF NOT EXISTS tokens (
    account_id INTEGER NOT NULL,
    environment_id INTEGER NOT NULL,
    access_token TEXT NOT NULL,
    expires_at TEXT,
    refresh_token TEXT,
    PRIMARY KEY (account_id, environment_id),
    FOREIGN KEY (account_id) REFERENCES accounts(id) ON DELETE CASCADE,
    FOREIGN KEY (environment_id) REFERENCES environments(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS active_session (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    account_id INTEGER,
    environment_id INTEGER,
    FOREIGN KEY (account_id) REFERENCES accounts(id) ON DELETE SET NULL,
    FOREIGN KEY (environment_id) REFERENCES environments(id) ON DELETE SET NULL
);

-- Ensure active_session has exactly one row
INSERT OR IGNORE INTO active_session (id, account_id, environment_id)
VALUES (1, NULL, NULL);
