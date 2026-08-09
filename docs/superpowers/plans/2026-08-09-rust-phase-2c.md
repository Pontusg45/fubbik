# Rust Phase 2c Implementation Plan — Unblocking the Web App

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Port `plans`, `staleness` and `search` to Rust, wire Apache AGE for both reads and writes, so the web app's critical path is fully served by the Rust backend.

**Architecture:** Repository → service → route, as established in Phases 1/2a/2b. Three new patterns: the first `sqlx` transaction, ownership derived through a two-level parent chain, and a hand-written DSL parser. AGE becomes live via the existing `crates/fubbik-db/src/age.rs`.

**Tech Stack:** Rust, axum 0.8, sqlx 0.8 (compile-time checked queries, `.sqlx` offline cache), thiserror, utoipa, Apache AGE 1.7.0, pgvector, pg_trgm, Postgres 18.

## Global Constraints

Every task's requirements implicitly include this section.

- **Run cargo in the main tree. Do NOT set `CARGO_TARGET_DIR`. Do NOT create a git worktree** unless explicitly told to — disk runs near capacity and each `target/` is the largest consumer.
- **`export DATABASE_URL="postgres://postgres:password@localhost:5434/fubbik_rs"`** before running any test, or every sqlx test panics with `DATABASE_URL must be set: EnvVar(NotPresent)` — this looks like real breakage and is pure harness error.
- **Every `docker` command needs `--context orbstack`.** The default `desktop-linux` context points at a Docker Desktop that is not running, and its error misleadingly reads as an API version mismatch. Never run `docker context use` — it rewrites the user's global config.
- **axum 0.8 path syntax is `/{id}`, NOT `/:id`** — the colon form panics at router build.
- **Every `#[sqlx::test]` in `fubbik-api` needs `migrations = "../fubbik-db/migrations"`**, else the DB is empty and everything 500s with a misleading routing-like error.
- **Never edit an applied migration.** Checksums are recorded; no test catches the break. Every table in this slice already exists in `crates/fubbik-db/migrations/0001_init.sql`.
- **`cargo sqlx prepare --workspace` alone DROPS test-target-only entries.** Use `cargo sqlx prepare --workspace -- --tests`; verify the `.sqlx` diff is additive-only.
- **Stale `_sqlx_test_*` databases cause phantom FK failures.** Drop before investigating any failure:
  `docker --context orbstack exec fubbik-rs-db psql -U postgres -d postgres -Atc "SELECT 'DROP DATABASE IF EXISTS \"'||datname||'\";' FROM pg_database WHERE datname LIKE '\_sqlx\_test%'" | docker --context orbstack exec -i fubbik-rs-db psql -U postgres -d postgres`
- **`user` and `order` are SQL reserved words** — always double-quoted. `plan_requirement."order"`, `plan_task."order"`, `plan_analyze_item."order"`, `plan_external_link."order"`, `plan_task_external_link."order"` all need it.
- **camelCase on every wire type** (`#[serde(rename_all = "camelCase")]`) — 106 web files depend on it.
- **`UtcTimestamp` for every exposed timestamp.** A raw `NaiveDateTime` serialises without a `Z`, which JavaScript parses as local time.
- **User scoping in SQL, never left to the caller.**
- **An `id` tiebreaker on every list query**, so ordering is total.
- **`deserialize_some` tri-state** for any nullable field Node lets a client clear with an explicit `null`.
- **No validation Node lacks**, beyond the divergences this plan names explicitly. Every deliberate divergence is escalated individually; a silent one is worse than the looseness.
- **Run tests in the FOREGROUND.** Do not background a long run and idle.
- Commit with the explicit pathspec form (`git commit -m "msg" -- <paths>`); `git add` new files first; never `git add -A`. No `Co-Authored-By` or "Generated with Claude" trailers.
- Do NOT touch `apps/web` or `packages/`. Do NOT start the Node server or write to `postgresql://pontus@localhost:5432/fubbik` — the user's live knowledge base. Reading its source is expected.

### Testing constraints

- **Test BOTH layers.** Confirmed across five consecutive Phase 2b tasks: an API-level test **cannot** detect a removed SQL ownership guard, because the service pre-check 404s first. Only repo-level tests catch it. Every guard is proven load-bearing by removal, and every removal experiment **names which layer's test failed**.
- **Enumerate guards, then check each has a test.** Phase 2b's whole-branch review found a correct guard with zero tests at either layer — deleting it left 34 tests green.
- **Ordering-stability tests seed FORCED-IDENTICAL sort keys** plus an explicit `ANALYZE`. Distinct keys pass with or without a tiebreaker and prove nothing. The working pattern is 20 rows.
- **Cross-user tests assert 404 AND unchanged victim data.** A status-only assertion passes even when a rejected write already mutated something.

### The four free-text fields — do NOT model these as enums

Unconstrained at both the Elysia schema level and the DB level. The only enforcement is an `Array.includes` check in a service function raising `ValidationError`. There is no Postgres enum and no CHECK constraint. **Replicate as an application-layer check.** Phase 2b hit this trap twice.

| Field | Allowed values | Enforced in |
| --- | --- | --- |
| `plan.status` | `draft`, `analyzing`, `ready`, `in_progress`, `completed`, `archived` | `packages/api/src/plans/service.ts:7` |
| `plan_task.status` | `pending`, `in_progress`, `done`, `skipped`, `blocked` | `packages/api/src/plans/tasks.ts:22` |
| `plan_task_chunk.relation` | `context`, `created`, `modified` | `packages/api/src/plans/service.ts:9` |
| `plan_analyze_item.kind` | `chunk`, `file`, `risk`, `assumption`, `question` | `packages/api/src/plans/service.ts:8` |

### Deliberate divergences introduced by this slice

- **#12** — mark-done + unblock become atomic (Task 6).
- **#13** — every plans route gets ownership enforcement Node lacks (Tasks 3–6).
- **#14** — `dismissStaleFlag` gets an ownership guard (Task 7).
- **#11 resolved** — chunk list limit clamp aligns to Node's 100 (Task 11).

---

## File Structure

**Create:**
- `crates/fubbik-db/src/repo/plan.rs` — plan + 7 child tables. Largest repo file in the slice; if it exceeds ~900 lines, split as `plan/mod.rs`, `plan/task.rs`, `plan/analyze.rs`.
- `crates/fubbik-db/src/repo/staleness.rs`
- `crates/fubbik-db/src/repo/saved_query.rs`
- `crates/fubbik-api/src/plans/{mod,dto,service,routes}.rs`
- `crates/fubbik-api/src/staleness/{mod,dto,service,routes}.rs`
- `crates/fubbik-api/src/search/{mod,dto,parser,service,routes}.rs` — `parser.rs` is standalone and dependency-free by design, so it can be unit-tested without a database.
- Tests: `crates/fubbik-db/tests/{plan,staleness,saved_query,age_sync}.rs`, `crates/fubbik-api/tests/{plans,staleness,search,search_parser}.rs`

**Modify:**
- `crates/fubbik-db/src/repo/connection.rs` — add AGE projection (Task 10)
- `crates/fubbik-api/src/chunks/dto.rs:121` — limit clamp (Task 11)
- `crates/fubbik-api/src/{lib,openapi}.rs`, `crates/fubbik-db/src/repo/mod.rs` — wiring
- `crates/fubbik-api/tests/differential.rs` — harness paths (Task 11)

---

## Task 1: Capture Node's contract for all 41 endpoints

**Files:**
- Create: `tests/fixtures/node-contract-2c/*.json`, `tests/fixtures/node-contract-2c/_mutating.md`, `tests/fixtures/node-contract-2c/_questions.md`

This is the practice that paid for itself in 2a and 2b. In Phase 1 the response envelope, eleven missing fields, and the timestamp format were all discovered by the harness *after* the domains were built, then retrofitted across several commits. The information was available the whole time.

**This task needs the Node server. It writes staleness flags to the user's live knowledge base on startup — ASK THE HUMAN PARTNER BEFORE STARTING IT.** Issue GETs only; never write to that database yourself.

- [ ] **Step 1: Ask permission, then start Node**

Ask the human partner. On approval: `pnpm dev:server` (server-only — do not start the web app). Wait for `curl -sf http://localhost:3000/api/health`.

- [ ] **Step 2: Capture every GET response verbatim**

Capture raw bytes, not a pretty-printed re-serialisation, for: `GET /api/plans`, `GET /api/plans/{id}`, `GET /api/plans/{id}/activity`, `GET /api/plans/{id}/links`, `GET /api/plans/{id}/analyze`, `GET /api/plans/{id}/tasks/{taskId}/links`, `GET /api/chunks/stale`, `GET /api/chunks/stale/count`, `GET /api/search/parse?q=...`, `GET /api/search/autocomplete?field=tag&prefix=a`, `GET /api/search/saved`.

Write each to `tests/fixtures/node-contract-2c/<name>.json`.

- [ ] **Step 3: Record mutating-endpoint behaviour in `_mutating.md`**

For all 30 non-GET endpoints: exact path, verbatim request schema, status code, response shape. **Verify the claim that every plans endpoint returns 200 and none returns 201** — check at least three POSTs directly rather than trusting the schema.

- [ ] **Step 4: Answer these four questions in `_questions.md`, with evidence**

1. `GET /api/plans/{id}/analyze` returns an object keyed by kind (`{chunk:[],file:[],risk:[],assumption:[],question:[]}`), not an array. Confirm against real bytes — it is the one outlier among list-shaped GETs in this domain.
2. `POST /api/search/query` with a `near:` clause — does it return results, and does `graphMeta.type` appear? Include a `similar-to:` query and confirm whether `graphMeta.type` is the string `"semantic"` (the TS interface declares only three literals; `service.ts:123` assigns a fourth via `as any`).
3. `GET /api/search/parse?q=` with each of these exact strings, recording the full clause array: `connections:3`, `connections:3+`, `updated:30`, `updated:30d`, `Tag:api`, `affected-by:x hops:3`, `near:"Auth Flow" hops:2`, `path:"A"->"B"`, `NOT tag:deprecated`, `tag:a,b`, `"quoted phrase"`, `bare words here`.
4. Does `GET /api/chunks/stale` return `reason` values beyond `age`, `requirement_uncovered`, `requirement_failing`, `upstream_impact`? Specifically, is `diverged_duplicate` ever written — grep the whole repo including `apps/` and `scripts/` for its writer. Confirm `file_changed` has zero writers.

- [ ] **Step 5: Stop Node and verify the port is clear**

`lsof -ti:3000` must return nothing. Leave no stray `turbo`/`bun` processes.

- [ ] **Step 6: Commit**

```bash
git add tests/fixtures/node-contract-2c/
git commit -m "test: capture Node's response contract for the phase 2c domains" -- tests/fixtures/node-contract-2c/
```

**STOP and report** if any answer contradicts this plan. The plan is wrong in that case, not the capture.

---

## Task 2: The search DSL parser (no database)

**Files:**
- Create: `crates/fubbik-api/src/search/parser.rs`, `crates/fubbik-api/tests/search_parser.rs`
- Reference: `packages/api/src/search/parser.ts`

**Interfaces:**
- Produces: `pub struct QueryClause { pub field: String, pub operator: String, pub value: String, pub params: Option<BTreeMap<String,String>>, pub negate: Option<bool> }` and `pub fn parse_query_string(q: &str) -> Vec<QueryClause>` and `pub fn clauses_to_query_string(clauses: &[QueryClause]) -> String`.

This task is first among implementation tasks because it needs no database, no AGE, and no other domain — it is pure logic, and its correctness is entirely testable in isolation.

**The parser NEVER fails.** There is no error type. Unknown fields become `{field, operator:"is", value}` and are ignored downstream. Do not add a `Result`.

- [ ] **Step 1: Write the failing table-driven test**

```rust
use fubbik_api::search::parser::{parse_query_string, QueryClause};

fn c(field: &str, operator: &str, value: &str) -> QueryClause {
    QueryClause { field: field.into(), operator: operator.into(), value: value.into(), params: None, negate: None }
}

#[test]
fn parses_every_catalogued_query_form() {
    // (input, expected clauses)
    let cases: Vec<(&str, Vec<QueryClause>)> = vec![
        ("type:reference", vec![c("type", "is", "reference")]),
        ("tag:api", vec![c("tag", "is", "api")]),
        // comma => any_of, value stays UNSPLIT at parse time
        ("tag:a,b", vec![c("tag", "any_of", "a,b")]),
        // trailing + => gte, + stripped
        ("connections:3+", vec![c("connections", "gte", "3")]),
        // NO trailing + => plain `is`, NOT gte. Looks like a bug; it is Node's behaviour.
        ("connections:3", vec![c("connections", "is", "3")]),
        // Nd => within, d stripped
        ("updated:30d", vec![c("updated", "within", "30")]),
        // NO trailing d => plain `is`, NOT within.
        ("updated:30", vec![c("updated", "is", "30")]),
        // case-sensitive: `Tag` is NOT `tag`
        ("Tag:api", vec![c("Tag", "is", "api")]),
        // bare words each become their own text/contains clause
        ("bare words", vec![c("text", "contains", "bare"), c("text", "contains", "words")]),
        // quoted phrase is ONE clause, quotes stripped
        ("\"quoted phrase\"", vec![c("text", "contains", "quoted phrase")]),
    ];
    for (input, expected) in cases {
        assert_eq!(parse_query_string(input), expected, "input: {input}");
    }
}

#[test]
fn not_negates_only_the_next_clause() {
    let got = parse_query_string("NOT tag:deprecated type:note");
    assert_eq!(got[0].negate, Some(true), "NOT must negate the clause that follows it");
    assert_eq!(got[1].negate, None, "NOT must not leak onto the clause after that");
}

#[test]
fn hops_attaches_to_the_most_recent_near_clause() {
    let got = parse_query_string("near:abc hops:2");
    assert_eq!(got.len(), 1, "hops is not a standalone clause");
    assert_eq!(got[0].field, "near");
    assert_eq!(got[0].params.as_ref().unwrap().get("hops").map(String::as_str), Some("2"));
}

#[test]
fn hops_after_a_non_near_clause_is_silently_dropped() {
    // Looks like a bug. It is Node's behaviour: hops scans backwards for `near` only.
    let got = parse_query_string("affected-by:req-1 hops:3");
    assert_eq!(got.len(), 1);
    assert_eq!(got[0].field, "affected-by");
    assert!(got[0].params.is_none(), "hops must NOT attach to affected-by");
}

#[test]
fn hops_with_no_preceding_near_is_a_no_op() {
    assert_eq!(parse_query_string("hops:2"), vec![], "orphan hops produces no clause at all");
}

#[test]
fn path_splits_on_arrow_into_from_to_params() {
    let got = parse_query_string("path:\"A\"->\"B\"");
    assert_eq!(got.len(), 1);
    assert_eq!(got[0].field, "path");
    assert_eq!(got[0].operator, "is");
    assert_eq!(got[0].value, "from");
    let p = got[0].params.as_ref().unwrap();
    assert_eq!(p.get("from").map(String::as_str), Some("A"));
    assert_eq!(p.get("to").map(String::as_str), Some("B"));
}

#[test]
fn serialiser_round_trips_and_quotes_values_containing_spaces() {
    let clauses = parse_query_string("near:\"Auth Flow\" hops:2");
    let s = clauses_to_query_string(&clauses);
    assert!(s.contains("near:\"Auth Flow\""), "values with spaces must be re-quoted, got: {s}");
    assert_eq!(parse_query_string(&s), clauses, "round trip must be lossless");
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p fubbik-api --test search_parser`
Expected: FAIL — `parser` module does not exist.

- [ ] **Step 3: Implement the parser**

Port `packages/api/src/search/parser.ts` exactly. The tokenizer splits on whitespace; a `"…"` run is one token with quotes stripped, with **no escape handling** — a `\"` is not special, the loop reads to the next `"`. `field:value` splits on the **first** `:` and requires the colon index to be `> 0`. Re-strip leading/trailing `"` from the value.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p fubbik-api --test search_parser`
Expected: PASS, all 7 tests.

- [ ] **Step 5: Prove the edge-case tests are non-vacuous**

Change the `connections` branch to always return `gte`. Confirm `parses_every_catalogued_query_form` FAILS on the `connections:3` case. Restore. Report the failure message. These cases look like bugs, so a future reader will be tempted to "fix" them — the tests must be the thing that stops them.

- [ ] **Step 6: Commit**

```bash
git add crates/fubbik-api/src/search/parser.rs crates/fubbik-api/tests/search_parser.rs
git commit -m "feat(search): port the query DSL parser with table-driven tests" -- crates/fubbik-api/src/search/parser.rs crates/fubbik-api/tests/search_parser.rs
```

---

## Task 3: Plans — repo layer for `plan` + ownership guard

**Files:**
- Create: `crates/fubbik-db/src/repo/plan.rs`, `crates/fubbik-db/tests/plan.rs`
- Modify: `crates/fubbik-db/src/repo/mod.rs`

**Interfaces:**
- Produces: `plan::create`, `plan::find_by_id(&pool, user_id, id) -> AppResult<Option<Plan>>`, `plan::list`, `plan::update`, `plan::delete`, `plan::duplicate`.

**Every function takes `user_id` and filters on it in SQL.** This is divergence #13 — Node's `getPlan(id)` (`packages/db/src/repository/plan.ts:145-150`) selects by id alone, so any authenticated user can read and write any plan. The Rust port refuses.

`plan` schema (`packages/db/src/schema/plan.ts:13-42`): `id` text PK, `title` text NOT NULL, `description` text NULL, `status` text NOT NULL DEFAULT `'draft'`, `user_id` text NOT NULL → user ON DELETE CASCADE, `space_id` text NULL → space ON DELETE SET NULL, `created_at`/`updated_at` timestamp NOT NULL DEFAULT now(), `completed_at` timestamp NULL, `metadata` jsonb NOT NULL DEFAULT `{}`.

- [ ] **Step 1: Write the failing cross-user test**

```rust
#[sqlx::test]
async fn find_by_id_is_user_scoped(pool: PgPool) {
    let alice = seed_user(&pool, "alice").await;
    let bob = seed_user(&pool, "bob").await;
    let p = plan::create(&pool, &alice, "Alice's plan", None, None).await.unwrap();

    let got = plan::find_by_id(&pool, &bob, &p.id).await.unwrap();
    assert!(got.is_none(), "bob must not read alice's plan by id — Node allows this, the port must not");

    let still = plan::find_by_id(&pool, &alice, &p.id).await.unwrap();
    assert!(still.is_some(), "alice must still read her own plan");
}

#[sqlx::test]
async fn update_is_user_scoped_and_leaves_the_victims_row_intact(pool: PgPool) {
    let alice = seed_user(&pool, "alice").await;
    let bob = seed_user(&pool, "bob").await;
    let p = plan::create(&pool, &alice, "Original", None, None).await.unwrap();

    let res = plan::update(&pool, &bob, &p.id, Some("Hijacked"), None, None).await.unwrap();
    assert!(res.is_none(), "bob's update must not match any row");

    let after = plan::find_by_id(&pool, &alice, &p.id).await.unwrap().unwrap();
    assert_eq!(after.title, "Original", "alice's title must be unchanged after bob's rejected update");
}

#[sqlx::test]
async fn list_breaks_created_at_ties_by_id(pool: PgPool) {
    let alice = seed_user(&pool, "alice").await;
    for i in 0..20 {
        plan::create(&pool, &alice, &format!("plan {i}"), None, None).await.unwrap();
    }
    sqlx::query("UPDATE plan SET created_at = now()").execute(&pool).await.unwrap();
    sqlx::query("ANALYZE plan").execute(&pool).await.unwrap();

    let a: Vec<String> = plan::list(&pool, &alice, Default::default()).await.unwrap()
        .into_iter().map(|p| p.id).collect();
    let mut expected = a.clone();
    expected.sort();
    assert_eq!(a, expected, "ties on created_at must be broken by ascending id, not left to query-plan chance");
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p fubbik-db --test plan`
Expected: FAIL — `plan` module does not exist.

- [ ] **Step 3: Implement the repo**

Every query filters `user_id = $1` in SQL. `list` orders `created_at ASC, id ASC` — Node's `listPlans` uses `.orderBy(asc(plan.createdAt))` with no tiebreaker; the `id` tiebreaker is divergence #6, already accepted. `duplicate` copies plan + requirements + analyze items + tasks + task-chunks + dependencies **inside one transaction**, matching Node's `duplicatePlan` (`plan.ts:177-284`), which is the one place Node already uses `db.transaction`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p fubbik-db --test plan`
Expected: PASS.

- [ ] **Step 5: Prove each guard load-bearing**

For `find_by_id`, `update`, `delete`: remove the `user_id` predicate, confirm the named test FAILS, restore. **Report each failure message and which layer's test failed.** For the tiebreaker: remove `id ASC`, confirm `list_breaks_created_at_ties_by_id` fails, restore.

- [ ] **Step 6: Commit**

```bash
git add crates/fubbik-db/src/repo/plan.rs crates/fubbik-db/tests/plan.rs
git commit -m "feat(db): plan repo with user-scoped access Node lacks" -- crates/fubbik-db/src/repo/plan.rs crates/fubbik-db/tests/plan.rs crates/fubbik-db/src/repo/mod.rs
```

---

## Task 4: Plans — the 10 `routes.ts` endpoints

**Files:**
- Create: `crates/fubbik-api/src/plans/{mod,dto,service,routes}.rs`, `crates/fubbik-api/tests/plans.rs`
- Modify: `crates/fubbik-api/src/{lib,openapi}.rs`

**Interfaces:**
- Consumes: `plan::{create,find_by_id,list,update,delete,duplicate}` from Task 3.
- Produces: `plans::router()` mounted at `/api/plans`.

**Every endpoint returns 200. None returns 201.** Verified across the whole domain.

| Method | Path | Response |
| --- | --- | --- |
| GET | `/api/plans` | bare array |
| GET | `/api/plans/{id}` | **enveloped** `{plan, requirements, analyze, tasks, dependencies}` |
| POST | `/api/plans` | bare object |
| PATCH | `/api/plans/{id}` | bare object |
| DELETE | `/api/plans/{id}` | `{ok: true}` |
| POST | `/api/plans/{id}/duplicate` | bare object |
| GET | `/api/plans/{id}/activity` | bare array, merged plan+task events, sorted `createdAt` desc, `.slice(0,100)` |
| GET | `/api/plans/{id}/links` | bare array |
| POST | `/api/plans/{id}/links` | bare object; `system` defaults to `"url"`, `label` to `null` |
| DELETE | `/api/plans/{id}/links/{linkId}` | `{ok: true}` |

`GET /api/plans/{id}` being enveloped while `GET /api/plans` is a bare array is expected — but response shape is **per-endpoint** in this codebase, so check each against the Task 1 fixture rather than assuming.

- [ ] **Step 1: Write the failing HTTP tests**

```rust
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn get_plan_detail_is_enveloped(pool: PgPool) {
    let app = test_app(pool.clone()).await;
    let id = create_plan(&app, "My plan").await;

    let body: serde_json::Value = get_json(&app, &format!("/api/plans/{id}")).await;
    for key in ["plan", "requirements", "analyze", "tasks", "dependencies"] {
        assert!(body.get(key).is_some(), "detail envelope must carry `{key}`");
    }
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn create_returns_200_not_201(pool: PgPool) {
    let app = test_app(pool.clone()).await;
    let res = post(&app, "/api/plans", json!({"title": "x"})).await;
    assert_eq!(res.status(), 200, "Node returns 200 for every plans POST; it never sets 201");
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn patch_rejects_an_unknown_status(pool: PgPool) {
    let app = test_app(pool.clone()).await;
    let id = create_plan(&app, "x").await;
    let res = patch(&app, &format!("/api/plans/{id}"), json!({"status": "bogus"})).await;
    assert_eq!(res.status(), 400, "status is validated in the service layer, not by a DB enum");
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn detail_on_another_users_plan_is_404_and_leaves_it_intact(pool: PgPool) {
    let app = test_app(pool.clone()).await;
    let (alice, bob) = two_users(&pool).await;
    let id = create_plan_as(&pool, &alice, "Alice's").await;

    let res = get_as(&app, &bob, &format!("/api/plans/{id}")).await;
    assert_eq!(res.status(), 404, "divergence #13: Node returns 200 here");

    let row = plan::find_by_id(&pool, &alice, &id).await.unwrap().unwrap();
    assert_eq!(row.title, "Alice's", "the victim's plan must be untouched");
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p fubbik-api --test plans`
Expected: FAIL — `plans` module does not exist.

- [ ] **Step 3: Implement dto, service and routes**

`status` is validated in the service against `["draft","analyzing","ready","in_progress","completed","archived"]`, raising `AppError::Validation` — **not** a Rust enum on the DTO, because Node accepts any string at the schema level and rejects it in the service. `PATCH` uses `deserialize_some` for `description` and `spaceId`, both of which Node declares as `t.Union([t.String(), t.Null()])`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p fubbik-api --test plans`
Expected: PASS.

- [ ] **Step 5: Regenerate the OpenAPI document**

Run: `cargo test -p fubbik-api committed_openapi_json_is_current`
If it fails, regenerate `openapi.json` and re-run.

- [ ] **Step 6: Commit**

```bash
git add crates/fubbik-api/src/plans crates/fubbik-api/tests/plans.rs
git commit -m "feat(plans): port the core plan routes with ownership enforcement" -- crates/fubbik-api/src/plans crates/fubbik-api/tests/plans.rs crates/fubbik-api/src/lib.rs crates/fubbik-api/src/openapi.rs openapi.json
```

---

## Task 5: Plans — requirements links (3) and analyze items (5)

**Files:**
- Modify: `crates/fubbik-db/src/repo/plan.rs`, `crates/fubbik-api/src/plans/{service,routes}.rs`, `crates/fubbik-db/tests/plan.rs`, `crates/fubbik-api/tests/plans.rs`

**Interfaces:**
- Produces: `plan::{list_requirements,add_requirement,remove_requirement,reorder_requirements,list_analyze_items,create_analyze_item,update_analyze_item,delete_analyze_item,reorder_analyze_items}`.

`GET /api/plans/{id}/analyze` returns an **object keyed by kind** — `{chunk:[], file:[], risk:[], assumption:[], question:[]}` — not an array. It is the one outlier among list-shaped GETs here.

`plan_analyze_item` is discriminated by `kind` (plain `text`, no CHECK, no enum). Kind-specific columns are all nullable: `chunk_id` (for `kind=chunk`), `file_path` (`kind=file`), `text` (used by `risk`/`assumption`/`question`, and settable regardless of kind), `metadata` jsonb NOT NULL DEFAULT `{}`.

**Reorder leaves unmentioned rows untouched.** Node loops per item — `UPDATE ... SET "order" = i WHERE id = ? AND plan_id = ?` — inside a transaction. Rows absent from the request keep whatever `order` they had. They are not renumbered and not deleted. `reorder_analyze_items` additionally scopes by `kind`.

- [ ] **Step 1: Write the failing tests**

```rust
#[sqlx::test]
async fn reorder_leaves_unmentioned_rows_untouched(pool: PgPool) {
    let alice = seed_user(&pool, "alice").await;
    let p = plan::create(&pool, &alice, "p", None, None).await.unwrap();
    let a = plan::create_analyze_item(&pool, &alice, &p.id, "risk", None, None, Some("a"), None).await.unwrap();
    let b = plan::create_analyze_item(&pool, &alice, &p.id, "risk", None, None, Some("b"), None).await.unwrap();
    let c = plan::create_analyze_item(&pool, &alice, &p.id, "risk", None, None, Some("c"), None).await.unwrap();
    let c_order_before = c.order;

    // mention only a and b, swapped
    plan::reorder_analyze_items(&pool, &alice, &p.id, "risk", &[b.id.clone(), a.id.clone()]).await.unwrap();

    let items = plan::list_analyze_items(&pool, &alice, &p.id).await.unwrap();
    let c_after = items.iter().find(|i| i.id == c.id).unwrap();
    assert_eq!(c_after.order, c_order_before,
        "a row absent from the reorder request must keep its original order, not be renumbered");
}

#[sqlx::test]
async fn analyze_kind_accepts_only_the_five_known_kinds(pool: PgPool) {
    // Validation lives in the SERVICE layer, matching Node. The DB column is free text.
    let alice = seed_user(&pool, "alice").await;
    let p = plan::create(&pool, &alice, "p", None, None).await.unwrap();
    let raw = sqlx::query("INSERT INTO plan_analyze_item (id, plan_id, kind) VALUES ($1,$2,'nonsense')")
        .bind("x").bind(&p.id).execute(&pool).await;
    assert!(raw.is_ok(), "the DB must NOT constrain kind — Node has no CHECK and no enum here");
}

#[sqlx::test]
async fn cannot_add_a_requirement_to_another_users_plan(pool: PgPool) {
    let alice = seed_user(&pool, "alice").await;
    let bob = seed_user(&pool, "bob").await;
    let p = plan::create(&pool, &alice, "p", None, None).await.unwrap();
    let r = seed_requirement(&pool, &alice).await;

    let res = plan::add_requirement(&pool, &bob, &p.id, &r).await.unwrap();
    assert!(res.is_none(), "ownership derives from plan.user_id, guarded in SQL");

    let links = plan::list_requirements(&pool, &alice, &p.id).await.unwrap();
    assert!(links.is_empty(), "the victim's plan must have gained no requirement link");
}
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p fubbik-db --test plan`
Expected: FAIL — functions do not exist.

- [ ] **Step 3: Implement**

Scope every child query through the parent: `... WHERE pai.plan_id = $2 AND EXISTS (SELECT 1 FROM plan p WHERE p.id = $2 AND p.user_id = $1)`. Reorder runs per-item inside a transaction. Ordering: `list_analyze_items` orders `kind ASC, "order" ASC, id ASC`; `list_requirements` orders `"order" ASC, id ASC`. Both need `"order"` double-quoted.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p fubbik-db --test plan && cargo test -p fubbik-api --test plans`
Expected: PASS.

- [ ] **Step 5: Prove the guards load-bearing**

Remove the `EXISTS` parent guard from `add_requirement` and from `create_analyze_item`; confirm the named tests FAIL; restore. **Report which layer caught each** — expect the repo-level test to fail while the API-level test still passes, masked by the service pre-check.

- [ ] **Step 6: Commit**

```bash
git commit -m "feat(plans): requirement links and analyze items" -- crates/fubbik-db/src/repo/plan.rs crates/fubbik-api/src/plans crates/fubbik-db/tests/plan.rs crates/fubbik-api/tests/plans.rs openapi.json
```

---

## Task 6: Plans — tasks (11 endpoints) and the first transaction

**Files:**
- Modify: `crates/fubbik-db/src/repo/plan.rs`, `crates/fubbik-api/src/plans/{service,routes}.rs`, tests

**Interfaces:**
- Produces: `plan::{list_tasks,create_task,update_task,delete_task,reorder_tasks,add_task_chunk,remove_task_chunk,add_task_dependency,remove_task_dependency,list_task_links,add_task_link,remove_task_link}` and `plan::mark_task_done_and_unblock(&pool, user_id, plan_id, task_id) -> AppResult<Vec<String>>`.

**This is the transaction task — divergence #12.**

Node (`packages/db/src/repository/plan.ts:536-551` + `plans/tasks.ts:113-122`) issues `updateTask` and then `unblockDependentsOf` as two independent effects. `unblockDependentsOf` itself is a `SELECT` followed by an `UPDATE`, not wrapped. A failure between them leaves a task `done` with dependents still `blocked`.

The semantics to replicate exactly: selecting `plan_task_dependency` rows where `depends_on_task_id = $task`, then updating those dependent tasks `SET status='pending'` **only where the current status is exactly `'blocked'`**. Tasks in `pending`, `in_progress`, `done` or `skipped` are left alone even when they depend on the completed task.

Rust does all of it in **one `sqlx` transaction**. This pattern is new to the codebase and every later multi-step write will copy it — establish it deliberately.

`plan_task_dependency` has **no self-reference guard** in Node — `task_id = depends_on_task_id` is unenforced. Do not add one; that would be an unrequested divergence.

- [ ] **Step 1: Write the failing atomicity test**

```rust
#[sqlx::test]
async fn marking_done_and_unblocking_is_atomic(pool: PgPool) {
    let alice = seed_user(&pool, "alice").await;
    let p = plan::create(&pool, &alice, "p", None, None).await.unwrap();
    let a = plan::create_task(&pool, &alice, &p.id, "a").await.unwrap();
    let b = plan::create_task(&pool, &alice, &p.id, "b").await.unwrap();
    plan::add_task_dependency(&pool, &alice, &p.id, &b.id, &a.id).await.unwrap();
    plan::update_task_status(&pool, &alice, &p.id, &b.id, "blocked").await.unwrap();

    let unblocked = plan::mark_task_done_and_unblock(&pool, &alice, &p.id, &a.id).await.unwrap();
    assert_eq!(unblocked, vec![b.id.clone()]);

    let tasks = plan::list_tasks(&pool, &alice, &p.id).await.unwrap();
    assert_eq!(tasks.iter().find(|t| t.id == a.id).unwrap().status, "done");
    assert_eq!(tasks.iter().find(|t| t.id == b.id).unwrap().status, "pending");
}

#[sqlx::test]
async fn only_blocked_dependents_are_unblocked(pool: PgPool) {
    let alice = seed_user(&pool, "alice").await;
    let p = plan::create(&pool, &alice, "p", None, None).await.unwrap();
    let a = plan::create_task(&pool, &alice, &p.id, "a").await.unwrap();
    let b = plan::create_task(&pool, &alice, &p.id, "b").await.unwrap();
    plan::add_task_dependency(&pool, &alice, &p.id, &b.id, &a.id).await.unwrap();
    plan::update_task_status(&pool, &alice, &p.id, &b.id, "in_progress").await.unwrap();

    plan::mark_task_done_and_unblock(&pool, &alice, &p.id, &a.id).await.unwrap();

    let tasks = plan::list_tasks(&pool, &alice, &p.id).await.unwrap();
    assert_eq!(tasks.iter().find(|t| t.id == b.id).unwrap().status, "in_progress",
        "only dependents in exactly 'blocked' are moved to 'pending'");
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p fubbik-db --test plan`
Expected: FAIL — `mark_task_done_and_unblock` does not exist.

- [ ] **Step 3: Implement with `sqlx::Transaction`**

```rust
pub async fn mark_task_done_and_unblock(
    pool: &PgPool, user_id: &str, plan_id: &str, task_id: &str,
) -> AppResult<Vec<String>> {
    let mut tx = pool.begin().await?;

    let updated = sqlx::query!(
        r#"UPDATE plan_task SET status = 'done', updated_at = now()
           WHERE id = $1 AND plan_id = $2
             AND EXISTS (SELECT 1 FROM plan p WHERE p.id = $2 AND p.user_id = $3)
           RETURNING id"#,
        task_id, plan_id, user_id
    ).fetch_optional(&mut *tx).await?;

    if updated.is_none() {
        tx.rollback().await?;
        return Ok(vec![]);
    }

    let unblocked = sqlx::query_scalar!(
        r#"UPDATE plan_task SET status = 'pending', updated_at = now()
           WHERE plan_id = $2 AND status = 'blocked'
             AND id IN (SELECT task_id FROM plan_task_dependency WHERE depends_on_task_id = $1)
           RETURNING id"#,
        task_id, plan_id
    ).fetch_all(&mut *tx).await?;

    tx.commit().await?;
    Ok(unblocked)
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p fubbik-db --test plan`
Expected: PASS.

- [ ] **Step 5: Prove atomicity by forcing a mid-transaction failure**

Add a temporary `sqlx::query("SELECT 1/0").execute(&mut *tx).await?;` between the two statements. Confirm that after the failure the task is **still not** `done` — i.e. the first statement rolled back. Remove the injected failure and restore. **Report the observed state.**

A test that only asserts both statements ran proves nothing about atomicity; this step is the one that does.

- [ ] **Step 6: Commit**

```bash
git commit -m "feat(plans): tasks, dependencies, and atomic mark-done/unblock" -- crates/fubbik-db/src/repo/plan.rs crates/fubbik-api/src/plans crates/fubbik-db/tests/plan.rs crates/fubbik-api/tests/plans.rs openapi.json
```

---

## Task 7: Staleness — 5 non-AGE endpoints and the startup scan

**Files:**
- Create: `crates/fubbik-db/src/repo/staleness.rs`, `crates/fubbik-api/src/staleness/{mod,dto,service,routes}.rs`, `crates/fubbik-db/tests/staleness.rs`, `crates/fubbik-api/tests/staleness.rs`

**Routes live under `/api/chunks/...`, not `/api/staleness/...`:**

| Method | Path | Response |
| --- | --- | --- |
| GET | `/api/chunks/stale` | bare array |
| GET | `/api/chunks/stale/count` | raw number |
| POST | `/api/chunks/{id}/dismiss-staleness` | update result |
| POST | `/api/chunks/suppress-duplicate` | body `{chunkIdA, chunkIdB}` |
| POST | `/api/chunks/stale/scan-age` | `{flagged: n}` — the sum of BOTH detectors |

**`reason` is unconstrained free text.** Confirmed writers: `age`, `requirement_uncovered`, `requirement_failing`, `upstream_impact`, and `diverged_duplicate`. **`file_changed` has zero writers anywhere** — schema comment, a seed doc string, and dead frontend icon-mapping only. Do not implement it.

**Dismiss vs suppress are different mechanisms.** Dismiss sets `dismissed_at` + `dismissed_by` on **one flag by id**. Suppress sets `suppress_pair` (the sorted `"idA:idB"` key) on **all matching undismissed `diverged_duplicate` rows** for that pair. Reads exclude a row if **either** is set.

**Divergence #14:** Node's `dismissStaleFlag(flagId, userId)` updates by flag id and merely stamps `dismissedBy` — any authenticated user can dismiss any flag. Rust scopes through the flag's parent chunk.

`scan-age` runs **both** `detect_age_stale_chunks` (threshold default **90** days, overridable via `thresholdDays`) and `detect_uncovered_chunks` (threshold default **30** days), returning the summed count. Idempotency comes from a **pre-filter**, not `ON CONFLICT`: exclude chunks already carrying an undismissed flag of that reason. There is no unique constraint for `ON CONFLICT` to target.

`detect_uncovered_chunks` queries `requirement_chunk`. That table exists in the Rust migrations but has no API yet, so it is empty — `NOT IN (empty set)` is true for every row, meaning **all chunks flag as uncovered**. That is Node's behaviour too. Expected, not a defect.

The startup scan: env `STALENESS_SCAN_INTERVAL_HOURS`, default `24`, **disabled when `<= 0`**. Initial run after a 30-second delay, then on the interval. It calls **only** `detect_age_stale_chunks` — never `detect_uncovered_chunks`, which runs solely via the explicit route. No git or filesystem access; it is a pure `updated_at` comparison.

- [ ] **Step 1: Write the failing tests**

```rust
#[sqlx::test]
async fn cannot_dismiss_a_flag_on_another_users_chunk(pool: PgPool) {
    let alice = seed_user(&pool, "alice").await;
    let bob = seed_user(&pool, "bob").await;
    let chunk = seed_chunk(&pool, &alice).await;
    let flag = seed_flag(&pool, &chunk, "age").await;

    let res = staleness::dismiss(&pool, &bob, &flag).await.unwrap();
    assert!(res.is_none(), "divergence #14: Node lets any user dismiss any flag by id");

    let flags = staleness::list(&pool, &alice, Default::default()).await.unwrap();
    assert_eq!(flags.len(), 1, "alice's flag must still be undismissed after bob's attempt");
}

#[sqlx::test]
async fn scan_age_is_idempotent(pool: PgPool) {
    let alice = seed_user(&pool, "alice").await;
    let c = seed_chunk(&pool, &alice).await;
    sqlx::query("UPDATE chunk SET updated_at = now() - interval '200 days' WHERE id = $1")
        .bind(&c).execute(&pool).await.unwrap();

    let first = staleness::detect_age_stale_chunks(&pool, &alice, None, 90).await.unwrap();
    let second = staleness::detect_age_stale_chunks(&pool, &alice, None, 90).await.unwrap();
    assert_eq!(first, 1);
    assert_eq!(second, 0, "re-running must not create a duplicate flag — the pre-filter is the guarantee");
}

#[sqlx::test]
async fn suppress_hides_the_pair_while_dismiss_hides_one_flag(pool: PgPool) {
    let alice = seed_user(&pool, "alice").await;
    let a = seed_chunk(&pool, &alice).await;
    let b = seed_chunk(&pool, &alice).await;
    seed_duplicate_flag(&pool, &a, &b).await;
    seed_duplicate_flag(&pool, &b, &a).await;

    staleness::suppress_duplicate(&pool, &alice, &a, &b).await.unwrap();
    let flags = staleness::list(&pool, &alice, Default::default()).await.unwrap();
    assert!(flags.is_empty(), "suppress must hide BOTH directions of the pair, not just one row");
}
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p fubbik-db --test staleness`
Expected: FAIL — module does not exist.

- [ ] **Step 3: Implement repo, service, routes, and the startup scan**

`list` orders `detected_at DESC, id ASC`, default limit 50. Wire the interval scan beside the existing startup code, honouring the disable condition.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p fubbik-db --test staleness && cargo test -p fubbik-api --test staleness`
Expected: PASS.

- [ ] **Step 5: Prove the guard load-bearing**

Remove the parent-chunk `EXISTS` guard from `dismiss`; confirm `cannot_dismiss_a_flag_on_another_users_chunk` FAILS; restore. Report the message and the layer.

- [ ] **Step 6: Commit**

```bash
git commit -m "feat(staleness): flags, scans, and the interval job" -- crates/fubbik-db/src/repo/staleness.rs crates/fubbik-api/src/staleness crates/fubbik-db/tests/staleness.rs crates/fubbik-api/tests/staleness.rs crates/fubbik-api/src/lib.rs openapi.json
```

---

## Task 8: Search — parse, saved queries, autocomplete, and the non-graph query path

**Files:**
- Create: `crates/fubbik-db/src/repo/saved_query.rs`, `crates/fubbik-api/src/search/{mod,dto,service,routes}.rs`, tests

**Interfaces:**
- Consumes: `parse_query_string(&str) -> Vec<QueryClause>` from Task 2; `chunk::list` (already ported).
- Produces: `search::router()` mounted at `/api/search`; `saved_query::{list,create,delete}`.

This task ships **five of the six** search endpoints; the graph clauses inside
`POST /api/search/query` land in Task 9.

| Method | Path | Response |
| --- | --- | --- |
| GET | `/api/search/parse` | `{clauses: [...]}` — the raw clause array, **not** a normalised string or a validation verdict |
| POST | `/api/search/query` | `SearchResult` — `{chunks, total, graphMeta?, duplicateHints?}` |
| GET | `/api/search/autocomplete` | bare `string[]` |
| GET | `/api/search/saved` | bare array |
| POST | `/api/search/saved` | bare object |
| DELETE | `/api/search/saved/{id}` | `{message: "Deleted"}` |

`GET /api/search/parse` takes `q: String` and returns `{clauses: parse_query_string(q)}`. It has
**no error path** — the parser never fails — so the only non-200 is a 401 from the session guard.

`POST /api/search/query` **always returns 200.** Every internal step in Node is wrapped
`.pipe(Effect.orElse(() => Effect.succeed(...)))`, so even database errors degrade to
`{chunks: [], total: 0}` rather than propagating. Reproduce that: a failing query must not
surface a 500.

`saved_query` (`packages/db/src/schema/saved-query.ts:7-20`) **carries its own `user_id`** — ownership is not derived. `space_id` is nullable → space ON DELETE SET NULL. `query` is opaque JSONB, not re-validated on read. **No unique constraint on `(user_id, name)`** — duplicate names are allowed.

**`DELETE /api/search/saved/{id}` never 404s.** The route ignores the delete result and always returns `{message:"Deleted"}`. Same shape as favorites in 2b. Reproduce it faithfully.

**`SearchQuery.join` (`"and" | "or"`) is dead.** Accepted by both route schemas, never read in `service.ts`; every param is AND-combined by `listChunks`'s SQL. Port it as an accepted, ignored field — implementing OR would be a behaviour change.

`autocomplete` takes `field` ∈ `{tag, chunk, requirement}` and returns `string[]` (names/titles, not ids), max 10. `tag` filters in application code, case-insensitively, on `starts_with`. `chunk` and `requirement` use SQL `ILIKE '%prefix%'` — **contains, not prefix** — with **no `ORDER BY`**. Any other `field` returns `[]`. `requirement` queries a table with no API yet, so it returns empty; that matches Node when the table is empty.

- [ ] **Step 1: Write the failing tests**

```rust
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn deleting_a_nonexistent_saved_query_still_returns_200_deleted(pool: PgPool) {
    let app = test_app(pool.clone()).await;
    let res = delete(&app, "/api/search/saved/does-not-exist").await;
    assert_eq!(res.status(), 200);
    assert_eq!(body_json(res).await, json!({"message": "Deleted"}),
        "Node ignores the delete result and never 404s here");
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn deleting_another_users_saved_query_returns_200_but_deletes_nothing(pool: PgPool) {
    let app = test_app(pool.clone()).await;
    let (alice, bob) = two_users(&pool).await;
    let id = saved_query::create(&pool, &alice, "mine", json!({}), None).await.unwrap().id;

    let res = delete_as(&app, &bob, &format!("/api/search/saved/{id}")).await;
    assert_eq!(res.status(), 200, "the response is indistinguishable — that is Node's behaviour");

    let mine = saved_query::list(&pool, &alice, None).await.unwrap();
    assert_eq!(mine.len(), 1, "but the row must survive: the DELETE is user-scoped in SQL");
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn duplicate_saved_query_names_are_allowed(pool: PgPool) {
    let pool2 = pool.clone();
    let alice = seed_user(&pool, "alice").await;
    saved_query::create(&pool, &alice, "same", json!({}), None).await.unwrap();
    let second = saved_query::create(&pool2, &alice, "same", json!({}), None).await;
    assert!(second.is_ok(), "there is no unique constraint on (user_id, name)");
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn join_or_is_accepted_and_ignored(pool: PgPool) {
    let app = test_app(pool.clone()).await;
    let and_res = post(&app, "/api/search/query", json!({"clauses": [], "join": "and"})).await;
    let or_res  = post(&app, "/api/search/query", json!({"clauses": [], "join": "or"})).await;
    assert_eq!(and_res.status(), 200);
    assert_eq!(or_res.status(), 200);
    assert_eq!(body_json(and_res).await, body_json(or_res).await,
        "`join` is dead in Node — accepted by the schema, never read");
}
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p fubbik-api --test search`
Expected: FAIL — module does not exist.

- [ ] **Step 3: Implement**

`list` orders `created_at DESC, id ASC`. `POST /api/search/query` maps clauses onto `chunk::list` params via a `match` over `clause.field` covering `type|tag|text|connections|updated|origin|review`; unrecognised fields contribute nothing, exactly as Node's `switch` does. `connections:abc+` yields a non-numeric value — Node produces `NaN` and the comparison silently matches nothing; handle it as "no filter applied" and note the choice in the report.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p fubbik-api --test search`
Expected: PASS.

- [ ] **Step 5: Prove the delete scoping load-bearing**

Remove `AND user_id = $2` from the saved-query DELETE; confirm `deleting_another_users_saved_query_returns_200_but_deletes_nothing` FAILS; restore. **Because the endpoint never 404s, the status code cannot reveal this bug — the surviving-row assertion is the only thing that can.** Report the failure message.

- [ ] **Step 6: Commit**

```bash
git commit -m "feat(search): saved queries, autocomplete, and clause-to-list mapping" -- crates/fubbik-db/src/repo/saved_query.rs crates/fubbik-api/src/search crates/fubbik-api/tests/search.rs openapi.json
```

---

## Task 9: Wire AGE — the four graph clauses and `scan-impact`

**Files:**
- Modify: `crates/fubbik-db/src/age.rs` (add query helpers), `crates/fubbik-api/src/search/service.rs`, `crates/fubbik-api/src/staleness/{service,routes}.rs`
- Create: `crates/fubbik-db/tests/age_queries.rs`

**Interfaces:**
- Produces, all in `crates/fubbik-db/src/age.rs`:
  - `pub async fn get_neighborhood(pool: &PgPool, chunk_id: &str, hops: i32) -> AppResult<Vec<String>>`
  - `pub async fn get_neighborhood_in_graph(pool: &PgPool, graph: &str, chunk_id: &str, hops: i32) -> AppResult<Vec<String>>` — the graph name is a parameter purely so the degradation test can point at a nonexistent graph; `get_neighborhood` delegates to it with `"knowledge"`.
  - `pub async fn find_shortest_path_with_details(pool: &PgPool, from: &str, to: &str) -> AppResult<Option<PathDetails>>` where `pub struct PathDetails { pub chunk_ids: Vec<String>, pub edges: Vec<PathEdgeInfo> }`
  - `pub async fn get_chunks_affected_by_requirement(pool: &PgPool, requirement_id: &str, hops: i32) -> AppResult<Vec<String>>`
  - `pub async fn compute_impact_ripple(pool: &PgPool, chunk_id: &str) -> AppResult<Vec<String>>`
- Consumes: the existing `age::cypher`, `age::is_available`, `age::esc_cypher`, `age::parse_agtype`.

`crates/fubbik-db/src/age.rs` has six passing unit tests and **zero callers** — it is dead code. This task makes it live.

**The one non-obvious mechanic, established by Phase 1's spike:** extraction must use `::varchar`, **not** `::text`. An explicit text cast routes through `agtype_value_to_text`, which rejects vertex, edge and path values with `unsupported argument agtype 6`. This is already encoded in `age.rs`. Do not "simplify" it.

`GRAPH_FIELDS` (`packages/api/src/search/service.ts:24`) is `{near, path, affected-by, similar-to}`:
- `near` → `get_neighborhood(value, hops)`; hops defaults to **1** at the service when the parser attached none.
- `path` → `find_shortest_path_with_details(from, to)`; sets `graphMeta.type = "path"` with `pathChunks` and `pathEdges`.
- `affected-by` → `get_chunks_affected_by_requirement`; hops defaults to **2**.
- `similar-to` → embeddings, **not AGE**; sets `graphMeta.type = "semantic"`.

**`graphMeta.type` carries a fourth value the TypeScript interface does not declare.** `types.ts:48-59` lists only `"neighborhood" | "path" | "requirement-reach"`, but `service.ts:123` assigns `"semantic" as any`. Preserve the runtime value; do not "correct" it to match the interface.

**Every graph clause must degrade to empty, not 500.** Node wraps each in `Effect.orElse(() => Effect.succeed([]))`. Ollama's embedding call for `similar-to` degrades the same way.

- [ ] **Step 1: Write the failing tests, including the degradation path**

```rust
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn neighborhood_returns_connected_chunk_ids(pool: PgPool) {
    let alice = seed_user(&pool, "alice").await;
    let a = seed_chunk(&pool, &alice).await;
    let b = seed_chunk(&pool, &alice).await;
    age::ensure_vertex(&pool, &a).await.unwrap();
    age::ensure_vertex(&pool, &b).await.unwrap();
    age::create_edge(&pool, "connects", &a, &b).await.unwrap();

    let ids = age::get_neighborhood(&pool, &a, 1).await.unwrap();
    assert!(ids.contains(&b), "a 1-hop neighbourhood must include the directly connected chunk");
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn a_graph_clause_degrades_to_empty_when_age_is_unavailable(pool: PgPool) {
    // Point the query at a graph name that does not exist, simulating AGE being unavailable.
    let ids = age::get_neighborhood_in_graph(&pool, "no_such_graph", "whatever", 1).await;
    assert_eq!(ids.unwrap_or_default(), Vec::<String>::new(),
        "AGE failures must degrade to empty results, never a 500 — Node wraps each clause in Effect.orElse");
}
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p fubbik-db --test age_queries`
Expected: FAIL — helpers do not exist.

- [ ] **Step 3: Implement the query helpers and wire them into search and staleness**

Every helper follows this shape — note `::varchar`, and note that the error path returns an
empty vector rather than propagating:

```rust
pub async fn get_neighborhood_in_graph(
    pool: &PgPool, graph: &str, chunk_id: &str, hops: i32,
) -> AppResult<Vec<String>> {
    // `::varchar`, NOT `::text`. Verified against AGE 1.7.0: an explicit text cast routes
    // through agtype_value_to_text, which rejects vertex/edge/path with
    // "unsupported argument agtype 6".
    let q = format!(
        "MATCH (a:chunk {{id: '{}'}})-[*1..{}]-(b:chunk) RETURN DISTINCT b.id",
        esc_cypher(chunk_id), hops
    );
    let sql = format!("SELECT v::varchar AS v FROM cypher('{graph}', $ {q} $) AS (v agtype)");
    match sqlx::query_scalar::<_, String>(&sql).fetch_all(pool).await {
        Ok(rows) => Ok(rows.iter().filter_map(|r| parse_agtype(r)).collect()),
        // Node wraps every graph clause in Effect.orElse(() => Effect.succeed([])).
        // An unavailable graph must yield no results, never a 500.
        Err(_) => Ok(vec![]),
    }
}

pub async fn get_neighborhood(pool: &PgPool, chunk_id: &str, hops: i32) -> AppResult<Vec<String>> {
    get_neighborhood_in_graph(pool, "knowledge", chunk_id, hops).await
}
```

In `search/service.rs`, dispatch on the clause field before building list params, mirroring
Node's `GRAPH_FIELDS` check: `near` → `get_neighborhood(value, hops.unwrap_or(1))`;
`path` → `find_shortest_path_with_details(from, to)` and set `graph_meta.type = "path"`;
`affected-by` → `get_chunks_affected_by_requirement(value, hops.unwrap_or(2))`;
`similar-to` → the embedding path, setting `graph_meta.type = "semantic"`.

Resolved graph ids become an id filter on the chunk list; they do **not** bypass user scoping —
`chunk::list`'s unconditional `user_id` predicate still applies, so a graph edge pointing at
another user's chunk cannot leak it. Add a test for exactly that.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p fubbik-db --test age_queries && cargo test -p fubbik-api --test search`
Expected: PASS.

- [ ] **Step 5: Verify the degradation path is genuinely exercised**

Make `get_neighborhood` propagate its error instead of degrading. Confirm `a_graph_clause_degrades_to_empty_when_age_is_unavailable` FAILS. Restore. Report the message. A happy-path-only test would let a 500 regression ship.

- [ ] **Step 6: Commit**

```bash
git commit -m "feat(age): wire graph queries into search clauses and scan-impact" -- crates/fubbik-db/src/age.rs crates/fubbik-db/tests/age_queries.rs crates/fubbik-api/src/search crates/fubbik-api/src/staleness openapi.json
```

---

## Task 10: Project connections into the graph, plus an idempotent backfill

**Files:**
- Modify: `crates/fubbik-db/src/repo/connection.rs`
- Create: `crates/fubbik-db/tests/age_sync.rs`, a backfill entry point in the `fubbik` binary

**Interfaces:**
- Produces: `age::ensure_vertex(&PgPool, chunk_id) -> AppResult<()>`,
  `age::create_edge(&PgPool, label: &str, from: &str, to: &str) -> AppResult<()>`,
  `age::delete_edge(&PgPool, label: &str, from: &str, to: &str) -> AppResult<()>`,
  `age::backfill_connections(&PgPool) -> AppResult<u64>` (returns rows projected),
  `age::count_edges_between(&PgPool, a: &str, b: &str) -> AppResult<i64>` (test helper).
- Consumes: `age::cypher`, `age::esc_cypher` from Task 9.

**This modifies `connections`, a Phase 2a domain covered by the differential harness. It must not regress.**

Node's `packages/db/src/repository/connection.ts` calls `ensureVertex` twice plus `createEdge("connects", …)` on create, and `deleteEdge` on delete — 8 call sites. The Rust port does none of it, so the `connects` edge set stopped tracking `chunk_connection`.

**Backfill:** existing rows were never projected. Ship a one-time, **idempotent** walk of `chunk_connection` issuing `ensure_vertex`/`create_edge` per row. It runs **explicitly** — never automatically at startup. An implicit graph rewrite at boot is exactly the kind of thing that should not surprise anyone.

- [ ] **Step 1: Write the failing tests**

```rust
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn creating_a_connection_projects_an_edge(pool: PgPool) {
    let alice = seed_user(&pool, "alice").await;
    let a = seed_chunk(&pool, &alice).await;
    let b = seed_chunk(&pool, &alice).await;

    connection::create(&pool, &alice, &a, &b, "related_to").await.unwrap();

    let ids = age::get_neighborhood(&pool, &a, 1).await.unwrap();
    assert!(ids.contains(&b), "creating a connection must project a `connects` edge into the graph");
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn deleting_a_connection_removes_the_edge(pool: PgPool) {
    let alice = seed_user(&pool, "alice").await;
    let a = seed_chunk(&pool, &alice).await;
    let b = seed_chunk(&pool, &alice).await;
    let c = connection::create(&pool, &alice, &a, &b, "related_to").await.unwrap();

    connection::delete(&pool, &alice, &c.id).await.unwrap();

    let ids = age::get_neighborhood(&pool, &a, 1).await.unwrap();
    assert!(!ids.contains(&b), "deleting a connection must remove its edge");
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn backfill_is_idempotent(pool: PgPool) {
    let alice = seed_user(&pool, "alice").await;
    let a = seed_chunk(&pool, &alice).await;
    let b = seed_chunk(&pool, &alice).await;
    // insert a row directly, bypassing the projection, to simulate pre-existing data
    sqlx::query("INSERT INTO chunk_connection (id, source_id, target_id, relation) VALUES ($1,$2,$3,'related_to')")
        .bind("conn-1").bind(&a).bind(&b).execute(&pool).await.unwrap();

    let first = age::backfill_connections(&pool).await.unwrap();
    let second = age::backfill_connections(&pool).await.unwrap();
    assert_eq!(first, 1);

    let edges = age::count_edges_between(&pool, &a, &b).await.unwrap();
    assert_eq!(edges, 1, "re-running the backfill must not duplicate the edge (ran twice, got {second} the second time)");
}
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p fubbik-db --test age_sync`
Expected: FAIL — connections do not project.

- [ ] **Step 3: Implement projection and backfill**

`ensure_vertex` must be idempotent — Cypher `MERGE`, not `CREATE`, or the backfill duplicates
vertices on its second run:

```rust
pub async fn ensure_vertex(pool: &PgPool, chunk_id: &str) -> AppResult<()> {
    let q = format!("MERGE (c:chunk {{id: '{}'}})", esc_cypher(chunk_id));
    cypher(pool, &q).await.map(|_| ())
}

pub async fn create_edge(pool: &PgPool, label: &str, from: &str, to: &str) -> AppResult<()> {
    // MERGE, not CREATE — re-running the backfill must not add a second parallel edge.
    let q = format!(
        "MATCH (a:chunk {{id: '{}'}}), (b:chunk {{id: '{}'}}) MERGE (a)-[:{}]->(b)",
        esc_cypher(from), esc_cypher(to), esc_cypher(label)
    );
    cypher(pool, &q).await.map(|_| ())
}
```

In `connection::create`, project **after** the SQL insert succeeds, mirroring Node's ordering
(`ensure_vertex` on both endpoints, then `create_edge("connects", …)`). In `connection::delete`,
call `delete_edge` after the row is removed.

**Do not roll back the SQL write if projection fails.** Node's projection errors do not undo the
insert, so a stricter Rust would diverge silently — the connection would vanish where Node keeps
it. Log the failure and return success, and state this explicitly in the report.

The backfill walks `chunk_connection` in id order and issues `ensure_vertex`/`create_edge` per
row. Expose it as an explicit subcommand on the `fubbik` binary — **never on the startup path**.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p fubbik-db --test age_sync && cargo test -p fubbik-db --test connection`
Expected: PASS — including the **pre-existing** Phase 2a connection tests, unchanged.

- [ ] **Step 5: Prove no regression in the Phase 2a domain**

Run the full existing connection suite and confirm every prior test still passes untouched. If a projection failure can make `connection::create` fail, decide and document whether that is acceptable — Node's projection errors do not roll back the SQL insert.

- [ ] **Step 6: Commit**

```bash
git commit -m "feat(connections): project vertices and edges into AGE, with idempotent backfill" -- crates/fubbik-db/src/repo/connection.rs crates/fubbik-db/tests/age_sync.rs
```

---

## Task 11: Limit clamp, harness extension, and the live differential run

**Files:**
- Modify: `crates/fubbik-api/src/chunks/dto.rs:121`, `crates/fubbik-api/tests/differential.rs`

**Divergence #11, resolved: align Rust's chunk-list limit clamp to Node's 100.**

Node caps at 100 (`packages/api/src/chunks/service.ts:50`, `Math.min(Number(query.limit ?? 50), 100)`); Rust caps at 500 (`.clamp(1, 500)`). Change Rust to `.clamp(1, 100)`.

**The clamp fix must land WITH a harness case above both caps.** This survived three phases unnoticed because the harness's only limit case is `?limit=5` — below both caps, so it could never see the difference. Fixing the clamp without fixing the blind spot leaves the next such divergence equally invisible.

**Ordering — compare as multisets only where Node has no `ORDER BY`.** Verified: `listTaskChunks`, `listTaskChunksWithTitles`, `listTaskDependencies`, `searchChunkTitles`, `searchRequirementTitles`, `getTagsForUser`, `getStaleFlagsForChunk`. Everything else here has an explicit `orderBy` and stays sequence-compared. **Do not blanket-mark endpoints unordered** — that makes the harness pass by being blind, which is the failure it exists to prevent.

**The harness runs as a single user**, so it cannot see divergences #13 and #14 at all. Do not treat a clean run as evidence those guards work; their proof lives in the cross-user tests.

- [ ] **Step 1: Write the failing harness test for the clamp**

```rust
#[tokio::test]
#[ignore = "requires both stacks running"]
async fn chunk_list_limit_is_clamped_identically_above_both_caps() {
    // ?limit=5 (the only pre-existing case) is below BOTH caps and can never catch this.
    let node = get_json(node_url("/api/chunks?limit=200")).await;
    let rust = get_json(rust_url("/api/chunks?limit=200")).await;
    assert_eq!(
        node["chunks"].as_array().unwrap().len(),
        rust["chunks"].as_array().unwrap().len(),
        "both stacks must clamp the chunk list to the same maximum"
    );
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p fubbik-api --test differential -- --ignored chunk_list_limit`
Expected: FAIL with 100 vs 200 — proving the harness now sees the divergence it was blind to.

- [ ] **Step 3: Fix the clamp and add the new GET paths**

Change `.clamp(1, 500)` to `.clamp(1, 100)`. Add harness paths for every new GET, with the ordered/unordered decision above.

- [ ] **Step 4: Confirm the harness still skips cleanly**

Run: `cargo test -p fubbik-api --test differential`
Expected: PASS with the live tests reported as **ignored** — and `-- --ignored` unconfigured must still hard-fail with `FUBBIK_NODE_URL not set`, the behaviour added in 2b. Verify both directions.

- [ ] **Step 5: Run the live comparison**

**Needs the Node server — ASK THE HUMAN PARTNER before starting it.** Then:
`DATABASE_URL="postgres://pontus@localhost:5432/fubbik" ./scripts/differential.sh`

Report every mismatch with its classification: Rust defect (fix), known divergence (record), environmental difference, or Node bug (record, do not fix). Divergence #6 — Rust's `id` tiebreaker versus Node's undefined ordering — is the most likely to surface, as it was in 2b.

Stop Node afterwards and confirm `lsof -ti:3000` is empty.

- [ ] **Step 6: Commit**

```bash
git commit -m "fix(chunks): align limit clamp to Node and extend the differential harness" -- crates/fubbik-api/src/chunks/dto.rs crates/fubbik-api/tests/differential.rs openapi.json
```

---

## Exit Criteria

- [ ] `cargo test --workspace` green; report the count (baseline at branch point: **355 passed, 6 ignored**).
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` zero; `cargo fmt --check` clean.
- [ ] `SQLX_OFFLINE=true cargo check --workspace --all-targets` clean.
- [ ] `openapi.json` regenerated and its staleness guard passing.
- [ ] All 41 endpoints reachable and user-scoped, with cross-user tests asserting 404 **and** unchanged victim data.
- [ ] Every SQL ownership guard enumerated and proven load-bearing by removal, **each naming the layer whose test failed**.
- [ ] Every list query has a total ordering, with a stability test verified to fail without its tiebreaker.
- [ ] Mark-done + unblock proven atomic by a forced mid-transaction failure.
- [ ] AGE graph clauses proven to degrade to empty, not 500, when AGE is unavailable.
- [ ] Connections project into the graph on create and delete; the backfill is idempotent.
- [ ] The chunk-list limit clamp matches Node, with a harness case **above both caps**.
- [ ] `crates/fubbik-db/migrations/` unchanged; `.sqlx` additive-only.
- [ ] `apps/web` and `packages/` byte-identical to the branch base, web type check at zero errors.
- [ ] The differential harness covers the new GET endpoints and has been run live at least once, with results recorded.
