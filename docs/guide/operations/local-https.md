---
tags:
    - guide
    - operations
description: Optional local HTTPS with a Caddy reverse proxy, and the env vars that carry your hostnames
---

# Local HTTPS with Caddy

Optional. Plain `http://localhost:3001` (web) and `http://localhost:3000` (API) work without any of this. Use a reverse proxy when you want
HTTPS or friendly hostnames in local development.

## Recipe

Pick a domain you control locally, for example `<your-domain>.test`, and map it to Caddy:

- `app.<your-domain>.test:8443` -> `localhost:3001` (web)
- `api.<your-domain>.test:8443` -> `localhost:3000` (API)

1. Add both hostnames to `/etc/hosts`, pointing at `127.0.0.1`.
2. Add a site block per hostname to your Caddyfile (for example `~/.config/caddy/Caddyfile`) using `reverse_proxy localhost:<port>` and
   `tls internal`.
3. Start Caddy and trust its local CA (`caddy trust`).

## Tell fubbik about the hostnames

Set these in the gitignored root `.env` (see `.env.example`):

```bash
# Extra hostnames the Vite dev server accepts (comma-separated). localhost is always allowed.
FUBBIK_WEB_ALLOWED_HOSTS=app.<your-domain>.test

# Origins the API accepts for browser requests (comma-separated).
CORS_ORIGIN=http://localhost:3001,https://app.<your-domain>.test:8443
```

Restart the dev server after changing either value.

## Other personal settings in `.env`

- `FUBBIK_SEED_SPACE_PATH` - local path of the seeded code space. Defaults to the current directory when seeding.
- `FUBBIK_SEED_REMOTE_URL` - remote of the seeded code space. Defaults to `git remote get-url origin` in the current directory, or none.
