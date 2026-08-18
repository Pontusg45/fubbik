pub mod activity;
pub mod auth;
pub mod chunks;
pub mod collections;
pub mod connections;
pub mod error;
pub mod extract;
pub mod favorites;
pub mod notifications;
pub mod openapi;
pub mod plans;
pub mod proposals;
pub mod saved_graphs;
pub mod search;
pub mod settings;
pub mod spaces;
pub mod staleness;
pub mod stats;
pub mod tag_types;
pub mod tags;
pub mod templates;
pub mod use_cases;
pub mod vocabulary;
pub mod workspaces;

use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};
use sqlx::PgPool;

use crate::error::ApiResult;

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub implicit_dev_session: bool,
    /// `BETTER_AUTH_SECRET`, resolved once at startup — never read from the
    /// environment per request. Used to verify the HMAC signature on
    /// better-auth's session cookies (see `auth::better_auth_cookie`).
    pub better_auth_secret: String,
}

/// Deliberately unauthenticated and not utoipa-annotated — it's an
/// operational probe, not part of the public API surface.
async fn health(State(state): State<AppState>) -> ApiResult<Json<serde_json::Value>> {
    let db_ok = sqlx::query("SELECT 1").execute(&state.pool).await.is_ok();
    Ok(Json(serde_json::json!({
        "status": if db_ok { "ok" } else { "degraded" },
        "database": db_ok,
        "version": env!("CARGO_PKG_VERSION"),
    })))
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .merge(activity::routes::router())
        .merge(auth::routes::router())
        .merge(chunks::routes::router())
        .merge(collections::routes::router())
        .merge(connections::routes::router())
        .merge(favorites::routes::router())
        .merge(notifications::routes::router())
        .merge(plans::routes::router())
        .merge(proposals::routes::router())
        .merge(saved_graphs::routes::router())
        .merge(search::routes::router())
        .merge(settings::routes::router())
        .merge(spaces::routes::router())
        .merge(staleness::routes::router())
        .merge(stats::routes::router())
        .merge(tag_types::routes::router())
        .merge(tags::routes::router())
        .merge(templates::routes::router())
        .merge(use_cases::routes::router())
        .merge(vocabulary::routes::router())
        .merge(workspaces::routes::router())
        .route("/api/health", get(health))
        .with_state(state)
}
