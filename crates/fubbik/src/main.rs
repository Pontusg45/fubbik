use clap::{CommandFactory, Parser, Subcommand};
use std::ffi::OsString;
use std::time::Duration;

#[derive(Parser)]
#[command(name = "fubbik", version, about = "Local-first knowledge framework")]
struct Cli {
    /// Emit machine-readable JSON for CLI commands and plugins
    #[arg(long, global = true, conflicts_with = "quiet")]
    json: bool,
    /// Emit only identifiers or scalar values
    #[arg(short, long, global = true, conflicts_with = "json")]
    quiet: bool,
    /// API server URL (overrides FUBBIK_URL and fubbik.config.json)
    #[arg(long, global = true)]
    url: Option<String>,
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
    /// One-time backfill: project every existing `chunk_connection` row
    /// into the AGE graph (`ensure_vertex` + `create_edge` per row).
    ///
    /// Idempotent — safe to run more than once, since `create_edge` uses
    /// Cypher `MERGE` rather than `CREATE`. Never runs automatically; this
    /// is the only entry point.
    BackfillConnections,
    /// Generate shell completion definitions
    Completions { shell: clap_complete::Shell },
    #[command(flatten)]
    Cli(fubbik_cli::Command),
    /// Dispatch to a `fubbik-*` plugin executable. Must stay last: clap's
    /// `external_subcommand` cannot live inside a flattened enum without
    /// stealing built-in names like `serve` and `openapi`.
    #[command(external_subcommand)]
    External(Vec<OsString>),
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
/// Reads `NODE_ENV` deliberately, not a Rust-native name. The Rust container
/// sets `NODE_ENV=production`, preserving the former server's authentication
/// behavior and preventing a deployment from silently entering relaxed mode.
/// Do not rename this to
/// `FUBBIK_ENV` or similar without also updating every deployment config
/// that sets `NODE_ENV=production`. (`docker-compose.selfhost.yml` also sets
/// `FUBBIK_IMPLICIT_DEV_SESSION=true` explicitly today — self-host is
/// intentionally relaxed regardless of this rule.)
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

/// Splits `CORS_ORIGIN` on `,` and trims each entry, mirroring Node's
/// `env.CORS_ORIGIN.includes(",") ? env.CORS_ORIGIN.split(",").map(s =>
/// s.trim()) : env.CORS_ORIGIN` (`apps/server/src/index.ts:36`), which
/// `CLAUDE.md` documents as supported. Deliberately does **not** strip
/// trailing slashes from any entry — Node doesn't either, and matching its
/// normalisation exactly matters more than "improving" on it, since a
/// silently-added slash would make an origin that used to match stop
/// matching.
///
/// A malformed entry (not a valid HTTP header value) panics rather than
/// silently dropping the origin: `CORS_ORIGIN` is fixed configuration read
/// once at startup, same fail-fast posture as the `?` this replaced when
/// the whole variable was parsed as one `HeaderValue`.
fn parse_cors_origins(value: &str) -> Vec<axum::http::HeaderValue> {
    value
        .split(',')
        .map(str::trim)
        .map(|origin| {
            origin
                .parse::<axum::http::HeaderValue>()
                .unwrap_or_else(|_| {
                    panic!("CORS_ORIGIN entry {origin:?} is not a valid header value")
                })
        })
        .collect()
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();
    let explicit_url = cli.url.clone();
    let output = if cli.json {
        fubbik_cli::OutputMode::Json
    } else if cli.quiet {
        fubbik_cli::OutputMode::Quiet
    } else {
        fubbik_cli::OutputMode::Human
    };

    match cli.command {
        Commands::Serve { port, host } => {
            let database_url = std::env::var("DATABASE_URL")
                .map_err(|_| anyhow::anyhow!("DATABASE_URL is required"))?;
            let better_auth_secret = std::env::var("BETTER_AUTH_SECRET")
                .map_err(|_| anyhow::anyhow!("BETTER_AUTH_SECRET is required"))?;
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
            let background = fubbik_api::background::BackgroundRuntime::new();
            fubbik_api::staleness::service::spawn_background_scan(pool.clone(), background.clone());
            fubbik_api::graph::sync::spawn_behavior_sync(pool.clone(), background.clone());
            fubbik_api::graph::projection::spawn_projection_worker(
                pool.clone(),
                background.clone(),
            );
            let shutdown_pool = pool.clone();
            let state = fubbik_api::AppState {
                pool,
                implicit_dev_session,
                better_auth_secret,
                ai: fubbik_ai::OllamaClient::from_env(),
                rate_limiter: Default::default(),
                background: background.clone(),
            };

            let cors_origin_env =
                std::env::var("CORS_ORIGIN").unwrap_or_else(|_| "http://localhost:3001".into());
            let cors = tower_http::cors::CorsLayer::new()
                .allow_origin(tower_http::cors::AllowOrigin::list(parse_cors_origins(
                    &cors_origin_env,
                )))
                .allow_credentials(true)
                // `Any` panics when combined with `allow_credentials(true)`
                // (browsers reject the combination outright); mirroring the
                // request's own Access-Control-Request-Method/Headers is the
                // credentials-compatible equivalent of "allow everything".
                .allow_methods(tower_http::cors::AllowMethods::mirror_request())
                .allow_headers(tower_http::cors::AllowHeaders::mirror_request());

            let app = fubbik_api::router(state)
                .layer(cors)
                .layer(tower_http::catch_panic::CatchPanicLayer::new())
                .layer(tower_http::trace::TraceLayer::new_for_http())
                .layer(tower_http::request_id::PropagateRequestIdLayer::x_request_id())
                .layer(tower_http::request_id::SetRequestIdLayer::x_request_id(
                    tower_http::request_id::MakeRequestUuid,
                ));
            let listener = tokio::net::TcpListener::bind((host, port)).await?;
            tracing::info!("fubbik listening on http://{host}:{port}");
            axum::serve(listener, app)
                .with_graceful_shutdown(shutdown_signal())
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
        Commands::BackfillConnections => {
            let database_url = std::env::var("DATABASE_URL")
                .map_err(|_| anyhow::anyhow!("DATABASE_URL is required"))?;
            let pool = fubbik_db::connect(&database_url).await?;
            let count = fubbik_db::age::backfill_connections(&pool).await?;
            println!("projected {count} chunk_connection row(s) into the AGE graph");
            Ok(())
        }
        Commands::Completions { shell } => {
            let mut command = Cli::command();
            let name = command.get_name().to_string();
            clap_complete::generate(shell, &mut command, name, &mut std::io::stdout());
            Ok(())
        }
        Commands::Cli(cmd) => {
            let base = match fubbik_cli::config::resolve_base_url(explicit_url.as_deref()) {
                Ok(base) => base,
                Err(_)
                    if matches!(
                        cmd,
                        fubbik_cli::Command::Doctor | fubbik_cli::Command::Init { .. }
                    ) =>
                {
                    explicit_url
                        .clone()
                        .or_else(|| std::env::var("FUBBIK_URL").ok())
                        .unwrap_or_else(|| "http://localhost:3100".into())
                }
                Err(error) => return Err(error),
            };
            fubbik_cli::run(cmd, &base, output).await
        }
        Commands::External(args) => {
            let base = fubbik_cli::config::resolve_base_url(explicit_url.as_deref())?;
            fubbik_cli::plugin::execute(args, &base, output).await
        }
    }
}

async fn shutdown_signal() {
    let ctrl_c = async {
        if let Err(error) = tokio::signal::ctrl_c().await {
            tracing::error!(%error, "failed to install Ctrl-C handler");
        }
    };

    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut signal) => {
                signal.recv().await;
            }
            Err(error) => tracing::error!(%error, "failed to install SIGTERM handler"),
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
    tracing::info!("shutdown signal received; draining requests and background tasks");
}

#[cfg(test)]
mod tests {
    use clap::CommandFactory;

    use super::{Cli, parse_cors_origins, resolve_implicit_dev_session};

    #[test]
    fn rust_cli_exposes_the_server_backed_parity_commands() {
        let command = Cli::command();
        let names: Vec<_> = command
            .get_subcommands()
            .map(|subcommand| subcommand.get_name())
            .collect();
        for expected in [
            "space", "tag", "link", "unlink", "req", "stats", "enrich", "stale", "status", "docs",
            "chunk",
        ] {
            assert!(names.contains(&expected), "missing `{expected}` command");
        }
    }

    #[test]
    fn cors_accepts_a_comma_separated_origin_list() {
        let origins = parse_cors_origins("http://localhost:3001, https://app.fubbik.test:8443");
        assert_eq!(
            origins.len(),
            2,
            "CLAUDE.md documents comma-separated CORS_ORIGIN as supported"
        );
    }

    #[test]
    fn cors_single_origin_still_works() {
        let origins = parse_cors_origins("http://localhost:3001");
        assert_eq!(origins.len(), 1);
        assert_eq!(origins[0], "http://localhost:3001");
    }

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
