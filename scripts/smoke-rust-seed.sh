#!/usr/bin/env bash
set -euo pipefail

readonly IMAGE_NAME="fubbik-seed-smoke:local"
readonly TEST_DB_NAME="fubbik_seed_smoke_$$"
readonly TEST_DB_ADMIN_URL="postgresql://postgres:password@localhost:${FUBBIK_RUST_TEST_DB_PORT:-5434}/postgres"
readonly TEST_DB_URL="postgresql://postgres:password@localhost:${FUBBIK_RUST_TEST_DB_PORT:-5434}/${TEST_DB_NAME}"

cleanup() {
  psql "${TEST_DB_ADMIN_URL}" -v ON_ERROR_STOP=1 -c "DROP DATABASE IF EXISTS ${TEST_DB_NAME} WITH (FORCE)" >/dev/null || true
}
trap cleanup EXIT

./scripts/rust-test-db.sh start >/dev/null
psql "${TEST_DB_ADMIN_URL}" -v ON_ERROR_STOP=1 -c "CREATE DATABASE ${TEST_DB_NAME} TEMPLATE template0 LOCALE_PROVIDER icu ICU_LOCALE 'en-US'" >/dev/null
docker build -t "${IMAGE_NAME}" -f docker/build/seed.Dockerfile .

docker run --rm --entrypoint sh "${IMAGE_NAME}" -c '
  for binary in node npm pnpm bun; do
    if command -v "$binary" >/dev/null 2>&1; then
      echo "Unexpected runtime executable: $binary" >&2
      exit 1
    fi
  done
'

for run in 1 2; do
  docker run --rm \
    --network container:fubbik-rust-test-db \
    -e "DATABASE_URL=postgresql://postgres:password@localhost:5432/${TEST_DB_NAME}" \
    "${IMAGE_NAME}" fubbik seed --quiet
  counts="$(psql "${TEST_DB_URL}" -At -v ON_ERROR_STOP=1 -c "SELECT (SELECT count(*) FROM chunk), (SELECT count(*) FROM chunk_connection), (SELECT count(*) FROM behavior_cell)")"
  if [[ "${counts}" != "52|64|184" ]]; then
    echo "Seed run ${run} produced unexpected counts: ${counts}" >&2
    exit 1
  fi
done

echo "Rust seed image passed runtime inspection and two idempotent demo runs."
