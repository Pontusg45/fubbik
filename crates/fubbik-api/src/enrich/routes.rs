use std::time::Duration;

use axum::extract::{Path, State};
use axum::response::IntoResponse;
use axum::routing::post;
use axum::{Json, Router};

use crate::AppState;
use crate::auth::CurrentUser;
use crate::error::ApiResult;

/// Node's limits (`enrich/routes.ts:19`, `chunks/routes.ts:197`).
const ENRICH_MAX: u32 = 10;
const ENRICH_WINDOW: Duration = Duration::from_secs(60);

/// Node's `listChunks(userId, { limit: "1000", offset: "0" })` and
/// `{ concurrency: 3 }` (`enrich/routes.ts:34-45`). Both numbers are
/// matched exactly rather than improved: changing them is a product
/// decision, not a port decision.
const ENRICH_ALL_LIMIT: i64 = 1000;
const ENRICH_ALL_CONCURRENCY: usize = 3;

#[utoipa::path(post, path = "/api/chunks/{id}/enrich",
    params(("id" = String, Path,)),
    responses((status = 200), (status = 404), (status = 429)))]
pub async fn enrich_chunk(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<String>,
) -> ApiResult<axum::response::Response> {
    let decision =
        state
            .rate_limiter
            .check(&format!("enrich:{}", user.id), ENRICH_MAX, ENRICH_WINDOW);
    if !decision.allowed {
        return Ok((
            axum::http::StatusCode::TOO_MANY_REQUESTS,
            Json(serde_json::json!({
                "error": "Rate limit exceeded",
                "retryAfter": decision.retry_after_secs,
            })),
        )
            .into_response());
    }

    let enriched = super::service::enrich_chunk(&state.pool, &state.ai, &user.id, &id).await?;
    Ok(Json(enriched).into_response())
}

#[utoipa::path(post, path = "/api/chunks/enrich-all", responses((status = 200)))]
pub async fn enrich_all(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
) -> ApiResult<Json<serde_json::Value>> {
    let ids =
        fubbik_db::repo::chunk::list_ids_for_user(&state.pool, &user.id, ENRICH_ALL_LIMIT).await?;

    // Node's `Effect.forEach(..., { concurrency: 3 })` with a per-item
    // `catchAll` that yields null: one chunk failing must not abort the
    // sweep, and only successful, non-null results are counted.
    //
    // A `Semaphore` plus `JoinSet` rather than `futures::buffer_unordered`,
    // because `futures` is not a dependency of this workspace and tokio
    // (already present with `features = ["full"]`) covers it.
    let permits = std::sync::Arc::new(tokio::sync::Semaphore::new(ENRICH_ALL_CONCURRENCY));
    let mut tasks = tokio::task::JoinSet::new();
    for id in ids {
        let pool = state.pool.clone();
        let ai = state.ai.clone();
        let user_id = user.id.clone();
        let permits = permits.clone();
        tasks.spawn(async move {
            let _permit = permits.acquire().await.expect("semaphore never closed");
            let result = super::service::enrich_chunk(&pool, &ai, &user_id, &id).await;
            if let Err(ref err) = result {
                // The sweep's contract (a bare count, matching Node's
                // catchAll) doesn't change — this is purely so a partial
                // sweep is debuggable instead of a silent gap.
                tracing::warn!("enrich-all failed for chunk {id}: {err}");
            }
            result.ok().flatten().is_some()
        });
    }

    let mut enriched = 0usize;
    while let Some(result) = tasks.join_next().await {
        // A panicking task counts as a failure, not an abort — same as
        // Node's per-item catchAll.
        if result.unwrap_or(false) {
            enriched += 1;
        }
    }

    Ok(Json(serde_json::json!({ "enriched": enriched })))
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/chunks/{id}/enrich", post(enrich_chunk))
        .route("/api/chunks/enrich-all", post(enrich_all))
}
