# Rust Phase 4a Implementation Plan — Graph & AGE

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Serve `GET /api/graph` from Rust — the graph page's only backend call — and project behavior rules into the AGE graph on a schedule, fixing the silently-broken space filter on the way.

**Architecture:** Repository → service → route, as every ported domain. Five plain-SQL reads in a new `fubbik-db` repo module, two AGE reads through a multi-column Cypher helper that already exists privately, and a `tokio` background job beside the existing staleness scan.

**Tech Stack:** Rust (axum 0.8, sqlx 0.8 with the compile-time-checked `query_as!` macro, utoipa, tokio), Apache AGE (Cypher over `agtype`), TypeScript (`openapi-typescript`).

**Spec:** `docs/superpowers/specs/2026-08-26-rust-phase-4a-design.md` — read it first. It records *why* eleven routes and five domains are deliberately not ported; without it this plan looks like it forgot half the work.

## Global Constraints

- **Run cargo in the main tree. Do NOT set `CARGO_TARGET_DIR`. Do NOT create a git worktree** unless told to — disk runs near capacity.
- **`export DATABASE_URL="postgres://postgres:password@localhost:5434/fubbik_rs"`** before running any Rust test, or every sqlx test panics with `DATABASE_URL must be set: EnvVar(NotPresent)` — this looks like real breakage and is pure harness error.
- **Every `docker` command needs `--context orbstack`.** The default `desktop-linux` context points at a Docker Desktop that is not running, and its error misleadingly reads as an API version mismatch. Never run `docker context use` — it rewrites the user's global config.
- **Run tests in the FOREGROUND.**
- **Commit each task as its tests pass.** Do not hold a large uncommitted draft.
- Never edit an applied migration. `crates/fubbik-db/migrations/` must end this slice byte-identical to base **except** for the comment correction in Task 8, which touches only a `--` comment block and no SQL.
- **`cargo sqlx prepare --workspace` alone DROPS test-target-only entries.** Use `cargo sqlx prepare --workspace -- --tests`; verify the `.sqlx` diff is additive-only.
- Stale `_sqlx_test_*` databases cause phantom FK failures — drop before investigating any failure:
  `docker --context orbstack exec fubbik-rs-db psql -U postgres -d postgres -Atc "SELECT 'DROP DATABASE IF EXISTS \"'||datname||'\";' FROM pg_database WHERE datname LIKE '\_sqlx\_test%'" | docker --context orbstack exec -i fubbik-rs-db psql -U postgres -d postgres`
- Commit with the explicit pathspec form (`git commit -m "msg" -- <paths>`); `git add` new files first; never `git add -A`. No `Co-Authored-By` or "Generated with Claude" trailers.
- **Do NOT start the Node server against the user's live knowledge base without asking.** Task 8 edits Node source but requires no running server.
- Branch: `rust-phase4a`, already created off `rust-phase2e` at `357f07f`.

### AGE is not available in CI. This slice's core is therefore local-verify-only.

`.github/workflows/rust.yml:17` runs `pgvector/pgvector:pg18`, which has **no `age` extension**. `0001_init.sql:6-16` wraps `CREATE EXTENSION age` in an exception handler precisely so migrations still succeed without it, and `fubbik_db::age::cypher` returns `Ok(vec![])` when `is_available` is false.

The consequence: **every AGE test in this slice passes vacuously in CI.** `crates/fubbik-db/tests/age.rs:17-20` shows the established shape — `if !age::is_available(&pool) { eprintln!("skipping"); return; }`. A green CI run proves nothing about Tasks 2, 5 or the AGE half of Task 4.

Therefore:
- Task 0 verifies AGE is present locally and **fails the task if it is not**. Do not proceed without it.
- Every AGE test keeps the `is_available` guard (so CI stays green) but Task 0's check is what makes local runs meaningful.
- When reporting results, state explicitly whether AGE was available in the run being reported. "Tests pass" without that qualifier is not a claim about this slice.

### Two deliberate divergences from Node — do not "fix" them back

Both were decided in the spec and approved. Anyone reviewing against Node will see a difference; it is intended.

1. **`BEHAVIOR_GRAPH_SYNC_INTERVAL_HOURS`**, default `24`, `0` disables. Node gates behavior sync behind `STALENESS_SCAN_INTERVAL_HOURS` (`packages/api/src/startup.ts:60`), which has nothing to do with it.
2. **Behavior sync runs for every user**, not only the implicit dev user. Node calls `syncBehaviorsToGraph(IMPLICIT_DEV_USER_ID)` (`startup.ts:52`).

### Correction to the spec, found while planning

The spec says to *add* a multi-column Cypher helper. **It already exists.** `crates/fubbik-db/src/age.rs:104` defines `async fn cypher_multi(pool, graph, query, columns)` — private, and already used twice internally (lines 505 and 727). Task 1 is therefore *publishing* it, not writing it.

Conversely, the spec did not note that `ensure_vertex` (`age.rs:277`), `create_edge` (`age.rs:302`) and `delete_edge` (`age.rs:329`) are all hardcoded to the `chunk` label and the `connects` edge type. Behavior sync needs `behavior_rule` vertices and `governs` edges, so Task 2 is real new work the spec under-described. Net effort is roughly unchanged; the shape is different.

---

## File Structure

| File | Responsibility |
| --- | --- |
| `crates/fubbik-db/src/age.rs` (modify) | Publish `cypher_columns`; add the five `behavior_rule` / `governs` graph primitives |
| `crates/fubbik-db/src/repo/graph.rs` (create) | The four plain-SQL reads the graph payload needs that no repo module already provides |
| `crates/fubbik-db/src/repo/mod.rs` (modify) | Register `graph` |
| `crates/fubbik-api/src/graph/mod.rs` (create) | Module wiring |
| `crates/fubbik-api/src/graph/dto.rs` (create) | The seven-field response shape |
| `crates/fubbik-api/src/graph/service.rs` (create) | Compose the SQL reads with the two AGE reads; degrade AGE to empty |
| `crates/fubbik-api/src/graph/routes.rs` (create) | `GET /api/graph`, query params, utoipa annotation |
| `crates/fubbik-api/src/graph/sync.rs` (create) | Behavior-rule projection + the background job |
| `crates/fubbik-api/src/lib.rs` (modify) | `pub mod graph;` + `.merge(graph::routes::router())` |
| `crates/fubbik-api/src/openapi.rs` (modify) | Register the path and the schemas |
| `crates/fubbik/src/main.rs` (modify) | Spawn the sync job beside the staleness scan |
| `crates/fubbik-db/tests/age_behavior.rs` (create) | AGE primitives: round-trip, idempotence, quote safety |
| `crates/fubbik-db/tests/graph_repo.rs` (create) | The scoping matrix for the SQL reads |
| `crates/fubbik-api/tests/graph.rs` (create) | HTTP: shape, scoping, auth |
| `packages/api/src/graph/routes.ts` (modify) | `codebaseId` → `spaceId` on all six routes |
| `apps/web/src/features/graph/use-graph-data.ts` etc. (modify) | Four call sites onto `api` |

---

## Task 0: Restore the test database and record the baseline

**Files:** none — this task produces a verified environment and two recorded numbers.

**Interfaces:**
- Produces: a running `fubbik-rs-db` on port 5434 with `vector`, `pg_trgm` **and `age`** installed; the current pass/fail/ignored counts that every later task compares against.

`docker --context orbstack ps -a` currently lists no `fubbik-rs-db` container at all — not stopped, absent. It must be recreated before anything else runs.

- [ ] **Step 1: Check whether the container exists**

Run: `docker --context orbstack ps -a --format '{{.Names}}\t{{.Status}}' | grep fubbik-rs-db`

If it prints a stopped container, `docker --context orbstack start fubbik-rs-db` and skip to Step 3.
If it prints nothing, continue to Step 2.

- [ ] **Step 2: Create it**

The image must provide **all three** of `pgvector`, `pg_trgm` and `age`, and must initialise with the ICU locale provider (see the comment block in `.github/workflows/rust.yml` — libc ordering silently breaks `crates/fubbik-db/tests/collation.rs`).

```bash
docker --context orbstack run -d --name fubbik-rs-db \
  -p 5434:5432 \
  -e POSTGRES_PASSWORD=password \
  -e POSTGRES_DB=fubbik_rs \
  -e POSTGRES_INITDB_ARGS="--locale-provider=icu --icu-locale=en-US" \
  apache/age:PG16_latest
```

**`apache/age` ships AGE but not pgvector.** If `CREATE EXTENSION vector` fails in Step 3, stop and ask the human partner which image they were using before — do not silently proceed on a database missing an extension, and do not edit `0001_init.sql` to work around it. Getting this wrong produces a run where half the suite skips and the output still says "ok".

- [ ] **Step 3: Verify all three extensions**

```bash
export DATABASE_URL="postgres://postgres:password@localhost:5434/fubbik_rs"
docker --context orbstack exec fubbik-rs-db psql -U postgres -d fubbik_rs -c \
  "CREATE EXTENSION IF NOT EXISTS vector; CREATE EXTENSION IF NOT EXISTS pg_trgm; CREATE EXTENSION IF NOT EXISTS age;"
docker --context orbstack exec fubbik-rs-db psql -U postgres -d fubbik_rs -Atc \
  "SELECT extname FROM pg_extension ORDER BY extname"
```

Expected output must contain `age`, `pg_trgm` and `vector`. **If `age` is missing, this task has failed** — report it and stop. Every AGE assertion in Tasks 1, 2, 4 and 5 would silently no-op.

- [ ] **Step 4: Record the baseline**

Run: `cargo test --workspace 2>&1 | tail -30`

Write the exact `N passed; M failed; K ignored` line into the task report. Every later task compares against it. Also run and record:

```bash
cd apps/web && pnpm exec tsgo -p tsconfig.json --noEmit 2>&1 | tail -5
```

- [ ] **Step 5: Confirm AGE was live during that run**

Run: `cargo test -p fubbik-db --test age 2>&1 | grep -c "skipping"`

Expected: `0`. A non-zero count means the tests degraded rather than ran, and Step 3 lied.

No commit — nothing changed.

---

## Task 1: Publish the multi-column Cypher helper

**Files:**
- Modify: `crates/fubbik-db/src/age.rs:104` (`cypher_multi` — change visibility, add a public wrapper)
- Test: `crates/fubbik-db/tests/age.rs` (append)

**Interfaces:**
- Consumes: nothing.
- Produces: `pub async fn cypher_columns(pool: &PgPool, query: &str, columns: &[&str]) -> Result<Vec<std::collections::HashMap<String, serde_json::Value>>, sqlx::Error>` — fixed to the `"knowledge"` graph, exactly as `cypher` is. Tasks 2 and 4 call it.

`cypher_multi` already does the work: it builds the `AS (col agtype, …)` record definition, casts every column `::varchar` (sqlx has no `agtype` decoder), and parses each cell with `parse_agtype`. It is private and graph-parameterized. Mirror the existing `cypher` / `cypher_in_graph` split rather than inventing a new shape.

- [ ] **Step 1: Write the failing test**

Append to `crates/fubbik-db/tests/age.rs`:

```rust
#[sqlx::test]
async fn cypher_columns_returns_named_columns(pool: sqlx::PgPool) {
    if !age::is_available(&pool).await {
        eprintln!("AGE unavailable in this database — skipping");
        return;
    }

    age::cypher(
        &pool,
        r#"CREATE (n:behavior_rule {id: 'r1', title: 'He said "hi"', layer: 'invariant'})"#,
    )
    .await
    .unwrap();

    let rows = age::cypher_columns(
        &pool,
        "MATCH (r:behavior_rule) RETURN r.id AS id, r.title AS title, r.layer AS layer",
        &["id", "title", "layer"],
    )
    .await
    .unwrap();

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["id"], "r1");
    assert_eq!(rows[0]["layer"], "invariant");
    // The whole point of parse_agtype over Node's `.replace(/"/g, "")`
    // (packages/api/src/graph/service.ts:86-127): an embedded double quote
    // survives instead of being stripped out of the middle of the value.
    assert_eq!(rows[0]["title"], r#"He said "hi""#);
}
```

- [ ] **Step 2: Run it and watch it fail**

Run: `cargo test -p fubbik-db --test age cypher_columns_returns_named_columns`
Expected: FAIL — `no function or associated item named cypher_columns found`.

- [ ] **Step 3: Add the public wrapper**

In `crates/fubbik-db/src/age.rs`, directly below `cypher` (which ends at line 34), add:

```rust
/// Multi-column sibling of [`cypher`], fixed to the `"knowledge"` graph.
/// Returns one `column name -> parsed value` map per row.
///
/// Prefer this over hand-parsing a single `v` column when a query `RETURN`s
/// several values: it inherits [`parse_agtype`], so quoted characters inside
/// property values survive. The TypeScript original stripped them with a raw
/// `String(v).replace(/"/g, "")` (`packages/api/src/graph/service.ts:86-127`),
/// silently corrupting any title containing a double quote.
pub async fn cypher_columns(
    pool: &PgPool,
    query: &str,
    columns: &[&str],
) -> Result<Vec<HashMap<String, serde_json::Value>>, sqlx::Error> {
    cypher_multi(pool, "knowledge", query, columns).await
}
```

`HashMap` is already imported at `age.rs:1`. Leave `cypher_multi` private — its two existing callers (lines 505, 727) pass an explicit graph name and must keep doing so.

- [ ] **Step 4: Run it and watch it pass**

Run: `cargo test -p fubbik-db --test age cypher_columns_returns_named_columns`
Expected: PASS.

- [ ] **Step 5: Confirm nothing regressed**

Run: `cargo test -p fubbik-db --test age`
Expected: all pass, zero `skipping` lines.

- [ ] **Step 6: Commit**

```bash
git commit -m "feat(age): publish the multi-column cypher helper" -- crates/fubbik-db/src/age.rs crates/fubbik-db/tests/age.rs
```

---

## Task 2: Behavior-rule graph primitives

**Files:**
- Modify: `crates/fubbik-db/src/age.rs` (append a new section)
- Test: `crates/fubbik-db/tests/age_behavior.rs` (create)

**Interfaces:**
- Consumes: `cypher_columns` from Task 1; existing `cypher` and `esc_cypher`.
- Produces:
  - `pub struct BehaviorRuleVertex { pub id: String, pub title: String, pub layer: String, pub matrix_id: String, pub category: String }` (derives `Debug, Clone, PartialEq, serde::Serialize, utoipa::ToSchema`, `#[serde(rename_all = "camelCase")]`)
  - `pub struct GovernsEdge { pub source_id: String, pub target_id: String, pub kind: String }` (same derives)
  - `pub async fn upsert_behavior_rule(pool: &PgPool, rule: &BehaviorRuleVertex) -> AppResult<()>`
  - `pub async fn delete_governs_edges(pool: &PgPool, rule_id: &str) -> AppResult<()>`
  - `pub async fn link_governs(pool: &PgPool, rule_id: &str, kind: &str, code_ref: &str) -> AppResult<()>`
  - `pub async fn list_behavior_rule_vertices(pool: &PgPool) -> AppResult<Vec<BehaviorRuleVertex>>`
  - `pub async fn list_governs_edges(pool: &PgPool) -> AppResult<Vec<GovernsEdge>>`

Task 4 uses the two `list_*`; Task 5 uses the three writers.

- [ ] **Step 1: Write the failing tests**

Create `crates/fubbik-db/tests/age_behavior.rs`:

```rust
use fubbik_db::age::{self, BehaviorRuleVertex};

fn rule(id: &str, title: &str) -> BehaviorRuleVertex {
    BehaviorRuleVertex {
        id: id.into(),
        title: title.into(),
        layer: "invariant".into(),
        matrix_id: "m1".into(),
        category: "auth".into(),
    }
}

#[sqlx::test]
async fn upsert_behavior_rule_is_idempotent_and_updates_props(pool: sqlx::PgPool) {
    if !age::is_available(&pool).await {
        eprintln!("AGE unavailable in this database — skipping");
        return;
    }

    age::upsert_behavior_rule(&pool, &rule("r1", "First")).await.unwrap();
    age::upsert_behavior_rule(&pool, &rule("r1", "Renamed")).await.unwrap();

    let rules = age::list_behavior_rule_vertices(&pool).await.unwrap();
    // MERGE on id, not CREATE: running twice must not stack a second vertex.
    assert_eq!(rules.len(), 1);
    assert_eq!(rules[0].title, "Renamed");
    assert_eq!(rules[0].matrix_id, "m1");
    assert_eq!(rules[0].category, "auth");
}

#[sqlx::test]
async fn behavior_rule_title_survives_quotes_and_apostrophes(pool: sqlx::PgPool) {
    if !age::is_available(&pool).await {
        eprintln!("AGE unavailable in this database — skipping");
        return;
    }

    age::upsert_behavior_rule(&pool, &rule("r1", r#"it's a "quoted" rule"#))
        .await
        .unwrap();

    let rules = age::list_behavior_rule_vertices(&pool).await.unwrap();
    assert_eq!(rules.len(), 1, "an apostrophe must not break the Cypher literal");
    assert_eq!(rules[0].title, r#"it's a "quoted" rule"#);
}

#[sqlx::test]
async fn governs_edges_rebuild_without_duplicating(pool: sqlx::PgPool) {
    if !age::is_available(&pool).await {
        eprintln!("AGE unavailable in this database — skipping");
        return;
    }

    age::cypher(&pool, "CREATE (:code_file {id: 'src/auth/session.ts'})")
        .await
        .unwrap();
    age::upsert_behavior_rule(&pool, &rule("r1", "Sessions expire")).await.unwrap();

    for _ in 0..2 {
        age::delete_governs_edges(&pool, "r1").await.unwrap();
        age::link_governs(&pool, "r1", "file", "auth/session.ts").await.unwrap();
    }

    let edges = age::list_governs_edges(&pool).await.unwrap();
    assert_eq!(edges.len(), 1, "delete-then-relink must not accumulate edges");
    assert_eq!(edges[0].source_id, "r1");
    assert_eq!(edges[0].target_id, "src/auth/session.ts");
    assert_eq!(edges[0].kind, "file");
}

#[sqlx::test]
async fn link_governs_is_a_noop_when_no_code_vertex_matches(pool: sqlx::PgPool) {
    if !age::is_available(&pool).await {
        eprintln!("AGE unavailable in this database — skipping");
        return;
    }

    age::upsert_behavior_rule(&pool, &rule("r1", "Orphan")).await.unwrap();
    // No code_file vertices exist at all — the normal state of this system,
    // since code-index is not ported (see the spec). MATCH finds nothing and
    // MERGE never runs; this must not error.
    age::link_governs(&pool, "r1", "file", "auth/session.ts").await.unwrap();

    assert!(age::list_governs_edges(&pool).await.unwrap().is_empty());
}
```

- [ ] **Step 2: Run them and watch them fail**

Run: `cargo test -p fubbik-db --test age_behavior`
Expected: FAIL to compile — `unresolved import fubbik_db::age::BehaviorRuleVertex`.

- [ ] **Step 3: Implement the primitives**

Append to `crates/fubbik-db/src/age.rs`:

```rust
// ---------------------------------------------------------------------------
// Behavior rules
// ---------------------------------------------------------------------------
//
// Ports `packages/api/src/matrices/graph-sync.ts`. Kept here rather than in
// `fubbik-api` for the same reason `get_chunks_affected_by_requirement` is
// here: hand-written Cypher belongs to the AGE layer, and the service above
// should not be assembling query strings.

/// A `behavior_rule` vertex as the graph stores it.
#[derive(Debug, Clone, PartialEq, serde::Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BehaviorRuleVertex {
    pub id: String,
    pub title: String,
    pub layer: String,
    pub matrix_id: String,
    pub category: String,
}

/// A `governs` edge from a rule to the code it controls.
#[derive(Debug, Clone, PartialEq, serde::Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct GovernsEdge {
    pub source_id: String,
    pub target_id: String,
    /// `file | symbol` — which vertex label the target is.
    pub kind: String,
}

/// `MERGE` the vertex on `id`, then `SET` its properties.
///
/// Two statements, not one `MERGE ... SET`, mirroring Node
/// (`graph-sync.ts:44-50`). AGE's `MERGE` support is the shakiest corner of
/// its Cypher implementation, and the split form is the one already proven to
/// work here by [`ensure_vertex`].
pub async fn upsert_behavior_rule(pool: &PgPool, rule: &BehaviorRuleVertex) -> AppResult<()> {
    cypher(
        pool,
        &format!(
            "MERGE (:behavior_rule {{id: '{}'}})",
            esc_cypher(&rule.id)
        ),
    )
    .await?;
    cypher(
        pool,
        &format!(
            "MATCH (r:behavior_rule {{id: '{}'}}) \
             SET r.title = '{}', r.layer = '{}', r.matrixId = '{}', r.category = '{}'",
            esc_cypher(&rule.id),
            esc_cypher(&rule.title),
            esc_cypher(&rule.layer),
            esc_cypher(&rule.matrix_id),
            esc_cypher(&rule.category),
        ),
    )
    .await?;
    Ok(())
}

/// Removes every `governs` edge leaving this rule, so the caller can rebuild
/// them. Deleting before relinking is what makes the sweep idempotent — the
/// alternative, `MERGE`-ing each edge, leaves edges behind for cell-code links
/// that were since deleted.
pub async fn delete_governs_edges(pool: &PgPool, rule_id: &str) -> AppResult<()> {
    cypher(
        pool,
        &format!(
            "MATCH (r:behavior_rule {{id: '{}'}})-[e:governs]->() DELETE e",
            esc_cypher(rule_id)
        ),
    )
    .await?;
    Ok(())
}

/// Links a rule to the code vertex whose id ends with `code_ref`.
///
/// `ENDS WITH` rather than `=` because `behavior_cell_code.ref` holds a
/// repo-relative path while `code_file.id` is absolute (`graph-sync.ts:57`).
/// A no-op when no such vertex exists, which is the normal state: `code-index`
/// is not ported, so nothing writes `code_file` or `code_symbol` vertices. See
/// the spec's "Nothing renders code or concept nodes".
pub async fn link_governs(
    pool: &PgPool,
    rule_id: &str,
    kind: &str,
    code_ref: &str,
) -> AppResult<()> {
    let label = if kind == "symbol" {
        "code_symbol"
    } else {
        "code_file"
    };
    cypher(
        pool,
        &format!(
            "MATCH (r:behavior_rule {{id: '{}'}}), (c:{label}) \
             WHERE c.id ENDS WITH '{}' \
             MERGE (r)-[:governs {{kind: '{}'}}]->(c)",
            esc_cypher(rule_id),
            esc_cypher(code_ref),
            esc_cypher(kind),
        ),
    )
    .await?;
    Ok(())
}

/// Every `behavior_rule` vertex. Degrades to empty when AGE is unavailable.
pub async fn list_behavior_rule_vertices(pool: &PgPool) -> AppResult<Vec<BehaviorRuleVertex>> {
    let rows = cypher_columns(
        pool,
        "MATCH (r:behavior_rule) \
         RETURN r.id AS id, r.title AS title, r.layer AS layer, \
                r.matrixId AS matrix_id, r.category AS category",
        &["id", "title", "layer", "matrix_id", "category"],
    )
    .await?;

    Ok(rows
        .into_iter()
        .filter_map(|row| {
            Some(BehaviorRuleVertex {
                id: row.get("id")?.as_str()?.to_string(),
                title: row.get("title").and_then(|v| v.as_str()).unwrap_or_default().to_string(),
                layer: row.get("layer").and_then(|v| v.as_str()).unwrap_or_default().to_string(),
                matrix_id: row.get("matrix_id").and_then(|v| v.as_str()).unwrap_or_default().to_string(),
                category: row.get("category").and_then(|v| v.as_str()).unwrap_or_default().to_string(),
            })
        })
        .collect())
}

/// Every `governs` edge. Degrades to empty when AGE is unavailable.
pub async fn list_governs_edges(pool: &PgPool) -> AppResult<Vec<GovernsEdge>> {
    let rows = cypher_columns(
        pool,
        "MATCH (r:behavior_rule)-[g:governs]->(c) \
         RETURN r.id AS source_id, c.id AS target_id, g.kind AS kind",
        &["source_id", "target_id", "kind"],
    )
    .await?;

    Ok(rows
        .into_iter()
        .filter_map(|row| {
            Some(GovernsEdge {
                source_id: row.get("source_id")?.as_str()?.to_string(),
                target_id: row.get("target_id")?.as_str()?.to_string(),
                kind: row.get("kind").and_then(|v| v.as_str()).unwrap_or_default().to_string(),
            })
        })
        .collect())
}
```

Note the asymmetry in the two `filter_map`s: `id` fields use `?` (a row without one is meaningless and gets dropped), while `title`/`layer`/`category`/`kind` fall back to `""` (a rule with no category is ordinary — Node writes `''` for it at `graph-sync.ts:48`).

- [ ] **Step 4: Run the tests and watch them pass**

Run: `cargo test -p fubbik-db --test age_behavior`
Expected: 4 passed, zero `skipping` lines.

- [ ] **Step 5: Commit**

```bash
git add crates/fubbik-db/tests/age_behavior.rs
git commit -m "feat(age): behavior_rule vertices and governs edges" -- crates/fubbik-db/src/age.rs crates/fubbik-db/tests/age_behavior.rs
```

---

## Task 3: The graph repository

**Files:**
- Create: `crates/fubbik-db/src/repo/graph.rs`
- Modify: `crates/fubbik-db/src/repo/mod.rs`
- Test: `crates/fubbik-db/tests/graph_repo.rs` (create)

**Interfaces:**
- Consumes: nothing from earlier tasks.
- Produces, all in `fubbik_db::repo::graph`:
  - `pub struct ChunkMeta { pub id: String, pub title: String, pub chunk_type: String, pub summary: Option<String>, pub created_at: UtcTimestamp }` — `#[serde(rename_all = "camelCase")]` with `#[serde(rename = "type")]` on `chunk_type`
  - `pub struct GraphConnection { pub id: String, pub source_id: String, pub target_id: String, pub relation: String }`
  - `pub struct ChunkTagWithType { pub chunk_id: String, pub tag_id: String, pub tag_name: String, pub tag_type_id: Option<String>, pub tag_type_name: Option<String>, pub tag_type_color: Option<String> }`
  - `pub struct ChunkSpaceMapping { pub chunk_id: String, pub space_id: String, pub space_name: String }`
  - `pub async fn list_chunk_meta(pool: &PgPool, user_id: &str, space_id: Option<&str>, workspace_id: Option<&str>) -> AppResult<Vec<ChunkMeta>>`
  - `pub async fn list_connections(pool: &PgPool, user_id: &str) -> AppResult<Vec<GraphConnection>>`
  - `pub async fn list_chunk_tags_with_types(pool: &PgPool, user_id: &str) -> AppResult<Vec<ChunkTagWithType>>`
  - `pub async fn list_chunk_space_mappings(pool: &PgPool, user_id: &str) -> AppResult<Vec<ChunkSpaceMapping>>`

There is no fifth function: tag types come from the existing `fubbik_db::repo::tag_type::list(pool, user_id)` (`tag_type.rs:74`), which already returns exactly the columns Node's `db.select().from(tagType)` does.

- [ ] **Step 1: Write the failing tests**

Create `crates/fubbik-db/tests/graph_repo.rs`:

```rust
use fubbik_db::repo::{chunk, graph, space, tag, tag_type, user, workspace};

async fn a_chunk(pool: &sqlx::PgPool, uid: &str, title: &str) -> String {
    chunk::create(
        pool,
        uid,
        chunk::NewChunk {
            title: title.into(),
            content: String::new(),
            chunk_type: "note".into(),
            ..Default::default()
        },
    )
    .await
    .unwrap()
    .id
}

async fn link_space(pool: &sqlx::PgPool, chunk_id: &str, space_id: &str) {
    sqlx::query!(
        "INSERT INTO chunk_space (chunk_id, space_id) VALUES ($1, $2)",
        chunk_id,
        space_id
    )
    .execute(pool)
    .await
    .unwrap();
}

async fn a_space(pool: &sqlx::PgPool, uid: &str, name: &str) -> String {
    space::create(
        pool,
        uid,
        space::NewSpace {
            name: name.into(),
            kind: "code".into(),
            description: None,
        },
        None,
    )
    .await
    .unwrap()
    .id
}

#[sqlx::test]
async fn chunk_meta_unscoped_returns_only_this_users_chunks(pool: sqlx::PgPool) {
    let mine = user::create(&pool, "a@b.test", "A", None).await.unwrap().id;
    let theirs = user::create(&pool, "c@d.test", "C", None).await.unwrap().id;
    a_chunk(&pool, &mine, "Mine").await;
    a_chunk(&pool, &theirs, "Theirs").await;

    let rows = graph::list_chunk_meta(&pool, &mine, None, None).await.unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].title, "Mine");
}

#[sqlx::test]
async fn chunk_meta_space_scope_includes_global_chunks(pool: sqlx::PgPool) {
    let uid = user::create(&pool, "a@b.test", "A", None).await.unwrap().id;
    let target = a_space(&pool, &uid, "Target").await;
    let other = a_space(&pool, &uid, "Other").await;

    let in_target = a_chunk(&pool, &uid, "In target").await;
    let in_other = a_chunk(&pool, &uid, "In other").await;
    a_chunk(&pool, &uid, "Global").await;
    link_space(&pool, &in_target, &target).await;
    link_space(&pool, &in_other, &other).await;

    let rows = graph::list_chunk_meta(&pool, &uid, Some(&target), None).await.unwrap();
    let titles: Vec<&str> = rows.iter().map(|r| r.title.as_str()).collect();

    // The rule that is easiest to lose in translation from Drizzle's
    // `OR id NOT IN (SELECT chunk_id FROM chunk_space)`: a chunk belonging to
    // NO space is global and appears under every scope.
    assert!(titles.contains(&"In target"));
    assert!(titles.contains(&"Global"), "global chunks must survive space scoping");
    assert!(!titles.contains(&"In other"));
}

#[sqlx::test]
async fn chunk_meta_workspace_scope_spans_member_spaces_and_wins_over_space_id(
    pool: sqlx::PgPool,
) {
    let uid = user::create(&pool, "a@b.test", "A", None).await.unwrap().id;
    let a = a_space(&pool, &uid, "A").await;
    let b = a_space(&pool, &uid, "B").await;
    let outside = a_space(&pool, &uid, "Outside").await;

    let ws = workspace::create(
        &pool,
        &uid,
        workspace::NewWorkspace {
            name: "WS".into(),
            description: None,
        },
    )
    .await
    .unwrap()
    .id;
    workspace::add_space(&pool, &uid, &ws, &a).await.unwrap();
    workspace::add_space(&pool, &uid, &ws, &b).await.unwrap();

    let in_a = a_chunk(&pool, &uid, "In A").await;
    let in_b = a_chunk(&pool, &uid, "In B").await;
    let in_outside = a_chunk(&pool, &uid, "In outside").await;
    link_space(&pool, &in_a, &a).await;
    link_space(&pool, &in_b, &b).await;
    link_space(&pool, &in_outside, &outside).await;

    // `outside` passed as space_id AND a workspace passed: Node's else-if
    // (packages/db/src/repository/graph.ts:13-25) means workspace wins and
    // space_id is ignored entirely.
    let rows = graph::list_chunk_meta(&pool, &uid, Some(&outside), Some(&ws))
        .await
        .unwrap();
    let titles: Vec<&str> = rows.iter().map(|r| r.title.as_str()).collect();

    assert!(titles.contains(&"In A"));
    assert!(titles.contains(&"In B"));
    assert!(!titles.contains(&"In outside"), "workspace must win over space_id");
}

#[sqlx::test]
async fn connections_include_edges_pointing_at_my_chunks(pool: sqlx::PgPool) {
    let mine = user::create(&pool, "a@b.test", "A", None).await.unwrap().id;
    let theirs = user::create(&pool, "c@d.test", "C", None).await.unwrap().id;
    let m = a_chunk(&pool, &mine, "Mine").await;
    let t = a_chunk(&pool, &theirs, "Theirs").await;

    // Inbound from a chunk I do not own: Node matches on source OR target,
    // so this counts as mine.
    sqlx::query!(
        "INSERT INTO chunk_connection (id, source_id, target_id, relation)
         VALUES ('c1', $1, $2, 'related_to')",
        t,
        m
    )
    .execute(&pool)
    .await
    .unwrap();

    let rows = graph::list_connections(&pool, &mine).await.unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].relation, "related_to");
}

#[sqlx::test]
async fn chunk_tags_keep_untyped_tags(pool: sqlx::PgPool) {
    let uid = user::create(&pool, "a@b.test", "A", None).await.unwrap().id;
    let c = a_chunk(&pool, &uid, "T").await;

    let tt = tag_type::create(&pool, &uid, "Layer", Some("#fff"), None)
        .await
        .unwrap()
        .id;
    let typed = tag::create(&pool, &uid, "backend", Some(&tt)).await.unwrap().id;
    let untyped = tag::create(&pool, &uid, "loose", None).await.unwrap().id;

    for tag_id in [&typed, &untyped] {
        sqlx::query!(
            "INSERT INTO chunk_tag (chunk_id, tag_id) VALUES ($1, $2)",
            c,
            tag_id
        )
        .execute(&pool)
        .await
        .unwrap();
    }

    let rows = graph::list_chunk_tags_with_types(&pool, &uid).await.unwrap();
    // A LEFT join, not an inner one: an untyped tag must still appear.
    assert_eq!(rows.len(), 2);
    let loose = rows.iter().find(|r| r.tag_name == "loose").unwrap();
    assert!(loose.tag_type_id.is_none());
    assert!(loose.tag_type_name.is_none());
}

#[sqlx::test]
async fn chunk_space_mappings_are_scoped_by_space_owner(pool: sqlx::PgPool) {
    let mine = user::create(&pool, "a@b.test", "A", None).await.unwrap().id;
    let theirs = user::create(&pool, "c@d.test", "C", None).await.unwrap().id;

    let my_space = a_space(&pool, &mine, "Mine").await;
    let their_space = a_space(&pool, &theirs, "Theirs").await;
    let mc = a_chunk(&pool, &mine, "MC").await;
    let tc = a_chunk(&pool, &theirs, "TC").await;
    link_space(&pool, &mc, &my_space).await;
    link_space(&pool, &tc, &their_space).await;

    let rows = graph::list_chunk_space_mappings(&pool, &mine).await.unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].space_name, "Mine");
}
```

If `tag_type::create` or `tag::create` have different signatures than assumed above, read them at `crates/fubbik-db/src/repo/tag_type.rs` and `tag.rs` and adjust the *call*, not the assertion.

- [ ] **Step 2: Run them and watch them fail**

Run: `cargo test -p fubbik-db --test graph_repo`
Expected: FAIL to compile — `unresolved import fubbik_db::repo::graph`.

- [ ] **Step 3: Create the module**

Create `crates/fubbik-db/src/repo/graph.rs`:

```rust
//! The four reads behind `GET /api/graph`.
//!
//! Ports `packages/db/src/repository/graph.ts` (107 LOC). Tag types are NOT
//! here — `tag_type::list` already returns exactly the columns Node's
//! `db.select().from(tagType)` does, and duplicating it would give the graph
//! its own drifting copy.
//!
//! Every query is ordered, unlike Node, which leaves order to the planner.
//! The web sorts nothing, so this costs nothing and makes the tests
//! deterministic.

use fubbik_core::error::AppResult;
use sqlx::PgPool;

use crate::timestamp::UtcTimestamp;

/// The five columns the graph needs off `chunk` — deliberately not the whole
/// row. The payload carries one of these per node and the full chunk is
/// fetched separately when a node is opened.
#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ChunkMeta {
    pub id: String,
    pub title: String,
    #[serde(rename = "type")]
    pub chunk_type: String,
    pub summary: Option<String>,
    #[schema(value_type = chrono::NaiveDateTime)]
    pub created_at: UtcTimestamp,
}

#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct GraphConnection {
    pub id: String,
    pub source_id: String,
    pub target_id: String,
    pub relation: String,
}

#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ChunkTagWithType {
    pub chunk_id: String,
    pub tag_id: String,
    pub tag_name: String,
    /// `None` for an untyped tag — the `tag_type` join is a LEFT join.
    pub tag_type_id: Option<String>,
    pub tag_type_name: Option<String>,
    pub tag_type_color: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ChunkSpaceMapping {
    pub chunk_id: String,
    pub space_id: String,
    pub space_name: String,
}

/// Chunk metadata, optionally scoped to a workspace or a single space.
///
/// **`workspace_id` wins over `space_id`.** Node's `if (workspaceId) … else if
/// (codebaseId)` (`graph.ts:13-25`) never evaluates the space branch when a
/// workspace is present, and the web can send both.
///
/// **A chunk in no space is global and appears under every scope.** That is
/// the `NOT IN (SELECT chunk_id FROM chunk_space)` disjunct — the same idiom
/// `chunk::list` uses at `chunk.rs:523-525`. Dropping it would make the graph
/// hide every un-spaced chunk the moment a space is selected.
pub async fn list_chunk_meta(
    pool: &PgPool,
    user_id: &str,
    space_id: Option<&str>,
    workspace_id: Option<&str>,
) -> AppResult<Vec<ChunkMeta>> {
    let rows = sqlx::query_as!(
        ChunkMeta,
        r#"SELECT c.id, c.title, c.type AS chunk_type, c.summary,
                  c.created_at AS "created_at: UtcTimestamp"
           FROM chunk c
           WHERE c.user_id = $1
             AND (
               ($2::text IS NULL AND $3::text IS NULL)
               OR c.id NOT IN (SELECT chunk_id FROM chunk_space)
               OR ($3::text IS NOT NULL AND c.id IN (
                     SELECT cs.chunk_id FROM chunk_space cs
                     JOIN workspace_space ws ON ws.space_id = cs.space_id
                     WHERE ws.workspace_id = $3))
               OR ($3::text IS NULL AND $2::text IS NOT NULL AND c.id IN (
                     SELECT chunk_id FROM chunk_space WHERE space_id = $2))
             )
           ORDER BY c.created_at DESC, c.id ASC"#,
        user_id,
        space_id,
        workspace_id
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Connections where either endpoint is one of this user's chunks.
///
/// Deliberately **not** space-scoped, matching Node: the web filters against
/// the chunk-id set it already has (`search-graph.tsx:87`), and scoping here
/// would silently drop edges that cross a space boundary.
pub async fn list_connections(pool: &PgPool, user_id: &str) -> AppResult<Vec<GraphConnection>> {
    let rows = sqlx::query_as!(
        GraphConnection,
        r#"SELECT cc.id, cc.source_id, cc.target_id, cc.relation
           FROM chunk_connection cc
           WHERE cc.source_id IN (SELECT id FROM chunk WHERE user_id = $1)
              OR cc.target_id IN (SELECT id FROM chunk WHERE user_id = $1)
           ORDER BY cc.id ASC"#,
        user_id
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Every chunk-tag pairing with its tag type, scoped by **tag** ownership —
/// `tag.user_id`, not `chunk.user_id`. That is what Node filters on
/// (`graph.ts:52`).
pub async fn list_chunk_tags_with_types(
    pool: &PgPool,
    user_id: &str,
) -> AppResult<Vec<ChunkTagWithType>> {
    let rows = sqlx::query_as!(
        ChunkTagWithType,
        r#"SELECT ct.chunk_id, t.id AS tag_id, t.name AS tag_name,
                  t.tag_type_id, tt.name AS tag_type_name, tt.color AS tag_type_color
           FROM chunk_tag ct
           JOIN tag t ON t.id = ct.tag_id
           LEFT JOIN tag_type tt ON tt.id = t.tag_type_id
           WHERE t.user_id = $1
           ORDER BY ct.chunk_id ASC, t.id ASC"#,
        user_id
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Chunk → space mappings, scoped by **space** ownership (`space.user_id`),
/// matching `graph.ts:78`. Only read when a workspace is selected; the service
/// keeps that conditional.
pub async fn list_chunk_space_mappings(
    pool: &PgPool,
    user_id: &str,
) -> AppResult<Vec<ChunkSpaceMapping>> {
    let rows = sqlx::query_as!(
        ChunkSpaceMapping,
        r#"SELECT cs.chunk_id, cs.space_id, s.name AS space_name
           FROM chunk_space cs
           JOIN space s ON s.id = cs.space_id
           WHERE s.user_id = $1
           ORDER BY cs.chunk_id ASC, cs.space_id ASC"#,
        user_id
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}
```

- [ ] **Step 4: Register the module**

In `crates/fubbik-db/src/repo/mod.rs`, add `pub mod graph;` in alphabetical position (between `feature` and `insights`).

- [ ] **Step 5: Run the tests and watch them pass**

Run: `cargo test -p fubbik-db --test graph_repo`
Expected: 6 passed.

If `query_as!` fails to compile with "error returned from database", the live database is the source of truth — confirm `DATABASE_URL` is exported and the container is up.

- [ ] **Step 6: Refresh the offline cache**

```bash
cargo sqlx prepare --workspace -- --tests
git status --short .sqlx
```

Expected: only additions under `.sqlx/`. If any file is **deleted**, the `-- --tests` flag was dropped — re-run.

- [ ] **Step 7: Commit**

```bash
git add crates/fubbik-db/src/repo/graph.rs crates/fubbik-db/tests/graph_repo.rs .sqlx
git commit -m "feat(graph): repository layer for the graph payload" -- crates/fubbik-db/src/repo/graph.rs crates/fubbik-db/src/repo/mod.rs crates/fubbik-db/tests/graph_repo.rs .sqlx
```

---

## Task 4: `GET /api/graph`

**Files:**
- Create: `crates/fubbik-api/src/graph/mod.rs`, `dto.rs`, `service.rs`, `routes.rs`
- Modify: `crates/fubbik-api/src/lib.rs`, `crates/fubbik-api/src/openapi.rs`
- Test: `crates/fubbik-api/tests/graph.rs` (create)

**Interfaces:**
- Consumes: `fubbik_db::repo::graph::*` and `tag_type::list` (Task 3); `fubbik_db::age::{list_behavior_rule_vertices, list_governs_edges, BehaviorRuleVertex, GovernsEdge}` (Task 2).
- Produces: `crate::graph::dto::GraphResponse`, `crate::graph::routes::get_graph`, `crate::graph::routes::router()`.

- [ ] **Step 1: Write the failing test**

Create `crates/fubbik-api/tests/graph.rs`. Copy the `state`, `signup`, `json_body` and `send` helpers verbatim from `crates/fubbik-api/tests/insights.rs:14-60` — every HTTP test file in this crate carries its own copy; do not try to share them.

Then:

```rust
#[sqlx::test]
async fn graph_requires_a_session(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    let res = app
        .oneshot(Request::get("/api/graph").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}

#[sqlx::test]
async fn graph_returns_the_seven_documented_fields(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    let cookie = signup(app.clone(), "a@b.test", "A").await;

    let res = send(app, &cookie, "GET", "/api/graph", serde_json::Value::Null).await;
    assert_eq!(res.status(), StatusCode::OK);
    let body = json_body(res).await;

    for field in [
        "chunks",
        "connections",
        "chunkTags",
        "tagTypes",
        "chunkCodebases",
        "behaviorRules",
        "governsEdges",
    ] {
        assert!(body.get(field).is_some(), "missing field {field}");
        assert!(body[field].is_array(), "{field} must be an array");
    }

    // The six fields Node returns and nothing reads are deliberately absent —
    // see the spec. Their absence is the documentation.
    for dropped in ["communities", "bridges", "codeFiles", "codeSymbols", "concepts", "coRefEdges"] {
        assert!(body.get(dropped).is_none(), "{dropped} should not be served");
    }
}

#[sqlx::test]
async fn graph_space_scoping_actually_filters(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "a@b.test", "A").await;

    let space = send(
        app.clone(),
        &cookie,
        "POST",
        "/api/spaces",
        serde_json::json!({ "name": "Target", "kind": "code" }),
    )
    .await;
    let space_id = json_body(space).await["id"].as_str().unwrap().to_string();

    let scoped = send(
        app.clone(),
        &cookie,
        "POST",
        "/api/chunks",
        serde_json::json!({ "title": "In target", "content": "x", "type": "note",
                            "spaceIds": [space_id] }),
    )
    .await;
    assert_eq!(scoped.status(), StatusCode::OK);

    let other_space = send(
        app.clone(),
        &cookie,
        "POST",
        "/api/spaces",
        serde_json::json!({ "name": "Other", "kind": "code" }),
    )
    .await;
    let other_id = json_body(other_space).await["id"].as_str().unwrap().to_string();
    send(
        app.clone(),
        &cookie,
        "POST",
        "/api/chunks",
        serde_json::json!({ "title": "In other", "content": "x", "type": "note",
                            "spaceIds": [other_id] }),
    )
    .await;

    let res = send(
        app,
        &cookie,
        "GET",
        &format!("/api/graph?spaceId={space_id}"),
        serde_json::Value::Null,
    )
    .await;
    let body = json_body(res).await;
    let titles: Vec<&str> = body["chunks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["title"].as_str().unwrap())
        .collect();

    // This is the bug being fixed. On Node the param is named `codebaseId`
    // (packages/api/src/graph/routes.ts:18), Elysia strips the unknown
    // `spaceId`, and BOTH titles come back.
    assert!(titles.contains(&"In target"));
    assert!(!titles.contains(&"In other"), "spaceId must actually scope the graph");
}
```

- [ ] **Step 2: Run it and watch it fail**

Run: `cargo test -p fubbik-api --test graph`
Expected: FAIL — all three 404, because no route is registered yet.

**This is the step that matters most in this task.** `graph_space_scoping_actually_filters` must be seen failing for the *right* reason before the fix. Record the observed failure in the task report — the rename bug survived because nothing ever asserted this.

- [ ] **Step 3: Write the DTO**

Create `crates/fubbik-api/src/graph/dto.rs`:

```rust
//! The `GET /api/graph` response.
//!
//! Seven fields, one per thing the web actually reads. Node returns thirteen;
//! the other six (`communities`, `bridges`, `codeFiles`, `codeSymbols`,
//! `concepts`, `coRefEdges`) have no reader in any client, and three of them
//! are permanently empty on Node too. Because `apps/web` types itself from
//! `openapi.json`, omitting them turns a future reader into a compile error
//! instead of a silent `undefined`.

use fubbik_db::age::{BehaviorRuleVertex, GovernsEdge};
use fubbik_db::repo::graph::{ChunkMeta, ChunkSpaceMapping, ChunkTagWithType, GraphConnection};
use fubbik_db::repo::tag_type::TagType;

#[derive(Debug, serde::Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct GraphResponse {
    pub chunks: Vec<ChunkMeta>,
    pub connections: Vec<GraphConnection>,
    pub chunk_tags: Vec<ChunkTagWithType>,
    pub tag_types: Vec<TagType>,
    /// Named `chunkCodebases` on the wire: the `codebase → space` rename never
    /// reached this field, and `apps/web/src/features/graph/group-strategies.ts:69`
    /// destructures the old name. Renaming it is a web change, not a port.
    #[serde(rename = "chunkCodebases")]
    pub chunk_spaces: Vec<ChunkSpaceMapping>,
    pub behavior_rules: Vec<BehaviorRuleVertex>,
    pub governs_edges: Vec<GovernsEdge>,
}
```

- [ ] **Step 4: Write the service**

Create `crates/fubbik-api/src/graph/service.rs`:

```rust
//! Assembles the graph payload.
//!
//! The AGE half degrades to empty on any failure, matching Node's
//! `Effect.catchAll` (`packages/api/src/graph/service.ts:38`): a database
//! without the extension still serves a working graph rather than a 500.

use fubbik_core::error::AppResult;
use fubbik_db::repo::{graph as repo, tag_type};
use sqlx::PgPool;

use super::dto::GraphResponse;

pub async fn build(
    pool: &PgPool,
    user_id: &str,
    space_id: Option<&str>,
    workspace_id: Option<&str>,
) -> AppResult<GraphResponse> {
    let chunks = repo::list_chunk_meta(pool, user_id, space_id, workspace_id).await?;
    let connections = repo::list_connections(pool, user_id).await?;
    let chunk_tags = repo::list_chunk_tags_with_types(pool, user_id).await?;
    let tag_types = tag_type::list(pool, user_id).await?;

    // Only populated for the workspace view. Node returns an empty array
    // otherwise (`service.ts:21-23`) rather than paying for the join, and the
    // only consumer — the "group by space" strategy — is workspace-only.
    let chunk_spaces = if workspace_id.is_some() {
        repo::list_chunk_space_mappings(pool, user_id).await?
    } else {
        Vec::new()
    };

    let behavior_rules = fubbik_db::age::list_behavior_rule_vertices(pool)
        .await
        .unwrap_or_default();
    let governs_edges = fubbik_db::age::list_governs_edges(pool)
        .await
        .unwrap_or_default();

    Ok(GraphResponse {
        chunks,
        connections,
        chunk_tags,
        tag_types,
        chunk_spaces,
        behavior_rules,
        governs_edges,
    })
}
```

- [ ] **Step 5: Write the route**

Create `crates/fubbik-api/src/graph/routes.rs`:

```rust
//! `GET /api/graph` — everything the graph page renders, in one payload.

use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};

use super::dto::GraphResponse;
use super::service;
use crate::AppState;
use crate::auth::CurrentUser;
use crate::error::ApiResult;
use crate::extract::Query;

#[derive(serde::Deserialize, utoipa::IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct GraphQuery {
    /// `spaceId`, NOT `codebaseId`. Node still declares the pre-rename name
    /// (`packages/api/src/graph/routes.ts:18`) while the web sends `spaceId`,
    /// so Elysia strips it and the filter silently does nothing. Task 8 fixes
    /// Node to match this.
    pub space_id: Option<String>,
    /// Takes precedence over `space_id` when both are present.
    pub workspace_id: Option<String>,
}

#[utoipa::path(get, path = "/api/graph", params(GraphQuery),
    responses((status = 200, body = GraphResponse)))]
pub async fn get_graph(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Query(query): Query<GraphQuery>,
) -> ApiResult<Json<GraphResponse>> {
    let response = service::build(
        &state.pool,
        &user.id,
        query.space_id.as_deref(),
        query.workspace_id.as_deref(),
    )
    .await?;
    Ok(Json(response))
}

pub fn router() -> Router<AppState> {
    Router::new().route("/api/graph", get(get_graph))
}
```

Create `crates/fubbik-api/src/graph/mod.rs`:

```rust
pub mod dto;
pub mod routes;
pub mod service;
```

- [ ] **Step 6: Register the module, the router and the OpenAPI entries**

In `crates/fubbik-api/src/lib.rs`: add `pub mod graph;` alphabetically among the other `pub mod` lines, and `.merge(graph::routes::router())` into the router chain (after `.merge(favorites::routes::router())`, before `.merge(density::routes::router())` — the list is not strictly alphabetical, so match its local ordering).

In `crates/fubbik-api/src/openapi.rs`: add `crate::graph::routes::get_graph,` to the `paths(...)` list and these to `components(schemas(...))`:

```rust
crate::graph::dto::GraphResponse,
fubbik_db::repo::graph::ChunkMeta,
fubbik_db::repo::graph::GraphConnection,
fubbik_db::repo::graph::ChunkTagWithType,
fubbik_db::repo::graph::ChunkSpaceMapping,
fubbik_db::age::BehaviorRuleVertex,
fubbik_db::age::GovernsEdge,
```

`TagType` is already registered — do not add it twice.

- [ ] **Step 7: Run the tests and watch them pass**

Run: `cargo test -p fubbik-api --test graph`
Expected: 3 passed.

- [ ] **Step 8: Regenerate `openapi.json` and confirm the guard**

```bash
cargo run -- openapi > openapi.json
cargo test -p fubbik-api --test openapi
cargo test -p fubbik-api --test schema_names
```

Expected: both pass. `schema_names` is the collision guard added after the `Position` incident — if it fails, two schemas registered under one name and one silently overwrote the other.

- [ ] **Step 9: Commit**

```bash
git add crates/fubbik-api/src/graph crates/fubbik-api/tests/graph.rs
git commit -m "feat(graph): GET /api/graph, with spaceId actually scoping" -- crates/fubbik-api/src/graph crates/fubbik-api/src/lib.rs crates/fubbik-api/src/openapi.rs crates/fubbik-api/tests/graph.rs openapi.json
```

---

## Task 5: The behavior-sync background job

**Files:**
- Create: `crates/fubbik-api/src/graph/sync.rs`
- Modify: `crates/fubbik-api/src/graph/mod.rs`, `crates/fubbik/src/main.rs:157`
- Test: unit tests inside `sync.rs` for the interval parser; an integration test appended to `crates/fubbik-api/tests/graph.rs`

**Interfaces:**
- Consumes: `fubbik_db::age::{upsert_behavior_rule, delete_governs_edges, link_governs, BehaviorRuleVertex}` (Task 2).
- Produces: `pub async fn sync_once(pool: &PgPool) -> AppResult<u64>` (returns rules synced) and `pub fn spawn_behavior_sync(pool: PgPool)`.

- [ ] **Step 1: Write the failing tests**

Append to `crates/fubbik-api/tests/graph.rs`:

```rust
#[sqlx::test]
async fn behavior_sync_projects_rules_for_every_user_and_is_idempotent(pool: sqlx::PgPool) {
    if !fubbik_db::age::is_available(&pool).await {
        eprintln!("AGE unavailable in this database — skipping");
        return;
    }

    let app = fubbik_api::router(state(pool.clone()));

    // Two DIFFERENT users, each with a matrix. Node syncs only the implicit
    // dev user (packages/api/src/startup.ts:52); this port syncs both, and
    // that divergence is the point of this assertion.
    for (email, name, title) in [
        ("a@b.test", "A", "Sessions expire"),
        ("c@d.test", "C", "Inputs are validated"),
    ] {
        let cookie = signup(app.clone(), email, name).await;
        let matrix = send(
            app.clone(),
            &cookie,
            "POST",
            "/api/matrices",
            serde_json::json!({ "name": "M", "layer": "invariant" }),
        )
        .await;
        let matrix_id = json_body(matrix).await["id"].as_str().unwrap().to_string();
        send(
            app.clone(),
            &cookie,
            "POST",
            &format!("/api/matrices/{matrix_id}/rules"),
            serde_json::json!({ "title": title, "category": "auth" }),
        )
        .await;
    }

    let first = fubbik_api::graph::sync::sync_once(&pool).await.unwrap();
    assert_eq!(first, 2, "both users' rules must be projected");

    let second = fubbik_api::graph::sync::sync_once(&pool).await.unwrap();
    assert_eq!(second, 2);

    let vertices = fubbik_db::age::list_behavior_rule_vertices(&pool).await.unwrap();
    assert_eq!(vertices.len(), 2, "a second sweep must not duplicate vertices");
    let titles: Vec<&str> = vertices.iter().map(|v| v.title.as_str()).collect();
    assert!(titles.contains(&"Sessions expire"));
    assert!(titles.contains(&"Inputs are validated"));
}
```

And, inside `sync.rs` itself, a `#[cfg(test)] mod tests` for the interval parser (no database needed):

```rust
#[cfg(test)]
mod tests {
    use super::resolve_sync_interval;

    #[test]
    fn defaults_to_24_hours_when_unset_or_garbage() {
        assert_eq!(resolve_sync_interval(None).unwrap().as_secs(), 86_400);
        assert_eq!(resolve_sync_interval(Some("banana")).unwrap().as_secs(), 86_400);
    }

    #[test]
    fn zero_and_negative_disable_the_job() {
        assert!(resolve_sync_interval(Some("0")).is_none());
        assert!(resolve_sync_interval(Some("-1")).is_none());
    }

    #[test]
    fn fractional_hours_are_honoured() {
        assert_eq!(resolve_sync_interval(Some("0.5")).unwrap().as_secs(), 1_800);
    }
}
```

- [ ] **Step 2: Run them and watch them fail**

Run: `cargo test -p fubbik-api --test graph behavior_sync`
Expected: FAIL to compile — `could not find sync in graph`.

- [ ] **Step 3: Write the sync module**

Create `crates/fubbik-api/src/graph/sync.rs`:

```rust
//! Projects behavior rules into the AGE graph on a schedule.
//!
//! Ports `packages/api/src/matrices/graph-sync.ts`, with two deliberate
//! divergences recorded in
//! `docs/superpowers/specs/2026-08-26-rust-phase-4a-design.md`:
//!
//! 1. Its own interval variable, `BEHAVIOR_GRAPH_SYNC_INTERVAL_HOURS`. Node
//!    gates this job behind `STALENESS_SCAN_INTERVAL_HOURS`
//!    (`packages/api/src/startup.ts:60`), which names a different job.
//! 2. Every user, not only the implicit dev user (`startup.ts:52`). On a
//!    single-user install these are identical; on any other, Node's version
//!    silently syncs nothing for everyone else.
//!
//! One SQL read replaces Node's per-matrix, per-rule, per-cell loop: the join
//! it walks by hand is a join.

use fubbik_core::error::AppResult;
use fubbik_db::age::{self, BehaviorRuleVertex};
use sqlx::PgPool;

/// `None` means "disabled". Mirrors `staleness::service::resolve_scan_interval`
/// exactly, including its tolerance of unparseable input (fall back to the
/// default rather than crash a server at boot over a typo in an env var).
pub fn resolve_sync_interval(hours: Option<&str>) -> Option<std::time::Duration> {
    let hours: f64 = hours.and_then(|s| s.parse().ok()).unwrap_or(24.0);
    if hours <= 0.0 {
        None
    } else {
        Some(std::time::Duration::from_secs_f64(hours * 3600.0))
    }
}

#[derive(sqlx::FromRow)]
struct RuleRow {
    id: String,
    title: String,
    layer: String,
    matrix_id: String,
    category: Option<String>,
}

#[derive(sqlx::FromRow)]
struct CodeLinkRow {
    rule_id: String,
    kind: String,
    code_ref: String,
}

/// Runs one full sweep. Returns the number of rules projected.
///
/// Degrades to `Ok(0)` when AGE is unavailable rather than erroring — a
/// server without the extension must still boot and serve.
pub async fn sync_once(pool: &PgPool) -> AppResult<u64> {
    if !age::is_available(pool).await {
        return Ok(0);
    }

    let rules = sqlx::query_as!(
        RuleRow,
        r#"SELECT r.id, r.title, m.layer, r.matrix_id, r.category
           FROM behavior_rule r
           JOIN behavior_matrix m ON m.id = r.matrix_id
           ORDER BY r.id ASC"#
    )
    .fetch_all(pool)
    .await?;

    let links = sqlx::query_as!(
        CodeLinkRow,
        r#"SELECT c.rule_id, bcc.kind, bcc.ref AS code_ref
           FROM behavior_cell_code bcc
           JOIN behavior_cell c ON c.id = bcc.cell_id
           WHERE bcc.kind IN ('file', 'symbol')
           ORDER BY c.rule_id ASC, bcc.id ASC"#
    )
    .fetch_all(pool)
    .await?;

    let mut synced = 0u64;
    for rule in &rules {
        age::upsert_behavior_rule(
            pool,
            &BehaviorRuleVertex {
                id: rule.id.clone(),
                title: rule.title.clone(),
                layer: rule.layer.clone(),
                matrix_id: rule.matrix_id.clone(),
                // Node writes '' for a missing category (`graph-sync.ts:48`).
                category: rule.category.clone().unwrap_or_default(),
            },
        )
        .await?;

        // Delete before relinking: MERGE alone would leave edges behind for
        // cell-code links that have since been deleted.
        age::delete_governs_edges(pool, &rule.id).await?;
        for link in links.iter().filter(|l| l.rule_id == rule.id) {
            age::link_governs(pool, &rule.id, &link.kind, &link.code_ref).await?;
        }

        synced += 1;
    }

    tracing::info!(rules = synced, "Behavior rules synced to graph");
    Ok(synced)
}

/// Spawns the recurring sweep. Called once, from `Commands::Serve`.
pub fn spawn_behavior_sync(pool: PgPool) {
    let Some(interval) = resolve_sync_interval(
        std::env::var("BEHAVIOR_GRAPH_SYNC_INTERVAL_HOURS")
            .ok()
            .as_deref(),
    ) else {
        tracing::info!("Behavior graph sync disabled (BEHAVIOR_GRAPH_SYNC_INTERVAL_HOURS<=0)");
        return;
    };
    tracing::info!(
        interval_secs = interval.as_secs(),
        "Behavior graph sync enabled"
    );

    tokio::spawn(async move {
        // 40s, matching Node's offset from the staleness scan
        // (`packages/api/src/startup.ts:78`) — the two jobs both touch AGE and
        // staggering them keeps a cold start from contending.
        tokio::time::sleep(std::time::Duration::from_secs(40)).await;
        run_once_logged(&pool).await;
        loop {
            tokio::time::sleep(interval).await;
            run_once_logged(&pool).await;
        }
    });
}

async fn run_once_logged(pool: &PgPool) {
    match sync_once(pool).await {
        Ok(n) => tracing::info!(rules = n, "Behavior graph sync completed"),
        Err(e) => tracing::error!(error = %e, "Behavior graph sync failed"),
    }
}
```

Then append the `#[cfg(test)] mod tests` block from Step 1 to the end of this file, and add `pub mod sync;` to `crates/fubbik-api/src/graph/mod.rs`.

**Verify the column names before running.** `behavior_cell_code`'s columns are declared in `crates/fubbik-db/migrations/0004_behavior_matrix_completion.sql` and mirrored in `crates/fubbik-db/src/repo/behavior_matrix.rs:141` (`BehaviorCellCode`). `ref` is a reserved-ish word in some contexts — if `query_as!` rejects it, quote it as `bcc."ref"`. Read the struct rather than guessing at `kind`'s allowed values.

- [ ] **Step 4: Run the tests and watch them pass**

Run: `cargo test -p fubbik-api --test graph`
Run: `cargo test -p fubbik-api graph::sync`
Expected: all pass, no `skipping` line in the first.

- [ ] **Step 5: Wire it into the binary**

In `crates/fubbik/src/main.rs`, directly after line 157:

```rust
            fubbik_api::staleness::service::spawn_background_scan(pool.clone());
            fubbik_api::graph::sync::spawn_behavior_sync(pool.clone());
```

- [ ] **Step 6: Refresh the offline cache and verify the build**

```bash
cargo sqlx prepare --workspace -- --tests
git status --short .sqlx
SQLX_OFFLINE=true cargo check --workspace --all-targets
```

Expected: additive-only `.sqlx` diff; clean check.

- [ ] **Step 7: Commit**

```bash
git add crates/fubbik-api/src/graph/sync.rs
git commit -m "feat(graph): schedule behavior-rule projection into AGE" -- crates/fubbik-api/src/graph/sync.rs crates/fubbik-api/src/graph/mod.rs crates/fubbik/src/main.rs crates/fubbik-api/tests/graph.rs .sqlx
```

---

## Task 6: Document the new environment variable

**Files:**
- Modify: `CLAUDE.md` (the "Environment Variables" section)

**Interfaces:**
- Consumes: the variable name from Task 5.
- Produces: nothing code-level.

Folded in here rather than left to the end because a variable nobody documented is a variable nobody sets.

- [ ] **Step 1: Add the entry**

In `CLAUDE.md`, under "## Environment Variables", after the `STALENESS_SCAN_INTERVAL_HOURS` line:

```markdown
- `BEHAVIOR_GRAPH_SYNC_INTERVAL_HOURS` — Hours between behavior-rule projections into the AGE graph (default: `24`, set to `0` to disable). Rust backend only.
```

- [ ] **Step 2: Commit**

```bash
git commit -m "docs: record BEHAVIOR_GRAPH_SYNC_INTERVAL_HOURS" -- CLAUDE.md
```

---

## Task 7: Migrate the web call sites

**Files:**
- Modify: `apps/web/src/features/graph/use-graph-data.ts:27`, `apps/web/src/features/graph/saved-graph-view.tsx:150-151`, `apps/web/src/features/search/search-graph.tsx:75`, `apps/web/src/routes/browse.tsx:24`
- Modify: `apps/web/src/utils/api-types.ts` (regenerated, not hand-edited)

**Interfaces:**
- Consumes: `openapi.json` as regenerated in Task 4.
- Produces: four call sites typed from Rust's schema.

Migrating the call sites is what *verifies* the port — request and response shapes become compile-time checked. A ported domain left on `legacyApi` throws that away, which is how three real bugs hid behind the requirements port.

- [ ] **Step 1: Regenerate the types**

```bash
cd apps/web && pnpm gen:api
git diff --stat src/utils/api-types.ts
```

Expected: additions covering `/api/graph`.

- [ ] **Step 2: Move the four call sites**

In each file, change `legacyApi` to `api` and delete the accompanying "no Rust route yet" comment. `use-graph-data.ts:27` becomes:

```ts
            return unwrapEden(
                await api.api.graph.get({
                    query: {
                        ...(workspaceId ? { workspaceId } : {}),
                        ...(spaceId && spaceId !== "global" && !workspaceId ? { spaceId } : {})
                    }
                })
            );
```

The other three pass `{ query: {} }`. Update each file's import: drop `legacyApi` from the `@/utils/api` import if nothing else in the file uses it, and add `api`.

- [ ] **Step 3: Remove any casts the move makes unnecessary**

Search the four files for `as any` / `as {` around these calls and delete them. **If removing a cast produces a type error, that is a finding, not an obstacle** — it means Rust's DTO and the web's expectation disagree, and one of them is wrong. Report it rather than re-adding the cast.

- [ ] **Step 4: Type-check**

Run: `cd apps/web && pnpm exec tsgo -p tsconfig.json --noEmit`
Expected: 0 errors — the same baseline recorded in Task 0.

- [ ] **Step 5: Commit**

```bash
git commit -m "refactor(web): migrate graph call sites; regen types" -- apps/web/src/features/graph/use-graph-data.ts apps/web/src/features/graph/saved-graph-view.tsx apps/web/src/features/search/search-graph.tsx apps/web/src/routes/browse.tsx apps/web/src/utils/api-types.ts
```

---

## Task 8: Fix Node's scoping bug and correct two stale comments

**Files:**
- Modify: `packages/api/src/graph/routes.ts` (six occurrences)
- Modify: `crates/fubbik-db/migrations/0004_behavior_matrix_completion.sql:19-23` (comment only)
- Modify: `apps/web/src/utils/api.ts` (the migration doc block)

**Interfaces:** none — this task changes no runtime behaviour in Rust.

No server needs to run for any of this.

- [ ] **Step 1: Rename the query param in Node**

In `packages/api/src/graph/routes.ts`, replace every `ctx.query.codebaseId` with `ctx.query.spaceId` and every `t.Object({ codebaseId: t.Optional(t.String()), workspaceId: ... })` with `t.Object({ spaceId: t.Optional(t.String()), workspaceId: ... })`. Six routes, twelve edits.

The service parameter is still named `codebaseId` in `graph/service.ts:13` and `packages/db/src/repository/graph.ts:9` — **leave those alone.** They are internal names, Phase 6 deletes the package, and widening this edit risks the rest of the file for no gain.

- [ ] **Step 2: Verify Node still type-checks**

Run: `pnpm run check-types`
Expected: no new errors.

- [ ] **Step 3: Correct the migration comment**

In `crates/fubbik-db/migrations/0004_behavior_matrix_completion.sql`, replace lines 19-23 with:

```sql
-- `graph_event`, `usage_event`, `account` and `verification` are absent from
-- these migrations and are deliberately left that way. `account` and
-- `verification` belong to better-auth, which Rust does not use (it has its
-- own `user`/`session` tables and its own argon2 password path).
--
-- The other two need a correction to what this comment said before Phase 4a.
-- `usage_event` was described as having "no reader"; it has a reader
-- (`packages/api/src/usage/service.ts:28`) but no *writer* — the only insert
-- path is `events/handlers.ts:17`, on a `CHUNK_VIEWED` event that nothing in
-- `packages/api/src` ever emits. It is dead from the other end. `graph_event`
-- does have a live reader (`packages/api/src/graph/timeline-service.ts`, for
-- `/graph/at` and `/graph/events`); those two routes have no caller in any
-- client and are out of scope for the Rust port, which is why the table stays
-- absent. See `docs/superpowers/specs/2026-08-26-rust-phase-4a-design.md`.
```

**This edits only `--` comment lines.** Confirm no SQL statement changed:

Run: `git diff crates/fubbik-db/migrations/0004_behavior_matrix_completion.sql | grep '^[+-]' | grep -v '^[+-][+-]' | grep -v '^\s*[+-]\s*--'`
Expected: **no output.** Any line printed is a SQL change and must be reverted — the file is an applied migration.

- [ ] **Step 4: Correct the web migration doc block**

In `apps/web/src/utils/api.ts`, the comment block currently lists ten unported domains. Six of them shipped in earlier slices and are wrong. Replace the list with:

```
//   ai, context, graph (until this slice lands)
```

and remove `graph` once Task 7 is committed — leaving only `ai` and `context`. Also fix the `PATCH /api/chunks/{id}` paragraph, which claims `UpdateChunkBody` accepts "only title/content/type/rationale/consequences": it grew to seventeen fields in `e2b2129`, and that whole caveat no longer applies.

Verify the corrected list against reality rather than trusting this plan:

```bash
node -e "const d=require('./openapi.json');console.log([...new Set(Object.keys(d.paths).map(p=>p.split('/')[2]))].sort().join(' '))"
grep -rn "legacyApi\." apps/web/src | grep -v utils/api.ts
```

The second command's output IS the list. Write what it shows.

- [ ] **Step 5: Commit**

```bash
git commit -m "fix(graph): accept spaceId on Node; correct two stale comments" -- packages/api/src/graph/routes.ts crates/fubbik-db/migrations/0004_behavior_matrix_completion.sql apps/web/src/utils/api.ts
```

---

## Task 9: Full verification

**Files:** none.

- [ ] **Step 1: The Rust gates**

```bash
export DATABASE_URL="postgres://postgres:password@localhost:5434/fubbik_rs"
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --check
SQLX_OFFLINE=true cargo check --workspace --all-targets
```

Expected: pass count = Task 0's baseline + 13 new tests (1 + 4 + 6 in db, 3 + 1 in api, 3 unit — recount rather than trusting this arithmetic). Zero clippy warnings.

- [ ] **Step 2: Confirm the AGE tests actually ran**

Run: `cargo test --workspace 2>&1 | grep -c "AGE unavailable"`
Expected: `0`. Any non-zero count means part of this slice was never exercised, and the report must say so.

- [ ] **Step 3: The TypeScript gates**

```bash
cd apps/web && pnpm exec tsgo -p tsconfig.json --noEmit
cd ../.. && pnpm run check-types
```

- [ ] **Step 4: Confirm the migration directory is otherwise untouched**

Run: `git diff main --stat -- crates/fubbik-db/migrations/`
Expected: only `0004_behavior_matrix_completion.sql`, comment lines only.

- [ ] **Step 5: Confirm `.sqlx` is additive**

Run: `git diff main --stat -- .sqlx | tail -1` and `git diff main --diff-filter=D --name-only -- .sqlx`
Expected: the second prints nothing.

- [ ] **Step 6: Report**

State: the pass/fail/ignored counts before and after; whether AGE was available; which of the four web call sites needed a cast removed and what it revealed; and anything in the plan that turned out to be wrong. That last one is the most valuable line in the report.

---

## Exit Criteria

- [ ] `cargo test --workspace` green, with **zero** `AGE unavailable` skips.
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` clean; `cargo fmt --check` clean.
- [ ] `SQLX_OFFLINE=true cargo check --workspace --all-targets` clean.
- [ ] `apps/web` type check at 0 errors; `pnpm run check-types` no new errors.
- [ ] `GET /api/graph` serves exactly seven fields; the six dropped ones are absent, asserted.
- [ ] `spaceId` scoping proven by a test that was **observed failing** before the route existed.
- [ ] A rule title containing `"` and `'` round-trips through AGE intact.
- [ ] Behavior sync is idempotent across two sweeps and covers more than one user.
- [ ] `crates/fubbik-db/migrations/` differs from base in comments only.
- [ ] `.sqlx` diff additive-only.
- [ ] No `legacyApi` reference to `api.graph` remains in `apps/web/src`.
