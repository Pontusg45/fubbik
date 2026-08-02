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

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    match Cli::parse().command {
        Commands::Serve { port, host } => {
            let database_url = std::env::var("DATABASE_URL")
                .map_err(|_| anyhow::anyhow!("DATABASE_URL is required"))?;
            let implicit_dev_session =
                std::env::var("FUBBIK_IMPLICIT_DEV_SESSION").as_deref() == Ok("true");

            let pool = fubbik_db::connect(&database_url).await?;
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
