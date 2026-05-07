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
- `BETTER_AUTH_URL` — Auth server URL (e.g., `http://localhost:3000`)

## Optional

- `PORT` — Server port (default: `3000`)
- `CORS_ORIGIN` — Comma-separated allowed origins (default: `http://localhost:3001`)
- `OLLAMA_URL` — Ollama server URL (default: `http://localhost:11434`)
- `STALENESS_SCAN_INTERVAL_HOURS` — Hours between automatic staleness scans (default: `24`, set to `0` to disable)

## Local Development with Caddy

For HTTPS in local development, configure Caddy as a reverse proxy:

- `app.fubbik.test:8443` → `localhost:3001` (web)
- `api.fubbik.test:8443` → `localhost:3000` (API)

Configure in `~/.config/caddy/Caddyfile` and add domains to `/etc/hosts`.
