-- Migration: Remove settings table
-- Settings are now handled by the global settings system in dataverse-tui
-- This will reset all indexer settings to their defaults

DROP TABLE IF EXISTS settings;
