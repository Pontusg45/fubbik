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
docker exec fubbik-postgres pg_dumpall -U postgres > fubbik_backup.sql
```

**2. Stop all services:**

```bash
docker compose down
```

**3. Remove the old volume:**

```bash
docker volume rm fubbik_fubbik_postgres_data
```

**4. Pull the latest code** (which has the updated docker-compose files).

**5. Start only the database:**

```bash
docker compose up -d db
```

**6. Restore the backup:**

```bash
docker exec -i fubbik-postgres psql -U postgres < fubbik_backup.sql
```

**7. Reindex (recommended by PG18 release notes for pg_trgm):**

```bash
docker exec fubbik-postgres psql -U postgres -d fubbik -c "REINDEX DATABASE fubbik;"
```

**8. Start the rest of the stack:**

```bash
docker compose up -d
```

**9. Verify:**

```bash
docker exec fubbik-postgres psql -U postgres -c "SELECT version();"
curl http://localhost:3000/api/health
```

### Option B: Fresh start (no existing data)

```bash
docker compose down
docker volume rm fubbik_fubbik_postgres_data
docker compose up -d
```

### Post-Upgrade Notes

- **pg_trgm indexes:** PG18 changed collation handling. The `REINDEX` ensures trigram indexes for fuzzy search are correct.
- **pgvector HNSW index:** The reindex also covers the embedding index.
- **Drizzle migrations:** The entrypoint runs `drizzle-kit migrate` on startup.
- **New PG18 features available:** `uuidv7()`, virtual generated columns, `RETURNING OLD/NEW`, async I/O.
