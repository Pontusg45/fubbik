# Mutating endpoints — documented from source, not executed

## Why source-only, despite the brief asking for live POSTs

`task-1-brief.md` step 3 says: "Verify the claim that every plans endpoint returns 200 and none
returns 201 — check at least three POSTs rather than trusting the schema." The orchestrating
instructions for this run are stricter and take precedence: **"You must never psql, INSERT,
UPDATE, DELETE, or otherwise write to that database yourself. Issue GETs only."** A `POST
/api/plans` (or any of the other 29 mutating calls) against `postgres://pontus@localhost:5432/fubbik`
writes a row to the user's live personal knowledge base — that is exactly the write the
authorization forbids. This matches the precedent set in Phase 2b, where all mutating endpoints
were also documented from source only, never executed. **This is a deviation from the brief's
step 3, called out explicitly per "STOP and report if any finding contradicts the plan"** — see
the top-level report for the one-line summary.

In place of live execution, the "no route sets 201" claim is verified by direct grep, and every
response shape below is read from the exact route + service + repository code, cited by
file:line. Where a repository function's true runtime return shape is a surprise not visible
from its declared return type, that is flagged explicitly (see the two ⚠ items below) — those
specifically need a real POST against a disposable/seed database (not the user's live one)
before an implementer relies on the exact field names.

All routes below require a session (`requireSession`, `packages/api/src/require-session.ts`) and
go through the same global error handler as every other domain
(`packages/api/src/index.ts:180-206`):

| `_tag`            | HTTP status | Body |
|--------------------|------------|------|
| `ValidationError`  | 400        | `{ message }` |
| `AuthError`         | 401        | `{ message: "Authentication required" }` |
| `NotFoundError`     | 404        | `{ message: "<resource> not found" }` |
| `AiError`           | 502        | `{ message: "AI service error" }` |
| `StepValidationError` | 400     | `{ message: "Invalid steps", errors }` |
| `DatabaseError`     | 500        | `{ message: "Internal server error" }` |

**Confirmed by grep: no route file in this slice (`plans/routes.ts`, `plans/tasks.ts`,
`plans/analyze.ts`, `plans/requirements.ts`, `search/routes.ts`, `staleness/routes.ts`) contains
`set.status = 201` or any other explicit status assignment anywhere.** Every successful mutating
response in this slice therefore falls through to Elysia's default success status, **200** — none
returns 201. This matches the brief's claim exactly; nothing here contradicts it.

Total: **23 plans + 3 search + 4 staleness = 30 mutating endpoints**, matching the brief's count.

---

## Plans — `packages/api/src/plans/routes.ts`

### `POST /plans` (routes.ts:39-79)
Body: `{ title: string, description?, spaceId?, requirementIds?: string[], tasks?: [{title, description?, acceptanceCriteria?: string[]}], metadata?: Record<string, unknown> }`.
`title.trim()` required non-empty (`service.ts:126-128`, else 400 `ValidationError`).
Handler (`service.ts:124-157`): inserts plan (`status: "draft"`, `description ?? null`, `spaceId ?? null`, `metadata ?? {}`), then loops `requirementIds` through `addPlanRequirement`, then loops `tasks` through `createTask`. Also fires `createActivity` (action `"created"`).
Response: the created `Plan` row (`.returning()`, `repository/plan.ts:152-158`) — id/title/description/status/userId/spaceId/createdAt/updatedAt/completedAt(null)/metadata. Status **200**.

### `PATCH /plans/:id` (routes.ts:80-112)
Body: `{ title?, description?: string|null, status?, spaceId?: string|null, metadata? }`. All optional; only provided keys are patched (`service.ts:167-187`).
`status`, if given, must be one of `draft|analyzing|ready|in_progress|completed|archived` (400 `ValidationError` otherwise). Transitioning into `"completed"` sets `completedAt = now`; transitioning out of it sets `completedAt = null` (else left unchanged).
404 `NotFoundError` if plan missing. Activity action is `"status_changed"` if `status` was in the body, else `"updated"`.
Response: updated `Plan` row (`.returning()`). Status **200**.

### `DELETE /plans/:id` (routes.ts:113-133)
No body. 404 if missing. Deletes row, fires activity `"deleted"`.
Response: **`{ "ok": true }`** (route explicitly discards the delete Effect's value and returns this literal — routes.ts:132). Status **200**.

### `POST /plans/:id/duplicate` (routes.ts:134-153)
No body. 404 if source missing (`getPlan` check first). Deep-copies the plan in one transaction (`repository/plan.ts:177-`): plan row (title → `"<source title> (copy)"`, status reset to `"draft"`), `planRequirement` rows (order preserved), `planAnalyzeItem` rows (new ids), `planTask` rows (new ids via a remap table, **status reset to `"pending"` on every task regardless of source status**), and `planTaskChunk` links rewritten to the new task ids. Fires activity `"duplicated"`.
Response: the new `Plan` row only — the endpoint does not return the copied children, matching the plain create/update endpoints' shape (a client must `GET /plans/:id` on the new id to see tasks/analyze/requirements). Status **200**.

### `POST /plans/:id/links` (routes.ts:192-215)
Body: `{ url: string (max 2000), system?: string (max 40, default "url"), label?: string (max 200) }`. 404 if plan missing.
Response: created `PlanExternalLink` row — `{id, planId, system, url, label, order, createdAt}` (`.returning()`, `repository/plan.ts:570-576`). Status **200**.

### `DELETE /plans/:id/links/:linkId` (routes.ts:216-224)
No body. Response: `{ "ok": true }`. Status **200**.

### `POST /plans/:id/requirements` (requirements.ts:9-20)
Body: `{ requirementId: string }`. 404 if plan missing. Appends at `maxOrder+1`.
Response: created `PlanRequirement` row `{id, planId, requirementId, order, createdAt}`. Status **200**.

### `DELETE /plans/:id/requirements/:requirementId` (requirements.ts:21-29)
Response: `{ "ok": true }`. Status **200**.

### `POST /plans/:id/requirements/reorder` (requirements.ts:30-42)
Body: `{ requirementIds: string[] }`. Transactionally sets `order` to array index for each id.
Response: `{ "ok": true }` (route explicitly, `.map(() => ...)` not used here — same discard pattern as delete). Status **200**.

### `POST /plans/:id/tasks` (tasks.ts:46-98)
Body: `{ title: string, description?, acceptanceCriteria?: (string | {text,done})[], chunks?: [{chunkId, relation}], dependsOnTaskIds?: string[], metadata? }`.
`acceptanceCriteria` accepts legacy `string[]` or `{text,done}[]`; normalised to `{text,done}[]` before insert (`normaliseCriteriaForWrite`, tasks.ts:17-20) — strings become `{text: item, done: false}`.
`chunks[].relation` must be one of `context|created|modified` (400 `ValidationError` otherwise).
Response: created `PlanTask` row (status forced to `"pending"` regardless of any input). Status **200**.

### `PATCH /plans/:id/tasks/:taskId` (tasks.ts:99-146)
Body: `{ title?, description?: string|null, acceptanceCriteria?, status?, metadata? }`.
`status` must be one of `pending|in_progress|done|skipped|blocked`. Setting `status: "done"` triggers `unblockDependentsOf` — any task depending on this one that is currently `blocked` flips to `pending` (side effect not reflected in this endpoint's own response body).
Response: updated `PlanTask` row. Status **200**.

### `DELETE /plans/:id/tasks/:taskId` (tasks.ts:147-166)
Response: `{ "ok": true }`. Status **200**.

### `POST /plans/:id/tasks/reorder` (tasks.ts:167-179)
Body: `{ taskIds: string[] }`. Response: `{ "ok": true }`. Status **200**.

### `POST /plans/:id/tasks/:taskId/chunks` (tasks.ts:180-192)
Body: `{ chunkId: string, relation: string }` (`relation` validated same as create-task). Response: created `PlanTaskChunk` row `{id, taskId, chunkId, relation}`. Status **200**.

### `DELETE /plans/:id/tasks/:taskId/chunks/:linkId` (tasks.ts:193-201)
Response: `{ "ok": true }`. Status **200**.

### `POST /plans/:id/tasks/:taskId/dependencies` (tasks.ts:204-214)
Body: `{ dependsOnTaskId: string }`. **No cycle check visible in this handler** (raw insert into `planTaskDependency`) — worth flagging to implementers, not asked for by the four questions but a real gap. Response: created `PlanTaskDependency` row `{id, taskId, dependsOnTaskId}`. Status **200**.

### `DELETE /plans/:id/tasks/:taskId/dependencies/:depId` (tasks.ts:215-223)
Response: `{ "ok": true }`. Status **200**.

### `POST /plans/:id/tasks/:taskId/links` (tasks.ts:233-256)
Body: same shape as plan links. Response: created `PlanTaskExternalLink` row `{id, taskId, system, url, label, order, createdAt}`. Status **200**.

### `DELETE /plans/:id/tasks/:taskId/links/:linkId` (tasks.ts:257-265)
Response: `{ "ok": true }`. Status **200**.

### `POST /plans/:id/analyze` (analyze.ts:49-78)
Body: `{ kind: string, chunkId?, filePath?, text?, metadata?: Record<string, string|number|boolean|null> }`. `kind` must be one of `chunk|file|risk|assumption|question` (400 otherwise). Appends at `maxOrder+1` scoped to `(planId, kind)`.
Response: created `PlanAnalyzeItem` row `{id, planId, kind, order, chunkId, filePath, text, metadata, createdAt, updatedAt}`. Status **200**.

### `PATCH /plans/:id/analyze/:itemId` (analyze.ts:79-97)
Body: `{ text?, metadata?, chunkId?, filePath? }` — **note: `kind` is not patchable here** (no field for it in the body schema). Response: updated `PlanAnalyzeItem` row. Status **200**.

### `DELETE /plans/:id/analyze/:itemId` (analyze.ts:98-106)
Response: `{ "ok": true }`. Status **200**.

### `POST /plans/:id/analyze/reorder` (analyze.ts:107-120)
Body: `{ kind: string, itemIds: string[] }`. `kind` validated. Reorder is scoped per-kind (a reorder call only touches items of that kind). Response: `{ "ok": true }`. Status **200**.

---

## Search — `packages/api/src/search/routes.ts`

### `POST /search/query` (routes.ts:17-44)
Body: `{ clauses: [{field, operator, value, params?: Record<string,string>, negate?: boolean}], join?: "and"|"or" (default "and"), sort?: "relevance"|"newest"|"oldest"|"updated", limit?: number, offset?: number, spaceId?: string }`.
Handler `executeSearch` (`search/service.ts:77-255`) never fails — the whole pipeline is wrapped in `Effect.orElse(() => Effect.succeed({chunks:[],total:0}))` at the end, and every internal repo call has its own `Effect.orElse` fallback, so this endpoint has no error path that reaches the global handler under normal conditions.
Response shape: `{ chunks: SearchResultChunk[], total: number, graphMeta?: GraphMeta, duplicateHints?: DuplicateHint[] }`. `graphMeta` is present only when at least one graph clause (`near|path|affected-by|similar-to`) was in the query; `duplicateHints` is present only when `findDuplicatePairs` found overlap among the result set (undefined, i.e. **absent**, otherwise — `service.ts:253`, `duplicateHints.length > 0 ? duplicateHints : undefined`). See `_questions.md` Q2 for the `graphMeta.type` literal values, confirmed by source (not executed — see the top-of-file rationale).
Status **200** always (no explicit status set; falls through the empty-catch success path too, since the Effect never fails).

### `POST /search/saved` (routes.ts:76-104)
Body: `{ name: string (max 200), query: {clauses, join?, sort?: string, spaceId?}, spaceId?: string }`.
Response: created `SavedQuery` row via `.returning()` (`repository/saved-query.ts:18-23`) — `{id, name, query (jsonb), userId, spaceId, createdAt, ...}` (full column set, whatever `savedQuery` schema declares — not independently re-verified here since no saved_query rows exist to capture live). Status **200**.

### `DELETE /search/saved/:id` (routes.ts:105-112)
No body param beyond `:id`. Deletes scoped to `(id, userId)` — a query for someone else's saved query silently deletes nothing (`deleteSavedQuery` returns `deleted ?? null`, route ignores that value).
Response: **`{ "message": "Deleted" }`** (`.map(() => ({ message: "Deleted" }))`, routes.ts:109) — note this is a different shape from the plans domain's `{ok:true}` convention; the search domain uses the `{message}` convention seen elsewhere in the app (matches 2b's finding of mixed ack shapes). Status **200**, even when nothing was actually deleted (no 404 on missing/foreign id).

---

## Staleness — `packages/api/src/staleness/routes.ts`

### `POST /chunks/:id/dismiss-staleness` (routes.ts:44-55)
No body. Handler: `dismissStaleFlag(flagId, userId)` → `repository/staleness.ts:91-95`:
```ts
export function dismissStaleFlag(flagId: string, userId: string) {
    return dbEffect(() =>
        db.update(chunkStaleness).set({ dismissedAt: new Date(), dismissedBy: userId }).where(eq(chunkStaleness.id, flagId))
    );
}
```
⚠ **No `.returning()`, and the arrow function's expression body returns the Drizzle update builder directly (not wrapped/discarded like the plans-domain deletes).** The awaited value is therefore whatever `drizzle-orm/node-postgres`'s update-without-`.returning()` resolves to — the underlying `pg` driver's raw `QueryResult` (fields like `command`, `rowCount`, `rows: []`, `fields: []`, `oid`), **not** a curated `{ok:true}`/`{message}` ack and **not** the updated row. The route does no transform (`routes.ts:46-49`) — whatever this Effect resolves to is serialized straight to the client. Also **no existence check** — dismissing a nonexistent flag id updates zero rows and still returns 200 with this same shape (`rowCount: 0`). Status **200**.
**This needs live confirmation against a disposable DB before implementers copy exact field names** — the shape is a driver implementation detail, not app-level API contract, and wasn't executed here per the GETs-only restriction.

### `POST /chunks/suppress-duplicate` (routes.ts:56-68)
Body: `{ chunkIdA: string, chunkIdB: string }`. Handler `suppressDuplicatePair` (`repository/staleness.ts:97-110`) has the **identical pattern** — no `.returning()`, expression-bodied arrow returning the update builder directly. Same ⚠ as above: raw driver `QueryResult`, not app-shaped JSON, unverified live. Only rows with `reason = "diverged_duplicate"` and undismissed match the `WHERE`; since `diverged_duplicate` is never written by any runtime code path (see `_questions.md` Q4), **this update statement can only ever match zero rows in practice** — it always resolves to a zero-row-affected `QueryResult`. Status **200** regardless.

### `POST /chunks/stale/scan-age` (routes.ts:69-92)
Body: `{ spaceId?: string, thresholdDays?: number }`. Runs `detectAgeStaleChunks` + `detectUncoveredChunks` in parallel (`Effect.all`), sums their `flagged` counts.
Response: `{ "flagged": number }` (sum of both scans' insert counts). Status **200**.

### `POST /chunks/:id/scan-impact` (routes.ts:93-105)
Body: `{ title?: string }` (defaults to `"Unknown"` if omitted). Handler `flagImpactRipple` (`detect-impact.ts:5-32`): computes ripple targets via `computeImpactRipple`, skips already-flagged (same `updatedChunkId` in `relatedChunkId`), inserts new `upstream_impact` flags for the rest.
Response: `{ "flagged": number }`. Status **200**.
