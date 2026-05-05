# PostgreSQL 18 Upgrade Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Upgrade from PostgreSQL 16 (pgvector 0.8.0) to PostgreSQL 18 (pgvector 0.8.2), updating all Docker configuration, documentation, and providing a migration path for existing deployments.

**Architecture:** The upgrade touches Docker Compose files (image tag + volume mount path), documentation files (CLAUDE.md, CONTRIBUTING.md, getting-started guide), and adds a new migration guide for selfhosters. No application code changes needed — the `pg` driver, Drizzle ORM, pgvector, and pg_trgm all work with PG18. The critical gotcha is PG18's Docker `PGDATA` path change from `/var/lib/postgresql/data` to `/var/lib/postgresql/18/docker`, which breaks naive image-tag swaps.

**Tech Stack:** PostgreSQL 18, pgvector 0.8.2, Docker Compose

---

## File Map

| Action | File | Responsibility |
|--------|------|----------------|
| Modify | `docker-compose.yml:70-81` | Dev Docker: image tag + volume mount |
| Modify | `docker-compose.selfhost.yml:18,24` | Selfhost Docker: image tag + volume mount |
| Modify | `CLAUDE.md` | Project docs: note PG18 |
| Modify | `CONTRIBUTING.md:8` | Prerequisites: note PG18 |
| Modify | `docs/guide/getting-started.md` | Getting started: note PG18 |
| Create | `docs/guide/upgrading-postgresql.md` | Migration guide for existing users |

---

### Task 1: Update docker-compose.yml

**Files:**
- Modify: `docker-compose.yml:70-81`

- [ ] **Step 1: Update image tag**

In `docker-compose.yml`, change the `db` service image from:

```yaml
image: pgvector/pgvector:0.8.0-pg16
```

to:

```yaml
image: pgvector/pgvector:0.8.2-pg18
```

- [ ] **Step 2: Update volume mount path**

In the same `db` service, change the volume mount from:

```yaml
volumes:
    - fubbik_postgres_data:/var/lib/postgresql/data
```

to:

```yaml
volumes:
    - fubbik_postgres_data:/var/lib/postgresql
```

This is required because PG18's Docker image changed `PGDATA` to `/var/lib/postgresql/18/docker` (version-specific subdirectory). Mounting at the parent directory lets PG18 create its own subdirectory and also enables future `pg_upgrade` across major versions without mount boundary issues.

- [ ] **Step 3: Verify the file looks correct**

Run: `grep -A 15 'db:' docker-compose.yml | head -20`

Expected output should show `pgvector/pgvector:0.8.2-pg18` and `fubbik_postgres_data:/var/lib/postgresql`.

- [ ] **Step 4: Commit**

```bash
git add docker-compose.yml
git commit -m "chore: upgrade docker-compose db to PostgreSQL 18 + pgvector 0.8.2"
```

---

### Task 2: Update docker-compose.selfhost.yml

**Files:**
- Modify: `docker-compose.selfhost.yml:18,24`

- [ ] **Step 1: Update image tag**

In `docker-compose.selfhost.yml`, change the `db` service image from:

```yaml
image: pgvector/pgvector:0.8.0-pg16
```

to:

```yaml
image: pgvector/pgvector:0.8.2-pg18
```

- [ ] **Step 2: Update volume mount path**

Change the volume mount from:

```yaml
volumes:
    - fubbik_data:/var/lib/postgresql/data
```

to:

```yaml
volumes:
    - fubbik_data:/var/lib/postgresql
```

- [ ] **Step 3: Verify the file looks correct**

Run: `grep -A 10 'db:' docker-compose.selfhost.yml | head -12`

Expected output should show `pgvector/pgvector:0.8.2-pg18` and `fubbik_data:/var/lib/postgresql`.

- [ ] **Step 4: Commit**

```bash
git add docker-compose.selfhost.yml
git commit -m "chore: upgrade selfhost docker-compose db to PostgreSQL 18 + pgvector 0.8.2"
```

---

### Task 3: Update CLAUDE.md

**Files:**
- Modify: `CLAUDE.md`

- [ ] **Step 1: Update the Database section**

In `CLAUDE.md`, the Tech Stack > Database section currently reads:

```markdown
- Database: postgres
```

Change to:

```markdown
- Database: postgres (v18)
```

- [ ] **Step 2: Update the Extensions line**

The current line:

```markdown
- Extensions: pgvector (embeddings), pg_trgm (fuzzy text search)
```

Change to:

```markdown
- Extensions: pgvector 0.8.2 (embeddings), pg_trgm (fuzzy text search)
```

- [ ] **Step 3: Commit**

```bash
git add CLAUDE.md
git commit -m "docs: update CLAUDE.md to reflect PostgreSQL 18 + pgvector 0.8.2"
```

---

### Task 4: Update CONTRIBUTING.md

**Files:**
- Modify: `CONTRIBUTING.md:8`

- [ ] **Step 1: Update PostgreSQL prerequisite**

Change line 8 from:

```markdown
- PostgreSQL with pgvector extension
```

to:

```markdown
- PostgreSQL 18+ with pgvector extension
```

- [ ] **Step 2: Commit**

```bash
git add CONTRIBUTING.md
git commit -m "docs: update CONTRIBUTING.md PostgreSQL version requirement"
```

---

### Task 5: Update getting-started guide

**Files:**
- Modify: `docs/guide/getting-started.md` (check for any PostgreSQL version references)

- [ ] **Step 1: Read the file and find PostgreSQL references**

Run: `grep -n -i 'postgres\|pgvector\|pg16\|pg_' docs/guide/getting-started.md`

Update any version-specific references to reflect PostgreSQL 18. If the file references `pgvector/pgvector:0.8.0-pg16` or a specific PG version, update it to `pgvector/pgvector:0.8.2-pg18`. If it references the volume path `/var/lib/postgresql/data`, update to `/var/lib/postgresql`.

- [ ] **Step 2: Commit (if changes were made)**

```bash
git add docs/guide/getting-started.md
git commit -m "docs: update getting-started guide for PostgreSQL 18"
```

---

### Task 6: Create PostgreSQL upgrade migration guide

**Files:**
- Create: `docs/guide/upgrading-postgresql.md`

- [ ] **Step 1: Create the migration guide**

Create `docs/guide/upgrading-postgresql.md` with the following content:

```markdown
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
```

- [ ] **Step 2: Commit**

```bash
git add docs/guide/upgrading-postgresql.md
git commit -m "docs: add PostgreSQL upgrade guide (PG16 → PG18)"
```

---

### Task 7: Local dev verification (non-Docker)

If you run PostgreSQL locally (outside Docker, e.g., via Homebrew), this task
covers upgrading your local instance.

**Files:** None (operational task)

- [ ] **Step 1: Check current local PostgreSQL version**

Run: `psql --version`

If this already shows 18.x, skip to step 4.

- [ ] **Step 2: Dump local database**

```bash
pg_dumpall -U postgres > fubbik_local_backup.sql
```

- [ ] **Step 3: Upgrade PostgreSQL**

On macOS with Homebrew:

```bash
brew upgrade postgresql@18
```

Or if switching from postgresql@16:

```bash
brew install postgresql@18
brew services stop postgresql@16
brew services start postgresql@18
```

Then restore:

```bash
psql -U postgres < fubbik_local_backup.sql
```

And reindex:

```bash
psql -U postgres -d fubbik -c "REINDEX DATABASE fubbik;"
```

- [ ] **Step 4: Verify extensions**

```bash
psql -U postgres -d fubbik -c "SELECT extname, extversion FROM pg_extension;"
```

Should show `vector` and `pg_trgm`.

- [ ] **Step 5: Verify app starts**

```bash
pnpm dev
```

Hit `http://localhost:3000/api/health` — should return OK.
Run a search on `http://localhost:3001/search` to exercise pg_trgm.

---

### Task 8: Smoke test the full Docker stack

**Files:** None (operational task)

- [ ] **Step 1: Build and start from scratch**

```bash
docker compose down
docker volume rm fubbik_fubbik_postgres_data 2>/dev/null || true
docker compose up -d
```

- [ ] **Step 2: Wait for healthy**

```bash
docker compose ps
```

All services should show `healthy` / `running`.

- [ ] **Step 3: Verify PostgreSQL version**

```bash
docker exec fubbik-postgres psql -U postgres -c "SELECT version();"
```

Expected: `PostgreSQL 18.x`

- [ ] **Step 4: Verify extensions**

```bash
docker exec fubbik-postgres psql -U postgres -d fubbik -c "SELECT extname, extversion FROM pg_extension;"
```

Expected: `vector` (0.8.2) and `pg_trgm`.

- [ ] **Step 5: Verify app health endpoint**

```bash
curl http://localhost:3000/api/health
```

Expected: 200 OK.

- [ ] **Step 6: Seed and verify data flows**

```bash
docker compose down
SEED_DATABASE=true docker compose up -d
```

Wait for healthy, then:

```bash
# Check chunks exist
curl http://localhost:3000/api/stats

# Check semantic search (if Ollama available)
curl "http://localhost:3000/api/chunks/search/semantic?q=testing"
```

- [ ] **Step 7: Final commit (if any fixups were needed)**

```bash
git add -A
git commit -m "fix: address issues found during PG18 smoke test"
```
