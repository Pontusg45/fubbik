//! The main window. `Workspace` owns all app state; the poll loop refreshes
//! repo snapshots on the background executor and pushes them back here.

mod actions;
mod agents;
mod changes;
mod chrome;
mod command_palette;
mod commands;
mod deps_view;
mod digest_view;
mod fields;
mod hotkeys;
mod insights;
pub mod keymap;
mod knowledge;
mod knowledge_graph;
mod lists;
mod mcp_publish;
mod palette;
mod plans;
mod polling;
mod preferences;
mod projects;
mod prs;
mod run_prompt;
mod scripts_view;
mod setup_view;
mod sidebar;
mod sync;
mod tasks;
mod terminal_view;
mod terminals;
mod tests_view;
mod text;
mod tmux_live;
pub mod today;
mod tools_view;
#[cfg(test)]
mod ui_tests;
mod views;
mod widgets;

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use gpui::{
    ClipboardItem, Context, ExternalPaths, FocusHandle, IntoElement, KeyDownEvent,
    PathPromptOptions, ScrollHandle, SharedString, Task, Window, actions, div, prelude::*, px,
};

use crate::config::{self, AppConfig, RepoEntry};
use crate::fubbik::{ApiHealth, ChunkRef, FubbikClient, Knowledge};
use crate::model::{RepoSnapshot, Status, process_key};
use crate::services::ServicesFile;
use crate::sessions::{AiSession, SessionIndex, SessionState};
use crate::supervisor::{self, Supervisor};
use crate::{claude_md, detect, gaps, git, launch, poll, setup, theme, tmux, worktree};

use palette::{Command, Confirm, Entry, RunTarget, RunWhat};
use text::{KeyOutcome, TextField, apply_key};

actions!(
    fubbik_desktop,
    [
        Quit,
        QuitKeepServer,
        TogglePalette,
        NextRepo,
        PrevRepo,
        ShowOverview,
        ShowProcesses,
        ShowDatabases,
        ShowLogs,
        ShowKnowledge,
        ShowWorktrees,
        ShowSessions,
        ShowPlans,
        ShowSetup,
        ShowTerminal,
        ShowAgents,
        ShowToday,
        ShowInsights,
        ShowChanges,
        ShowTools,
        ShowDeps,
        ShowTests,
        ShowHome,
        CaptureNote,
        ToggleTheme,
        ChooseTheme,
        RegisterRepo,
        Refresh,
        ShowCheatSheet,
        FocusSidebar,
        NextNeedsYou,
        ReplyLatest,
        StartNextTask,
        SelectRepo1,
        SelectRepo2,
        SelectRepo3,
        SelectRepo4,
        SelectRepo5,
        SelectRepo6,
        SelectRepo7,
        SelectRepo8,
        SelectRepo9,
        NewShell,
        CloseTerminalTab,
        PrevTerminalTab,
        NextTerminalTab,
        SplitTerminal,
        ClearTerminal,
        TerminalTab1,
        TerminalTab2,
        TerminalTab3,
        TerminalTab4,
        TerminalTab5,
        TerminalTab6,
        TerminalTab7,
        TerminalTab8,
        TerminalTab9,
    ]
);

const TICK: Duration = Duration::from_secs(1);
/// Probe every third tick; logs refresh every tick.
const PROBE_EVERY: u64 = 3;
/// fubbik knowledge every 30 ticks.
const KNOWLEDGE_EVERY: u64 = 30;
/// The macOS titlebar the content runs under.
const TITLEBAR_HEIGHT: f32 = 28.0;
/// Knowledge fetches back off this way while the fubbik API is down.
const API_BACKOFF_MIN: Duration = Duration::from_secs(60);
const API_BACKOFF_MAX: Duration = Duration::from_secs(300);
/// File changes wait this long for more before triggering one poll.
const WATCH_DEBOUNCE: Duration = Duration::from_millis(300);
/// A launched agent counts as running this long, until its session appears.
const RECENT_LAUNCH: Duration = Duration::from_secs(30);
const TOAST_FOR: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Overview,
    Processes,
    Databases,
    Logs,
    Worktrees,
    Sessions,
    Plans,
    Knowledge,
    Setup,
    Terminal,
    /// Every agent in every repo.
    Agents,
    /// What needs you, what is in flight and what is next, across repos.
    Today,
    /// One repo's summary, health and activity.
    Insights,
    /// Changed files, diffs, staging and committing.
    Changes,
    /// The machine's developer tools and services (a Home tab).
    Tools,
    /// Dependencies across repos, and where they disagree (a Home tab).
    Deps,
    /// Test runners, failing tests, rerun and fix.
    Tests,
}

impl Tab {
    pub const ALL: [Tab; 17] = [
        Tab::Today,
        Tab::Overview,
        Tab::Changes,
        Tab::Tests,
        Tab::Processes,
        Tab::Logs,
        Tab::Worktrees,
        Tab::Sessions,
        Tab::Plans,
        Tab::Knowledge,
        Tab::Terminal,
        Tab::Databases,
        Tab::Insights,
        Tab::Agents,
        Tab::Tools,
        Tab::Deps,
        Tab::Setup,
    ];

    /// The tab's own color: its dot and underline in the tab strip.
    pub fn color(self) -> gpui::Hsla {
        use theme::Hue;
        match self {
            Tab::Overview | Tab::Today => theme::hue(Hue::Blue),
            Tab::Processes => theme::hue(Hue::Green),
            Tab::Databases => theme::hue(Hue::Cyan),
            Tab::Logs | Tab::Terminal | Tab::Changes => theme::hue(Hue::Lime),
            Tab::Worktrees => theme::hue(Hue::Violet),
            Tab::Sessions | Tab::Agents => theme::hue(Hue::Orange),
            Tab::Plans | Tab::Insights => theme::hue(Hue::Pink),
            Tab::Knowledge => theme::hue(Hue::Yellow),
            Tab::Setup | Tab::Tools => theme::text_muted(),
            Tab::Deps => theme::hue(Hue::Violet),
            Tab::Tests => theme::hue(Hue::Green),
        }
    }

    /// Views across every repo (shown outside a repo, next to Home).
    pub fn is_global(self) -> bool {
        matches!(self, Tab::Today | Tab::Agents | Tab::Tools | Tab::Deps)
    }

    /// The key that shows the tab, as currently bound (keymap.toml).
    pub fn shortcut(self) -> String {
        keymap::keys_for(self.action_id()).unwrap_or_default()
    }

    /// The tab's action in the keymap.
    pub fn action_id(self) -> &'static str {
        match self {
            Tab::Overview => "show_overview",
            Tab::Processes => "show_processes",
            Tab::Databases => "show_databases",
            Tab::Logs => "show_logs",
            Tab::Worktrees => "show_worktrees",
            Tab::Sessions => "show_sessions",
            Tab::Plans => "show_plans",
            Tab::Knowledge => "show_knowledge",
            Tab::Setup => "show_setup",
            Tab::Terminal => "show_terminal",
            Tab::Agents => "show_agents",
            Tab::Today => "show_today",
            Tab::Insights => "show_insights",
            Tab::Changes => "show_changes",
            Tab::Tools => "show_tools",
            Tab::Deps => "show_deps",
            Tab::Tests => "show_tests",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Tab::Overview => "Overview",
            Tab::Processes => "Processes",
            Tab::Databases => "Databases",
            Tab::Logs => "Logs",
            Tab::Worktrees => "Worktrees",
            Tab::Sessions => "AI Sessions",
            Tab::Plans => "Plans",
            Tab::Knowledge => "Knowledge",
            Tab::Setup => "Setup",
            Tab::Terminal => "Terminal",
            Tab::Agents => "All agents",
            Tab::Today => "Today",
            Tab::Insights => "Insights",
            Tab::Changes => "Changes",
            Tab::Tools => "Tools",
            Tab::Deps => "Dependencies",
            Tab::Tests => "Tests",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Field {
    Palette,
    LogFilter,
    ScriptFilter,
    NoteTitle,
    NoteBody,
    Group,
    NewWorktree,
    Reply,
    FubbikUrl,
    WebUrl,
    FubbikBin,
    Commit,
    GraphSearch,
    BranchFilter,
}

/// One repo's poll. `knowledge` and `setup` only come with slow polls.
struct PollResult {
    path: PathBuf,
    snapshot: RepoSnapshot,
    knowledge: Option<Result<Knowledge, String>>,
    setup: Option<RepoSetup>,
}

/// Work started on demand, shown while it runs.
#[derive(Debug, Clone)]
pub enum Loadable<T> {
    Loading,
    Done(Result<T, String>),
}

/// The generated CLAUDE.md and each worktree's copy of it.
#[derive(Debug, Clone)]
pub struct ClaudeMdSync {
    pub config: claude_md::ClaudeMdConfig,
    pub generated: Result<String, String>,
    pub worktrees: Vec<(PathBuf, claude_md::MdState)>,
}

/// Local setup for fubbik's agent tools, re-read with the slow poll.
#[derive(Debug, Clone)]
pub struct RepoSetup {
    pub mcp: setup::McpStatus,
    pub hooks: Vec<(PathBuf, setup::HookStatus)>,
    /// None: no `claudeMd` in fubbik.config.json.
    pub claude_md: Option<ClaudeMdSync>,
}

/// What a worktree's changes touch in the knowledge base.
#[derive(Debug, Clone, Default)]
pub struct BranchContext {
    pub files: Vec<String>,
    /// `for-files` context, by section.
    pub sections: Vec<(String, Vec<ChunkRef>)>,
    /// `check-files`: chunks linked to a changed file by file-ref or
    /// applies-to, with the files that matched.
    pub linked: Vec<(ChunkRef, Vec<String>)>,
}

/// A modal with text, such as a dropped file's context.
pub struct Panel {
    pub title: String,
    pub body: Loadable<String>,
    /// Buttons under the title, e.g. "Open PR".
    pub actions: Vec<(String, Command)>,
}

/// The server and the local binary, re-read with the slow poll.
#[derive(Debug, Clone)]
pub struct GlobalStatus {
    pub api: Result<(ApiHealth, Vec<String>), String>,
    pub fubbik_bin: Option<PathBuf>,
}

struct RepoView {
    entry: RepoEntry,
    snapshot: Option<RepoSnapshot>,
    /// None until fetched. Err: fubbik is not reachable.
    knowledge: Option<Result<Knowledge, String>>,
    setup: Option<RepoSetup>,
    branch_context: HashMap<PathBuf, Loadable<BranchContext>>,
    gaps: Option<Loadable<gaps::GapReport>>,
    insights: Option<Loadable<insights::Insights>>,
    /// Known vulnerabilities from the last dependency audit.
    audit: Option<usize>,
    /// Insights are being recomputed (the last ones stay on screen).
    insights_refreshing: bool,
    /// A dependency audit is running.
    auditing: bool,
    /// Pull requests and their checks, by branch (GitHub remotes only).
    prs: prs::RepoPrs,
    /// Background fetch and pull state.
    sync: sync::SyncState,
}

impl RepoView {
    fn new(entry: RepoEntry) -> Self {
        Self {
            entry,
            snapshot: None,
            knowledge: None,
            setup: None,
            branch_context: HashMap::new(),
            gaps: None,
            insights: None,
            insights_refreshing: false,
            auditing: false,
            prs: Default::default(),
            sync: Default::default(),
            audit: None,
        }
    }

    fn space_id(&self) -> Option<String> {
        self.knowledge
            .as_ref()?
            .as_ref()
            .ok()?
            .space
            .as_ref()
            .map(|s| s.id.clone())
    }
}

/// One embedded terminal: a shell in `dir`, maybe running an AI session.
pub struct TermTab {
    pub id: usize,
    pub dir: PathBuf,
    pub label: String,
    pub view: gpui::Entity<terminal_view::TerminalView>,
    /// The tmux session behind it (on the app's server), when persistent.
    pub tmux: Option<String>,
}

struct Toast {
    message: SharedString,
    error: bool,
    at: Instant,
    /// Clicking the toast runs this (e.g. jump to the session it is about).
    action: Option<Command>,
}

/// Toasts kept at once.
const TOASTS_MAX: usize = 4;

/// An agent launch waiting for a free slot.
#[derive(Debug, Clone)]
pub struct QueuedLaunch {
    pub dir: PathBuf,
    pub label: String,
    pub command: String,
}

/// A notification's in-app side: a toast you can click.
pub struct Alert {
    pub message: String,
    pub target: Command,
}

/// Screen-change tracking for agents without a status registry (Codex).
#[derive(Default)]
struct Quiet {
    preview: String,
    unchanged_polls: u32,
    /// The screen changed at least once (the agent was working).
    was_working: bool,
}

pub struct Workspace {
    config: AppConfig,
    config_path: PathBuf,
    repos: Vec<RepoView>,
    selected: usize,
    tab: Tab,
    /// Home: every project and the computer's stats, instead of one repo.
    home: bool,
    /// The last repo tab shown, for coming back from Today or All agents.
    repo_tab: Tab,
    supervisor: Supervisor,
    client: FubbikClient,
    focus: FocusHandle,
    palette_field: TextField,
    log_filter: TextField,
    script_filter: TextField,
    /// Scripts run last, per repo path, newest first (the "Recent" row).
    script_recent: std::collections::HashMap<PathBuf, Vec<String>>,
    note_title: TextField,
    note_body: TextField,
    group_field: TextField,
    worktree_field: TextField,
    commit_field: TextField,
    graph_search: TextField,
    branch_filter: TextField,
    /// Home → Tools: the last check, and whether one is running.
    tools: Option<Loadable<crate::tools::Report>>,
    tools_refreshing: bool,
    /// Home → Dependencies.
    deps: deps_view::DepsState,
    /// The Knowledge tab's graph.
    graph: knowledge_graph::GraphState,
    /// The Tests tab: runners and runs per repo.
    tests: tests_view::TestsState,
    /// `gh` is on PATH (checked once at startup).
    gh_available: bool,
    changes: changes::ChangesState,
    /// Transcript read positions, kept across polls.
    session_index: Arc<Mutex<SessionIndex>>,
    /// Re-read worktrees on the next poll (they are slower than the probes).
    refresh_worktrees: bool,
    palette_open: bool,
    palette_selected: usize,
    /// Chunk search results for a palette query starting with `?`.
    palette_search: Vec<ChunkRef>,
    /// A `?` search is in flight.
    palette_searching: bool,
    palette_search_error: Option<String>,
    /// Commands run from the palette, newest first; listed first when the
    /// query is empty.
    palette_recent: Vec<Command>,
    /// The built-in server's status at the last tick, to notice changes.
    backend_seen: crate::backend::Status,
    /// Script rows unfolded past "+N more" (`repo:tool:group`).
    scripts_expanded: std::collections::HashSet<String>,
    global: Option<GlobalStatus>,
    panel: Option<Panel>,
    /// The worktree whose branch context is shown.
    expanded_worktree: Option<PathBuf>,
    /// Embedded shells, one per directory, kept while the app runs.
    terminals: Vec<TermTab>,
    next_terminal_id: usize,
    /// The terminal tab shown, when it belongs to the selected repo.
    active_terminal: Option<usize>,
    /// Terminal tabs are tmux sessions (tmux is installed and not turned off).
    tmux_terminals: bool,
    reply_field: TextField,
    fubbik_url_field: TextField,
    web_url_field: TextField,
    fubbik_bin_field: TextField,
    /// The plan the board shows (else the first in progress).
    board_plan: Option<String>,
    /// The task card whose checklist is expanded.
    open_checklist: Option<String>,
    /// Agent launches waiting for a free slot (`max_agents`).
    launch_queue: std::collections::VecDeque<QueuedLaunch>,
    /// Agents launched in the last moments, counted as running until their
    /// session shows up in the registry.
    recent_launches: Vec<Instant>,
    /// Days the Insights summary covers (1: standup, 7: week).
    insights_days: i64,
    /// The quick-capture note overlay is open.
    capture_open: bool,
    /// "Where should this run?" while open.
    run_prompt: Option<RunWhat>,
    /// The prompt's "always use this" checkbox.
    run_prompt_remember: bool,
    /// The ⌘/ cheat sheet is open.
    cheat_sheet_open: bool,
    /// ⇧⌘E: j/k move through the repos until ⏎ or esc.
    sidebar_nav: bool,
    /// The highlighted card on the plan board: (column, row).
    board_cursor: Option<(usize, usize)>,
    /// Where ⇧⌘J goes next among "Needs you".
    needs_cursor: usize,
    /// A second terminal shown beside the active one (Split).
    split_with: Option<usize>,
    /// The highlighted row in AI Sessions (j/k, enter).
    list_cursor: Option<usize>,
    /// Per agent session id (Codex): how long its screen has been still.
    quiet: HashMap<String, Quiet>,
    /// How long knowledge fetches wait after the API failed (doubles).
    api_backoff: Duration,
    /// Watches services files and Claude's live-session registry.
    _watcher: Option<notify::RecommendedWatcher>,
    _watch_task: Task<()>,
    _events_task: Task<()>,
    /// Terminal tabs that rang the bell and have not been looked at since.
    bells: Vec<(usize, Instant)>,
    /// Watch alerts already toasted.
    alerts_seen: HashSet<String>,
    alerts_announced: bool,
    /// When a title-bar double-click last zoomed the window.
    titlebar_zoomed_at: Option<Instant>,
    /// The daily digest is being saved.
    digest_saving: bool,
    knowledge_paused_until: Option<Instant>,
    /// Ghostty theme names, read once at startup.
    ghostty_themes: Vec<String>,
    /// The agent's tmux pane a reply goes to.
    reply_target: Option<tmux::Pane>,
    /// Focus the terminal on the next render (after switching to it).
    focus_terminal: bool,
    /// The window background last applied; follows the theme in `render`.
    applied_translucent: Option<bool>,
    log_key: Option<String>,
    /// A frozen copy of the log while paused.
    log_paused: Option<Vec<String>>,
    log_scroll: ScrollHandle,
    /// The repo tab strip's sideways scroll, and the tab last scrolled into view.
    tabs_scroll: ScrollHandle,
    tabs_scrolled_to: std::cell::Cell<Option<usize>>,
    home_tabs_scroll: ScrollHandle,
    /// The log follows new output only while it was at the bottom.
    log_follow: bool,
    /// Processes the user stopped, so their exit is not reported as a crash.
    user_stopped: HashSet<String>,
    /// Newest last; info fades after `TOAST_FOR`, errors stay until closed.
    toasts: Vec<Toast>,
    /// How each process or script last ended, for "exited 1 · 2m ago".
    last_exit: HashMap<String, (Option<i32>, Instant)>,
    /// Runs started in a terminal tab or Terminal.app (the app cannot see them).
    launched_in: HashMap<String, RunTarget>,
    /// Starts and stops the next poll has not confirmed yet ("Starting…").
    pending: HashMap<String, (Instant, &'static str)>,
    /// Knowledge lists every stale flag, not just the first few.
    knowledge_all: bool,
    /// "Enrich all" is running (since).
    enriching: Option<Instant>,
    /// The board shows its finished tasks (folded by default).
    board_done_open: bool,
    /// AI Sessions lists every ended session, not just the last few.
    sessions_all: bool,
    /// Today shows only this repo's lines (None: every repo).
    today_filter: Option<String>,
    /// Home: every listening port, not just the first few.
    ports_all: bool,
    /// "Are you sure?" while open.
    confirm: Option<Confirm>,
    /// True while a confirmed command runs, so its handler does not ask again.
    confirmed: bool,
    polling: bool,
    poll_again: bool,
    tick: u64,
    _ticker: Task<()>,
}

impl Workspace {
    pub fn new(supervisor: Supervisor, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let config_path = config::path();
        let (config, config_error) = match AppConfig::load_from(&config_path) {
            Ok(config) => (config, None),
            Err(error) => (AppConfig::default(), Some(error)),
        };
        let client = FubbikClient::new(config.fubbik_url());
        let repos = config.repos.iter().cloned().map(RepoView::new).collect();
        let focus = cx.focus_handle();
        focus.focus(window);

        let ticker = cx.spawn(async move |this, cx| {
            loop {
                if this.update(cx, |this, cx| this.on_tick(cx)).is_err() {
                    break;
                }
                cx.background_executor().timer(TICK).await;
            }
        });

        let mut workspace = Self {
            config,
            config_path,
            repos,
            selected: 0,
            tab: Tab::Overview,
            home: true,
            repo_tab: Tab::Overview,
            supervisor,
            client,
            focus,
            palette_field: TextField::new(cx),
            log_filter: TextField::new(cx),
            script_filter: TextField::new(cx),
            script_recent: Default::default(),
            note_title: TextField::new(cx),
            note_body: TextField::new(cx),
            group_field: TextField::new(cx),
            worktree_field: TextField::new(cx),
            commit_field: TextField::new(cx),
            graph_search: TextField::new(cx),
            branch_filter: TextField::new(cx),
            tools: None,
            tools_refreshing: false,
            deps: Default::default(),
            graph: Default::default(),
            tests: Default::default(),
            gh_available: std::process::Command::new("gh")
                .arg("--version")
                .output()
                .is_ok_and(|o| o.status.success()),
            changes: Default::default(),
            session_index: Arc::new(Mutex::new(SessionIndex::new())),
            refresh_worktrees: true,
            palette_open: false,
            palette_selected: 0,
            palette_search: Vec::new(),
            palette_searching: false,
            palette_search_error: None,
            palette_recent: Vec::new(),
            backend_seen: crate::backend::status(),
            scripts_expanded: Default::default(),
            global: None,
            panel: None,
            expanded_worktree: None,
            applied_translucent: None,
            terminals: Vec::new(),
            next_terminal_id: 0,
            active_terminal: None,
            tmux_terminals: false,
            reply_field: TextField::new(cx),
            fubbik_url_field: TextField::new(cx),
            web_url_field: TextField::new(cx),
            fubbik_bin_field: TextField::new(cx),
            list_cursor: None,
            split_with: None,
            board_plan: None,
            open_checklist: None,
            launch_queue: Default::default(),
            recent_launches: Vec::new(),
            insights_days: 1,
            capture_open: false,
            run_prompt: None,
            run_prompt_remember: false,
            cheat_sheet_open: false,
            sidebar_nav: false,
            needs_cursor: 0,
            board_cursor: None,
            quiet: HashMap::new(),
            api_backoff: Duration::ZERO,
            _watcher: None,
            _watch_task: Task::ready(()),
            _events_task: Task::ready(()),
            bells: Vec::new(),
            alerts_seen: HashSet::new(),
            alerts_announced: false,
            titlebar_zoomed_at: None,
            digest_saving: false,
            knowledge_paused_until: None,
            ghostty_themes: crate::ghostty::list_themes(&crate::ghostty::theme_dirs()),
            reply_target: None,
            focus_terminal: false,
            log_key: None,
            log_paused: None,
            log_scroll: ScrollHandle::new(),
            tabs_scroll: ScrollHandle::new(),
            tabs_scrolled_to: std::cell::Cell::new(None),
            home_tabs_scroll: ScrollHandle::new(),
            log_follow: true,
            user_stopped: HashSet::new(),
            toasts: Vec::new(),
            ports_all: false,
            today_filter: None,
            sessions_all: false,
            board_done_open: false,
            knowledge_all: false,
            enriching: None,
            last_exit: HashMap::new(),
            launched_in: HashMap::new(),
            pending: HashMap::new(),
            confirm: None,
            confirmed: false,
            polling: false,
            poll_again: false,
            tick: 0,
            _ticker: ticker,
        };
        if let Some(error) = config_error {
            workspace.show_error(format!(
                "Could not read the config, starting empty: {error}"
            ));
        }
        workspace.sync_group_field();
        workspace.sync_preference_fields();
        workspace.watch_files(cx);
        workspace.tmux_terminals = workspace.config.tmux_persistence() && tmux::available();
        if workspace.tmux_terminals {
            workspace.restore_terminals(cx);
            workspace.watch_tmux_events(cx);
        }
        // A developer hook for smoke runs: open on a tab by name.
        if !workspace.repos.is_empty() {
            workspace.tab = Tab::Today;
        }
        if std::env::var("FUBBIK_DESKTOP_TAB").is_ok_and(|t| !t.eq_ignore_ascii_case("home")) {
            workspace.home = false;
        }
        if let Ok(name) = std::env::var("FUBBIK_DESKTOP_TAB")
            && let Some(tab) = Tab::ALL
                .into_iter()
                .find(|t| t.label().eq_ignore_ascii_case(&name))
        {
            workspace.tab = tab;
            workspace.focus_terminal = tab == Tab::Terminal;
        }
        workspace
    }

    // ── state helpers ──────────────────────────────────────────────────

    fn push_toast(&mut self, toast: Toast) {
        // The same message again refreshes it instead of stacking.
        self.toasts.retain(|t| t.message != toast.message);
        self.toasts.push(toast);
        if self.toasts.len() > TOASTS_MAX {
            self.toasts.remove(0);
        }
    }

    fn show_error(&mut self, message: impl Into<SharedString>) {
        self.push_toast(Toast {
            message: message.into(),
            error: true,
            at: Instant::now(),
            action: None,
        });
    }

    fn show_info(&mut self, message: impl Into<SharedString>) {
        self.push_toast(Toast {
            message: message.into(),
            error: false,
            at: Instant::now(),
            action: None,
        });
    }

    /// A toast that does something when clicked.
    fn show_alert(&mut self, alert: Alert) {
        self.push_toast(Toast {
            message: alert.message.into(),
            error: false,
            at: Instant::now(),
            action: Some(alert.target),
        });
    }

    /// The newest toast.
    #[cfg(test)]
    fn toast(&self) -> Option<&Toast> {
        self.toasts.last()
    }

    /// For a destructive handler: true when the user already confirmed;
    /// otherwise opens "Are you sure?" for `command` and returns false.
    fn confirmed_or_ask(
        &mut self,
        title: impl Into<String>,
        detail: impl Into<String>,
        action: &str,
        command: Command,
    ) -> bool {
        if self.confirmed {
            return true;
        }
        self.ask(title, detail, action, command);
        false
    }

    /// Asks before running `command`.
    fn ask(
        &mut self,
        title: impl Into<String>,
        detail: impl Into<String>,
        action: &str,
        command: Command,
    ) {
        self.confirm = Some(Confirm {
            title: title.into(),
            detail: detail.into(),
            action: action.to_string(),
            danger: true,
            command,
        });
    }

    fn save_config(&mut self) {
        self.config.repos = self.repos.iter().map(|r| r.entry.clone()).collect();
        if let Err(error) = self.config.save_to(&self.config_path) {
            self.show_error(format!("Could not save the config: {error}"));
        }
    }

    fn select(&mut self, index: usize) {
        if index < self.repos.len() {
            self.home = false;
            if index != self.selected {
                self.changes = Default::default();
            }
            // Today and All agents are not a repo's: show its last repo tab.
            if self.tab.is_global() {
                self.tab = self.repo_tab;
            }
            self.selected = index;
            self.list_cursor = None;
            self.board_cursor = None;
            self.log_key = None;
            self.log_paused = None;
            self.sync_group_field();
        }
    }

    fn sync_group_field(&mut self) {
        self.group_field.text = self
            .repos
            .get(self.selected)
            .and_then(|r| r.entry.group.clone())
            .unwrap_or_default();
    }

    /// Runs blocking work off the main thread, then reports and re-polls.
    /// Like `run_background`, then `after(ok)` on the main thread (to clear
    /// a field only once its action worked, or put text back on failure).
    fn run_background_then(
        &mut self,
        cx: &mut Context<Self>,
        done: &'static str,
        work: impl FnOnce() -> Result<(), String> + Send + 'static,
        after: impl FnOnce(&mut Self, bool) + 'static,
    ) {
        let task = cx.background_executor().spawn(async move { work() });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| {
                let ok = result.is_ok();
                match result {
                    Ok(()) if !done.is_empty() => this.show_info(done),
                    Ok(()) => {}
                    Err(error) => this.show_error(error),
                }
                after(this, ok);
                this.poll_now(false, cx);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn run_background(
        &mut self,
        cx: &mut Context<Self>,
        done: &'static str,
        work: impl FnOnce() -> Result<(), String> + Send + 'static,
    ) {
        let task = cx.background_executor().spawn(async move { work() });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| {
                match result {
                    Ok(()) if !done.is_empty() => this.show_info(done),
                    Ok(()) => {}
                    Err(error) => this.show_error(error),
                }
                this.poll_now(false, cx);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }
}

/// `/Users/me/x` → `~/x`.
fn display_path(path: &std::path::Path) -> String {
    let text = path.display().to_string();
    match std::env::var("HOME") {
        Ok(home) if !home.is_empty() && text.starts_with(&home) => {
            format!("~{}", &text[home.len()..])
        }
        _ => text,
    }
}

#[cfg(test)]
mod tests {
    /// Every action in `actions!` needs a listener (Quit's is in main.rs).
    /// A missing one fails silently: its key binding and menu item do nothing.
    #[test]
    fn every_action_has_a_listener() {
        // Given
        let source = concat!(include_str!("mod.rs"), include_str!("chrome.rs"));
        let main = include_str!("../main.rs");
        let start = source.find("actions!(").unwrap();
        let end = source[start..].find(");").unwrap() + start;
        let declared: Vec<&str> = source[start..end]
            .split(['[', ']', ',', '\n', ' '])
            .map(str::trim)
            .filter(|w| w.chars().next().is_some_and(char::is_uppercase))
            .collect();

        // When
        let missing: Vec<&&str> = declared
            .iter()
            .filter(|name| {
                let pattern = format!("_: &{name},");
                !source.contains(&pattern) && !main.contains(&pattern)
            })
            .collect();

        // Then
        assert!(declared.len() > 10, "parsed the actions list: {declared:?}");
        assert!(
            missing.is_empty(),
            "actions without a listener: {missing:?}"
        );
    }
}
