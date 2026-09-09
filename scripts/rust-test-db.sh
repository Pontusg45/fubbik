#!/usr/bin/env bash
set -euo pipefail

readonly CONTAINER_NAME="fubbik-rust-test-db"
readonly AGE_IMAGE_NAME="fubbik-postgres:pg18-vector-age"
readonly WITHOUT_AGE_IMAGE_NAME="pgvector/pgvector:pg18"
readonly DATABASE_NAME="fubbik_rs"
readonly DATABASE_USER="postgres"
readonly DATABASE_PASSWORD="password"
readonly HOST_PORT="${FUBBIK_RUST_TEST_DB_PORT:-5434}"
readonly DATABASE_URL="postgresql://${DATABASE_USER}:${DATABASE_PASSWORD}@localhost:${HOST_PORT}/${DATABASE_NAME}"

image_for_profile() {
  case "$1" in
    age) echo "${AGE_IMAGE_NAME}" ;;
    without-age) echo "${WITHOUT_AGE_IMAGE_NAME}" ;;
    *) echo "Unknown Rust test database profile: $1" >&2; exit 2 ;;
  esac
}

build_image() {
  local profile="$1"
  local image_name
  image_name="$(image_for_profile "${profile}")"
  if ! docker image inspect "${image_name}" >/dev/null 2>&1; then
    if [[ "${profile}" == "age" ]]; then
      docker build -t "${image_name}" -f docker/postgres/Dockerfile .
    else
      docker pull "${image_name}" >/dev/null
    fi
  fi
}

verify() {
  local profile="$1"
  local deadline=$((SECONDS + 60))
  until docker exec "${CONTAINER_NAME}" psql -U "${DATABASE_USER}" -d "${DATABASE_NAME}" -Atc "SELECT 1" >/dev/null 2>&1; do
    if (( SECONDS >= deadline )); then
      docker logs "${CONTAINER_NAME}" >&2 || true
      echo "Rust test database did not become ready within 60 seconds" >&2
      exit 1
    fi
    sleep 1
  done

  local require_age="false"
  if [[ "${profile}" == "age" ]]; then require_age="true"; fi
  docker exec "${CONTAINER_NAME}" psql -v ON_ERROR_STOP=1 -U "${DATABASE_USER}" -d "${DATABASE_NAME}" -Atc \
    "SELECT CASE
       WHEN current_setting('server_version_num')::int / 10000 <> 18 THEN 1
       WHEN (SELECT datlocprovider::text FROM pg_database WHERE datname = current_database()) <> 'i' THEN 1
       WHEN NOT EXISTS (SELECT 1 FROM pg_available_extensions WHERE name = 'vector') THEN 1
       WHEN ${require_age} AND NOT EXISTS (SELECT 1 FROM pg_available_extensions WHERE name = 'age') THEN 1
       WHEN NOT ${require_age} AND EXISTS (SELECT 1 FROM pg_available_extensions WHERE name = 'age') THEN 1
       ELSE 0
     END" | grep -qx '0' || {
       echo "Rust test database does not satisfy profile ${profile}" >&2
       exit 1
     }
}

start() {
  local profile="$1"
  local image_name
  image_name="$(image_for_profile "${profile}")"
  build_image "${profile}"
  if docker container inspect "${CONTAINER_NAME}" >/dev/null 2>&1; then
    local current_profile
    current_profile="$(docker inspect -f '{{index .Config.Labels "fubbik.rust-test-profile"}}' "${CONTAINER_NAME}")"
    if [[ "${current_profile}" != "${profile}" ]]; then
      docker rm -f "${CONTAINER_NAME}" >/dev/null
    elif [[ "$(docker inspect -f '{{.State.Running}}' "${CONTAINER_NAME}")" != "true" ]]; then
      docker start "${CONTAINER_NAME}" >/dev/null
    fi
  fi
  if ! docker container inspect "${CONTAINER_NAME}" >/dev/null 2>&1; then
    docker run -d \
      --name "${CONTAINER_NAME}" \
      --label "fubbik.rust-test-profile=${profile}" \
      -e POSTGRES_DB="${DATABASE_NAME}" \
      -e POSTGRES_USER="${DATABASE_USER}" \
      -e POSTGRES_PASSWORD="${DATABASE_PASSWORD}" \
      -e POSTGRES_INITDB_ARGS="--locale-provider=icu --icu-locale=en-US" \
      -p "127.0.0.1:${HOST_PORT}:5432" \
      "${image_name}" >/dev/null
  fi
  verify "${profile}"
  echo "${DATABASE_URL}"
}

stop() {
  if docker container inspect "${CONTAINER_NAME}" >/dev/null 2>&1; then
    docker rm -f "${CONTAINER_NAME}" >/dev/null
  fi
}

case "${1:-}" in
  start) start "${2:-age}" ;;
  stop) stop ;;
  verify)
    profile="$(docker inspect -f '{{index .Config.Labels "fubbik.rust-test-profile"}}' "${CONTAINER_NAME}")"
    verify "${profile}"
    ;;
  url) echo "${DATABASE_URL}" ;;
  *)
    echo "Usage: $0 {start [age|without-age]|stop|verify|url}" >&2
    exit 2
    ;;
esac
