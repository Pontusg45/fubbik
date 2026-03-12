#!/bin/sh
set -e

echo "Running database schema push..."
cd /app/packages/db
bun drizzle-kit push --force 2>&1
echo "Schema push complete."

echo "Running SQL migrations..."
bun run src/run-sql-migrations.ts 2>&1

if [ "$SEED_DATABASE" = "true" ]; then
    echo "Seeding database..."
    bun run src/seed.ts 2>&1
    echo "Seeding complete."
fi

echo "Starting server..."
cd /app/apps/server
exec bun run dist/index.mjs
