-- Add 'match_condition' to transforms CHECK constraint.

CREATE TABLE IF NOT EXISTS transforms_new (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    entity_mapping_id INTEGER NOT NULL,
    parent_type TEXT NOT NULL CHECK (parent_type IN ('field_mapping', 'variable', 'match_branch', 'guard_fallback', 'match_default', 'coalesce_chain', 'find_condition', 'find_default', 'match_condition')),
    parent_id INTEGER NOT NULL,
    "order" INTEGER NOT NULL,
    transform_type TEXT NOT NULL,
    data BLOB NOT NULL,
    FOREIGN KEY (entity_mapping_id) REFERENCES entity_mappings(id) ON DELETE CASCADE
);

INSERT INTO transforms_new (id, entity_mapping_id, parent_type, parent_id, "order", transform_type, data)
    SELECT id, entity_mapping_id, parent_type, parent_id, "order", transform_type, data
    FROM transforms;

DROP TABLE transforms;
ALTER TABLE transforms_new RENAME TO transforms;

CREATE INDEX IF NOT EXISTS idx_transforms_entity_mapping
    ON transforms (entity_mapping_id);

CREATE INDEX IF NOT EXISTS idx_transforms_parent
    ON transforms (parent_type, parent_id, "order");
