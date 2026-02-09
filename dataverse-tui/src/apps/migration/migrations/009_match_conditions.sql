-- Match conditions: each row represents one condition within a match config (find mode)
CREATE TABLE IF NOT EXISTS match_conditions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    entity_mapping_id INTEGER NOT NULL,
    target_field TEXT NOT NULL,
    "order" INTEGER NOT NULL,
    FOREIGN KEY (entity_mapping_id) REFERENCES entity_mappings(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_match_conditions_entity_mapping
    ON match_conditions (entity_mapping_id, "order");
