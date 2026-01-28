-- Migration: Bincode v2 upgrade
-- Clear all saved queries due to incompatible bincode format change
-- (bincode 1.x -> 2.x have different wire formats)

DELETE FROM saved_queries;
