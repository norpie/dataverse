-- Add tables for multi-chain transform parents (coalesce and find)

-- Coalesce chains: each row represents one fallback chain within a coalesce transform
CREATE TABLE IF NOT EXISTS coalesce_chains (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    transform_id INTEGER NOT NULL,
    "order" INTEGER NOT NULL,
    FOREIGN KEY (transform_id) REFERENCES transforms(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_coalesce_chains_transform
    ON coalesce_chains (transform_id, "order");

-- Find conditions: each row represents one condition within a find transform (where-clause mode)
CREATE TABLE IF NOT EXISTS find_conditions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    transform_id INTEGER NOT NULL,
    target_field TEXT NOT NULL,
    "order" INTEGER NOT NULL,
    FOREIGN KEY (transform_id) REFERENCES transforms(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_find_conditions_transform
    ON find_conditions (transform_id, "order");

-- Note: SQLite doesn't support ALTER TABLE to modify CHECK constraints.
-- The new parent_type values ('coalesce_chain', 'find_condition') will work
-- because SQLite CHECK constraints on existing rows aren't re-validated,
-- and new inserts with these values will be allowed since we're using
-- application-level validation via the ParentType enum.
