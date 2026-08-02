use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "fubbik", version, about = "Local-first knowledge framework")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run the API server and web UI
    Serve {
        #[arg(long, env = "PORT", default_value = "3100")]
        port: u16,
        /// Address to bind to. Defaults to loopback-only: combined with
        /// FUBBIK_IMPLICIT_DEV_SESSION=true (under which any invalid
        /// cookie yields the dev user), binding wider than 127.0.0.1
        /// exposes full access to anyone who can reach that address, so
        /// it must be opted into deliberately.
        #[arg(long, env = "HOST", default_value = "127.0.0.1")]
        host: std::net::IpAddr,
    },
    /// Run the MCP server over stdio
    Mcp,
    /// Print the OpenAPI document to stdout
    Openapi,
    #[command(flatten)]
    Cli(fubbik_cli::Command),
}

/// Resolves whether relaxed (implicit-dev-session) HTTP auth applies: any
/// request — including one with a garbage or missing session cookie — is
/// served as the dev user rather than rejected with 401.
///
/// Mirrors the TS server's rule verbatim (`packages/auth/src/index.ts`):
/// `NODE_ENV !== "production" || FUBBIK_IMPLICIT_DEV_SESSION === "true"`.
/// The explicit flag always wins, even under `NODE_ENV=production`.
///
/// Reads `NODE_ENV` deliberately, not a Rust-native name: this backend
/// replaces the Node server, and existing deployment configs
/// (`docker-compose.yml`, and `docker-compose.selfhost.yml` via
/// `docker/build/server.Dockerfile`'s `ENV NODE_ENV=production`) already set
/// it for production. Reading the same variable means those deployments
/// keep working unchanged, and do not silently end up with auth relaxed in
/// production just because this binary looked at a different name. Do not
/// rename this to `FUBBIK_ENV` or similar without also updating every
/// deployment config that currently sets `NODE_ENV=production`.
fn resolve_implicit_dev_session(node_env: Option<&str>, explicit_flag: bool) -> bool {
    explicit_flag || node_env != Some("production")
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    match Cli::parse().command {
        Commands::Serve { port, host } => {
            let database_url = std::env::var("DATABASE_URL")
                .map_err(|_| anyhow::anyhow!("DATABASE_URL is required"))?;
            let node_env = std::env::var("NODE_ENV").ok();
            let explicit_flag =
                std::env::var("FUBBIK_IMPLICIT_DEV_SESSION").as_deref() == Ok("true");
            let implicit_dev_session =
                resolve_implicit_dev_session(node_env.as_deref(), explicit_flag);

            let pool = fubbik_db::connect(&database_url).await?;
            fubbik_db::warn_if_not_icu_collation(&pool).await;
            let state = fubbik_api::AppState {
                pool,
                implicit_dev_session,
            };

            let cors = tower_http::cors::CorsLayer::new()
                .allow_origin(
                    std::env::var("CORS_ORIGIN")
                        .unwrap_or_else(|_| "http://localhost:3001".into())
                        .parse::<axum::http::HeaderValue>()?,
                )
                .allow_credentials(true)
                // `Any` panics when combined with `allow_credentials(true)`
                // (browsers reject the combination outright); mirroring the
                // request's own Access-Control-Request-Method/Headers is the
                // credentials-compatible equivalent of "allow everything".
                .allow_methods(tower_http::cors::AllowMethods::mirror_request())
                .allow_headers(tower_http::cors::AllowHeaders::mirror_request());

            let app = fubbik_api::router(state).layer(cors);
            let listener = tokio::net::TcpListener::bind((host, port)).await?;
            tracing::info!("fubbik listening on http://{host}:{port}");
            axum::serve(listener, app).await?;
            Ok(())
        }
        Commands::Mcp => {
            println!("mcp — not yet implemented");
            Ok(())
        }
        Commands::Openapi => {
            use utoipa::OpenApi;
            let doc = fubbik_api::openapi::ApiDoc::openapi();
            println!("{}", serde_json::to_string_pretty(&doc)?);
            Ok(())
        }
        Commands::Cli(cmd) => {
            let base =
                std::env::var("FUBBIK_URL").unwrap_or_else(|_| "http://localhost:3100".into());
            fubbik_cli::run(cmd, &base).await
        }
    }
}

#[cfg(test)]
mod tests {
    use super::resolve_implicit_dev_session;

    // Pure-function tests only: `NODE_ENV`/`FUBBIK_IMPLICIT_DEV_SESSION` are
    // process-global and tests run in parallel, so the resolution logic is
    // exercised via explicit parameters rather than by mutating the real
    // environment.

    #[test]
    fn unset_environment_is_relaxed() {
        assert!(resolve_implicit_dev_session(None, false));
    }

    #[test]
    fn production_is_not_relaxed() {
        assert!(!resolve_implicit_dev_session(Some("production"), false));
    }

    #[test]
    fn explicit_flag_wins_even_under_production() {
        assert!(resolve_implicit_dev_session(Some("production"), true));
    }

    #[test]
    fn development_is_relaxed() {
        assert!(resolve_implicit_dev_session(Some("development"), false));
    }
}
