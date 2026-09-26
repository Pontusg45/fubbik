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
trap 'rm -r -- "$stage_root"' EXIT
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
echo "Release archive smoke passed: ${archive_name}"
