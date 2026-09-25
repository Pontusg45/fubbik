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
    /// Add a task to a plan
    AddTask {
        plan_id: String,
        title: String,
        #[arg(short, long)]
        description: Option<String>,
    },
    /// Mark a plan task as done
    TaskDone { plan_id: String, task_id: String },
    /// Link a requirement to a plan
    LinkRequirement {
        plan_id: String,
        requirement_id: String,
    },
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
    Done {
        id: String,
        #[arg(short, long)]
        note: Option<String>,
    },
}

#[derive(Subcommand)]
pub enum PluginCommand {
    /// List discovered fubbik-* executables
    List,
    /// Inspect plugin discovery and protocol configuration
    Doctor,
}

#[derive(Subcommand)]
pub enum PromptCommand {
    /// List reusable prompt templates
    List,
    /// Get a prompt template by title or id
    Get { name: String },
    /// Create a reusable prompt template
    Add {
        #[arg(short, long)]
        title: String,
        #[arg(short, long, conflicts_with = "content_file")]
        content: Option<String>,
        #[arg(long, value_name = "PATH", conflicts_with = "content")]
        content_file: Option<PathBuf>,
    },
}

#[derive(Subcommand)]
pub enum HooksCommand {
    /// Install the chunk-aware pre-commit hook
    Install {
        #[arg(long)]
        force: bool,
    },
    /// Remove a pre-commit hook installed by fubbik
    Uninstall,
}

#[derive(Subcommand)]
pub enum MatrixCommand {
    /// List matrices
    List {
        #[arg(long, value_parser = ["invariant", "contract"])]
        layer: Option<String>,
    },
    /// Create a matrix
    Create {
        name: String,
        #[arg(long, value_parser = ["invariant", "contract"])]
        layer: String,
        #[arg(long)]
        description: Option<String>,
        #[arg(short, long, visible_alias = "codebase")]
        space: Option<String>,
    },
    /// Show a matrix as a grid
    Show { id: String },
    /// Add a dimension to a matrix
    AddDimension { matrix_id: String, name: String },
    /// Add a rule to a matrix
    AddRule {
        matrix_id: String,
        title: String,
        #[arg(long)]
        category: Option<String>,
        #[arg(long)]
        rationale: Option<String>,
        #[arg(long)]
        alternatives: Option<String>,
        #[arg(long)]
        consequences: Option<String>,
        #[arg(long)]
        counterexample: Option<String>,
    },
    /// Toggle whether a matrix cell is relevant
    Cell {
        matrix_id: String,
        rule_id: String,
        dimension_id: String,
    },
    /// List unspecified and violated cells
    Gaps { id: String },
    /// Link a requirement to a cell
    Link {
        cell_id: String,
        requirement_id: String,
        #[arg(long)]
        matrix: String,
    },
    /// Link a code reference to a cell
    LinkCode {
        cell_id: String,
        #[arg(long)]
        matrix: String,
        #[arg(long, value_parser = ["file", "symbol", "test"])]
        kind: String,
        #[arg(long = "ref")]
        code_ref: String,
    },
    /// Report a test result for a cell
    ReportTest {
        cell_id: String,
        #[arg(long)]
        matrix: String,
        #[arg(long)]
        test_ref: String,
        #[arg(long, value_parser = ["pass", "fail"])]
        status: String,
        #[arg(long)]
        detail: Option<String>,
    },
    /// Show a rule's version history
    History {
        rule_id: String,
        #[arg(long)]
        matrix: String,
    },
    /// Find behavioral rules governing a file
    BehaviorsFor { path: String },
}

#[derive(Subcommand)]
pub enum ContextCommand {
    /// Get context about a concept through semantic search
    About {
        concept: String,
        #[arg(short = 't', long, default_value = "8000")]
        max_tokens: usize,
        #[arg(short, long, visible_alias = "codebase")]
        space: Option<String>,
    },
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
    /// Get context scoped to a plan
    ForPlan {
        plan_id: String,
        #[arg(short = 't', long, default_value = "8000")]
        max_tokens: usize,
        #[arg(short, long, visible_alias = "codebase")]
        space: Option<String>,
    },
    /// Get context for files changed in the working tree
    ForDiff {
        #[arg(long)]
        staged: bool,
        #[arg(short = 't', long, default_value = "8000")]
        max_tokens: usize,
        #[arg(short, long, visible_alias = "codebase")]
        space: Option<String>,
    },
    /// Generate context for files in a directory
    Dir {
        directory: PathBuf,
        #[arg(short, long, visible_alias = "codebase")]
        space: Option<String>,
        #[arg(short, long)]
        output: Option<PathBuf>,
        #[arg(long, default_value = "200")]
        max_files: usize,
        #[arg(short = 't', long)]
        max_tokens: Option<usize>,
    },
    /// Manage frozen context snapshots
    Snapshot {
        #[command(subcommand)]
        command: ContextSnapshotCommand,
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
pub enum ContextSnapshotCommand {
    /// Create a frozen context snapshot
    Create {
        #[arg(long)]
        plan: Option<String>,
        #[arg(long)]
        task: Option<String>,
        #[arg(long)]
        about: Option<String>,
        #[arg(long, value_delimiter = ',')]
        files: Vec<String>,
        #[arg(long, default_value = "8000")]
        max_tokens: usize,
        #[arg(short, long, visible_alias = "codebase")]
        space: Option<String>,
    },
    /// Retrieve a frozen context snapshot
    Get { snapshot_id: String },
    /// List frozen context snapshots
    List,
    /// Delete a frozen context snapshot
    Delete { snapshot_id: String },
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
    /// Identify and optionally fix tag variants, broad tags, and filename tags
    Normalize {
        #[arg(long)]
        confirm: bool,
        #[arg(short, long, visible_alias = "codebase")]
        space: Option<String>,
    },
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
    /// Import requirements from a Gherkin feature file
    Import {
        file: PathBuf,
        #[arg(short, long, visible_alias = "codebase")]
        space: Option<String>,
        #[arg(long, default_value = "should", value_parser = ["must", "should", "could", "wont"])]
        priority: String,
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
    /// Extract source comments, or import a versioned source manifest JSON
    Extract {
        /// Project root with --language; otherwise a manifest JSON file
        path: PathBuf,
        #[arg(long, value_parser = ["javascript", "typescript", "java"])]
        language: Option<String>,
        #[arg(long, requires = "language")]
        project: Option<String>,
        #[arg(short, long)]
        space: Option<String>,
        /// Print the validated manifest without contacting the API
        #[arg(long)]
        preview: bool,
    },
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
    /// Recursively import Markdown files from a directory
    ImportDir {
        dir: PathBuf,
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
pub enum GenerateCommand {
    /// Generate CLAUDE.md instructions
    #[command(name = "claude.md")]
    ClaudeMd {
        #[arg(short, long)]
        space: String,
    },
    /// Generate AGENTS.md instructions
    #[command(name = "agents.md")]
    AgentsMd {
        #[arg(short, long)]
        space: String,
    },
    /// Generate Cursor rules
    Cursorrules {
        #[arg(short, long)]
        space: String,
    },
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
    /// Quickly create a chunk from a title and optional piped content
    Quick {
        #[arg(num_args = 0..)]
        title_words: Vec<String>,
        #[arg(short, long)]
        title: Option<String>,
        #[arg(long = "type", default_value = "note")]
        chunk_type: String,
        #[arg(long, value_delimiter = ',')]
        tags: Vec<String>,
        #[arg(long)]
        global: bool,
        #[arg(short, long, visible_alias = "codebase")]
        space: Option<String>,
        #[arg(long = "tag")]
        update_tag: Option<String>,
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
    /// Scan this project and populate its knowledge base
    Setup {
        /// Server URL (prefer the global --url option for new scripts)
        #[arg(long)]
        server: Option<String>,
        #[arg(long)]
        dry_run: bool,
        #[arg(short = 'y', long)]
        yes: bool,
        #[arg(long)]
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
    /// Generate instruction files from the knowledge base
    Generate {
        #[command(subcommand)]
        command: GenerateCommand,
    },
    /// List chunk updates grouped by update tag
    Updates {
        #[arg(long, conflicts_with = "tags")]
        tag: Option<String>,
        #[arg(long, conflicts_with = "tag")]
        tags: bool,
        #[arg(long)]
        space_id: Option<String>,
    },
    /// Show the knowledge and decisions associated with a file
    Why {
        path: PathBuf,
        #[arg(short, long, visible_alias = "codebase")]
        space: Option<String>,
    },
    /// Suggest knowledge chunks for a source file
    Suggest {
        path: PathBuf,
        #[arg(short, long, visible_alias = "codebase")]
        space: Option<String>,
    },
    /// Find source files with no associated knowledge
    Gaps {
        #[arg(default_value = ".")]
        directory: PathBuf,
        #[arg(short, long, visible_alias = "codebase")]
        space: Option<String>,
        #[arg(long, default_value = "30")]
        limit: usize,
    },
    /// Summarize recent knowledge base changes
    Recap {
        #[arg(long, default_value = "7d")]
        since: String,
        #[arg(short, long, visible_alias = "codebase")]
        space: Option<String>,
    },
    /// Show knowledge base changes since a date
    KbDiff {
        #[arg(long, default_value = "7d")]
        since: String,
        #[arg(short, long, visible_alias = "codebase")]
        space: Option<String>,
    },
    /// Check files for directly associated knowledge chunks
    CheckFiles {
        files: Vec<String>,
        #[arg(long)]
        staged: bool,
    },
    /// Import chunks from a JSONL file
    BulkAdd {
        #[arg(long, value_name = "PATH")]
        file: PathBuf,
    },
    /// Extract conventions from an instruction file and create chunks
    SeedConventions {
        #[arg(long, default_value = "CLAUDE.md")]
        file: PathBuf,
        #[arg(long)]
        dry_run: bool,
    },
    /// Import chunks from JSON or Markdown
    Import {
        path: PathBuf,
        #[arg(short, long, visible_alias = "codebase")]
        space: Option<String>,
        #[arg(long = "type", default_value = "document")]
        default_type: String,
        #[arg(long = "no-recursive", action = clap::ArgAction::SetFalse, default_value_t = true)]
        recursive: bool,
    },
    /// Export the knowledge base
    Export {
        #[arg(long, default_value = "json", value_parser = ["json", "md"])]
        format: String,
        #[arg(long, default_value = "export")]
        out: PathBuf,
        #[arg(short, long, visible_alias = "codebase")]
        space: Option<String>,
    },
    /// Generate a static HTML site from the knowledge base
    ExportSite {
        #[arg(short, long, default_value = "fubbik-site")]
        output: PathBuf,
        #[arg(short, long, visible_alias = "codebase")]
        space: Option<String>,
    },
    /// Check knowledge chunks for quality issues
    Lint {
        #[arg(short, long, visible_alias = "codebase")]
        space: Option<String>,
        #[arg(long)]
        fix: bool,
        #[arg(long)]
        score: bool,
    },
    /// Identify and optionally remove low-value chunks
    Cleanup {
        #[arg(long)]
        confirm: bool,
        #[arg(long = "type")]
        chunk_type: Option<String>,
        #[arg(short, long, visible_alias = "codebase")]
        space: Option<String>,
    },
    /// Watch file changes and show associated knowledge
    Watch {
        #[arg(long, default_value = ".")]
        dir: PathBuf,
    },
    /// Manage behavioral specification matrices
    Matrix {
        #[command(subcommand)]
        command: MatrixCommand,
    },
    /// Manage chunk-aware Git hooks
    Hooks {
        #[command(subcommand)]
        command: HooksCommand,
    },
    /// Manage reusable prompt templates
    Prompt {
        #[command(subcommand)]
        command: PromptCommand,
    },
    /// Regenerate the configured CLAUDE.md context file
    #[command(visible_alias = "sync-claude-md")]
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
        /// Include only global, unscoped chunks
        #[arg(long)]
        global: bool,
        /// Regenerate continuously
        #[arg(long)]
        watch: bool,
        /// Polling interval for --watch
        #[arg(long, default_value = "30", requires = "watch")]
        interval: u64,
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
    /// Open the web application, a named page, or a chunk
    Open { target: Option<String> },
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
        Command::Quick {
            title_words,
            title,
            chunk_type,
            tags,
            global,
            space,
            update_tag,
        } => {
            use std::io::{IsTerminal, Read};

            let title = title.unwrap_or_else(|| title_words.join(" "));
            if title.trim().is_empty() {
                anyhow::bail!("title is required; pass it as arguments or use --title");
            }
            let mut content = String::new();
            if !std::io::stdin().is_terminal() {
                std::io::stdin().read_to_string(&mut content)?;
            }
            let space = if global {
                None
            } else {
                client.resolve_space(space.as_deref()).await?
            };
            let spaces = space.into_iter().collect::<Vec<_>>();
            let chunk = client
                .create_chunk_with_update_tag(
                    &title,
                    &content,
                    &chunk_type,
                    &tags,
                    &spaces,
                    update_tag.as_deref(),
                )
                .await?;
            if !output::id_or_json(output, &chunk.id, &chunk)? {
                println!("created {} {}", chunk.id, chunk.title);
            }
            Ok(())
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
        Command::Setup {
            server,
            dry_run,
            yes,
            force,
        } => {
            let setup_client = server
                .as_deref()
                .map(client::Client::new)
                .unwrap_or_else(|| client::Client::new(base_url));
            commands::setup::run(&setup_client, dry_run, yes, force, output).await
        }
        Command::Doctor => commands::config::doctor(&client, output).await,
        Command::Config { command } => commands::config::run(command, output),
        Command::Context { command } => commands::context::run(&client, command, output).await,
        Command::Generate { command } => commands::generate::run(&client, command, output).await,
        Command::Updates {
            tag,
            tags,
            space_id,
        } => {
            commands::updates::run(&client, tag.as_deref(), tags, space_id.as_deref(), output).await
        }
        Command::Why { path, space } => {
            commands::why::run(&client, &path, space.as_deref(), output).await
        }
        Command::Suggest { path, space } => {
            commands::suggest::run(&client, &path, space.as_deref(), output).await
        }
        Command::Gaps {
            directory,
            space,
            limit,
        } => commands::gaps::run(&client, &directory, space.as_deref(), limit, output).await,
        Command::Recap { since, space } => {
            commands::recap::run(&client, &since, space.as_deref(), output).await
        }
        Command::KbDiff { since, space } => {
            commands::kb_diff::run(&client, &since, space.as_deref(), output).await
        }
        Command::CheckFiles { files, staged } => {
            commands::check_files::run(&client, files, staged, output).await
        }
        Command::BulkAdd { file } => commands::bulk_add::run(&client, &file, output).await,
        Command::SeedConventions { file, dry_run } => {
            commands::seed_conventions::run(&client, &file, dry_run, output).await
        }
        Command::Import {
            path,
            space,
            default_type,
            recursive,
        } => {
            commands::import::run(
                &client,
                &path,
                space.as_deref(),
                &default_type,
                recursive,
                output,
            )
            .await
        }
        Command::Export { format, out, space } => {
            commands::export::run(&client, &format, &out, space.as_deref(), output).await
        }
        Command::ExportSite {
            output: output_dir,
            space,
        } => commands::export_site::run(&client, &output_dir, space.as_deref(), output).await,
        Command::Lint { space, fix, score } => {
            commands::lint::run(&client, space.as_deref(), fix, score, output).await
        }
        Command::Cleanup {
            confirm,
            chunk_type,
            space,
        } => {
            commands::cleanup::run(
                &client,
                confirm,
                chunk_type.as_deref(),
                space.as_deref(),
                output,
            )
            .await
        }
        Command::Watch { dir } => commands::watch::run(&client, &dir, output).await,
        Command::Matrix { command } => commands::matrix::run(&client, command, output).await,
        Command::Hooks { command } => commands::hooks::run(command, output),
        Command::Prompt { command } => commands::prompt::run(&client, command, output).await,
        Command::Sync {
            output: path,
            space,
            tag,
            max_tokens,
            dry_run,
            global,
            watch,
            interval,
        } => {
            if interval == 0 {
                anyhow::bail!("watch interval must be greater than zero");
            }
            loop {
                commands::context::sync(
                    &client,
                    commands::context::SyncOptions {
                        output_path: path.as_deref(),
                        space: space.as_deref(),
                        tag: tag.as_deref(),
                        max_tokens,
                        dry_run,
                        global,
                    },
                    output,
                )
                .await?;
                if !watch {
                    break Ok(());
                }
                tokio::time::sleep(std::time::Duration::from_secs(interval)).await;
            }
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
        Command::Open { target } => commands::open::run(target.as_deref(), output),
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
