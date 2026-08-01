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
    },
    /// Run the MCP server over stdio
    Mcp,
    /// Print the OpenAPI document to stdout
    Openapi,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    match Cli::parse().command {
        Commands::Serve { port } => {
            println!("serve on {port} — not yet implemented");
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
    }
}
