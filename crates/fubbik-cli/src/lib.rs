pub mod client;
pub mod commands;
pub mod config;
pub mod output;
pub mod plugin;

use std::ffi::OsString;
use std::path::PathBuf;

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
        #[arg(long)]
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
pub enum ContextCommand {
    /// Export token-budgeted project context
    Export {
        #[arg(long)]
        max_tokens: Option<usize>,
        #[arg(short, long)]
        space: Option<String>,
        #[arg(long, default_value = "markdown", value_parser = ["markdown", "json"])]
        format: String,
        #[arg(long = "for")]
        for_path: Option<String>,
    },
    /// Export focused context for a file
    For {
        path: PathBuf,
        #[arg(short, long)]
        space: Option<String>,
        #[arg(long)]
        max_tokens: Option<usize>,
        #[arg(long, default_value = "structured-md", value_parser = ["structured-md", "structured-json", "json-legacy"])]
        format: String,
    },
    /// Generate CLAUDE.md-compatible project instructions
    ClaudeMd {
        #[arg(short, long)]
        space: Option<String>,
        #[arg(long)]
        tag: Option<String>,
        #[arg(long)]
        max_tokens: Option<usize>,
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
}

#[derive(Subcommand)]
pub enum ConfigCommand {
    /// Show the effective configuration
    Show,
    /// Set a configuration value (url, default-type, space, context.max-tokens, claude-md.tag, claude-md.output)
    Set { key: String, value: String },
}

#[derive(Subcommand)]
pub enum Command {
    /// Create a chunk
    Add {
        title: String,
        #[arg(short, long, conflicts_with_all = ["file", "stdin"])]
        content: Option<String>,
        #[arg(short, long, value_name = "PATH", conflicts_with_all = ["content", "stdin"])]
        file: Option<PathBuf>,
        #[arg(long, conflicts_with_all = ["content", "file"])]
        stdin: bool,
        #[arg(short = 't', long)]
        r#type: Option<String>,
        #[arg(long, value_delimiter = ',')]
        tags: Vec<String>,
        #[arg(long = "space", value_delimiter = ',')]
        spaces: Vec<String>,
    },
    /// Show a chunk by id
    Get { id: String },
    /// Print only a chunk's content
    Cat { id: String },
    /// Update a chunk
    Update {
        id: String,
        #[arg(long)]
        title: Option<String>,
        #[arg(short, long, conflicts_with_all = ["file", "stdin"])]
        content: Option<String>,
        #[arg(short, long, value_name = "PATH", conflicts_with_all = ["content", "stdin"])]
        file: Option<PathBuf>,
        #[arg(long, conflicts_with_all = ["content", "file"])]
        stdin: bool,
        #[arg(short = 't', long)]
        r#type: Option<String>,
        #[arg(long, value_delimiter = ',')]
        tags: Option<Vec<String>>,
        #[arg(long = "space", value_delimiter = ',')]
        spaces: Option<Vec<String>>,
    },
    /// Delete a chunk
    Delete {
        id: String,
        /// Skip the confirmation prompt
        #[arg(short = 'y', long)]
        yes: bool,
    },
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
    /// Initialize fubbik configuration in the current project
    Init {
        #[arg(long)]
        server: Option<String>,
        #[arg(short, long)]
        force: bool,
    },
    /// Diagnose CLI configuration and server connectivity
    Doctor,
    /// Read or update project configuration
    Config {
        #[command(subcommand)]
        command: ConfigCommand,
    },
    /// Export context for AI tools
    Context {
        #[command(subcommand)]
        command: ContextCommand,
    },
    /// Regenerate the configured CLAUDE.md context file
    Sync {
        #[arg(short, long)]
        output: Option<PathBuf>,
        #[arg(short, long)]
        space: Option<String>,
        #[arg(long)]
        tag: Option<String>,
        #[arg(long)]
        max_tokens: Option<usize>,
        #[arg(long)]
        dry_run: bool,
    },
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
            file,
            stdin,
            r#type,
            tags,
            spaces,
        } => {
            let content =
                commands::input::read(content, file.as_deref(), stdin, true)?.unwrap_or_default();
            let (settings, _) = config::load()?;
            let chunk_type = r#type
                .as_deref()
                .or(settings.default_type.as_deref())
                .unwrap_or("note");
            commands::add::run(
                &client, &title, &content, chunk_type, &tags, &spaces, output,
            )
            .await
        }
        Command::Get { id } => commands::get::run(&client, &id, output).await,
        Command::Cat { id } => commands::get::cat(&client, &id, output).await,
        Command::Update {
            id,
            title,
            content,
            file,
            stdin,
            r#type,
            tags,
            spaces,
        } => {
            let content = commands::input::read(content, file.as_deref(), stdin, false)?;
            commands::update::run(
                &client,
                &id,
                title.as_deref(),
                content.as_deref(),
                r#type.as_deref(),
                tags.as_deref(),
                spaces.as_deref(),
                output,
            )
            .await
        }
        Command::Delete { id, yes } => commands::delete::run(&client, &id, yes, output).await,
        Command::List { r#type, limit } => {
            commands::list::run(&client, r#type.as_deref(), limit, output).await
        }
        Command::Search { query, limit } => {
            commands::search::run(&client, &query, limit, output).await
        }
        Command::Health => commands::health::run(&client, output).await,
        Command::Init { server, force } => commands::config::init(server.as_deref(), force, output),
        Command::Doctor => commands::config::doctor(&client, output).await,
        Command::Config { command } => commands::config::run(command, output),
        Command::Context { command } => commands::context::run(&client, command, output).await,
        Command::Sync {
            output: path,
            space,
            tag,
            max_tokens,
            dry_run,
        } => {
            commands::context::sync(
                &client,
                path.as_deref(),
                space.as_deref(),
                tag.as_deref(),
                max_tokens,
                dry_run,
                output,
            )
            .await
        }
        Command::Review { command } => commands::review::run(&client, command, output).await,
        Command::Plan { command } => commands::plan::run(&client, command, output).await,
        Command::Task { command } => commands::task::run(&client, command, output).await,
        Command::Plugin { command } => commands::plugin::run(command, output),
        Command::External(args) => plugin::execute(args, base_url, output).await,
    }
}
