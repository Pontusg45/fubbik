# Rust Rewrite Phase 2c — Unblocking the Web App

**Date:** 2026-08-08
**Status:** Approved design, pending implementation plan
**Follows:** `2026-08-07-rust-phase-2b-design.md` (notifications, favorites, workspaces, settings, activity, collections)

## Why this slice

Earlier slices ordered domains by web call sites per unit of porting cost. This one is chosen
differently, because the ranking had started optimising the wrong thing: the web app has been
blocked for three phases, and the deferred-migration risk compounds every slice.

Three unported domains are the entire critical path to a booting, navigable dashboard:

| Domain | api LOC | api+db LOC | Endpoints | Why it blocks the shell |
| --- | --- | --- | --- | --- |
| plans | 858 | ~1,733 | 29 | `ActivePlanCard` renders on the dashboard |
| staleness | 163 | ~485 | 6 (5 in scope) | staleness banner + nav badge render on the dashboard |
| search | 659 | +33 | 6 | header nav calls saved/parse/autocomplete on load and keystroke |

Total in scope: **40 endpoints**.

`dashboard.tsx` itself calls only `stats`, which is ported. What breaks it is
`apps/web/src/features/dashboard/active-plan-card.tsx` (plans) and the staleness banner
(`chunks.stale`). The header bar
(`apps/web/src/features/nav/header-search-bar.tsx`,
`use-header-search-suggestions.ts`) calls `GET /search/saved` on load and
`GET /search/parse` / `GET /search/autocomplete` on interaction.

Every other unported domain — `matrices`, `graph`, `requirements`, `features`, `context`,
`coverage`, `vocabulary`, `documents`, `templates` — is reached only by an explicit nav click,
not by app-shell load.

## Scope

**40 endpoints across three domains.** Whole domains, with one deliberate exception.

- **plans (29)** — `routes.ts` 10, `requirements.ts` 3, `analyze.ts` 5, `tasks.ts` 11.
  The count is 29, not the 10 a `routes.ts`-only reading suggests. Sibling route files carry
  endpoints, the same trap that under-counted `requirements` in an earlier analysis. **Any
  estimate derived from `routes.ts` alone is wrong.**
- **search (6)** — `POST /search/query`, `GET /search/parse`, `GET /search/autocomplete`,
  and saved-search CRUD (`GET`/`POST`/`DELETE /search/saved`).

- **staleness (6)** — list, count, dismiss, suppress-duplicate, scan-age, **and scan-impact**.

**41 endpoints total**, plus wiring Apache AGE into live routes.

### Scope correction, made during planning

This spec originally scoped staleness at 5 of 6, deferring `scan-impact` because it needs AGE,
on the stated premise that it was the only AGE-dependent endpoint in the slice. **That premise
was wrong.**

`packages/api/src/search/service.ts:24` declares
`const GRAPH_FIELDS = new Set(["near", "path", "affected-by", "similar-to"])`, and those
clauses call `getNeighborhood` and `findShortestPathWithDetails` — AGE Cypher queries. Search
is chunk-only in its *response shape* (always `SearchResultChunk[]`); it is **not** chunk-only
in its implementation.

Each graph clause is wrapped in `Effect.orElse(() => Effect.succeed([]))`, so shipping without
AGE would not crash — it would silently return nothing for queries the UI advertises
(`apps/web/src/features/search/query-input.tsx:21`'s placeholder is
`type:note tag:architecture near:chunk-id:2 NOT text:deprecated`).

**Decision, on corrected facts: wire AGE in this slice**, and fold `scan-impact` back in.

### Second scope correction: the AGE write sync is in

Wiring AGE for **reads** alone would have shipped an inert feature. Ported `connections` never
call `ensureVertex`/`createEdge`/`deleteEdge`, so nothing the Rust stack writes reaches the
graph. Search's `near:`/`path:` clauses would execute correctly against an edge set Rust never
populates — returning empty in real use, while tests that seed vertices directly via Cypher
would pass. Exactly the kind of gap that looks solved and is not.

**So this slice also ports the write half**, matching Node's 8 call sites in
`packages/db/src/repository/connection.ts`: `ensureVertex` twice plus `createEdge("connects", …)`
on create, `deleteEdge` on delete. The AGE gap closes completely here rather than half here and
half later.

**This modifies `connections`, a Phase 2a domain.** Together with the chunks limit clamp, 2c
touches two already-merged domains. Both are deliberate; neither may regress. `connections` is
covered by the differential harness, so its no-regression evidence matters as much as the new
behaviour.

**Backfill, decided:** existing `chunk_connection` rows created by the Rust port were never
projected. The slice ships a one-time, **idempotent** backfill that walks `chunk_connection` and
issues `ensureVertex`/`createEdge` per row, safe to re-run. It runs **explicitly**, not
automatically on startup — an implicit graph rewrite at boot is the kind of thing that should
never surprise anyone. Re-running it must not duplicate edges; that needs a test.

### Out of scope

- **The requirements domain.** Plans can be ported completely without it.
- **The web migration itself.** This slice makes the app *able* to boot; flipping `apps/web`
  onto Rust stays parked at `apps/web/src/utils/api-proxy.future.ts`.

## Findings that shaped this design

All four came from reading the source before writing the spec — the habit 2a established and
2b confirmed pays for itself.

### Search is chunk-only, so there is no degradation problem

The obvious worry — that a cross-entity search would return empty categories for unported
entities — does not exist. `executeSearch` (`packages/api/src/search/service.ts:77-255`) only
ever queries the `chunk` table via the already-ported `listChunks`. `SearchResult`
(`search/types.ts:48-59`) has a single `chunks[]` field. Matrices, vocabulary and documents
have **zero references** in `search/`.

Requirements touch search in exactly two places — `autocomplete(field="requirement")` and the
`affected-by` graph clause — and **both are already wrapped in
`Effect.orElse(() => Effect.succeed([]))`**, so they degrade to empty rather than erroring.

Search ships whole, with no divergence.

### Plans is portable without requirements

`plan_requirement` links to `requirement`, which has no API. But `listPlanRequirements`
(`packages/db/src/repository/plan.ts:294-297`) returns bare join rows with no title/status
join, and `addPlanRequirement` (`plan.ts:300-311`) needs only the FK target to exist — the
`requirement` table is present in the Rust migrations (`0001_init.sql:618`).

**No plan endpoint is broken by requirements being unported.** Linked-requirement *titles* are
not resolvable until requirements ships, which is a UI limitation, not a port blocker.

### The `file_changed` staleness reason is unimplemented

It is documented (`packages/db/src/repository/staleness.ts:15`) and seeded
(`seed/modules/chunks.ts:857`), but **has no writer anywhere in the codebase**. There is
nothing to port. The Rust port must not invent one — an implementer reading the docs would
reasonably try.

### `scan-age` does more than its name says

`POST /chunks/stale/scan-age` (`staleness/routes.ts:75-77`) synchronously runs **both**
`detectAgeStaleChunks` and `detectUncoveredChunks`. The latter emits a third reason,
`requirement_uncovered`, by querying `requirement_chunk` — whose schema exists in the Rust
migrations (`0001_init.sql:621`). So it ports and runs correctly, but returns nothing
meaningful until the requirements domain ships. Expected, not a defect.

The background scan (`STALENESS_SCAN_INTERVAL_HOURS`) is an in-process `setInterval` in
`initStartupTasks()` (`packages/api/src/startup.ts:59-91`) calling only `detectAgeStaleChunks`.
It needs no git or filesystem access — it is a pure `updated_at` comparison. Porting the
scheduled job is **in scope**; it is a few lines beside the existing startup wiring.

## Architecture

No new architecture. Repository → service → route, `AppResult<T>` from `fubbik-core`,
`ApiError` owning the HTTP concern, `#[serde(rename_all = "camelCase")]` on every wire type,
`UtcTimestamp` on every timestamp, user scoping in SQL, an `id` tiebreaker on every list query,
`deserialize_some` for tri-state nullable fields, utoipa annotations with a guarded
`openapi.json`.

Three things in this slice have no analogue in 2a or 2b.

### The first transactional write

Every port so far has been single-statement. Marking a task done must atomically unblock its
dependents.

Node does **not** do this atomically. `unblockDependentsOf`
(`packages/db/src/repository/plan.ts:536-551`) is a `SELECT` then an `UPDATE`, invoked as a
*second, separate* `dbEffect` after `updateTask` (`plans/tasks.ts:119-121`). A failure between
them leaves a task done with its dependents still blocked.

**Decision: fix it.** The Rust port wraps mark-done and unblock in one `sqlx` transaction —
**deliberate divergence #12**, the same shape as accepted divergences #4, #9 and #10 where the
port is stricter than Node.

This introduces a transaction pattern the codebase does not have. It must be established once,
deliberately, and copied — every later domain with a multi-step write depends on it. The test
must prove atomicity by **forcing a failure mid-transaction and asserting the first statement
rolled back**. A test that merely asserts both statements ran proves nothing about atomicity.

### A two-level ownership chain

`plan → plan_task → plan_task_chunk`. All seven child tables — `plan_requirement`,
`plan_analyze_item`, `plan_task`, `plan_task_chunk`, `plan_task_dependency`,
`plan_external_link`, `plan_task_external_link` — FK-cascade to their parent and **none carries
its own `user_id`** (`packages/db/src/schema/plan.ts:24-25,47-97,109-196`). Ownership derives
entirely from `plan.userId`.

Every prior join derived ownership from **one** parent. This derives through **two**, so a
guard on the wrong level is both easy to write and invisible to a test that only checks the
top level.

### The first live use of Apache AGE

`crates/fubbik-db/src/age.rs` (233 LOC) has six passing unit tests and **zero callers** — it is
dead code today. This slice makes it live, for reads only: `getNeighborhood`,
`findShortestPathWithDetails`, `getChunksAffectedByRequirement` (search's graph clauses) and
`computeImpactRipple` (`scan-impact`).

Phase 1's AGE spike established the one non-obvious mechanic: extraction must use `::varchar`,
**not** `::text`. An explicit text cast routes through `agtype_value_to_text`, which rejects
vertex, edge and path values with `unsupported argument agtype 6`. That is already encoded in
`age.rs`; implementers must not "simplify" it.

Node wraps every graph clause in `Effect.orElse(() => Effect.succeed([]))`. The Rust port must
degrade the same way — a graph query failing because AGE is unavailable returns empty results,
not a 500. There must be a test for the unavailable path, not only the happy path.

### A query-language parser

`GET /search/parse` parses a search DSL with clauses including `affected-by` and `similar-to`.
This is the first port where the interesting failures are in **parsing**, not in SQL scoping —
malformed input, precedence, unknown clauses, partial matches. It needs table-driven tests over
real query strings, and its estimate is the least trustworthy in this slice because nothing
prior resembles it.

Text search uses `pg_trgm`'s `%` operator plus `ilike`; `similar-to` uses pgvector `<=>`. Both
extensions are present in the Rust migrations (`0001_init.sql:1-2`). The Ollama dependency is
confined to `similar-to` and already degrades gracefully.

## Deliberate divergences introduced by this slice

**#12 — mark-done and unblock become atomic.** Node's `unblockDependentsOf`
(`packages/db/src/repository/plan.ts:536-551`) is a `SELECT` then an `UPDATE`, invoked as a
separate `dbEffect` after `updateTask` (`plans/tasks.ts:113-122`). A failure between them
leaves a task `done` with dependents still `blocked`. Rust wraps both in one transaction.
Unblock changes a dependent from exactly `"blocked"` to `"pending"`, and only from `"blocked"` —
tasks in other states are untouched even when they depend on the completed task.

**#13 — the plans domain gets ownership enforcement Node lacks.** This is the largest
divergence in the port so far and it was found while planning, not designing.

`packages/db/src/repository/plan.ts:145-150`:
```ts
export function getPlan(id: string): Effect.Effect<Plan | null, DatabaseError> {
    return dbEffect(async () => {
        const [row] = await db.select().from(plan).where(eq(plan.id, id)).limit(1);
        return row ?? null;
    });
}
```
Selected by `id` alone. The service wrapper adds a 404-if-missing check but no ownership check,
and `userId` appears in the plans routes only for activity logging and for create/list/duplicate.
**Any authenticated user can read and write any other user's plan by id** — its tasks, analyze
items, requirement links and external links. Only `GET /plans` filters by owner, so the data is
hidden from browsing but fully reachable directly. 26 of 29 endpoints are affected.

Rust scopes every route through `plan.user_id`, returning 404. **The differential harness runs
as a single user and cannot see this**, so it needs explicit cross-user tests at both layers.

**#14 — `dismissStaleFlag` gets an ownership guard.** `dismissStaleFlag(flagId, userId)`
updates by flag id and merely stamps `dismissedBy` — any authenticated user can dismiss any
flag. Same family as #13; scoped through the flag's parent chunk.

## Facts that must not be "improved" in the port

Each of these looks like a bug and is not this slice's to fix. Every one was verified in source.

- **All four enum-like fields are unconstrained free text**, at both the Elysia schema level and
  the DB level: `plan.status`, `plan_task.status`, `plan_task_chunk.relation`,
  `plan_analyze_item.kind`. The only enforcement is an `Array.includes` check in a service
  function raising `ValidationError`. There is no Postgres enum and no CHECK constraint — the
  `plan*` tables have no CHECK migrations at all. **Replicate as an application-layer check.**
  Phase 2b hit this trap twice (`notification.type`, `activity_log.action`).
  Allowed values: `plan.status` ∈ {draft, analyzing, ready, in_progress, completed, archived};
  `plan_task.status` ∈ {pending, in_progress, done, skipped, blocked};
  `plan_task_chunk.relation` ∈ {context, created, modified};
  `plan_analyze_item.kind` ∈ {chunk, file, risk, assumption, question}.
- **Every plans endpoint returns 200**, including every `POST`. No route sets a status; none
  returns 201.
- **`SearchQuery.join` (`"and" | "or"`) is dead** — accepted by both route schemas, never read
  in `service.ts`. All params are AND-combined by `listChunks`'s SQL. Port it as an accepted,
  ignored field. Implementing OR would be a behaviour change.
- **`graphMeta.type` is declared as three literals but assigns a fourth at runtime** —
  `service.ts:123` writes `"semantic" as any` for `similar-to`. Preserve the runtime value.
- **`DELETE /search/saved/:id` never 404s** — it ignores the delete result and always returns
  `{message:"Deleted"}`. Same shape as favorites in 2b.
- **`file_changed` staleness has zero writers** anywhere in the repo — schema comment, a seed
  doc string, and dead frontend icon-mapping only. Nothing to port; do not implement it.
- **The parser never throws.** Unknown fields become `{field, operator:"is", value}` and are
  silently ignored downstream. `connections:abc+` yields `Number("abc")` → `NaN`. The grammar
  is case-sensitive: `NOT` must be uppercase, and `Tag:x` is not `tag:x`.
- **`hops:N` attaches only to a preceding `near` clause**, by scanning backwards for the most
  recent one. After `affected-by:`, it is silently dropped, and the service defaults to 2.
- **Reorder endpoints leave unmentioned rows untouched** — per-item `UPDATE ... SET order=i`
  inside a transaction, not a wholesale renumber.
- **`plan_task_dependency` has no self-reference guard** — `taskId = dependsOnTaskId` is
  unenforced at DB and code level in Node.

## Testing

Carrying 2b's lessons forward, plus one new obligation:

- **Both layers, always.** The finding confirmed across five consecutive 2b tasks: API-level
  tests **cannot** detect a removed SQL ownership guard, because the service pre-check 404s
  first. Only repo-level tests catch it. Every guard is proven load-bearing by removal, and
  every removal experiment **names which layer's test failed**. This matters most here, where
  ownership derives through two levels.
- **Guards nobody wrote a test for.** 2b's whole-branch review found a correct guard with zero
  tests at either layer — deleting it left 34 tests green. Enumerate the guards, then check
  each has a test, rather than only testing the guards a task brief happens to mention.
- **Atomicity**, proven by forced mid-transaction failure.
- Ordering-stability tests seeded with **forced-identical** sort keys plus an explicit
  `ANALYZE`; distinct keys pass regardless and prove nothing.
- Cross-user tests asserting **404 and unchanged victim data**.
- `#[sqlx::test(migrations = "../fubbik-db/migrations")]` on every API test.
- Differential-harness paths added per domain. The harness diffs **GETs only** — a clean run
  never means full parity. It also runs as a **single user**, so it cannot see divergences
  #13/#14 at all; those need dedicated cross-user tests.
- **Queries with no `orderBy` in Node must be compared as multisets, not sequences.** Verified:
  `listTaskChunks`, `listTaskChunksWithTitles`, `listTaskDependencies`, `searchChunkTitles`,
  `searchRequirementTitles`, `getTagsForUser`, `getStaleFlagsForChunk`. Everything else in these
  domains has an explicit `orderBy` and stays sequence-compared. Do not blanket-mark endpoints
  unordered — that makes the harness pass by being blind, which is what it exists to prevent.
- **The DSL parser gets table-driven tests** over at least the 18 catalogued query strings,
  including the edge cases that look like bugs: `connections:3` (no `+` → `is`, not `gte`),
  `updated:30` (no `d` → `is`, not `within`), `Tag:api` (case-sensitive miss), and
  `affected-by:x hops:3` (hops silently dropped).
- **AGE degradation** tested explicitly: a graph clause must return empty, not 500, when AGE is
  unavailable — the failure path, not only the happy path.

## Risks

| Risk | Mitigation |
| --- | --- |
| plans at 29 endpoints is the largest domain yet | Split along its four route files; ~4 tasks |
| Estimates from `routes.ts` alone undercount by 3x | Endpoint counts in this spec are from every route file; re-verify per task |
| The DSL parser has no prior analogue | Table-driven tests over real query strings; treat its estimate as soft and re-scope early if wrong |
| Transaction pattern established badly gets copied | Establish once, deliberately, with a forced-failure test; later domains copy it |
| Two-level ownership guarded at the wrong level | Repo-level tests per level, each proven by removal |
| An implementer implements `file_changed` | Recorded above as unimplemented in Node — nothing to port |
| AGE wiring expands into the connections sync | Explicitly out of scope above; reads only in this slice |
| Divergence #13's 404s look like port bugs during the live diff run | The harness runs single-user and will not surface them; cross-user tests carry the proof |
| An implementer models the four free-text fields as Rust enums | Called out per-field above with the exact allowed values and the enforcement layer |
| The connections change regresses a Phase 2a domain | It is harness-covered; no-regression evidence required alongside the new projection |
| The backfill duplicates edges on re-run | Idempotency is a stated requirement with its own test; it runs explicitly, never at boot |

## Carried from earlier slices, still open

- **Divergence #11 — RESOLVED in this slice: align to Node.** The chunk-list limit clamp
  differed — Node caps at 100 (`packages/api/src/chunks/service.ts:50`), Rust at 500
  (`crates/fubbik-api/src/chunks/dto.rs:121`), so `?limit=200` returned 100 rows versus 200.
  2c changes Rust's `.clamp(1, 500)` to `.clamp(1, 100)`, **removing the divergence** rather
  than documenting it. Safe to change now precisely because the web app is not yet on Rust, so
  nothing depends on the larger page size.

  **This must land with a harness case at a limit above both caps** (e.g. `?limit=200`). The
  existing harness could not see the divergence at all: its only limit case is `?limit=5`,
  below both caps. Fixing the clamp without fixing the blind spot leaves the next such
  divergence equally invisible.
- **404 message casing** splits by domain: `tags`, `tag_types` and `chunks` emit lowercase where
  Node capitalises; `spaces` and `connections` are correct.
  `crates/fubbik-api/tests/error_responses.rs:161` **pins the wrong form** with a passing
  assertion, so fixing it means editing that assertion.
- **The AGE gap closes in this slice**, both halves — reads (search's graph clauses,
  `scan-impact`) and writes (the connections vertex/edge sync plus an idempotent backfill). It
  had survived two phases as a carried item. What remains for the graph domain is the `/api/graph`
  endpoints themselves, not the projection.

## What this slice does NOT deliver

The web app will be *able* to boot on Rust; it will not have been migrated. That is deliberate
— the parked Proxy client carries 1,016 type errors from `noUncheckedIndexedAccess`, and
resolving those is its own body of work with its own failure modes.

After this slice, 18 of the 26 domains the web app calls exist in Rust. The remaining eight
include `requirements`, `matrices`, `context` and `graph`.
