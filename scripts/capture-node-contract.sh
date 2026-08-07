#!/usr/bin/env bash
# Records Node's response contract so the Rust port can be built against it
# rather than discovering divergences afterwards.
#
# READ-ONLY against the Node database: this script issues GET requests only.
# Mutating endpoints (POST/PATCH/PUT/DELETE) are documented by reading the
# route source (see tests/fixtures/node-contract/_mutating.md and
# tests/fixtures/node-contract-2b/_mutating.md), never by executing them
# against the user's live knowledge base.
#
# Phase 2a captured spaces/tags/tag-types/stats into
# tests/fixtures/node-contract/. Phase 2b extends this same script (rather
# than adding a second one) to capture notifications/settings/workspaces/
# favorites/collections/activity into tests/fixtures/node-contract-2b/.
set -euo pipefail

BASE="${1:?usage: capture-node-contract.sh http://localhost:3000}"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
OUT="${REPO_ROOT}/tests/fixtures/node-contract"
OUT2B="${REPO_ROOT}/tests/fixtures/node-contract-2b"
mkdir -p "$OUT" "$OUT2B"

echo "==> Checking Node server is reachable at ${BASE}"
if ! curl -sf --max-time 10 "${BASE}/api/health" >/dev/null; then
  echo "ERROR: Node server not reachable at ${BASE}/api/health." >&2
  echo "This script never starts the Node server — it runs against the" >&2
  echo "user's live knowledge base and must already be running." >&2
  exit 1
fi

# capture <outdir> <name> <path-and-query>
# Records HTTP status, content-type, and pretty-printed JSON body (or, for
# an empty body, a literal note that the body was empty) into <outdir>.
capture() {
  local outdir="$1" name="$2" path="$3"
  local tmp_body tmp_headers code ct

  tmp_body="$(mktemp)"
  tmp_headers="$(mktemp)"
  code=$(curl -s -o "$tmp_body" -D "$tmp_headers" -w '%{http_code}' --max-time 10 "$BASE$path")
  ct=$( (grep -i '^content-type:' "$tmp_headers" || true) | head -1 | tr -d '\r' | sed 's/^[Cc]ontent-[Tt]ype: *//')
  ct="${ct:-<none>}"

  if [[ -s "$tmp_body" ]]; then
    jq . "$tmp_body" > "$outdir/$name.json" 2>/dev/null || cp "$tmp_body" "$outdir/$name.json"
  else
    printf '<empty body>\n' > "$outdir/$name.json"
  fi

  rm -f "$tmp_body" "$tmp_headers"
  printf '%-28s %s %s\n' "$name" "$code" "$ct" | tee -a "$outdir/_index.txt"
}

: > "$OUT/_index.txt"

# --- spaces ---
capture "$OUT" spaces-list /api/spaces

# Detail and detect need a real space id / real remote-url or local-path to
# exercise a match. Discover the first "code" space from the list capture
# above rather than hardcoding an id that may not exist in every DB.
first_space_id=$(jq -r '.[0].id // empty' "$OUT/spaces-list.json" 2>/dev/null || true)
if [[ -n "$first_space_id" ]]; then
  capture "$OUT" spaces-detail "/api/spaces/$first_space_id"
else
  echo "WARN: no spaces found; skipping spaces-detail capture" >&2
fi

# detect: exercise both the no-match and match paths so the fixtures show
# what Node returns for each (empty body vs. a space object).
capture "$OUT" spaces-detect-nomatch "/api/spaces/detect?remoteUrl=https%3A%2F%2Fexample.com%2Fnonexistent.git"

first_local_path=$(jq -r '.code.localPaths[0] // empty' "$OUT/spaces-detail.json" 2>/dev/null || true)
if [[ -n "$first_local_path" ]]; then
  encoded_path=$(jq -rn --arg p "$first_local_path" '$p|@uri')
  capture "$OUT" spaces-detect-match "/api/spaces/detect?localPath=${encoded_path}"
else
  echo "WARN: no local path found on first space; skipping spaces-detect-match capture" >&2
fi

# --- tags ---
capture "$OUT" tags-list /api/tags

# --- tag-types ---
capture "$OUT" tag-types-list /api/tag-types

# --- stats ---
capture "$OUT" stats /api/stats

echo "==> Done. See $OUT/_index.txt"

# ============================================================================
# Phase 2b: notifications, settings, workspaces, favorites, collections,
# activity
# ============================================================================

: > "$OUT2B/_index.txt"

# --- notifications ---
capture "$OUT2B" notifications-list /api/notifications
capture "$OUT2B" notifications-list-unread-only "/api/notifications?unreadOnly=true"
capture "$OUT2B" notifications-count /api/notifications/count

# --- settings ---
capture "$OUT2B" settings-features /api/settings/features
capture "$OUT2B" settings-user /api/settings/user
capture "$OUT2B" settings-instance /api/settings/instance

# codebase settings need a real spaceId; reuse the phase-2a spaces capture if
# present, otherwise ask Node directly.
codebase_settings_space_id=$(jq -r '.[0].id // empty' "$OUT/spaces-list.json" 2>/dev/null || true)
if [[ -z "$codebase_settings_space_id" ]]; then
  codebase_settings_space_id=$(curl -s --max-time 10 "$BASE/api/spaces" | jq -r '.[0].id // empty' 2>/dev/null || true)
fi
if [[ -n "$codebase_settings_space_id" ]]; then
  capture "$OUT2B" settings-codebase "/api/settings/codebase?codebaseId=${codebase_settings_space_id}"
else
  echo "WARN: no space found; skipping settings-codebase capture" >&2
fi

# --- workspaces ---
capture "$OUT2B" workspaces-list /api/workspaces

first_workspace_id=$(jq -r '.[0].id // empty' "$OUT2B/workspaces-list.json" 2>/dev/null || true)
if [[ -n "$first_workspace_id" ]]; then
  capture "$OUT2B" workspaces-detail "/api/workspaces/$first_workspace_id"
else
  echo "WARN: no workspaces found; skipping workspaces-detail capture" >&2
fi

# --- favorites ---
capture "$OUT2B" favorites-list /api/favorites

# --- collections ---
capture "$OUT2B" collections-list /api/collections

# Capture /collections/{id}/chunks for every distinct filter shape present
# (the DB currently has one collection filtered by `type` and one by
# `tags` — capturing both shows how each filter key is evaluated). Name each
# fixture after its filter keys (sorted, joined by "-") rather than a
# positional index, so the fixture name is self-describing and stable.
collection_rows=$(jq -c '.[] | {id, keys: ([.filter | keys[]] | sort | join("-"))}' "$OUT2B/collections-list.json" 2>/dev/null || true)
if [[ -n "$collection_rows" ]]; then
  while IFS= read -r row; do
    cid=$(jq -r '.id' <<< "$row")
    keys=$(jq -r '.keys' <<< "$row")
    keys="${keys:-no-filter}"
    capture "$OUT2B" "collections-chunks-filter-${keys}" "/api/collections/${cid}/chunks"
  done <<< "$collection_rows"
else
  echo "WARN: no collections found; skipping collections-chunks captures" >&2
fi

# --- activity ---
capture "$OUT2B" activity-list /api/activity

echo "==> Done. See $OUT2B/_index.txt"
