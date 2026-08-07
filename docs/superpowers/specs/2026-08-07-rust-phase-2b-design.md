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

## What this slice does NOT deliver

The web app still will not run on Rust. It calls 26 domains; after this slice **17 exist**.
The remaining nine include the four largest (`requirements`, `context`, `plans`, `matrices`),
so the UI stays blocked until roughly the end of the next slice.
