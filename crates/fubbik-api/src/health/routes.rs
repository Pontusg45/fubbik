//! `/api/health` and `/api/health/knowledge`.
//!
//! The two are unrelated despite the shared prefix: the first is an
//! unauthenticated liveness probe, the second is a per-user report on chunks
//! that need attention.

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use fubbik_db::repo::knowledge_health as kh;

use crate::AppState;
use crate::auth::CurrentUser;
use crate::error::ApiResult;

#[derive(serde::Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct HealthResponse {
    /// `ok | degraded`.
    pub status: String,
    /// `connected | disconnected`.
    pub db: String,
    pub age_available: bool,
}

/// **Deliberately unauthenticated**, matching Node — this is what a load
/// balancer or the nav bar's connection indicator polls, and requiring a
/// session would make an expired cookie look like an outage.
///
/// Answers **503** when the database is unreachable, so a probe that only
/// looks at the status code still works. `ageAvailable` is reported but never
/// degrades the status: the graph extension is optional, and the app runs
/// without it.
#[utoipa::path(get, path = "/api/health",
    responses((status = 200, body = HealthResponse), (status = 503, body = HealthResponse)))]
pub async fn health(State(state): State<AppState>) -> (StatusCode, Json<HealthResponse>) {
    let db_ok = kh::db_reachable(&state.pool).await;
    // Checked even when the DB is down: `is_available` swallows its own
    // errors and returns false, so this cannot turn a 503 into a 500.
    let age_available = fubbik_db::age::is_available(&state.pool).await;

    let status = if db_ok {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };
    (
        status,
        Json(HealthResponse {
            status: if db_ok {
                "ok".into()
            } else {
                "degraded".into()
            },
            db: if db_ok {
                "connected".into()
            } else {
                "disconnected".into()
            },
            age_available,
        }),
    )
}

#[derive(serde::Deserialize, utoipa::IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct KnowledgeHealthQuery {
    pub space_id: Option<String>,
}

/// The five buckets, in the order the panel renders them.
#[derive(serde::Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct KnowledgeHealth {
    pub orphans: kh::HealthBucket<kh::OrphanChunk>,
    pub stale: kh::HealthBucket<kh::StaleChunk>,
    pub thin: kh::HealthBucket<kh::ThinChunk>,
    pub stale_embeddings: kh::HealthBucket<kh::StaleEmbedding>,
    pub file_refs: kh::FileRefBucket,
}

/// Node runs the five queries with `concurrency: "unbounded"`; these run in
/// sequence. Each is a single indexed scan against the same pool, so the
/// difference is a few round trips rather than a change in what is read —
/// and sequencing keeps them from competing for connections when several
/// users open the panel at once.
#[utoipa::path(get, path = "/api/health/knowledge", params(KnowledgeHealthQuery),
    responses((status = 200, body = KnowledgeHealth)))]
pub async fn knowledge_health(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Query(query): Query<KnowledgeHealthQuery>,
) -> ApiResult<Json<KnowledgeHealth>> {
    let space = query.space_id.as_deref();
    Ok(Json(KnowledgeHealth {
        orphans: kh::orphan_chunks(&state.pool, &user.id, space).await?,
        stale: kh::stale_chunks(&state.pool, &user.id, space).await?,
        thin: kh::thin_chunks(&state.pool, &user.id, space).await?,
        stale_embeddings: kh::stale_embeddings(&state.pool, &user.id, space).await?,
        file_refs: kh::file_refs(&state.pool, &user.id, space).await?,
    }))
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/health", get(health))
        .route("/api/health/knowledge", get(knowledge_health))
}
