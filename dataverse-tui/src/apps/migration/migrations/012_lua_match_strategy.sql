-- Add Lua match strategy support.
-- New column for storing the Lua match script.
ALTER TABLE entity_mappings ADD COLUMN match_lua_script TEXT;

-- Widen the match_strategy CHECK constraint to allow 'lua'.
-- SQLite doesn't support ALTER CHECK, so we drop and re-create.
-- Dropping a CHECK requires a table rebuild, but we can add a new check
-- by using a trigger approach. However, SQLite CHECK constraints added
-- at CREATE TABLE time cannot be dropped. Instead, we rely on the
-- application layer for validation (MatchStrategy::from_str) and leave
-- the old CHECK in place — SQLite will accept 'lua' values only if we
-- recreate the constraint.
--
-- Pragmatic approach: recreate the table with the updated constraint.

CREATE TABLE entity_mappings_new (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    phase_id INTEGER NOT NULL,
    "order" INTEGER NOT NULL,
    name TEXT NOT NULL,
    source_entity TEXT NOT NULL,
    target_entity TEXT NOT NULL,
    mode TEXT NOT NULL CHECK (mode IN ('declarative', 'lua')),
    lua_script TEXT,
    match_strategy TEXT NOT NULL CHECK (match_strategy IN ('same_id', 'find', 'lua')),
    match_find_config BLOB,
    match_lua_script TEXT,
    no_match_fallback TEXT NOT NULL CHECK (no_match_fallback IN ('error', 'create', 'ignore')),
    orphan_strategy TEXT NOT NULL CHECK (orphan_strategy IN ('delete', 'deactivate', 'ignore', 'error')),
    create_pass_enabled INTEGER NOT NULL DEFAULT 1,
    activate_pass_enabled INTEGER NOT NULL DEFAULT 1,
    update_pass_enabled INTEGER NOT NULL DEFAULT 1,
    delete_pass_enabled INTEGER NOT NULL DEFAULT 1,
    deactivate_pass_enabled INTEGER NOT NULL DEFAULT 1,
    associate_pass_enabled INTEGER NOT NULL DEFAULT 1,
    disassociate_pass_enabled INTEGER NOT NULL DEFAULT 1,
    source_filter BLOB,
    target_filter BLOB,
    test_guids TEXT,
    FOREIGN KEY (phase_id) REFERENCES phases(id) ON DELETE CASCADE
);

INSERT INTO entity_mappings_new (
    id, phase_id, "order", name, source_entity, target_entity, mode, lua_script,
    match_strategy, match_find_config, match_lua_script, no_match_fallback, orphan_strategy,
    create_pass_enabled, activate_pass_enabled, update_pass_enabled,
    delete_pass_enabled, deactivate_pass_enabled,
    associate_pass_enabled, disassociate_pass_enabled,
    source_filter, target_filter, test_guids
)
SELECT
    id, phase_id, "order", name, source_entity, target_entity, mode, lua_script,
    match_strategy, match_find_config, NULL, no_match_fallback, orphan_strategy,
    create_pass_enabled, activate_pass_enabled, update_pass_enabled,
    delete_pass_enabled, deactivate_pass_enabled,
    associate_pass_enabled, disassociate_pass_enabled,
    source_filter, target_filter, test_guids
FROM entity_mappings;

DROP TABLE entity_mappings;
ALTER TABLE entity_mappings_new RENAME TO entity_mappings;
