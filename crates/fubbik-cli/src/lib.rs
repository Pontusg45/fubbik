pub mod client;
pub mod commands;
pub mod output;
pub mod plugin;

use std::ffi::OsString;

use anyhow::Result;
use clap::Subcommand;

pub use output::OutputMode;

#[derive(Subcommand)]
pub enum ReviewCommand {
    /// List proposals waiting for review
    List {
        #[arg(short, long, default_value = "pending")]
        status: String,
        #[arg(short, long)]
        chunk: Option<String>,
        #[arg(short, long, default_value = "20")]
        limit: u32,
    },
    /// Show a proposal
    Show { id: String },
    /// Approve and apply a proposal
    Approve {
        id: String,
        #[arg(short, long)]
        note: Option<String>,
    },
    /// Reject a proposal
    Reject {
        id: String,
        #[arg(short, long)]
        note: Option<String>,
    },
}

#[derive(Subcommand)]
pub enum PlanCommand {
    /// List plans
    List {
        #[arg(short, long)]
        status: Option<String>,
        #[arg(short, long)]
        space: Option<String>,
    },
    /// Show a plan and its tasks
    Show { id: String },
    /// Create a plan
    Create {
        title: String,
        #[arg(short, long)]
        description: Option<String>,
        #[arg(short, long)]
        space: Option<String>,
    },
    /// Change plan status
    Status { id: String, status: String },
}

#[derive(Subcommand)]
pub enum TaskCommand {
    /// Create a quick single-task plan
    Add {
        title: String,
        #[arg(short, long)]
        description: Option<String>,
        #[arg(short, long)]
        space: Option<String>,
    },
    /// List quick tasks
    List {
        #[arg(short, long, default_value = "in_progress")]
        status: String,
    },
    /// Mark a quick task in progress (the id is its plan id)
    Claim { id: String },
    /// Complete a quick task and its plan (the id is its plan id)
    Done { id: String },
}

#[derive(Subcommand)]
pub enum PluginCommand {
    /// List discovered fubbik-* executables
    List,
    /// Inspect plugin discovery and protocol configuration
    Doctor,
}

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
    /// Review proposed knowledge changes
    Review {
        #[command(subcommand)]
        command: ReviewCommand,
    },
    /// Manage plans
    Plan {
        #[command(subcommand)]
        command: PlanCommand,
    },
    /// Manage quick tasks backed by single-task plans
    Task {
        #[command(subcommand)]
        command: TaskCommand,
    },
    /// Inspect installed external commands
    Plugin {
        #[command(subcommand)]
        command: PluginCommand,
    },
    #[command(external_subcommand)]
    External(Vec<OsString>),
}

pub async fn run(cmd: Command, base_url: &str, output: OutputMode) -> Result<()> {
    let client = client::Client::new(base_url);
    match cmd {
        Command::Add {
            title,
            content,
            r#type,
        } => commands::add::run(&client, &title, &content, &r#type, output).await,
        Command::Get { id } => commands::get::run(&client, &id, output).await,
        Command::List { r#type, limit } => {
            commands::list::run(&client, r#type.as_deref(), limit, output).await
        }
        Command::Search { query, limit } => {
            commands::search::run(&client, &query, limit, output).await
        }
        Command::Health => commands::health::run(&client, output).await,
        Command::Review { command } => commands::review::run(&client, command, output).await,
        Command::Plan { command } => commands::plan::run(&client, command, output).await,
        Command::Task { command } => commands::task::run(&client, command, output).await,
        Command::Plugin { command } => commands::plugin::run(command, output),
        Command::External(args) => plugin::execute(args, base_url, output).await,
    }
}
