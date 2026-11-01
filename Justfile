set dotenv-load := false
set shell := ["bash", "-euo", "pipefail", "-c"]

# Share one cargo target dir across every worktree: the main checkout's target/.
# `--git-common-dir` is <main>/.git from the main checkout and from any worktree.
export CARGO_TARGET_DIR := env("CARGO_TARGET_DIR", parent_directory(shell('git rev-parse --path-format=absolute --git-common-dir 2>/dev/null || echo "$1"', justfile_directory() / ".git")) / "target")

# Show grouped commands.
default:
    @just --groups

# Install workspace dependencies.
[group('Development')]
install:
    pnpm install

# Free the dev ports, then start the API (3000) and web (3001) in parallel (Ctrl+C stops all).
[group('Development')]
dev: kill-dev
    #!/usr/bin/env bash
    trap 'kill 0' EXIT
    just start-api &
    just start-web &
    wait

# Stop processes on the default dev ports. Stops instead if the desktop app holds :3000 (its built-in server).
[group('Development')]
kill-dev:
    #!/usr/bin/env bash
    set -euo pipefail
    holder="$( (lsof -nP -iTCP:3000 -sTCP:LISTEN -Fc 2>/dev/null || true) | sed -n 's/^c//p' | head -1)"
    if [[ "$holder" == fubbik-desktop* ]]; then
        echo "The fubbik desktop app serves :3000 (its built-in server). Quit it first, or use its server as is." >&2
        exit 1
    fi
    cargo run -q -p kit -- port kill 3000 3001

# Builds, then `exec`s the binary so this terminal's foreground process is the
# server itself: Ctrl+C reaches it directly (no leftover just/cargo wrapper that
# can die and orphan the server on :3000).
#
# Start the Rust API server on 3000, with .env loaded.
[group('Development')]
start-api:
    #!/usr/bin/env bash
    set -euo pipefail
    echo "🚀 Starting the fubbik API..."
    set -a
    source .env
    set +a
    cargo build -p fubbik
    exec "$CARGO_TARGET_DIR/debug/fubbik" serve --port 3000

# Start the web app on 3001 (waits for the API health check first).
[group('Development')]
start-web:
    #!/usr/bin/env bash
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
    echo "🌐 Starting the web app..."
    exec pnpm run dev:web

alias dev-server := start-api
alias dev-rust := start-api
alias dev-web := start-web

# Its embedded terminal (libghostty-vt 0.2.2) builds with Zig 0.15.2: `brew install zig@0.15`.
#
# Start the GPUI desktop app (its own cargo workspace in apps/desktop).
[group('Development')]
start-desktop:
    PATH="/opt/homebrew/opt/zig@0.15/bin:$PATH" SQLX_OFFLINE=true cargo run --manifest-path apps/desktop/Cargo.toml

alias desktop := start-desktop

# Build the desktop app as ~/Applications/fubbik.app and pin it to the Dock.
[group('Development')]
desktop-install:
    ./apps/desktop/scripts/install-app.sh

# Build all applications and packages.
[group('Build')]
build:
    pnpm run build

# Type-check all applications and packages.
[group('Build')]
check-types:
    pnpm run check-types

# Run the test suite (starts the Rust test database first; `@fubbik/db` tests need it).
[group('Test')]
test: rust-test-db-start
    pnpm run test

# Start the canonical PostgreSQL 18 + vector + AGE Rust test adapter.
[group('Test')]
rust-test-db-start:
    ./scripts/test/rust-test-db.sh start

# Stop and remove the canonical Rust test database.
[group('Test')]
rust-test-db-stop:
    ./scripts/test/rust-test-db.sh stop

# Run the complete Rust suite against the canonical database adapter.
[group('Test')]
rust-test:
    ./scripts/test/test-rust.sh

alias test-rust := rust-test

# Run database-free Rust libraries and CLI HTTP contract tests.
[group('Test')]
rust-test-fast:
    cargo fmt --all --check
    CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 SQLX_OFFLINE=true cargo test -p fubbik-contracts -p fubbik-cli -p fubbik-mcp -p review-harness -p kit

alias test-rust-fast := rust-test-fast

# Test only crates changed since a base (default main) plus reverse deps. Flags: --dry-run.
[group('Test')]
test-changed *args:
    ./scripts/test/test-changed.sh {{args}}

# Lint the workspace.
[group('Build')]
lint:
    pnpm run lint

# Format the workspace.
[group('Build')]
format:
    pnpm run fmt

# Check workspace formatting.
[group('Build')]
format-check:
    pnpm run fmt:check

# Run the full CI pipeline locally (starts the Rust test database first).
[group('Build')]
ci: rust-test-db-start
    pnpm run ci

# Start the development database.
[group('Database')]
db-start:
    pnpm run db:start

# Stop the development database.
[group('Database')]
db-stop:
    pnpm run db:stop

# Stop and remove the development database containers.
[group('Database')]
db-down:
    pnpm run db:down

# Open Drizzle Studio.
[group('Database')]
db-studio:
    pnpm run db:studio

# Seed the database.
[group('Database')]
seed:
    pnpm run seed

# Start the self-hosted Docker stack.
[group('Docker')]
docker-up:
    pnpm run docker:up

# Stop the self-hosted Docker stack.
[group('Docker')]
docker-down:
    pnpm run docker:down

# Follow logs from the self-hosted Docker stack.
[group('Docker')]
docker-logs:
    pnpm run docker:logs
