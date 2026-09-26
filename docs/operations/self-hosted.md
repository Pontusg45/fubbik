# Self-hosted Rust operations

The production stack is one Rust server image, a Rust seed command, and
PostgreSQL 18 with pgvector and AGE. The server serves both `/api/*` and the
static browser application on the published port (4717 by default).

## Health and logs

```sh
docker compose -f docker-compose.selfhost.yml ps
curl --fail --silent http://localhost:4717/api/health
docker compose -f docker-compose.selfhost.yml logs --tail=200 server db
```

The container health check uses `/api/health`. A direct browser route such as
`/graph` should also return the HTML shell. For a sustained log stream, use
`docker compose -f docker-compose.selfhost.yml logs -f server`.

## Backup and restore

Create a PostgreSQL custom-format backup before every upgrade and retain it
outside the Compose volume:

```sh
docker compose -f docker-compose.selfhost.yml exec -T db \
  pg_dump -U fubbik -d fubbik -Fc > fubbik.backup
```

Verify the archive by restoring it to a disposable database on the same
PostgreSQL version:

```sh
docker compose -f docker-compose.selfhost.yml exec -T db createdb -U fubbik fubbik_restore_check
docker compose -f docker-compose.selfhost.yml exec -T db \
  pg_restore -U fubbik -d fubbik_restore_check --clean --if-exists < fubbik.backup
docker compose -f docker-compose.selfhost.yml exec -T db dropdb -U fubbik fubbik_restore_check
```

For a real restore, stop the server and seed service, restore into an empty
database or volume, and start the server against that restored database.
Restoring over an active server can race with writes and migrations.

## Seed and upgrade

Run the demo seed only when intended; it can reset selected fixture rows:

```sh
SEED_DATABASE=true docker compose -f docker-compose.selfhost.yml run --rm seed
```

To upgrade, take and verify a backup, record the currently deployed image
digest, then build or pull the intended server and seed images and restart the
stack. The Rust server applies forward-only SQLx migrations on startup. Check
`/api/health`, a browser route, and the server logs after the restart. Leave
`SEED_DATABASE` unset for ordinary upgrades.

To roll back application code, restore the previous image digest. If the new
server applied a schema migration, also restore the matching pre-upgrade
database backup: SQLx migrations are forward-only, so an old binary is not
assumed to understand a new schema.

## Release archives

`scripts/package-release.sh VERSION` bundles a version-matched Rust binary,
static web assets, Bash/Zsh/Fish completions, and a SHA-256 checksum. The
release workflow builds one archive on Linux and one on macOS. Verify an
archive before installation with `scripts/smoke-release-archive.sh PATH`.
Unpack it, place `fubbik` on `PATH`, and set `FUBBIK_WEB_ASSETS_DIR` to the
unpacked `web` directory when running `fubbik serve`. Configure
`DATABASE_URL` and `BETTER_AUTH_SECRET` first. The executable's `completions`
command can regenerate completion definitions for the installed version.
