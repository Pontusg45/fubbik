#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

export COMPOSE_PROJECT_NAME="fubbik-smoke-$$"
export FUBBIK_API_PORT="${FUBBIK_SMOKE_PORT:-4717}"
compose=(docker compose -f docker-compose.selfhost.yml)

cleanup() {
    result=$?
    if (( result != 0 )); then
        "${compose[@]}" logs --no-color >&2 || true
    fi
    "${compose[@]}" down --volumes --remove-orphans >/dev/null 2>&1 || true
    exit "$result"
}
trap cleanup EXIT

"${compose[@]}" build server seed

for service in server seed; do
    "${compose[@]}" run --rm --no-deps --entrypoint sh "$service" -ec '
        for binary in node npm pnpm bun; do
            if command -v "$binary" >/dev/null 2>&1; then
                echo "unexpected runtime binary: $binary" >&2
                exit 1
            fi
        done
        for directory in /app/apps /app/packages /app/node_modules /app/deploy; do
            if [ -e "$directory" ]; then
                echo "unexpected legacy runtime directory: $directory" >&2
                exit 1
            fi
        done
        if find /app -type f \( -name "*.ts" -o -name "*.tsx" \) | grep -q .; then
            echo "TypeScript source found in runtime image" >&2
            exit 1
        fi
        test "$(id -u)" != 0
    '
done

"${compose[@]}" up -d --wait db server
SEED_DATABASE=true "${compose[@]}" run --rm seed

base="http://127.0.0.1:${FUBBIK_API_PORT}"
curl --fail --silent --show-error "$base/api/health" >/dev/null
curl --fail --silent --show-error -H 'Accept: text/html' "$base/" | grep -q '<html'
curl --fail --silent --show-error -H 'Accept: text/html' "$base/graph" | grep -q '<html'
curl --fail --silent --show-error -H 'Accept: text/html' "$base/plans/example" | grep -q '<html'
asset_path="$(curl --fail --silent --show-error "$base/" | grep -oE '/assets/[^" ]+\.js' | head -1)"
test -n "$asset_path"
curl --fail --silent --show-error "$base$asset_path" >/dev/null
test "$(curl --silent --output /dev/null --write-out '%{http_code}' -H 'Accept: text/html' "$base/api/unknown")" = 404

"${compose[@]}" exec -T server fubbik --url http://127.0.0.1:3000 --json stats >/dev/null
seeded="$("${compose[@]}" exec -T db psql -U fubbik -d fubbik -Atqc 'select count(*) from chunk')"
test "$seeded" -gt 0

"${compose[@]}" stop server
container_id="$("${compose[@]}" ps -a -q server)"
test "$(docker inspect --format '{{.State.ExitCode}}' "$container_id")" = 0
echo "Production image smoke passed: Rust API, static web, CLI, seed, and shutdown"
