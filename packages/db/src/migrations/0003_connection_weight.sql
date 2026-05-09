-- Add weight column to chunk_connection for co-access pattern tracking
ALTER TABLE chunk_connection ADD COLUMN IF NOT EXISTS weight integer NOT NULL DEFAULT 1;
