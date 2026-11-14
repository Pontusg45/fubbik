# Seed data, Ollama, environment variables

Reference for CLAUDE.md. Read when seeding, working on AI enrichment/embeddings, or adding/reading env vars.

## Seed Data

Run `pnpm seed` (= `cargo run --locked -p fubbik -- seed`) to populate the database with sample data. Seeding is native Rust
(`crates/fubbik-db/src/seed/`), built from modules with dependencies, selected by scenario:

- Every run first ensures the dev user (`dev@localhost`) exists, in one transaction with the selected modules
- `--scenario minimal`: `core` (built-in chunk types, relations, templates) and `self-documenting`
- `--scenario demo` (default) and `extended`: every module — `core`, `codebases` (the fubbik code space), `tags`, `chunks`, `connections`,
  `file-links`, `use-cases`, `requirements`, `plans`, `agent-coordination`, `documents`, `vocabulary`, `workspaces`, `collections`,
  `matrices`, `self-documenting`
- `--only <modules>` / `--skip <modules>` narrow the set (dependencies are pulled in; skipping a required one is an error);
  `--reset auto|none` controls clearing the selected modules first
- Fixture data lives in `crates/fubbik-db/src/seed/*-catalog.json`
- The seeded space's local path and remote URL come from `FUBBIK_SEED_SPACE_PATH` and `FUBBIK_SEED_REMOTE_URL`
- The seed does not call Ollama; run `fubbik enrich --all` afterwards for summaries, aliases and embeddings

## Ollama (Optional)

Required for chunk enrichment (summary, aliases, not_about generation), semantic search, and duplicate detection.

- `ollama pull nomic-embed-text` — embedding model
- `ollama pull llama3.2` — generation model for metadata
- `OLLAMA_URL` env var (default: `http://localhost:11434`)
- Without Ollama, all other features work normally
- Editing a chunk's title or content triggers a **full re-enrich**, not just an embedding
  refresh: `summary`, `aliases` and `not_about` are regenerated too, overwriting a
  hand-written summary. Fire-and-forget; failures are logged and never fail the request.
- Enrichment concurrency: 3 (for enrich-all), which also caps at 1000 chunks per sweep
- Ollama failures are handled four different ways, all matching the original TypeScript:
  `enrich` probes availability first and degrades to `null`; `GET /api/chunks/search/semantic`
  does not probe, so an unreachable Ollama surfaces as a 502; the `similar-to:` search
  clause degrades to an empty clause so one clause's outage cannot fail a whole query;
  and `suggest_vocabulary` is infallible — no probe, no error path, every failure mode
  collapses to `[]`.

## Environment Variables

Personal settings live in `.env` (see `docs/guide/getting-started/configuration/environment.md`); project feature switches live in
`fubbik.toml` (see "Feature switches").

- `PORT` — Server port for `fubbik serve` (default: `3000`)
- `HOST` — Bind address for `fubbik serve` (default: `127.0.0.1`)
- `CORS_ORIGIN` — Comma-separated allowed origins (default: `http://localhost:3001`)
- `DATABASE_URL` — PostgreSQL connection string (required by `serve` and `seed`)
- `BETTER_AUTH_SECRET` — Auth secret (min 32 chars); the Rust server uses it to verify session cookies
- `BETTER_AUTH_URL` — Auth server URL. Validated by `packages/env`; the Rust server does not read it (it accepts both the plain and the
  `__Secure-` session cookie names)
- `OLLAMA_URL` — Ollama server URL (default: `http://localhost:11434`)
- `STALENESS_SCAN_INTERVAL_HOURS` — Hours between automatic staleness scans (default: `24`, set to `0` to disable)
- `BEHAVIOR_GRAPH_SYNC_INTERVAL_HOURS` — Hours between behavior-rule projections into the AGE graph (default: `24`, set to `0` to
  disable). Not started when `matrices` is off.
- `NODE_ENV` / `FUBBIK_IMPLICIT_DEV_SESSION` — unless `NODE_ENV=production`, or whenever `FUBBIK_IMPLICIT_DEV_SESSION=true`, the server
  serves unauthenticated requests as the dev user (it logs a warning at startup). Set `NODE_ENV=production` to require real sessions
- `FUBBIK_CONFIG` — Path to `fubbik.toml` (default: `./fubbik.toml`)
- `FUBBIK_URL` — API URL for CLI commands; `FUBBIK_SERVER_URL` — API URL for `fubbik mcp`
- `FUBBIK_SEED_SPACE_PATH` / `FUBBIK_SEED_REMOTE_URL` — Local path / remote URL of the seeded code space (defaults: the current
  directory / its `origin` remote)
- `FUBBIK_WEB_ALLOWED_HOSTS` — Comma-separated extra hostnames for the Vite dev server's `server.allowedHosts`

