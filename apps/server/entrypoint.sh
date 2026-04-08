#!/bin/sh

echo "Enabling PostgreSQL extensions..."
# Extensions must be created before schema migration (vector type used in tables)
if [ -n "$DATABASE_URL" ]; then
    bun -e "
const pg = require('pg');
const pool = new pg.Pool({ connectionString: process.env.DATABASE_URL });
(async () => {
    try {
        await pool.query('CREATE EXTENSION IF NOT EXISTS vector');
        await pool.query('CREATE EXTENSION IF NOT EXISTS pg_trgm');
        console.log('  Extensions enabled');
    } catch(e) { console.log('  Extension warning:', e.message); }
    await pool.end();
})();
" 2>&1 || echo "Warning: extension setup failed. Continuing..."
fi

echo "Running database migrations..."
cd /app/packages/db

# drizzle-kit is in the flat node_modules at /app/node_modules
export PATH="/app/node_modules/.bin:$PATH"

# Apply Drizzle-managed schema migrations (tracked in __drizzle_migrations table)
if ! drizzle-kit push 2>&1; then
    echo "Warning: drizzle-kit push failed. Trying migrate..."
    drizzle-kit migrate 2>&1 || echo "Warning: drizzle-kit migrate also failed. Continuing..."
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
