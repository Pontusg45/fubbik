# Rust Phase 2a Implementation Plan — Tags, Spaces, Connections, Stats

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Port five domains (19 endpoints) from the TypeScript backend to Rust, establishing the user-scoped many-to-many join-table pattern that every later slice depends on.

**Architecture:** Unchanged from Phase 1 — repository → service → route, `AppResult<T>` from `fubbik-core`, `ApiError` owning the HTTP concern in `fubbik-api`, utoipa annotations, user scoping pushed into SQL. This slice adds no new architecture; it adds a pattern.

**Tech Stack:** Rust, axum 0.8, sqlx, utoipa, thiserror.

**Spec:** `docs/superpowers/specs/2026-08-03-rust-phase-2a-design.md`

## Global Constraints

Everything from Phase 1 still binds. The ones that bit hardest, restated:

- **axum 0.8 path syntax is `/{id}`, not `/:id`.** The colon form panics at router build.
- **`#[sqlx::test]` in `fubbik-api` needs `migrations = "../fubbik-db/migrations"`.** Without it the test database is empty and everything 500s with a misleading routing-like error.
- **Stale `_sqlx_test_*` databases cause phantom FK failures.** Drop them before debugging anything:
  `docker exec fubbik-rs-db psql -U postgres -d postgres -Atc "SELECT 'DROP DATABASE IF EXISTS \"'||datname||'\";' FROM pg_database WHERE datname LIKE '\_sqlx\_test%'" | docker exec -i fubbik-rs-db psql -U postgres -d postgres`
- **Never edit an applied migration.** Checksums are recorded; editing one breaks every existing database, and no test catches it because every test path provisions a fresh database.
- **Every wire type carries `#[serde(rename_all = "camelCase")]`.** 106 web files depend on it.
- **Every timestamp uses the `UtcTimestamp` newtype** so it serialises with a `Z`. A tz-less ISO string is parsed as *local* time by JavaScript.
- **User scoping goes in the SQL**, not in the caller.
- **`cargo clippy --workspace --all-targets -- -D warnings` must stay at zero, `cargo fmt --check` clean.** CI enforces both.
- Test DB: `postgres://postgres:password@localhost:5434/fubbik_rs` (container `fubbik-rs-db`, ICU collation provider).
- Never write to the Node database at `postgresql://pontus@localhost:5432/fubbik`. Reading is fine.
- No `Co-Authored-By` or "Generated with Claude" trailers.

## Schema facts (verified, not assumed)

```
chunk_tag    (chunk_id, tag_id)          -- composite PK, NO id, NO user_id
chunk_space  (chunk_id, space_id)        -- composite PK, NO id, NO user_id
tag          (id, name, tag_type_id NULLABLE, user_id, created_at, origin,
              review_status, reviewed_by, reviewed_at)
tag_type     (id, name, color NOT NULL DEFAULT '#8b5cf6', icon, user_id, created_at)
space        (id, name, kind FK→space_kind, description, user_id, created_at, updated_at)
space_code_metadata (space_id, user_id, remote_url, local_paths jsonb DEFAULT '[]')
chunk_connection (id, source_id, target_id, relation FK→connection_relation,
              created_at, origin, review_status, reviewed_by, reviewed_at, weight)
```

Two facts that matter: **`tag.tag_type_id` is nullable** — tags do not require a type. And **the join tables carry no ownership of their own**; it derives from the parent rows.

---

### Task 1: Capture Node's response contract

This is the process change from Phase 1 and it comes first for a reason. Phase 1 discovered the response envelope, eleven missing fields and the timestamp format *after* building the domain, then paid to retrofit all three. That information was in the Node server the whole time.

**Files:**
- Create: `tests/fixtures/node-contract/` (one JSON file per endpoint)
- Create: `scripts/capture-node-contract.sh`

**Interfaces:**
- Produces: committed fixtures recording Node's exact response for all 19 endpoints, used as the specification for Tasks 2-6 and as expected values in tests.

- [ ] **Step 1: Write the capture script**

It must start nothing and write nothing to the Node database — it only issues GETs and records responses, plus POSTs/PATCHes/DELETEs against a scratch copy.

Given the Node server must be running and that touches the user's live data, the script takes the base URL as an argument and refuses to run without it. Capture for each endpoint: HTTP status, `content-type`, and the pretty-printed JSON body.

```bash
#!/usr/bin/env bash
# Records Node's response contract so the Rust port can be built against it
# rather than discovering divergences afterwards.
# READ-ONLY against the Node database: GETs only. Mutating endpoints are
# documented by reading the route source, not by executing them.
set -euo pipefail
BASE="${1:?usage: capture-node-contract.sh http://localhost:3000}"
OUT="tests/fixtures/node-contract"
mkdir -p "$OUT"

capture() {  # capture <name> <path>
  local name="$1" path="$2"
  local code ct
  code=$(curl -s -o "$OUT/$name.json.tmp" -w '%{http_code}' --max-time 10 "$BASE$path")
  ct=$(curl -s -o /dev/null -w '%{content_type}' --max-time 10 "$BASE$path")
  jq . "$OUT/$name.json.tmp" > "$OUT/$name.json" 2>/dev/null || mv "$OUT/$name.json.tmp" "$OUT/$name.json"
  rm -f "$OUT/$name.json.tmp"
  printf '%-28s %s %s\n' "$name" "$code" "$ct" | tee -a "$OUT/_index.txt"
}

: > "$OUT/_index.txt"
capture spaces-list       /api/spaces
capture tags-list         /api/tags
capture tag-types-list    /api/tag-types
capture stats             /api/stats
```

- [ ] **Step 2: Run it against the Node server**

The Node server must be running. Ask your human partner before starting it — it runs against their live knowledge base, and startup triggers scheduled tasks that write staleness flags. Note that `pnpm dev:server` may fail on a broken `node_modules`; `pnpm install --frozen-lockfile` repairs it without touching the lockfile.

Run: `./scripts/capture-node-contract.sh http://localhost:3000`
Expected: `_index.txt` lists each endpoint with status and content-type; each `.json` holds the pretty-printed body.

- [ ] **Step 3: Document the mutating endpoints from source**

The POST/PATCH/DELETE endpoints are not executed against live data. Read their route definitions and record the request body shape, response shape and status code in `tests/fixtures/node-contract/_mutating.md`:

- `POST /api/spaces`, `PATCH /api/spaces/{id}`, `POST /api/spaces/{id}/reset`, `DELETE /api/spaces/{id}`, and the space detect endpoint
- `POST /api/tags`, `PATCH /api/tags/{id}`, `DELETE /api/tags/{id}`, `POST /api/tags/merge`
- `POST /api/tag-types`, `PATCH /api/tag-types/{id}`, `DELETE /api/tag-types/{id}`
- `POST /api/connections`, `DELETE /api/connections/{id}`

For each, record: the exact path, the Elysia `t.Object({...})` body schema, what the handler returns, and the status code.

- [ ] **Step 4: Record the answers to three specific questions**

These are known unknowns that will otherwise be discovered late:

1. **Does each list endpoint return a bare array or an envelope?** Chunks returns `{chunks, total, limit, offset}`. Do spaces/tags/tag-types? Record verbatim.
2. **What does `DELETE /api/tag-types/{id}` do when tags still reference it** — restrict, cascade, or null out `tag_type_id`? Read the repository code. `tag.tag_type_id` is nullable, so all three are plausible.
3. **What does `POST /api/spaces/{id}/reset` actually reset?** The name is ambiguous. Read the service.

- [ ] **Step 5: Commit**

```bash
git add scripts/capture-node-contract.sh tests/fixtures/node-contract
git commit -m "test: capture Node's response contract for the phase 2a domains" -- scripts/capture-node-contract.sh tests/fixtures/node-contract
```

---

### Task 2: `tag_type` domain

Foundation first: `tag.tag_type_id` is an FK to this table, so tags cannot land before it. It is user-owned CRUD, not a seeded catalog — Phase 1 deliberately excluded it from the reference-data migration.

**Files:**
- Create: `crates/fubbik-db/src/repo/tag_type.rs`
- Modify: `crates/fubbik-db/src/repo/mod.rs`
- Create: `crates/fubbik-api/src/tag_types/{mod,dto,service,routes}.rs`
- Modify: `crates/fubbik-api/src/lib.rs`
- Test: `crates/fubbik-db/tests/tag_type.rs`, `crates/fubbik-api/tests/tag_types.rs`

**Interfaces:**
- Consumes: `AppResult` (fubbik-core), `CurrentUser`/`AppState` (fubbik-api), `new_id()` (fubbik-db).
- Produces: `fubbik_db::repo::tag_type::{TagType, list, create, update, delete}`; routes `GET|POST /api/tag-types`, `PATCH|DELETE /api/tag-types/{id}`.

- [ ] **Step 1: Write the failing repository test**

`crates/fubbik-db/tests/tag_type.rs`:

```rust
use fubbik_db::repo::{tag_type, user};

#[sqlx::test]
async fn crud_round_trips_and_is_user_scoped(pool: sqlx::PgPool) {
    let alice = user::create(&pool, "a@b.test", "Alice", None).await.unwrap().id;
    let bob = user::create(&pool, "c@d.test", "Bob", None).await.unwrap().id;

    let t = tag_type::create(&pool, &alice, "Topic", "#ff0000", None).await.unwrap();
    assert_eq!(t.name, "Topic");
    assert_eq!(t.color, "#ff0000");

    // Bob cannot see or touch Alice's tag type.
    assert!(tag_type::list(&pool, &bob).await.unwrap().is_empty());
    assert!(tag_type::update(&pool, &bob, &t.id, Some("Hijacked"), None, None).await.unwrap().is_none());
    assert!(!tag_type::delete(&pool, &bob, &t.id).await.unwrap());

    // Alice's row is untouched by Bob's attempts.
    let still = tag_type::list(&pool, &alice).await.unwrap();
    assert_eq!(still.len(), 1);
    assert_eq!(still[0].name, "Topic");

    assert!(tag_type::delete(&pool, &alice, &t.id).await.unwrap());
    assert!(tag_type::list(&pool, &alice).await.unwrap().is_empty());
}
```

- [ ] **Step 2: Run it and confirm it fails**

Run: `cargo test -p fubbik-db --test tag_type`
Expected: FAIL — `repo::tag_type` does not exist.

- [ ] **Step 3: Implement the repository**

Follow `crates/fubbik-db/src/repo/chunk.rs` for structure. Requirements specific to this table:

- `TagType` derives `Debug, Clone, serde::Serialize, sqlx::FromRow, utoipa::ToSchema` with `#[serde(rename_all = "camelCase")]`.
- `created_at` uses the `UtcTimestamp` newtype (see `crates/fubbik-db/src/timestamp.rs`).
- `color` is NOT NULL with a database default of `'#8b5cf6'` — `create` should let the caller supply one and fall back to the default rather than hardcoding it in Rust.
- Every query filters `user_id = $N` in SQL.
- `update` uses `COALESCE($n, column)` so unset fields are not cleared, matching `chunk::update`.
- `create` uses `fetch_one` (INSERT…RETURNING always yields a row); lookups and `update` use `fetch_optional`.

- [ ] **Step 4: Run the test to green**

Run: `cargo test -p fubbik-db --test tag_type`
Expected: PASS

- [ ] **Step 5: Implement the API layer**

Follow `crates/fubbik-api/src/chunks/` for structure. Four routes with utoipa annotations, `CurrentUser` on every one, axum 0.8 `/{id}` syntax. The response shape must match what Task 1 captured — check `_index.txt` for whether the list endpoint returns a bare array or an envelope, and mirror it.

Deletion semantics come from Task 1 Step 4 question 2. Implement what Node does; if Node restricts while tags reference the type, return `AppError::Conflict`.

- [ ] **Step 6: Write and run the HTTP tests**

`crates/fubbik-api/tests/tag_types.rs` — every test carries `#[sqlx::test(migrations = "../fubbik-db/migrations")]`. Cover: create → list round-trip, cross-user 404 on PATCH and DELETE **with an assertion that the victim's row is unchanged**, unauthenticated 401, and the deletion-with-referencing-tags case.

Run: `cargo test -p fubbik-api --test tag_types`
Expected: PASS

- [ ] **Step 7: Regenerate the OpenAPI document and commit**

```bash
cargo run -- openapi > openapi.json
cargo sqlx prepare --workspace
cargo fmt && cargo clippy --workspace --all-targets -- -D warnings
git add crates .sqlx openapi.json
git commit -m "feat(api): tag-types domain" -- crates .sqlx openapi.json
```

---

### Task 3: `tags` domain and the `chunk_tag` join

**This is the task the slice exists for.** `chunk_tag` is the first user-scoped many-to-many join table in the codebase. Whatever pattern lands here gets copied by `chunk_feature_delta`, `plan_task_chunk`, `requirement_chunk` and `behavior_cell_code` in later slices.

**Files:**
- Create: `crates/fubbik-db/src/repo/tag.rs`
- Create: `crates/fubbik-api/src/tags/{mod,dto,service,routes}.rs`
- Test: `crates/fubbik-db/tests/tag.rs`, `crates/fubbik-api/tests/tags.rs`

**Interfaces:**
- Consumes: `tag_type` (Task 2), `chunk` repo (Phase 1).
- Produces: `fubbik_db::repo::tag::{Tag, list, create, update, delete, merge, set_chunk_tags, tags_for_chunk}`; routes `GET|POST /api/tags`, `PATCH|DELETE /api/tags/{id}`, `POST /api/tags/merge`.

- [ ] **Step 1: Write the failing join-scoping test — both directions**

The two holes are distinct and a fix for one does not fix the other. Tagging *another user's chunk*, and attaching *another user's tag* to your own chunk, both have to be impossible.

`crates/fubbik-db/tests/tag.rs`:

```rust
use fubbik_db::repo::{chunk, tag, user};

async fn seed(pool: &sqlx::PgPool, email: &str) -> String {
    user::create(pool, email, "U", None).await.unwrap().id
}

async fn a_chunk(pool: &sqlx::PgPool, uid: &str, title: &str) -> String {
    chunk::create(pool, uid, chunk::NewChunk {
        title: title.into(), content: String::new(),
        chunk_type: "note".into(), rationale: None,
    }).await.unwrap().id
}

#[sqlx::test]
async fn cannot_tag_another_users_chunk(pool: sqlx::PgPool) {
    let alice = seed(&pool, "a@b.test").await;
    let bob = seed(&pool, "c@d.test").await;
    let alices_chunk = a_chunk(&pool, &alice, "Alice's").await;
    let bobs_tag = tag::create(&pool, &bob, "bobs-tag", None).await.unwrap();

    // Bob attaches his own tag to Alice's chunk — must be rejected.
    let n = tag::set_chunk_tags(&pool, &bob, &alices_chunk, &[bobs_tag.id.clone()])
        .await
        .unwrap();
    assert_eq!(n, 0, "must not tag another user's chunk");
    assert!(tag::tags_for_chunk(&pool, &alice, &alices_chunk).await.unwrap().is_empty());
}

#[sqlx::test]
async fn cannot_attach_another_users_tag(pool: sqlx::PgPool) {
    let alice = seed(&pool, "a@b.test").await;
    let bob = seed(&pool, "c@d.test").await;
    let alices_chunk = a_chunk(&pool, &alice, "Alice's").await;
    let bobs_tag = tag::create(&pool, &bob, "bobs-tag", None).await.unwrap();

    // Alice attaches Bob's tag to her own chunk — must be rejected.
    let n = tag::set_chunk_tags(&pool, &alice, &alices_chunk, &[bobs_tag.id.clone()])
        .await
        .unwrap();
    assert_eq!(n, 0, "must not attach another user's tag");
    assert!(tag::tags_for_chunk(&pool, &alice, &alices_chunk).await.unwrap().is_empty());
}
```

- [ ] **Step 2: Run and confirm both fail**

Run: `cargo test -p fubbik-db --test tag`
Expected: FAIL — `repo::tag` does not exist.

- [ ] **Step 3: Implement the repository, scoping the join through BOTH parents**

`chunk_tag` has no `user_id` of its own. The insert must therefore verify ownership of the chunk *and* the tag in SQL:

```sql
INSERT INTO chunk_tag (chunk_id, tag_id)
SELECT c.id, t.id
FROM chunk c
JOIN tag t ON t.id = ANY($3)
WHERE c.id = $2
  AND c.user_id = $1
  AND t.user_id = $1
ON CONFLICT (chunk_id, tag_id) DO NOTHING
```

Both `c.user_id = $1` and `t.user_id = $1` are load-bearing. Dropping either opens one of the two holes the tests cover. Return the number of rows affected so callers (and the tests) can detect a rejected attach.

`tags_for_chunk` scopes the same way — through the parent chunk's owner.

- [ ] **Step 4: Run to green**

Run: `cargo test -p fubbik-db --test tag`
Expected: PASS — both directions rejected.

- [ ] **Step 5: Implement `merge`, matching Node exactly**

Node's implementation is at `packages/db/src/repository/tag-new.ts:76`. Its semantics, which must be preserved:

1. In a transaction, verify **both** tags belong to the calling user — Node selects both ids `WHERE id IN (source, target) AND user_id = ?` and fails unless exactly 2 rows come back.
2. Re-point `chunk_tag` rows from source to target with `ON CONFLICT (chunk_id, tag_id) DO NOTHING`, so a chunk already carrying the target tag does not error.
3. Delete the remaining `chunk_tag` rows for the source tag.
4. Delete the source tag.

The whole thing is one transaction. A partial merge would leave chunks pointing at a deleted tag.

Add a test proving a chunk that already has *both* tags ends with exactly one `chunk_tag` row afterwards — that is the case `ON CONFLICT` exists for, and a naive `UPDATE` would violate the primary key instead.

- [ ] **Step 6: Implement the API layer and its tests**

Five routes. `POST /api/tags/merge` takes `{sourceId, targetId}` per Node's body schema. Cross-user tests at the HTTP boundary for every mutating route, asserting 404 **and** unchanged victim data.

- [ ] **Step 7: Regenerate OpenAPI, verify gates, commit**

Run `cargo run -- openapi > openapi.json`, `cargo sqlx prepare --workspace`, `cargo fmt`, `cargo clippy --workspace --all-targets -- -D warnings`, then commit.

---

### Task 4: `spaces` domain and the `chunk_space` join

**Files:**
- Create: `crates/fubbik-db/src/repo/space.rs`
- Create: `crates/fubbik-api/src/spaces/{mod,dto,service,routes}.rs`
- Test: `crates/fubbik-db/tests/space.rs`, `crates/fubbik-api/tests/spaces.rs`

**Interfaces:**
- Produces: `fubbik_db::repo::space::{Space, list, find_by_id, create, update, delete, detect, reset, set_chunk_spaces}`; routes `GET|POST /api/spaces`, `GET|PATCH|DELETE /api/spaces/{id}`, `POST /api/spaces/{id}/reset`, and the detect endpoint.

- [ ] **Step 1: Handle the `space_code_metadata` side table**

`space` has a `kind` FK to the seeded `space_kind` catalog. For `kind = 'code'`, extra fields live in a **side table**: `space_code_metadata (space_id, user_id, remote_url, local_paths jsonb DEFAULT '[]')`.

Read Task 1's captured contract to see whether Node flattens those fields into the space response or nests them. Mirror it exactly — this is precisely the shape of divergence that cost Phase 1 a retrofit.

- [ ] **Step 2: Write the failing tests**

Cover: create/list/detail/update/delete round-trip; cross-user invisibility on all of them; a `code`-kind space round-tripping its `remote_url` and `local_paths`; and `chunk_space` scoped through both parents exactly as `chunk_tag` is in Task 3.

- [ ] **Step 3: Run and confirm failure, then implement**

Same structure as Tasks 2 and 3. The `chunk_space` insert scopes through both the chunk's and the space's `user_id`.

- [ ] **Step 4: Implement `detect` and `reset`**

Both need Node's semantics from Task 1 Step 4. `detect` resolves a space from a git remote URL or a local path (see `GET /api/spaces/detect?remoteUrl=&localPath=`). `reset` is ambiguously named — implement what Node's service actually does, and if the captured contract left it unclear, read `packages/api/src/spaces/` before writing code rather than guessing.

- [ ] **Step 5: API layer, tests, OpenAPI, commit**

As Task 3 Step 6-7.

---

### Task 5: `connections` domain

**Files:**
- Create: `crates/fubbik-db/src/repo/connection.rs`
- Create: `crates/fubbik-api/src/connections/{mod,dto,service,routes}.rs`
- Test: `crates/fubbik-db/tests/connection.rs`, `crates/fubbik-api/tests/connections.rs`

**Interfaces:**
- Produces: `fubbik_db::repo::connection::{Connection, create, delete}`; routes `POST /api/connections`, `DELETE /api/connections/{id}`.

- [ ] **Step 1: Write the failing test**

`chunk_connection` links two chunks. The scoping requirement is that **both endpoints must belong to the caller** — creating an edge from your chunk to someone else's would leak the existence of their chunk and create a cross-tenant reference.

Test: Alice creating a connection from her chunk to Bob's chunk must fail, and no row may be created.

- [ ] **Step 2: Run, confirm failure, implement**

The insert verifies ownership of both `source_id` and `target_id` in SQL:

```sql
INSERT INTO chunk_connection (id, source_id, target_id, relation)
SELECT $1, s.id, t.id, $4
FROM chunk s, chunk t
WHERE s.id = $2 AND t.id = $3 AND s.user_id = $5 AND t.user_id = $5
```

`relation` is an FK to the seeded `connection_relation` catalog (13 rows) and defaults to `'related_to'`. `weight` defaults to 1.

There is a unique index on `(source_id, target_id, relation)` — a duplicate must surface as `AppError::Conflict` (409), not a 500. Detect it with `db_err.is_unique_violation()`, not string matching, as the auth sign-up path does.

- [ ] **Step 3: API layer, tests, OpenAPI, commit**

Include a test for the duplicate-connection 409 and for cross-user rejection at the HTTP boundary.

---

### Task 6: `stats` domain

**Files:**
- Create: `crates/fubbik-api/src/stats/{mod,service,routes}.rs`
- Test: `crates/fubbik-api/tests/stats.rs`

**Interfaces:**
- Produces: route `GET /api/stats`.

- [ ] **Step 1: Match Node's response shape exactly**

Read `packages/api/src/stats/routes.ts` and its service, plus Task 1's captured `stats.json`. It is an aggregate count endpoint; the field names and nesting must match, because four web call sites read them.

Counts must be **user-scoped** — a user must not see totals that include another user's data.

- [ ] **Step 2: Write the failing test, implement, run to green**

Test that counts reflect only the calling user's rows: seed data for two users and assert each sees only their own totals.

- [ ] **Step 3: OpenAPI, gates, commit**

---

### Task 7: Extend the differential harness

**Files:**
- Modify: `crates/fubbik-api/tests/differential.rs`

**Interfaces:**
- Consumes: all five domains from Tasks 2-6.

- [ ] **Step 1: Add the new GET paths**

The harness compares Node and Rust responses for the same request, normalising away IDs and timestamps. Add: `/api/spaces`, `/api/tags`, `/api/tag-types`, `/api/stats`.

- [ ] **Step 2: Confirm the harness still skips cleanly without both stacks**

Run: `cargo test -p fubbik-api --test differential`
Expected: PASS, test reported as ignored — the live comparison only runs with `FUBBIK_NODE_URL` and `FUBBIK_RUST_URL` set.

- [ ] **Step 3: Run the live comparison**

This needs the Node server, which runs against your human partner's live knowledge base — **ask before starting it**. Use `./scripts/differential.sh`, which provisions a dedicated ICU-collated `fubbik_diff` database, seeds it read-only from the Node database, and never writes to Node.

Report every mismatch. A mismatch here is the expected outcome of a first run, not a failure of the task — Phase 1's first run found four real gaps.

- [ ] **Step 4: Fix any mismatches, or record them**

For each: fix it if it is a Rust defect; record it in the spec if it is environmental (as the ICU collation divergence was) or a deliberate divergence.

- [ ] **Step 5: Commit**

## Exit Criteria

- [ ] `cargo test --workspace` green; report the count (baseline is 100).
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` zero; `cargo fmt --check` clean.
- [ ] `SQLX_OFFLINE=true cargo check --workspace` clean.
- [ ] `openapi.json` regenerated, committed, staleness guard passing.
- [ ] All 19 endpoints reachable and user-scoped, with cross-user tests asserting 404 **and** unchanged victim data.
- [ ] The differential harness covers the four new GET endpoints and has been run live at least once, with results recorded.
- [ ] `apps/web` untouched and still at zero type errors.
