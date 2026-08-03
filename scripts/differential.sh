#!/usr/bin/env bash
# Runs the differential harness against both stacks.
#
# READ-ONLY on the Node side. This script's only interaction with the Node
# database is a single `pg_dump` FROM $DATABASE_URL. It must never `psql`,
# `INSERT`, `UPDATE`, or otherwise write to $DATABASE_URL — doing so would
# risk corrupting the user's live knowledge base. Everything this script
# creates, migrates, truncates, and restores into lives in a dedicated
# comparison database (`fubbik_diff`) on the Rust Postgres container,
# entirely separate from `fubbik_rs` (which may hold local dev/test data)
# and from the Node database itself.
#
# Prerequisites this script does NOT start for you:
#   - The Node server running and reachable (default http://localhost:3000).
#     Never started here — it runs against the user's live knowledge base
#     and must be started by hand.
#   - The fubbik-rs-db container (port 5434) up and running.
set -euo pipefail

if [[ -z "${DATABASE_URL:-}" ]]; then
  echo "ERROR: DATABASE_URL is not set." >&2
  echo "It must point at the Node database — this script reads it once via" >&2
  echo "pg_dump and never writes to it. Example:" >&2
  echo '  DATABASE_URL="postgres://user@localhost:5432/fubbik" ./scripts/differential.sh' >&2
  exit 1
fi

NODE_URL="${FUBBIK_NODE_URL:-http://localhost:3000}"
RUST_URL="${FUBBIK_RUST_URL:-http://localhost:3100}"
RUST_ADMIN_URL="postgres://postgres:password@localhost:5434/postgres"
DIFF_DB="fubbik_diff"
DIFF_DB_URL="postgres://postgres:password@localhost:5434/${DIFF_DB}"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
MIGRATIONS_DIR="${REPO_ROOT}/crates/fubbik-db/migrations"

echo "==> Recreating ${DIFF_DB} (dedicated comparison database, ICU locale provider)"
# WITH (FORCE) drops the database even if a previous run's Rust server is
# still holding connections open (e.g. this script was interrupted).
# `fubbik_diff` is separate from `fubbik_rs` on purpose: `fubbik_rs` already
# holds migration 0002's reference rows plus whatever local dev/test data
# exists, and a full data-only load on top of that collides on primary
# keys. Ordering here depends on ICU collation (see
# fubbik_db::warn_if_not_icu_collation), so the database is created off
# template0 with the ICU provider explicitly rather than inheriting
# whatever the cluster defaults to.
psql "${RUST_ADMIN_URL}" -v ON_ERROR_STOP=1 -c "DROP DATABASE IF EXISTS ${DIFF_DB} WITH (FORCE);"
psql "${RUST_ADMIN_URL}" -v ON_ERROR_STOP=1 -c "CREATE DATABASE ${DIFF_DB} LOCALE_PROVIDER icu ICU_LOCALE 'en-US' TEMPLATE template0;"

echo "==> Running migrations against ${DIFF_DB}"
sqlx migrate run --source "${MIGRATIONS_DIR}" --database-url "${DIFF_DB_URL}"

echo "==> Clearing migration 0002's seeded reference rows before the data load"
# Migration 0002 seeds chunk_type, connection_relation, and space_kind, and
# the Node dump below also contains rows for all three — without this,
# the data load collides on primary keys.
psql "${DIFF_DB_URL}" -v ON_ERROR_STOP=1 \
  -c "TRUNCATE chunk_type, connection_relation, space_kind CASCADE;"

echo "==> Loading data from the Node database (read-only: DATABASE_URL is only ever read)"
# Default (plain/COPY) format, NOT --column-inserts: --column-inserts was
# measured to take over 7 minutes on this data volume and timed out;
# COPY-format restore is dramatically faster.
#
# --exclude-table drops account/verification: the Node schema has these
# tables, the Rust schema deliberately does not, and their INSERTs would
# otherwise fail the restore outright.
#
# `set -o pipefail` (enabled above) makes this pipeline's exit status
# non-zero if pg_dump fails, even though psql (the last command) may still
# exit 0 having received a truncated stream — without it, a failed pg_dump
# could silently leave fubbik_diff full of nothing but migration-seeded
# rows, and the comparison below would then "pass" by finding nothing to
# disagree on.
pg_dump --data-only --no-owner \
  --exclude-table=account --exclude-table=verification \
  "${DATABASE_URL}" \
  | psql "${DIFF_DB_URL}" -v ON_ERROR_STOP=1 >/dev/null

# Belt-and-suspenders on top of pipefail: a pg_dump that "succeeds" but
# emits no rows (e.g. DATABASE_URL points at an empty database) would
# still leave the pipeline's exit code at 0. Fail loudly instead of
# silently comparing two empty stacks.
loaded_users="$(psql "${DIFF_DB_URL}" -v ON_ERROR_STOP=1 -Atc 'SELECT count(*) FROM "user";')"
if [[ "${loaded_users}" -eq 0 ]]; then
  echo "ERROR: the data load produced zero rows in \"user\"." >&2
  echo "DATABASE_URL likely points at an empty or wrong database — refusing" >&2
  echo "to run a comparison against an effectively empty ${DIFF_DB}." >&2
  exit 1
fi
echo "==> Loaded data for ${loaded_users} user(s)"

echo "==> Checking the Node server is reachable at ${NODE_URL}"
if ! curl -sf "${NODE_URL}/api/health" >/dev/null; then
  echo "ERROR: Node server not reachable at ${NODE_URL}/api/health." >&2
  echo "This script never starts the Node server — it runs against the" >&2
  echo "user's live knowledge base and must be started by hand (e.g." >&2
  echo "'pnpm dev' from the repo root, or set FUBBIK_NODE_URL to point at" >&2
  echo "wherever it's already running)." >&2
  echo "Refusing to start the Rust server and run a comparison against a" >&2
  echo "dead Node server — that would produce a meaningless result." >&2
  exit 1
fi

echo "==> Starting the Rust server against ${DIFF_DB}"
DATABASE_URL="${DIFF_DB_URL}" \
  FUBBIK_IMPLICIT_DEV_SESSION=true \
  cargo run --quiet -p fubbik -- serve &
RUST_PID=$!
trap 'kill "${RUST_PID}" 2>/dev/null || true' EXIT

until curl -sf "${RUST_URL}/api/health" >/dev/null; do sleep 0.5; done

echo "==> Diffing"
FUBBIK_NODE_URL="${NODE_URL}" \
  FUBBIK_RUST_URL="${RUST_URL}" \
  cargo test -p fubbik-api --test differential -- --ignored --nocapture
