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
- **staleness (5 of 6)** — list, count, dismiss, suppress-duplicate, scan-age.
- **search (6)** — `POST /search/query`, `GET /search/parse`, `GET /search/autocomplete`,
  and saved-search CRUD (`GET`/`POST`/`DELETE /search/saved`).

### Out of scope

- **`POST /chunks/:id/scan-impact`** — the sixth staleness endpoint. It calls
  `computeImpactRipple` from `@fubbik/db/age/impact` and needs Apache AGE. Deferred to the
  graph slice, which must close the AGE gap properly (see "Carried" below).
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

### A query-language parser

`GET /search/parse` parses a search DSL with clauses including `affected-by` and `similar-to`.
This is the first port where the interesting failures are in **parsing**, not in SQL scoping —
malformed input, precedence, unknown clauses, partial matches. It needs table-driven tests over
real query strings, and its estimate is the least trustworthy in this slice because nothing
prior resembles it.

Text search uses `pg_trgm`'s `%` operator plus `ilike`; `similar-to` uses pgvector `<=>`. Both
extensions are present in the Rust migrations (`0001_init.sql:1-2`). The Ollama dependency is
confined to `similar-to` and already degrades gracefully.

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
  never means full parity.

## Risks

| Risk | Mitigation |
| --- | --- |
| plans at 29 endpoints is the largest domain yet | Split along its four route files; ~4 tasks |
| Estimates from `routes.ts` alone undercount by 3x | Endpoint counts in this spec are from every route file; re-verify per task |
| The DSL parser has no prior analogue | Table-driven tests over real query strings; treat its estimate as soft and re-scope early if wrong |
| Transaction pattern established badly gets copied | Establish once, deliberately, with a forced-failure test; later domains copy it |
| Two-level ownership guarded at the wrong level | Repo-level tests per level, each proven by removal |
| An implementer implements `file_changed` | Recorded above as unimplemented in Node — nothing to port |

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
- **The AGE gap.** `crates/fubbik-db/src/age.rs` (233 LOC) has 6 passing unit tests and
  **nothing calls it** — it is dead code. Meanwhile Node's
  `packages/db/src/repository/connection.ts` calls `ensureVertex`/`createEdge`/`deleteEdge` at
  8 sites, so the `connects` edge set has already stopped tracking `chunk_connection` in the
  Rust port. The graph slice must close both halves: wire `age.rs` into live routes **and**
  backfill vertex/edge sync into connections. `scan-impact` joins that slice.

## What this slice does NOT deliver

The web app will be *able* to boot on Rust; it will not have been migrated. That is deliberate
— the parked Proxy client carries 1,016 type errors from `noUncheckedIndexedAccess`, and
resolving those is its own body of work with its own failure modes.

After this slice, 18 of the 26 domains the web app calls exist in Rust. The remaining eight
include `requirements`, `matrices`, `context` and `graph`.
