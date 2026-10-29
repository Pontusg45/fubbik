# Three required questions — answered with evidence

## Q1: Bare array or envelope, per list endpoint?

Chunks (Phase 1, for reference) returns `{chunks, total, limit, offset}`.
**None of the three list endpoints in this slice do that — all three are bare
arrays**, and stats is a bare object (not paginated, no list semantics).
Verbatim from the captured fixtures:

- `GET /api/spaces` → `spaces-list.json`, top-level value is `[ {...}, {...} ]` (a JSON array, 2 elements in this DB).
- `GET /api/tags` → `tags-list.json`, top-level value is `[ {...}, ... ]` (a JSON array).
- `GET /api/tag-types` → `tag-types-list.json`, top-level value is `[ {...}, ... ]` (a JSON array).
- `GET /api/stats` → `stats.json`, top-level value is `{"chunks":65,"connections":64,"tags":64}` — a bare object, no wrapping.

Source confirms this isn't accidental — none of `listSpaces`, `getUserTags`,
`listTagTypes` do any pagination/limiting:
- `packages/api/src/spaces/service.ts:18-20` `listSpaces` → `listSpacesRepo(userId)` → `packages/db/src/repository/space.ts:74-76`: `db.select().from(space).where(eq(space.userId, userId))`, unbounded, no `limit`/`offset` params anywhere in the route (`packages/api/src/spaces/routes.ts:18`) or service.
- `packages/api/src/tags/service-new.ts:16-18` `getUserTags` → `packages/db/src/repository/tag-new.ts:23-41`, unbounded.
- `packages/api/src/tag-types/service.ts:11-13` `listTagTypes` → `packages/db/src/repository/tag-type.ts:13-15`, unbounded.

**This is a real inconsistency worth calling out loudly**: chunks is the
only endpoint in the whole API (at least among those examined for Phase 1
and this task) that paginates and envelopes its list response. Spaces, tags,
and tag-types return everything, always, as a bare array. The Rust port
must NOT default to wrapping these three in `{items, total, ...}` out of
habit from the chunks port — that would silently diverge from Node.

## Q2: `DELETE /api/tag-types/{id}` when tags still reference it — restrict, cascade, or null out?

**Null out** (`ON DELETE SET NULL`) — but the nulling is a **database-level
foreign-key behavior**, not application code. There is no explicit
"unlink referencing tags" step anywhere in the route or service.

Evidence:
- `packages/db/src/schema/tag.ts:27`:
  ```ts
  tagTypeId: text("tag_type_id").references(() => tagType.id, { onDelete: "set null" }),
  ```
- The route/service/repository chain for the delete
  (`packages/api/src/tag-types/routes.ts:46-53` →
  `packages/api/src/tag-types/service.ts:26-30` →
  `packages/db/src/repository/tag-type.ts:28-36`) does nothing but
  `db.delete(tagType).where(...)`. No query touches the `tag` table.
- So: Postgres itself sets `tag.tag_type_id = NULL` for every referencing
  row as a side effect of the FK constraint when the `tag_type` row is
  deleted. The API response is unaffected either way — `{ message: "Deleted"
  }`, 200 — it never reports how many tags were affected.
- **Port implication**: the Rust schema/migration must declare the same
  `ON DELETE SET NULL` on `tag.tag_type_id`, or reproduce the nulling
  explicitly in the service/repository layer — otherwise a delete would
  either fail (if `NO ACTION`/`RESTRICT` is the Postgres default and no
  explicit `ON DELETE` was written) or leave dangling foreign keys.

## Q3: What does `POST /api/spaces/{id}/reset` actually reset?

It wipes the space's **content**, not the space's own identity/config
fields, and not the space row itself.

Handler chain: `packages/api/src/spaces/routes.ts:60-62` →
`spaceService.resetSpace` (`packages/api/src/spaces/service.ts:82-87`,
404s if the space doesn't exist for the caller) →
`resetSpaceDataRepo` (`packages/db/src/repository/space.ts:144-184`).

What `resetSpaceData` actually does, in order:
1. Finds every `chunk_space` row for this `spaceId`.
2. Of those chunk ids, computes which are **exclusive** to this space (not
   also linked to any other space via `chunk_space`).
3. **Hard-deletes** the exclusive chunks from the `chunk` table outright
   (scoped to `chunk.userId = userId`) — `chunksDeleted` count.
4. Deletes **all** `chunk_space` rows for this space (line 165) — including
   rows for chunks that are shared with other spaces. Shared chunks
   themselves survive (deleted from step 3 only if exclusive), but they lose
   their association with this space; if they still belong to another
   space, they remain reachable from there.
5. Deletes `document` rows scoped to `spaceId` + `userId` (`docsDeleted`).
6. Deletes `plan` rows scoped to `spaceId` + `userId` (`plansDeleted`).
7. Deletes `requirement` rows scoped to `spaceId` + `userId` (`requirementsDeleted`).
8. Returns `{ chunksDeleted, docsDeleted, plansDeleted, requirementsDeleted }`.

**What it does NOT touch**: the `space` row itself (name, kind, description,
timestamps untouched), `space_code_metadata` (remoteUrl/localPaths survive
a reset), tags, tag types, and connections (all global entities per
CLAUDE.md, not space-scoped, so nothing here could touch them anyway).

So "reset" = "delete this space's exclusively-owned chunks and all
space-scoped documents/plans/requirements, but keep the space and its
git-remote/local-path config." It is effectively the same wipe
`DELETE /api/spaces/:id` performs as its first step
(`packages/api/src/spaces/service.ts:89-95`, `deleteSpace` calls
`resetSpaceData` then deletes the `space` row) — `reset` is "delete
everything a space deletion would delete, minus the space itself."

---

# Also captured, since already there

## Timestamps

Every timestamp observed across spaces/tags/tag-types
(`createdAt`, `updatedAt`) is ISO-8601 **with a trailing `Z`** and
**millisecond** precision (3 fractional digits), e.g.
`"2026-06-02T23:39:08.719Z"`. No microsecond precision, no
timezone-offset form (`+00:00`), no bare-local-time strings observed
anywhere in this slice. This matches the "always emit `Z`" fix Phase 1 had
to retrofit for chunks — good, nothing new to retrofit here, but the Rust
serializer must replicate the `Z` suffix + millisecond truncation exactly
(not `.719000Z` / not omitting the `Z`).

## Field casing

All JSON keys across every captured response are `camelCase`
(`tagTypeId`, `tagTypeName`, `localPaths`, `remoteUrl`, `userId`,
`createdAt`), consistent with Drizzle's default column-name → JS
mapping. No snake_case leaks through in any response body.

## Null vs. empty-array / empty-object conventions

- `space.description` → `null` when absent (not an empty string), see
  every row in `spaces-list.json` / `spaces-detail.json`.
- `tagType.icon` → `null` when absent, see every row in `tag-types-list.json`.
- `GET /api/spaces/detect` with no match → **empty HTTP body** (`Content-Length: 0`,
  no `content-type` header at all — not `null`, not `{}`, not 404). See
  `spaces-detect-nomatch.json` (recorded as `<empty body>`) and the raw
  `curl -i` capture below. This is worth flagging precisely because it is
  the shape most likely to be "improved" into `null`/`{}`/404 by an
  implementer working from intuition rather than the actual contract —
  Elysia returns exactly nothing when the handler resolves `null`/`undefined`
  and no `t.Object` response schema forces a shape.
- Chunk-list join fields (`tagTypeName`, `tagTypeColor`, `tagTypeIcon`,
  `chunkCount`) are present on every row of `GET /api/tags` even when a tag
  has no `tagTypeId` — need to re-verify with a tag that actually has
  `tagTypeId: null` in this DB (none in the seed data happened to lack one
  in the sampled output above the truncation point); if `tagTypeId` is
  `null`, the `leftJoin` in `getTagsForUser`
  (`packages/db/src/repository/tag-new.ts:23-41`) means `tagTypeName`,
  `tagTypeColor`, `tagTypeIcon` all resolve to `null` (not omitted) for
  that row — this follows directly from the LEFT JOIN and doesn't need a
  live example to be true by construction, but no live example of it was
  captured; treat this specific case as reasoned from source, not observed.

## `spaces-detect-nomatch` raw response headers (for the "empty body, no content-type" claim)

```
HTTP/1.1 200 OK
Access-Control-Allow-Headers: Content-Type, Authorization
Access-Control-Allow-Credentials: true
Vary: Origin
Access-Control-Allow-Methods: GET, POST, PATCH, DELETE, OPTIONS
Access-Control-Expose-Headers: host, user-agent, accept
RateLimit-Limit: 100
RateLimit-Remaining: 99
RateLimit-Reset: 60
Content-Length: 0
```
(No `Content-Type` header present at all — confirmed with `curl -i`
against `GET /api/spaces/detect?remoteUrl=<nonexistent>`.)

## `space` detail vs. list: nested `code` metadata, not flattened

This was flagged in the brief as "the likeliest shape divergence in this
slice," and it's real:

- `GET /api/spaces` (list) → each element is the bare `space` row only.
  **No `remoteUrl`/`localPaths` fields at all** — `space_code_metadata` is
  never joined in `listSpaces` (`packages/db/src/repository/space.ts:74-76`
  is a plain `select().from(space)`).
- `GET /api/spaces/:id` (detail) → **`{ space: {...}, code: {...} }`** — a
  nested two-key envelope, not a flattened merge of the two tables. See
  `spaces-detail.json`: `code.remoteUrl`, `code.localPaths` are nested under
  a `code` key, sibling to `space`, not spread into the top-level object.
  Source: `getSpaceWithCodeMetadata`
  (`packages/db/src/repository/space.ts:58-72`) does
  `.select({ space, code: spaceCodeMetadata })...leftJoin(...)`, and
  `spaceService.getSpace` (`packages/api/src/spaces/service.ts:22-26`)
  returns that row **as-is**, un-flattened, straight to the client.
- `GET /api/spaces/detect` (match case) → **flat**, no `code` key at all —
  see `spaces-detect-match.json`. `detectSpace`
  (`packages/api/src/spaces/service.ts:97-102`) returns
  `getCodeSpaceByRemoteUrl`/`getCodeSpaceByLocalPath`
  (`packages/db/src/repository/space.ts:78-98`), both of which
  `.select({ space })` only — no code metadata joined at all, so a
  space found via `detect` never reveals its own `remoteUrl`/`localPaths`,
  unlike a space found via `GET /api/spaces/:id`.
- **Net: three different shapes for "a space" depending on which endpoint
  returned it** — bare (list), nested-with-code (detail), bare-again
  (detect-match). A Rust port that reuses one shared "Space" response
  struct across all three routes would diverge from at least two of them.

## `detect` by `remoteUrl` failed to match live seed data — a data hygiene finding, not (necessarily) a code bug

`space_code_metadata.remote_url` for `seed-codebase-fubbik` is stored,
un-normalized, as `git@github.com:Pontusg45/fubbik.git` (confirmed via a
direct read-only `psql` query against `space_code_metadata`). `detect`
normalizes its query input through `normalizeGitUrl`
(`packages/api/src/spaces/normalize-url.ts`), which would turn that same
string into `github.com/Pontusg45/fubbik` — the two will never compare
equal. Querying `detect?remoteUrl=<any form of that URL>` therefore always
returns the empty-body no-match response against this DB, even for the
literal stored value. `createSpace`
(`packages/api/src/spaces/service.ts:36-59`) does normalize `remoteUrl`
before writing it, so this looks like seed data written outside that code
path (bypassing normalization) rather than a bug in `detect` itself — but
it means **`detect`-by-`remoteUrl` could not be captured against a real
match in this DB**; `spaces-detect-match.json` was captured via
`localPath` instead, which matches on the raw stored array value with no
normalization involved (`getCodeSpaceByLocalPath`,
`packages/db/src/repository/space.ts:89-98`, uses a raw `@>` JSONB
containment check, no `normalizeGitUrl` call on the stored side either —
so local-path matching would break on non-normalized path forms too,
e.g. a trailing slash, but the seed value and the query happened to match
exactly).
