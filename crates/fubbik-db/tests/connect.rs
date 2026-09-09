//! Exercises `fubbik_db::connect()` directly. `crates/fubbik/src/main.rs`
//! is otherwise the only caller, and `connect()` is exactly where the
//! restart-breaking `search_path` defect previously lived (see the
//! doc comment on `connect()` itself): the `after_connect` hook loads AGE
//! on every pooled connection, and an earlier version also `SET
//! search_path`'d there, which mutated the session for whichever
//! connection `sqlx::migrate!` happened to pull from the pool next,
//! breaking migration bookkeeping on every subsequent server start.
//!
//! `#[sqlx::test]` provisions its own pool straight from a template
//! database and never calls `connect()`, so it cannot catch a regression
//! here. This test instead provisions a scratch database by hand — with a
//! randomized name so it never collides with another test run happening in
//! parallel — connects to it via `connect()` for real, and cleans up
//! afterward regardless of whether the checks pass.

use sqlx::Row;
use sqlx::postgres::PgPoolOptions;

/// Builds the URL for `db_name` by swapping the path component of `base`,
/// preserving any query string. `DATABASE_URL` in this workspace never
/// carries one in practice, but this keeps the helper honest either way.
fn url_for_database(base: &str, db_name: &str) -> String {
    let (before_query, query) = match base.split_once('?') {
        Some((b, q)) => (b, Some(q)),
        None => (base, None),
    };
    let last_slash = before_query
        .rfind('/')
        .expect("DATABASE_URL must contain a path segment naming a database");
    let mut url = format!("{}/{db_name}", &before_query[..last_slash]);
    if let Some(q) = query {
        url.push('?');
        url.push_str(q);
    }
    url
}

/// Runs the actual assertions and returns `Err` instead of panicking, so
/// the caller can guarantee the scratch database is dropped even when a
/// check fails.
async fn check_connect_against(scratch_url: &str) -> Result<(), String> {
    // First call: the restart-safety assertion below only means something
    // if this exercises `connect()` for real, not a hand-rolled pool.
    let pool = fubbik_db::connect(scratch_url)
        .await
        .map_err(|e| format!("connect() failed against a freshly created database: {e}"))?;

    // The pool must be usable: run a trivial query through it.
    let one: i32 = sqlx::query_scalar("SELECT 1")
        .fetch_one(&pool)
        .await
        .map_err(|e| format!("trivial query through connect()'s pool failed: {e}"))?;
    if one != 1 {
        return Err(format!("expected SELECT 1 to return 1, got {one}"));
    }

    // Migrations must have been applied: sqlx's bookkeeping table exists
    // and has rows, and a table migration 0001 creates (`chunk`) is
    // queryable.
    let migration_count: i64 = sqlx::query_scalar("SELECT count(*) FROM _sqlx_migrations")
        .fetch_one(&pool)
        .await
        .map_err(|e| format!("_sqlx_migrations bookkeeping table missing after connect(): {e}"))?;
    if migration_count == 0 {
        return Err("connect() must have applied at least one migration".into());
    }

    let chunk_count: i64 = sqlx::query_scalar("SELECT count(*) FROM chunk")
        .fetch_one(&pool)
        .await
        .map_err(|e| format!("migration-created `chunk` table is not queryable: {e}"))?;
    if chunk_count != 0 {
        return Err(format!(
            "freshly migrated database should start empty, found {chunk_count} chunk rows"
        ));
    }

    // Application relations must never inherit AGE's `ag_catalog` schema.
    // This is an observable migration contract: every runtime query resolves
    // these names through the default public search path.
    for relation in [
        "agent_run",
        "plan_task_claim",
        "coordination_entry",
        "projection_outbox",
        "account",
    ] {
        let schema: Option<String> = sqlx::query_scalar(
            "SELECT n.nspname \
             FROM pg_class c \
             JOIN pg_namespace n ON n.oid = c.relnamespace \
             WHERE c.relname = $1 AND c.relkind IN ('r', 'p')",
        )
        .bind(relation)
        .fetch_optional(&pool)
        .await
        .map_err(|e| format!("look up schema for `{relation}`: {e}"))?
        .flatten();
        if schema.as_deref() != Some("public") {
            return Err(format!(
                "migration-created relation `{relation}` must be in public, found {schema:?}"
            ));
        }
    }

    // The regression that mattered: `after_connect` must not leave a
    // mutated `search_path` on a connection handed back to the pool.
    // Queried on `pool` itself — the actual pool `connect()` returned,
    // with its own `after_connect` hook installed — not a freshly built
    // sibling pool. A sibling pool never runs `connect()`'s hook at all,
    // so it would report a clean `search_path` regardless of what the
    // hook does; that check would pass even with the historical bug
    // reintroduced; only a connection that actually came out of this pool
    // can prove anything about this hook.
    let search_path: String = sqlx::query("SHOW search_path")
        .fetch_one(&pool)
        .await
        .map_err(|e| format!("SHOW search_path failed: {e}"))?
        .try_get(0)
        .map_err(|e| format!("reading search_path column failed: {e}"))?;
    if search_path.contains("ag_catalog") {
        return Err(format!(
            "after_connect must not leave ag_catalog on the pooled connection's \
             search_path, got: {search_path}"
        ));
    }
    pool.close().await;

    // Restart scenario: `fubbik serve` calls `connect()` exactly once per
    // process start, so the defect this guards against only shows up
    // across separate calls against the same already-migrated database,
    // mirroring a server restart.
    let pool2 = fubbik_db::connect(scratch_url).await.map_err(|e| {
        format!("connect() failed on a second call against an already-migrated database: {e}")
    })?;
    let two: i32 = sqlx::query_scalar("SELECT 2")
        .fetch_one(&pool2)
        .await
        .map_err(|e| format!("query through the second connect()'s pool failed: {e}"))?;
    if two != 2 {
        return Err(format!("expected SELECT 2 to return 2, got {two}"));
    }
    pool2.close().await;

    Ok(())
}

/// Builds the database shape the retired Node runtime leaves behind: the
/// application baseline and Drizzle's bookkeeping table exist, but SQLx has
/// never recorded a migration. `connect()` must adopt that proven baseline,
/// preserve its rows, and apply every later Rust migration.
async fn check_connect_against_legacy_drizzle_database(scratch_url: &str) -> Result<(), String> {
    let legacy_pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(scratch_url)
        .await
        .map_err(|e| format!("connect to legacy scratch database: {e}"))?;

    sqlx::raw_sql("CREATE EXTENSION IF NOT EXISTS vector; CREATE EXTENSION IF NOT EXISTS pg_trgm;")
        .execute(&legacy_pool)
        .await
        .map_err(|e| format!("install extensions required by Drizzle: {e}"))?;
    for migration in [
        include_str!("../../../packages/db/src/migrations/0000_baseline.sql"),
        include_str!("../../../packages/db/src/migrations/0001_seed_builtin_catalogs.sql"),
        include_str!("../../../packages/db/src/migrations/0003_connection_weight.sql"),
        include_str!("../../../packages/db/src/migrations/0004_codebase_to_space.sql"),
    ] {
        sqlx::raw_sql(migration)
            .execute(&legacy_pool)
            .await
            .map_err(|e| format!("apply checked-in Drizzle migration: {e}"))?;
    }
    // PostgreSQL rejects multiple CREATE INDEX CONCURRENTLY statements sent
    // as one implicit transaction, so execute this migration statement by
    // statement as Drizzle's migration runner does.
    for statement in
        include_str!("../../../packages/db/src/migrations/0002_graph_indexes.sql").split(';')
    {
        if !statement.trim().is_empty() {
            sqlx::query(statement)
                .execute(&legacy_pool)
                .await
                .map_err(|e| format!("apply Drizzle graph-index migration: {e}"))?;
        }
    }
    sqlx::raw_sql(
        r#"CREATE SCHEMA drizzle;
           CREATE TABLE drizzle.__drizzle_migrations (
               id serial PRIMARY KEY,
               hash text NOT NULL,
               created_at bigint
           );
           INSERT INTO drizzle.__drizzle_migrations (hash, created_at)
           VALUES ('legacy-baseline', 1);
           INSERT INTO "user" (id, name, email)
           VALUES ('legacy-user', 'Legacy User', 'legacy@example.test');"#,
    )
    .execute(&legacy_pool)
    .await
    .map_err(|e| format!("install Drizzle marker and legacy row: {e}"))?;
    legacy_pool.close().await;

    let pool = fubbik_db::connect(scratch_url)
        .await
        .map_err(|e| format!("connect() did not adopt the Drizzle baseline: {e}"))?;

    let migration_count: i64 = sqlx::query_scalar("SELECT count(*) FROM _sqlx_migrations")
        .fetch_one(&pool)
        .await
        .map_err(|e| format!("read adopted SQLx migration history: {e}"))?;
    if migration_count != 8 {
        return Err(format!(
            "expected all 8 SQLx migrations after adoption, found {migration_count}"
        ));
    }

    let legacy_name: String =
        sqlx::query_scalar(r#"SELECT name FROM "user" WHERE id = 'legacy-user'"#)
            .fetch_one(&pool)
            .await
            .map_err(|e| format!("legacy user was not preserved: {e}"))?;
    if legacy_name != "Legacy User" {
        return Err(format!(
            "legacy user changed during migration: {legacy_name:?}"
        ));
    }

    let coordination_table_exists: bool =
        sqlx::query_scalar("SELECT to_regclass('public.agent_run') IS NOT NULL")
            .fetch_one(&pool)
            .await
            .map_err(|e| format!("check post-baseline migration table: {e}"))?;
    if !coordination_table_exists {
        return Err("migration 0005 was not applied after baseline adoption".into());
    }

    pool.close().await;

    // A second startup proves the adopted checksum is the exact checksum
    // SQLx expects for migration 0001, not merely a row that let the first
    // run skip the baseline.
    let restarted = fubbik_db::connect(scratch_url)
        .await
        .map_err(|e| format!("connect() failed after Drizzle adoption restart: {e}"))?;
    let restarted_count: i64 = sqlx::query_scalar("SELECT count(*) FROM _sqlx_migrations")
        .fetch_one(&restarted)
        .await
        .map_err(|e| format!("read migration history after restart: {e}"))?;
    if restarted_count != 8 {
        return Err(format!(
            "expected 7 migrations after adoption restart, found {restarted_count}"
        ));
    }
    restarted.close().await;
    Ok(())
}

async fn check_connect_rejects_incomplete_drizzle_database(
    scratch_url: &str,
) -> Result<(), String> {
    let legacy_pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(scratch_url)
        .await
        .map_err(|e| format!("connect to incomplete scratch database: {e}"))?;
    sqlx::raw_sql(
        r#"CREATE SCHEMA drizzle;
           CREATE TABLE drizzle.__drizzle_migrations (id serial PRIMARY KEY);
           CREATE TABLE chunk (id text PRIMARY KEY);"#,
    )
    .execute(&legacy_pool)
    .await
    .map_err(|e| format!("install incomplete Drizzle schema: {e}"))?;
    legacy_pool.close().await;

    let error = fubbik_db::connect(scratch_url)
        .await
        .expect_err("an incomplete Drizzle schema must not be adopted");
    let message = error.to_string();
    if !message.contains("not compatible with the Rust baseline")
        || !message.contains("No SQLx migration history was written")
    {
        return Err(format!("unexpected adoption error: {message}"));
    }

    let verification_pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(scratch_url)
        .await
        .map_err(|e| format!("reconnect after refused adoption: {e}"))?;
    let sqlx_history_exists: bool =
        sqlx::query_scalar("SELECT to_regclass('public._sqlx_migrations') IS NOT NULL")
            .fetch_one(&verification_pool)
            .await
            .map_err(|e| format!("check refused adoption bookkeeping: {e}"))?;
    if sqlx_history_exists {
        return Err("refused adoption must not create SQLx migration history".into());
    }
    verification_pool.close().await;
    Ok(())
}

#[tokio::test]
async fn connect_installs_hook_runs_migrations_and_is_restart_safe() {
    let database_url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set to run this test against a real Postgres cluster");

    // Admin connection (to the `postgres` maintenance database) used only to
    // create and drop the scratch database this test owns end to end. This
    // test never touches `database_url`'s own database.
    let admin_url = url_for_database(&database_url, "postgres");
    let admin_pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(&admin_url)
        .await
        .expect("connect to the postgres maintenance database");

    // Randomized name so concurrent test runs (or a re-run right after a
    // panic left one behind) never collide on CREATE DATABASE.
    let db_name = format!("fubbik_connect_test_{}", fubbik_db::new_id());
    sqlx::query(&format!(r#"CREATE DATABASE "{db_name}""#))
        .execute(&admin_pool)
        .await
        .expect("create scratch database");

    let scratch_url = url_for_database(&database_url, &db_name);
    let result = check_connect_against(&scratch_url).await;

    // Clean up regardless of whether the checks above passed, so a failing
    // run doesn't leak a database.
    let _ = sqlx::query(
        "SELECT pg_terminate_backend(pid) FROM pg_stat_activity \
         WHERE datname = $1 AND pid <> pg_backend_pid()",
    )
    .bind(&db_name)
    .execute(&admin_pool)
    .await;
    sqlx::query(&format!(r#"DROP DATABASE IF EXISTS "{db_name}""#))
        .execute(&admin_pool)
        .await
        .expect("drop scratch database");

    result.expect("connect() checks");
}

#[tokio::test]
async fn connect_adopts_a_legacy_drizzle_database_and_preserves_data() {
    let database_url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set to run this test against a real Postgres cluster");
    let admin_url = url_for_database(&database_url, "postgres");
    let admin_pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(&admin_url)
        .await
        .expect("connect to the postgres maintenance database");

    let db_name = format!("fubbik_drizzle_adoption_test_{}", fubbik_db::new_id());
    sqlx::query(&format!(r#"CREATE DATABASE "{db_name}""#))
        .execute(&admin_pool)
        .await
        .expect("create scratch database");

    let scratch_url = url_for_database(&database_url, &db_name);
    let result = check_connect_against_legacy_drizzle_database(&scratch_url).await;

    let _ = sqlx::query(
        "SELECT pg_terminate_backend(pid) FROM pg_stat_activity \
         WHERE datname = $1 AND pid <> pg_backend_pid()",
    )
    .bind(&db_name)
    .execute(&admin_pool)
    .await;
    sqlx::query(&format!(r#"DROP DATABASE IF EXISTS "{db_name}""#))
        .execute(&admin_pool)
        .await
        .expect("drop scratch database");

    result.expect("legacy Drizzle adoption checks");
}

#[tokio::test]
async fn connect_refuses_to_adopt_an_incomplete_drizzle_database() {
    let database_url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set to run this test against a real Postgres cluster");
    let admin_url = url_for_database(&database_url, "postgres");
    let admin_pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(&admin_url)
        .await
        .expect("connect to the postgres maintenance database");

    let db_name = format!("fubbik_drizzle_refusal_test_{}", fubbik_db::new_id());
    sqlx::query(&format!(r#"CREATE DATABASE "{db_name}""#))
        .execute(&admin_pool)
        .await
        .expect("create scratch database");

    let scratch_url = url_for_database(&database_url, &db_name);
    let result = check_connect_rejects_incomplete_drizzle_database(&scratch_url).await;

    let _ = sqlx::query(
        "SELECT pg_terminate_backend(pid) FROM pg_stat_activity \
         WHERE datname = $1 AND pid <> pg_backend_pid()",
    )
    .bind(&db_name)
    .execute(&admin_pool)
    .await;
    sqlx::query(&format!(r#"DROP DATABASE IF EXISTS "{db_name}""#))
        .execute(&admin_pool)
        .await
        .expect("drop scratch database");

    result.expect("incomplete Drizzle refusal checks");
}
