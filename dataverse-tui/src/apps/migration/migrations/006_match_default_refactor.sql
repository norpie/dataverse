-- Move default branch logic from match_branches to TransformData::Match { has_default }.
-- The is_default column is no longer used; SQLite doesn't support DROP COLUMN on older versions,
-- so we recreate the table without it.

CREATE TABLE IF NOT EXISTS match_branches_new (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    transform_id INTEGER NOT NULL,
    "order" INTEGER NOT NULL,
    condition BLOB NOT NULL,
    FOREIGN KEY (transform_id) REFERENCES transforms(id) ON DELETE CASCADE
);

-- Copy only non-default branches (default branches had no condition, which is now invalid)
INSERT INTO match_branches_new (id, transform_id, "order", condition)
    SELECT id, transform_id, "order", condition
    FROM match_branches
    WHERE is_default = 0 AND condition IS NOT NULL;

DROP TABLE match_branches;
ALTER TABLE match_branches_new RENAME TO match_branches;

CREATE INDEX IF NOT EXISTS idx_match_branches_transform
    ON match_branches (transform_id, "order");
