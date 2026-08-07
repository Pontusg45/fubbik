# Four required questions — answered with evidence

## Q1: Does the collection `filter` JSON map onto the chunks `ListParams`? Complete picture + what Task 7 needs to build

### Complete key set actually stored

```sql
SELECT DISTINCT jsonb_object_keys(filter) AS key FROM collection ORDER BY 1;
```
against the live Node database (`postgresql://pontus@localhost:5432/fubbik`) returns exactly:

```
 key
------
 tags
 type
```

Only two `collection` rows exist in this DB (seed data):

```sql
SELECT id, name, filter FROM collection;
```
```
                  id                  |       name       |            filter
--------------------------------------+------------------+------------------------------
 71627275-2653-4430-85b6-f058fb2bef18 | Conventions      | {"type": "convention"}
 d28efcbf-8b9c-4193-a36d-f4154399fb93 | Self-documenting | {"tags": "self-documenting"}
```

So **live data** only exercises `type` and `tags`. But the schema declares a
wider shape (`packages/db/src/schema/collection.ts:6-16`, `CollectionFilter`
interface) and the route's Elysia body validator
(`packages/api/src/collections/routes.ts:7-17`, `CollectionFilterSchema`)
accepts nine possible keys, all optional strings:

```ts
export interface CollectionFilter {
    type?: string;
    tags?: string;
    search?: string;
    sort?: string;
    after?: string;
    enrichment?: string;
    minConnections?: string;
    origin?: string;
    reviewStatus?: string;
}
```

This is a **1:1 name match** with 9 of the parameters `chunks/service.ts`
`listChunks` accepts (`packages/api/src/chunks/service.ts:25-47`): `type`,
`search`, `sort`, `tags`, `after`, `enrichment`, `minConnections`, `origin`,
`reviewStatus`. (`listChunks` also accepts `limit`/`offset`/`exclude`/
`scope`/`alias`/`tagMode`/`spaceId`/`workspaceId`/`global`/`allSpaces`, none
of which the collection filter schema exposes — `getCollectionChunks`
hardcodes `spaceId: col.spaceId` from the collection row itself and leaves
everything else at its default.)

### Rust `chunk::list` support today, key by key

Rust's `ListChunksQuery`/`ListParams`
(`crates/fubbik-api/src/chunks/dto.rs:34-43`,
`crates/fubbik-api/src/chunks/dto.rs:52-66`) implements exactly 7 fields:
`chunk_type` (`type`), `search`, `origin`, `review_status`, `sort`, `limit`,
`offset`.

| Collection filter key | In Rust `chunk::list` today? | Notes |
|---|---|---|
| `type` | **Yes** | direct `eq(chunk.type, ...)` equivalent |
| `search` | **Yes** | trigram similarity ordering, ported |
| `origin` | **Yes** | direct equality |
| `reviewStatus` | **Yes** | direct equality |
| `sort` | **Yes** (`newest`/`oldest`/`alpha`/`updated`, no `search`-driven similarity sort variant confirmed — check `Sort` enum) | |
| `tags` | **No** | not in `ListChunksQuery` at all. Feasible: Phase 2a ported the `tag`/`chunk_tag` tables and `tags::repo` already does `LEFT JOIN chunk_tag` (`crates/fubbik-db/src/repo/tag.rs:102`), so the join primitive exists — but chunk-list filtering by tag name (`WHERE chunk.id IN (SELECT chunk_id FROM chunk_tag JOIN tag ON ... WHERE tag.name = ANY($1))`) is **new code**, not reuse. |
| `after` | **No** | Node: `after = new Date(Date.now() - N*86400000)` then `gte(chunk.updatedAt, after)` — i.e. the stored value is a **day count**, not a date string, converted server-side. Needs a new field + the same day-offset-to-timestamp conversion. |
| `enrichment` | **No** | Node: `"missing"` → `summary IS NULL OR embedding IS NULL OR jsonb_array_length(aliases) = 0`; `"complete"` → `summary IS NOT NULL AND embedding IS NOT NULL` (`packages/db/src/repository/chunk.ts:123-127`). New code — depends on `aliases` (jsonb array) and `embedding` columns being present in the Rust chunk row/query, which they should be post-Phase-1. |
| `minConnections` | **No** | Node: correlated subquery counting `chunk_connection` rows where the chunk is source OR target, compared `>=` (`packages/db/src/repository/chunk.ts:88-95`). New code; needs the `chunk_connection` table (ported in Phase 1's connections domain per `crates/fubbik-api/src/connections/`). |

**Bottom line for Task 7**: given only the two rows that exist today,
the *minimum* to make `GET /collections/{id}/chunks` behave identically in
Rust is `type` (already works) + `tags` (needs the join above — small, the
join tables already exist). But because the filter schema is not
restricted to what's currently stored, and `updateCollection`/
`createCollection` accept the full 9-key schema without validating that the
values correspond to anything real, Task 7 should treat all four missing
keys (`tags`, `after`, `enrichment`, `minConnections`) as in scope if
`chunk::list` itself is meant to reach full parity — this task's brief
scopes only `chunks`'s own `ListParams`, so whichever of the four aren't
covered there will still be missing when `collections` is ported on top of
it.

### How `GET /collections/{id}/chunks` evaluates the filter

It does **not** build its own SQL and does **not** re-implement any
filtering. `collections/service.ts` `getCollectionChunks`
(`packages/api/src/collections/service.ts:59-78`) fetches the collection row,
then calls the **exact same** `listChunks` function used by
`GET /api/chunks`, spreading the collection's stored `filter` object
straight into `listChunks`'s query argument (plus `spaceId: col.spaceId`
pulled from the collection row, not the filter):

```ts
export function getCollectionChunks(id: string, userId: string) {
    return getCollectionById(id, userId).pipe(
        Effect.flatMap(found => (found ? Effect.succeed(found) : Effect.fail(new NotFoundError({ resource: "Collection" })))),
        Effect.flatMap(col => {
            const filter = col.filter;
            return listChunks(userId, {
                type: filter.type,
                search: filter.search,
                sort: filter.sort as ...,
                tags: filter.tags,
                after: filter.after,
                enrichment: filter.enrichment as ...,
                minConnections: filter.minConnections,
                origin: filter.origin,
                reviewStatus: filter.reviewStatus,
                spaceId: col.spaceId ?? undefined
            });
        })
    );
}
```

Confirmed live: `GET /collections/71627275.../chunks` (`type: "convention"`
filter) and `GET /collections/d28efcbf.../chunks` (`tags: "self-documenting"`
filter) both return the **same `{chunks, total, limit, offset}` envelope**
as `GET /api/chunks` — see `collections-chunks-filter-type.json` (7 chunks,
`total: 7, limit: 50, offset: 0`) and `collections-chunks-filter-tags.json`
(7 chunks, same envelope shape, titles all seed-system chunks tagged
`self-documenting`). This means **the response shape for
`/collections/{id}/chunks` is the chunks envelope, not a bare array** —
different from every other list endpoint captured in this slice (see Q3).

**Port implication**: Task 7 does not need a new query-building layer. It
needs `chunk::list`'s `ListParams` extended with the missing keys (see table
above), and a thin service function that loads a `collection` row, maps its
stored `filter` JSON onto `ListParams`, and calls the same `chunk::list`
used by `GET /api/chunks`.

---

## Q2: What is `/settings/features`?

It is a **computed view over `instance_settings`**, not a fourth settings
table. `GET /settings/features` has no `requireSession` guard (only route in
this slice callable with no session) and calls
`settingsService.getFeatureFlags()` (`packages/api/src/settings/service.ts:59-75`):

```ts
export function getFeatureFlags() {
    return getAllInstanceSettingsRepo().pipe(
        Effect.map(rows => {
            const map: Record<string, unknown> = {};
            for (const row of rows) map[row.key] = row.value;
            return {
                aiEnabled: (map.aiEnabled as boolean) ?? true,
                enrichmentEnabled: (map.enrichmentEnabled as boolean) ?? true,
                semanticSearchEnabled: (map.semanticSearchEnabled as boolean) ?? true,
                aiSuggestionsEnabled: (map.aiSuggestionsEnabled as boolean) ?? true,
                vocabularySuggestEnabled: (map.vocabularySuggestEnabled as boolean) ?? true
            };
        })
    );
}
```

It reads the **same** `instance_settings` table as `GET /settings/instance`
(`getAllInstanceSettingsRepo` — identical repo call, `packages/db/src/repository` via `@fubbik/db/repository`),
projects exactly 5 fixed boolean keys out of the full `InstanceSettingsMap`
(`packages/db/src/schema/settings.ts:71-80`, which also has `ollamaUrl`,
`registrationEnabled`, `maxChunksPerCodebase`), and **defaults every key to
`true`** if the row is absent. Confirmed live: `instance_settings` table is
empty (0 rows) in this DB, and `settings-features.json` still returns
`{"aiEnabled":true,"enrichmentEnabled":true,"semanticSearchEnabled":true,"aiSuggestionsEnabled":true,"vocabularySuggestEnabled":true}`
— all five defaults, proving the `?? true` fallback, not a missing-row
error. `settings-instance.json` (the raw view) is `{}` for the same empty
table, confirming `/settings/features` is a narrower, defaulted, public
(no-session) projection of the same data `/settings/instance` exposes raw.

**Port implication**: Rust needs a `feature_flags` DTO with these 5 fields,
each independently defaulting to `true` if the corresponding
`instance_settings` key is absent — not a new table, and no auth
requirement.

---

## Q3: Bare arrays or envelopes, per list endpoint?

**All list endpoints in this slice are bare arrays or bare objects, except
`GET /collections/{id}/chunks`, which is the chunks envelope** because it
literally calls the chunks list function (see Q1). This is a third distinct
pattern beyond "chunks always envelopes" / "everything else is bare" — this
slice contains an endpoint that inherits the chunks envelope by
delegation, without being a chunks-domain endpoint itself.

Verbatim, from the captured fixtures:

- `GET /api/notifications` → `notifications-list.json` = `[]` (bare array; 0 rows in this DB). Source: `listNotificationsRepo` (`packages/db/src/repository/notification.ts:6-19`) does a plain `.limit(opts.limit ?? 50)` select, no envelope, no `total`.
- `GET /api/notifications/count` → `notifications-count.json` = `{"count":0}` — bare object with a single computed field, not a list endpoint.
- `GET /api/settings/user` → `settings-user.json` = `{}` (bare key→value map, empty because `user_settings` has 0 rows for this user). Source: `getAllUserSettings` reduces rows into a plain object (`packages/api/src/settings/service.ts:11-21`) — no envelope, no array.
- `GET /api/settings/instance` → `settings-instance.json` = `{}` — same pattern, `instance_settings` empty.
- `GET /api/settings/codebase?codebaseId=...` → `settings-codebase.json` = `{}` — same pattern, `codebase_settings` empty for this space.
- `GET /api/settings/features` → `settings-features.json` = bare object, 5 fixed keys (see Q2) — not a map of arbitrary settings, a fixed projection.
- `GET /api/workspaces` → `workspaces-list.json` = `[ {...} ]` (bare array, 1 row). Source: `listWorkspacesRepo` (`packages/db/src/repository/workspace.ts:36-38`) — plain `db.select()...`, unbounded, no envelope.
- `GET /api/workspaces/:id` → `workspaces-detail.json` = bare object `{...workspace fields, spaces: [...]}` — detail, not a list, but worth noting `spaces` is a **nested bare array** inside the object, not itself paginated.
- `GET /api/favorites` → `favorites-list.json` = `[]` (bare array, 0 rows). Source: `listFavoritesRepo` (`packages/db/src/repository/favorite.ts:6-8`) — plain select ordered by `order`, unbounded, no envelope.
- `GET /api/collections` → `collections-list.json` = `[ {...}, {...} ]` (bare array, 2 rows). Source: `listCollectionsRepo` (`packages/db/src/repository/collection.ts:8-10`) — plain select, unbounded, no envelope.
- `GET /api/collections/:id/chunks` → `collections-chunks-filter-type.json` / `collections-chunks-filter-tags.json` = **`{chunks, total, limit, offset}` envelope** — the one exception, because it delegates to `listChunks` (see Q1).
- `GET /api/activity` → `activity-list.json` = `[]` (bare array, 0 rows, but the route *does* accept `limit`/`offset` query params — see Q4). Source: `listActivityRepo` (`packages/db/src/repository/activity.ts:6-29`) applies `.limit()`/`.offset()` in the query itself but returns a bare array with **no `total` count** — so pagination exists but there's no way to know the total row count from the response, unlike chunks' envelope.

**Port implication**: only `collections::chunks` should return the chunks
envelope type; every other list endpoint in this domain slice (notifications,
workspaces, favorites, collections-list, activity) should return a bare
`Vec<T>` (or bare object for the settings maps), matching Phase 2a's finding
that chunks is the outlier, not the norm.

---

## Q4: What does `activity` actually expose?

Exactly **one route**, `GET /activity` (`packages/api/src/activity/routes.ts:7-30`,
28 lines including the query schema — the domain's 52 LOC total is
route+service+repository combined, `packages/api/src/activity/service.ts` is
23 lines, `packages/db/src/repository/activity.ts` is 45 lines including
`createActivity`). No mutating HTTP endpoint at all — `createActivity`
exists only as an internal function other domains call directly
(fire-and-forget audit logging), never exposed as a route.

Query params (`t.Object` schema, routes.ts:23-28), all optional:
- `spaceId: t.String()` — exact match filter (`eq(activityLog.spaceId, ...)`)
- `entityType: t.String()` — exact match filter (`eq(activityLog.entityType, ...)`); the service also accepts `entityId` in its options type but **no route ever passes it** — the query schema has no `entityId` field, so it's dead/unreachable from HTTP.
- `limit: t.Numeric()` — passed straight to `.limit(opts.limit ?? 50)`
- `offset: t.Numeric()` — passed straight to `.offset(opts.offset ?? 0)`

Response: bare array of `activity_log` rows (see Q3), ordered
`desc(activityLog.createdAt)`. Confirmed live: `activity-list.json` = `[]`
(0 rows in this DB).

Row shape (`packages/db/src/schema/activity.ts:6-25`): `id`, `userId`,
`entityType` (free-text `text`, comment says values are `"chunk"`,
`"requirement"`, `"connection"`, `"tag"`, `"codebase"` — **not** a DB-level
enum/check constraint, just a comment), `entityId`, `entityTitle` (nullable),
`action` (free-text `text`, comment says `"created"`, `"updated"`,
`"deleted"`, `"archived"`, `"restored"` — again, **not** a DB constraint),
`spaceId` (nullable, `ON DELETE SET NULL`), `createdAt`.

**Port implication**: both `entityType` and `action` are documented-by-
convention only, not DB-enforced — Postgres will happily accept any string.
If the Rust DTO wants them as enums (as the task brief suggests for
constrained fields), that's a **behavior change** — Node accepts arbitrary
strings today. `notification.type` (see below) is in the same position.

---

## Also captured: constrained-set fields, timestamps, and the favorites `order` column

### `notification.type` — also free-text, not DB-enforced

`packages/db/src/schema/notification.ts:12`: `type: text("type").notNull()`
with a comment listing `"stale_chunks"`, `"review_needed"`,
`"ai_suggestion"`, `"chunk_updated"` — same pattern as `activity_log.action`/
`entity_type`: documented convention, no CHECK constraint, no DB enum. The
live `notification` table has 0 rows so no values could be sampled directly;
the constraint (or lack of it) is confirmed from schema + `createNotification`
(`packages/api/src/notifications/service.ts:31-36`, `type: string` — untyped
pass-through, no validation).

### Timestamps

Every captured timestamp (`workspaces-list.json`, `workspaces-detail.json`)
uses full ISO-8601 with milliseconds and a trailing `Z`, e.g.
`"2026-06-02T23:39:08.785Z"` — Postgres `timestamp` (no timezone column
type) serialized by `postgres.js`/Drizzle as UTC with `Z` suffix. Same
convention as Phase 1's chunks domain. No fixture in this slice shows a
`timestamp with time zone` column — all six domains use plain
`timestamp("...")` (see each schema file), consistent with `defaultNow()`.

### Field casing and null-vs-omitted

All row fields are camelCase (Drizzle's default TS-side mapping from
snake_case columns) — no snake_case leaks anywhere in these fixtures.
Nullable columns that are actually null are **present with an explicit
`null` value**, never omitted — e.g. `workspace.description` would serialize
as `"description": null` if unset (not observed live since the one seeded
workspace has a description, but confirmed from the Elysia body schema
`t.Optional(t.Union([t.String(...), t.Null()]))` pattern used consistently
across `PATCH` bodies in this slice, e.g.
`packages/api/src/workspaces/routes.ts:47`).

### `user_favorite.order`

There **is** a reorder endpoint: `PUT /favorites/reorder`
(`packages/api/src/favorites/routes.ts:38-55`), body:
```ts
t.Array(t.Object({ chunkId: t.String({ maxLength: 100 }), order: t.Number() }))
```
i.e. a bare JSON array of `{chunkId, order}` pairs (not wrapped in an
object). Handler `reorderFavorites` (`packages/db/src/repository/favorite.ts:37-48`)
runs one `UPDATE ... SET order = $1 WHERE userId = $2 AND chunkId = $3` per
array entry inside a transaction — no batch/bulk SQL, no validation that the
full set of the user's favorites is represented (a partial reorder body is
accepted and only updates the entries present). `order` is also set on
insert in `addFavorite` (`packages/api/src/favorites/service.ts:17-31`):
`nextOrder = max(existing orders) + 1`, or `0` if no favorites exist yet —
new favorites are always appended to the end, never inserted at a specific
position. Response: `{"message": "Reordered"}`, status 200 (not documented
via a live capture — this is a mutating endpoint per the task's constraints,
documented from source only).
