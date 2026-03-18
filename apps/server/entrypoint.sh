#!/bin/sh

echo "Running database migrations..."
cd /app/packages/db

# drizzle-kit is in the flat node_modules at /app/node_modules
export PATH="/app/node_modules/.bin:$PATH"

# Apply Drizzle-managed schema migrations (tracked in __drizzle_migrations table)
if ! drizzle-kit migrate 2>&1; then
    echo "Warning: drizzle-kit migrate failed. Continuing..."
fi

echo "Running SQL extensions and seeds..."
bun run src/run-sql-migrations.ts 2>&1 || echo "Warning: some SQL migrations may have failed. Continuing..."

if [ "$SEED_DATABASE" = "true" ]; then
    echo "Seeding database..."
    bun run src/seed.ts 2>&1
    echo "Seeding complete."
fi

echo "Starting server..."
cd /app
exec bun run dist/index.mjs
