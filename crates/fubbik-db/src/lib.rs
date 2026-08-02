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
                //
                // Deliberately does NOT also `SET search_path` here (as an
                // earlier version did): that mutates search_path for the
                // rest of the connection's session, including the
                // connection `sqlx::migrate!` below pulls from this same
                // pool. Once AGE is installed, every subsequent process
                // start would see `ag_catalog` ahead of `public` in the
                // search path before migrations run, causing sqlx's
                // unqualified `_sqlx_migrations` bookkeeping table to
                // resolve under `ag_catalog` instead of `public` — sqlx
                // then thinks no migrations have run and reapplies them,
                // colliding with tables migration 0001 already created.
                // `age::cypher` sets search_path itself, per call, on its
                // own acquired connection, so nothing here needs it.
                if let Err(e) = conn.execute("LOAD 'age';").await {
                    tracing::debug!("AGE not available: {e}");
                }
                Ok(())
            })
        })
        .connect_with(opts)
        .await?;

    sqlx::migrate!("./migrations").run(&pool).await?;
    Ok(pool)
}

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
