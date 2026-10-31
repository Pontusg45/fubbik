//! The main window. `Workspace` owns all app state; the poll loop refreshes
//! repo snapshots on the background executor and pushes them back here.

mod actions;
mod agents;
mod knowledge;
mod palette;
mod plans;
mod setup_view;
mod sidebar;
mod terminal_view;
mod text;
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
        ToggleTheme,
        ChooseTheme,
        RegisterRepo,
        Refresh,
    ]
);

const TICK: Duration = Duration::from_secs(1);
/// Probe every third tick; logs refresh every tick.
const PROBE_EVERY: u64 = 3;
/// fubbik knowledge every 30 ticks.
const KNOWLEDGE_EVERY: u64 = 30;
/// The macOS titlebar the content runs under.
const TITLEBAR_HEIGHT: f32 = 28.0;
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
}

impl Tab {
    pub const ALL: [Tab; 10] = [
        Tab::Overview,
        Tab::Processes,
        Tab::Databases,
        Tab::Logs,
        Tab::Worktrees,
        Tab::Sessions,
        Tab::Plans,
        Tab::Knowledge,
        Tab::Setup,
        Tab::Terminal,
    ];

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
            global: None,
            panel: None,
            expanded_worktree: None,
            applied_translucent: None,
            terminals: Vec::new(),
            next_terminal_id: 0,
            active_terminal: None,
            tmux_terminals: false,
            reply_field: TextField::new(cx),
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
        workspace.tmux_terminals = workspace.config.tmux_persistence() && tmux::available();
        if workspace.tmux_terminals {
            workspace.restore_terminals(cx);
        }
        // A developer hook for smoke runs: open on a tab by name.
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

    // ── polling ────────────────────────────────────────────────────────

    fn on_tick(&mut self, cx: &mut Context<Self>) {
        if self.tick.is_multiple_of(PROBE_EVERY) {
            self.poll_now(self.tick.is_multiple_of(KNOWLEDGE_EVERY), cx);
        }
        self.tick += 1;
        if self
            .toast
            .as_ref()
            .is_some_and(|t| t.at.elapsed() > TOAST_FOR)
        {
            self.toast = None;
        }
        if self.tab == Tab::Logs && self.log_paused.is_none() {
            self.log_scroll.scroll_to_bottom();
        }
        cx.notify();
    }

    /// Probes every repo on the background executor. A call while a poll is
    /// running queues one more poll right after it.
    fn poll_now(&mut self, with_knowledge: bool, cx: &mut Context<Self>) {
        if self.polling {
            self.poll_again = true;
            return;
        }
        self.polling = true;
        let jobs: Vec<(RepoEntry, Option<RepoSnapshot>)> = self
            .repos
            .iter()
            .map(|r| (r.entry.clone(), r.snapshot.clone()))
            .collect();
        let supervisor = self.supervisor.clone();
        let client = self.client.clone();
        let index = self.session_index.clone();
        let refresh_worktrees = with_knowledge || std::mem::take(&mut self.refresh_worktrees);
        let work = cx.background_executor().spawn(async move {
            jobs.into_iter()
                .map(|(entry, prev)| {
                    let mut snapshot = poll::snapshot(&entry, &supervisor, prev.as_ref());
                    poll::agents(
                        &entry,
                        &mut snapshot,
                        prev.as_ref(),
                        &index,
                        refresh_worktrees,
                    );
                    let knowledge = (with_knowledge && !snapshot.missing)
                        .then(|| fetch_knowledge(&client, &entry, snapshot.remote.as_deref()));
                    let setup = (with_knowledge && !snapshot.missing).then(|| {
                        let space = knowledge
                            .as_ref()
                            .and_then(|k| k.as_ref().ok())
                            .and_then(|k| k.space.as_ref())
                            .map(|s| s.id.clone());
                        read_setup(&client, &entry, &snapshot, space.as_deref())
                    });
                    PollResult {
                        path: entry.path,
                        snapshot,
                        knowledge,
                        setup,
                    }
                })
                .collect::<Vec<_>>()
        });
        let global = with_knowledge.then(|| {
            let client = self.client.clone();
            let configured = self.config.fubbik_bin.clone();
            let repos: Vec<PathBuf> = self.repos.iter().map(|r| r.entry.path.clone()).collect();
            cx.background_executor().spawn(async move {
                GlobalStatus {
                    api: client
                        .health()
                        .and_then(|health| Ok((health, client.capabilities()?))),
                    fubbik_bin: setup::resolve_fubbik_bin(configured.as_deref(), &repos),
                }
            })
        });
        cx.spawn(async move |this, cx| {
            let results = work.await;
            let global = match global {
                Some(task) => Some(task.await),
                None => None,
            };
            this.update(cx, |this, cx| {
                if global.is_some() {
                    this.global = global;
                }
                this.apply(results, cx);
                this.polling = false;
                if std::mem::take(&mut this.poll_again) {
                    this.poll_now(false, cx);
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn apply(&mut self, results: Vec<PollResult>, cx: &mut Context<Self>) {
        for result in results {
            let Some(repo) = self.repos.iter_mut().find(|r| r.entry.path == result.path) else {
                continue; // removed while polling
            };
            if let Some(prev) = &repo.snapshot {
                notify_changes(&repo.entry, prev, &result.snapshot, &mut self.user_stopped);
            }
            repo.snapshot = Some(result.snapshot);
            if let Some(knowledge) = result.knowledge {
                repo.knowledge = Some(knowledge);
            }
            if let Some(setup) = result.setup {
                repo.setup = Some(setup);
            }
        }
        self.report_test_exits(cx);
    }

    // ── state helpers ──────────────────────────────────────────────────

    fn show_error(&mut self, message: impl Into<SharedString>) {
        self.toast = Some(Toast {
            message: message.into(),
            error: true,
            at: Instant::now(),
        });
    }

    fn show_info(&mut self, message: impl Into<SharedString>) {
        self.toast = Some(Toast {
            message: message.into(),
            error: false,
            at: Instant::now(),
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

    // ── commands ───────────────────────────────────────────────────────

    fn start_process(&mut self, repo: usize, index: usize, cx: &mut Context<Self>) {
        let Some(view) = self.repos.get(repo) else {
            return;
        };
        let Some(state) = view
            .snapshot
            .as_ref()
            .and_then(|s| s.processes.get(index))
            .cloned()
        else {
            return;
        };
        if state.owned {
            return;
        }
        if state.status.is_up() {
            let holder = state
                .pid
                .map(|pid| format!(" by pid {pid}"))
                .unwrap_or_default();
            let port = state
                .spec
                .port
                .map(|p| format!("port {p}"))
                .unwrap_or("its port".into());
            self.show_error(format!(
                "{} is already up: {port} is held{holder}",
                state.spec.name
            ));
            return;
        }
        self.user_stopped.remove(&state.key);
        self.log_key = Some(state.key.clone());
        let supervisor = self.supervisor.clone();
        let cwd = state.spec.cwd(&view.entry.path);
        self.run_background(cx, "", move || {
            supervisor
                .start(&state.key, &state.spec.command, &cwd)
                .map(|_| ())
        });
    }

    fn stop_process(&mut self, repo: usize, index: usize, cx: &mut Context<Self>) {
        let Some(state) = self
            .repos
            .get(repo)
            .and_then(|v| v.snapshot.as_ref())
            .and_then(|s| s.processes.get(index))
            .cloned()
        else {
            return;
        };
        if !state.owned {
            if state.pid.is_some() {
                self.show_error(format!(
                    "{} was not started here; use \"Kill\" to stop the external process",
                    state.spec.name
                ));
            }
            return;
        }
        self.user_stopped.insert(state.key.clone());
        let supervisor = self.supervisor.clone();
        self.run_background(cx, "", move || {
            supervisor.stop(&state.key);
            Ok(())
        });
    }

    fn restart_process(&mut self, repo: usize, index: usize, cx: &mut Context<Self>) {
        let Some(view) = self.repos.get(repo) else {
            return;
        };
        let Some(state) = view
            .snapshot
            .as_ref()
            .and_then(|s| s.processes.get(index))
            .cloned()
        else {
            return;
        };
        if !state.owned {
            self.start_process(repo, index, cx);
            return;
        }
        let supervisor = self.supervisor.clone();
        let cwd = state.spec.cwd(&view.entry.path);
        self.user_stopped.insert(state.key.clone());
        self.run_background(cx, "", move || {
            supervisor
                .restart(&state.key, &state.spec.command, &cwd)
                .map(|_| ())
        });
    }

    fn kill_external(&mut self, pid: u32, cx: &mut Context<Self>) {
        self.run_background(cx, "Sent SIGTERM", move || supervisor::kill_external(pid));
    }

    fn start_all(&mut self, repo: usize, cx: &mut Context<Self>) {
        let count = self
            .repos
            .get(repo)
            .and_then(|v| v.snapshot.as_ref())
            .map_or(0, |s| s.processes.len());
        for index in 0..count {
            let up = self.repos[repo]
                .snapshot
                .as_ref()
                .is_some_and(|s| s.processes[index].status.is_up());
            if !up {
                self.start_process(repo, index, cx);
            }
        }
    }

    fn stop_all(&mut self, repo: usize, cx: &mut Context<Self>) {
        let count = self
            .repos
            .get(repo)
            .and_then(|v| v.snapshot.as_ref())
            .map_or(0, |s| s.processes.len());
        for index in 0..count {
            self.stop_process(repo, index, cx);
        }
    }

    fn repos_in_group(&self, group: Option<&str>) -> Vec<usize> {
        (0..self.repos.len())
            .filter(|&i| self.repos[i].entry.group.as_deref() == group)
            .collect()
    }

    fn database_task(
        &mut self,
        repo: usize,
        index: usize,
        action: &'static str,
        cx: &mut Context<Self>,
    ) {
        let Some(view) = self.repos.get(repo) else {
            return;
        };
        let Some(db) = view
            .snapshot
            .as_ref()
            .and_then(|s| s.databases.get(index))
            .cloned()
        else {
            return;
        };
        let spec = &db.spec;
        let command = match action {
            "start" => spec.start_command(),
            "stop" => spec.stop_command(),
            "migrate" => spec.migrate.clone(),
            "seed" => spec.seed.clone(),
            "studio" => spec.studio.clone(),
            _ => None,
        };
        let Some(command) = command else {
            self.show_error(format!(
                "{} has no {action} command in {}",
                spec.name,
                crate::services::FILE_NAME
            ));
            return;
        };
        let key = process_key(&view.entry.path, &format!("{}:{action}", spec.name));
        self.log_key = Some(key.clone());
        self.user_stopped.insert(key.clone());
        let supervisor = self.supervisor.clone();
        let cwd = view.entry.path.clone();
        self.run_background(cx, "", move || {
            supervisor.stop(&key);
            supervisor.start(&key, &command, &cwd).map(|_| ())
        });
    }

    fn copy(&mut self, text: String, cx: &mut Context<Self>) {
        cx.write_to_clipboard(ClipboardItem::new_string(text));
        self.show_info("Copied");
    }

    fn register_repo(&mut self, cx: &mut Context<Self>) {
        let paths = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: true,
            prompt: Some("Register".into()),
        });
        cx.spawn(async move |this, cx| {
            let Ok(Ok(Some(paths))) = paths.await else {
                return;
            };
            this.update(cx, |this, cx| {
                let mut added = 0;
                for path in paths {
                    let root = git::toplevel(&path).unwrap_or(path);
                    if this.config.add_repo(root.clone()) {
                        this.repos.push(RepoView::new(RepoEntry {
                            path: root,
                            group: None,
                        }));
                        added += 1;
                    }
                }
                if added == 0 {
                    this.show_info("Already registered");
                    return;
                }
                this.save_config();
                this.select(this.repos.len() - 1);
                this.poll_now(true, cx);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn remove_repo(&mut self, repo: usize, cx: &mut Context<Self>) {
        if repo >= self.repos.len() {
            return;
        }
        self.stop_all(repo, cx);
        let removed = self.repos.remove(repo);
        self.config.remove_repo(&removed.entry.path);
        self.save_config();
        self.selected = self.selected.min(self.repos.len().saturating_sub(1));
        self.sync_group_field();
        self.show_info(format!(
            "Removed {} (its files are untouched)",
            removed.entry.name()
        ));
    }

    fn generate_services(&mut self, repo: usize, cx: &mut Context<Self>) {
        let Some(view) = self.repos.get(repo) else {
            return;
        };
        let path = view.entry.path.clone();
        if crate::services::path(&path).exists() {
            self.show_error(format!("{} already exists", crate::services::FILE_NAME));
            return;
        }
        self.run_background(
            cx,
            "Wrote fubbik.services.toml — edit it to add ports and health URLs",
            move || {
                let detected: ServicesFile = detect::detect(&path);
                detected.save(&path)
            },
        );
    }

    fn open_github(&mut self, repo: usize, cx: &mut Context<Self>) {
        let url = self
            .repos
            .get(repo)
            .and_then(|v| v.snapshot.as_ref())
            .and_then(|s| s.remote.as_deref())
            .and_then(git::web_url);
        match url {
            Some(url) => cx.open_url(&url),
            None => self.show_error("No origin remote to open"),
        }
    }

    fn create_worktree(&mut self, repo: usize, cx: &mut Context<Self>) {
        let Some(view) = self.repos.get(repo) else {
            return;
        };
        let branch = self.worktree_field.text.trim().to_string();
        if branch.is_empty() {
            self.show_error("Type a branch name for the new worktree");
            return;
        }
        self.worktree_field.text.clear();
        self.refresh_worktrees = true;
        let path = view.entry.path.clone();
        self.run_background(cx, "Worktree created", move || {
            worktree::create(&path, &branch).map(|_| ())
        });
    }

    fn remove_worktree(&mut self, repo: usize, path: PathBuf, cx: &mut Context<Self>) {
        let Some(view) = self.repos.get(repo) else {
            return;
        };
        let live = view
            .snapshot
            .iter()
            .flat_map(|s| s.sessions.iter())
            .any(|s: &AiSession| s.state.is_live() && s.cwd.starts_with(&path));
        if live {
            self.show_error("An AI session is still running in that worktree");
            return;
        }
        self.refresh_worktrees = true;
        let repo_path = view.entry.path.clone();
        self.run_background(cx, "Worktree removed (its branch is kept)", move || {
            worktree::remove(&repo_path, &path)
        });
    }

    fn run(&mut self, command: Command, cx: &mut Context<Self>) {
        match command {
            Command::SelectRepo(i) => self.select(i),
            Command::StartProcess(r, p) => self.start_process(r, p, cx),
            Command::StopProcess(r, p) => self.stop_process(r, p, cx),
            Command::RestartProcess(r, p) => self.restart_process(r, p, cx),
            Command::StartAll(r) => self.start_all(r, cx),
            Command::StopAll(r) => self.stop_all(r, cx),
            Command::ShowTab(tab) => {
                self.tab = tab;
                self.focus_terminal = tab == Tab::Terminal;
            }
            Command::RegisterRepo => self.register_repo(cx),
            Command::RemoveRepo(r) => self.remove_repo(r, cx),
            Command::GenerateServices(r) => self.generate_services(r, cx),
            Command::OpenEditor(r) => {
                if let Some(view) = self.repos.get(r) {
                    let path = view.entry.path.clone();
                    self.run_background(cx, "", move || launch::open_in_editor(&path));
                }
            }
            Command::OpenTerminal(r) => {
                if let Some(view) = self.repos.get(r) {
                    let path = view.entry.path.clone();
                    self.run_background(cx, "", move || launch::open_in_terminal(&path));
                }
            }
            Command::OpenGitHub(r) => self.open_github(r, cx),
            Command::Refresh => {
                // Picks up edits to the Ghostty config when following it.
                if self.config.ghostty_theme.as_deref() == Some("follow") {
                    let _ = theme::apply(Some("follow"));
                }
                self.poll_now(true, cx);
            }
            Command::SetColors(choice) => match theme::apply(choice.as_deref()) {
                Ok(label) => {
                    self.config.ghostty_theme = choice;
                    self.save_config();
                    self.show_info(format!("Colors: {label}"));
                }
                Err(error) => self.show_error(error),
            },
            Command::ToggleTheme => {
                let on = !theme::is_translucent();
                theme::set_translucent(on);
                self.config.theme = Some(if on { "translucent" } else { "opaque" }.into());
                self.save_config();
            }
            Command::OpenEditorAt(path) => {
                self.run_background(cx, "", move || launch::open_in_editor(&path))
            }
            Command::OpenTerminalAt(path) => {
                self.run_background(cx, "", move || launch::open_in_terminal(&path))
            }
            Command::NewSession(path) => {
                self.launch_in_terminal(path, "Claude".into(), "claude".into(), cx)
            }
            Command::ResumeSession(agent, id, cwd) => {
                let label = format!("{} {}", agent.label(), &id[..id.len().min(8)]);
                self.launch_in_terminal(cwd, label, agent.resume_command(&id), cx);
            }
            Command::CreateWorktree(r) => self.create_worktree(r, cx),
            Command::OpenWeb(path) => cx.open_url(&format!("{}{path}", self.config.web_url())),
            Command::ShellAt(path) => {
                match self
                    .terminals
                    .iter()
                    .find(|t| t.dir == path && t.label == "Shell")
                {
                    Some(existing) => self.active_terminal = Some(existing.id),
                    None => {
                        self.open_terminal(path, "Shell".into(), None, cx);
                    }
                }
                self.tab = Tab::Terminal;
                self.focus_terminal = true;
            }
            Command::CloseTerminal(id) => {
                // Closing a tab ends its session; quitting the app does not.
                if let Some(name) = self
                    .terminals
                    .iter()
                    .find(|t| t.id == id)
                    .and_then(|t| t.tmux.clone())
                {
                    let _ = tmux::kill(&tmux::Server::fubbik(), &name);
                }
                self.terminals.retain(|t| t.id != id);
            }
            Command::StartTask {
                repo,
                plan,
                task,
                new_worktree,
                background,
            } => self.start_task(repo, &plan, &task, new_worktree, background, cx),
            Command::RemoveWorktree(r, path) => self.remove_worktree(r, path, cx),
            Command::PruneWorktrees(r) => {
                if let Some(view) = self.repos.get(r) {
                    let repo = view.entry.path.clone();
                    self.refresh_worktrees = true;
                    self.run_background(cx, "Pruned stale worktrees", move || {
                        worktree::prune(&repo)
                    });
                }
            }
        }
        cx.notify();
    }

    /// `?query` searches chunks; anything else filters the commands.
    fn palette_matches(&self) -> Vec<Entry> {
        if self.palette_field.text.starts_with('?') {
            return self
                .palette_search
                .iter()
                .map(|chunk| Entry {
                    label: format!("Open chunk: {} ({})", chunk.title, chunk.kind),
                    command: Command::OpenWeb(format!("/chunks/{}", chunk.id)),
                })
                .collect();
        }
        palette::filter(&self.palette_entries(), &self.palette_field.text)
    }

    fn palette_entries(&self) -> Vec<Entry> {
        let mut entries = vec![
            Entry {
                label: "Register repo…".into(),
                command: Command::RegisterRepo,
            },
            Entry {
                label: "Refresh".into(),
                command: Command::Refresh,
            },
            Entry {
                label: "Theme: Built-in".into(),
                command: Command::SetColors(None),
            },
            Entry {
                label: "Theme: Follow Ghostty config".into(),
                command: Command::SetColors(Some("follow".into())),
            },
            Entry {
                label: if theme::is_translucent() {
                    "Theme: switch to opaque".into()
                } else {
                    "Theme: switch to translucent".into()
                },
                command: Command::ToggleTheme,
            },
            Entry {
                label: "Open fubbik dashboard".into(),
                command: Command::OpenWeb("/dashboard".into()),
            },
            Entry {
                label: "Open knowledge health".into(),
                command: Command::OpenWeb("/knowledge-health".into()),
            },
            Entry {
                label: "Open knowledge graph".into(),
                command: Command::OpenWeb("/graph".into()),
            },
        ];
        entries.extend(Tab::ALL.map(|tab| Entry {
            label: format!("Show {}", tab.label()),
            command: Command::ShowTab(tab),
        }));
        entries.extend(self.ghostty_themes.iter().map(|name| Entry {
            label: format!("Theme: {name} (Ghostty)"),
            command: Command::SetColors(Some(name.clone())),
        }));
        for (r, view) in self.repos.iter().enumerate() {
            let name = view.entry.name();
            entries.push(Entry {
                label: format!("Go to {name}"),
                command: Command::SelectRepo(r),
            });
            entries.push(Entry {
                label: format!("Start all — {name}"),
                command: Command::StartAll(r),
            });
            entries.push(Entry {
                label: format!("Stop all — {name}"),
                command: Command::StopAll(r),
            });
            entries.push(Entry {
                label: format!("Open in editor — {name}"),
                command: Command::OpenEditor(r),
            });
            entries.push(Entry {
                label: format!("Open in terminal — {name}"),
                command: Command::OpenTerminal(r),
            });
            entries.push(Entry {
                label: format!("Open on GitHub — {name}"),
                command: Command::OpenGitHub(r),
            });
            entries.push(Entry {
                label: format!("Remove repo — {name}"),
                command: Command::RemoveRepo(r),
            });
            entries.push(Entry {
                label: format!("Shell (embedded terminal) — {name}"),
                command: Command::ShellAt(view.entry.path.clone()),
            });
            entries.push(Entry {
                label: format!("New Claude session — {name}"),
                command: Command::NewSession(view.entry.path.clone()),
            });
            let snapshot = view.snapshot.as_ref();
            for worktree in snapshot
                .and_then(|s| s.worktrees.as_ref()?.as_ref().ok())
                .into_iter()
                .flatten()
            {
                if worktree.main || worktree.missing {
                    continue;
                }
                let branch = worktree.name();
                entries.push(Entry {
                    label: format!("Open worktree {branch} in editor — {name}"),
                    command: Command::OpenEditorAt(worktree.path.clone()),
                });
                entries.push(Entry {
                    label: format!("New Claude session in {branch} — {name}"),
                    command: Command::NewSession(worktree.path.clone()),
                });
            }
            if let Some(Ok(knowledge)) = &view.knowledge {
                if let Some(space) = &knowledge.space {
                    entries.push(Entry {
                        label: format!("Open space in fubbik — {name}"),
                        command: Command::OpenWeb(format!("/spaces/{}", space.id)),
                    });
                }
                for plan in &knowledge.plan_details {
                    for task in plan.tasks.iter().filter(|t| t.status == "pending") {
                        entries.push(Entry {
                            label: format!(
                                "Start task “{}” in a new worktree — {}",
                                task.title, plan.title
                            ),
                            command: Command::StartTask {
                                repo: r,
                                plan: plan.id.clone(),
                                task: task.id.clone(),
                                new_worktree: true,
                                background: false,
                            },
                        });
                    }
                }
            }
            for session in snapshot.iter().flat_map(|s| s.sessions.iter()).take(10) {
                entries.push(Entry {
                    label: format!(
                        "Resume “{}” ({}) — {name}",
                        session.title,
                        session.agent.label()
                    ),
                    command: Command::ResumeSession(
                        session.agent,
                        session.id.clone(),
                        session.cwd.clone(),
                    ),
                });
            }
            for (p, process) in view
                .snapshot
                .iter()
                .flat_map(|s| s.processes.iter())
                .enumerate()
            {
                let process = &process.spec.name;
                entries.push(Entry {
                    label: format!("Start {process} — {name}"),
                    command: Command::StartProcess(r, p),
                });
                entries.push(Entry {
                    label: format!("Stop {process} — {name}"),
                    command: Command::StopProcess(r, p),
                });
                entries.push(Entry {
                    label: format!("Restart {process} — {name}"),
                    command: Command::RestartProcess(r, p),
                });
            }
        }
        entries
    }

    fn toggle_palette(&mut self, window: &mut Window) {
        self.palette_open = !self.palette_open;
        self.palette_field.text.clear();
        self.palette_selected = 0;
        if self.palette_open {
            self.palette_field.focus.focus(window);
        } else {
            self.focus.focus(window);
        }
    }

    fn submit_note(&mut self, cx: &mut Context<Self>) {
        let title = self.note_title.text.trim().to_string();
        if title.is_empty() {
            self.show_error("A note needs a title");
            return;
        }
        let Some(space_id) = self
            .repos
            .get(self.selected)
            .and_then(|v| v.knowledge.as_ref())
            .and_then(|k| k.as_ref().ok())
            .and_then(|k| k.space.as_ref())
            .map(|s| s.id.clone())
        else {
            self.show_error("This repo has no fubbik space");
            return;
        };
        let content = self.note_body.text.clone();
        self.note_title.text.clear();
        self.note_body.text.clear();
        let client = self.client.clone();
        let task = cx
            .background_executor()
            .spawn(async move { client.quick_note(&space_id, &title, &content) });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| {
                match result {
                    Ok(()) => this.show_info("Note saved to fubbik"),
                    Err(error) => this.show_error(format!("Could not save the note: {error}")),
                }
                this.poll_now(true, cx);
            })
            .ok();
        })
        .detach();
    }

    fn submit_group(&mut self) {
        let group = self.group_field.text.trim().to_string();
        let Some(view) = self.repos.get_mut(self.selected) else {
            return;
        };
        view.entry.group = (!group.is_empty()).then_some(group);
        self.save_config();
        self.show_info("Group saved");
    }

    fn field_mut(&mut self, field: Field) -> &mut TextField {
        match field {
            Field::Palette => &mut self.palette_field,
            Field::LogFilter => &mut self.log_filter,
            Field::NoteTitle => &mut self.note_title,
            Field::NoteBody => &mut self.note_body,
            Field::Group => &mut self.group_field,
            Field::NewWorktree => &mut self.worktree_field,
            Field::Reply => &mut self.reply_field,
        }
    }

    fn on_field_key(
        &mut self,
        field: Field,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let keystroke = &event.keystroke;
        if keystroke.modifiers.platform && keystroke.key == "v" {
            if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
                self.field_mut(field)
                    .text
                    .push_str(text.lines().next().unwrap_or_default());
                cx.stop_propagation();
                cx.notify();
            }
            return;
        }
        let outcome = apply_key(
            &mut self.field_mut(field).text,
            &keystroke.key,
            keystroke.key_char.as_deref(),
            &keystroke.modifiers,
        );
        if outcome == KeyOutcome::Ignored {
            return;
        }
        cx.stop_propagation();
        match (field, outcome) {
            (Field::Palette, KeyOutcome::Edited) => {
                self.palette_selected = 0;
                self.search_chunks(cx);
            }
            (Field::Palette, KeyOutcome::Up) => {
                self.palette_selected = self.palette_selected.saturating_sub(1)
            }
            (Field::Palette, KeyOutcome::Down) => self.palette_selected += 1,
            (Field::Palette, KeyOutcome::Cancel) => self.toggle_palette(window),
            (Field::Palette, KeyOutcome::Submit) => {
                let matches = self.palette_matches();
                let chosen = matches
                    .get(self.palette_selected.min(matches.len().saturating_sub(1)))
                    .cloned();
                self.toggle_palette(window);
                if let Some(entry) = chosen {
                    self.run(entry.command, cx);
                }
            }
            (Field::NoteTitle, KeyOutcome::Submit) => self.note_body.focus.focus(window),
            (Field::NoteBody, KeyOutcome::Submit) => self.submit_note(cx),
            (Field::Reply, KeyOutcome::Submit) => self.send_reply(cx),
            (Field::NewWorktree, KeyOutcome::Submit) => self.create_worktree(self.selected, cx),
            (Field::Group, KeyOutcome::Submit) => {
                self.submit_group();
                self.focus.focus(window);
            }
            (_, KeyOutcome::Cancel) => self.focus.focus(window),
            _ => {}
        }
        cx.notify();
    }

    /// Wraps a field's look with focus tracking and key handling.
    fn field(
        &self,
        field: Field,
        placeholder: &str,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let text_field = match field {
            Field::Palette => &self.palette_field,
            Field::LogFilter => &self.log_filter,
            Field::NoteTitle => &self.note_title,
            Field::NoteBody => &self.note_body,
            Field::Group => &self.group_field,
            Field::NewWorktree => &self.worktree_field,
            Field::Reply => &self.reply_field,
        };
        let focus = text_field.focus.clone();
        widgets::text_box(&text_field.text, placeholder, focus.is_focused(window))
            .track_focus(&focus)
            .on_mouse_down(gpui::MouseButton::Left, move |_, window, _| {
                focus.focus(window)
            })
            .on_key_down(cx.listener(move |this, event, window, cx| {
                this.on_field_key(field, event, window, cx)
            }))
    }

    fn render_palette(&self, window: &Window, cx: &mut Context<Self>) -> impl IntoElement {
        let matches = self.palette_matches();
        let selected = self.palette_selected.min(matches.len().saturating_sub(1));
        let rows = matches
            .into_iter()
            .take(12)
            .enumerate()
            .map(|(i, entry)| {
                let command = entry.command.clone();
                div()
                    .id(("palette", i))
                    .px_3()
                    .py_1p5()
                    .rounded_md()
                    .text_sm()
                    .cursor_pointer()
                    .when(i == selected, |row| row.bg(theme::surface_hover()))
                    .hover(|row| row.bg(theme::surface_hover()))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.toggle_palette(window);
                        this.run(command.clone(), cx);
                    }))
                    .child(entry.label)
            })
            .collect::<Vec<_>>();
        div()
            .absolute()
            .inset_0()
            .flex()
            .justify_center()
            .pt(px(90.))
            .bg(theme::overlay())
            .child(
                div()
                    .id("palette")
                    .w(px(520.))
                    .h(px(420.))
                    .flex()
                    .flex_col()
                    .gap_2()
                    .p_3()
                    .rounded_lg()
                    .border_1()
                    .border_color(theme::border())
                    .bg(theme::elevated())
                    .shadow(theme::frame_shadow())
                    .on_mouse_down_out(cx.listener(|this, _, window, cx| {
                        this.toggle_palette(window);
                        cx.notify();
                    }))
                    .child(self.field(Field::Palette, "Type a command…", window, cx))
                    .child(div().flex().flex_col().children(rows)),
            )
    }

    /// The strip under the transparent system titlebar: traffic lights on
    /// Opens a terminal tab: a login shell in `dir`, typing `command` when
    /// given. Returns its id.
    fn open_terminal(
        &mut self,
        dir: PathBuf,
        label: String,
        command: Option<String>,
        cx: &mut Context<Self>,
    ) -> usize {
        self.open_terminal_with(dir, label, command, self.tmux_terminals, true, cx)
    }

    /// Opens a terminal tab. With `persist`, it is a tmux session on the
    /// app's server (it survives quitting the app); otherwise a plain shell.
    /// `activate` makes it the tab shown.
    fn open_terminal_with(
        &mut self,
        dir: PathBuf,
        label: String,
        command: Option<String>,
        persist: bool,
        activate: bool,
        cx: &mut Context<Self>,
    ) -> usize {
        let id = self.next_terminal_id;
        self.next_terminal_id += 1;
        let server = tmux::Server::fubbik();
        let session = persist
            .then(|| {
                let mut n = id;
                while tmux::exists(&server, &tmux::session_name(&dir, n)) {
                    n += 1;
                }
                let name = tmux::session_name(&dir, n);
                match tmux::create(&server, &name, &dir, &label, command.as_deref()) {
                    Ok(()) => Some(name),
                    Err(error) => {
                        self.show_error(format!("tmux failed, using a plain shell: {error}"));
                        None
                    }
                }
            })
            .flatten();
        let view = match &session {
            Some(name) => {
                let (dir, name) = (dir.clone(), name.clone());
                cx.new(|cx| terminal_view::TerminalView::tmux(dir, name, cx))
            }
            None => cx.new(|cx| terminal_view::TerminalView::new(dir.clone(), command, cx)),
        };
        self.terminals.push(TermTab {
            id,
            dir,
            label,
            view,
            tmux: session,
        });
        if activate {
            self.active_terminal = Some(id);
        }
        id
    }

    /// Reattaches the app's tmux sessions left from the last run as tabs.
    fn restore_terminals(&mut self, cx: &mut Context<Self>) {
        for session in tmux::sessions(&tmux::Server::fubbik()) {
            let id = self.next_terminal_id;
            self.next_terminal_id += 1;
            let (dir, name) = (session.dir.clone(), session.name.clone());
            let view = cx.new(|cx| terminal_view::TerminalView::tmux(dir, name, cx));
            self.terminals.push(TermTab {
                id,
                dir: session.dir,
                label: session.label,
                view,
                tmux: Some(session.name),
            });
        }
    }

    /// Runs a command for the user, in the Terminal tab or, with
    /// `terminal = "external"`, in Terminal.app.
    fn launch_in_terminal(
        &mut self,
        dir: PathBuf,
        label: String,
        command: String,
        cx: &mut Context<Self>,
    ) {
        self.launch(dir, label, command, false, cx);
    }

    /// `background`: open the tab without switching to it (with tmux, the
    /// session runs detached until you look).
    fn launch(
        &mut self,
        dir: PathBuf,
        label: String,
        command: String,
        background: bool,
        cx: &mut Context<Self>,
    ) {
        if self.config.external_terminal() {
            self.run_background(cx, "", move || launch::run_in_terminal(&dir, &command));
            return;
        }
        // Show it in the repo it belongs to.
        if let Some(repo) = self.repos.iter().position(|r| {
            dir.starts_with(&r.entry.path)
                || r.snapshot
                    .as_ref()
                    .is_some_and(|s| s.worktree_paths().contains(&dir))
        }) {
            self.select(repo);
        }
        if background {
            self.open_terminal_with(dir, label, Some(command), self.tmux_terminals, false, cx);
            return;
        }
        self.open_terminal(dir, label, Some(command), cx);
        self.tab = Tab::Terminal;
        self.focus_terminal = true;
    }

    /// Terminal tabs whose directory is the selected repo or one of its worktrees.
    fn terminals_for_selected(&self) -> Vec<&TermTab> {
        let Some(repo) = self.repos.get(self.selected) else {
            return Vec::new();
        };
        let worktrees = repo
            .snapshot
            .as_ref()
            .map(|s| s.worktree_paths())
            .unwrap_or_default();
        self.terminals
            .iter()
            .filter(|t| t.dir.starts_with(&repo.entry.path) || worktrees.contains(&t.dir))
            .collect()
    }

    /// The terminal the Terminal tab shows, opening a shell in the repo when
    /// it has none.
    fn terminal_for_selected(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<gpui::Entity<terminal_view::TerminalView>> {
        let repo_path = self.repos.get(self.selected)?.entry.path.clone();
        let ids: Vec<usize> = self.terminals_for_selected().iter().map(|t| t.id).collect();
        let id = match self
            .active_terminal
            .filter(|id| ids.contains(id))
            .or(ids.last().copied())
        {
            Some(id) => id,
            None => self.open_terminal(repo_path, "Shell".into(), None, cx),
        };
        self.active_terminal = Some(id);
        let view = self.terminals.iter().find(|t| t.id == id)?.view.clone();
        if std::mem::take(&mut self.focus_terminal) {
            view.read(cx).focus.focus(window);
        }
        Some(view)
    }

    /// The strip under the transparent system titlebar: traffic lights on
    /// the left, the title in the middle. Nothing in it is clickable, so the
    /// system can drag the window from it.
    fn render_titlebar(&self) -> impl IntoElement {
        let title = match self.repos.get(self.selected) {
            Some(repo) => format!("fubbik — {}", repo.entry.name()),
            None => "fubbik".to_string(),
        };
        div()
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .h(px(TITLEBAR_HEIGHT))
            .border_b_1()
            .border_color(theme::border())
            .bg(theme::sidebar())
            .text_xs()
            .text_color(theme::text_muted())
            .child(title)
    }

    fn render_toast(&self) -> Option<impl IntoElement> {
        let toast = self.toast.as_ref()?;
        Some(
            div()
                .absolute()
                .bottom_4()
                .right_4()
                .max_w(px(460.))
                .px_4()
                .py_2()
                .rounded_lg()
                .border_1()
                .border_color(if toast.error {
                    theme::danger()
                } else {
                    theme::border()
                })
                .bg(theme::elevated())
                .shadow(theme::frame_shadow())
                .text_sm()
                .text_color(if toast.error {
                    theme::danger()
                } else {
                    theme::text()
                })
                .child(toast.message.clone()),
        )
    }
}

/// MCP and hook state, and each worktree's CLAUDE.md against the generated one.
fn read_setup(
    client: &FubbikClient,
    entry: &RepoEntry,
    snapshot: &RepoSnapshot,
    space_id: Option<&str>,
) -> RepoSetup {
    let mut roots = snapshot.worktree_paths();
    if roots.is_empty() {
        roots.push(entry.path.clone());
    }
    let claude_md = claude_md::read_config(&entry.path).map(|config| {
        let generated = client.claude_md(&config.tag, config.max_tokens, space_id);
        let worktrees = match &generated {
            Ok(text) => roots
                .iter()
                .map(|root| (root.clone(), claude_md::state(root, &config, text)))
                .collect(),
            Err(_) => Vec::new(),
        };
        ClaudeMdSync {
            config,
            generated,
            worktrees,
        }
    });
    RepoSetup {
        mcp: setup::mcp_status(&entry.path, &setup::claude_json()),
        hooks: roots
            .iter()
            .map(|root| (root.clone(), setup::hook_status(root)))
            .collect(),
        claude_md,
    }
}

fn fetch_knowledge(
    client: &FubbikClient,
    entry: &RepoEntry,
    remote: Option<&str>,
) -> Result<Knowledge, String> {
    match client.detect_space(remote, &entry.path)? {
        Some(space) => client.knowledge(space),
        None => Ok(Knowledge::default()),
    }
}

/// A notification when a process goes down on its own, or starts failing
/// its health check.
fn notify_changes(
    entry: &RepoEntry,
    prev: &RepoSnapshot,
    next: &RepoSnapshot,
    user_stopped: &mut HashSet<String>,
) {
    for process in &next.processes {
        let Some(before) = prev.processes.iter().find(|p| p.key == process.key) else {
            continue;
        };
        let repo = entry.name();
        let name = &process.spec.name;
        match (before.status, process.status) {
            (Status::Running | Status::External | Status::Degraded, Status::Stopped) => {
                if !user_stopped.remove(&process.key) {
                    launch::notify(&repo, &format!("{name} stopped"));
                }
            }
            (Status::Running | Status::External, Status::Degraded) => {
                launch::notify(&repo, &format!("{name} is failing its health check"));
            }
            _ => {}
        }
    }
    for session in &next.sessions {
        let was_busy = prev
            .sessions
            .iter()
            .any(|s| s.id == session.id && s.state == SessionState::Busy);
        if was_busy && session.state == SessionState::Idle {
            launch::notify(
                &format!("{} — {}", session.agent.label(), entry.name()),
                &format!("Waiting for you: {}", session.title),
            );
        }
    }
}

impl Render for Workspace {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let translucent = theme::is_translucent();
        if self.applied_translucent != Some(translucent) {
            window.set_background_appearance(theme::window_background());
            self.applied_translucent = Some(translucent);
        }
        let main = if self.repos.is_empty() {
            self.render_empty(cx).into_any_element()
        } else {
            self.render_repo(window, cx).into_any_element()
        };
        div()
            .id("workspace")
            .relative()
            .flex()
            .flex_col()
            .size_full()
            .bg(theme::backdrop())
            .text_color(theme::text())
            .track_focus(&self.focus)
            .on_action(cx.listener(|this, _: &TogglePalette, window, cx| {
                this.toggle_palette(window);
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &NextRepo, _, cx| {
                if !this.repos.is_empty() {
                    this.select((this.selected + 1) % this.repos.len());
                }
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &PrevRepo, _, cx| {
                if !this.repos.is_empty() {
                    this.select((this.selected + this.repos.len() - 1) % this.repos.len());
                }
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &ShowOverview, _, cx| {
                this.run(Command::ShowTab(Tab::Overview), cx)
            }))
            .on_action(cx.listener(|this, _: &ShowProcesses, _, cx| {
                this.run(Command::ShowTab(Tab::Processes), cx)
            }))
            .on_action(cx.listener(|this, _: &ShowDatabases, _, cx| {
                this.run(Command::ShowTab(Tab::Databases), cx)
            }))
            .on_action(
                cx.listener(|this, _: &ShowLogs, _, cx| this.run(Command::ShowTab(Tab::Logs), cx)),
            )
            .on_action(cx.listener(|this, _: &ShowWorktrees, _, cx| {
                this.run(Command::ShowTab(Tab::Worktrees), cx)
            }))
            .on_action(
                cx.listener(|this, _: &ShowPlans, _, cx| {
                    this.run(Command::ShowTab(Tab::Plans), cx)
                }),
            )
            .on_action(
                cx.listener(|this, _: &ShowSetup, _, cx| {
                    this.run(Command::ShowTab(Tab::Setup), cx)
                }),
            )
            .on_action(cx.listener(|this, _: &ShowTerminal, _, cx| {
                this.run(Command::ShowTab(Tab::Terminal), cx)
            }))
            .on_action(
                cx.listener(|this, _: &ToggleTheme, _, cx| this.run(Command::ToggleTheme, cx)),
            )
            .on_action(cx.listener(|this, _: &ChooseTheme, window, cx| {
                if !this.palette_open {
                    this.toggle_palette(window);
                }
                this.palette_field.text = "theme ".into();
                this.palette_selected = 0;
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &ShowSessions, _, cx| {
                this.run(Command::ShowTab(Tab::Sessions), cx)
            }))
            .on_action(cx.listener(|this, _: &ShowKnowledge, _, cx| {
                this.run(Command::ShowTab(Tab::Knowledge), cx)
            }))
            .on_action(
                cx.listener(|this, _: &RegisterRepo, _, cx| this.run(Command::RegisterRepo, cx)),
            )
            .on_action(cx.listener(|this, _: &Refresh, _, cx| this.run(Command::Refresh, cx)))
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                if event.keystroke.key == "escape" && this.panel.is_some() {
                    this.panel = None;
                    cx.notify();
                }
            }))
            .on_drop(cx.listener(|this, paths: &ExternalPaths, _, cx| {
                this.show_file_context(paths.paths().to_vec(), cx);
            }))
            .child(self.render_titlebar())
            .child(
                div()
                    .flex()
                    .flex_1()
                    .overflow_hidden()
                    .child(self.render_sidebar(cx))
                    .child(main),
            )
            .when(self.palette_open, |root| {
                root.child(self.render_palette(window, cx))
            })
            .children(self.render_panel(cx))
            .children(self.render_toast())
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
        let source = include_str!("mod.rs");
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
