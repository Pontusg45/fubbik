# Mutating endpoints — documented from source, not executed

Per the task brief: POST/PATCH/DELETE endpoints are never issued against the
running Node server (it holds live user data). Everything below is read
directly from route + service + repository source, with file:line citations.

All routes below require a session (`requireSession`, `packages/api/src/require-session.ts`)
and go through the global error handler
(`packages/api/src/index.ts:180-206`), which maps Effect-typed errors to HTTP
status + `{ message }` (or `{ message, errors }` for `StepValidationError`):

| `_tag`            | HTTP status | Body |
|--------------------|------------|------|
| `ValidationError`  | 400        | `{ message }` |
| `AuthError`         | 401        | `{ message: "Authentication required" }` |
| `NotFoundError`     | 404        | `{ message: "<resource> not found" }` |
| `AiError`           | 502        | `{ message: "AI service error" }` |
| `StepValidationError` | 400     | `{ message: "Invalid steps", errors }` |
| `DatabaseError`     | 500        | `{ message: "Internal server error" }` |

Successful create endpoints explicitly set `ctx.set.status = 201`
(`Effect.tap` block). Successful update endpoints return 200 (Elysia
default) with the updated row. Successful delete endpoints return 200 with
`{ message: "Deleted" }` (not 204 — no route in this slice uses 204).

---

## Spaces — `packages/api/src/spaces/routes.ts`

### `POST /api/spaces` (routes.ts:19-41)

Body schema:
```ts
t.Object({
    name: t.String({ maxLength: 100 }),
    kind: t.Optional(t.String({ maxLength: 50 })),
    description: t.Optional(t.String({ maxLength: 1000 })),
    remoteUrl: t.Optional(t.String({ maxLength: 500 })),
    localPaths: t.Optional(t.Array(t.String({ maxLength: 500 }), { maxItems: 10 }))
})
```
Handler: `spaceService.createSpace` (service.ts:36-59).
- `kind` defaults to `"code"` if omitted.
- `remoteUrl`, if present, is normalized via `normalizeGitUrl` (normalize-url.ts) **before** being stored — unlike the seed data captured in `spaces-detail.json`, which stores the raw un-normalized SSH form (see `_questions.md` for why `detect` failed to match it).
- If `kind === "code"` and a normalized `remoteUrl` is given, checks `getCodeSpaceByRemoteUrl` for an existing space with the same URL for this user and fails with `ValidationError({ message: "A space with this remote URL already exists" })` (400) if found.
- On success: inserts into `space`, and if `kind === "code"`, also inserts a `space_code_metadata` row (`repository/space.ts:21-44`).
- Response: the inserted `space` row (bare object — no `code` metadata wrapper on create, unlike `GET /api/spaces/:id`). Status **201**.

### `PATCH /api/spaces/:id` (routes.ts:45-59)

Body schema:
```ts
t.Object({
    name: t.Optional(t.String({ maxLength: 100 })),
    description: t.Optional(t.Union([t.String({ maxLength: 1000 }), t.Null()])),
    remoteUrl: t.Optional(t.Union([t.String({ maxLength: 500 }), t.Null()])),
    localPaths: t.Optional(t.Array(t.String({ maxLength: 500 }), { maxItems: 10 }))
})
```
Handler: `spaceService.updateSpace` (service.ts:68-80).
- 404 (`NotFoundError({resource:"Space"})`) if the space doesn't exist for this user.
- `remoteUrl` normalized via `normalizeGitUrl` if truthy; `null`/omitted passed through as-is.
- Only rebuilds `space_code_metadata` (`code:` param) if the found space's `kind === "code"` — for non-code spaces, `remoteUrl`/`localPaths` in the body are silently ignored.
- Repository (`repository/space.ts:106-142`): builds a partial `SET` clause from only the keys actually present in `params` (so omitted fields are left untouched, not nulled); if no fields are set, falls back to a plain `SELECT` (still returns the row, doesn't no-op to `null`). Then, if `params.code` was passed, upserts `space_code_metadata` via `onConflictDoUpdate`.
- Response: the updated (or unchanged) `space` row, status 200. Bare — no nested `code`.

### `POST /api/spaces/:id/reset` (routes.ts:60-62)

No body. Handler: `spaceService.resetSpace` (service.ts:82-87) → see `_questions.md` Q3 for full behavior. Response: `resetSpaceDataRepo`'s return value, `{ chunksDeleted, docsDeleted, plansDeleted, requirementsDeleted }` (all numbers), status 200. 404 if the space doesn't exist for this user.

### `DELETE /api/spaces/:id` (routes.ts:63-70)

No body. Handler chain: `getSpaceById` (404 if missing) → `resetSpaceData` (same wipe as reset) → `deleteSpaceRepo` (`repository/space.ts:186-194`, deletes the `space` row itself). Route wraps the final Effect result with `Effect.map(() => ({ message: "Deleted" }))` — the repo's own return value (the deleted row) is discarded. Response: `{ message: "Deleted" }`, status 200.

### `GET /api/spaces/detect?remoteUrl=&localPath=` (routes.ts:8-17)

Not mutating, but documented here for completeness alongside the other space endpoints since it isn't in the plain 4-endpoint capture list. See captured fixtures `spaces-detect-nomatch.json` / `spaces-detect-match.json` and `_questions.md`.

---

## Tags — `packages/api/src/tags/routes.ts` (service: `service-new.ts`)

### `POST /api/tags` (routes.ts:9-28)

Body schema:
```ts
t.Object({
    name: t.String({ maxLength: 50 }),
    tagTypeId: t.Optional(t.String())
})
```
Handler: `tagService.createUserTag` (service-new.ts:20-31).
- `origin` is not in the request body schema (route strips it — Elysia validation would reject an extra field... actually Elysia by default ignores/strips unknown body properties rather than rejecting, but the type signature accepts an optional `origin` that a normal HTTP caller can never actually send through this route). Effectively always defaults to `origin = "human"` → `reviewStatus = "approved"`.
- No collision/uniqueness pre-check here (unlike update) — relies on the DB's `(user_id, name)` unique index; a duplicate name would surface as a `DatabaseError` → 500, not 400.
- Response: inserted `tag` row (bare — no `tagTypeName`/`tagTypeColor`/`chunkCount` join fields present on create, unlike the list endpoint's shape). Status 201.

### `PATCH /api/tags/:id` (routes.ts:29-42)

Body schema:
```ts
t.Object({
    name: t.Optional(t.String({ maxLength: 50 })),
    tagTypeId: t.Optional(t.Union([t.String(), t.Null()])),
    reviewStatus: t.Optional(t.Union([t.Literal("draft"), t.Literal("reviewed"), t.Literal("approved")]))
})
```
Handler: `tagService.updateUserTag` (service-new.ts:33-58).
- If `reviewStatus` is present, also sets `reviewedBy = userId` and `reviewedAt = new Date()` server-side (not client-settable).
- If `name` is present, pre-checks `tagNameConflict` (tag-new.ts:65-74) and fails `ValidationError({message: 'Tag "<name>" already exists'})` (400) on collision, to avoid surfacing the DB unique-index violation as a raw 500.
- 404 if not found/not owned.
- Response: updated `tag` row (bare, same shape as create's response — no joined `tagType*`/`chunkCount` fields). Status 200.

### `DELETE /api/tags/:id` (routes.ts:43-50)

No body. Handler: `deleteUserTag` → `deleteTagRepo` (tag-new.ts:111-119), 404 if not found/not owned. Response: `{ message: "Deleted" }`, status 200. Cascades: `chunk_tag` rows reference `tag.id` with `onDelete: "cascade"` (`packages/db/src/schema/tag.ts:48`), so deleting a tag also removes its `chunk_tag` join rows.

### `POST /api/tags/merge` (routes.ts:51-65)

Body schema:
```ts
t.Object({
    sourceId: t.String(),
    targetId: t.String()
})
```
Handler: `tagService.mergeUserTags` (service-new.ts:60-69) → `mergeTagsRepo` (tag-new.ts:76-109).
- Fails `ValidationError({message: "Cannot merge a tag into itself"})` (400) if `sourceId === targetId`.
- Repository runs in a single DB transaction (tag-new.ts:78-107):
  1. Confirms both `sourceId` and `targetId` belong to `userId` (throws a plain `Error` — NOT an Effect-typed error — if either is missing; this surfaces to the client as an untagged 500 via `dbEffect`'s wrapping, not a 404/400).
  2. Re-points `chunk_tag` rows from `sourceId` to `targetId` via raw SQL `INSERT ... ON CONFLICT (chunk_id, tag_id) DO NOTHING`, then deletes the leftover `chunk_tag` rows still pointing at `sourceId` (duplicates that already existed on the target).
  3. Deletes the source `tag` row itself.
  4. Returns the new total chunk count on the target.
- Response: `{ targetId: string, chunkCount: number }`, status 200 (no explicit 201 override on this route — it's a POST but semantically an update, not a create).
- Note: the tag type of the target is untouched; the source's `tagTypeId` is simply discarded along with the row.

---

## Tag types — `packages/api/src/tag-types/routes.ts` (service: `service.ts`)

### `POST /api/tag-types` (routes.ts:11-31)

Body schema:
```ts
t.Object({
    name: t.String({ maxLength: 50 }),
    color: t.Optional(t.String({ maxLength: 7 })),
    icon: t.Optional(t.Union([t.String({ maxLength: 40 }), t.Null()]))
})
```
Handler: `tagTypeService.createTagType` (service.ts:15-18).
- `color` defaults to `"#8b5cf6"` if omitted; `icon` defaults to `null`.
- No uniqueness pre-check (name collisions, if the schema has a constraint, would surface as 500).
- Response: inserted `tag_type` row, status 201.

### `PATCH /api/tag-types/:id` (routes.ts:32-45)

Body schema:
```ts
t.Object({
    name: t.Optional(t.String({ maxLength: 50 })),
    color: t.Optional(t.String({ maxLength: 7 })),
    icon: t.Optional(t.Union([t.String({ maxLength: 40 }), t.Null()]))
})
```
Handler: `updateTagType` (service.ts:20-24) → `updateTagTypeRepo` (tag-type.ts:17-26): plain `db.update(...).set(data)` — passes the raw body straight through as the `SET` clause (no partial-field filtering the way `updateSpace` does it), so omitted keys simply aren't included in `data` (Elysia's `t.Optional` fields are absent, not `undefined`-valued, when not sent — same net effect). 404 if not found/not owned. Response: updated `tag_type` row, status 200.

### `DELETE /api/tag-types/:id` (routes.ts:46-53)

No body. Handler: `deleteTagType` → `deleteTagTypeRepo` (tag-type.ts:28-36), 404 if not found/not owned. Response: `{ message: "Deleted" }`, status 200.
**Referencing tags are NOT touched by any application code** — see `_questions.md` Q2: the behavior is entirely a DB-level `ON DELETE SET NULL` foreign-key constraint on `tag.tag_type_id` (`packages/db/src/schema/tag.ts:27`).

---

## Connections — `packages/api/src/connections/routes.ts`

### `POST /api/connections` (routes.ts:8-29)

Body schema:
```ts
t.Object({
    sourceId: t.String({ maxLength: 100 }),
    targetId: t.String({ maxLength: 100 }),
    relation: t.String({ maxLength: 50 }),
    origin: t.Optional(t.Union([t.Literal("human"), t.Literal("ai")]))
})
```
Handler: `connectionService.createConnection` (service.ts:11-34).
- Fails `ValidationError({message: "Cannot connect a chunk to itself"})` (400) if `sourceId === targetId`.
- Looks up `getChunkById(sourceId, userId)` — 404 `NotFoundError({resource: "Source chunk"})` if missing/not owned.
- Looks up `getChunkById(targetId, userId)` — 404 `NotFoundError({resource: "Target chunk"})` if missing/not owned.
- `origin` defaults to `"human"` → `reviewStatus = "approved"`; `origin: "ai"` → `reviewStatus = "draft"`.
- `relation` is an unconstrained string at the API layer (`t.String({maxLength:50})` — no enum), even though CLAUDE.md documents a fixed vocabulary (related_to, part_of, depends_on, extends, references, supports, contradicts, alternative_to). Any DB-level enum/check constraint, if present, would be the actual source of truth for valid values — not checked as part of this task (would show up as a `DatabaseError` 500 on an invalid value).
- Response: inserted `connection` row, status 201.

### `DELETE /api/connections/:id` (routes.ts:30-37)

No body. Handler: `deleteConnection` (service.ts:36-51).
- Looks up the connection by id — 404 if missing.
- Then re-fetches **both** `source` and `target` chunks scoped to `userId`, and requires **at least one** of them (`source || target`) to resolve, else 404 `NotFoundError({resource: "Connection"})` — i.e. an authorization check disguised as a not-found: a connection between two chunks neither owned by the caller is invisible/undeletable, but only one end needs to belong to the caller for the delete to proceed.
- Deletes by `conn.id`.
- Response: `{ message: "Deleted" }`, status 200.

---

## Stats

`GET /api/stats` is the only stats endpoint — no mutating endpoints in this domain.
