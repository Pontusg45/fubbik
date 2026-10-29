# Node contract fixtures — Rust phase 2a, task 1

Captured from the live Node server (`http://localhost:3000`) via
`scripts/capture-node-contract.sh`, GET requests only. Mutating endpoints
(POST/PATCH/DELETE) are documented from route/service/repository source in
`_mutating.md`, never executed against live data. See `_questions.md` for
the three required questions plus additional timestamp/casing/shape
findings.

## Files

- `_index.txt` — one line per captured GET request: name, HTTP status, content-type.
- `spaces-list.json` — `GET /api/spaces`
- `spaces-detail.json` — `GET /api/spaces/:id` (real seeded space id)
- `spaces-detect-nomatch.json` — `GET /api/spaces/detect?remoteUrl=<no match>`
- `spaces-detect-match.json` — `GET /api/spaces/detect?localPath=<matches seeded space>`
- `tags-list.json` — `GET /api/tags`
- `tag-types-list.json` — `GET /api/tag-types`
- `stats.json` — `GET /api/stats`
- `_mutating.md` — request/response shapes for all POST/PATCH/DELETE endpoints in scope, read from source.
- `_questions.md` — answers to the three required questions (envelope-vs-array, tag-type delete behavior, space reset behavior), plus timestamp format, field casing, null-vs-empty-array conventions, and the space detail/list/detect shape divergence.

## Headline findings (see `_questions.md` for full detail/evidence)

1. Spaces, tags, and tag-types list endpoints all return **bare JSON
   arrays** — none of them use the `{chunks, total, limit, offset}` envelope
   chunks uses. `/api/stats` returns a bare object.
2. `DELETE /api/tag-types/:id` **nulls out** referencing `tag.tag_type_id`
   values via a DB-level `ON DELETE SET NULL` foreign key
   (`packages/db/src/schema/tag.ts:27`) — no application code does this
   explicitly.
3. `POST /api/spaces/:id/reset` deletes the space's exclusively-owned
   chunks plus all space-scoped documents/plans/requirements, but leaves
   the space row and its `space_code_metadata` (remoteUrl/localPaths)
   intact. It's the same wipe `DELETE /api/spaces/:id` does as its first
   step, minus the final space-row delete.
4. `GET /api/spaces/:id` returns a **nested** `{ space: {...}, code: {...} }`
   shape, while `GET /api/spaces` (list) and `GET /api/spaces/detect`
   (match case) both return **bare** space objects with no code metadata at
   all — three different "space" shapes across three endpoints.
