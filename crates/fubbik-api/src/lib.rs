pub mod auth;
pub mod chunks;

use axum::Router;
use sqlx::PgPool;

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub implicit_dev_session: bool,
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .merge(auth::routes::router())
        .merge(chunks::routes::router())
        .with_state(state)
}
