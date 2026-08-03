#!/usr/bin/env bash
# Records Node's response contract so the Rust port can be built against it
# rather than discovering divergences afterwards.
#
# READ-ONLY against the Node database: this script issues GET requests only.
# Mutating endpoints (POST/PATCH/PUT/DELETE) are documented by reading the
# route source (see tests/fixtures/node-contract/_mutating.md), never by
# executing them against the user's live knowledge base.
set -euo pipefail

BASE="${1:?usage: capture-node-contract.sh http://localhost:3000}"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
OUT="${REPO_ROOT}/tests/fixtures/node-contract"
mkdir -p "$OUT"

echo "==> Checking Node server is reachable at ${BASE}"
if ! curl -sf --max-time 10 "${BASE}/api/health" >/dev/null; then
  echo "ERROR: Node server not reachable at ${BASE}/api/health." >&2
  echo "This script never starts the Node server — it runs against the" >&2
  echo "user's live knowledge base and must already be running." >&2
  exit 1
fi

# capture <name> <path-and-query>
# Records HTTP status, content-type, and pretty-printed JSON body (or, for
# an empty body, a literal note that the body was empty).
capture() {
  local name="$1" path="$2"
  local tmp_body tmp_headers code ct

  tmp_body="$(mktemp)"
  tmp_headers="$(mktemp)"
  code=$(curl -s -o "$tmp_body" -D "$tmp_headers" -w '%{http_code}' --max-time 10 "$BASE$path")
  ct=$( (grep -i '^content-type:' "$tmp_headers" || true) | head -1 | tr -d '\r' | sed 's/^[Cc]ontent-[Tt]ype: *//')
  ct="${ct:-<none>}"

  if [[ -s "$tmp_body" ]]; then
    jq . "$tmp_body" > "$OUT/$name.json" 2>/dev/null || cp "$tmp_body" "$OUT/$name.json"
  else
    printf '<empty body>\n' > "$OUT/$name.json"
  fi

  rm -f "$tmp_body" "$tmp_headers"
  printf '%-28s %s %s\n' "$name" "$code" "$ct" | tee -a "$OUT/_index.txt"
}

: > "$OUT/_index.txt"

# --- spaces ---
capture spaces-list /api/spaces

# Detail and detect need a real space id / real remote-url or local-path to
# exercise a match. Discover the first "code" space from the list capture
# above rather than hardcoding an id that may not exist in every DB.
first_space_id=$(jq -r '.[0].id // empty' "$OUT/spaces-list.json" 2>/dev/null || true)
if [[ -n "$first_space_id" ]]; then
  capture spaces-detail "/api/spaces/$first_space_id"
else
  echo "WARN: no spaces found; skipping spaces-detail capture" >&2
fi

# detect: exercise both the no-match and match paths so the fixtures show
# what Node returns for each (empty body vs. a space object).
capture spaces-detect-nomatch "/api/spaces/detect?remoteUrl=https%3A%2F%2Fexample.com%2Fnonexistent.git"

first_local_path=$(jq -r '.code.localPaths[0] // empty' "$OUT/spaces-detail.json" 2>/dev/null || true)
if [[ -n "$first_local_path" ]]; then
  encoded_path=$(jq -rn --arg p "$first_local_path" '$p|@uri')
  capture spaces-detect-match "/api/spaces/detect?localPath=${encoded_path}"
else
  echo "WARN: no local path found on first space; skipping spaces-detect-match capture" >&2
fi

# --- tags ---
capture tags-list /api/tags

# --- tag-types ---
capture tag-types-list /api/tag-types

# --- stats ---
capture stats /api/stats

echo "==> Done. See $OUT/_index.txt"
