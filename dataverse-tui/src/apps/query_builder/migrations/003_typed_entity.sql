-- Migration: QueryData entity type changed from raw string to Entity enum
-- Clear saved queries because serialized query blobs are incompatible.

DELETE FROM saved_queries;
