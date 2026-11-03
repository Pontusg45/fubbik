//! Every key binding in one table: what binds the keys, what the ⌘/ cheat
//! sheet and the ⌘K hints show, and what `keymap.toml` overrides. One
//! source, so the menu, palette and cheat sheet never disagree.
//!
//! `~/.config/fubbik/keymap.toml` (next to desktop.toml):
//!
//! ```toml
//! [bindings]
//! toggle_palette = "cmd-p"   # rebind
//! show_today = "cmd-1"
//! next_needs_you = ""        # unbind
//! ```

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

use gpui::{App, KeyBinding};
use serde::Deserialize;

use super::*;

/// Context name the terminal view sets (`key_context`): bindings in it win
/// while a terminal has focus.
pub const TERMINAL: &str = "Terminal";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Binding {
    /// GPUI keystrokes, e.g. `cmd-shift-h`; empty when unbound.
    pub keys: String,
    /// The action's id in keymap.toml.
    pub action: &'static str,
    pub context: Option<&'static str>,
    pub group: &'static str,
    pub description: &'static str,
}

const fn b(
    keys: &'static str,
    action: &'static str,
    context: Option<&'static str>,
    group: &'static str,
    description: &'static str,
) -> (
    &'static str,
    &'static str,
    Option<&'static str>,
    &'static str,
    &'static str,
) {
    (keys, action, context, group, description)
}

const T: Option<&str> = Some(TERMINAL);

/// The defaults. Order is the cheat sheet's order.
const DEFAULTS: &[(&str, &str, Option<&str>, &str, &str)] = &[
    b(
        "cmd-k",
        "toggle_palette",
        None,
        "General",
        "Command palette (@ repos · # tasks · ! running · ? chunks · > commands)",
    ),
    b(
        "cmd-/",
        "show_cheat_sheet",
        None,
        "General",
        "This cheat sheet",
    ),
    b(
        "cmd-shift-n",
        "capture_note",
        None,
        "General",
        "Capture a note into the repo's fubbik space",
    ),
    b(
        "cmd-r",
        "refresh",
        None,
        "General",
        "Refresh everything now",
    ),
    b("cmd-o", "register_repo", None, "General", "Register a repo"),
    b(
        "alt-cmd-t",
        "toggle_theme",
        None,
        "General",
        "Translucent window on / off",
    ),
    b(
        "cmd-q",
        "quit",
        None,
        "General",
        "Quit (shells in tmux keep running)",
    ),
    b(
        "cmd-0",
        "show_home",
        None,
        "Tabs",
        "Home: every project and the computer's stats",
    ),
    b("cmd-1", "show_today", None, "Tabs", "Today"),
    b("cmd-2", "show_overview", None, "Tabs", "Overview"),
    b("cmd-3", "show_processes", None, "Tabs", "Processes"),
    b("cmd-4", "show_logs", None, "Tabs", "Logs"),
    b("cmd-5", "show_worktrees", None, "Tabs", "Worktrees"),
    b("cmd-6", "show_sessions", None, "Tabs", "AI Sessions"),
    b("cmd-7", "show_plans", None, "Tabs", "Plans"),
    b("cmd-8", "show_knowledge", None, "Tabs", "Knowledge"),
    b("cmd-9", "show_terminal", None, "Tabs", "Terminal"),
    b("cmd-shift-d", "show_databases", None, "Tabs", "Databases"),
    b("cmd-shift-i", "show_insights", None, "Tabs", "Insights"),
    b("cmd-shift-g", "show_changes", None, "Tabs", "Changes (git)"),
    b("cmd-shift-a", "show_agents", None, "Tabs", "All agents"),
    b("cmd-shift-t", "show_tools", None, "Tabs", "Tools"),
    b(
        "cmd-shift-l",
        "show_deps",
        None,
        "Tabs",
        "Dependencies across repos",
    ),
    b("cmd-shift-u", "show_tests", None, "Tabs", "Tests"),
    b(
        "cmd-,",
        "show_settings",
        None,
        "Tabs",
        "Settings (app-wide)",
    ),
    b(
        "cmd-shift-,",
        "show_setup",
        None,
        "Tabs",
        "This repo's setup",
    ),
    b("cmd-[", "prev_repo", None, "Repos", "Previous repo"),
    b("cmd-]", "next_repo", None, "Repos", "Next repo"),
    b(
        "ctrl-1",
        "select_repo_1",
        None,
        "Repos",
        "Repo 1 (⌃1–⌃9: repo 1–9)",
    ),
    b("ctrl-2", "select_repo_2", None, "Repos", "Repo 2"),
    b("ctrl-3", "select_repo_3", None, "Repos", "Repo 3"),
    b("ctrl-4", "select_repo_4", None, "Repos", "Repo 4"),
    b("ctrl-5", "select_repo_5", None, "Repos", "Repo 5"),
    b("ctrl-6", "select_repo_6", None, "Repos", "Repo 6"),
    b("ctrl-7", "select_repo_7", None, "Repos", "Repo 7"),
    b("ctrl-8", "select_repo_8", None, "Repos", "Repo 8"),
    b("ctrl-9", "select_repo_9", None, "Repos", "Repo 9"),
    b(
        "cmd-shift-e",
        "focus_sidebar",
        None,
        "Repos",
        "Move through repos with j/k (⏎ or esc to leave)",
    ),
    b(
        "cmd-shift-j",
        "next_needs_you",
        None,
        "Agents & tasks",
        "Jump to the next thing that needs you",
    ),
    b(
        "cmd-shift-r",
        "reply_latest",
        None,
        "Agents & tasks",
        "Reply to the agent waiting longest",
    ),
    b(
        "cmd-shift-s",
        "start_next_task",
        None,
        "Agents & tasks",
        "Start the next startable task in the background",
    ),
    b("cmd-t", "new_shell", T, "Terminal", "New shell tab"),
    b(
        "cmd-w",
        "close_terminal_tab",
        T,
        "Terminal",
        "Close this terminal tab",
    ),
    b(
        "cmd-shift-[",
        "prev_terminal_tab",
        T,
        "Terminal",
        "Previous terminal tab",
    ),
    b(
        "cmd-shift-]",
        "next_terminal_tab",
        T,
        "Terminal",
        "Next terminal tab",
    ),
    b("cmd-d", "split_terminal", T, "Terminal", "Split / unsplit"),
    b(
        "cmd-shift-k",
        "clear_terminal",
        T,
        "Terminal",
        "Clear the screen and scrollback",
    ),
    b(
        "cmd-1",
        "terminal_tab_1",
        T,
        "Terminal",
        "Terminal tab 1 (⌘1–⌘9 while a terminal has focus)",
    ),
    b("cmd-2", "terminal_tab_2", T, "Terminal", "Terminal tab 2"),
    b("cmd-3", "terminal_tab_3", T, "Terminal", "Terminal tab 3"),
    b("cmd-4", "terminal_tab_4", T, "Terminal", "Terminal tab 4"),
    b("cmd-5", "terminal_tab_5", T, "Terminal", "Terminal tab 5"),
    b("cmd-6", "terminal_tab_6", T, "Terminal", "Terminal tab 6"),
    b("cmd-7", "terminal_tab_7", T, "Terminal", "Terminal tab 7"),
    b("cmd-8", "terminal_tab_8", T, "Terminal", "Terminal tab 8"),
    b("cmd-9", "terminal_tab_9", T, "Terminal", "Terminal tab 9"),
];

/// Keys handled by the lists themselves (not rebindable), for the cheat
/// sheet and the hint bar.
pub const LIST_KEYS: &[(&str, &str, &str)] = &[
    ("Lists", "j / k  ↓ / ↑", "Move the highlight"),
    ("Lists", "⏎", "The highlighted row's main action"),
    ("Home", "⏎", "Open the repo's Overview"),
    (
        "Today",
        "⏎",
        "Open what needs you, the task's timeline, or start it",
    ),
    (
        "Processes",
        "s · r · l · o · a · K",
        "Start/stop · restart · logs · open in the browser · attach (tmux) · kill (outside the app)",
    ),
    (
        "Worktrees",
        "c · v · t · n · x",
        "Context · review · shell · new Claude · remove",
    ),
    ("AI Sessions", "⏎ · r", "Open its pane (or resume) · reply"),
    (
        "Plans",
        "h / l · ← / → · space · s · t · b",
        "Column · move card · checklist · start · timeline · block",
    ),
    (
        "Knowledge",
        "d · a · r · o",
        "Dismiss stale · approve · reject proposal · open",
    ),
    (
        "Terminal",
        "⌘F · ⌘C · ⌘V · ⌘-click",
        "Search · copy · paste · open link",
    ),
];

pub fn defaults() -> Vec<Binding> {
    DEFAULTS
        .iter()
        .map(|&(keys, action, context, group, description)| Binding {
            keys: keys.into(),
            action,
            context,
            group,
            description,
        })
        .collect()
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct KeymapFile {
    #[serde(default)]
    bindings: BTreeMap<String, String>,
}

pub fn path() -> PathBuf {
    crate::config::path().with_file_name("keymap.toml")
}

/// The defaults with `keymap.toml` applied, and what was wrong in it.
pub fn effective(text: Option<&str>) -> (Vec<Binding>, Vec<String>) {
    let mut bindings = defaults();
    let mut errors = Vec::new();
    let Some(text) = text else {
        return (bindings, errors);
    };
    let file: KeymapFile = match toml::from_str(text) {
        Ok(file) => file,
        Err(e) => return (bindings, vec![format!("keymap.toml: {}", e.message())]),
    };
    for (action, keys) in file.bindings {
        match bindings.iter_mut().find(|b| b.action == action) {
            Some(binding) => binding.keys = keys.trim().to_string(),
            None => errors.push(format!("keymap.toml: no action named “{action}”")),
        }
    }
    (bindings, errors)
}

/// Keys bound to more than one action in the same context.
pub fn conflicts(bindings: &[Binding]) -> Vec<(String, Vec<&'static str>)> {
    let mut by_keys: BTreeMap<(String, Option<&str>), Vec<&'static str>> = BTreeMap::new();
    for binding in bindings.iter().filter(|b| !b.keys.is_empty()) {
        by_keys
            .entry((binding.keys.clone(), binding.context))
            .or_default()
            .push(binding.action);
    }
    by_keys
        .into_iter()
        .filter(|(_, actions)| actions.len() > 1)
        .map(|((keys, _), actions)| (keys, actions))
        .collect()
}

/// `cmd-shift-h` → `⇧⌘H`, in the Mac's modifier order.
pub fn pretty(keys: &str) -> String {
    if keys.is_empty() {
        return String::new();
    }
    keys.split(' ')
        .map(|stroke| {
            let parts: Vec<&str> = stroke.split('-').collect();
            // "cmd--" style bindings for the minus key end in an empty part.
            let (mods, key) = match parts.split_last() {
                Some((&"", rest)) if !rest.is_empty() => (&rest[..rest.len() - 1], "-"),
                Some((key, rest)) => (rest, *key),
                None => (&[][..], ""),
            };
            let mut out = String::new();
            for (name, symbol) in [("ctrl", "⌃"), ("alt", "⌥"), ("shift", "⇧"), ("cmd", "⌘")]
            {
                if mods.contains(&name) {
                    out.push_str(symbol);
                }
            }
            out.push_str(&match key {
                "enter" => "⏎".to_string(),
                "escape" => "esc".to_string(),
                "space" => "Space".to_string(),
                "up" => "↑".to_string(),
                "down" => "↓".to_string(),
                "left" => "←".to_string(),
                "right" => "→".to_string(),
                "backspace" => "⌫".to_string(),
                other => other.to_uppercase(),
            });
            out
        })
        .collect::<Vec<_>>()
        .join(" ")
}

static EFFECTIVE: RwLock<Vec<Binding>> = RwLock::new(Vec::new());
static ERRORS: RwLock<Vec<String>> = RwLock::new(Vec::new());

/// The keys for an action, pretty, as currently bound (global context).
pub fn keys_for(action: &str) -> Option<String> {
    let bindings = EFFECTIVE.read().unwrap();
    let list: &[Binding] = if bindings.is_empty() { &[] } else { &bindings };
    list.iter()
        .chain(defaults().iter().filter(|_| list.is_empty()))
        .find(|b| b.action == action && b.context.is_none())
        .map(|b| pretty(&b.keys))
        .filter(|k| !k.is_empty())
}

pub fn current() -> Vec<Binding> {
    let bindings = EFFECTIVE.read().unwrap();
    if bindings.is_empty() {
        defaults()
    } else {
        bindings.clone()
    }
}

/// keymap.toml problems and key clashes, for Setup.
pub fn problems() -> Vec<String> {
    let mut out = ERRORS.read().unwrap().clone();
    for (keys, actions) in conflicts(&current()) {
        out.push(format!(
            "{} is bound to {}",
            pretty(&keys),
            actions.join(" and ")
        ));
    }
    out
}

fn make(action: &str, keys: &str, context: Option<&str>) -> Option<KeyBinding> {
    macro_rules! bind {
        ($($name:literal => $action:expr),* $(,)?) => {
            match action {
                $($name => Some(KeyBinding::new(keys, $action, context)),)*
                _ => None,
            }
        };
    }
    bind! {
        "toggle_palette" => TogglePalette,
        "show_cheat_sheet" => ShowCheatSheet,
        "capture_note" => CaptureNote,
        "refresh" => Refresh,
        "register_repo" => RegisterRepo,
        "toggle_theme" => ToggleTheme,
        "quit" => Quit,
        "show_today" => ShowToday,
        "show_overview" => ShowOverview,
        "show_processes" => ShowProcesses,
        "show_logs" => ShowLogs,
        "show_worktrees" => ShowWorktrees,
        "show_sessions" => ShowSessions,
        "show_plans" => ShowPlans,
        "show_knowledge" => ShowKnowledge,
        "show_terminal" => ShowTerminal,
        "show_databases" => ShowDatabases,
                "show_insights" => ShowInsights,
                "show_changes" => ShowChanges,
                "show_tools" => ShowTools,
        "show_deps" => ShowDeps,
        "show_tests" => ShowTests,
        "show_settings" => ShowSettings,
        "show_home" => ShowHome,
        "show_agents" => ShowAgents,
        "show_setup" => ShowSetup,
        "prev_repo" => PrevRepo,
        "next_repo" => NextRepo,
        "select_repo_1" => SelectRepo1,
        "select_repo_2" => SelectRepo2,
        "select_repo_3" => SelectRepo3,
        "select_repo_4" => SelectRepo4,
        "select_repo_5" => SelectRepo5,
        "select_repo_6" => SelectRepo6,
        "select_repo_7" => SelectRepo7,
        "select_repo_8" => SelectRepo8,
        "select_repo_9" => SelectRepo9,
        "focus_sidebar" => FocusSidebar,
        "next_needs_you" => NextNeedsYou,
        "reply_latest" => ReplyLatest,
        "start_next_task" => StartNextTask,
        "new_shell" => NewShell,
        "close_terminal_tab" => CloseTerminalTab,
        "prev_terminal_tab" => PrevTerminalTab,
        "next_terminal_tab" => NextTerminalTab,
        "split_terminal" => SplitTerminal,
        "clear_terminal" => ClearTerminal,
        "terminal_tab_1" => TerminalTab1,
        "terminal_tab_2" => TerminalTab2,
        "terminal_tab_3" => TerminalTab3,
        "terminal_tab_4" => TerminalTab4,
        "terminal_tab_5" => TerminalTab5,
        "terminal_tab_6" => TerminalTab6,
        "terminal_tab_7" => TerminalTab7,
        "terminal_tab_8" => TerminalTab8,
        "terminal_tab_9" => TerminalTab9,
    }
}

/// Reads keymap.toml, binds every key, and remembers the result for the
/// cheat sheet, the palette hints and Setup.
pub fn bind_all(cx: &mut App, file: &Path) {
    let text = std::fs::read_to_string(file).ok();
    let (bindings, mut errors) = effective(text.as_deref());
    let mut keys = Vec::new();
    for binding in bindings.iter().filter(|b| !b.keys.is_empty()) {
        match std::panic::catch_unwind(|| make(binding.action, &binding.keys, binding.context)) {
            Ok(Some(key)) => keys.push(key),
            Ok(None) => errors.push(format!("{} has no handler", binding.action)),
            Err(_) => errors.push(format!(
                "keymap.toml: “{}” is not a valid key for {}",
                binding.keys, binding.action
            )),
        }
    }
    cx.bind_keys(keys);
    *EFFECTIVE.write().unwrap() = bindings;
    *ERRORS.write().unwrap() = errors;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_default_action_can_be_bound() {
        // Given
        let bindings = defaults();

        // When
        let missing: Vec<&str> = bindings
            .iter()
            .filter(|b| make(b.action, &b.keys, b.context).is_none())
            .map(|b| b.action)
            .collect();

        // Then
        assert!(missing.is_empty(), "no KeyBinding for {missing:?}");
    }

    #[test]
    fn defaults_have_no_clashes() {
        // Given
        let bindings = defaults();

        // When
        let clashes = conflicts(&bindings);

        // Then: ⌘1–⌘9 appear twice, but in different contexts
        assert!(clashes.is_empty(), "{clashes:?}");
    }

    #[test]
    fn overrides_rebind_unbind_and_report_unknown_actions() {
        // Given
        let text = "[bindings]\ntoggle_palette = \"cmd-p\"\nnext_needs_you = \"\"\nshow_today = \"cmd-2\"\nfly = \"cmd-f\"\n";

        // When
        let (bindings, errors) = effective(Some(text));
        let find = |a: &str| {
            bindings
                .iter()
                .find(|b| b.action == a)
                .unwrap()
                .keys
                .clone()
        };

        // Then
        assert_eq!(find("toggle_palette"), "cmd-p");
        assert_eq!(find("next_needs_you"), "");
        assert_eq!(errors, ["keymap.toml: no action named “fly”"]);
        assert_eq!(
            conflicts(&bindings),
            [("cmd-2".to_string(), vec!["show_today", "show_overview"])]
        );
    }

    #[test]
    fn keys_read_the_mac_way() {
        // Given
        let keys = [
            "cmd-shift-h",
            "alt-cmd-t",
            "ctrl-1",
            "cmd-,",
            "cmd-shift-[",
            "cmd-k cmd-s",
            "enter",
        ];

        // When
        let shown: Vec<String> = keys.iter().map(|k| pretty(k)).collect();

        // Then
        assert_eq!(shown, ["⇧⌘H", "⌥⌘T", "⌃1", "⌘,", "⇧⌘[", "⌘K ⌘S", "⏎"]);
    }
}
