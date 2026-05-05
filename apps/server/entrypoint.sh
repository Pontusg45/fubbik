#!/bin/sh

# Extensions (vector, pg_trgm) are created by the PostgreSQL container's init scripts:
#   - docker-compose.yml: mounts docker/init-extensions.sql into /docker-entrypoint-initdb.d/
#   - docker-compose.selfhost.yml: creates the init script via entrypoint override
#   - Local dev: create extensions manually or via pnpm db:push
# They must exist before drizzle-kit migrate (the chunk table uses vector(768)).

echo "Running database migrations..."
cd /app/packages/db

# drizzle-kit is in the flat node_modules at /app/node_modules
export PATH="/app/node_modules/.bin:$PATH"

# Apply Drizzle-managed schema migrations (tracked in __drizzle_migrations table)
echo "Running drizzle-kit migrate..."
drizzle-kit migrate 2>&1 || { echo "ERROR: drizzle-kit migrate failed. Aborting."; exit 1; }

if [ "$SEED_DATABASE" = "true" ]; then
    echo "Seeding database..."
    bun run src/seed.ts 2>&1
    echo "Seeding complete."
fi

echo "Starting server..."
cd /app
exec bun run dist/index.mjs
