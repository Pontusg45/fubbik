pub mod activity;
pub mod ai;
pub mod assets;
pub mod auth;
pub mod background;
pub mod capabilities;
pub mod chunks;
pub mod collections;
pub mod comments;
pub mod connections;
pub mod context;
pub mod context_export;
pub mod context_for_file;
pub mod coordination;
pub mod coverage;
pub mod density;
pub mod documents;
pub mod enrich;
pub mod error;
pub mod extract;
pub mod favorites;
pub mod features;
pub mod file_refs;
pub mod generate_instructions;
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
pub mod tasks;
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
    pub background: background::BackgroundRuntime,
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
    /// The `fubbik.toml` `[features]` switches, read once at startup.
    /// `router()` mounts a group's routes only when its feature is on, and
    /// `GET /api/capabilities` reports the same set to clients.
    pub features: fubbik_config::Features,
}

pub fn router(state: AppState) -> Router {
    use fubbik_config::Feature;

    // `core`: always mounted, no switch.
    let mut app = Router::new()
        .merge(activity::routes::router())
        .merge(auth::routes::router())
        .merge(capabilities::router())
        .merge(chunks::routes::router())
        .merge(chunks::groups::router())
        .merge(chunks::import_docs::router())
        .merge(collections::routes::router())
        .merge(connections::routes::router())
        .merge(context::routes::router())
        .merge(context_export::routes::router())
        .merge(context_for_file::routes::router())
        .merge(documents::routes::router())
        .merge(documents::source_docs::router())
        .merge(enrich::routes::router())
        .merge(comments::routes::router())
        .merge(favorites::routes::router())
        .merge(graph::routes::router())
        .merge(density::routes::router())
        .merge(file_refs::routes::router())
        .merge(generate_instructions::routes::router())
        .merge(health::routes::router())
        .merge(scope_keys::routes::router())
        .merge(timeline::routes::router())
        .merge(notifications::routes::router())
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
        .merge(vocabularies::routes::router())
        .merge(vocabulary::routes::router())
        .merge(workspaces::routes::router());

    // Switchable groups — see `fubbik_config::Feature`.
    let switches = &state.features;
    if switches.enabled(Feature::Plans) {
        app = app.merge(plans::routes::router()).merge(tasks::router());
    }
    if switches.enabled(Feature::Requirements) {
        app = app
            .merge(requirements::routes::router())
            .merge(requirements::dependency_routes::router())
            .merge(use_cases::routes::router())
            .merge(coverage::routes::router());
    }
    if switches.enabled(Feature::Matrices) {
        app = app.merge(matrices::routes::router());
    }
    if switches.enabled(Feature::Coordination) {
        app = app.merge(coordination::routes::router());
    }
    if switches.enabled(Feature::LearningPaths) {
        app = app.merge(learning_paths::routes::router());
    }
    if switches.enabled(Feature::Overlays) {
        app = app.merge(features::routes::router());
    }
    if switches.enabled(Feature::Ai) {
        app = app.merge(ai::routes::router());
    }

    app.with_state(state)
}
