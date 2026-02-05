-- Add missing pass columns to entity_mappings table
ALTER TABLE entity_mappings ADD COLUMN delete_pass_enabled INTEGER NOT NULL DEFAULT 1;
ALTER TABLE entity_mappings ADD COLUMN deactivate_pass_enabled INTEGER NOT NULL DEFAULT 1;
ALTER TABLE entity_mappings ADD COLUMN associate_pass_enabled INTEGER NOT NULL DEFAULT 1;
ALTER TABLE entity_mappings ADD COLUMN disassociate_pass_enabled INTEGER NOT NULL DEFAULT 1;
