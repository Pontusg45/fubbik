use fubbik_core::error::{AppError, AppResult};
use fubbik_db::repo::staleness::{self, ListParams, RawUpdateResult, StaleFlag};
use sqlx::PgPool;

pub async fn list(pool: &PgPool, user_id: &str, params: ListParams) -> AppResult<Vec<StaleFlag>> {
    staleness::list(pool, user_id, params).await
}

pub async fn count(pool: &PgPool, user_id: &str, space_id: Option<&str>) -> AppResult<i64> {
    staleness::count(pool, user_id, space_id).await
}

/// Turns the repo's `None` (id doesn't exist, or exists but isn't the
/// caller's — see divergence #14 on `staleness::dismiss`) into 404.
pub async fn dismiss(pool: &PgPool, user_id: &str, flag_id: &str) -> AppResult<RawUpdateResult> {
    staleness::dismiss(pool, user_id, flag_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Staleness flag".into()))
}

/// Turns the repo's `None` (caller doesn't own one or both chunks — this
/// port's own guard, see the doc comment on `staleness::suppress_duplicate`)
/// into 404, mirroring `favorites::service::add`'s "404 before insert"
/// precedent.
pub async fn suppress_duplicate(
    pool: &PgPool,
    user_id: &str,
    chunk_id_a: &str,
    chunk_id_b: &str,
) -> AppResult<RawUpdateResult> {
    staleness::suppress_duplicate(pool, user_id, chunk_id_a, chunk_id_b)
        .await?
        .ok_or_else(|| AppError::NotFound("Chunk".into()))
}

/// Runs **both** detectors and sums their newly-flagged counts, matching
/// Node's `Effect.all([detectAgeStaleChunks(...), detectUncoveredChunks(...)])`
/// (`packages/api/src/staleness/routes.ts:69-92`).
///
/// `threshold_days` only overrides the age detector — Node's route passes
/// `ctx.body.thresholdDays` to `detectAgeStaleChunks` alone and calls
/// `detectUncoveredChunks(session.user.id, ctx.body.spaceId)` with no third
/// argument at all, so the uncovered detector always uses its own default
/// (30 days), never the request body's `thresholdDays`. Reproduced exactly:
/// the body's `threshold_days` is never threaded into the
/// `detect_uncovered_chunks` call below.
pub async fn scan_age(
    pool: &PgPool,
    user_id: &str,
    space_id: Option<&str>,
    threshold_days: Option<i64>,
) -> AppResult<i64> {
    let aged =
        staleness::detect_age_stale_chunks(pool, user_id, space_id, threshold_days.unwrap_or(90))
            .await?;
    let uncovered = staleness::detect_uncovered_chunks(pool, user_id, space_id, 30).await?;
    Ok(aged + uncovered)
}

/// Turns the repo's `None` (caller doesn't own `chunk_id` — this port's own
/// guard, see the doc comment on `staleness::flag_impact_ripple`) into 404,
/// same "404 before write" shape as `dismiss`/`suppress_duplicate` above.
pub async fn scan_impact(
    pool: &PgPool,
    user_id: &str,
    chunk_id: &str,
    title: &str,
) -> AppResult<i64> {
    staleness::flag_impact_ripple(pool, user_id, chunk_id, title)
        .await?
        .ok_or_else(|| AppError::NotFound("Chunk".into()))
}

/// Parses `STALENESS_SCAN_INTERVAL_HOURS` into a scan interval, or `None`
/// when scanning should be disabled.
///
/// Node: `Number(env.STALENESS_SCAN_INTERVAL_HOURS ?? "24")`, disabled when
/// `intervalHours <= 0` (`packages/api/src/startup.ts:60-64`). This port
/// diverges on exactly one edge case: an unparseable non-empty value (e.g.
/// `"garbage"`) falls back to the 24h default here, whereas Node's
/// `Number("garbage")` is `NaN`, `NaN <= 0` is `false` (never disables),
/// and `setInterval(fn, NaN)` degrades to a ~1ms interval — an accidental
/// busy-loop, not a considered behaviour worth reproducing.
pub fn resolve_scan_interval(hours: Option<&str>) -> Option<std::time::Duration> {
    let hours: f64 = hours.and_then(|s| s.parse().ok()).unwrap_or(24.0);
    if hours <= 0.0 {
        None
    } else {
        Some(std::time::Duration::from_secs_f64(hours * 3600.0))
    }
}

/// Runs `detect_age_stale_chunks` once for the implicit dev user, logging
/// the result. Never `detect_uncovered_chunks` — that detector only runs
/// via the explicit `POST /api/chunks/stale/scan-age` route
/// (`packages/api/src/startup.ts:25-37`'s `runStaleScan` calls
/// `detectAgeStaleChunks` alone).
///
/// Looks the dev user up by email at call time rather than relying on a
/// fixed id: this workspace's Rust port has not yet ported Node's
/// `IMPLICIT_DEV_USER_ID = "dev-user"` bootstrap (`ensureImplicitDevUserRow`)
/// — `auth::session::CurrentUser` already resolves the same user via
/// `DEV_EMAIL`, so the scan does the same lookup instead of assuming a row
/// that may not exist yet. If the dev user row hasn't been created, the
/// scan logs and skips rather than erroring the whole task.
async fn run_scan_once(pool: &PgPool) {
    use fubbik_db::repo::user;

    let start = std::time::Instant::now();
    match user::find_by_email(pool, crate::auth::session::DEV_EMAIL).await {
        Ok(Some(dev_user)) => {
            match staleness::detect_age_stale_chunks(pool, &dev_user.id, None, 90).await {
                Ok(flagged) => {
                    tracing::info!(flagged, duration_ms = %start.elapsed().as_millis(), "Staleness scan completed");
                }
                Err(err) => tracing::error!(?err, "Staleness scan failed"),
            }
        }
        Ok(None) => tracing::warn!("Staleness scan skipped: implicit dev user not found"),
        Err(err) => tracing::error!(?err, "Staleness scan failed: could not look up dev user"),
    }
}

/// Wires the background age-staleness scan beside the rest of this
/// process's startup, mirroring `packages/api/src/startup.ts::initStartupTasks`:
/// an initial run 30 seconds after boot (giving migrations time to
/// complete), then on `STALENESS_SCAN_INTERVAL_HOURS` (default 24), unless
/// that resolves to "disabled" per [`resolve_scan_interval`] — in which
/// case this returns immediately without spawning anything.
///
/// Deliberately not called from [`crate::router`]: dozens of `sqlx::test`
/// suites build a router via `fubbik_api::router(state)` per test, and
/// spawning a 30-second-delayed background task in every one of them would
/// be pure overhead against throwaway per-test databases. The caller
/// (`crates/fubbik/src/main.rs`'s `Commands::Serve` arm) invokes this once,
/// explicitly, the same place Node's `initStartupTasks()` is invoked from
/// `apps/server`'s boot path.
pub fn spawn_background_scan(pool: PgPool) {
    let Some(interval) = resolve_scan_interval(
        std::env::var("STALENESS_SCAN_INTERVAL_HOURS")
            .ok()
            .as_deref(),
    ) else {
        tracing::info!("Staleness scanning disabled (STALENESS_SCAN_INTERVAL_HOURS<=0)");
        return;
    };
    tracing::info!(
        interval_secs = interval.as_secs(),
        "Staleness scanning enabled"
    );

    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(30)).await;
        run_scan_once(&pool).await;
        loop {
            tokio::time::sleep(interval).await;
            run_scan_once(&pool).await;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::resolve_scan_interval;

    #[test]
    fn default_is_24_hours_when_unset() {
        assert_eq!(
            resolve_scan_interval(None),
            Some(std::time::Duration::from_secs(24 * 3600))
        );
    }

    #[test]
    fn zero_disables_scanning() {
        assert_eq!(resolve_scan_interval(Some("0")), None);
    }

    #[test]
    fn negative_disables_scanning() {
        assert_eq!(resolve_scan_interval(Some("-5")), None);
    }

    #[test]
    fn positive_value_is_honoured() {
        assert_eq!(
            resolve_scan_interval(Some("6")),
            Some(std::time::Duration::from_secs(6 * 3600))
        );
    }

    #[test]
    fn unparseable_value_falls_back_to_the_24_hour_default() {
        assert_eq!(
            resolve_scan_interval(Some("garbage")),
            Some(std::time::Duration::from_secs(24 * 3600))
        );
    }
}
