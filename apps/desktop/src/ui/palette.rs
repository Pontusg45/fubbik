//! The ⌘K command palette: every command the app can run, filtered by a
//! subsequence match on what was typed.

use std::path::PathBuf;

use super::Tab;
use crate::sessions::Agent;

/// Where a process or script runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunTarget {
    /// A child of the app, output in Logs.
    Background,
    /// A detached session on the app's tmux server: attach from any
    /// terminal; it keeps running when the app quits.
    Tmux,
    /// A terminal tab in the app.
    Tab,
    /// Terminal.app.
    TerminalApp,
}

impl RunTarget {
    pub const ALL: [RunTarget; 4] = [
        RunTarget::Background,
        RunTarget::Tmux,
        RunTarget::Tab,
        RunTarget::TerminalApp,
    ];

    pub fn key(self) -> &'static str {
        match self {
            RunTarget::Background => "background",
            RunTarget::Tmux => "tmux",
            RunTarget::Tab => "tab",
            RunTarget::TerminalApp => "terminal",
        }
    }

    pub fn from_key(key: &str) -> Option<RunTarget> {
        RunTarget::ALL.into_iter().find(|t| t.key() == key)
    }

    pub fn title(self) -> &'static str {
        match self {
            RunTarget::Background => "In the background",
            RunTarget::Tmux => "In a tmux session",
            RunTarget::Tab => "In a terminal tab",
            RunTarget::TerminalApp => "In Terminal.app",
        }
    }

    pub fn detail(self) -> &'static str {
        match self {
            RunTarget::Background => "The app runs it; output in Logs. Stops when the app quits.",
            RunTarget::Tmux => {
                "Detached on the app's tmux server: attach from any terminal; keeps running when the app quits."
            }
            RunTarget::Tab => {
                "Opens a tab in the Terminal view, where you can type into it. The app shows it as running but cannot stop or log it."
            }
            RunTarget::TerminalApp => "A new Terminal.app window. The app cannot stop or log it.",
        }
    }
}

/// A confirmation before something that cannot be undone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Confirm {
    pub title: String,
    pub detail: String,
    /// The confirm button's label ("Remove", "Kill", "Merge"…).
    pub action: String,
    pub danger: bool,
    pub command: Command,
}

/// What to run, before knowing where.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunWhat {
    Process {
        repo: usize,
        index: usize,
    },
    /// Every stopped process in a repo.
    All(usize),
    /// Every stopped process in each of these repos (a sidebar group).
    Group(Vec<usize>),
    Profile {
        repo: usize,
        profile: String,
    },
    Script {
        repo: usize,
        name: String,
        command: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    SelectRepo(usize),
    StartProcess(usize, usize),
    StopProcess(usize, usize),
    RestartProcess(usize, usize),
    StartAll(usize),
    StopAll(usize),
    ShowTab(Tab),
    RegisterRepo,
    RemoveRepo(usize),
    GenerateServices(usize),
    OpenEditor(usize),
    OpenTerminal(usize),
    OpenGitHub(usize),
    Refresh,
    OpenEditorAt(PathBuf),
    OpenTerminalAt(PathBuf),
    /// A new Claude Code session in Terminal, in this directory.
    NewSession(PathBuf),
    ResumeSession(Agent, String, PathBuf),
    CreateWorktree(usize),
    RemoveWorktree(usize, PathBuf),
    PruneWorktrees(usize),
    ToggleTheme,
    /// Terminal tabs as tmux sessions (true) or plain shells.
    SetPersistence(bool),
    /// AI sessions in Terminal.app (true) or the Terminal tab.
    SetExternalTerminal(bool),
    /// The built-in server (API + PostgreSQL) on (true) or off; on restart.
    SetEmbeddedBackend(bool),
    /// Replace the built-in database with a copy of this one.
    ImportDatabase(String),
    BackupNow,
    /// Run something where chosen; `remember` saves the choice.
    RunAt {
        what: RunWhat,
        target: RunTarget,
        remember: bool,
    },
    /// Where processes and scripts start: None asks each time.
    SetRunIn(Option<RunTarget>),
    /// A run's tmux session, in a terminal tab.
    AttachRun(String),
    /// `git branch -d` (merged branches only).
    DeleteBranch {
        repo: usize,
        name: String,
    },
    /// A missing repo's new folder (asks for it).
    LocateRepo(usize),
    /// Closes toast `index` (0 = oldest shown).
    DismissToast(usize),
    /// Asks first, then runs the confirmation's command.
    AskFirst(Box<Confirm>),
    /// Stops whatever the supervisor runs under this key (a script, a task).
    StopKey(String),
    /// SIGTERM to a process the app did not start (holding a port).
    KillPid {
        pid: u32,
        what: String,
    },
    /// Approve or reject a proposed knowledge change.
    Review {
        proposal: String,
        approve: bool,
    },
    /// `git worktree remove --force` after a plain remove was refused.
    ForceRemoveWorktree(usize, PathBuf),
    /// Empties a tmux run's log file.
    ClearRunLog(String),
    /// A database task from the services file (start, stop, migrate, seed, studio).
    DatabaseTask {
        repo: usize,
        index: usize,
        action: &'static str,
    },
    ShowBackups,
    /// Stop a `--serve` server left running by "Quit, Keep Server Running".
    StopBackgroundServer,
    SetTaskStatus {
        plan: String,
        task: String,
        status: &'static str,
    },
    ToggleCriterion {
        repo: usize,
        plan: String,
        task: String,
        index: usize,
    },
    TaskTimeline {
        repo: usize,
        plan: String,
        task: String,
    },
    SaveTaskNote {
        repo: usize,
        plan: String,
        task: String,
    },
    /// The plan the board shows.
    SelectPlan(String),
    /// Expands or collapses a task card's acceptance checklist.
    ToggleChecklist(String),
    /// Runs a repo script (just, npm, make) as a logged task.
    RunScript {
        repo: usize,
        name: String,
        command: String,
    },
    /// Starts exactly a profile's processes and stops the repo's others.
    StartProfile {
        repo: usize,
        profile: String,
    },
    SetMaxAgents(Option<usize>),
    /// Quick capture of a note into the selected repo's space.
    CaptureNote,
    /// An agent's tmux pane: its tab, or attached in a new one.
    OpenPane {
        pane: crate::tmux::Pane,
        cwd: PathBuf,
    },
    /// A process's (or script's) log, by its supervisor key.
    ShowLogs {
        repo: usize,
        key: String,
    },
    /// Select a repo and show one of its tabs.
    Focus {
        repo: usize,
        tab: Tab,
    },
    /// A worktree's branch against main, with PR and merge actions.
    ReviewWorktree {
        repo: usize,
        path: PathBuf,
    },
    /// Runs a command in a terminal tab (where its prompts can be answered).
    RunIn {
        dir: PathBuf,
        label: String,
        command: String,
    },
    /// None: built-in; `"follow"`: the Ghostty config; else a theme name.
    SetColors(Option<String>),
    /// The embedded terminal, in this directory.
    ShellAt(PathBuf),
    CloseTerminal(usize),
    /// Splits the Terminal tab: a new shell beside the active one; again to
    /// unsplit (the second shell keeps running as a tab).
    ToggleSplit,
    /// A path on the fubbik web app, such as `/chunks/<id>`.
    OpenWeb(String),
    StartTask {
        repo: usize,
        plan: String,
        task: String,
        new_worktree: bool,
        /// Start without switching to it (a detached tmux session).
        background: bool,
    },
    /// The reply box for the agent that has waited longest (needs a window:
    /// the palette handles it).
    ReplyWaiting,
    /// The first ready task under Today's "Next up", in the background.
    StartNextTask,
    /// The next "Needs you" line on Today.
    NextNeedsYou,
    ShowHome,
    /// Any URL in the browser (a pull request).
    OpenUrl(String),
    /// A terminal tab by id, in its repo (clears its bell).
    FocusTerminal(usize),
    /// `.mcp.json` and CLAUDE.md for a tracker agent in the repos' folder.
    SetupTracker,
    /// A watch's alert leaves Today.
    DismissAlert(String),
    /// A tracker todo is done.
    CompleteNote(String),
}

#[derive(Debug, Clone)]
pub struct Entry {
    pub label: String,
    pub command: Command,
}

/// Case-insensitive subsequence match, scored so that earlier and
/// contiguous hits rank first. None when `query` is not a subsequence.
pub fn score(label: &str, query: &str) -> Option<i32> {
    if query.is_empty() {
        return Some(0);
    }
    let label: Vec<char> = label.to_lowercase().chars().collect();
    let mut score = 0;
    let mut position = 0;
    let mut previous: Option<usize> = None;
    for wanted in query.to_lowercase().chars().filter(|c| !c.is_whitespace()) {
        let found = (position..label.len()).find(|&i| label[i] == wanted)?;
        score -= found as i32;
        if previous.is_some_and(|p| p + 1 == found) {
            score += 5;
        }
        previous = Some(found);
        position = found + 1;
    }
    Some(score)
}

/// Recent palette commands kept.
pub const RECENT: usize = 8;

/// Moves entries whose command is in `recent` to the front, newest first;
/// the rest keep their order.
pub fn recent_first(entries: &mut Vec<Entry>, recent: &[Command]) {
    let rank = |e: &Entry| {
        recent
            .iter()
            .position(|c| *c == e.command)
            .unwrap_or(usize::MAX)
    };
    entries.sort_by_key(rank);
    let mut seen = Vec::new();
    entries.retain(|e| {
        if rank(e) == usize::MAX {
            return true;
        }
        let first = !seen.contains(&e.command);
        seen.push(e.command.clone());
        first
    });
}

#[cfg(test)]
pub fn filter(entries: &[Entry], query: &str) -> Vec<Entry> {
    filter_with(entries, query, |_| 0)
}

/// [`filter`], with `bonus` added to each match's score (the selected
/// repo's entries rank first).
pub fn filter_with(entries: &[Entry], query: &str, bonus: impl Fn(&Entry) -> i32) -> Vec<Entry> {
    let mut scored: Vec<(i32, &Entry)> = entries
        .iter()
        .filter_map(|entry| score(&entry.label, query).map(|s| (s + bonus(entry), entry)))
        .collect();
    scored.sort_by_key(|(score, _)| std::cmp::Reverse(*score));
    scored.into_iter().map(|(_, entry)| entry.clone()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(label: &str) -> Entry {
        Entry {
            label: label.into(),
            command: Command::Refresh,
        }
    }

    #[test]
    fn subsequence_matches() {
        // Given
        let label = "Start api — fubbik";

        // When
        let hit = score(label, "stapi");
        let miss = score(label, "zz");

        // Then
        assert!(hit.is_some());
        assert!(miss.is_none());
    }

    #[test]
    fn contiguous_match_ranks_first() {
        // Given
        let entries = [entry("Stop all — fubbik"), entry("Start api — fubbik")];

        // When
        let ranked = filter(&entries, "start");

        // Then
        assert_eq!(ranked[0].label, "Start api — fubbik");
        assert_eq!(ranked.len(), 1);
    }

    #[test]
    fn recent_commands_come_first_newest_first() {
        // Given
        let mut entries = vec![
            Entry {
                label: "a".into(),
                command: Command::Refresh,
            },
            Entry {
                label: "b".into(),
                command: Command::RegisterRepo,
            },
            Entry {
                label: "c".into(),
                command: Command::ToggleTheme,
            },
        ];

        // When
        recent_first(&mut entries, &[Command::ToggleTheme, Command::RegisterRepo]);

        // Then
        assert_eq!(
            entries.iter().map(|e| e.label.as_str()).collect::<Vec<_>>(),
            ["c", "b", "a"]
        );
    }

    #[test]
    fn a_bonus_lifts_an_entry_above_a_better_match() {
        // Given
        let entries = [entry("Start api — other"), entry("Start api — mine")];

        // When
        let ranked = filter_with(&entries, "start", |e| {
            if e.label.ends_with("— mine") { 10 } else { 0 }
        });

        // Then
        assert_eq!(ranked[0].label, "Start api — mine");
    }

    #[test]
    fn empty_query_keeps_order() {
        // Given
        let entries = [entry("b"), entry("a")];

        // When
        let ranked = filter(&entries, "");

        // Then
        assert_eq!(
            ranked.iter().map(|e| e.label.as_str()).collect::<Vec<_>>(),
            ["b", "a"]
        );
    }
}
