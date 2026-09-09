# Fubbik

> A local-first knowledge framework built for both humans and machines.

Fubbik is a proof-of-concept tool for storing, navigating, and evolving structured knowledge. Information lives as discrete **chunks** —
each with its own metadata, history, and relationships — giving you a foundation that works equally well whether you're querying from a
terminal or exploring visually in a GUI.

---

## Features

### 🧩 Chunk-Based Storage

Everything in Fubbik is a chunk. Chunks are self-contained units of information that can reference each other, be tagged, typed, and
composed into larger structures. No rigid schemas — just content with shape.

### 🤖 AI & Human Friendly

Fubbik ships with a **CLI** optimized for programmatic and AI-agent access — clean output, pipeable, scriptable. The **GUI** is built for
human exploration: browsing, editing, and visualizing your data without needing to touch a terminal.

### 🏠 Local First

Your data lives on your machine. No accounts, no cloud sync required. Fubbik is designed to work fully offline, with your knowledge base
stored in a portable, open format you own and control.

### 📋 Transaction Logs

Every change is recorded. Fubbik maintains an append-only transaction log so you can audit history, replay changes, or roll back to any
previous state. Nothing is silently overwritten.

### 🏷️ Dynamic Metadata

Chunks carry metadata that evolves with your needs. Add, remove, or reshape metadata fields at any time without migrations. Fubbik adapts to
your data model as it grows.

### 🕸️ Visualizable Knowledge Graphs

Relationships between chunks form a graph you can actually see. Fubbik's graph view lets you explore connections, spot clusters, and
navigate your knowledge base spatially — not just as a flat list.

## Status

Fubbik is currently a **proof of concept**. APIs, storage formats, and interfaces are subject to change.

---

## Getting Started

First, install the dependencies:

```bash
pnpm install
```

## Database Setup

This project uses PostgreSQL. The Rust server and its embedded SQLx migrations
are the schema authority. Drizzle is retained only as a typed sample-data seed
adapter and for read-only Studio inspection; do not generate or apply schema
changes through Drizzle.

When upgrading an existing database from the former Node/Drizzle server,
startup recognizes Drizzle's migration history, repairs the known five-table
behavior-matrix omission in that migration set, and adopts the resulting
verified 58-table schema as SQLx migration 1 before applying later migrations.
This preserves existing data. Any other missing table—or a schema without
recognized Drizzle bookkeeping—fails without changing migration history; back
up and finish the legacy migrations before retrying.

1. Make sure you have a PostgreSQL database set up.
2. Update your `apps/server/.env` file with your PostgreSQL connection details.

3. Run the development server (it applies pending migrations automatically):

```bash
pnpm run dev
```

Open [http://localhost:3001](http://localhost:3001) in your browser to see the web application. The API is running at
[http://localhost:3000](http://localhost:3000). API documentation (Swagger) is available at
[http://localhost:3000/docs](http://localhost:3000/docs).

The server port can be configured via the `PORT` environment variable (default: `3000`).

### Rust CLI

The `fubbik` binary now covers the primary server-backed workflows directly:

```bash
fubbik add "Architecture note" --content "..."
fubbik space list
fubbik tag list
fubbik link <source-id> <target-id> --relation supports
fubbik req list --status failing
fubbik docs import README.md
fubbik enrich --all
fubbik stale list
fubbik stats
```

These workflows support the global `--json` and `--quiet` output modes. The
compatibility aliases `codebase`, `tags`, and `requirements` are also accepted.

### Self-hosting

Start the Rust API, web UI, and PostgreSQL stack with:

```bash
docker compose -f docker-compose.selfhost.yml up -d
```

For a first-run demo dataset, set `SEED_DATABASE=true`. Seeding runs as a
one-shot tool after the Rust server has applied migrations; Node/Bun is not
present in the API runtime image.

### Self-hosted backups

The Compose stack keeps PostgreSQL data in the `fubbik_data` volume. Create a
portable backup before upgrades:

```bash
docker compose -f docker-compose.selfhost.yml exec -T db pg_dump -U fubbik -d fubbik -Fc > fubbik.backup
```

Verify that the backup can be restored into a disposable database:

```bash
docker compose -f docker-compose.selfhost.yml exec -T db createdb -U fubbik fubbik_restore_check
docker compose -f docker-compose.selfhost.yml exec -T db pg_restore -U fubbik -d fubbik_restore_check --clean --if-exists < fubbik.backup
docker compose -f docker-compose.selfhost.yml exec -T db dropdb -U fubbik fubbik_restore_check
```

Run the restore check periodically; a backup that has never been restored is
not yet a verified backup.

## Project Structure

```
fubbik/
├── apps/
│   ├── web/         # Frontend application (React + TanStack Start)
│   ├── server/      # Retired TypeScript reference (outside active workspace)
│   └── cli/         # Reference for remaining local discovery workflows
├── packages/
│   ├── api/         # Retired Elysia reference (outside active workspace)
│   ├── auth/        # Retired Better Auth reference (outside active workspace)
│   ├── client/      # Generated Rust OpenAPI client and shared wire contracts
│   ├── config/      # Shared TypeScript config
│   ├── db/          # Typed seed adapter and legacy schema reference
│   └── env/         # Environment validation (Arktype + t3-env)
├── crates/          # Primary Rust API, database, CLI, and core libraries
```

## Available Scripts

- `pnpm run dev`: Start the web app and Rust API
- `pnpm run build`: Build all applications
- `pnpm run dev:web`: Start only the web application
- `pnpm run dev:server`: Start only the Rust server
- `pnpm run check-types`: Type-check all packages (uses `tsgo`)
- `pnpm run db:studio`: Open database studio UI
- `just rust-test`: Run Rust tests with the canonical PG18/vector/AGE adapter
- `pnpm run ci`: Run the TypeScript CI pipeline
- `pnpm run test:e2e`: Run the browser critical-path suite

---

## License

MIT
