-- Enable pgvector extension
CREATE EXTENSION IF NOT EXISTS vector;

-- Add retrieval-enhancing columns
ALTER TABLE chunk ADD COLUMN IF NOT EXISTS summary text;
ALTER TABLE chunk ADD COLUMN IF NOT EXISTS aliases jsonb NOT NULL DEFAULT '[]';
ALTER TABLE chunk ADD COLUMN IF NOT EXISTS not_about jsonb NOT NULL DEFAULT '[]';
ALTER TABLE chunk ADD COLUMN IF NOT EXISTS scope jsonb NOT NULL DEFAULT '{}';
ALTER TABLE chunk ADD COLUMN IF NOT EXISTS embedding vector(768);

-- GIN indexes for JSONB array/object filtering
CREATE INDEX IF NOT EXISTS chunk_aliases_gin_idx ON chunk USING gin (aliases);
CREATE INDEX IF NOT EXISTS chunk_not_about_gin_idx ON chunk USING gin (not_about);
CREATE INDEX IF NOT EXISTS chunk_scope_gin_idx ON chunk USING gin (scope);

-- HNSW index for approximate nearest neighbor search (cosine distance)
CREATE INDEX IF NOT EXISTS chunk_embedding_hnsw_idx ON chunk USING hnsw (embedding vector_cosine_ops);
