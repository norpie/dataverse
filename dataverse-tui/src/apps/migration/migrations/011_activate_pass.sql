-- Add activate_pass_enabled column to entity_mappings table.
-- The activate pass reactivates inactive target records before the update pass.
ALTER TABLE entity_mappings ADD COLUMN activate_pass_enabled INTEGER NOT NULL DEFAULT 1;
