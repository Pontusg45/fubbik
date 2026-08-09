pub mod activity;
pub mod assets;
pub mod auth;
pub mod chunks;
pub mod collections;
pub mod connections;
pub mod error;
pub mod extract;
pub mod favorites;
pub mod notifications;
pub mod openapi;
pub mod search;
pub mod settings;
pub mod spaces;
pub mod stats;
pub mod tag_types;
pub mod tags;
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
        .merge(settings::routes::router())
        .merge(spaces::routes::router())
        .merge(stats::routes::router())
        .merge(tag_types::routes::router())
        .merge(tags::routes::router())
        .merge(workspaces::routes::router())
        .route("/api/health", get(health))
        .with_state(state)
        .fallback(assets::serve)
}
