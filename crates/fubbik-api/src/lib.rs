pub mod auth;

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
        .with_state(state)
}
