# Rust Rewrite Phase 2a — Tags, Spaces, Connections, Stats

**Date:** 2026-08-03
**Status:** Approved design, pending implementation plan
**Follows:** `2026-07-28-rust-backend-cli-rewrite-design.md` (Phase 1)

## Why this slice

Phase 1 ported the chunks domain. Twenty-six domains remain. Ordering them by web
call sites divided by porting cost produces a steep curve — the top of it is 7–18×
better value than anything below:

| Domain | TS LOC | Web call sites | Calls per 100 LOC |
| --- | --- | --- | --- |
| stats | 22 | 4 | 18.1 |
| health | 19 | 3 | 15.7 *(already built in Phase 1)* |
| tags | 142 | 12 | 8.4 |
| connections | 88 | 7 | 7.9 |
| spaces | 192 | 14 | 7.2 |
| … | | | |
| requirements | 1,089 | 28 | 2.5 |
| matrices | 809 | 15 | 1.8 |
| context | 960 | 1 | 0.1 |

This slice is **~440 LOC and 19 endpoints for 33 web call sites**. Everything in it is
unblocked today: `tag`, `space` and `chunk_connection` hang off `chunk` (ported) plus the
`connection_relation` and `space_kind` catalogs (seeded by migration 0002).

The strategic reason to take it first is not the ratio, though — it is that this slice is
the first to exercise **user-scoped many-to-many join tables** (`chunk_tag`, `chunk_space`).
Nothing in Phase 1 touched that pattern, and every remaining slice depends on it.

## Scope

Five domains, every endpoint in each — 19 total:

- **spaces** (7) — list, detail, create, update, delete, detect-by-remote/path, reset
- **tags** (5) — including a merge operation the web calls
- **tag-types** (4) — catalog CRUD; `tag.tag_type` is an FK
- **connections** (2) — create and delete edges between chunks
- **stats** (1) — aggregate counts

**Whole domains, not subsets.** Phase 1 ported a subset of chunks and that directly caused
the eleven-missing-fields parity gap, which cost a later fix cycle. At 19 endpoints,
porting each domain wholesale is cheaper than choosing, documenting and defending a subset.

### Out of scope

- `workspaces` — depends on `spaces` from this slice; belongs to 2b.
- Everything else in the 26. Later slices.
- The web migration. It stays blocked until enough domains exist for the app to boot;
  the Proxy client remains parked at `apps/web/src/utils/api-proxy.future.ts`.

## The process change: capture Node's contract first

This is the most important change from Phase 1, and it comes directly from what went wrong
there.

In Phase 1 the response envelope (`{chunks, total, limit, offset}` versus a bare array),
eleven missing fields, and the timestamp format were all discovered by the differential
harness **after** the domain was built, then retrofitted across several commits. The
information was available the whole time — nobody looked.

So the first task of this slice records Node's actual behaviour for all 19 endpoints
before any Rust is written: exact JSON response shape, envelope or bare array, field names
and casing, null-versus-empty-array conventions, status codes, and query parameter names
and types. That capture becomes the specification the implementers build against.

Concretely: start the Node server, exercise every endpoint against a seeded database,
and commit the captured responses as fixtures. They are cheap to produce, they make the
contract reviewable before code exists, and they double as test data.

## Architecture

No new architecture. This slice reuses Phase 1's stack unchanged:

- Repository → service → route, with `AppResult<T>` from `fubbik-core`.
- `ApiError` in `fubbik-api` carries the HTTP concern; `fubbik-core` has no web dependency.
- `#[serde(rename_all = "camelCase")]` on every wire type — 106 web files depend on it.
- `UtcTimestamp` for every timestamp, so nothing serialises without a `Z`.
- User scoping pushed **into the SQL**, not left to the caller.
- utoipa annotations on every handler; `openapi.json` regenerated and guarded.

### The one genuinely new pattern: user-scoped join tables

`chunk_tag` and `chunk_space` are many-to-many. Neither carries a `user_id` of its own —
ownership derives from the parent `chunk`. The scoping question is therefore: *whose*
chunks, and *whose* tags?

Phase 1 established that scoping belongs in SQL rather than in the caller, after a review
found `chunk_meta` relying on handlers to call a user-scoped `get` first. The same rule
applies here, via the parent:

```sql
... WHERE ct.chunk_id = $1
    AND EXISTS (SELECT 1 FROM chunk c WHERE c.id = $1 AND c.user_id = $2)
```

Getting this right once is the point of the slice. Every later domain with a join table —
`chunk_feature_delta`, `plan_task_chunk`, `requirement_chunk`, `behavior_cell_code` — copies
whatever this establishes.

**Cross-user attach must be impossible in both directions.** Tagging another user's chunk,
and attaching another user's tag to your own chunk, are distinct holes. Both need tests.

### `tag_type` is user data, not a catalog

Phase 1 seeded `chunk_type`, `connection_relation` and `space_kind` as reference data but
deliberately excluded `tag_type`, which the live database shows as user-managed (8 rows).
So `tag-types` is real CRUD, not a fixed catalog, and `tag.tag_type` is an FK to a table
users can add to and delete from. Deletion needs a decision: restrict while tags reference
it, or cascade. **Match Node** — check before choosing.

## Testing

- Every domain lands with its differential-harness paths added, so parity is proven per
  domain rather than assumed and discovered later.
- Cross-user isolation tests at the HTTP boundary for every endpoint that touches user
  data, asserting 404 **and** that the victim's data is unchanged — a status-only
  assertion would pass even if a rejected write had already mutated something.
- `#[sqlx::test(migrations = "../fubbik-db/migrations")]` on every API test.
- The captured Node fixtures are used as expected values where practical.

## Risks

| Risk | Mitigation |
| --- | --- |
| Join-table scoping wrong in one direction only | Explicit tests both ways; it is the slice's whole point |
| Response shapes diverge as in Phase 1 | Capture Node's contract before implementing, not after |
| `tag_type` deletion semantics differ from Node | Check Node's behaviour explicitly rather than choosing |
| `tags.merge` has non-obvious semantics | Read Node's implementation before specifying it |
| Stale `_sqlx_test_*` databases produce phantom FK failures | Documented; drop before debugging |

## What this slice does NOT deliver

The web app still will not run on Rust. It calls **26 domains** (measured, excluding a grep
artifact); after this slice **6 of them exist** — chunks, spaces, tags, connections, stats,
health. `tag-types` is ported because `tag.tag_type` is an FK, not because the web calls it
directly.

That gap remains until roughly the end of slice 2c. Nothing in this slice should be
described as unblocking the UI, and the Proxy client stays parked.
