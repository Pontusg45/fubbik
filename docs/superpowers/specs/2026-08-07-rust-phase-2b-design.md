# Rust Rewrite Phase 2b — The Organisation Layer

**Date:** 2026-08-07
**Status:** Approved design, pending implementation plan
**Follows:** `2026-08-03-rust-phase-2a-design.md` (tags, spaces, connections, stats)

## Why this slice

Phase 2a took the Rust backend from 6 to 11 of the 26 domains the web app calls. Ordering
the remainder by web call sites per unit of porting cost puts this group next — every one
of them ranks above `requirements`, the largest remaining domain:

| Domain | TS LOC | Web calls | Endpoints |
| --- | --- | --- | --- |
| notifications | 98 | 5 | 5 |
| settings | 146 | 6 | 7 |
| activity | 52 | 2 | ~1 |
| workspaces | 191 | 7 | 7 |
| favorites | 98 | 3 | 4 |
| collections | 151 | 3 | 5 |
| *(for contrast)* requirements | 1,089 | 28 | — |

Roughly **736 LOC and ~29 endpoints for 26 web call sites**. Everything is unblocked:
`workspace_space` needs `space` (ported in 2a), and `collection` and `activity_log` both
reference `space` and `user`.

## Scope

Six domains, every endpoint in each. Whole domains, not subsets — the reasoning from 2a
still holds: choosing and defending a subset costs more than porting the rest.

### Out of scope

- The remaining nine domains (requirements, matrices, plans, search, features, context,
  documents, vocabulary, templates, ai, graph and the rest). Later slices.
- The web migration. It stays blocked until enough domains exist for the app to boot; the
  Proxy client remains parked at `apps/web/src/utils/api-proxy.future.ts`.

## Two findings that change what this slice is

Both came from reading the schema before writing the spec, which is the habit 2a
established.

### Collections are saved queries, not containers

`collection` has `(id, name, description, filter jsonb, user_id, space_id, created_at,
updated_at)` and **there is no join table**. A collection does not hold chunks — it holds a
filter, and its contents are computed by evaluating that filter.

So porting collections means porting a filter evaluator. The decisive question, which the
contract capture must answer before any code: **does the stored `filter` JSON map onto the
`ListParams` already built for the chunks list endpoint, or is it a separate dialect?**

If it maps, this is a small task reusing existing query-building. If it is its own dialect,
it needs an evaluator, and the effort estimate for this domain is wrong. Phase 1 supports
only seven of the chunk list's query parameters — `type`, `search`, `limit`, `offset`,
`sort`, `origin`, `reviewStatus` — so a filter referencing tags, spaces or scope may exceed
what exists today even if the shape matches.

### Settings is three key-value stores at different scopes

- `user_settings` — `(id, user_id, key, value jsonb, updated_at)`
- `codebase_settings` — `(id, space_id, key, value jsonb, updated_at)`; the name predates
  the codebase→space rename and is still what the table is called
- `instance_settings` — `(key, value jsonb, updated_at)`, **no `user_id`** — these rows are
  global

**Authorization on instance settings, decided:** faithful port. Node's routes use only
`requireSession` with no role or admin check (`packages/api/src/settings/routes.ts:60`), so
any authenticated user can read and write instance-wide settings. The Rust port does the
same.

Recording the resulting property so nobody has to rediscover it: **any authenticated user
can write global instance settings.** That is acceptable for a local-first single-user tool
and matches the reference implementation, but it is a real characteristic of the system, not
an oversight, and it should be revisited if fubbik ever becomes multi-tenant in earnest.

## Architecture

No new architecture. Everything reuses what 2a proved:

- Repository → service → route; `AppResult<T>` from `fubbik-core`; `ApiError` owns the HTTP
  concern; `fubbik-core` has no web dependency.
- `#[serde(rename_all = "camelCase")]` on every wire type.
- `UtcTimestamp` for every exposed timestamp — a raw `NaiveDateTime` serialises without a
  `Z`, which JavaScript parses as local time.
- User scoping in SQL, never left to the caller.
- An `id` tiebreaker on every list query, so ordering is total. 2a's live run proved that
  ordering over tied timestamps can skip or duplicate rows under `LIMIT`/`OFFSET`.
- Plain `Option<&str>` for nullable columns; the `deserialize_some` tri-state for any
  nullable field Node lets a client clear with an explicit `null`.
- Enum-typed DTO fields for any constrained set, so invalid values are rejected at
  deserialisation rather than reaching the database.

### The fourth join table

`workspace_space` is `(workspace_id, space_id)` with no ownership of its own — the same
shape as `chunk_tag` and `chunk_space`. It uses the pattern 2a established and proved:

1. Parent-A ownership guarded on INSERT
2. Parent-B ownership guarded on INSERT
3. Parent ownership guarded on the **DELETE half** of any replace-set operation

The third is the one that matters most and is easiest to miss: without it, a
correctly-rejected call still wipes the victim's existing rows before returning. Each guard
must be proven load-bearing by removing it and watching a named test fail.

## Process

Task 1 captures Node's response contract for all ~29 endpoints before any Rust is written,
as in 2a. That change paid for itself immediately there — its first finding was that list
endpoints return bare arrays rather than the envelope the chunks domain uses, which would
otherwise have been guessed wrong five times and discovered only at the end.

For this slice the capture must additionally answer:

1. Does the collection `filter` JSON match the chunks `ListParams` shape? Give the exact
   stored JSON from real rows.
2. What do the settings endpoints return — is `/settings/features` a fourth store or a
   computed view? Its presence alongside user/instance/codebase suggests it is derived.
3. Do these list endpoints return bare arrays like 2a's, or envelopes like chunks?
4. What does `activity` actually expose? It has 52 LOC and roughly one endpoint; confirm
   the shape rather than assuming a simple list.

## Testing

- Each domain lands with its differential-harness paths added.
- Cross-user isolation tests at the HTTP boundary for every endpoint touching user data,
  asserting 404 **and** unchanged victim data.
- `#[sqlx::test(migrations = "../fubbik-db/migrations")]` on every API test.
- Ordering-stability tests for every list query, each verified to fail without its
  tiebreaker. 2a found one that needed 20 rows plus an explicit `ANALYZE` before the
  planner would expose the bug — a smaller test passed regardless and proved nothing.

## Risks

| Risk | Mitigation |
| --- | --- |
| Collections' filter is its own dialect, blowing the estimate | Contract capture answers it before code; re-scope if so |
| Instance settings' global writes surprise someone later | Recorded explicitly above as a known property |
| `workspace_space` scoping wrong in one direction only | Both directions tested; each guard proven by removal |
| Three settings tables invite three inconsistent implementations | One shared key-value pattern, three scopes |
| List ordering non-total, as 2a found in both stacks | `id` tiebreaker plus a failure-verified stability test |

## Outcome

Delivered: all six domains, **29 endpoints — zero omissions**, verified by counting both
stacks' routers and cross-checking `openapi.json`. **355 tests** (from a baseline of 208), plus
6 ignored cross-stack tests. Clippy clean at `-D warnings`, fmt clean, offline check clean,
migrations unchanged, `.sqlx` additive-only, and `apps/web` **and** `packages/` byte-identical
to the branch base with the web type check at zero errors.

### Both pre-work findings held

Collections did **not** need a filter evaluator. Task 1's capture found that
`GET /collections/{id}/chunks` builds no query of its own — it calls the same `listChunks` as
`GET /api/chunks` (`packages/api/src/collections/service.ts:59-78`). The domain reduced to
extending `ListParams` with four missing keys. The spec's re-scope branch never triggered.

That extension did carry a real cost the spec did not anticipate: it changed `GET /api/chunks`,
a **Phase 1** endpoint and the only one the differential harness covers. It now accepts five
query params it lacked (`tags`, `after`, `enrichment`, `minConnections`, `spaceId`) — a
deliberate parity improvement, recorded rather than discovered.

### The join-table pattern held for a fourth table

`workspace_space`'s three guards were each proven load-bearing by removal. The DELETE-half
guard — the silent-data-loss case — has a named test that asserts `!removed` *before* checking
the association survived, so it fails on real data loss rather than on a status code.

### The finding that outlived the slice: API tests cannot detect a removed SQL guard

Confirmed in **five consecutive tasks**, in both directions: remove a repo's SQL ownership
predicate and the repo test fails while the **API test still passes**, because the service-layer
pre-check 404s first; remove the service pre-check and only the API tests fail. Neither layer's
suite alone catches both.

The guards are correct — this is a test-placement property, not a defect. But it means an
API-only suite gives false confidence about scoping, the most security-relevant property of the
port. Every later slice should test both layers and state which layer caught each removal.

**It bit for real, and only the whole-branch review could see it.** `spaces_for_workspace`'s
ownership guard (`crates/fubbik-db/src/repo/workspace.rs:287-288`) was correct but had zero
tests at either layer — all eight call sites paired a user with their own workspace. Deleting
the guard left 17/17 repo and 17/17 API tests green. Every per-task review missed it because
each asked "do the guards I was told about fail when removed?", never "is there a guard nobody
wrote a test for?"

### Deliberate divergences from Node, continuing 2a's list

8. `workspace_space` carries SQL-level ownership guards Node lacks — Node relies purely on two
   app-level pre-checks. Observable behaviour identical; defense-in-depth only.
9. `codebase_settings` is ownership-scoped. **In Node, any authenticated user can read and write
   any space's settings by guessing its id** — `routes.ts:29-49` passes `ctx.query.codebaseId`
   straight through and `service.ts:27-40` forwards it with no check. Rust 404s where Node 200s.
10. `GET /activity`'s `spaceId` filter is ownership-checked. Rust 404s where Node returns
    200 `[]`. **Not a leak in Node** — rows are already user-scoped, so a foreign `spaceId`
    intersects zero of the caller's own rows.
11. **Pre-existing from Phase 1, found by this slice's final review and not yet resolved:** the
    chunk-list limit clamp differs — Node caps at 100 (`packages/api/src/chunks/service.ts:50`),
    Rust at 500 (`crates/fubbik-api/src/chunks/dto.rs:121`), so `?limit=200` returns 100 rows
    versus 200. **The harness cannot see it**: its only limit case is `?limit=5`, below both
    caps. Decide in 2c whether to align to 100 or keep 500 deliberately, and add a harness case
    above both caps.

### The live differential run

4 passed, 2 failed — both the same root cause, both divergence #6. Six seeded chunks share the
identical `created_at` `2026-06-02 23:39:08.829586` (a bulk-seed artifact). Node's
`desc(chunk.createdAt)` has no tiebreaker, so their order is *undefined*; Rust's `id` tiebreaker
makes it deterministic and therefore different. Correctly left on sequence comparison rather
than silenced into the unordered set.

**The harness diffs GET endpoints only.** A clean run does not mean parity — divergences 1-5 and
7-11 are invisible to it by construction.

### Carried to 2c

- 404 message casing splits by domain: `tags`, `tag_types` and `chunks` emit lowercase where
  Node capitalizes, while `spaces` and `connections` are correct. `error_responses.rs:161`
  **pins the wrong form** with a passing assertion, so fixing it means editing that assertion.
- Divergence #11 above.
- Connections still do not project into the AGE graph (carried from 2a).

## What this slice does NOT deliver

The web app still will not run on Rust. It calls 26 domains; after this slice **17 exist**.
The remaining nine include the four largest (`requirements`, `context`, `plans`, `matrices`),
so the UI stays blocked until roughly the end of the next slice.
