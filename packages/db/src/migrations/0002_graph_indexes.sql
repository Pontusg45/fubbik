-- 0002_graph_indexes.sql
-- GIN trigram indexes for fast fuzzy text search via pg_trgm
CREATE INDEX CONCURRENTLY IF NOT EXISTS chunk_title_trgm_idx
  ON chunk USING GIN (title gin_trgm_ops);

CREATE INDEX CONCURRENTLY IF NOT EXISTS chunk_content_trgm_idx
  ON chunk USING GIN (content gin_trgm_ops);

-- HNSW index for fast approximate nearest-neighbor vector search via pgvector
-- m=16 (connections per node), ef_construction=64 (build-time accuracy)
-- Tuned for 768-dim nomic-embed-text embeddings
CREATE INDEX CONCURRENTLY IF NOT EXISTS chunk_embedding_hnsw_idx
  ON chunk USING hnsw (embedding vector_cosine_ops)
  WITH (m = 16, ef_construction = 64);
