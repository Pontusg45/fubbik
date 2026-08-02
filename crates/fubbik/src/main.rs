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

/// The only `NODE_ENV` values Node itself accepts: `packages/env/src/server.ts`
/// validates it through an arktype-checked `@t3-oss/env-core` schema whose
/// default `onValidationError` throws, crashing the Node process at boot on
/// anything else (wrong case, stray whitespace, `"prod"`, ...).
const VALID_NODE_ENVS: [&str; 3] = ["development", "production", "test"];

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
///
/// Unlike `fubbik_db::warn_if_not_icu_collation`, which only warns — a wrong
/// sort order must never take the server down — a malformed `NODE_ENV` fails
/// closed here (`Err`, refuse to start) rather than falling back to
/// "relaxed": Node crashes at boot on the same malformed input, and a typo
/// that silently disables authentication is a security defect, not a
/// cosmetic one. An *unset* `NODE_ENV` is not malformed — it is Node's own
/// signal for "no environment configured", and local dev depends on it
/// staying relaxed.
fn resolve_implicit_dev_session(
    node_env: Option<&str>,
    explicit_flag: bool,
) -> Result<bool, String> {
    let is_production = match node_env {
        None => false,
        Some(value) if VALID_NODE_ENVS.contains(&value) => value == "production",
        Some(value) => {
            return Err(format!(
                "NODE_ENV={value:?} is not a recognized value; expected one of \
                 {VALID_NODE_ENVS:?}, or unset"
            ));
        }
    };
    Ok(explicit_flag || !is_production)
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
                resolve_implicit_dev_session(node_env.as_deref(), explicit_flag)
                    .map_err(|e| anyhow::anyhow!(e))?;
            if implicit_dev_session {
                let reason = if explicit_flag {
                    "FUBBIK_IMPLICIT_DEV_SESSION=true is set"
                } else {
                    "NODE_ENV is not \"production\""
                };
                tracing::warn!(
                    reason,
                    "AUTHENTICATION IS NOT ENFORCED: every request to this server — \
                     including one with a missing, expired, or outright garbage session \
                     cookie — is served as the dev user ({dev_email}), because {reason}. \
                     This is expected and required for local development and the CLI, but \
                     it must never be silently true in a deployment you believe is secured. \
                     To require real sessions, ensure NODE_ENV=production and do not set \
                     FUBBIK_IMPLICIT_DEV_SESSION=true.",
                    dev_email = fubbik_api::auth::session::DEV_EMAIL,
                );
            }

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
        assert_eq!(resolve_implicit_dev_session(None, false), Ok(true));
    }

    #[test]
    fn production_is_not_relaxed() {
        assert_eq!(
            resolve_implicit_dev_session(Some("production"), false),
            Ok(false)
        );
    }

    #[test]
    fn explicit_flag_wins_even_under_production() {
        assert_eq!(
            resolve_implicit_dev_session(Some("production"), true),
            Ok(true)
        );
    }

    #[test]
    fn development_is_relaxed() {
        assert_eq!(
            resolve_implicit_dev_session(Some("development"), false),
            Ok(true)
        );
    }

    #[test]
    fn test_env_is_relaxed() {
        assert_eq!(resolve_implicit_dev_session(Some("test"), false), Ok(true));
    }

    #[test]
    fn wrong_case_is_rejected() {
        assert!(resolve_implicit_dev_session(Some("Production"), false).is_err());
    }

    #[test]
    fn abbreviated_value_is_rejected() {
        assert!(resolve_implicit_dev_session(Some("prod"), false).is_err());
    }

    #[test]
    fn whitespace_is_rejected() {
        assert!(resolve_implicit_dev_session(Some(" production"), false).is_err());
    }

    #[test]
    fn empty_string_is_rejected() {
        assert!(resolve_implicit_dev_session(Some(""), false).is_err());
    }

    #[test]
    fn rejection_still_happens_even_with_the_explicit_flag_set() {
        // Node validates NODE_ENV unconditionally at import time, before any
        // flag logic runs, so a malformed value must fail closed regardless
        // of FUBBIK_IMPLICIT_DEV_SESSION.
        assert!(resolve_implicit_dev_session(Some("Production"), true).is_err());
    }
}
