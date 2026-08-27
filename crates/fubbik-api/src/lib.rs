pub mod activity;
pub mod auth;
pub mod chunks;
pub mod collections;
pub mod comments;
pub mod connections;
pub mod coverage;
pub mod density;
pub mod documents;
pub mod enrich;
pub mod error;
pub mod extract;
pub mod favorites;
pub mod features;
pub mod file_refs;
pub mod graph;
pub mod health;
pub mod learning_paths;
pub mod matrices;
pub mod middleware;
pub mod notifications;
pub mod openapi;
pub mod plans;
pub mod proposals;
pub mod requirements;
pub mod saved_graphs;
pub mod scope_keys;
pub mod search;
pub mod settings;
pub mod spaces;
pub mod staleness;
pub mod stats;
pub mod tag_types;
pub mod tags;
pub mod templates;
pub mod timeline;
pub mod use_cases;
pub mod vocabularies;
pub mod vocabulary;
pub mod workspaces;

use axum::Router;
use sqlx::PgPool;

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub implicit_dev_session: bool,
    /// `BETTER_AUTH_SECRET`, resolved once at startup — never read from the
    /// environment per request. Used to verify the HMAC signature on
    /// better-auth's session cookies (see `auth::better_auth_cookie`).
    pub better_auth_secret: String,
    /// The Ollama transport, constructed once at startup. Carried here
    /// rather than resolved from `OLLAMA_URL` per call so that each test
    /// can inject its own `wiremock` base URL — see
    /// `crates/fubbik-ai/src/client.rs`'s module doc.
    pub ai: fubbik_ai::OllamaClient,
    /// Per-user request windows for the two endpoints Node rate-limits.
    /// Process-local, like Node's — this is not a distributed limiter and
    /// is not meant to be.
    pub rate_limiter: crate::middleware::rate_limit::RateLimiter,
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .merge(activity::routes::router())
        .merge(auth::routes::router())
        .merge(chunks::routes::router())
        .merge(collections::routes::router())
        .merge(connections::routes::router())
        .merge(coverage::routes::router())
        .merge(documents::routes::router())
        .merge(enrich::routes::router())
        .merge(comments::routes::router())
        .merge(favorites::routes::router())
        .merge(graph::routes::router())
        .merge(density::routes::router())
        .merge(file_refs::routes::router())
        .merge(health::routes::router())
        .merge(scope_keys::routes::router())
        .merge(timeline::routes::router())
        .merge(learning_paths::routes::router())
        .merge(matrices::routes::router())
        .merge(features::routes::router())
        .merge(notifications::routes::router())
        .merge(plans::routes::router())
        .merge(proposals::routes::router())
        .merge(requirements::routes::router())
        .merge(requirements::dependency_routes::router())
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
        .merge(vocabularies::routes::router())
        .merge(vocabulary::routes::router())
        .merge(workspaces::routes::router())
        .with_state(state)
}
