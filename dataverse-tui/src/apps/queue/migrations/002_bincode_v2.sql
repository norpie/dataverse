-- Migration: Bincode v2 upgrade
-- Clear all queue items and settings due to incompatible bincode format change
-- (bincode 1.x -> 2.x have different wire formats)

DELETE FROM queue_items;
DELETE FROM queue_settings;
DELETE FROM execution_history;
