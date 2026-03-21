-- One-time migration: Backfill embeddingUpdatedAt for existing chunks that have embeddings
-- Run manually: psql $DATABASE_URL -f packages/db/migrations/backfill-embedding-updated-at.sql
UPDATE chunk
SET embedding_updated_at = updated_at
WHERE embedding IS NOT NULL
  AND embedding_updated_at IS NULL;
