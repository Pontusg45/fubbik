# Rust Phase 2b Implementation Plan — The Organisation Layer

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Port six domains (~29 endpoints) — notifications, settings, workspaces, favorites, collections, activity — taking the Rust backend from 11 to 17 of the 26 domains the web app calls.

**Architecture:** Unchanged. Repository → service → route, `AppResult<T>` from `fubbik-core`, `ApiError` owning the HTTP concern, user scoping in SQL, utoipa annotations. This slice adds no new patterns; it applies proven ones.

**Spec:** `docs/superpowers/specs/2026-08-07-rust-phase-2b-design.md`

## Global Constraints

Everything from Phases 1 and 2a still binds. The ones that have actually cost time:

- **axum 0.8 path syntax is `/{id}`, not `/:id`.** The colon form panics at router build.
- **`#[sqlx::test]` in `fubbik-api` needs `migrations = "../fubbik-db/migrations"`.** Without it the test database is empty and everything 500s with a misleading routing-like error.
- **Stale `_sqlx_test_*` databases cause phantom FK failures.** Drop them before debugging anything:
  `docker exec fubbik-rs-db psql -U postgres -d postgres -Atc "SELECT 'DROP DATABASE IF EXISTS \"'||datname||'\";' FROM pg_database WHERE datname LIKE '\_sqlx\_test%'" | docker exec -i fubbik-rs-db psql -U postgres -d postgres`
- **Never edit an applied migration.** Checksums are recorded, and no test catches the break because every test path provisions a fresh database.
- **`cargo sqlx prepare --workspace` alone DROPS test-target-only cache entries.** Use `cargo sqlx prepare --workspace -- --tests` and verify the `.sqlx` diff is additive-only.
- **`user` and `order` are SQL reserved words.** Both must be double-quoted: `"user"`, `"order"`. `user_favorite.order` is new in this slice and is the easy one to miss.
- **Every wire type carries `#[serde(rename_all = "camelCase")]`** — the web app depends on it.
- **Every exposed timestamp uses `UtcTimestamp`.** A raw `NaiveDateTime` serialises without a `Z`, which JavaScript parses as *local* time — silently wrong by hours, no error.
- **User scoping goes in the SQL**, never left to the caller.
- **Every list query needs a total ordering** (an `id` tiebreaker, or `"order", id` where an explicit order column exists). 2a's live run proved ordering over tied timestamps can skip or duplicate rows under `LIMIT`/`OFFSET`.
- **Nullable fields Node lets a client clear with explicit `null`** use the `deserialize_some` tri-state helper. Fields Node does not allow `null` for stay plain `Option<T>`.
- **Constrained value sets are enum-typed DTO fields**, rejected at deserialisation rather than reaching the database.
- `cargo clippy --workspace --all-targets -- -D warnings` zero; `cargo fmt --check` clean. CI enforces both.
- Test DB: `postgres://postgres:password@localhost:5434/fubbik_rs` (container `fubbik-rs-db`, ICU collation).
- **Never write to the Node database** at `postgresql://pontus@localhost:5432/fubbik`. Reading is fine.
- No `Co-Authored-By` or "Generated with Claude" trailers. Commit with explicit pathspec: `git commit -m "msg" -- <paths>`.

## Schema facts (verified against the live database)

```
workspace        (id, name, description, user_id, created_at, updated_at)
workspace_space  (workspace_id, space_id)          -- composite, NO id, NO user_id
user_favorite    (id, user_id, chunk_id, "order" integer NOT NULL, created_at)
notification     (id, user_id, type, title, message, link_to, read boolean, created_at)
activity_log     (id, user_id, entity_type, entity_id, entity_title, action, space_id, created_at)
collection       (id, name, description, filter jsonb, user_id, space_id, created_at, updated_at)
user_settings     (id, user_id,  key, value jsonb, updated_at)
codebase_settings (id, space_id, key, value jsonb, updated_at)   -- name predates the space rename
instance_settings (        key, value jsonb, updated_at)          -- NO user_id: global
```

Two facts shape the work: **`collection` has no join table** — it stores a filter and its contents are computed. And **`instance_settings` has no owner** — those rows are global.

---

### Task 1: Capture Node's response contract

Same first task as 2a, for the same reason: there, its very first finding was that list endpoints return bare arrays rather than the envelope the chunks domain uses — which would otherwise have been guessed wrong five times and found only at the end.

**Files:**
- Create: `tests/fixtures/node-contract-2b/` (one file per endpoint, plus `_mutating.md`, `_questions.md`)

**Interfaces:**
- Produces: the specification Tasks 2-7 build against.

- [ ] **Step 1: Reuse the existing capture script**

`scripts/capture-node-contract.sh` exists from 2a. Extend or parameterise it for the new endpoints rather than writing a second script. It already refuses to run without a base URL and fails loudly if the server is unreachable — keep both properties.

- [ ] **Step 2: Capture the GET endpoints live**

The Node server must be running. **Ask your human partner before starting it** — it runs against their live knowledge base. Note `pnpm dev:server` has failed before on a broken `node_modules`; `pnpm install --frozen-lockfile` repairs it without lockfile churn.

GETs only. Never issue POST/PATCH/PUT/DELETE against port 3000.

- [ ] **Step 3: Document the mutating endpoints from source**

Read the route definitions and record path, Elysia `t.Object({...})` body schema, response shape and status code for every mutating endpoint across the six domains. Route sources are `packages/api/src/{notifications,settings,workspaces,favorites,collections,activity}/`.

- [ ] **Step 4: Answer four questions explicitly, with evidence**

These are known unknowns that would otherwise surface late:

1. **Does the collection `filter` JSON match the chunks `ListParams` shape?** Quote the actual stored JSON from real rows (`SELECT filter FROM collection` — reading the Node database is fine). This decides whether `GET /collections/{id}/chunks` reuses existing query-building or needs its own evaluator. Phase 1 implemented only seven chunk list parameters — `type`, `search`, `limit`, `offset`, `sort`, `origin`, `reviewStatus` — so note any filter key outside that set.
2. **What is `/settings/features`?** It sits alongside `/settings/user` and `/settings/instance` but there is no fourth table. Is it a computed view? Read the service.
3. **Bare arrays or envelopes** for each list endpoint? 2a found all of its were bare arrays while chunks uses an envelope; do not assume this slice matches either.
4. **What does `activity` actually expose?** 52 LOC and roughly one endpoint. Confirm the shape and any filtering it supports.

- [ ] **Step 5: Commit**

```bash
git add tests/fixtures/node-contract-2b scripts/
git commit -m "test: capture Node's response contract for the phase 2b domains" -- tests/fixtures/node-contract-2b scripts/
```

---

### Task 2: `notifications`

Straightforward user-scoped CRUD. Good first domain to re-establish rhythm.

**Files:**
- Create: `crates/fubbik-db/src/repo/notification.rs`, `crates/fubbik-api/src/notifications/{mod,dto,service,routes}.rs`
- Test: `crates/fubbik-db/tests/notification.rs`, `crates/fubbik-api/tests/notifications.rs`

**Interfaces:**
- Produces: `fubbik_db::repo::notification::{Notification, list, count_unread, mark_read, mark_all_read, delete}`; five routes.

- [ ] **Step 1: Write the failing repository test**

Cover: list is user-scoped (a second user's notifications are invisible); `count_unread` counts only unread and only the caller's; `mark_read` on another user's notification affects nothing and returns false; `mark_all_read` does not touch another user's rows.

Follow `crates/fubbik-db/tests/tag_type.rs` for shape.

- [ ] **Step 2: Run it and confirm it fails**

Run: `cargo test -p fubbik-db --test notification`
Expected: FAIL — `repo::notification` does not exist.

- [ ] **Step 3: Implement the repository**

`read` is `boolean NOT NULL`; `link_to` and the `type` field are the interesting ones — check the captured contract for whether `type` is a constrained set. If it is, make it an enum-typed DTO field (as `connections::dto::Origin` is), so an invalid value is rejected at deserialisation rather than persisted.

List ordering needs a total order: newest-first plus an `id` tiebreaker.

- [ ] **Step 4: Run to green, then implement the API layer**

Five routes with utoipa annotations, `CurrentUser` on each, axum `/{id}` syntax. Response shape from the captured fixtures.

- [ ] **Step 5: HTTP tests**

Cross-user 404 **and unchanged victim data** for every mutating route. `mark_all_read` deserves particular care — a bulk operation that ignores scoping would silently mark another user's notifications read.

- [ ] **Step 6: Ordering-stability test**

Seed several notifications with an **identical** `created_at`, list twice, assert byte-identical order and that it is the id order. Verify it fails without the tiebreaker by removing the tiebreaker and re-running. A test with distinct timestamps passes either way and proves nothing.

- [ ] **Step 7: Regenerate OpenAPI, verify gates, commit**

`cargo run -- openapi > openapi.json`, `cargo sqlx prepare --workspace -- --tests`, `cargo fmt`, `cargo clippy --workspace --all-targets -- -D warnings`, then commit.

---

### Task 3: `favorites`

Small, but has the slice's one novel wrinkle: an explicit `order` column whose name is a SQL reserved word.

**Files:**
- Create: `crates/fubbik-db/src/repo/favorite.rs`, `crates/fubbik-api/src/favorites/{mod,dto,service,routes}.rs`
- Test: `crates/fubbik-db/tests/favorite.rs`, `crates/fubbik-api/tests/favorites.rs`

**Interfaces:**
- Produces: `fubbik_db::repo::favorite::{Favorite, list, add, remove, reorder}`; four routes.

- [ ] **Step 1: Note the reserved word before writing any SQL**

`user_favorite.order` must be quoted `"order"` in every query, exactly as `"user"` is. An unquoted `order` is a syntax error, so this fails loudly rather than silently — but it will fail on the first query written.

- [ ] **Step 2: Write the failing test**

Cover: adding another user's chunk as a favorite is rejected (scope through the parent chunk, as the join tables do); listing is user-scoped; removing another user's favorite affects nothing.

`user_favorite` has its own `id` and `user_id`, so it is simpler than the composite joins — but the *chunk* it references belongs to someone, so adding still needs the parent check.

- [ ] **Step 3: Run, confirm failure, implement**

Ordering is by `"order"` — but ties are possible, so the total order is `"order" ASC, id ASC`. Include a stability test seeding rows with an identical `"order"` value, verified to fail without the id tiebreaker.

- [ ] **Step 4: API layer, tests, OpenAPI, commit**

Cross-user tests assert 404 and unchanged victim data.

---

### Task 4: `workspaces` and the `workspace_space` join

The fourth user-scoped many-to-many join. The pattern is proven — copy it.

**Files:**
- Create: `crates/fubbik-db/src/repo/workspace.rs`, `crates/fubbik-api/src/workspaces/{mod,dto,service,routes}.rs`
- Test: `crates/fubbik-db/tests/workspace.rs`, `crates/fubbik-api/tests/workspaces.rs`

**Interfaces:**
- Produces: `fubbik_db::repo::workspace::{Workspace, list, find_by_id, create, update, delete, add_space, remove_space, spaces_for_workspace}`; seven routes.

- [ ] **Step 1: Copy the proven join pattern**

Read `crates/fubbik-db/src/repo/tag.rs` (`set_chunk_tags`) and `space.rs`. `workspace_space` is `(workspace_id, space_id)` with no ownership of its own, exactly like `chunk_tag` and `chunk_space`. Three guards, each of which must be proven load-bearing:

1. Workspace ownership on INSERT
2. Space ownership on INSERT
3. Parent ownership on the **DELETE half** of any replace-set operation

The third is the one that matters most and is easiest to omit: without it, a correctly-rejected call still wipes the victim's existing rows before returning.

- [ ] **Step 2: Write both scoping tests plus the delete-half test**

Three tests: cannot add another user's space to my workspace; cannot add my space to another user's workspace; a rejected call does not wipe the victim's existing associations.

They are not redundant — a fix for one direction does not fix the other.

- [ ] **Step 3: Run, confirm all three fail, implement**

Verify each guard by removing it and watching its specific test fail. Report each failure message.

- [ ] **Step 4: API layer, tests, OpenAPI, commit**

Seven routes. Note `DELETE /workspaces/{id}/spaces/{spaceId}` is a nested path — axum 0.8 brace syntax for both parameters.

---

### Task 5: `settings` — three scopes, one pattern

**Files:**
- Create: `crates/fubbik-db/src/repo/settings.rs`, `crates/fubbik-api/src/settings/{mod,dto,service,routes}.rs`
- Test: `crates/fubbik-db/tests/settings.rs`, `crates/fubbik-api/tests/settings.rs`

**Interfaces:**
- Produces: read/write for each of the three scopes; seven routes.

- [ ] **Step 1: One shared key-value pattern, three scopes**

`user_settings` (scoped by `user_id`), `codebase_settings` (scoped by `space_id`), `instance_settings` (global, no owner). All three are `(key, value jsonb, updated_at)` plus their scope column.

Write one internal helper shape rather than three divergent implementations — three near-identical implementations is how inconsistency enters, and the final review of 2a found exactly that class of bug (one DTO missing a tri-state its two siblings had).

- [ ] **Step 2: Instance settings are deliberately ungated**

Node's routes use only `requireSession` — no role or admin check (`packages/api/src/settings/routes.ts:60`). This is a **faithful port**, decided explicitly: any authenticated user can read and write global instance settings.

Add a code comment stating this is deliberate and matches Node, so nobody later adds gating believing it an oversight. Do NOT add an authorization check.

- [ ] **Step 3: `codebase_settings` is scoped by space — check ownership**

The table name predates the codebase→space rename. Its rows are scoped by `space_id`, and spaces belong to users — so writing a setting for another user's space must be rejected. Scope through the parent space, as the join tables do.

- [ ] **Step 4: `/settings/features`**

The contract capture (Task 1, question 2) establishes what this is. Implement what it actually does; if it is a computed view rather than a stored table, do not invent a table for it.

- [ ] **Step 5: Tests, OpenAPI, commit**

Cover: user settings isolated per user; codebase settings rejected for another user's space; instance settings readable and writable by any authenticated user (asserting the deliberate behaviour, so a future change breaks a test rather than passing silently).

---

### Task 6: `activity`

Smallest domain. Shape comes from the contract capture.

**Files:**
- Create: `crates/fubbik-db/src/repo/activity.rs`, `crates/fubbik-api/src/activity/{mod,service,routes}.rs`
- Test: `crates/fubbik-api/tests/activity.rs`

- [ ] **Step 1: Implement from the captured contract**

`activity_log` is `(id, user_id, entity_type, entity_id, entity_title, action, space_id, created_at)`. Task 1 question 4 establishes the endpoint shape and any filtering.

- [ ] **Step 2: Scoping and ordering**

User-scoped in SQL. Newest-first with an `id` tiebreaker, plus a failure-verified stability test.

- [ ] **Step 3: Tests, OpenAPI, commit**

---

### Task 7: `collections` — scope depends on Task 1

**This task's size is not yet known.** Collections store a `filter jsonb` and have no join table, so `GET /collections/{id}/chunks` evaluates that filter. Task 1 question 1 establishes whether the filter maps onto the existing chunk `ListParams`.

**Files:**
- Create: `crates/fubbik-db/src/repo/collection.rs`, `crates/fubbik-api/src/collections/{mod,dto,service,routes}.rs`
- Test: `crates/fubbik-db/tests/collection.rs`, `crates/fubbik-api/tests/collections.rs`

- [ ] **Step 1: Read Task 1's answer before writing code**

If the filter maps onto `ListParams`, reuse the existing query-building from `chunk::list`.

If it is a separate dialect, or references keys outside the seven parameters Phase 1 implemented (`type`, `search`, `limit`, `offset`, `sort`, `origin`, `reviewStatus` — so anything touching tags, spaces or scope), **STOP and report** rather than building a partial evaluator. A filter evaluator that silently ignores keys it does not understand would return wrong results with no error, which is worse than not shipping the endpoint.

- [ ] **Step 2: CRUD first, filter evaluation second**

The four CRUD endpoints are ordinary user-scoped work and do not depend on the filter question. Land those, then handle `GET /collections/{id}/chunks`.

- [ ] **Step 3: Scoping**

`collection` references both `user_id` and `space_id`. Creating a collection in another user's space must be rejected — scope through the parent space.

- [ ] **Step 4: Tests, OpenAPI, commit**

Include a test that a collection's computed chunks contain only the caller's chunks. A filter evaluator that forgets user scoping would leak other users' content through a saved query — the highest-severity failure available in this slice.

---

### Task 8: Extend the differential harness

- [ ] **Step 1: Add the new GET paths**

`/api/notifications/count`, `/api/favorites`, `/api/workspaces`, `/api/settings/user`, `/api/settings/instance`, `/api/collections`, plus whatever `activity` exposes.

- [ ] **Step 2: Check the unordered-comparison list**

The harness compares some endpoints as multisets because Node has no `ORDER BY` on them. Grep Node for each new endpoint: if it has no ordering, add it to the unordered set; if it does order, keep it sequence-compared. Do not make everything unordered — that blinds the harness to real ordering regressions.

- [ ] **Step 3: Confirm it still skips cleanly without both stacks**

Run: `cargo test -p fubbik-api --test differential`
Expected: PASS with the live tests reported as ignored.

- [ ] **Step 4: Run the live comparison**

Needs the Node server — **ask your human partner before starting it**. Use `./scripts/differential.sh`.

Report every mismatch. A mismatch is the expected outcome of a first run: 2a's found a real ordering bug present in both stacks.

- [ ] **Step 5: Fix or record each mismatch**

Fix Rust defects. Record environmental differences and deliberate divergences in the spec — the running list stands at seven from earlier slices.

## Exit Criteria

- [ ] `cargo test --workspace` green; report the count (baseline is 208).
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` zero; `cargo fmt --check` clean.
- [ ] `SQLX_OFFLINE=true cargo check --workspace --all-targets` clean.
- [ ] `openapi.json` regenerated and its staleness guard passing.
- [ ] All ~29 endpoints reachable and user-scoped, with cross-user tests asserting 404 **and** unchanged victim data.
- [ ] Every list query has a total ordering, with a stability test verified to fail without its tiebreaker.
- [ ] All three `workspace_space` guards proven load-bearing by removal.
- [ ] The differential harness covers the new GET endpoints and has been run live at least once, with results recorded.
- [ ] `apps/web` untouched and still at zero type errors.
