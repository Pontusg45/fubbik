pub mod client;
pub mod commands;
pub mod config;
pub mod output;
pub mod plugin;

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
        #[arg(short, long, visible_alias = "codebase")]
        space: Option<String>,
    },
    /// Show a plan and its tasks
    Show { id: String },
    /// Create a plan
    Create {
        title: String,
        #[arg(short, long)]
        description: Option<String>,
        #[arg(short, long, visible_alias = "codebase")]
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
        #[arg(short, long, visible_alias = "codebase")]
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
        #[arg(short, long, visible_alias = "codebase")]
        space: Option<String>,
        #[arg(long, default_value = "markdown", value_parser = ["markdown", "json"])]
        format: String,
        #[arg(long = "for")]
        for_path: Option<String>,
    },
    /// Export focused context for a file
    For {
        path: PathBuf,
        #[arg(short, long, visible_alias = "codebase")]
        space: Option<String>,
        #[arg(long)]
        max_tokens: Option<usize>,
        #[arg(long, default_value = "structured-md", value_parser = ["structured-md", "structured-json", "json-legacy"])]
        format: String,
    },
    /// Generate CLAUDE.md-compatible project instructions
    ClaudeMd {
        #[arg(short, long, visible_alias = "codebase")]
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
pub enum SpaceCommand {
    /// List spaces
    List,
    /// Register the current directory as a code space
    Add { name: String },
    /// Remove a space by exact name
    Remove {
        name: String,
        #[arg(short, long)]
        force: bool,
    },
    /// Detect the space for the current directory
    Current,
}

#[derive(Subcommand)]
pub enum TagCommand {
    /// List tags with chunk counts
    List,
    /// Create a tag
    Add {
        name: String,
        #[arg(long)]
        tag_type: Option<String>,
    },
    /// Rename a tag
    Rename { id: String, name: String },
    /// Delete a tag
    Remove { id: String },
}

#[derive(Subcommand)]
pub enum RequirementCommand {
    /// List requirements
    List {
        #[arg(long)]
        status: Option<String>,
        #[arg(short, long, visible_alias = "codebase")]
        space: Option<String>,
        #[arg(long)]
        priority: Option<String>,
    },
    /// Create a requirement
    Add {
        title: String,
        #[arg(long = "step")]
        step: Vec<String>,
        #[arg(short, long, visible_alias = "codebase")]
        space: Option<String>,
        #[arg(long, value_parser = ["must", "should", "could", "wont"])]
        priority: Option<String>,
    },
    /// Update requirement status
    Status {
        id: String,
        #[arg(value_parser = ["passing", "failing", "untested"])]
        status: String,
    },
    /// Export requirements
    Export {
        #[arg(long, default_value = "gherkin", value_parser = ["gherkin", "vitest", "markdown"])]
        format: String,
        #[arg(short, long, visible_alias = "codebase")]
        space: Option<String>,
    },
    /// Report requirements that are not passing
    Verify {
        #[arg(short, long, visible_alias = "codebase")]
        space: Option<String>,
    },
}

#[derive(Subcommand)]
pub enum StaleCommand {
    /// List stale chunks
    List {
        #[arg(long)]
        reason: Option<String>,
        #[arg(short, long, visible_alias = "codebase")]
        space: Option<String>,
        #[arg(short, long, default_value = "50")]
        limit: u32,
    },
    /// Dismiss a chunk's current staleness flag
    Dismiss { id: String },
}

#[derive(Subcommand)]
pub enum DocsCommand {
    /// List imported documents
    List {
        #[arg(short, long, visible_alias = "codebase")]
        space: Option<String>,
    },
    /// Show a document and its chunks
    Show { id: String },
    /// Import one Markdown file
    Import {
        path: PathBuf,
        #[arg(short, long, visible_alias = "codebase")]
        space: Option<String>,
    },
    /// Re-import a document from its recorded source path
    Sync {
        id: String,
        #[arg(short, long, visible_alias = "codebase")]
        space: Option<String>,
    },
    /// Reconstruct a document as Markdown
    Render { id: String },
}

#[derive(Subcommand)]
pub enum ChunkCommand {
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
        #[arg(long = "space", visible_alias = "codebase", value_delimiter = ',')]
        spaces: Vec<String>,
    },
    /// Show a chunk
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
        #[arg(long = "space", visible_alias = "codebase", value_delimiter = ',')]
        spaces: Option<Vec<String>>,
    },
    /// Delete a chunk
    Remove {
        id: String,
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
    /// Search chunks
    Search {
        query: String,
        #[arg(short, long, default_value = "50")]
        limit: u32,
    },
    /// Create a connection between two chunks
    Link {
        source_id: String,
        target_id: String,
        #[arg(short, long, default_value = "related_to")]
        relation: String,
    },
    /// Delete a connection by id
    Unlink { id: String },
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
        #[arg(long = "space", visible_alias = "codebase", value_delimiter = ',')]
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
        #[arg(long = "space", visible_alias = "codebase", value_delimiter = ',')]
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
        #[arg(short, long, visible_alias = "codebase")]
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
    /// Manage spaces
    #[command(visible_alias = "codebase")]
    Space {
        #[command(subcommand)]
        command: SpaceCommand,
    },
    /// Manage tags
    #[command(visible_alias = "tags")]
    Tag {
        #[command(subcommand)]
        command: TagCommand,
    },
    /// Create a connection between two chunks
    Link {
        source_id: String,
        target_id: String,
        #[arg(short, long, default_value = "related_to")]
        relation: String,
    },
    /// Delete a connection by id
    Unlink { id: String },
    /// Manage requirements
    #[command(name = "req", visible_alias = "requirements")]
    Requirement {
        #[command(subcommand)]
        command: RequirementCommand,
    },
    /// Show server-side knowledge statistics
    Stats,
    /// Trigger AI enrichment for one chunk or all chunks
    Enrich {
        id: Option<String>,
        #[arg(long)]
        all: bool,
    },
    /// Inspect and dismiss stale knowledge
    Stale {
        #[command(subcommand)]
        command: StaleCommand,
    },
    /// Show configuration-free server and knowledge-base status
    Status,
    /// Manage imported Markdown documents
    Docs {
        #[command(subcommand)]
        command: DocsCommand,
    },
    /// Manage chunks
    Chunk {
        #[command(subcommand)]
        command: ChunkCommand,
    },
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
            let spaces = client.resolve_spaces(&spaces).await?;
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
            let spaces = match spaces {
                Some(spaces) => Some(client.resolve_spaces(&spaces).await?),
                None => None,
            };
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
        Command::Space { command } => commands::space::run(&client, command, output).await,
        Command::Tag { command } => commands::tag::run(&client, command, output).await,
        Command::Link {
            source_id,
            target_id,
            relation,
        } => commands::connection::link(&client, &source_id, &target_id, &relation, output).await,
        Command::Unlink { id } => commands::connection::unlink(&client, &id, output).await,
        Command::Requirement { command } => {
            commands::requirement::run(&client, command, output).await
        }
        Command::Stats => commands::stats::run(&client, output).await,
        Command::Enrich { id, all } => {
            commands::enrich::run(&client, id.as_deref(), all, output).await
        }
        Command::Stale { command } => commands::stale::run(&client, command, output).await,
        Command::Status => commands::status::run(&client, output).await,
        Command::Docs { command } => commands::docs::run(&client, command, output).await,
        Command::Chunk { command } => run_chunk(command, &client, output).await,
    }
}

async fn run_chunk(
    command: ChunkCommand,
    client: &client::Client,
    output: OutputMode,
) -> Result<()> {
    match command {
        ChunkCommand::Add {
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
            let spaces = client.resolve_spaces(&spaces).await?;
            commands::add::run(client, &title, &content, chunk_type, &tags, &spaces, output).await
        }
        ChunkCommand::Get { id } => commands::get::run(client, &id, output).await,
        ChunkCommand::Cat { id } => commands::get::cat(client, &id, output).await,
        ChunkCommand::Update {
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
            let spaces = match spaces {
                Some(spaces) => Some(client.resolve_spaces(&spaces).await?),
                None => None,
            };
            commands::update::run(
                client,
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
        ChunkCommand::Remove { id, yes } => commands::delete::run(client, &id, yes, output).await,
        ChunkCommand::List { r#type, limit } => {
            commands::list::run(client, r#type.as_deref(), limit, output).await
        }
        ChunkCommand::Search { query, limit } => {
            commands::search::run(client, &query, limit, output).await
        }
        ChunkCommand::Link {
            source_id,
            target_id,
            relation,
        } => commands::connection::link(client, &source_id, &target_id, &relation, output).await,
        ChunkCommand::Unlink { id } => commands::connection::unlink(client, &id, output).await,
    }
}
