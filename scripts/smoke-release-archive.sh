#!/usr/bin/env bash
set -euo pipefail

archive="${1:?Usage: $0 PATH_TO_RELEASE_ARCHIVE}"
archive="$(cd "$(dirname "$archive")" && pwd)/$(basename "$archive")"
archive_name="$(basename "$archive")"
bundle_name="${archive_name%.tar.gz}"
checksum="${archive}.sha256"

test -f "$checksum"
(
    cd "$(dirname "$archive")"
    shasum -a 256 -c "$(basename "$checksum")"
)

stage_root="$(mktemp -d)"
server_pid=""
server_log="${stage_root}/server.log"
cleanup() {
    result=$?
    if [[ -n "$server_pid" ]] && kill -0 "$server_pid" 2>/dev/null; then
        kill -TERM "$server_pid" 2>/dev/null || true
        wait "$server_pid" 2>/dev/null || true
    fi
    if (( result != 0 )) && [[ -s "$server_log" ]]; then
        cat "$server_log" >&2
    fi
    rm -r -- "$stage_root"
}
trap cleanup EXIT
tar -C "$stage_root" -xzf "$archive"
bundle_dir="${stage_root}/${bundle_name}"
test -x "${bundle_dir}/fubbik"
test -f "${bundle_dir}/web/index.html"
test -f "${bundle_dir}/LICENSE"
test -f "${bundle_dir}/VERSION"
for shell in bash zsh fish; do
    test -s "${bundle_dir}/completions/fubbik.${shell}"
done

expected_version="$(cat "${bundle_dir}/VERSION")"
[[ "$bundle_name" == "fubbik-v${expected_version}-"* ]]
test "$("${bundle_dir}/fubbik" --version)" = "fubbik ${expected_version}"
"${bundle_dir}/fubbik" openapi >/dev/null

if [[ -n "${FUBBIK_RELEASE_SMOKE_DATABASE_URL:-}" ]]; then
    port="${FUBBIK_RELEASE_SMOKE_PORT:-4718}"
    base="http://127.0.0.1:${port}"
    DATABASE_URL="$FUBBIK_RELEASE_SMOKE_DATABASE_URL" \
        BETTER_AUTH_SECRET="release-archive-smoke-secret-32-chars" \
        FUBBIK_IMPLICIT_DEV_SESSION=true \
        FUBBIK_WEB_ASSETS_DIR="${bundle_dir}/web" \
        "${bundle_dir}/fubbik" serve --port "$port" >"$server_log" 2>&1 &
    server_pid=$!
    ready=false
    for (( attempt = 0; attempt < 100; attempt++ )); do
        if curl --fail --silent "$base/api/health" >/dev/null; then
            ready=true
            break
        fi
        if ! kill -0 "$server_pid" 2>/dev/null; then
            echo "Release server exited before becoming healthy" >&2
            exit 1
        fi
        sleep 0.2
    done
    if [[ "$ready" != true ]]; then
        echo "Release server did not become healthy" >&2
        exit 1
    fi

    curl --fail --silent -H 'Accept: text/html' "$base/graph" | grep -q '<html'
    asset_path="$(curl --fail --silent -H 'Accept: text/html' "$base/" | grep -oE '/assets/[^" ]+\.js' | head -1)"
    test -n "$asset_path"
    curl --fail --silent "$base$asset_path" >/dev/null
    "${bundle_dir}/fubbik" --url "$base" --json stats >/dev/null

    kill -TERM "$server_pid"
    wait "$server_pid"
    server_pid=""
fi
echo "Release archive smoke passed: ${archive_name}"
