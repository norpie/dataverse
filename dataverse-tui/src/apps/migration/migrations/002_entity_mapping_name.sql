-- Add name column to entity_mappings

ALTER TABLE entity_mappings ADD COLUMN name TEXT NOT NULL DEFAULT '';
