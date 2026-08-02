pub mod client;
pub mod commands;

use anyhow::Result;
use clap::Subcommand;

#[derive(Subcommand)]
pub enum Command {
    /// Create a chunk
    Add {
        title: String,
        #[arg(short, long, default_value = "")]
        content: String,
        #[arg(short = 't', long, default_value = "note")]
        r#type: String,
    },
    /// Show a chunk by id
    Get { id: String },
    /// List chunks
    List {
        #[arg(short = 't', long)]
        r#type: Option<String>,
        #[arg(short, long, default_value = "50")]
        limit: u32,
    },
    /// Search chunks by text
    Search {
        query: String,
        #[arg(short, long, default_value = "50")]
        limit: u32,
    },
    /// Check server health
    Health,
}

pub async fn run(cmd: Command, base_url: &str) -> Result<()> {
    let client = client::Client::new(base_url);
    match cmd {
        Command::Add {
            title,
            content,
            r#type,
        } => commands::add::run(&client, &title, &content, &r#type).await,
        Command::Get { id } => commands::get::run(&client, &id).await,
        Command::List { r#type, limit } => {
            commands::list::run(&client, r#type.as_deref(), limit).await
        }
        Command::Search { query, limit } => commands::search::run(&client, &query, limit).await,
        Command::Health => commands::health::run(&client).await,
    }
}
