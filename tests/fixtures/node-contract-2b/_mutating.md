# Mutating endpoints — documented from source, not executed

Per the task brief: POST/PATCH/PUT/DELETE endpoints are never issued against
the running Node server (it holds live user data). Everything below is read
directly from route + service + repository source, with file:line citations.

All routes below require a session (`requireSession`,
`packages/api/src/require-session.ts`) — **except** `POST /settings/*` is
gated by session but `GET /settings/features` is not (see `_questions.md`
Q2) — and go through the global error handler
(`packages/api/src/index.ts:179-204`), which maps Effect-typed errors to
HTTP status + `{ message }` (or `{ message, errors }` for
`StepValidationError`):

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
`{ message: "Deleted" }` (not 204). This slice adds two more response
conventions not seen in Phase 2a: bare `{ message: "..." }` acks for
non-CRUD mutations (mark-read, reorder), and a `PUT` used for a
non-idempotent-in-practice bulk update (`/favorites/reorder`).

Total across the six domains: **17 mutating + 12 GET = 29 endpoints**,
matching the brief's estimate exactly.

---

## Notifications — `packages/api/src/notifications/routes.ts`

### `POST /notifications/read-all` (routes.ts:16-23)

No body.
Handler: `notificationService.markAllAsRead` (service.ts:27-29) →
`markAllAsReadRepo` (`packages/db/src/repository/notification.ts:42-49`):
`UPDATE notification SET read = true WHERE userId = ? AND read = false`
(unconditional, no `id`/`ids` param — marks *every* unread notification for
the user).
Response: `{ "message": "All marked as read" }`. Status **200**.

### `PATCH /notifications/:id/read` (routes.ts:44-48)

No body.
Handler: `notificationService.markAsRead` (service.ts:21-25) →
`markAsReadRepo` (`packages/db/src/repository/notification.ts:31-40`):
`UPDATE notification SET read = true WHERE id = ? AND userId = ? RETURNING *`.
- Fails with `NotFoundError({ resource: "Notification" })` (404) if no row
  matched (wrong id, or belongs to another user — the `userId` scoping is in
  the same `WHERE`, so cross-user access looks identical to "not found").
Response on success: the **updated notification row itself** (not wrapped
in `{message}`) — `{ id, userId, type, title, message, linkTo, read: true,
createdAt }`. Status **200**.

### `DELETE /notifications/:id` (routes.ts:49-56)

No body.
Handler: `notificationService.deleteNotification` (service.ts:38-42) →
`deleteNotificationRepo` (`packages/db/src/repository/notification.ts:58-66`):
`DELETE FROM notification WHERE id = ? AND userId = ? RETURNING *`.
- 404 (`NotFoundError({ resource: "Notification" })`) if no row matched.
Response: `{ "message": "Deleted" }`. Status **200**.

---

## Settings — `packages/api/src/settings/routes.ts`

All three PATCH endpoints share one shape: `{ key, value }` (or
`{ codebaseId, key, value }` for the codebase variant) upserted into a
key→jsonb table. `value: t.Unknown()` — **no validation at all** on the
value's shape, despite the typed `UserSettingsMap`/`CodebaseSettingsMap`/
`InstanceSettingsMap` interfaces in `packages/db/src/schema/settings.ts:54-80`
existing purely as TS-side documentation, never enforced at the API
boundary. A client can PATCH `theme` to `42` and Node will store it as-is.

### `PATCH /settings/user` (routes.ts:12-27)

Body:
```ts
t.Object({
    key: t.String(),
    value: t.Unknown()
})
```
Handler: `settingsService.setUserSetting` (service.ts:23-25) →
`setUserSettingRepo` — upsert on `(userId, key)` unique index
(`user_settings_user_key_idx`, `packages/db/src/schema/settings.ts:20`).
Response: `{ "message": "Updated" }`. Status **200**.

### `PATCH /settings/codebase` (routes.ts:38-54)

Body:
```ts
t.Object({
    codebaseId: t.String(),
    key: t.String(),
    value: t.Unknown()
})
```
Note: `codebaseId` here is a `spaceId` — the field name is a legacy holdover
(`codebase` predates the `space` rename elsewhere in the codebase; see
top-level CLAUDE.md's "Spaces & Workspaces" section).
Handler: `settingsService.setCodebaseSetting` (service.ts:39-41) — upsert on
`(spaceId, key)` unique index (`codebase_settings_cb_key_idx`). **No
ownership/existence check on `codebaseId`** — an arbitrary/nonexistent
`spaceId` string is accepted silently (no FK enforcement visible in the
route/service; `codebaseSettings.spaceId` does reference `space.id` with
`onDelete: cascade` in the schema, so a genuinely nonexistent id would
actually fail at the DB FK level and surface as a `DatabaseError` → 500, not
a clean `ValidationError`/`NotFoundError`).
Response: `{ "message": "Updated" }`. Status **200**.

### `PATCH /settings/instance` (routes.ts:56-71)

Body:
```ts
t.Object({
    key: t.String(),
    value: t.Unknown()
})
```
Handler: `settingsService.setInstanceSetting` (service.ts:55-57) — upsert
keyed on `instance_settings.key` (primary key, no composite scoping — this
is instance-global, not per-user). **No auth-level restriction beyond
`requireSession`** — any authenticated user can flip instance-wide flags
like `aiEnabled`; there is no admin/role check anywhere in this route.
Response: `{ "message": "Updated" }`. Status **200**.

---

## Workspaces — `packages/api/src/workspaces/routes.ts`

### `POST /workspaces` (routes.ts:11-30)

Body:
```ts
t.Object({
    name: t.String({ maxLength: 200 }),
    description: t.Optional(t.String({ maxLength: 2000 }))
})
```
Handler: `workspaceService.createWorkspace` (service.ts:31-51).
- Fails `ValidationError({ message: "Workspace name is required" })` (400)
  if `name.trim()` is empty (whitespace-only names rejected despite passing
  the `t.String` schema).
- `name` is trimmed before insert; `description` passed through as-is
  (`undefined` if omitted).
- **No uniqueness check in application code** — but `workspace_user_name_idx`
  is a DB-level unique index on `(userId, name)`
  (`packages/db/src/schema/workspace.ts:22`), so a duplicate name for the
  same user will throw a raw Postgres unique-violation, caught as
  `DatabaseError` → 500 (not a clean 400 `ValidationError` — no
  code-level duplicate check the way `space.remoteUrl` gets in the spaces
  domain per Phase 2a's `_mutating.md`).
Response: the inserted `workspace` row. Status **201**.

### `PATCH /workspaces/:id` (routes.ts:36-50)

Body:
```ts
t.Object({
    name: t.Optional(t.String({ maxLength: 200 })),
    description: t.Optional(t.Union([t.String({ maxLength: 2000 }), t.Null()]))
})
```
Handler: `workspaceService.updateWorkspace` (service.ts:53-76).
- 404 (`NotFoundError({ resource: "Workspace" })`) if no workspace with
  that `id`+`userId` exists.
- If `name` is provided and trims to empty, 400
  (`ValidationError({ message: "Workspace name cannot be empty" })`).
- `description: null` explicitly clears the field (distinguished from
  `undefined` = "don't touch", via `t.Union([..., t.Null()])` — same pattern
  as spaces `PATCH` in Phase 2a).
- If the body supplies *no* updatable fields (both undefined), repo
  short-circuits to a plain re-select rather than an `UPDATE`
  (`packages/db/src/repository/workspace.ts:51-57`) — a no-op PATCH is a
  200 with the row unchanged, not a 400.
Response: the updated (or re-selected) `workspace` row. Status **200**.

### `DELETE /workspaces/:id` (routes.ts:51-58)

No body. Handler: `workspaceService.deleteWorkspace` (service.ts:78-82) —
404 if no row matched `id`+`userId`. Response: `{ "message": "Deleted" }`.
Status **200**. Note: `workspace_space` join rows for this workspace are
**not** explicitly deleted in application code, but
`workspaceSpace.workspaceId` has `onDelete: cascade`
(`packages/db/src/schema/workspace.ts:30`) so Postgres removes them as a FK
cascade side effect — same "DB does it, not app code" pattern as
tag_type→tag nulling from Phase 2a.

### `POST /workspaces/:id/spaces` (routes.ts:60-78)

Body:
```ts
t.Object({
    spaceId: t.String()
})
```
Handler: `workspaceService.addSpaceToWorkspace` (service.ts:84-94).
- 404 `{ resource: "Workspace" }` if the workspace doesn't exist for this
  user.
- 404 `{ resource: "Space" }` if the space doesn't exist for this user
  (`getSpaceById(spaceId, userId)` — so a space owned by another user is
  treated as not found, same as elsewhere).
- Insert uses `.onConflictDoNothing()` on the composite PK
  `(workspaceId, spaceId)` — adding an already-linked space is a **no-op,
  not an error**; the repo falls back to returning
  `{ workspaceId, spaceId }` (a plain object, not a full DB row with
  timestamps — `workspace_space` has no timestamp columns) when the insert
  affected 0 rows.
Response: `{ workspaceId, spaceId }` (either the inserted row or the
fallback literal — structurally identical either way since the join table
has only those two columns). Status **201**.

### `DELETE /workspaces/:id/spaces/:spaceId` (routes.ts:79-86)

No body. Handler: `workspaceService.removeSpaceFromWorkspace`
(service.ts:96-105).
- 404 `{ resource: "Workspace" }` if the workspace doesn't exist for this
  user.
- 404 `{ resource: "WorkspaceSpace" }` if the join row didn't exist (space
  was never linked, or already removed).
Response: `{ "message": "Deleted" }`. Status **200**.

---

## Favorites — `packages/api/src/favorites/routes.ts`

### `POST /favorites` (routes.ts:11-29)

Body:
```ts
t.Object({
    chunkId: t.String({ maxLength: 100 })
})
```
Handler: `favoriteService.addFavorite` (service.ts:17-31).
- 404 `{ resource: "Chunk" }` if `chunkId` doesn't resolve via
  `getChunkById(chunkId, userId)`.
- `order` is computed application-side, **not DB-generated**:
  `existing.length > 0 ? Math.max(...existing.map(f => f.order)) + 1 : 0` —
  new favorites always append to the end of the user's current ordering.
  This is a read-then-write (list favorites, compute max, insert) — no
  transaction wraps it, so this is a benign but real TOCTOU race under
  concurrent adds (two simultaneous `POST /favorites` could compute the same
  `nextOrder` and both succeed, producing two favorites with the same
  `order`, since `order` has no unique constraint — only `(userId, chunkId)`
  is unique).
- Insert uses `.onConflictDoNothing()` on `(userId, chunkId)`
  (`favorite_user_chunk_idx`) — favoriting an already-favorited chunk
  returns `created ?? null`, i.e. **`null`**, not an error and not the
  existing row. The route has no explicit handling for a `null` result here
  (unlike other repos where `null` triggers `NotFoundError`) — it flows
  straight through as the Effect's success value, so the HTTP response body
  would literally be `null` with status 201 on a duplicate favorite. Worth
  flagging as a real Node quirk to preserve or fix in the port.
Response: the inserted favorite row `{ id, userId, chunkId, order,
createdAt }`, or `null` on a duplicate (see above). Status **201**
(unconditionally, even for the `null`-body duplicate case — `Effect.tap`
sets status before checking the result).

### `DELETE /favorites/:chunkId` (routes.ts:30-37)

No body. Handler: `favoriteService.removeFavorite` (service.ts:33-35) →
plain delete, `deleted ?? null` — **no 404 check at the route/service
level** (unlike notifications/workspaces deletes). Deleting a
non-favorited chunk is a silent no-op that still returns
`{ "message": "Deleted" }`. Status **200**, always (no way to distinguish
"deleted something" from "there was nothing to delete" from the response).

### `PUT /favorites/reorder` (routes.ts:38-55)

Body (bare array, not wrapped in an object):
```ts
t.Array(
    t.Object({
        chunkId: t.String({ maxLength: 100 }),
        order: t.Number()
    })
)
```
Handler: `favoriteService.reorderFavorites` (service.ts:41-43) →
`reorderFavoritesRepo` (`packages/db/src/repository/favorite.ts:37-48`):
loops the array inside a single DB transaction, running one
`UPDATE user_favorite SET order = ? WHERE userId = ? AND chunkId = ?` per
entry. Notes:
- **Partial reorder is accepted** — the body doesn't need to include every
  favorite the user has; entries not mentioned keep their existing `order`.
- **No validation that `chunkId`s belong to the caller beyond the `WHERE`
  clause** — an unrecognized `chunkId` for this user simply updates 0 rows,
  silently, no error surfaced per-entry.
- **No validation of `order` uniqueness/contiguity** — duplicate or
  out-of-sequence `order` values are accepted as-is; ordering on read
  (`listFavorites`) is `ORDER BY order ASC`, so ties fall back to whatever
  order Postgres returns them in (undefined without a secondary sort key).
Response: `{ "message": "Reordered" }`. Status **200**.

---

## Collections — `packages/api/src/collections/routes.ts`

The `filter` object shares one schema (`CollectionFilterSchema`,
routes.ts:7-17) across create/update — see `_questions.md` Q1 for its full
field-by-field mapping onto chunks' `ListParams`.

### `POST /collections` (routes.ts:23-44)

Body:
```ts
t.Object({
    name: t.String({ maxLength: 100 }),
    description: t.Optional(t.String({ maxLength: 500 })),
    filter: t.Object({
        type: t.Optional(t.String()),
        tags: t.Optional(t.String()),
        search: t.Optional(t.String()),
        sort: t.Optional(t.String()),
        after: t.Optional(t.String()),
        enrichment: t.Optional(t.String()),
        minConnections: t.Optional(t.String()),
        origin: t.Optional(t.String()),
        reviewStatus: t.Optional(t.String())
    }),
    spaceId: t.Optional(t.String())
})
```
Handler: `collectionService.createCollection` (service.ts:18-35) — thin
pass-through insert, no validation on `filter`'s contents beyond the schema
shape above (e.g. `sort: "bogus-value"` is accepted; only interpreted, and
silently ignored by `listChunks`'s `switch`, when the collection's chunks
are actually fetched). **No uniqueness check in application code**, but
`collection_user_name_idx` is a DB-level unique index on `(userId, name)`
(`packages/db/src/schema/collection.ts:35`) — a duplicate name throws a raw
Postgres violation → `DatabaseError` → 500, same pattern as workspaces.
Response: the inserted `collection` row (including the stored `filter`
verbatim). Status **201**.

### `PATCH /collections/:id` (routes.ts:45-60)

Body:
```ts
t.Object({
    name: t.Optional(t.String({ maxLength: 100 })),
    description: t.Optional(t.String({ maxLength: 500 })),
    filter: t.Optional(/* same CollectionFilterSchema as above */)
})
```
Handler: `collectionService.updateCollection` (service.ts:37-51).
- 404 `{ resource: "Collection" }` if no row matches `id`+`userId` (checked
  explicitly before the update, then **again** after — the repo's
  `updated ?? null` is re-checked with a second `NotFoundError`, a
  belt-and-suspenders double check not seen elsewhere in this slice).
- `filter`, if provided, **replaces the whole object** (`.set(params)` where
  `params.filter` is passed as-is) — not a shallow merge with the existing
  filter. Sending `{filter: {type: "note"}}` on a collection that had
  `{type: "convention", tags: "x"}` drops the `tags` key entirely.
- Note: `spaceId` is **not** in the update body schema at all — a
  collection's `spaceId` is immutable after creation via this endpoint.
Response: the updated `collection` row. Status **200**.

### `DELETE /collections/:id` (routes.ts:61-68)

No body. Handler: `collectionService.deleteCollection` (service.ts:53-57) —
404 if no row matched. Response: `{ "message": "Deleted" }`. Status **200**.

---

## Activity — `packages/api/src/activity/routes.ts`

**No mutating HTTP endpoint exists.** `activityService.createActivity`
(service.ts:10-22) is exported and used internally by other domains
(fire-and-forget audit logging when chunks/requirements/connections/tags
change — call sites are elsewhere in the codebase, not in
`activity/routes.ts`), but there is no `POST /activity` route. This is
consistent with the brief's "52 LOC, roughly one endpoint" — the entire
domain surface is the single `GET /activity` documented in `_questions.md`
Q4.
