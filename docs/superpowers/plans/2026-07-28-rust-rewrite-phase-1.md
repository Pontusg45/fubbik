# Rust Rewrite Phase 1 — Walking Skeleton Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a complete but thin Rust implementation of fubbik — one binary serving the API, the embedded SPA, and a handful of CLI commands — with the chunks domain ported end-to-end and proven equivalent to the Node backend.

**Architecture:** A cargo workspace of seven crates producing a single `fubbik` binary. axum handles HTTP, sqlx owns the schema through SQL migrations, and `thiserror` replaces the Effect error stack. The Rust server runs on port 3100 against its own `fubbik_rs` database, so the existing Node stack keeps working untouched throughout. Type safety reaches the TypeScript web app through utoipa-generated OpenAPI plus a Proxy-based client that preserves Eden's existing call shape.

**Tech Stack:** Rust, axum, sqlx (Postgres), thiserror, utoipa, rust-embed, clap v4, argon2, tracing, reqwest.

**Spec:** `docs/superpowers/specs/2026-07-28-rust-backend-cli-rewrite-design.md`

## Global Constraints

- **Rust edition 2024.** Pin the toolchain in `rust-toolchain.toml`.
- **axum 0.8 path syntax is `/{id}`, not `/:id`.** The older colon syntax panics at router build time. Every route path in this plan uses braces.
- **The `.sqlx` offline cache is committed.** Regenerate with `cargo sqlx prepare --workspace` whenever a query changes. CI builds must not require a live database.
- **`openapi.json` is committed** at repo root as `openapi.json`. CI fails if regeneration produces a diff.
- **The Rust server listens on port 3100.** Node server stays on 3000, web on 3001. Never reuse 3000 in this phase.
- **The Rust database is `fubbik_rs`,** running in Docker with Apache AGE available:
  `DATABASE_URL=postgres://postgres:password@localhost:5434/fubbik_rs`.
  The container is already built and running as `fubbik-rs-db` from the repo's own
  `fubbik-postgres:pg18-vector-age` image (AGE 1.7.0, pgvector 0.8.2, pg_trgm 1.6).
  Never point the Rust binary at the Node database (`postgresql://pontus@localhost:5432/fubbik`),
  which is Homebrew Postgres and has **no AGE**.
- **Extracting `agtype` from AGE uses `::varchar`, never `::text`.** Verified against
  AGE 1.7.0: `v::text` raises `agtype_value_to_text: unsupported argument agtype 6`
  for vertex, edge, and path values, and `agtype_out(v)` returns pseudo-type `cstring`,
  which sqlx cannot decode and Postgres cannot materialise.
- **The CLI is an HTTP client.** `fubbik-cli` must not depend on `fubbik-db`. Only `init`, `hooks`, and `doctor` may work offline.
- **No better-auth compatibility.** Sessions and password hashes are new. Do not attempt to read existing `account` rows.
- **`#[sqlx::test]` inside `fubbik-api` must point at the db crate's migrations:**
  `#[sqlx::test(migrations = "../fubbik-db/migrations")]`. The macro looks for a
  `migrations/` directory local to the crate, and `fubbik-api` has none — without the
  attribute the test database is provisioned empty and every request fails with a 500
  that looks like a routing bug. Applies to Tasks 10, 11, 12 and 14 as well as Task 7.
- **Every task ends on a green `cargo test` and a commit.**

## File Structure

```
rust-toolchain.toml            toolchain pin
Cargo.toml                     workspace manifest
openapi.json                   generated, committed, CI-diffed
crates/
  fubbik-core/
    src/lib.rs                 re-exports
    src/error.rs               AppError enum + IntoResponse
    src/id.rs                  ID generation matching the TS format
  fubbik-db/
    migrations/0001_init.sql   full schema, derived from pg_dump
    src/lib.rs                 pool construction, AGE after_connect hook
    src/age.rs                 cypher() helper + agtype parsing
    src/repo/chunk.rs          chunk CRUD + list filtering
    src/repo/chunk_version.rs  append-only version history
    src/repo/chunk_meta.rs     applies_to + file_ref sub-resources
    src/repo/user.rs           user lookup + creation
    src/repo/session.rs        session create/lookup/delete
  fubbik-api/
    src/lib.rs                 router assembly, AppState
    src/auth/mod.rs            CurrentUser extractor, dev-session fallback
    src/auth/password.rs       argon2 hash/verify
    src/auth/routes.rs         signup, login, logout, get-session
    src/chunks/mod.rs          chunk domain module
    src/chunks/routes.rs       axum handlers + utoipa annotations
    src/chunks/service.rs      business logic over repositories
    src/chunks/dto.rs          request/response types
    src/openapi.rs             OpenApi derive + JSON emit
    src/assets.rs              rust-embed SPA serving + fallback
    tests/differential.rs      Node-vs-Rust response diffing
  fubbik-cli/
    src/lib.rs                 clap Cli enum
    src/client.rs              reqwest HTTP client
    src/commands/{add,get,list,search,health}.rs
  fubbik/
    src/main.rs                binary entrypoint: serve | mcp | commands
```

`fubbik-ai` and `fubbik-mcp` from the spec's seven-crate layout are **not** created in Phase 1 — nothing here needs them. They arrive in Phases 4 and 5 respectively. Phase 1 builds five crates.

```
apps/web/src/utils/api.ts      replaced: Proxy client over generated types
apps/web/src/utils/api-types.ts  generated by openapi-typescript
apps/web/src/lib/auth-client.ts  replaced: plain fetch
```

---

### Task 1: Cargo workspace and binary skeleton

**Files:**
- Create: `rust-toolchain.toml`, `Cargo.toml`, `crates/fubbik/Cargo.toml`, `crates/fubbik/src/main.rs`
- Create: `crates/fubbik-core/Cargo.toml`, `crates/fubbik-core/src/lib.rs`
- Create: `.gitignore` additions for `target/`

**Interfaces:**
- Produces: a `fubbik` binary with subcommands `serve` and `mcp`; workspace members `fubbik-core`, `fubbik`.

- [ ] **Step 1: Create the workspace manifest**

`Cargo.toml`:

```toml
[workspace]
resolver = "3"
members = ["crates/*"]

[workspace.package]
edition = "2024"
version = "0.1.0"

[workspace.dependencies]
anyhow = "1"
axum = "0.8"
clap = { version = "4", features = ["derive", "env"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
sqlx = { version = "0.8", features = ["runtime-tokio", "postgres", "chrono", "uuid", "json", "macros"] }
thiserror = "2"
tokio = { version = "1", features = ["full"] }
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
```

`rust-toolchain.toml`:

```toml
[toolchain]
channel = "stable"
components = ["rustfmt", "clippy"]
```

- [ ] **Step 2: Create `fubbik-core` as an empty library**

`crates/fubbik-core/Cargo.toml`:

```toml
[package]
name = "fubbik-core"
edition.workspace = true
version.workspace = true

[dependencies]
thiserror.workspace = true
```

`crates/fubbik-core/src/lib.rs`:

```rust
pub mod error;
```

Create `crates/fubbik-core/src/error.rs` as an empty file for now; Task 4 fills it.

- [ ] **Step 3: Write the failing CLI test**

`crates/fubbik/tests/cli.rs`:

```rust
use std::process::Command;

#[test]
fn help_lists_serve_and_mcp() {
    let out = Command::new(env!("CARGO_BIN_EXE_fubbik"))
        .arg("--help")
        .output()
        .expect("binary runs");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("serve"), "missing serve subcommand:\n{stdout}");
    assert!(stdout.contains("mcp"), "missing mcp subcommand:\n{stdout}");
}
```

- [ ] **Step 4: Run the test to verify it fails**

Run: `cargo test -p fubbik`
Expected: FAIL — the `fubbik` package does not exist yet.

- [ ] **Step 5: Implement the binary**

`crates/fubbik/Cargo.toml`:

```toml
[package]
name = "fubbik"
edition.workspace = true
version.workspace = true

[[bin]]
name = "fubbik"
path = "src/main.rs"

[dependencies]
clap.workspace = true
tokio.workspace = true
tracing-subscriber.workspace = true
fubbik-core = { path = "../fubbik-core" }
```

`crates/fubbik/src/main.rs`:

```rust
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "fubbik", version, about = "Local-first knowledge framework")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run the API server and web UI
    Serve {
        #[arg(long, env = "PORT", default_value = "3100")]
        port: u16,
    },
    /// Run the MCP server over stdio
    Mcp,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    match Cli::parse().command {
        Commands::Serve { port } => {
            println!("serve on {port} — not yet implemented");
            Ok(())
        }
        Commands::Mcp => {
            println!("mcp — not yet implemented");
            Ok(())
        }
    }
}
```

Add `anyhow.workspace = true` to the `fubbik` dependencies.

- [ ] **Step 6: Run the test to verify it passes**

Run: `cargo test -p fubbik`
Expected: PASS

- [ ] **Step 7: Commit**

```bash
git add Cargo.toml rust-toolchain.toml crates/ .gitignore
git commit -m "feat(rust): cargo workspace and fubbik binary skeleton"
```

---

### Task 2: Database schema and connection pool

**Files:**
- Create: `crates/fubbik-db/Cargo.toml`, `crates/fubbik-db/src/lib.rs`
- Create: `crates/fubbik-db/migrations/0001_init.sql`
- Modify: `Cargo.toml` (workspace deps)

**Interfaces:**
- Consumes: workspace from Task 1.
- Produces: `fubbik_db::connect(database_url: &str) -> Result<PgPool, sqlx::Error>`, which runs migrations and installs the AGE `after_connect` hook.

- [ ] **Step 1: Generate the base schema from the running Node database**

The `fubbik_rs` database already exists in the running `fubbik-rs-db` container. Dump the schema from the **Node** database, which is the reference:

```bash
pg_dump --schema-only --no-owner --no-privileges \
  "postgresql://pontus@localhost:5432/fubbik" \
  > crates/fubbik-db/migrations/0001_init.sql
```

Then hand-edit `0001_init.sql`:
- Delete the `account` and `verification` table definitions and their indexes. Auth is a clean slate.
- Add `password_hash text` to the `user` table.
- Remove any `SET` statements referencing `pg_dump` internals (`SET idle_in_transaction_session_timeout`, `SET default_table_access_method`, etc.).
- Replace any `CREATE EXTENSION` lines with the block below, placed at the very top of the file.

### Reference data is NOT in the schema dump

`pg_dump --schema-only` carries table definitions but no rows, and three tables in this
schema are enum-like reference tables that FK constraints point at. Without their rows,
inserting a chunk fails on `chunk_type_id_fk`, a connection fails on
`connection_relation_id_fk`, and a space fails on `space_kind_id_fk`.

Add `crates/fubbik-db/migrations/0002_seed_reference_data.sql`. Do **not** hand-write the
values — the TypeScript app already has canonical seed migrations for exactly this, and
copying them keeps both stacks on identical vocabulary:

- `packages/db/src/migrations/0001_seed_builtin_catalogs.sql` — `chunk_type` (7 rows) and
  `connection_relation` (13 rows), including the trailing `UPDATE` statements that wire
  `connection_relation.inverse_of_id`. Those updates must run *after* the INSERT because
  the column is a self-referential FK.
- `packages/db/src/migrations/0004_codebase_to_space.sql` — the `INSERT INTO space_kind`
  block (4 rows).

These tables are NOT id-only. Each carries NOT NULL columns (`label` at minimum, plus
`color`, `examples`, `display_order`, `built_in`, and for `connection_relation` also
`arrow_style` and `direction`). An id-only INSERT fails on `label`. Copy the full column
lists from the source migrations.

`tag_type` is user-managed data, not reference data — it has rows in the live database but
must NOT be seeded here.

Extensions must be created **by the migration**, not by hand: `#[sqlx::test]` provisions a brand-new database per test, and those databases inherit nothing. `vector` and `pg_trgm` are hard requirements. AGE is soft — the graph layer is designed to degrade when it is absent, and a plain Postgres without AGE must still run every non-graph endpoint.

```sql
CREATE EXTENSION IF NOT EXISTS vector;
CREATE EXTENSION IF NOT EXISTS pg_trgm;

-- AGE is optional. Swallow the failure so a Postgres without the extension
-- still migrates cleanly; fubbik_db::age degrades to empty results.
DO $$
BEGIN
    CREATE EXTENSION IF NOT EXISTS age;
    LOAD 'age';
    PERFORM ag_catalog.create_graph('knowledge');
EXCEPTION
    WHEN duplicate_schema THEN NULL;  -- graph already exists
    WHEN OTHERS THEN
        RAISE NOTICE 'AGE unavailable, graph features disabled: %', SQLERRM;
END $$;
```

The full schema ships in migration 0001 even though Phase 1 only implements chunks. Deferring tables would force schema churn in later phases.

- [ ] **Step 2: Write the failing pool test**

`crates/fubbik-db/tests/pool.rs`:

```rust
#[sqlx::test]
async fn migrations_create_chunk_table(pool: sqlx::PgPool) {
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM chunk")
        .fetch_one(&pool)
        .await
        .expect("chunk table exists");
    assert_eq!(count, 0);
}
```

`#[sqlx::test]` applies `migrations/` to a fresh throwaway database per test and rolls it back afterwards.

- [ ] **Step 3: Run the test to verify it fails**

Run: `cargo test -p fubbik-db`
Expected: FAIL — package does not exist.

- [ ] **Step 4: Implement the crate and pool**

`crates/fubbik-db/Cargo.toml`:

```toml
[package]
name = "fubbik-db"
edition.workspace = true
version.workspace = true

[dependencies]
chrono = { version = "0.4", features = ["serde"] }
serde.workspace = true
serde_json.workspace = true
sqlx.workspace = true
thiserror.workspace = true
tracing.workspace = true
fubbik-core = { path = "../fubbik-core" }

[dev-dependencies]
tokio.workspace = true
```

`crates/fubbik-db/src/lib.rs`:

```rust
pub mod age;
pub mod repo;

use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::{Executor, PgPool};
use std::str::FromStr;

/// Connects, installs the AGE per-connection setup, and runs migrations.
pub async fn connect(database_url: &str) -> Result<PgPool, sqlx::Error> {
    let opts = PgConnectOptions::from_str(database_url)?;

    let pool = PgPoolOptions::new()
        .max_connections(10)
        .after_connect(|conn, _meta| {
            Box::pin(async move {
                // AGE is optional. A database without it must still serve
                // every non-graph endpoint, so failures here are logged
                // and swallowed rather than failing the connection.
                if let Err(e) = conn.execute("LOAD 'age';").await {
                    tracing::debug!("AGE not available: {e}");
                    return Ok(());
                }
                conn.execute(r#"SET search_path = ag_catalog, "$user", public;"#)
                    .await?;
                Ok(())
            })
        })
        .connect_with(opts)
        .await?;

    sqlx::migrate!("./migrations").run(&pool).await?;
    Ok(pool)
}
```

Create `crates/fubbik-db/src/age.rs` and `crates/fubbik-db/src/repo/mod.rs` as empty files; Tasks 3 and 8 fill them.

- [ ] **Step 5: Run the test to verify it passes**

Run: `DATABASE_URL=postgres://postgres:password@localhost:5434/fubbik_rs cargo test -p fubbik-db`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add crates/fubbik-db Cargo.toml
git commit -m "feat(rust): db crate with schema migration and pool"
```

---

### Task 3: AGE spike — cypher round-trip and agtype parsing

This is the sharpest unknown in the whole rewrite and is deliberately front-loaded. Rust has no typed AGE driver, so `agtype` values come back as an opaque Postgres type that sqlx cannot decode. The workaround is casting to `text` in SQL and parsing the result as JSON.

**Files:**
- Modify: `crates/fubbik-db/src/age.rs`
- Create: `crates/fubbik-db/tests/age.rs`

**Interfaces:**
- Consumes: `fubbik_db::connect` from Task 2.
- Produces:
  - `fubbik_db::age::is_available(pool: &PgPool) -> bool`
  - `fubbik_db::age::esc_cypher(value: &str) -> String`
  - `fubbik_db::age::cypher(pool: &PgPool, query: &str) -> Result<Vec<serde_json::Value>, sqlx::Error>`

- [ ] **Step 1: Write the failing escaping test**

`crates/fubbik-db/tests/age.rs`:

```rust
use fubbik_db::age;

#[test]
fn esc_cypher_escapes_backslashes_before_quotes() {
    assert_eq!(age::esc_cypher(r"a\b"), r"a\\b");
    assert_eq!(age::esc_cypher("it's"), r"it\'s");
    // Order matters: escaping quotes first would double-escape the backslash.
    assert_eq!(age::esc_cypher(r"\'"), r"\\\'");
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p fubbik-db --test age`
Expected: FAIL — `age::esc_cypher` not found.

- [ ] **Step 3: Implement escaping and the cypher helper**

`crates/fubbik-db/src/age.rs`:

```rust
use sqlx::{PgPool, Row};

/// Escapes a value for use inside a Cypher single-quoted literal.
/// Backslashes must be escaped before quotes or the quote's escape
/// character gets doubled. Mirrors `escCypher` in the TS implementation.
pub fn esc_cypher(value: &str) -> String {
    value.replace('\\', r"\\").replace('\'', r"\'")
}

/// Reports whether the AGE extension is installed and the catalog readable.
pub async fn is_available(pool: &PgPool) -> bool {
    sqlx::query("SELECT 1 FROM ag_catalog.ag_graph LIMIT 0")
        .execute(pool)
        .await
        .is_ok()
}

/// Runs a Cypher query against the `knowledge` graph and returns each row
/// as JSON. Returns an empty vec when AGE is unavailable, matching the TS
/// behaviour of degrading rather than failing.
///
/// The `::text` cast is essential: sqlx has no decoder for `agtype`, so the
/// value must be stringified by Postgres before it crosses the wire.
pub async fn cypher(pool: &PgPool, query: &str) -> Result<Vec<serde_json::Value>, sqlx::Error> {
    if !is_available(pool).await {
        return Ok(Vec::new());
    }

    // `::varchar`, NOT `::text`. Verified against AGE 1.7.0: the explicit
    // text cast routes through agtype_value_to_text, which rejects vertex,
    // edge, and path values with "unsupported argument agtype 6". The
    // varchar coercion uses the type's output representation and handles
    // every shape. `agtype_out(v)` also produces the right string but
    // returns pseudo-type cstring, which sqlx cannot decode.
    let sql = format!(
        "SELECT v::varchar AS v FROM cypher('knowledge', $$ {query} $$) AS (v agtype)"
    );

    let rows = sqlx::query(&sql).fetch_all(pool).await?;

    Ok(rows
        .into_iter()
        .filter_map(|row| {
            let raw: String = row.try_get("v").ok()?;
            parse_agtype(&raw)
        })
        .collect())
}

/// Parses an agtype text representation into JSON.
///
/// Verified shapes from AGE 1.7.0:
///   vertex: {"id": 1125899906842625, "label": "chunk", "properties": {...}}::vertex
///   scalars: 42 | 1.5 | "plain string"   (no suffix)
///
/// Composite results nest their suffixes, so stripping only a trailing one is
/// not enough. A path comes back as:
///   [{...}::vertex, {...}::edge, {...}::vertex]::path
/// Removing just the outer `::path` leaves inner suffixes that are not valid
/// JSON, and the row is then silently dropped.
///
/// Every `::identifier` outside a JSON string literal is stripped. Tracking
/// string state is what keeps property values containing `::`
/// (e.g. {"code": "a::b"}) intact.
fn parse_agtype(raw: &str) -> Option<serde_json::Value> {
    let mut out = String::with_capacity(raw.len());
    let bytes = raw.as_bytes();
    let mut i = 0;
    let mut in_string = false;

    while i < bytes.len() {
        let c = bytes[i];

        if in_string {
            // Copy escape pairs wholesale so a escaped quote cannot end the string.
            if c == b'\\' && i + 1 < bytes.len() {
                out.push_str(&raw[i..i + 2]);
                i += 2;
                continue;
            }
            if c == b'"' {
                in_string = false;
            }
            out.push(c as char);
            i += 1;
            continue;
        }

        if c == b'"' {
            in_string = true;
            out.push('"');
            i += 1;
            continue;
        }

        // Outside a string: `::ident` is a type tag, never data.
        if c == b':' && i + 1 < bytes.len() && bytes[i + 1] == b':' {
            let mut j = i + 2;
            while j < bytes.len() && bytes[j].is_ascii_lowercase() {
                j += 1;
            }
            if j > i + 2 {
                i = j;
                continue;
            }
        }

        out.push(c as char);
        i += 1;
    }

    serde_json::from_str(out.trim()).ok()
}

#[cfg(test)]
mod tests {
    use super::parse_agtype;

    #[test]
    fn strips_vertex_suffix() {
        // Exact output captured from AGE 1.7.0.
        let raw = r#"{"id": 1125899906842625, "label": "chunk", "properties": {"url": "https://x.test", "title": "hello"}}::vertex"#;
        let v = parse_agtype(raw).unwrap();
        assert_eq!(v["label"], "chunk");
        assert_eq!(v["properties"]["title"], "hello");
        assert_eq!(v["properties"]["url"], "https://x.test");
    }

    #[test]
    fn parses_bare_scalars() {
        assert_eq!(parse_agtype("42").unwrap(), 42);
        assert_eq!(parse_agtype("1.5").unwrap(), 1.5);
        assert_eq!(parse_agtype(r#""plain string""#).unwrap(), "plain string");
    }

    #[test]
    fn preserves_property_values_containing_double_colons() {
        let raw = r#"{"id": 1407374883553281, "label": "probe", "properties": {"code": "a::b"}}::vertex"#;
        let v = parse_agtype(raw).unwrap();
        assert_eq!(v["properties"]["code"], "a::b");
    }
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p fubbik-db`
Expected: PASS

- [ ] **Step 5: Write the integration test proving a real round-trip**

Append to `crates/fubbik-db/tests/age.rs`:

```rust
#[sqlx::test]
async fn cypher_round_trips_a_real_vertex(pool: sqlx::PgPool) {
    // Migration 0001 installs AGE and creates the 'knowledge' graph, so this
    // exercises real agtype output rather than the degradation path.
    if !age::is_available(&pool).await {
        eprintln!("AGE unavailable in this database — skipping round-trip");
        return;
    }

    let created = age::cypher(
        &pool,
        "CREATE (n:chunk {title: 'from rust', code: 'a::b'}) RETURN n",
    )
    .await
    .unwrap();

    assert_eq!(created.len(), 1);
    let v = &created[0];
    assert_eq!(v["label"], "chunk");
    assert_eq!(v["properties"]["title"], "from rust");
    assert_eq!(
        v["properties"]["code"], "a::b",
        "property values containing :: must survive suffix stripping"
    );

    let matched = age::cypher(&pool, "MATCH (n:chunk) RETURN n").await.unwrap();
    assert_eq!(matched.len(), 1);
}

#[sqlx::test]
async fn cypher_returns_scalars(pool: sqlx::PgPool) {
    if !age::is_available(&pool).await {
        return;
    }
    let rows = age::cypher(&pool, "RETURN 42").await.unwrap();
    assert_eq!(rows[0], 42);
}
```

- [ ] **Step 6: Run and verify**

Run: `cargo test -p fubbik-db --test age`
Expected: PASS

- [ ] **Step 7: Record the spike outcome**

Append a short section to the spec at `docs/superpowers/specs/2026-07-28-rust-backend-cli-rewrite-design.md` under Risks, recording what the spike established:

- `v::text` is unusable — AGE raises `agtype_value_to_text: unsupported argument agtype 6` for vertex, edge, and path values.
- `agtype_out(v)` returns pseudo-type `cstring`; sqlx cannot decode it and Postgres cannot materialise it into a table.
- `v::varchar` is the working extraction, verified across vertices and scalars.
- Suffix stripping is bounded to a trailing `::identifier`, so property values containing `::` survive.

State plainly whether your tests confirmed all four points, and note any agtype shape you encountered that the parser mishandles. Phase 4 builds the whole graph layer on this conclusion.

- [ ] **Step 8: Commit**

```bash
git add crates/fubbik-db docs/superpowers/specs/
git commit -m "feat(rust): AGE cypher helper with agtype parsing"
```

---

### Task 4: Error model

**Files:**
- Modify: `crates/fubbik-core/src/error.rs`, `crates/fubbik-core/Cargo.toml`

**Interfaces:**
- Produces: `fubbik_core::error::AppError` with variants `Database`, `NotFound`, `Auth`, `Validation`, `Conflict`, `External`; `impl IntoResponse for AppError`; `pub type AppResult<T> = Result<T, AppError>`.

- [ ] **Step 1: Write the failing status-mapping test**

`crates/fubbik-core/src/error.rs` (test module at the bottom, written first):

```rust
#[cfg(test)]
mod tests {
    use super::AppError;
    use axum::http::StatusCode;
    use axum::response::IntoResponse;

    #[test]
    fn maps_variants_to_status_codes() {
        let cases = [
            (AppError::NotFound("chunk".into()), StatusCode::NOT_FOUND),
            (AppError::Auth, StatusCode::UNAUTHORIZED),
            (AppError::Validation("bad".into()), StatusCode::BAD_REQUEST),
            (AppError::Conflict("dupe".into()), StatusCode::CONFLICT),
        ];
        for (err, expected) in cases {
            assert_eq!(err.into_response().status(), expected);
        }
    }
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p fubbik-core`
Expected: FAIL — `AppError` not defined.

- [ ] **Step 3: Implement `AppError`**

Add to `crates/fubbik-core/Cargo.toml`:

```toml
axum = { workspace = true }
serde_json.workspace = true
sqlx.workspace = true
```

Prepend to `crates/fubbik-core/src/error.rs`:

```rust
use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

pub type AppResult<T> = Result<T, AppError>;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("database error: {0}")]
    Database(sqlx::Error),

    #[error("{0} not found")]
    NotFound(String),

    #[error("unauthorized")]
    Auth,

    #[error("validation failed: {0}")]
    Validation(String),

    #[error("conflict: {0}")]
    Conflict(String),

    #[error("external service error: {0}")]
    External(String),
}

/// Hand-written rather than `#[from]`, so a missing row becomes a 404 instead
/// of a 500. `fetch_one()` on an absent row yields `RowNotFound`; the derived
/// conversion would fold that into `Database` and report a server error for
/// what is really a client-visible absence. Phase 2 adds 48 route domains
/// where that mistake would otherwise be easy to make repeatedly.
impl From<sqlx::Error> for AppError {
    fn from(err: sqlx::Error) -> Self {
        match err {
            // The conversion cannot know the entity type, hence the generic name.
            // Repositories that can name it should map absence explicitly instead.
            sqlx::Error::RowNotFound => AppError::NotFound("resource".into()),
            other => AppError::Database(other),
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = match &self {
            AppError::Database(_) => StatusCode::INTERNAL_SERVER_ERROR,
            AppError::NotFound(_) => StatusCode::NOT_FOUND,
            AppError::Auth => StatusCode::UNAUTHORIZED,
            AppError::Validation(_) => StatusCode::BAD_REQUEST,
            AppError::Conflict(_) => StatusCode::CONFLICT,
            AppError::External(_) => StatusCode::BAD_GATEWAY,
        };

        // Internal errors are logged in full but never leak detail to the
        // client. Everything else is safe to surface verbatim.
        let message = match &self {
            AppError::Database(e) => {
                tracing::error!("database error: {e:?}");
                "Internal server error".to_string()
            }
            other => other.to_string(),
        };

        (status, Json(serde_json::json!({ "message": message }))).into_response()
    }
}
```

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p fubbik-core`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/fubbik-core
git commit -m "feat(rust): AppError with HTTP status mapping"
```

---

### Task 5: Password hashing

**Files:**
- Create: `crates/fubbik-api/Cargo.toml`, `crates/fubbik-api/src/lib.rs`, `crates/fubbik-api/src/auth/mod.rs`, `crates/fubbik-api/src/auth/password.rs`

**Interfaces:**
- Consumes: `AppError` from Task 4.
- Produces: `hash_password(plain: &str) -> AppResult<String>`, `verify_password(plain: &str, hash: &str) -> bool`.

- [ ] **Step 1: Write the failing round-trip test**

`crates/fubbik-api/src/auth/password.rs` (tests first):

```rust
#[cfg(test)]
mod tests {
    use super::{hash_password, verify_password};

    #[test]
    fn verifies_correct_password() {
        let hash = hash_password("correct horse").unwrap();
        assert!(verify_password("correct horse", &hash));
    }

    #[test]
    fn rejects_wrong_password() {
        let hash = hash_password("correct horse").unwrap();
        assert!(!verify_password("wrong horse", &hash));
    }

    #[test]
    fn salts_differ_across_hashes() {
        assert_ne!(hash_password("same").unwrap(), hash_password("same").unwrap());
    }

    #[test]
    fn rejects_malformed_hash_without_panicking() {
        assert!(!verify_password("anything", "not-a-phc-string"));
    }
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p fubbik-api`
Expected: FAIL — package does not exist.

- [ ] **Step 3: Create the crate and implement hashing**

`crates/fubbik-api/Cargo.toml`:

```toml
[package]
name = "fubbik-api"
edition.workspace = true
version.workspace = true

[dependencies]
argon2 = "0.5"
axum = { workspace = true, features = ["macros"] }
axum-extra = { version = "0.10", features = ["cookie"] }
chrono = { version = "0.4", features = ["serde"] }
rand = "0.8"
serde.workspace = true
serde_json.workspace = true
sqlx.workspace = true
thiserror.workspace = true
tokio.workspace = true
tower-http = { version = "0.6", features = ["cors", "trace"] }
tracing.workspace = true
utoipa = { version = "5", features = ["axum_extras", "chrono"] }
fubbik-core = { path = "../fubbik-core" }
fubbik-db = { path = "../fubbik-db" }
```

`crates/fubbik-api/src/auth/password.rs`, prepended above the tests:

```rust
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString, rand_core::OsRng};
use argon2::Argon2;
use fubbik_core::error::{AppError, AppResult};

pub fn hash_password(plain: &str) -> AppResult<String> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(plain.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| AppError::External(format!("password hashing failed: {e}")))
}

/// Returns false for malformed hashes rather than erroring, so a corrupt
/// stored hash reads as a failed login instead of a 500.
pub fn verify_password(plain: &str, hash: &str) -> bool {
    let Ok(parsed) = PasswordHash::new(hash) else {
        return false;
    };
    Argon2::default()
        .verify_password(plain.as_bytes(), &parsed)
        .is_ok()
}
```

`crates/fubbik-api/src/auth/mod.rs`:

```rust
pub mod password;
```

`crates/fubbik-api/src/lib.rs`:

```rust
pub mod auth;
```

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p fubbik-api`
Expected: PASS — all four tests.

- [ ] **Step 5: Commit**

```bash
git add crates/fubbik-api
git commit -m "feat(rust): argon2 password hashing"
```

---

### Task 6: Session store and CurrentUser extractor

**Files:**
- Create: `crates/fubbik-db/src/repo/user.rs`, `crates/fubbik-db/src/repo/session.rs`
- Modify: `crates/fubbik-db/src/repo/mod.rs`
- Create: `crates/fubbik-api/src/auth/session.rs`
- Modify: `crates/fubbik-api/src/auth/mod.rs`, `crates/fubbik-api/src/lib.rs`

**Interfaces:**
- Consumes: `connect` (Task 2), `AppError` (Task 4).
- Produces:
  - `fubbik_db::repo::user::{User, find_by_email, find_by_id, create}`
  - `fubbik_db::repo::session::{create, find_valid, delete}`
  - `fubbik_api::AppState { pool: PgPool, implicit_dev_session: bool }`
  - `fubbik_api::auth::CurrentUser(pub User)` — an axum extractor rejecting with `AppError::Auth`.

- [ ] **Step 1: Write the failing repository test**

`crates/fubbik-db/tests/session.rs`:

```rust
use fubbik_db::repo::{session, user};

#[sqlx::test]
async fn session_round_trips_and_expires(pool: sqlx::PgPool) {
    let u = user::create(&pool, "a@b.test", "Alice", Some("hash"))
        .await
        .unwrap();

    let token = session::create(&pool, &u.id, chrono::Duration::days(7))
        .await
        .unwrap();

    let found = session::find_valid(&pool, &token).await.unwrap();
    assert_eq!(found.unwrap().id, u.id);

    let expired = session::create(&pool, &u.id, chrono::Duration::seconds(-1))
        .await
        .unwrap();
    assert!(session::find_valid(&pool, &expired).await.unwrap().is_none());
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p fubbik-db --test session`
Expected: FAIL — `repo::user` not found.

- [ ] **Step 3: Implement the user repository**

`crates/fubbik-db/src/repo/user.rs`:

```rust
use fubbik_core::error::AppResult;
use sqlx::PgPool;

#[derive(Debug, Clone, serde::Serialize)]
pub struct User {
    pub id: String,
    pub email: String,
    pub name: String,
    #[serde(skip)]
    pub password_hash: Option<String>,
}

pub async fn create(pool: &PgPool, email: &str, name: &str, password_hash: Option<&str>) -> AppResult<User> {
    let id = crate::new_id();
    let user = sqlx::query_as!(
        User,
        r#"INSERT INTO "user" (id, email, name, password_hash, email_verified)
           VALUES ($1, $2, $3, $4, true)
           RETURNING id, email, name, password_hash"#,
        id,
        email,
        name,
        password_hash
    )
    .fetch_one(pool)
    .await?;
    Ok(user)
}

pub async fn find_by_email(pool: &PgPool, email: &str) -> AppResult<Option<User>> {
    let user = sqlx::query_as!(
        User,
        r#"SELECT id, email, name, password_hash FROM "user" WHERE email = $1"#,
        email
    )
    .fetch_optional(pool)
    .await?;
    Ok(user)
}

pub async fn find_by_id(pool: &PgPool, id: &str) -> AppResult<Option<User>> {
    let user = sqlx::query_as!(
        User,
        r#"SELECT id, email, name, password_hash FROM "user" WHERE id = $1"#,
        id
    )
    .fetch_optional(pool)
    .await?;
    Ok(user)
}
```

Add ID generation to `crates/fubbik-db/src/lib.rs`:

```rust
/// Generates a 24-character lowercase alphanumeric ID, matching the format
/// the TS implementation stores in `text` primary keys.
pub fn new_id() -> String {
    use rand::Rng;
    const ALPHABET: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789";
    let mut rng = rand::thread_rng();
    (0..24)
        .map(|_| ALPHABET[rng.gen_range(0..ALPHABET.len())] as char)
        .collect()
}
```

Add `rand = "0.8"` to `fubbik-db` dependencies.

- [ ] **Step 4: Implement the session repository**

`crates/fubbik-db/src/repo/session.rs`:

```rust
use chrono::{Duration, Utc};
use fubbik_core::error::AppResult;
use sqlx::PgPool;

use super::user::User;

/// Creates a session and returns its opaque token.
pub async fn create(pool: &PgPool, user_id: &str, ttl: Duration) -> AppResult<String> {
    let id = crate::new_id();
    let token = format!("{}{}", crate::new_id(), crate::new_id());
    let expires_at = Utc::now() + ttl;

    sqlx::query!(
        r#"INSERT INTO session (id, token, user_id, expires_at, created_at, updated_at)
           VALUES ($1, $2, $3, $4, now(), now())"#,
        id,
        token,
        user_id,
        expires_at.naive_utc()
    )
    .execute(pool)
    .await?;

    Ok(token)
}

/// Returns the owning user if the token exists and has not expired.
pub async fn find_valid(pool: &PgPool, token: &str) -> AppResult<Option<User>> {
    let user = sqlx::query_as!(
        User,
        r#"SELECT u.id, u.email, u.name, u.password_hash
           FROM session s
           JOIN "user" u ON u.id = s.user_id
           WHERE s.token = $1 AND s.expires_at > now()"#,
        token
    )
    .fetch_optional(pool)
    .await?;
    Ok(user)
}

pub async fn delete(pool: &PgPool, token: &str) -> AppResult<()> {
    sqlx::query!("DELETE FROM session WHERE token = $1", token)
        .execute(pool)
        .await?;
    Ok(())
}
```

`crates/fubbik-db/src/repo/mod.rs`:

```rust
pub mod session;
pub mod user;
```

- [ ] **Step 5: Run to verify it passes**

Run: `cargo test -p fubbik-db --test session`
Expected: PASS

- [ ] **Step 6: Implement `AppState` and the `CurrentUser` extractor**

`crates/fubbik-api/src/auth/session.rs`:

```rust
use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum_extra::extract::CookieJar;
use fubbik_core::error::AppError;
use fubbik_db::repo::{session, user};

use crate::AppState;

pub const COOKIE_NAME: &str = "fubbik_session";

/// Extractor yielding the authenticated user, or rejecting with 401.
pub struct CurrentUser(pub user::User);

impl FromRequestParts<AppState> for CurrentUser {
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Self::Rejection> {
        let jar = CookieJar::from_headers(&parts.headers);

        if let Some(cookie) = jar.get(COOKIE_NAME) {
            if let Some(u) = session::find_valid(&state.pool, cookie.value()).await? {
                return Ok(CurrentUser(u));
            }
        }

        // Local-first escape hatch, mirroring FUBBIK_IMPLICIT_DEV_SESSION in
        // the TS server: fall back to the dev user rather than 401ing.
        if state.implicit_dev_session {
            if let Some(u) = user::find_by_email(&state.pool, DEV_EMAIL).await? {
                return Ok(CurrentUser(u));
            }
        }

        Err(AppError::Auth)
    }
}

pub const DEV_EMAIL: &str = "dev@fubbik.local";
```

`crates/fubbik-api/src/lib.rs`:

```rust
pub mod auth;

use sqlx::PgPool;

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub implicit_dev_session: bool,
}
```

Add `pub mod session;` to `crates/fubbik-api/src/auth/mod.rs` and re-export: `pub use session::CurrentUser;`

- [ ] **Step 7: Run the full suite**

Run: `cargo test`
Expected: PASS

- [ ] **Step 8: Commit**

```bash
git add crates/
git commit -m "feat(rust): session store and CurrentUser extractor"
```

---

### Task 7: Auth routes

**Files:**
- Create: `crates/fubbik-api/src/auth/routes.rs`
- Modify: `crates/fubbik-api/src/auth/mod.rs`, `crates/fubbik-api/src/lib.rs`

**Interfaces:**
- Consumes: `hash_password`/`verify_password` (Task 5), session repo and `AppState` (Task 6).
- Produces: `fubbik_api::auth::routes::router() -> axum::Router<AppState>` mounting `POST /api/auth/sign-up/email`, `POST /api/auth/sign-in/email`, `POST /api/auth/sign-out`, `GET /api/auth/get-session`. Paths match what the web client already calls.

- [ ] **Step 1: Write the failing signup-then-login test**

`crates/fubbik-api/tests/auth.rs`:

```rust
use axum::body::Body;
use axum::http::{Request, StatusCode};
use tower::ServiceExt;

fn state(pool: sqlx::PgPool) -> fubbik_api::AppState {
    fubbik_api::AppState { pool, implicit_dev_session: false }
}

#[sqlx::test]
async fn signup_then_signin_sets_cookie(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));

    let signup = app
        .clone()
        .oneshot(
            Request::post("/api/auth/sign-up/email")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"email":"a@b.test","password":"hunter22","name":"Alice"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(signup.status(), StatusCode::OK);

    let signin = app
        .oneshot(
            Request::post("/api/auth/sign-in/email")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"email":"a@b.test","password":"hunter22"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(signin.status(), StatusCode::OK);
    assert!(
        signin.headers().get("set-cookie").is_some(),
        "sign-in must set a session cookie"
    );
}

#[sqlx::test]
async fn wrong_password_is_unauthorized(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    app.clone()
        .oneshot(
            Request::post("/api/auth/sign-up/email")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"email":"a@b.test","password":"hunter22","name":"Alice"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    let res = app
        .oneshot(
            Request::post("/api/auth/sign-in/email")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"email":"a@b.test","password":"wrong"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}
```

Add to `fubbik-api` dev-dependencies: `tower = { version = "0.5", features = ["util"] }`, `http-body-util = "0.1"`.

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p fubbik-api --test auth`
Expected: FAIL — `fubbik_api::router` not found.

- [ ] **Step 3: Implement the auth routes**

`crates/fubbik-api/src/auth/routes.rs`:

```rust
use axum::extract::State;
use axum::routing::{get, post};
use axum::{Json, Router};
use axum_extra::extract::CookieJar;
use axum_extra::extract::cookie::{Cookie, SameSite};
use chrono::Duration;
use fubbik_core::error::{AppError, AppResult};
use fubbik_db::repo::{session, user};

use super::password::{hash_password, verify_password};
use super::session::COOKIE_NAME;
use crate::AppState;

#[derive(serde::Deserialize)]
pub struct SignUpBody {
    pub email: String,
    pub password: String,
    pub name: String,
}

#[derive(serde::Deserialize)]
pub struct SignInBody {
    pub email: String,
    pub password: String,
}

#[derive(serde::Serialize)]
pub struct UserResponse {
    pub id: String,
    pub email: String,
    pub name: String,
}

impl From<user::User> for UserResponse {
    fn from(u: user::User) -> Self {
        Self { id: u.id, email: u.email, name: u.name }
    }
}

/// One source of truth for the session lifetime. The cookie's max-age and the
/// database row's `expires_at` must agree: without a max-age the cookie dies on
/// browser close while the server still considers the session live for 30 days,
/// silently logging the user out.
const SESSION_TTL_DAYS: i64 = 30;

fn session_cookie(token: String) -> Cookie<'static> {
    Cookie::build((COOKIE_NAME, token))
        .path("/")
        .http_only(true)
        .same_site(SameSite::Lax)
        .max_age(time::Duration::days(SESSION_TTL_DAYS))
        .build()
}

async fn sign_up(
    State(state): State<AppState>,
    jar: CookieJar,
    Json(body): Json<SignUpBody>,
) -> AppResult<(CookieJar, Json<UserResponse>)> {
    if body.password.len() < 8 {
        return Err(AppError::Validation("password must be at least 8 characters".into()));
    }
    if user::find_by_email(&state.pool, &body.email).await?.is_some() {
        return Err(AppError::Conflict("email already registered".into()));
    }

    let hash = hash_password(&body.password)?;

    // The pre-check above handles the common case with a clear message, but it
    // is a TOCTOU against the UNIQUE constraint. Catch the violation too, so a
    // concurrent duplicate signup still yields 409 rather than a 500.
    let u = match user::create(&state.pool, &body.email, &body.name, Some(&hash)).await {
        Err(AppError::Database(sqlx::Error::Database(e))) if e.is_unique_violation() => {
            return Err(AppError::Conflict("email already registered".into()));
        }
        other => other?,
    };

    let token = session::create(&state.pool, &u.id, Duration::days(SESSION_TTL_DAYS)).await?;

    Ok((jar.add(session_cookie(token)), Json(u.into())))
}

async fn sign_in(
    State(state): State<AppState>,
    jar: CookieJar,
    Json(body): Json<SignInBody>,
) -> AppResult<(CookieJar, Json<UserResponse>)> {
    let u = user::find_by_email(&state.pool, &body.email)
        .await?
        .ok_or(AppError::Auth)?;

    let stored = u.password_hash.as_deref().ok_or(AppError::Auth)?;
    if !verify_password(&body.password, stored) {
        return Err(AppError::Auth);
    }

    let token = session::create(&state.pool, &u.id, Duration::days(SESSION_TTL_DAYS)).await?;
    Ok((jar.add(session_cookie(token)), Json(u.into())))
}

async fn sign_out(State(state): State<AppState>, jar: CookieJar) -> AppResult<CookieJar> {
    if let Some(c) = jar.get(COOKIE_NAME) {
        session::delete(&state.pool, c.value()).await?;
    }
    Ok(jar.remove(Cookie::from(COOKIE_NAME)))
}

async fn get_session(current: super::CurrentUser) -> Json<UserResponse> {
    Json(current.0.into())
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/auth/sign-up/email", post(sign_up))
        .route("/api/auth/sign-in/email", post(sign_in))
        .route("/api/auth/sign-out", post(sign_out))
        .route("/api/auth/get-session", get(get_session))
}
```

Add to `crates/fubbik-api/src/lib.rs`:

```rust
use axum::Router;

pub fn router(state: AppState) -> Router {
    Router::new()
        .merge(auth::routes::router())
        .with_state(state)
}
```

Add `pub mod routes;` to `crates/fubbik-api/src/auth/mod.rs`.

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p fubbik-api --test auth`
Expected: PASS — both tests.

- [ ] **Step 5: Commit**

```bash
git add crates/fubbik-api
git commit -m "feat(rust): email/password auth routes"
```

---

### Task 8: Chunk repository — CRUD

**Files:**
- Create: `crates/fubbik-db/src/repo/chunk.rs`
- Modify: `crates/fubbik-db/src/repo/mod.rs`

**Interfaces:**
- Consumes: pool (Task 2), `new_id` (Task 6).
- Produces: `fubbik_db::repo::chunk::{Chunk, NewChunk, ChunkPatch, create, find_by_id, update, delete}`.

- [ ] **Step 1: Write the failing CRUD test**

`crates/fubbik-db/tests/chunk.rs`:

```rust
use fubbik_db::repo::{chunk, user};

async fn seed_user(pool: &sqlx::PgPool) -> String {
    user::create(pool, "a@b.test", "Alice", None).await.unwrap().id
}

#[sqlx::test]
async fn create_read_update_delete(pool: sqlx::PgPool) {
    let uid = seed_user(&pool).await;

    let created = chunk::create(
        &pool,
        &uid,
        chunk::NewChunk {
            title: "Naming conventions".into(),
            content: "Use kebab-case.".into(),
            chunk_type: "note".into(),
            rationale: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(created.title, "Naming conventions");

    let patch = chunk::ChunkPatch { title: Some("Renamed".into()), ..Default::default() };
    let updated = chunk::update(&pool, &uid, &created.id, patch).await.unwrap().unwrap();
    assert_eq!(updated.title, "Renamed");
    assert_eq!(updated.content, "Use kebab-case.", "unset patch fields must not clear columns");

    assert!(chunk::delete(&pool, &uid, &created.id).await.unwrap());
    assert!(chunk::find_by_id(&pool, &uid, &created.id).await.unwrap().is_none());
}

#[sqlx::test]
async fn other_users_chunks_are_invisible(pool: sqlx::PgPool) {
    let owner = seed_user(&pool).await;
    let intruder = user::create(&pool, "c@d.test", "Bob", None).await.unwrap().id;

    let c = chunk::create(
        &pool,
        &owner,
        chunk::NewChunk {
            title: "Secret".into(),
            content: String::new(),
            chunk_type: "note".into(),
            rationale: None,
        },
    )
    .await
    .unwrap();

    assert!(chunk::find_by_id(&pool, &intruder, &c.id).await.unwrap().is_none());
    assert!(!chunk::delete(&pool, &intruder, &c.id).await.unwrap());
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p fubbik-db --test chunk`
Expected: FAIL — `repo::chunk` not found.

- [ ] **Step 3: Implement the repository**

`crates/fubbik-db/src/repo/chunk.rs`:

```rust
use chrono::NaiveDateTime;
use fubbik_core::error::AppResult;
use sqlx::PgPool;

/// `camelCase` serialisation is mandatory, not cosmetic: the 106 web files
/// that consume this API were written against Drizzle's camelCase output.
/// Emitting snake_case would silently break every one of them.
#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Chunk {
    pub id: String,
    pub title: String,
    pub content: String,
    #[serde(rename = "type")]
    pub chunk_type: String,
    pub user_id: String,
    pub summary: Option<String>,
    pub rationale: Option<String>,
    pub consequences: Option<String>,
    pub origin: String,
    pub review_status: String,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
    pub archived_at: Option<NaiveDateTime>,
}

pub struct NewChunk {
    pub title: String,
    pub content: String,
    pub chunk_type: String,
    pub rationale: Option<String>,
}

#[derive(Default)]
pub struct ChunkPatch {
    pub title: Option<String>,
    pub content: Option<String>,
    pub chunk_type: Option<String>,
    pub rationale: Option<String>,
    pub consequences: Option<String>,
}

const COLUMNS: &str = r#"id, title, content, type AS chunk_type, user_id, summary,
    rationale, consequences, origin, review_status, created_at, updated_at, archived_at"#;

pub async fn create(pool: &PgPool, user_id: &str, new: NewChunk) -> AppResult<Chunk> {
    let id = crate::new_id();
    let c = sqlx::query_as!(
        Chunk,
        r#"INSERT INTO chunk (id, title, content, type, user_id, rationale)
           VALUES ($1, $2, $3, $4, $5, $6)
           RETURNING id, title, content, type AS chunk_type, user_id, summary,
                     rationale, consequences, origin, review_status,
                     created_at, updated_at, archived_at"#,
        id,
        new.title,
        new.content,
        new.chunk_type,
        user_id,
        new.rationale
    )
    .fetch_one(pool)
    .await?;
    Ok(c)
}

pub async fn find_by_id(pool: &PgPool, user_id: &str, id: &str) -> AppResult<Option<Chunk>> {
    let c = sqlx::query_as!(
        Chunk,
        r#"SELECT id, title, content, type AS chunk_type, user_id, summary,
                  rationale, consequences, origin, review_status,
                  created_at, updated_at, archived_at
           FROM chunk WHERE id = $1 AND user_id = $2"#,
        id,
        user_id
    )
    .fetch_optional(pool)
    .await?;
    Ok(c)
}

/// Applies only the fields present in the patch. COALESCE keeps unset
/// columns untouched, so a partial PATCH cannot silently clear data.
pub async fn update(pool: &PgPool, user_id: &str, id: &str, patch: ChunkPatch) -> AppResult<Option<Chunk>> {
    let c = sqlx::query_as!(
        Chunk,
        r#"UPDATE chunk SET
             title = COALESCE($3, title),
             content = COALESCE($4, content),
             type = COALESCE($5, type),
             rationale = COALESCE($6, rationale),
             consequences = COALESCE($7, consequences),
             updated_at = now()
           WHERE id = $1 AND user_id = $2
           RETURNING id, title, content, type AS chunk_type, user_id, summary,
                     rationale, consequences, origin, review_status,
                     created_at, updated_at, archived_at"#,
        id,
        user_id,
        patch.title,
        patch.content,
        patch.chunk_type,
        patch.rationale,
        patch.consequences
    )
    .fetch_optional(pool)
    .await?;
    Ok(c)
}

pub async fn delete(pool: &PgPool, user_id: &str, id: &str) -> AppResult<bool> {
    let res = sqlx::query!("DELETE FROM chunk WHERE id = $1 AND user_id = $2", id, user_id)
        .execute(pool)
        .await?;
    Ok(res.rows_affected() > 0)
}
```

Note: `COLUMNS` is declared for reuse by Task 9's list query. `query_as!` requires literal SQL, so the column list is repeated inline rather than interpolated.

Add `utoipa` to `fubbik-db` dependencies. Add `pub mod chunk;` to `repo/mod.rs`.

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p fubbik-db --test chunk`
Expected: PASS — both tests.

- [ ] **Step 5: Regenerate the offline query cache**

Run: `cargo sqlx prepare --workspace`

- [ ] **Step 6: Commit**

```bash
git add crates/fubbik-db .sqlx
git commit -m "feat(rust): chunk repository CRUD"
```

---

### Task 9: Chunk listing with filters and sorting

**Files:**
- Modify: `crates/fubbik-db/src/repo/chunk.rs`
- Modify: `crates/fubbik-db/tests/chunk.rs`

**Interfaces:**
- Consumes: `Chunk` from Task 8.
- Produces: `fubbik_db::repo::chunk::{ListParams, list}` returning `AppResult<Vec<Chunk>>`.

Phase 1 supports these query parameters from the Node route at `packages/api/src/chunks/routes.ts:24-44`: `type`, `search`, `limit`, `offset`, `sort`, `origin`, `reviewStatus`. The remaining parameters (`tags`, `tagMode`, `scope`, `alias`, `spaceId`, `workspaceId`, `global`, `allSpaces`, `after`, `enrichment`, `minConnections`, `exclude`) belong to Phase 2, which brings the tag and space domains.

- [ ] **Step 1: Write the failing listing test**

Append to `crates/fubbik-db/tests/chunk.rs`:

```rust
#[sqlx::test]
async fn list_filters_sorts_and_paginates(pool: sqlx::PgPool) {
    let uid = seed_user(&pool).await;

    for (title, ty) in [("Alpha", "note"), ("Beta", "document"), ("Gamma", "note")] {
        chunk::create(
            &pool,
            &uid,
            chunk::NewChunk {
                title: title.into(),
                content: format!("{title} body"),
                chunk_type: ty.into(),
                rationale: None,
            },
        )
        .await
        .unwrap();
    }

    let notes = chunk::list(
        &pool,
        &uid,
        chunk::ListParams { chunk_type: Some("note".into()), ..Default::default() },
    )
    .await
    .unwrap();
    assert_eq!(notes.len(), 2);

    let searched = chunk::list(
        &pool,
        &uid,
        chunk::ListParams { search: Some("Beta".into()), ..Default::default() },
    )
    .await
    .unwrap();
    assert_eq!(searched.len(), 1);
    assert_eq!(searched[0].title, "Beta");

    let alpha = chunk::list(
        &pool,
        &uid,
        chunk::ListParams { sort: chunk::Sort::Alpha, ..Default::default() },
    )
    .await
    .unwrap();
    assert_eq!(
        alpha.iter().map(|c| c.title.as_str()).collect::<Vec<_>>(),
        ["Alpha", "Beta", "Gamma"]
    );

    let page = chunk::list(
        &pool,
        &uid,
        chunk::ListParams { limit: 2, offset: 1, sort: chunk::Sort::Alpha, ..Default::default() },
    )
    .await
    .unwrap();
    assert_eq!(page.len(), 2);
    assert_eq!(page[0].title, "Beta");
}

#[sqlx::test]
async fn search_is_case_insensitive_and_covers_content(pool: sqlx::PgPool) {
    let uid = seed_user(&pool).await;
    chunk::create(
        &pool,
        &uid,
        chunk::NewChunk {
            title: "Title".into(),
            content: "UNIQUEBODY".into(),
            chunk_type: "note".into(),
            rationale: None,
        },
    )
    .await
    .unwrap();

    let found = chunk::list(
        &pool,
        &uid,
        chunk::ListParams { search: Some("uniquebody".into()), ..Default::default() },
    )
    .await
    .unwrap();
    assert_eq!(found.len(), 1);
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p fubbik-db --test chunk`
Expected: FAIL — `chunk::list` not found.

- [ ] **Step 3: Implement listing**

Append to `crates/fubbik-db/src/repo/chunk.rs`:

```rust
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Sort {
    #[default]
    Newest,
    Oldest,
    Alpha,
    Updated,
}

pub struct ListParams {
    pub chunk_type: Option<String>,
    pub search: Option<String>,
    pub origin: Option<String>,
    pub review_status: Option<String>,
    pub sort: Sort,
    pub limit: i64,
    pub offset: i64,
}

impl Default for ListParams {
    fn default() -> Self {
        Self {
            chunk_type: None,
            search: None,
            origin: None,
            review_status: None,
            sort: Sort::Newest,
            limit: 50,
            offset: 0,
        }
    }
}

/// Lists a user's non-archived chunks.
///
/// Uses QueryBuilder rather than `query_as!` because the filter set is
/// dynamic. Every user value is pushed as a bind parameter, never
/// formatted into the SQL string.
pub async fn list(pool: &PgPool, user_id: &str, params: ListParams) -> AppResult<Vec<Chunk>> {
    let mut qb = sqlx::QueryBuilder::new(
        "SELECT id, title, content, type AS chunk_type, user_id, summary, \
         rationale, consequences, origin, review_status, \
         created_at, updated_at, archived_at \
         FROM chunk WHERE archived_at IS NULL AND user_id = ",
    );
    qb.push_bind(user_id);

    if let Some(t) = &params.chunk_type {
        qb.push(" AND type = ").push_bind(t);
    }
    if let Some(o) = &params.origin {
        qb.push(" AND origin = ").push_bind(o);
    }
    if let Some(r) = &params.review_status {
        qb.push(" AND review_status = ").push_bind(r);
    }
    if let Some(s) = &params.search {
        // ILIKE with escaped wildcards: a user searching for "100%" must not
        // match everything.
        let pattern = format!("%{}%", s.replace('\\', r"\\").replace('%', r"\%").replace('_', r"\_"));
        qb.push(" AND (title ILIKE ").push_bind(pattern.clone());
        qb.push(" OR content ILIKE ").push_bind(pattern);
        qb.push(")");
    }

    qb.push(match params.sort {
        Sort::Newest => " ORDER BY created_at DESC",
        Sort::Oldest => " ORDER BY created_at ASC",
        Sort::Alpha => " ORDER BY title ASC",
        Sort::Updated => " ORDER BY updated_at DESC",
    });

    qb.push(" LIMIT ").push_bind(params.limit.clamp(1, 500));
    qb.push(" OFFSET ").push_bind(params.offset.max(0));

    let rows = qb.build_query_as::<Chunk>().fetch_all(pool).await?;
    Ok(rows)
}
```

`build_query_as` relies on the `sqlx::FromRow` derive already present on `Chunk` from Task 8; the `query_as!` macro forms do not need it, but this dynamic query does.

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p fubbik-db --test chunk`
Expected: PASS — all four tests in the file.

- [ ] **Step 5: Commit**

```bash
git add crates/fubbik-db
git commit -m "feat(rust): chunk listing with filters, sort, pagination"
```

---

### Task 10: Chunk HTTP routes with OpenAPI annotations

**Files:**
- Create: `crates/fubbik-api/src/chunks/mod.rs`, `dto.rs`, `service.rs`, `routes.rs`
- Modify: `crates/fubbik-api/src/lib.rs`

**Interfaces:**
- Consumes: chunk repo (Tasks 8-9), `CurrentUser` (Task 6), `AppError` (Task 4).
- Produces: `fubbik_api::chunks::routes::router() -> Router<AppState>` mounting `GET /api/chunks`, `POST /api/chunks`, `GET /api/chunks/{id}`, `PATCH /api/chunks/{id}`, `DELETE /api/chunks/{id}`.

- [ ] **Step 1: Write the failing route test**

`crates/fubbik-api/tests/chunks.rs`:

```rust
use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use tower::ServiceExt;

fn dev_state(pool: sqlx::PgPool) -> fubbik_api::AppState {
    fubbik_api::AppState { pool, implicit_dev_session: true }
}

async fn seed_dev_user(pool: &sqlx::PgPool) {
    fubbik_db::repo::user::create(pool, "dev@fubbik.local", "Dev", None)
        .await
        .unwrap();
}

#[sqlx::test]
async fn create_then_fetch_chunk(pool: sqlx::PgPool) {
    seed_dev_user(&pool).await;
    let app = fubbik_api::router(dev_state(pool));

    let res = app
        .clone()
        .oneshot(
            Request::post("/api/chunks")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"title":"Naming","content":"kebab-case"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let body = res.into_body().collect().await.unwrap().to_bytes();
    let created: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let id = created["id"].as_str().unwrap();
    assert_eq!(created["type"], "note", "type must default to note");

    let res = app
        .oneshot(Request::get(format!("/api/chunks/{id}")).body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}

#[sqlx::test]
async fn missing_chunk_is_404(pool: sqlx::PgPool) {
    seed_dev_user(&pool).await;
    let app = fubbik_api::router(dev_state(pool));

    let res = app
        .oneshot(Request::get("/api/chunks/nonexistent").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

#[sqlx::test]
async fn unauthenticated_request_is_401(pool: sqlx::PgPool) {
    let app = fubbik_api::router(fubbik_api::AppState { pool, implicit_dev_session: false });

    let res = app
        .oneshot(Request::get("/api/chunks").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}

#[sqlx::test]
async fn blank_title_is_400(pool: sqlx::PgPool) {
    seed_dev_user(&pool).await;
    let app = fubbik_api::router(dev_state(pool));

    let res = app
        .oneshot(
            Request::post("/api/chunks")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"title":"   "}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p fubbik-api --test chunks`
Expected: FAIL — no `/api/chunks` route, returns 404.

- [ ] **Step 3: Write the DTOs**

`crates/fubbik-api/src/chunks/dto.rs`:

```rust
use fubbik_db::repo::chunk::Sort;

#[derive(serde::Deserialize, utoipa::ToSchema)]
pub struct CreateChunkBody {
    pub title: String,
    #[serde(default)]
    pub content: String,
    #[serde(rename = "type")]
    pub chunk_type: Option<String>,
    pub rationale: Option<String>,
}

#[derive(serde::Deserialize, utoipa::ToSchema)]
pub struct UpdateChunkBody {
    pub title: Option<String>,
    pub content: Option<String>,
    #[serde(rename = "type")]
    pub chunk_type: Option<String>,
    pub rationale: Option<String>,
    pub consequences: Option<String>,
}

/// Query params arrive as strings from the web client, matching the Elysia
/// route's `t.Optional(t.String())` shape, so numeric fields parse leniently.
#[derive(serde::Deserialize, utoipa::IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct ListChunksQuery {
    #[serde(rename = "type")]
    pub chunk_type: Option<String>,
    pub search: Option<String>,
    pub origin: Option<String>,
    pub review_status: Option<String>,
    pub sort: Option<Sort>,
    pub limit: Option<String>,
    pub offset: Option<String>,
}

impl ListChunksQuery {
    pub fn into_params(self) -> fubbik_db::repo::chunk::ListParams {
        fubbik_db::repo::chunk::ListParams {
            chunk_type: self.chunk_type,
            search: self.search,
            origin: self.origin,
            review_status: self.review_status,
            sort: self.sort.unwrap_or_default(),
            limit: self.limit.and_then(|s| s.parse().ok()).unwrap_or(50),
            offset: self.offset.and_then(|s| s.parse().ok()).unwrap_or(0),
        }
    }
}
```

- [ ] **Step 4: Write the service layer**

`crates/fubbik-api/src/chunks/service.rs`:

```rust
use fubbik_core::error::{AppError, AppResult};
use fubbik_db::repo::chunk::{self, Chunk, ChunkPatch, ListParams, NewChunk};
use sqlx::PgPool;

use super::dto::{CreateChunkBody, UpdateChunkBody};

pub async fn list(pool: &PgPool, user_id: &str, params: ListParams) -> AppResult<Vec<Chunk>> {
    chunk::list(pool, user_id, params).await
}

pub async fn create(pool: &PgPool, user_id: &str, body: CreateChunkBody) -> AppResult<Chunk> {
    let title = body.title.trim();
    if title.is_empty() {
        return Err(AppError::Validation("title is required".into()));
    }
    if title.chars().count() > 200 {
        return Err(AppError::Validation("title must be at most 200 characters".into()));
    }

    chunk::create(
        pool,
        user_id,
        NewChunk {
            title: title.to_string(),
            content: body.content,
            chunk_type: body.chunk_type.unwrap_or_else(|| "note".into()),
            rationale: body.rationale,
        },
    )
    .await
}

pub async fn get(pool: &PgPool, user_id: &str, id: &str) -> AppResult<Chunk> {
    chunk::find_by_id(pool, user_id, id)
        .await?
        .ok_or_else(|| AppError::NotFound("chunk".into()))
}

pub async fn update(pool: &PgPool, user_id: &str, id: &str, body: UpdateChunkBody) -> AppResult<Chunk> {
    chunk::update(
        pool,
        user_id,
        id,
        ChunkPatch {
            title: body.title,
            content: body.content,
            chunk_type: body.chunk_type,
            rationale: body.rationale,
            consequences: body.consequences,
        },
    )
    .await?
    .ok_or_else(|| AppError::NotFound("chunk".into()))
}

pub async fn delete(pool: &PgPool, user_id: &str, id: &str) -> AppResult<()> {
    if chunk::delete(pool, user_id, id).await? {
        Ok(())
    } else {
        Err(AppError::NotFound("chunk".into()))
    }
}
```

- [ ] **Step 5: Write the routes**

`crates/fubbik-api/src/chunks/routes.rs`:

```rust
use axum::extract::{Path, Query, State};
use axum::routing::get;
use axum::{Json, Router};
use fubbik_core::error::AppResult;
use fubbik_db::repo::chunk::Chunk;

use super::dto::{CreateChunkBody, ListChunksQuery, UpdateChunkBody};
use super::service;
use crate::auth::CurrentUser;
use crate::AppState;

#[utoipa::path(
    get, path = "/api/chunks", params(ListChunksQuery),
    responses((status = 200, body = Vec<Chunk>))
)]
async fn list_chunks(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Query(query): Query<ListChunksQuery>,
) -> AppResult<Json<Vec<Chunk>>> {
    Ok(Json(service::list(&state.pool, &user.id, query.into_params()).await?))
}

#[utoipa::path(post, path = "/api/chunks", request_body = CreateChunkBody,
    responses((status = 200, body = Chunk)))]
async fn create_chunk(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Json(body): Json<CreateChunkBody>,
) -> AppResult<Json<Chunk>> {
    Ok(Json(service::create(&state.pool, &user.id, body).await?))
}

#[utoipa::path(get, path = "/api/chunks/{id}", params(("id" = String, Path,)),
    responses((status = 200, body = Chunk), (status = 404)))]
async fn get_chunk(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<String>,
) -> AppResult<Json<Chunk>> {
    Ok(Json(service::get(&state.pool, &user.id, &id).await?))
}

#[utoipa::path(patch, path = "/api/chunks/{id}", request_body = UpdateChunkBody,
    params(("id" = String, Path,)), responses((status = 200, body = Chunk), (status = 404)))]
async fn update_chunk(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<String>,
    Json(body): Json<UpdateChunkBody>,
) -> AppResult<Json<Chunk>> {
    Ok(Json(service::update(&state.pool, &user.id, &id, body).await?))
}

#[utoipa::path(delete, path = "/api/chunks/{id}", params(("id" = String, Path,)),
    responses((status = 200), (status = 404)))]
async fn delete_chunk(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<String>,
) -> AppResult<Json<serde_json::Value>> {
    service::delete(&state.pool, &user.id, &id).await?;
    Ok(Json(serde_json::json!({ "success": true })))
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/chunks", get(list_chunks).post(create_chunk))
        .route(
            "/api/chunks/{id}",
            get(get_chunk).patch(update_chunk).delete(delete_chunk),
        )
}
```

`crates/fubbik-api/src/chunks/mod.rs`:

```rust
pub mod dto;
pub mod routes;
pub mod service;
```

Update `crates/fubbik-api/src/lib.rs`:

```rust
pub mod auth;
pub mod chunks;

pub fn router(state: AppState) -> Router {
    Router::new()
        .merge(auth::routes::router())
        .merge(chunks::routes::router())
        .with_state(state)
}
```

- [ ] **Step 6: Run to verify it passes**

Run: `cargo test -p fubbik-api --test chunks`
Expected: PASS — all four tests.

- [ ] **Step 7: Commit**

```bash
git add crates/fubbik-api
git commit -m "feat(rust): chunk CRUD routes with OpenAPI annotations"
```

---

### Task 11: Chunk version history

**Files:**
- Create: `crates/fubbik-db/src/repo/chunk_version.rs`
- Modify: `crates/fubbik-db/src/repo/mod.rs`, `crates/fubbik-api/src/chunks/{service.rs,routes.rs}`

**Interfaces:**
- Consumes: chunk repo (Task 8), service layer (Task 10).
- Produces: `fubbik_db::repo::chunk_version::{ChunkVersion, snapshot, list_for_chunk}`; route `GET /api/chunks/{id}/history`.

The `chunk_version` table is append-only. The service snapshots the pre-edit state on every update, matching the TS behaviour.

- [ ] **Step 1: Write the failing history test**

`crates/fubbik-db/tests/chunk_version.rs`:

```rust
use fubbik_db::repo::{chunk, chunk_version, user};

#[sqlx::test]
async fn snapshot_records_pre_edit_state(pool: sqlx::PgPool) {
    let uid = user::create(&pool, "a@b.test", "Alice", None).await.unwrap().id;
    let c = chunk::create(
        &pool,
        &uid,
        chunk::NewChunk {
            title: "Original".into(),
            content: "v1".into(),
            chunk_type: "note".into(),
            rationale: None,
        },
    )
    .await
    .unwrap();

    chunk_version::snapshot(&pool, &c).await.unwrap();

    let history = chunk_version::list_for_chunk(&pool, &c.id).await.unwrap();
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].title, "Original");
    assert_eq!(history[0].content, "v1");
    assert_eq!(history[0].version, 1, "first snapshot is version 1");
}

#[sqlx::test]
async fn version_numbers_increment_per_chunk(pool: sqlx::PgPool) {
    let uid = user::create(&pool, "a@b.test", "Alice", None).await.unwrap().id;
    let c = chunk::create(
        &pool,
        &uid,
        chunk::NewChunk {
            title: "T".into(),
            content: String::new(),
            chunk_type: "note".into(),
            rationale: None,
        },
    )
    .await
    .unwrap();

    chunk_version::snapshot(&pool, &c).await.unwrap();
    chunk_version::snapshot(&pool, &c).await.unwrap();

    let history = chunk_version::list_for_chunk(&pool, &c.id).await.unwrap();
    assert_eq!(
        history.iter().map(|v| v.version).collect::<Vec<_>>(),
        [2, 1],
        "history is newest-first with incrementing versions"
    );
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p fubbik-db --test chunk_version`
Expected: FAIL — `repo::chunk_version` not found.

- [ ] **Step 3: Implement the version repository**

The `chunk_version` table has three NOT NULL columns beyond the obvious ones — `version` (integer), `type` (text), and `tags` (jsonb). Omitting any of them makes the INSERT fail at runtime. `version` is a per-chunk incrementing counter, computed in the same statement to avoid a read-then-write race.

`tags` is written as an empty array in Phase 1 because the tag domain does not exist yet. **Phase 2 must revisit this** when tags land, or version history will record every chunk as untagged.

`crates/fubbik-db/src/repo/chunk_version.rs`:

```rust
use chrono::NaiveDateTime;
use fubbik_core::error::AppResult;
use sqlx::PgPool;

use super::chunk::Chunk;

#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ChunkVersion {
    pub id: String,
    pub chunk_id: String,
    pub version: i32,
    pub title: String,
    pub content: String,
    #[serde(rename = "type")]
    pub chunk_type: String,
    pub rationale: Option<String>,
    pub consequences: Option<String>,
    pub created_at: NaiveDateTime,
}

/// Appends the chunk's current state to its history. Call before applying
/// an update so the snapshot captures the pre-edit version.
///
/// The version number is derived inside the INSERT rather than by a prior
/// SELECT, so two concurrent snapshots cannot both compute the same value.
/// The aggregate over an empty set yields NULL, which COALESCE turns into
/// the first version, 1.
pub async fn snapshot(pool: &PgPool, current: &Chunk) -> AppResult<()> {
    let id = crate::new_id();
    sqlx::query!(
        r#"INSERT INTO chunk_version
             (id, chunk_id, version, title, content, type, tags,
              rationale, consequences, created_at)
           SELECT $1, $2, COALESCE(MAX(v.version), 0) + 1, $3, $4, $5,
                  '[]'::jsonb, $6, $7, now()
           FROM chunk_version v WHERE v.chunk_id = $2"#,
        id,
        current.id,
        current.title,
        current.content,
        current.chunk_type,
        current.rationale,
        current.consequences
    )
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn list_for_chunk(pool: &PgPool, chunk_id: &str) -> AppResult<Vec<ChunkVersion>> {
    let rows = sqlx::query_as!(
        ChunkVersion,
        r#"SELECT id, chunk_id, version, title, content, type AS chunk_type,
                  rationale, consequences, created_at
           FROM chunk_version WHERE chunk_id = $1 ORDER BY version DESC"#,
        chunk_id
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}
```

Add `pub mod chunk_version;` to `repo/mod.rs`.

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p fubbik-db --test chunk_version`
Expected: PASS

- [ ] **Step 5: Snapshot on update, and expose the history route**

In `crates/fubbik-api/src/chunks/service.rs`, replace the body of `update` so it snapshots first:

```rust
pub async fn update(pool: &PgPool, user_id: &str, id: &str, body: UpdateChunkBody) -> AppResult<Chunk> {
    let current = get(pool, user_id, id).await?;
    fubbik_db::repo::chunk_version::snapshot(pool, &current).await?;

    chunk::update(
        pool,
        user_id,
        id,
        ChunkPatch {
            title: body.title,
            content: body.content,
            chunk_type: body.chunk_type,
            rationale: body.rationale,
            consequences: body.consequences,
        },
    )
    .await?
    .ok_or_else(|| AppError::NotFound("chunk".into()))
}

pub async fn history(pool: &PgPool, user_id: &str, id: &str) -> AppResult<Vec<fubbik_db::repo::chunk_version::ChunkVersion>> {
    // Fetch the chunk first so another user's history cannot be read.
    get(pool, user_id, id).await?;
    fubbik_db::repo::chunk_version::list_for_chunk(pool, id).await
}
```

In `crates/fubbik-api/src/chunks/routes.rs`, add the handler and route:

```rust
#[utoipa::path(get, path = "/api/chunks/{id}/history", params(("id" = String, Path,)),
    responses((status = 200, body = Vec<fubbik_db::repo::chunk_version::ChunkVersion>)))]
async fn chunk_history(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<String>,
) -> AppResult<Json<Vec<fubbik_db::repo::chunk_version::ChunkVersion>>> {
    Ok(Json(service::history(&state.pool, &user.id, &id).await?))
}
```

Add to `router()`: `.route("/api/chunks/{id}/history", get(chunk_history))`

- [ ] **Step 6: Write the failing route test**

Append to `crates/fubbik-api/tests/chunks.rs`:

```rust
#[sqlx::test]
async fn update_records_history(pool: sqlx::PgPool) {
    seed_dev_user(&pool).await;
    let app = fubbik_api::router(dev_state(pool));

    let res = app
        .clone()
        .oneshot(
            Request::post("/api/chunks")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"title":"V1","content":"first"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let id = serde_json::from_slice::<serde_json::Value>(&body).unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();

    app.clone()
        .oneshot(
            Request::patch(format!("/api/chunks/{id}"))
                .header("content-type", "application/json")
                .body(Body::from(r#"{"title":"V2"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    let res = app
        .oneshot(
            Request::get(format!("/api/chunks/{id}/history"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let history: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(history.as_array().unwrap().len(), 1);
    assert_eq!(history[0]["title"], "V1", "history stores the pre-edit title");
}
```

- [ ] **Step 7: Run to verify it passes**

Run: `cargo test -p fubbik-api --test chunks`
Expected: PASS

- [ ] **Step 8: Commit**

```bash
git add crates/
git commit -m "feat(rust): chunk version history"
```

---

### Task 12: Chunk sub-resources — applies-to and file-refs

**Files:**
- Create: `crates/fubbik-db/src/repo/chunk_meta.rs`
- Modify: `crates/fubbik-db/src/repo/mod.rs`, `crates/fubbik-api/src/chunks/{service.rs,routes.rs}`

**Interfaces:**
- Consumes: chunk service (Task 10).
- Produces: `fubbik_db::repo::chunk_meta::{AppliesTo, FileRef, get_applies_to, replace_applies_to, get_file_refs, replace_file_refs}`; routes `GET|PUT /api/chunks/{id}/applies-to` and `GET|PUT /api/chunks/{id}/file-refs`.

Both are replace-whole-set operations, matching the TS `PUT` semantics.

Both tables carry columns Phase 1 does not populate, all of which are safe to omit from the INSERTs:

- `chunk_applies_to` — `note` (nullable, unused until the UI exposes it)
- `chunk_file_ref` — `anchor` (nullable) and `relation` (NOT NULL, defaults to `'documents'`)

- [ ] **Step 1: Write the failing test**

`crates/fubbik-db/tests/chunk_meta.rs`:

```rust
use fubbik_db::repo::{chunk, chunk_meta, user};

#[sqlx::test]
async fn replace_applies_to_is_idempotent(pool: sqlx::PgPool) {
    let uid = user::create(&pool, "a@b.test", "Alice", None).await.unwrap().id;
    let c = chunk::create(
        &pool,
        &uid,
        chunk::NewChunk {
            title: "T".into(),
            content: String::new(),
            chunk_type: "note".into(),
            rationale: None,
        },
    )
    .await
    .unwrap();

    chunk_meta::replace_applies_to(&pool, &c.id, &["src/**/*.ts".into(), "docs/**".into()])
        .await
        .unwrap();
    assert_eq!(chunk_meta::get_applies_to(&pool, &c.id).await.unwrap().len(), 2);

    // Replacing with a smaller set must delete the old rows, not merge.
    chunk_meta::replace_applies_to(&pool, &c.id, &["src/**/*.ts".into()])
        .await
        .unwrap();
    let patterns = chunk_meta::get_applies_to(&pool, &c.id).await.unwrap();
    assert_eq!(patterns.len(), 1);
    assert_eq!(patterns[0].pattern, "src/**/*.ts");
}

#[sqlx::test]
async fn replace_file_refs_round_trips(pool: sqlx::PgPool) {
    let uid = user::create(&pool, "a@b.test", "Alice", None).await.unwrap().id;
    let c = chunk::create(
        &pool,
        &uid,
        chunk::NewChunk {
            title: "T".into(),
            content: String::new(),
            chunk_type: "note".into(),
            rationale: None,
        },
    )
    .await
    .unwrap();

    chunk_meta::replace_file_refs(&pool, &c.id, &["src/index.ts".into()])
        .await
        .unwrap();
    let refs = chunk_meta::get_file_refs(&pool, &c.id).await.unwrap();
    assert_eq!(refs.len(), 1);
    assert_eq!(refs[0].path, "src/index.ts");
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p fubbik-db --test chunk_meta`
Expected: FAIL — `repo::chunk_meta` not found.

- [ ] **Step 3: Implement the repository**

`crates/fubbik-db/src/repo/chunk_meta.rs`:

```rust
use fubbik_core::error::AppResult;
use sqlx::PgPool;

#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AppliesTo {
    pub id: String,
    pub chunk_id: String,
    pub pattern: String,
}

#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct FileRef {
    pub id: String,
    pub chunk_id: String,
    pub path: String,
}

pub async fn get_applies_to(pool: &PgPool, chunk_id: &str) -> AppResult<Vec<AppliesTo>> {
    let rows = sqlx::query_as!(
        AppliesTo,
        "SELECT id, chunk_id, pattern FROM chunk_applies_to WHERE chunk_id = $1 ORDER BY pattern",
        chunk_id
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Replaces the whole pattern set in one transaction, so a failure part-way
/// through cannot leave the chunk with a truncated set.
pub async fn replace_applies_to(pool: &PgPool, chunk_id: &str, patterns: &[String]) -> AppResult<()> {
    let mut tx = pool.begin().await?;

    sqlx::query!("DELETE FROM chunk_applies_to WHERE chunk_id = $1", chunk_id)
        .execute(&mut *tx)
        .await?;

    for pattern in patterns {
        let id = crate::new_id();
        sqlx::query!(
            "INSERT INTO chunk_applies_to (id, chunk_id, pattern) VALUES ($1, $2, $3)",
            id,
            chunk_id,
            pattern
        )
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;
    Ok(())
}

pub async fn get_file_refs(pool: &PgPool, chunk_id: &str) -> AppResult<Vec<FileRef>> {
    let rows = sqlx::query_as!(
        FileRef,
        "SELECT id, chunk_id, path FROM chunk_file_ref WHERE chunk_id = $1 ORDER BY path",
        chunk_id
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn replace_file_refs(pool: &PgPool, chunk_id: &str, paths: &[String]) -> AppResult<()> {
    let mut tx = pool.begin().await?;

    sqlx::query!("DELETE FROM chunk_file_ref WHERE chunk_id = $1", chunk_id)
        .execute(&mut *tx)
        .await?;

    for path in paths {
        let id = crate::new_id();
        sqlx::query!(
            "INSERT INTO chunk_file_ref (id, chunk_id, path) VALUES ($1, $2, $3)",
            id,
            chunk_id,
            path
        )
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;
    Ok(())
}
```

Add `pub mod chunk_meta;` to `repo/mod.rs`.

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p fubbik-db --test chunk_meta`
Expected: PASS

- [ ] **Step 5: Add the routes**

Append to `crates/fubbik-api/src/chunks/routes.rs`:

```rust
use axum::routing::put;
use fubbik_db::repo::chunk_meta::{self, AppliesTo, FileRef};

#[derive(serde::Deserialize, utoipa::ToSchema)]
pub struct PatternsBody {
    pub patterns: Vec<String>,
}

#[derive(serde::Deserialize, utoipa::ToSchema)]
pub struct PathsBody {
    pub paths: Vec<String>,
}

#[utoipa::path(get, path = "/api/chunks/{id}/applies-to", params(("id" = String, Path,)),
    responses((status = 200, body = Vec<AppliesTo>)))]
async fn get_applies_to(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<String>,
) -> AppResult<Json<Vec<AppliesTo>>> {
    service::get(&state.pool, &user.id, &id).await?;
    Ok(Json(chunk_meta::get_applies_to(&state.pool, &id).await?))
}

#[utoipa::path(put, path = "/api/chunks/{id}/applies-to", request_body = PatternsBody,
    params(("id" = String, Path,)), responses((status = 200, body = Vec<AppliesTo>)))]
async fn put_applies_to(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<String>,
    Json(body): Json<PatternsBody>,
) -> AppResult<Json<Vec<AppliesTo>>> {
    service::get(&state.pool, &user.id, &id).await?;
    chunk_meta::replace_applies_to(&state.pool, &id, &body.patterns).await?;
    Ok(Json(chunk_meta::get_applies_to(&state.pool, &id).await?))
}

#[utoipa::path(get, path = "/api/chunks/{id}/file-refs", params(("id" = String, Path,)),
    responses((status = 200, body = Vec<FileRef>)))]
async fn get_file_refs(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<String>,
) -> AppResult<Json<Vec<FileRef>>> {
    service::get(&state.pool, &user.id, &id).await?;
    Ok(Json(chunk_meta::get_file_refs(&state.pool, &id).await?))
}

#[utoipa::path(put, path = "/api/chunks/{id}/file-refs", request_body = PathsBody,
    params(("id" = String, Path,)), responses((status = 200, body = Vec<FileRef>)))]
async fn put_file_refs(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<String>,
    Json(body): Json<PathsBody>,
) -> AppResult<Json<Vec<FileRef>>> {
    service::get(&state.pool, &user.id, &id).await?;
    chunk_meta::replace_file_refs(&state.pool, &id, &body.paths).await?;
    Ok(Json(chunk_meta::get_file_refs(&state.pool, &id).await?))
}
```

Add to `router()`:

```rust
.route("/api/chunks/{id}/applies-to", get(get_applies_to).put(put_applies_to))
.route("/api/chunks/{id}/file-refs", get(get_file_refs).put(put_file_refs))
```

- [ ] **Step 6: Run the full suite**

Run: `cargo test`
Expected: PASS

- [ ] **Step 7: Commit**

```bash
git add crates/
git commit -m "feat(rust): chunk applies-to and file-ref sub-resources"
```

---

### Task 13: OpenAPI document generation

**Files:**
- Create: `crates/fubbik-api/src/openapi.rs`, `crates/fubbik-api/tests/openapi.rs`
- Modify: `crates/fubbik-api/src/lib.rs`, `crates/fubbik/src/main.rs`
- Create: `openapi.json`

**Interfaces:**
- Consumes: annotated handlers (Tasks 10-12).
- Produces: `fubbik_api::openapi::ApiDoc::openapi()`; the `fubbik openapi` subcommand printing the document to stdout.

- [ ] **Step 1: Write the failing test**

`crates/fubbik-api/tests/openapi.rs`:

```rust
use utoipa::OpenApi;

#[test]
fn document_contains_every_chunk_path() {
    let doc = fubbik_api::openapi::ApiDoc::openapi();
    let json = serde_json::to_value(&doc).unwrap();
    let paths = json["paths"].as_object().unwrap();

    for expected in [
        "/api/chunks",
        "/api/chunks/{id}",
        "/api/chunks/{id}/history",
        "/api/chunks/{id}/applies-to",
        "/api/chunks/{id}/file-refs",
    ] {
        assert!(paths.contains_key(expected), "missing path {expected}");
    }
}

#[test]
fn committed_openapi_json_is_current() {
    let doc = fubbik_api::openapi::ApiDoc::openapi();
    let generated = serde_json::to_string_pretty(&doc).unwrap();
    let committed = std::fs::read_to_string(
        concat!(env!("CARGO_MANIFEST_DIR"), "/../../openapi.json")
    )
    .expect("openapi.json exists at repo root");

    assert_eq!(
        generated.trim(),
        committed.trim(),
        "openapi.json is stale — run `cargo run -- openapi > openapi.json`"
    );
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p fubbik-api --test openapi`
Expected: FAIL — `fubbik_api::openapi` not found.

- [ ] **Step 3: Implement the document**

`crates/fubbik-api/src/openapi.rs`:

```rust
use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    info(title = "Fubbik API", version = "0.1.0"),
    paths(
        crate::chunks::routes::list_chunks,
        crate::chunks::routes::create_chunk,
        crate::chunks::routes::get_chunk,
        crate::chunks::routes::update_chunk,
        crate::chunks::routes::delete_chunk,
        crate::chunks::routes::chunk_history,
        crate::chunks::routes::get_applies_to,
        crate::chunks::routes::put_applies_to,
        crate::chunks::routes::get_file_refs,
        crate::chunks::routes::put_file_refs,
    ),
    components(schemas(
        fubbik_db::repo::chunk::Chunk,
        fubbik_db::repo::chunk_version::ChunkVersion,
        fubbik_db::repo::chunk_meta::AppliesTo,
        fubbik_db::repo::chunk_meta::FileRef,
        crate::chunks::dto::CreateChunkBody,
        crate::chunks::dto::UpdateChunkBody,
        crate::chunks::routes::PatternsBody,
        crate::chunks::routes::PathsBody,
    ))
)]
pub struct ApiDoc;
```

Make every annotated handler `pub` in `routes.rs` — utoipa's `paths(...)` requires visible items.

Add `pub mod openapi;` to `crates/fubbik-api/src/lib.rs`.

- [ ] **Step 4: Add the `openapi` subcommand**

In `crates/fubbik/src/main.rs`, add a variant and arm:

```rust
    /// Print the OpenAPI document to stdout
    Openapi,
```

```rust
        Commands::Openapi => {
            use utoipa::OpenApi;
            let doc = fubbik_api::openapi::ApiDoc::openapi();
            println!("{}", serde_json::to_string_pretty(&doc)?);
            Ok(())
        }
```

Add `fubbik-api`, `serde_json`, and `utoipa` to the `fubbik` binary's dependencies.

- [ ] **Step 5: Generate and commit the document**

```bash
cargo run -- openapi > openapi.json
```

- [ ] **Step 6: Run to verify both tests pass**

Run: `cargo test -p fubbik-api --test openapi`
Expected: PASS

- [ ] **Step 7: Add the CI guard**

Create `.github/workflows/rust.yml`:

```yaml
name: rust
on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    services:
      postgres:
        image: pgvector/pgvector:pg18
        env:
          POSTGRES_PASSWORD: postgres
        options: >-
          --health-cmd pg_isready --health-interval 10s
          --health-timeout 5s --health-retries 5
        ports: ["5432:5432"]
    env:
      DATABASE_URL: postgres://postgres:postgres@localhost:5432/fubbik_rs
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo fmt --check
      - run: cargo clippy --all-targets -- -D warnings
      - run: cargo test --workspace
```

The stale-document check runs as part of `cargo test`, via `committed_openapi_json_is_current`.

Note: the CI service image `pgvector/pgvector:pg18` has no AGE, so migration 0001's `DO` block logs its notice and the AGE round-trip tests take their skip branch. That is expected — AGE coverage comes from the local `fubbik-rs-db` container. Do not "fix" CI by deleting those tests.

- [ ] **Step 8: Commit**

```bash
git add crates/ openapi.json .github/
git commit -m "feat(rust): OpenAPI generation with CI staleness guard"
```

---

### Task 14: Serve — HTTP server with embedded SPA

**Files:**
- Create: `crates/fubbik-api/src/assets.rs`
- Modify: `crates/fubbik-api/src/lib.rs`, `crates/fubbik/src/main.rs`

**Interfaces:**
- Consumes: `router` (Task 10), `AppState` (Task 6).
- Produces: a working `fubbik serve` binding port 3100, serving the API plus the SPA with history fallback.

- [ ] **Step 1: Write the failing fallback test**

`crates/fubbik-api/tests/assets.rs`:

```rust
use axum::body::Body;
use axum::http::{Request, StatusCode};
use tower::ServiceExt;

#[sqlx::test]
async fn unknown_api_path_is_404_not_spa_fallback(pool: sqlx::PgPool) {
    let app = fubbik_api::router(fubbik_api::AppState { pool, implicit_dev_session: true });

    let res = app
        .oneshot(Request::get("/api/does-not-exist").body(Body::empty()).unwrap())
        .await
        .unwrap();

    // The SPA fallback must never swallow unmatched API routes — doing so
    // returns HTML to a fetch() caller and produces confusing parse errors.
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p fubbik-api --test assets`
Expected: FAIL — the fallback does not exist yet, so behaviour is undefined or the route is missing.

- [ ] **Step 3: Implement asset serving**

`crates/fubbik-api/src/assets.rs`:

```rust
use axum::http::{StatusCode, Uri, header};
use axum::response::{IntoResponse, Response};
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "$CARGO_MANIFEST_DIR/../../apps/web/dist/"]
struct Assets;

/// Serves an embedded asset, falling back to index.html so client-side
/// routes resolve. API paths are excluded: an unmatched /api/* must 404
/// rather than return HTML.
pub async fn serve(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');

    if uri.path().starts_with("/api/") {
        return StatusCode::NOT_FOUND.into_response();
    }

    if let Some(file) = Assets::get(path) {
        let mime = mime_guess::from_path(path).first_or_octet_stream();
        return ([(header::CONTENT_TYPE, mime.as_ref())], file.data).into_response();
    }

    match Assets::get("index.html") {
        Some(index) => (
            [(header::CONTENT_TYPE, "text/html")],
            index.data,
        )
            .into_response(),
        // A binary built without the web app still serves the API.
        None => (StatusCode::NOT_FOUND, "web UI not bundled").into_response(),
    }
}
```

Add to `fubbik-api` dependencies: `rust-embed = "8"`, `mime_guess = "2"`.

Note: `rust-embed` requires the folder to exist at compile time. Create `apps/web/dist/.gitkeep` so a fresh clone compiles before the web app is built.

Update `crates/fubbik-api/src/lib.rs`:

```rust
pub mod assets;

pub fn router(state: AppState) -> Router {
    Router::new()
        .merge(auth::routes::router())
        .merge(chunks::routes::router())
        .with_state(state)
        .fallback(assets::serve)
}
```

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p fubbik-api --test assets`
Expected: PASS

- [ ] **Step 5: Wire up `serve`**

Replace the `Serve` arm in `crates/fubbik/src/main.rs`:

```rust
        Commands::Serve { port } => {
            let database_url = std::env::var("DATABASE_URL")
                .map_err(|_| anyhow::anyhow!("DATABASE_URL is required"))?;
            let implicit_dev_session =
                std::env::var("FUBBIK_IMPLICIT_DEV_SESSION").as_deref() == Ok("true");

            let pool = fubbik_db::connect(&database_url).await?;
            let state = fubbik_api::AppState { pool, implicit_dev_session };

            let cors = tower_http::cors::CorsLayer::new()
                .allow_origin(
                    std::env::var("CORS_ORIGIN")
                        .unwrap_or_else(|_| "http://localhost:3001".into())
                        .parse::<axum::http::HeaderValue>()?,
                )
                .allow_credentials(true)
                .allow_methods(tower_http::cors::Any)
                .allow_headers(tower_http::cors::Any);

            let app = fubbik_api::router(state).layer(cors);
            let listener = tokio::net::TcpListener::bind(("0.0.0.0", port)).await?;
            tracing::info!("fubbik listening on http://localhost:{port}");
            axum::serve(listener, app).await?;
            Ok(())
        }
```

Add `fubbik-db`, `tower-http`, and `axum` to the binary's dependencies.

- [ ] **Step 6: Verify manually**

```bash
DATABASE_URL=postgres://postgres:password@localhost:5434/fubbik_rs FUBBIK_IMPLICIT_DEV_SESSION=true cargo run -- serve
curl -s localhost:3100/api/chunks
```

Expected: `[]` — an empty JSON array, not a 401 or HTML.

- [ ] **Step 7: Commit**

```bash
git add crates/ apps/web/dist/.gitkeep
git commit -m "feat(rust): serve command with embedded SPA and history fallback"
```

---

### Task 15: TypeScript client — Eden-shaped Proxy over generated types

**Files:**
- Create: `apps/web/src/utils/api-types.ts` (generated)
- Modify: `apps/web/src/utils/api.ts`
- Create: `apps/web/src/utils/api.test.ts`
- Modify: `apps/web/package.json`

**Interfaces:**
- Consumes: `openapi.json` (Task 13).
- Produces: an `api` export preserving the existing call shape `api.api.chunks.get()`, `api.api.chunks({ id }).get()`, so the other 106 files that import it need no changes.

- [ ] **Step 1: Generate the types**

```bash
cd apps/web
pnpm add -D openapi-typescript
pnpm exec openapi-typescript ../../openapi.json -o src/utils/api-types.ts
```

Add a script to `apps/web/package.json`:

```json
"gen:api": "openapi-typescript ../../openapi.json -o src/utils/api-types.ts"
```

- [ ] **Step 2: Write the failing client test**

`apps/web/src/utils/api.test.ts`:

```ts
import { describe, expect, it, vi, beforeEach } from "vitest";
import { createClient } from "./api";

describe("proxy api client", () => {
    beforeEach(() => {
        vi.stubGlobal("fetch", vi.fn(async () => new Response(JSON.stringify({ ok: true }), {
            status: 200,
            headers: { "content-type": "application/json" }
        })));
    });

    it("builds a path from property access", async () => {
        const api = createClient("http://x.test");
        await api.api.chunks.get();
        expect(fetch).toHaveBeenCalledWith(
            "http://x.test/api/chunks",
            expect.objectContaining({ method: "GET", credentials: "include" })
        );
    });

    it("interpolates path params from a call segment", async () => {
        const api = createClient("http://x.test");
        await api.api.chunks({ id: "abc" }).get();
        expect(fetch).toHaveBeenCalledWith(
            "http://x.test/api/chunks/abc",
            expect.objectContaining({ method: "GET" })
        );
    });

    it("converts camelCase segments to kebab-case paths", async () => {
        const api = createClient("http://x.test");
        await api.api.chunks({ id: "abc" })["applies-to"].get();
        expect(fetch).toHaveBeenCalledWith(
            "http://x.test/api/chunks/abc/applies-to",
            expect.anything()
        );
    });

    it("sends a JSON body on post", async () => {
        const api = createClient("http://x.test");
        await api.api.chunks.post({ title: "T" });
        expect(fetch).toHaveBeenCalledWith(
            "http://x.test/api/chunks",
            expect.objectContaining({
                method: "POST",
                body: JSON.stringify({ title: "T" })
            })
        );
    });

    it("serialises query params", async () => {
        const api = createClient("http://x.test");
        await api.api.chunks.get({ query: { type: "note", limit: "10" } });
        expect(fetch).toHaveBeenCalledWith(
            "http://x.test/api/chunks?type=note&limit=10",
            expect.anything()
        );
    });

    it("returns { data, error } like eden", async () => {
        const api = createClient("http://x.test");
        const res = await api.api.chunks.get();
        expect(res).toEqual({ data: { ok: true }, error: null });
    });

    it("puts the payload in error on a failed response", async () => {
        vi.stubGlobal("fetch", vi.fn(async () => new Response(JSON.stringify({ message: "nope" }), {
            status: 404,
            headers: { "content-type": "application/json" }
        })));
        const api = createClient("http://x.test");
        const res = await api.api.chunks.get();
        expect(res.data).toBeNull();
        expect(res.error).toEqual({ status: 404, value: { message: "nope" } });
    });
});
```

- [ ] **Step 3: Run to verify it fails**

Run: `cd apps/web && pnpm vitest run src/utils/api.test.ts`
Expected: FAIL — `createClient` not exported.

- [ ] **Step 4: Implement the client**

Replace `apps/web/src/utils/api.ts`:

```ts
import { env } from "@fubbik/env/web";

import type { paths } from "./api-types";

type Method = "get" | "post" | "patch" | "put" | "delete";

const METHODS: readonly string[] = ["get", "post", "patch", "put", "delete"];

export interface EdenLikeResponse<T> {
    data: T | null;
    error: { status: number; value: unknown } | null;
}

/**
 * Recursively maps the generated OpenAPI `paths` object into Eden's
 * property-access call shape, so existing call sites keep compiling.
 *
 * `api.api.chunks({ id }).get()` maps to GET /api/chunks/{id}.
 */
type PathSegments<P extends string> = P extends `/${infer Head}/${infer Rest}`
    ? [Head, ...PathSegments<`/${Rest}`>]
    : P extends `/${infer Last}`
      ? [Last]
      : [];

type Client = {
    [segment: string]: Client & ((params: Record<string, string>) => Client) & {
        [M in Method]: (
            body?: unknown,
            options?: { query?: Record<string, string | undefined> }
        ) => Promise<EdenLikeResponse<unknown>>;
    };
};

function buildUrl(base: string, segments: string[], query?: Record<string, string | undefined>): string {
    const path = segments.join("/");
    const url = `${base}/${path}`;
    if (!query) return url;

    const search = new URLSearchParams();
    for (const [k, v] of Object.entries(query)) {
        if (v !== undefined) search.set(k, v);
    }
    const qs = search.toString();
    return qs ? `${url}?${qs}` : url;
}

async function request(
    base: string,
    segments: string[],
    method: Method,
    body?: unknown,
    options?: { query?: Record<string, string | undefined> }
): Promise<EdenLikeResponse<unknown>> {
    // GET takes its query from the first argument; other verbs take a body.
    const query = method === "get" ? (body as { query?: Record<string, string> })?.query : options?.query;
    const payload = method === "get" ? undefined : body;

    const res = await fetch(buildUrl(base, segments, query), {
        method: method.toUpperCase(),
        credentials: "include",
        ...(payload === undefined
            ? {}
            : { headers: { "content-type": "application/json" }, body: JSON.stringify(payload) })
    });

    const value = res.headers.get("content-type")?.includes("application/json")
        ? await res.json()
        : await res.text();

    return res.ok
        ? { data: value, error: null }
        : { data: null, error: { status: res.status, value } };
}

export function createClient(base: string): Client {
    const make = (segments: string[]): unknown =>
        new Proxy(function () {} as unknown as object, {
            get(_target, prop: string) {
                if (METHODS.includes(prop)) {
                    return (body?: unknown, options?: { query?: Record<string, string> }) =>
                        request(base, segments, prop as Method, body, options);
                }
                return make([...segments, prop]);
            },
            // A call segment supplies path params: chunks({ id: "abc" })
            apply(_target, _this, args: [Record<string, string>]) {
                const values = Object.values(args[0] ?? {});
                return make([...segments, ...values]);
            }
        });

    return make([]) as Client;
}

export const api = createClient(env.VITE_SERVER_URL);

// Referenced so the generated types participate in type-checking even
// though the Proxy is dynamically typed at the boundary.
export type ApiPaths = paths;
export type ApiPathSegments<P extends keyof paths & string> = PathSegments<P>;
```

- [ ] **Step 5: Run to verify all seven tests pass**

Run: `cd apps/web && pnpm vitest run src/utils/api.test.ts`
Expected: PASS

- [ ] **Step 6: Type-check the web app**

Run: `cd apps/web && pnpm check-types`
Expected: PASS. If call sites fail, fix them individually — do not weaken the `Client` type to `any` to make errors disappear.

- [ ] **Step 7: Commit**

```bash
git add apps/web/src/utils apps/web/package.json
git commit -m "feat(web): proxy API client over generated OpenAPI types"
```

---

### Task 16: Web migration — auth client and SSR removal

**Files:**
- Modify: `apps/web/src/lib/auth-client.ts`
- Delete: `apps/web/src/entry-server.ts`, `apps/web/src/functions/get-user.ts`
- Modify: `apps/web/vite.config.ts`, `apps/web/package.json`

**Interfaces:**
- Consumes: auth routes (Task 7), api client (Task 15).
- Produces: a static SPA build at `apps/web/dist/` for Task 14 to embed.

- [ ] **Step 1: Find every consumer of the removed pieces**

```bash
cd apps/web
grep -rn "get-user\|getUser" src
grep -rn "authClient" src | head -30
```

Record the list. Each one needs a replacement in Step 3.

- [ ] **Step 2: Write the failing auth-client test**

`apps/web/src/lib/auth-client.test.ts`:

```ts
import { describe, expect, it, vi, beforeEach } from "vitest";
import { authClient } from "./auth-client";

describe("auth client", () => {
    beforeEach(() => {
        vi.stubGlobal("fetch", vi.fn(async () => new Response(
            JSON.stringify({ id: "u1", email: "a@b.test", name: "Alice" }),
            { status: 200, headers: { "content-type": "application/json" } }
        )));
    });

    it("signs in against the rust auth route", async () => {
        await authClient.signIn.email({ email: "a@b.test", password: "hunter22" });
        expect(fetch).toHaveBeenCalledWith(
            expect.stringContaining("/api/auth/sign-in/email"),
            expect.objectContaining({ method: "POST", credentials: "include" })
        );
    });

    it("returns null from getSession when unauthenticated", async () => {
        vi.stubGlobal("fetch", vi.fn(async () => new Response("", { status: 401 })));
        expect(await authClient.getSession()).toBeNull();
    });
});
```

- [ ] **Step 3: Run to verify it fails**

Run: `cd apps/web && pnpm vitest run src/lib/auth-client.test.ts`
Expected: FAIL — the current module exports a better-auth client with a different shape.

- [ ] **Step 4: Replace the auth client**

`apps/web/src/lib/auth-client.ts`:

```ts
import { env } from "@fubbik/env/web";

export interface SessionUser {
    id: string;
    email: string;
    name: string;
}

async function post(path: string, body?: unknown): Promise<Response> {
    return fetch(`${env.VITE_SERVER_URL}${path}`, {
        method: "POST",
        credentials: "include",
        ...(body === undefined
            ? {}
            : { headers: { "content-type": "application/json" }, body: JSON.stringify(body) })
    });
}

export const authClient = {
    signIn: {
        async email(input: { email: string; password: string }): Promise<SessionUser> {
            const res = await post("/api/auth/sign-in/email", input);
            if (!res.ok) throw new Error("Invalid email or password");
            return res.json();
        }
    },
    signUp: {
        async email(input: { email: string; password: string; name: string }): Promise<SessionUser> {
            const res = await post("/api/auth/sign-up/email", input);
            if (!res.ok) throw new Error("Sign up failed");
            return res.json();
        }
    },
    async signOut(): Promise<void> {
        await post("/api/auth/sign-out");
    },
    /** Returns null when unauthenticated rather than throwing. */
    async getSession(): Promise<SessionUser | null> {
        const res = await fetch(`${env.VITE_SERVER_URL}/api/auth/get-session`, {
            credentials: "include"
        });
        return res.ok ? res.json() : null;
    }
};
```

- [ ] **Step 5: Run to verify it passes**

Run: `cd apps/web && pnpm vitest run src/lib/auth-client.test.ts`
Expected: PASS

- [ ] **Step 6: Remove SSR**

Delete `src/entry-server.ts` and `src/functions/get-user.ts`. Replace every `getUser()` call found in Step 1 with `authClient.getSession()`, and convert the two route loaders to client-side queries using the existing `useApiQuery` hook.

In `vite.config.ts`, disable SSR on the TanStack Start plugin:

```ts
tanstackStart({ spa: { enabled: true }, ssr: { enabled: false } })
```

- [ ] **Step 7: Build and verify the output is static**

```bash
cd apps/web && pnpm build
ls dist/index.html
```

Expected: `dist/index.html` exists. If the build emits a server bundle, SSR is still enabled — fix the config before proceeding.

- [ ] **Step 8: Verify end to end against the Rust server**

```bash
DATABASE_URL=postgres://postgres:password@localhost:5434/fubbik_rs cargo run -- serve
```

Open `http://localhost:3100`, sign up, create a chunk, reload the page, confirm it persists.

- [ ] **Step 9: Commit**

```bash
git add apps/web
git commit -m "feat(web): plain-fetch auth client and static SPA build"
```

---

### Task 17: CLI commands

**Files:**
- Create: `crates/fubbik-cli/Cargo.toml`, `src/lib.rs`, `src/client.rs`, `src/commands/{mod,add,get,list,search,health}.rs`
- Modify: `crates/fubbik/src/main.rs`

**Interfaces:**
- Consumes: the HTTP API (Tasks 10-12).
- Produces: `fubbik_cli::Command` enum and `fubbik_cli::run(cmd, base_url) -> anyhow::Result<()>`; subcommands `add`, `get`, `list`, `search`, `health`.

`fubbik-cli` must not depend on `fubbik-db`.

- [ ] **Step 1: Write the failing client test**

`crates/fubbik-cli/tests/client.rs`:

```rust
use fubbik_cli::client::Client;

#[tokio::test]
async fn list_builds_the_expected_query_string() {
    let server = wiremock::MockServer::start().await;

    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .and(wiremock::matchers::path("/api/chunks"))
        .and(wiremock::matchers::query_param("type", "note"))
        .and(wiremock::matchers::query_param("limit", "10"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
        .mount(&server)
        .await;

    let client = Client::new(server.uri());
    let chunks = client.list_chunks(Some("note"), None, 10).await.unwrap();
    assert!(chunks.is_empty());
}

#[tokio::test]
async fn surfaces_server_errors_as_anyhow() {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .respond_with(wiremock::ResponseTemplate::new(401))
        .mount(&server)
        .await;

    let client = Client::new(server.uri());
    let err = client.list_chunks(None, None, 50).await.unwrap_err();
    assert!(err.to_string().contains("401"), "error should mention the status: {err}");
}
```

Add dev-dependencies: `wiremock = "0.6"`, `tokio` with `macros` and `rt-multi-thread`.

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p fubbik-cli`
Expected: FAIL — package does not exist.

- [ ] **Step 3: Implement the HTTP client**

`crates/fubbik-cli/Cargo.toml`:

```toml
[package]
name = "fubbik-cli"
edition.workspace = true
version.workspace = true

[dependencies]
anyhow.workspace = true
clap.workspace = true
comfy-table = "7"
owo-colors = "4"
reqwest = { version = "0.12", features = ["json", "cookies"] }
serde.workspace = true
serde_json.workspace = true
tokio.workspace = true

[dev-dependencies]
wiremock = "0.6"
```

`crates/fubbik-cli/src/client.rs`:

```rust
use anyhow::{Context, Result, bail};

/// Mirrors the API's camelCase output. The CLI deserialises the wire format,
/// not the database row, so this must match `fubbik_db::repo::chunk::Chunk`'s
/// serde representation rather than its field names.
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Chunk {
    pub id: String,
    pub title: String,
    #[serde(rename = "type")]
    pub chunk_type: String,
    pub content: String,
    pub updated_at: String,
}

pub struct Client {
    base: String,
    http: reqwest::Client,
}

impl Client {
    pub fn new(base: impl Into<String>) -> Self {
        Self {
            base: base.into(),
            http: reqwest::Client::builder()
                .cookie_store(true)
                .build()
                .expect("http client builds"),
        }
    }

    async fn get_json<T: serde::de::DeserializeOwned>(&self, path: &str, query: &[(&str, String)]) -> Result<T> {
        let res = self
            .http
            .get(format!("{}{path}", self.base))
            .query(query)
            .send()
            .await
            .with_context(|| format!("could not reach fubbik at {}", self.base))?;

        if !res.status().is_success() {
            bail!("request to {path} failed with {}", res.status());
        }
        Ok(res.json().await?)
    }

    pub async fn list_chunks(&self, chunk_type: Option<&str>, search: Option<&str>, limit: u32) -> Result<Vec<Chunk>> {
        let mut query = vec![("limit", limit.to_string())];
        if let Some(t) = chunk_type {
            query.push(("type", t.to_string()));
        }
        if let Some(s) = search {
            query.push(("search", s.to_string()));
        }
        self.get_json("/api/chunks", &query).await
    }

    pub async fn get_chunk(&self, id: &str) -> Result<Chunk> {
        self.get_json(&format!("/api/chunks/{id}"), &[]).await
    }

    pub async fn create_chunk(&self, title: &str, content: &str, chunk_type: &str) -> Result<Chunk> {
        let res = self
            .http
            .post(format!("{}/api/chunks", self.base))
            .json(&serde_json::json!({ "title": title, "content": content, "type": chunk_type }))
            .send()
            .await
            .with_context(|| format!("could not reach fubbik at {}", self.base))?;

        if !res.status().is_success() {
            bail!("create failed with {}", res.status());
        }
        Ok(res.json().await?)
    }

    pub async fn health(&self) -> Result<serde_json::Value> {
        self.get_json("/api/health", &[]).await
    }
}
```

- [ ] **Step 4: Run to verify the client tests pass**

Run: `cargo test -p fubbik-cli`
Expected: PASS

- [ ] **Step 5: Add a health endpoint to the API**

The CLI's `health` command needs a server endpoint. In `crates/fubbik-api/src/lib.rs`:

```rust
async fn health(State(state): State<AppState>) -> AppResult<Json<serde_json::Value>> {
    let db_ok = sqlx::query("SELECT 1").execute(&state.pool).await.is_ok();
    Ok(Json(serde_json::json!({
        "status": if db_ok { "ok" } else { "degraded" },
        "database": db_ok,
        "version": env!("CARGO_PKG_VERSION"),
    })))
}
```

Mount it with `.route("/api/health", get(health))` before `.with_state(state)`. Health is deliberately unauthenticated.

- [ ] **Step 6: Implement the commands**

`crates/fubbik-cli/src/lib.rs`:

```rust
pub mod client;
pub mod commands;

use anyhow::Result;
use clap::Subcommand;

#[derive(Subcommand)]
pub enum Command {
    /// Create a chunk
    Add {
        title: String,
        #[arg(short, long, default_value = "")]
        content: String,
        #[arg(short = 't', long, default_value = "note")]
        r#type: String,
    },
    /// Show a chunk by id
    Get { id: String },
    /// List chunks
    List {
        #[arg(short = 't', long)]
        r#type: Option<String>,
        #[arg(short, long, default_value = "50")]
        limit: u32,
    },
    /// Search chunks by text
    Search {
        query: String,
        #[arg(short, long, default_value = "50")]
        limit: u32,
    },
    /// Check server health
    Health,
}

pub async fn run(cmd: Command, base_url: &str) -> Result<()> {
    let client = client::Client::new(base_url);
    match cmd {
        Command::Add { title, content, r#type } => commands::add::run(&client, &title, &content, &r#type).await,
        Command::Get { id } => commands::get::run(&client, &id).await,
        Command::List { r#type, limit } => commands::list::run(&client, r#type.as_deref(), limit).await,
        Command::Search { query, limit } => commands::search::run(&client, &query, limit).await,
        Command::Health => commands::health::run(&client).await,
    }
}
```

`crates/fubbik-cli/src/commands/mod.rs`:

```rust
pub mod add;
pub mod get;
pub mod health;
pub mod list;
pub mod search;

use crate::client::Chunk;
use comfy_table::{Table, presets::UTF8_FULL};

/// Shared table renderer so every listing command formats identically.
pub fn render_table(chunks: &[Chunk]) {
    if chunks.is_empty() {
        println!("No chunks found.");
        return;
    }

    let mut table = Table::new();
    table.load_preset(UTF8_FULL);
    table.set_header(vec!["ID", "Type", "Title", "Updated"]);
    for c in chunks {
        table.add_row(vec![
            c.id.chars().take(8).collect::<String>(),
            c.chunk_type.clone(),
            c.title.chars().take(60).collect::<String>(),
            c.updated_at.chars().take(10).collect::<String>(),
        ]);
    }
    println!("{table}");
}
```

`crates/fubbik-cli/src/commands/list.rs`:

```rust
use anyhow::Result;

use crate::client::Client;

pub async fn run(client: &Client, chunk_type: Option<&str>, limit: u32) -> Result<()> {
    let chunks = client.list_chunks(chunk_type, None, limit).await?;
    super::render_table(&chunks);
    Ok(())
}
```

`crates/fubbik-cli/src/commands/search.rs`:

```rust
use anyhow::Result;

use crate::client::Client;

pub async fn run(client: &Client, query: &str, limit: u32) -> Result<()> {
    let chunks = client.list_chunks(None, Some(query), limit).await?;
    super::render_table(&chunks);
    Ok(())
}
```

`crates/fubbik-cli/src/commands/add.rs`:

```rust
use anyhow::Result;
use owo_colors::OwoColorize;

use crate::client::Client;

pub async fn run(client: &Client, title: &str, content: &str, chunk_type: &str) -> Result<()> {
    let chunk = client.create_chunk(title, content, chunk_type).await?;
    println!("{} {} {}", "created".green(), chunk.id.dimmed(), chunk.title);
    Ok(())
}
```

`crates/fubbik-cli/src/commands/get.rs`:

```rust
use anyhow::Result;
use owo_colors::OwoColorize;

use crate::client::Client;

pub async fn run(client: &Client, id: &str) -> Result<()> {
    let chunk = client.get_chunk(id).await?;
    println!("{}", chunk.title.bold());
    println!("{} {}", "type:".dimmed(), chunk.chunk_type);
    println!("{} {}", "id:".dimmed(), chunk.id);
    println!();
    println!("{}", chunk.content);
    Ok(())
}
```

`crates/fubbik-cli/src/commands/health.rs`:

```rust
use anyhow::Result;
use owo_colors::OwoColorize;

use crate::client::Client;

pub async fn run(client: &Client) -> Result<()> {
    let health = client.health().await?;
    let status = health["status"].as_str().unwrap_or("unknown");
    let rendered = if status == "ok" { status.green().to_string() } else { status.red().to_string() };
    println!("status:   {rendered}");
    println!("database: {}", health["database"]);
    println!("version:  {}", health["version"].as_str().unwrap_or("?"));
    Ok(())
}
```

- [ ] **Step 7: Wire the commands into the binary**

In `crates/fubbik/src/main.rs`, flatten the CLI commands into the top-level enum:

```rust
#[derive(Subcommand)]
enum Commands {
    Serve {
        #[arg(long, env = "PORT", default_value = "3100")]
        port: u16,
    },
    Mcp,
    Openapi,
    #[command(flatten)]
    Cli(fubbik_cli::Command),
}
```

And the arm:

```rust
        Commands::Cli(cmd) => {
            let base = std::env::var("FUBBIK_URL").unwrap_or_else(|_| "http://localhost:3100".into());
            fubbik_cli::run(cmd, &base).await
        }
```

Add `fubbik-cli` to the binary's dependencies.

- [ ] **Step 8: Verify end to end**

```bash
cargo run -- add "CLI works" --content "created from the cli"
cargo run -- list
cargo run -- search "CLI works"
cargo run -- health
```

Expected: the chunk appears in `list` and `search`; `health` reports `ok`.

- [ ] **Step 9: Commit**

```bash
git add crates/
git commit -m "feat(rust): cli with add, get, list, search, health"
```

---

### Task 18: Differential harness

**Files:**
- Create: `crates/fubbik-api/tests/differential.rs`
- Create: `scripts/differential.sh`

**Interfaces:**
- Consumes: a running Node server (port 3000) and Rust server (port 3100).
- Produces: `cargo test -p fubbik-api --test differential -- --ignored`, which diffs responses between the two stacks.

The harness skips silently unless both `FUBBIK_NODE_URL` and `FUBBIK_RUST_URL` are set, so it never breaks a normal `cargo test`.

- [ ] **Step 1: Write the harness**

`crates/fubbik-api/tests/differential.rs`:

```rust
//! Compares Node and Rust responses for the ported endpoints.
//!
//! Run with both stacks live:
//!   FUBBIK_NODE_URL=http://localhost:3000 \
//!   FUBBIK_RUST_URL=http://localhost:3100 \
//!   cargo test -p fubbik-api --test differential -- --ignored --nocapture

use serde_json::Value;

fn urls() -> Option<(String, String)> {
    Some((std::env::var("FUBBIK_NODE_URL").ok()?, std::env::var("FUBBIK_RUST_URL").ok()?))
}

/// Removes values that legitimately differ between stacks: generated IDs and
/// timestamps. Comparing them would produce noise, not signal.
fn normalise(value: &mut Value) {
    match value {
        Value::Object(map) => {
            for key in ["id", "createdAt", "updatedAt", "userId"] {
                map.remove(key);
            }
            for (_, v) in map.iter_mut() {
                normalise(v);
            }
        }
        Value::Array(items) => items.iter_mut().for_each(normalise),
        _ => {}
    }
}

async fn fetch(base: &str, path: &str) -> (u16, Value) {
    let res = reqwest::get(format!("{base}{path}")).await.expect("request succeeds");
    let status = res.status().as_u16();
    let body = res.json::<Value>().await.unwrap_or(Value::Null);
    (status, body)
}

async fn assert_same(path: &str) {
    let Some((node, rust)) = urls() else {
        eprintln!("skipping {path}: FUBBIK_NODE_URL / FUBBIK_RUST_URL not set");
        return;
    };

    let (node_status, mut node_body) = fetch(&node, path).await;
    let (rust_status, mut rust_body) = fetch(&rust, path).await;

    assert_eq!(node_status, rust_status, "status mismatch for {path}");

    normalise(&mut node_body);
    normalise(&mut rust_body);
    assert_eq!(node_body, rust_body, "body mismatch for {path}");
}

#[tokio::test]
#[ignore = "requires both stacks running"]
async fn chunk_endpoints_match() {
    for path in [
        "/api/chunks",
        "/api/chunks?type=note",
        "/api/chunks?limit=5",
        "/api/chunks?sort=alpha",
        "/api/chunks?search=convention",
    ] {
        assert_same(path).await;
    }
}
```

Add `reqwest` to `fubbik-api` dev-dependencies.

- [ ] **Step 2: Write the runner script**

`scripts/differential.sh`:

```bash
#!/usr/bin/env bash
# Runs the differential harness against both stacks.
# Assumes the Node server is on :3000 and seeds the Rust database to match.
set -euo pipefail

echo "==> Seeding fubbik_rs from the Node database"
pg_dump --data-only --no-owner "${DATABASE_URL}" \
  | psql "postgres://postgres:password@localhost:5434/fubbik_rs" >/dev/null

echo "==> Starting the Rust server"
DATABASE_URL="postgres://postgres:password@localhost:5434/fubbik_rs" \
  FUBBIK_IMPLICIT_DEV_SESSION=true \
  cargo run --quiet -- serve &
RUST_PID=$!
trap 'kill $RUST_PID 2>/dev/null || true' EXIT

until curl -sf http://localhost:3100/api/health >/dev/null; do sleep 0.5; done

echo "==> Diffing"
FUBBIK_NODE_URL=http://localhost:3000 \
  FUBBIK_RUST_URL=http://localhost:3100 \
  cargo test -p fubbik-api --test differential -- --ignored --nocapture
```

Then: `chmod +x scripts/differential.sh`

- [ ] **Step 3: Verify the harness skips cleanly by default**

Run: `cargo test -p fubbik-api --test differential`
Expected: PASS with the test reported as ignored.

- [ ] **Step 4: Run it for real**

Start the Node stack (`pnpm dev:server`), then:

Run: `./scripts/differential.sh`
Expected: PASS. Any mismatch is a genuine parity bug — fix the Rust side, since Node is the reference.

- [ ] **Step 5: Record the results**

Note in the commit message which paths matched and which required normalisation beyond IDs and timestamps. Phase 2 inherits this harness and needs to know its known-weak spots.

- [ ] **Step 6: Commit**

```bash
git add crates/fubbik-api/tests scripts/
git commit -m "test(rust): differential harness comparing node and rust responses"
```

---

## Phase 1 Exit Criteria

Verify all of these before starting Phase 2:

- [ ] `cargo test --workspace` passes.
- [ ] `cargo clippy --all-targets -- -D warnings` is clean.
- [ ] `./scripts/differential.sh` reports no mismatches for the chunk endpoints.
- [ ] `cargo run -- serve` serves the web UI at `http://localhost:3100` with working sign-up, sign-in, chunk create, chunk edit, and chunk delete.
- [ ] `cargo run -- add/get/list/search/health` all work against the running server.
- [ ] `openapi.json` is committed and current.
- [ ] The spec's Risks section records the AGE spike outcome.
