---
tags:
    - guide
    - configuration
    - environment
description: Required and optional environment variables
---

# Environment Variables

## Required

- `DATABASE_URL` — PostgreSQL connection string (e.g., `postgresql://user:pass@localhost:5432/fubbik`)
- `BETTER_AUTH_SECRET` — Auth secret (min 32 chars, generate with `openssl rand -base64 32`)

## Optional

- `PORT` — Server port (default: `3000`)
- `BETTER_AUTH_URL` — Not read by the Rust server; it is only used by the retired TypeScript code in `legacy/`, so you can leave it unset
- `CORS_ORIGIN` — Comma-separated allowed origins (default: `http://localhost:3001`)
- `OLLAMA_URL` — Ollama server URL (default: `http://localhost:11434`)
- `STALENESS_SCAN_INTERVAL_HOURS` — Hours between automatic staleness scans (default: `24`, set to `0` to disable)
- `FUBBIK_SEED_SPACE_PATH` — Local path of the seeded code space (default: the canonicalised current directory)
- `FUBBIK_SEED_REMOTE_URL` — Remote URL of the seeded code space (default: `git remote get-url origin` in the current directory, or none)
- `FUBBIK_WEB_ALLOWED_HOSTS` — Comma-separated extra hostnames for the Vite dev server's `server.allowedHosts`, e.g. `app.example.test`
  (default: none; localhost is always allowed)

## Local Development with Caddy

Optional HTTPS via a Caddy reverse proxy: see [Local HTTPS](../../operations/local-https.md).
