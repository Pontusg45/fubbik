#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
version="${1:-}"
version="${version#v}"
target="${2:-$(rustc -vV | awk '/^host:/ {print $2}')}"
binary="${FUBBIK_RELEASE_BINARY:-${repo_root}/target/release/fubbik}"
web_dir="${FUBBIK_RELEASE_WEB_DIR:-${repo_root}/apps/web/dist}"
output_dir="${FUBBIK_RELEASE_OUTPUT_DIR:-${repo_root}/dist/release}"

if [[ ! "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+([-+][A-Za-z0-9.-]+)?$ ]]; then
    echo "Usage: $0 VERSION [RUST_TARGET] (VERSION must be a semantic version)" >&2
    exit 2
fi
if [[ ! -x "$binary" || ! -f "$web_dir/index.html" ]]; then
    echo "Build the Rust binary and static web app before packaging." >&2
    exit 1
fi
if [[ "$("$binary" --version)" != "fubbik $version" ]]; then
    echo "Binary version does not match requested release $version" >&2
    exit 1
fi

mkdir -p "$output_dir"
output_dir="$(cd "$output_dir" && pwd)"
bundle_name="fubbik-v${version}-${target}"
stage_root="$(mktemp -d)"
trap 'rm -r -- "$stage_root"' EXIT
bundle_dir="${stage_root}/${bundle_name}"
mkdir -p "${bundle_dir}/completions"
cp "$binary" "${bundle_dir}/fubbik"
cp -R "$web_dir" "${bundle_dir}/web"
cp "${repo_root}/LICENSE" "${bundle_dir}/LICENSE"
printf '%s\n' "$version" > "${bundle_dir}/VERSION"

for shell in bash zsh fish; do
    "$binary" completions "$shell" > "${bundle_dir}/completions/fubbik.${shell}"
done

cat > "${bundle_dir}/README.txt" <<'EOF'
Fubbik release archive

The fubbik executable provides the API server, CLI, MCP server, and database
maintenance commands. The web directory contains the static browser app.

Set DATABASE_URL and BETTER_AUTH_SECRET before running the server. From this
directory, run:

  FUBBIK_WEB_ASSETS_DIR="$PWD/web" ./fubbik serve

The server applies forward-only SQLx migrations on startup. Back up the
database before upgrading. See the repository README for backup and restore
instructions. Shell completion definitions are in completions/.
EOF

archive="${output_dir}/${bundle_name}.tar.gz"
tar -C "$stage_root" -czf "$archive" "$bundle_name"
(
    cd "$output_dir"
    shasum -a 256 "${bundle_name}.tar.gz" > "${bundle_name}.tar.gz.sha256"
)
echo "$archive"
