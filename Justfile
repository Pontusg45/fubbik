set shell := ["zsh", "-cu"]

# List available recipes.
default:
    @just --list

# Install workspace dependencies.
install:
    pnpm install

# Free dev ports, then start web (3001) and the Rust API (3000).
dev:
    @just kill-dev
    @just _dev-services

[parallel]
_dev-services: dev-web dev-rust

# Stop processes on the default dev ports.
kill-dev:
    -lsof -ti :3000 | xargs kill -9 2>/dev/null || true
    -lsof -ti :3001 | xargs kill -9 2>/dev/null || true

# Start only the Rust API server (replaces the Node/Bun server).
dev-server: dev-rust

dev-rust:
    #!/usr/bin/env zsh
    set -euo pipefail
    set -a
    source apps/server/.env
    set +a
    exec cargo run -p fubbik -- serve --port 3000

# Start only the web application (waits for the API health check first).
dev-web:
    #!/usr/bin/env zsh
    set -euo pipefail
    echo "Waiting for API at http://127.0.0.1:3000/api/health ..."
    for i in {1..120}; do
      if curl -sf http://127.0.0.1:3000/api/health >/dev/null; then
        echo "API ready."
        break
      fi
      if (( i == 120 )); then
        echo "API did not become ready in time." >&2
        exit 1
      fi
      sleep 0.5
    done
    exec pnpm run dev:web

# Build all applications and packages.
build:
    pnpm run build

# Type-check all applications and packages.
check-types:
    pnpm run check-types

# Run the test suite.
test:
    pnpm run test

# Start the canonical PostgreSQL 18 + vector + AGE Rust test adapter.
rust-test-db-start:
    ./scripts/rust-test-db.sh start

# Stop and remove the canonical Rust test database.
rust-test-db-stop:
    ./scripts/rust-test-db.sh stop

# Run the complete Rust suite against the canonical database adapter.
rust-test:
    #!/usr/bin/env zsh
    set -euo pipefail
    ./scripts/rust-test-db.sh start >/dev/null
    database_url="$(./scripts/rust-test-db.sh url)"
    trap './scripts/rust-test-db.sh stop' EXIT
    SQLX_OFFLINE=true DATABASE_URL="$database_url" cargo test --workspace

# Lint the workspace.
lint:
    pnpm run lint

# Format the workspace.
format:
    pnpm run fmt

# Check workspace formatting.
format-check:
    pnpm run fmt:check

# Run the full CI pipeline locally.
ci:
    pnpm run ci

# Start the development database.
db-start:
    pnpm run db:start

# Stop the development database.
db-stop:
    pnpm run db:stop

# Stop and remove the development database containers.
db-down:
    pnpm run db:down

# Open Drizzle Studio.
db-studio:
    pnpm run db:studio

# Seed the database.
seed:
    pnpm run seed

# Start the self-hosted Docker stack.
docker-up:
    pnpm run docker:up

# Stop the self-hosted Docker stack.
docker-down:
    pnpm run docker:down

# Follow logs from the self-hosted Docker stack.
docker-logs:
    pnpm run docker:logs
