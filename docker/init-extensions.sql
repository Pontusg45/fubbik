-- Extensions required by fubbik (runs on first database creation via docker-entrypoint-initdb.d)
CREATE EXTENSION IF NOT EXISTS vector;
CREATE EXTENSION IF NOT EXISTS pg_trgm;
CREATE EXTENSION IF NOT EXISTS age;
