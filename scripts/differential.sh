#!/usr/bin/env bash
# Runs the differential harness against both stacks.
# Assumes the Node server is on :3000 and seeds the Rust database to match.
set -euo pipefail

echo "==> Seeding fubbik_rs from the Node database"
pg_dump --data-only --no-owner "${DATABASE_URL}" \
  | psql "postgres://postgres:password@localhost:5434/fubbik_rs" >/dev/null

echo "==> Starting the Rust server"
DATABASE_URL="postgres://postgres:password@localhost:5434/fubbik_rs" \
  FUBBIK_IMPLICIT_DEV_SESSION=true \
  cargo run --quiet -- serve &
RUST_PID=$!
trap 'kill $RUST_PID 2>/dev/null || true' EXIT

until curl -sf http://localhost:3100/api/health >/dev/null; do sleep 0.5; done

echo "==> Diffing"
FUBBIK_NODE_URL=http://localhost:3000 \
  FUBBIK_RUST_URL=http://localhost:3100 \
  cargo test -p fubbik-api --test differential -- --ignored --nocapture
