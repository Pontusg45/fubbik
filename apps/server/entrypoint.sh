#!/bin/sh

echo "Running database schema push..."
cd /app/packages/db

# drizzle-kit push may fail if pgvector extension is not available
# In that case, we continue — embedding column is optional
if ! bun drizzle-kit push --force 2>&1; then
    echo "Warning: drizzle-kit push failed (likely missing pgvector). Continuing..."
fi

echo "Running SQL migrations..."
bun run src/run-sql-migrations.ts 2>&1 || echo "Warning: some SQL migrations may have failed. Continuing..."

if [ "$SEED_DATABASE" = "true" ]; then
    echo "Seeding database..."
    bun run src/seed.ts 2>&1
    echo "Seeding complete."
fi

echo "Starting server..."
cd /app/apps/server
exec bun run dist/index.mjs
