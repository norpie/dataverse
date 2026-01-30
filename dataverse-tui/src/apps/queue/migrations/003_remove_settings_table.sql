-- Migration: Remove queue_settings table
-- Settings are now handled by the global settings system in dataverse-tui
-- This will reset all queue settings to their defaults

DROP TABLE IF EXISTS queue_settings;
