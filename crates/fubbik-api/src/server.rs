//! Running the API: one function shared by `fubbik serve` and the desktop
//! app, which runs the server inside its own process. It connects (applying
//! migrations), starts the background jobs, serves until `shutdown`
//! resolves, then drains the background jobs and closes the pool.

use std::future::Future;
use std::path::PathBuf;
use std::time::Duration;

use axum::http::HeaderValue;
use tower_http::cors::{AllowHeaders, AllowMethods, AllowOrigin, CorsLayer};
use tower_http::request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer};

use crate::{AppState, background, graph, staleness};

/// Everything `serve` needs; the callers decide where each value comes from
/// (the environment for `fubbik serve`, the app's own files for the desktop).
pub struct ServerOptions {
    pub database_url: String,
    pub better_auth_secret: String,
    /// Serve unauthenticated requests as the dev user.
    pub implicit_dev_session: bool,
    pub features: fubbik_config::Features,
    pub ai: fubbik_ai::OllamaClient,
    /// Browser origins allowed to send credentials.
    pub cors_origins: Vec<HeaderValue>,
    /// A built web app (`apps/web/dist`) to serve at `/`, if any.
    pub web_assets: Option<PathBuf>,
}

/// `CORS_ORIGIN`-style comma-separated origins.
///
/// A malformed entry (not a valid HTTP header value) panics rather than
/// silently dropping the origin: it is fixed configuration read once at
/// startup.
pub fn parse_cors_origins(value: &str) -> Vec<HeaderValue> {
    value
        .split(',')
        .map(str::trim)
        .map(|origin| {
            origin.parse::<HeaderValue>().unwrap_or_else(|_| {
                panic!("CORS_ORIGIN entry {origin:?} is not a valid header value")
            })
        })
        .collect()
}

/// Serves the API on `listener` until `shutdown` resolves.
pub async fn serve(
    options: ServerOptions,
    listener: tokio::net::TcpListener,
    shutdown: impl Future<Output = ()> + Send + 'static,
) -> anyhow::Result<()> {
    if let Some(dir) = &options.web_assets
        && !dir.join("index.html").is_file()
    {
        anyhow::bail!("web asset directory has no index.html: {}", dir.display());
    }
    let pool = fubbik_db::connect(&options.database_url).await?;
    fubbik_db::warn_if_not_icu_collation(&pool).await;
    let background = background::BackgroundRuntime::new();
    staleness::service::spawn_background_scan(pool.clone(), background.clone());
    // Behavior rules only exist to the user through the matrix routes; with
    // `matrices` off there is nothing to project.
    if options.features.enabled(fubbik_config::Feature::Matrices) {
        graph::sync::spawn_behavior_sync(pool.clone(), background.clone());
    }
    graph::projection::spawn_projection_worker(pool.clone(), background.clone());
    let shutdown_pool = pool.clone();
    let state = AppState {
        pool,
        implicit_dev_session: options.implicit_dev_session,
        better_auth_secret: options.better_auth_secret,
        ai: options.ai,
        rate_limiter: Default::default(),
        background: background.clone(),
        features: options.features,
    };

    let cors = CorsLayer::new()
        .allow_origin(AllowOrigin::list(options.cors_origins))
        .allow_credentials(true)
        // `Any` panics when combined with `allow_credentials(true)` (browsers
        // reject the combination outright); mirroring the request's own
        // Access-Control-Request-Method/Headers is the credentials-compatible
        // equivalent of "allow everything".
        .allow_methods(AllowMethods::mirror_request())
        .allow_headers(AllowHeaders::mirror_request());

    let mut app = crate::router(state);
    if let Some(dir) = options.web_assets {
        app = app.merge(crate::assets::router(dir));
    }
    let app = app
        .layer(cors)
        .layer(tower_http::catch_panic::CatchPanicLayer::new())
        .layer(tower_http::trace::TraceLayer::new_for_http())
        .layer(PropagateRequestIdLayer::x_request_id())
        .layer(SetRequestIdLayer::x_request_id(MakeRequestUuid));

    if let Ok(addr) = listener.local_addr() {
        tracing::info!("fubbik listening on http://{addr}");
    }
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown)
        .await?;
    background.shutdown(Duration::from_secs(10)).await;
    if tokio::time::timeout(Duration::from_secs(5), shutdown_pool.close())
        .await
        .is_err()
    {
        tracing::warn!("database pool did not close before shutdown deadline");
    }
    Ok(())
}
