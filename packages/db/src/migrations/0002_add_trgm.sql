-- Enable trigram extension for fuzzy search
CREATE EXTENSION IF NOT EXISTS pg_trgm;

-- Add GIN trigram indexes for fast fuzzy search
CREATE INDEX IF NOT EXISTS chunk_title_trgm_idx ON chunk USING gin (title gin_trgm_ops);
CREATE INDEX IF NOT EXISTS chunk_content_trgm_idx ON chunk USING gin (content gin_trgm_ops);

-- GIN index for JSONB tags queries
CREATE INDEX IF NOT EXISTS chunk_tags_gin_idx ON chunk USING gin (tags);
