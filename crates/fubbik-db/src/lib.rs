pub mod age;
pub mod embedding;
pub mod repo;
pub mod timestamp;

use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::{Executor, PgPool, Row};
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

/// Checks the connected database's locale provider and, if it is not ICU
/// (`i`), emits a loud `tracing::warn!` naming the mismatch and the fix.
///
/// The reference implementation (the Node backend, on Homebrew Postgres)
/// runs on ICU, which ignores punctuation at the primary comparison level.
/// A database created with the libc provider instead compares raw bytes, so
/// e.g. `"Catalog tables:"` and `"Catalog-driven"` sort in opposite order
/// between the two — silently, with no error, on every `ORDER BY` over
/// text. This check does not fail startup: a wrong sort order must not take
/// the server down, but it must be impossible to miss in the logs.
///
/// Query failures (e.g. insufficient privilege on `pg_database`, though the
/// default `postgres` role can always read it) are logged at `debug` and
/// otherwise ignored — this is a diagnostic, not a required capability.
pub async fn warn_if_not_icu_collation(pool: &PgPool) {
    let row = match sqlx::query(
        "SELECT datlocprovider::text AS provider, datcollate, datlocale \
         FROM pg_database WHERE datname = current_database()",
    )
    .fetch_optional(pool)
    .await
    {
        Ok(Some(row)) => row,
        Ok(None) => return,
        Err(e) => {
            tracing::debug!("collation provider check failed: {e}");
            return;
        }
    };

    let provider: String = row.try_get("provider").unwrap_or_default();
    if provider != "i" {
        let collate: String = row.try_get("datcollate").unwrap_or_default();
        let locale: Option<String> = row.try_get("datlocale").unwrap_or(None);
        tracing::warn!(
            provider = %provider,
            collate = %collate,
            locale = ?locale,
            "DATABASE COLLATION MISMATCH: this database's locale provider is '{provider}' \
             (collate '{collate}', locale {locale:?}), not ICU ('i'). Text ordering (e.g. \
             `ORDER BY title`) will differ from the reference Node backend, which runs on \
             ICU — ICU ignores punctuation at the primary comparison level, while the libc \
             provider compares raw bytes, so \"Catalog tables:\" and \"Catalog-driven\" sort \
             in opposite order between the two. Fix: recreate this database with the ICU \
             locale provider, e.g. `CREATE DATABASE ... LOCALE_PROVIDER icu ICU_LOCALE \
             'en-US' TEMPLATE template0`, or set `POSTGRES_INITDB_ARGS=\"--locale-provider=icu \
             --icu-locale=en-US\"` before the Postgres cluster is first initialized. See \
             README.md's Database Setup section."
        );
    }
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
