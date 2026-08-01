pub mod assets;
pub mod auth;
pub mod chunks;
pub mod openapi;

use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};
use fubbik_core::error::AppResult;
use sqlx::PgPool;

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub implicit_dev_session: bool,
}

/// Deliberately unauthenticated and not utoipa-annotated — it's an
/// operational probe, not part of the public API surface.
async fn health(State(state): State<AppState>) -> AppResult<Json<serde_json::Value>> {
    let db_ok = sqlx::query("SELECT 1").execute(&state.pool).await.is_ok();
    Ok(Json(serde_json::json!({
        "status": if db_ok { "ok" } else { "degraded" },
        "database": db_ok,
        "version": env!("CARGO_PKG_VERSION"),
    })))
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .merge(auth::routes::router())
        .merge(chunks::routes::router())
        .route("/api/health", get(health))
        .with_state(state)
        .fallback(assets::serve)
}
