# Upgrading PostgreSQL

## PG16 → PG18 (pgvector 0.8.0 → 0.8.2)

PostgreSQL 18 changed the default data directory inside Docker from
`/var/lib/postgresql/data` to `/var/lib/postgresql/18/docker`. This means you
**cannot** simply change the image tag — the new container won't find your data.

### Prerequisites

- Your fubbik stack is currently running on `pgvector/pgvector:0.8.0-pg16`
- You have updated to the latest fubbik code (which uses the new image + volume mount)

### Option A: Dump & Restore (recommended)

Best for most fubbik installations. Downtime: a few seconds to minutes depending
on data size.

**1. Dump the database from the running PG16 container:**

```bash
# For docker-compose.yml (dev)
docker exec fubbik-postgres pg_dumpall -U postgres > fubbik_backup.sql

# For docker-compose.selfhost.yml
docker exec fubbik-selfhost-db-1 pg_dumpall -U fubbik > fubbik_backup.sql
```

**2. Stop all services:**

```bash
# Dev
docker compose down

# Selfhost
docker compose -f docker-compose.selfhost.yml down
```

**3. Remove the old volume:**

> ⚠️ This deletes the PG16 data. Your backup from step 1 is your safety net.

```bash
# Dev
docker volume rm fubbik_fubbik_postgres_data

# Selfhost
docker volume rm fubbik_fubbik_data
```

If the volume name differs, check with `docker volume ls | grep fubbik`.

**4. Pull the latest code** (which has the updated docker-compose files).

**5. Start only the database:**

```bash
# Dev
docker compose up -d db

# Selfhost
docker compose -f docker-compose.selfhost.yml up -d db
```

Wait for it to be healthy:

```bash
docker compose ps
```

**6. Restore the backup:**

```bash
# Dev
docker exec -i fubbik-postgres psql -U postgres < fubbik_backup.sql

# Selfhost
docker exec -i fubbik-selfhost-db-1 psql -U fubbik < fubbik_backup.sql
```

**7. Reindex (recommended by PG18 release notes for pg_trgm):**

```bash
# Dev
docker exec fubbik-postgres psql -U postgres -d fubbik -c "REINDEX DATABASE fubbik;"

# Selfhost
docker exec fubbik-selfhost-db-1 psql -U fubbik -d fubbik -c "REINDEX DATABASE fubbik;"
```

**8. Start the rest of the stack:**

```bash
# Dev
docker compose up -d

# Selfhost
docker compose -f docker-compose.selfhost.yml up -d
```

**9. Verify:**

```bash
# Check PostgreSQL version
docker exec fubbik-postgres psql -U postgres -c "SELECT version();"
# Should show: PostgreSQL 18.x

# Check extensions
docker exec fubbik-postgres psql -U postgres -d fubbik -c "SELECT extname, extversion FROM pg_extension;"
# Should show vector and pg_trgm

# Check app health
curl http://localhost:3000/api/health
```

### Option B: Fresh start (no existing data)

If you don't need to preserve data (e.g., dev environment), just remove the old
volume and start fresh:

```bash
docker compose down
docker volume rm fubbik_fubbik_postgres_data
docker compose up -d
```

The entrypoint script will run migrations and optionally seed the database
(`SEED_DATABASE=true`).

### Post-Upgrade Notes

- **pg_trgm indexes:** PG18 changed collation handling. The `REINDEX` in step 7
  ensures trigram indexes for fuzzy search are correct.
- **pgvector HNSW index:** The reindex also covers the embedding index. No
  separate action needed.
- **Drizzle migrations:** The entrypoint runs `drizzle-kit migrate` on startup.
  No manual migration step required.
- **New PG18 features available:** `uuidv7()`, virtual generated columns,
  `RETURNING OLD/NEW`, async I/O (up to 3× read perf improvement).
