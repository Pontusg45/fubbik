//! The main window. `Workspace` owns all app state; the poll loop refreshes
//! repo snapshots on the background executor and pushes them back here.

mod actions;
mod agents;
mod chrome;
mod command_palette;
mod commands;
mod fields;
mod hotkeys;
mod insights;
pub mod keymap;
mod knowledge;
mod lists;
mod palette;
mod plans;
mod polling;
mod preferences;
mod setup_view;
mod sidebar;
mod tasks;
mod terminal_view;
mod terminals;
mod text;
pub mod today;
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

use palette::{Command, Entry};
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
}

impl Tab {
    pub const ALL: [Tab; 13] = [
        Tab::Today,
        Tab::Overview,
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
        Tab::Setup,
    ];

    /// The tab's own color: its dot and underline in the tab strip.
    pub fn color(self) -> gpui::Hsla {
        use theme::Hue;
        match self {
            Tab::Overview | Tab::Today => theme::hue(Hue::Blue),
            Tab::Processes => theme::hue(Hue::Green),
            Tab::Databases => theme::hue(Hue::Cyan),
            Tab::Logs | Tab::Terminal => theme::hue(Hue::Lime),
            Tab::Worktrees => theme::hue(Hue::Violet),
            Tab::Sessions | Tab::Agents => theme::hue(Hue::Orange),
            Tab::Plans | Tab::Insights => theme::hue(Hue::Pink),
            Tab::Knowledge => theme::hue(Hue::Yellow),
            Tab::Setup => theme::text_muted(),
        }
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
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Field {
    Palette,
    LogFilter,
    NoteTitle,
    NoteBody,
    Group,
    NewWorktree,
    Reply,
    FubbikUrl,
    WebUrl,
    FubbikBin,
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
    supervisor: Supervisor,
    client: FubbikClient,
    focus: FocusHandle,
    palette_field: TextField,
    log_filter: TextField,
    note_title: TextField,
    note_body: TextField,
    group_field: TextField,
    worktree_field: TextField,
    /// Transcript read positions, kept across polls.
    session_index: Arc<Mutex<SessionIndex>>,
    /// Re-read worktrees on the next poll (they are slower than the probes).
    refresh_worktrees: bool,
    palette_open: bool,
    palette_selected: usize,
    /// Chunk search results for a palette query starting with `?`.
    palette_search: Vec<ChunkRef>,
    /// Commands run from the palette, newest first; listed first when the
    /// query is empty.
    palette_recent: Vec<Command>,
    /// The built-in server's status at the last tick, to notice changes.
    backend_seen: crate::backend::Status,
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
    /// Processes the user stopped, so their exit is not reported as a crash.
    user_stopped: HashSet<String>,
    toast: Option<Toast>,
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
            supervisor,
            client,
            focus,
            palette_field: TextField::new(cx),
            log_filter: TextField::new(cx),
            note_title: TextField::new(cx),
            note_body: TextField::new(cx),
            group_field: TextField::new(cx),
            worktree_field: TextField::new(cx),
            session_index: Arc::new(Mutex::new(SessionIndex::new())),
            refresh_worktrees: true,
            palette_open: false,
            palette_selected: 0,
            palette_search: Vec::new(),
            palette_recent: Vec::new(),
            backend_seen: crate::backend::status(),
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
            cheat_sheet_open: false,
            sidebar_nav: false,
            needs_cursor: 0,
            board_cursor: None,
            quiet: HashMap::new(),
            api_backoff: Duration::ZERO,
            _watcher: None,
            _watch_task: Task::ready(()),
            knowledge_paused_until: None,
            ghostty_themes: crate::ghostty::list_themes(&crate::ghostty::theme_dirs()),
            reply_target: None,
            focus_terminal: false,
            log_key: None,
            log_paused: None,
            log_scroll: ScrollHandle::new(),
            user_stopped: HashSet::new(),
            toast: None,
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
        }
        // A developer hook for smoke runs: open on a tab by name.
        if !workspace.repos.is_empty() {
            workspace.tab = Tab::Today;
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

    fn show_error(&mut self, message: impl Into<SharedString>) {
        self.toast = Some(Toast {
            message: message.into(),
            error: true,
            at: Instant::now(),
            action: None,
        });
    }

    fn show_info(&mut self, message: impl Into<SharedString>) {
        self.toast = Some(Toast {
            message: message.into(),
            error: false,
            at: Instant::now(),
            action: None,
        });
    }

    /// A toast that does something when clicked.
    fn show_alert(&mut self, alert: Alert) {
        self.toast = Some(Toast {
            message: alert.message.into(),
            error: false,
            at: Instant::now(),
            action: Some(alert.target),
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
