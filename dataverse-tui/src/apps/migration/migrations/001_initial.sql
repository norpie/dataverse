-- Migration database initial schema

-- Migrations
CREATE TABLE IF NOT EXISTS migrations (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    description TEXT,
    source_environment_id INTEGER NOT NULL,
    target_environment_id INTEGER NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_migrations_created
    ON migrations (created_at DESC);

-- Phases
CREATE TABLE IF NOT EXISTS phases (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    migration_id INTEGER NOT NULL,
    "order" INTEGER NOT NULL,
    name TEXT NOT NULL,
    mode TEXT NOT NULL CHECK (mode IN ('declarative', 'lua')),
    lua_script TEXT,
    FOREIGN KEY (migration_id) REFERENCES migrations(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_phases_migration
    ON phases (migration_id, "order");

-- Entity Mappings
CREATE TABLE IF NOT EXISTS entity_mappings (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    phase_id INTEGER NOT NULL,
    "order" INTEGER NOT NULL,
    source_entity TEXT NOT NULL,
    target_entity TEXT NOT NULL,
    mode TEXT NOT NULL CHECK (mode IN ('declarative', 'lua')),
    lua_script TEXT,
    match_strategy TEXT NOT NULL CHECK (match_strategy IN ('same_id', 'find')),
    match_find_config BLOB,
    no_match_fallback TEXT NOT NULL CHECK (no_match_fallback IN ('error', 'create', 'ignore')),
    orphan_strategy TEXT NOT NULL CHECK (orphan_strategy IN ('delete', 'deactivate', 'ignore', 'error')),
    create_pass_enabled INTEGER NOT NULL DEFAULT 1,
    update_pass_enabled INTEGER NOT NULL DEFAULT 1,
    source_filter BLOB,
    target_filter BLOB,
    test_guids TEXT,
    FOREIGN KEY (phase_id) REFERENCES phases(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_entity_mappings_phase
    ON entity_mappings (phase_id, "order");

-- Variables
CREATE TABLE IF NOT EXISTS variables (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    entity_mapping_id INTEGER NOT NULL,
    "order" INTEGER NOT NULL,
    name TEXT NOT NULL,
    FOREIGN KEY (entity_mapping_id) REFERENCES entity_mappings(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_variables_entity_mapping
    ON variables (entity_mapping_id, "order");

-- Field Mappings
CREATE TABLE IF NOT EXISTS field_mappings (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    entity_mapping_id INTEGER NOT NULL,
    "order" INTEGER NOT NULL,
    target_field TEXT NOT NULL,
    FOREIGN KEY (entity_mapping_id) REFERENCES entity_mappings(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_field_mappings_entity_mapping
    ON field_mappings (entity_mapping_id, "order");

-- Transforms
CREATE TABLE IF NOT EXISTS transforms (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    entity_mapping_id INTEGER NOT NULL,
    parent_type TEXT NOT NULL CHECK (parent_type IN ('field_mapping', 'variable', 'match_branch', 'guard_fallback')),
    parent_id INTEGER NOT NULL,
    "order" INTEGER NOT NULL,
    transform_type TEXT NOT NULL,
    data BLOB NOT NULL,
    FOREIGN KEY (entity_mapping_id) REFERENCES entity_mappings(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_transforms_entity_mapping
    ON transforms (entity_mapping_id);

CREATE INDEX IF NOT EXISTS idx_transforms_parent
    ON transforms (parent_type, parent_id, "order");

-- Match Branches
CREATE TABLE IF NOT EXISTS match_branches (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    transform_id INTEGER NOT NULL,
    "order" INTEGER NOT NULL,
    condition BLOB,
    is_default INTEGER NOT NULL DEFAULT 0,
    FOREIGN KEY (transform_id) REFERENCES transforms(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_match_branches_transform
    ON match_branches (transform_id, "order");

-- Phase Runs (execution history)
CREATE TABLE IF NOT EXISTS phase_runs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    phase_id INTEGER NOT NULL,
    started_at TEXT NOT NULL,
    completed_at TEXT,
    status TEXT NOT NULL CHECK (status IN ('running', 'completed', 'failed', 'cancelled')),
    queue_item_ids TEXT,
    error TEXT,
    FOREIGN KEY (phase_id) REFERENCES phases(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_phase_runs_phase
    ON phase_runs (phase_id, started_at DESC);
