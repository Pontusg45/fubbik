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

    // The regression that mattered: `after_connect` must not leave a
    // mutated `search_path` on a connection handed back to the pool. Pin
    // `max_connections(1)` on a second pool to the same database so every
    // acquisition is guaranteed to reuse the exact physical connection
    // `connect()`'s own `after_connect` hook (and the migration run)
    // touched.
    let single = PgPoolOptions::new()
        .max_connections(1)
        .connect(scratch_url)
        .await
        .map_err(|e| format!("single-connection pool to the scratch database failed: {e}"))?;
    let search_path: String = sqlx::query("SHOW search_path")
        .fetch_one(&single)
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
    single.close().await;
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
