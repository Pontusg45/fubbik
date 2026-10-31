//! UI tests: a real `Workspace` in a GPUI test window, driven by actions and
//! simulated keystrokes. Each test uses its own config file with tmux
//! persistence off, so nothing touches the user's setup.

use gpui::{Entity, TestAppContext, VisualTestContext};

use super::*;
use crate::supervisor::Supervisor;

/// One config path for the whole test binary: `config::path()` reads it from
/// the environment, which tests share. Each test rewrites its contents.
fn config_file() -> PathBuf {
    static DIR: std::sync::OnceLock<tempfile::TempDir> = std::sync::OnceLock::new();
    let dir = DIR.get_or_init(|| tempfile::tempdir().unwrap());
    let path = dir.path().join("desktop.toml");
    // SAFETY: tests that read FUBBIK_DESKTOP_CONFIG all set it to this value.
    unsafe { std::env::set_var("FUBBIK_DESKTOP_CONFIG", &path) };
    path
}

/// UI tests share the config file and the theme globals: run them one at a time.
fn serial() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

fn open<'a>(
    cx: &'a mut TestAppContext,
    config: &str,
) -> (Entity<Workspace>, &'a mut VisualTestContext) {
    std::fs::write(
        config_file(),
        format!("terminal_persistence = \"none\"\n{config}"),
    )
    .unwrap();
    theme::set_translucent(false);
    let (view, cx) = cx.add_window_view(|window, cx| Workspace::new(Supervisor::new(), window, cx));
    cx.run_until_parked();
    (view, cx)
}

#[gpui::test]
fn tab_actions_switch_tabs(cx: &mut TestAppContext) {
    let _guard = serial();
    // Given
    let (view, cx) = open(cx, "");

    // When / Then
    fn check<A: gpui::Action>(
        view: &Entity<Workspace>,
        cx: &mut VisualTestContext,
        action: A,
        tab: Tab,
    ) {
        cx.dispatch_action(action);
        cx.run_until_parked();
        assert_eq!(view.read_with(cx, |w, _| w.tab), tab);
    }
    check(&view, cx, ShowProcesses, Tab::Processes);
    check(&view, cx, ShowLogs, Tab::Logs);
    check(&view, cx, ShowWorktrees, Tab::Worktrees);
    check(&view, cx, ShowSessions, Tab::Sessions);
    check(&view, cx, ShowPlans, Tab::Plans);
    check(&view, cx, ShowKnowledge, Tab::Knowledge);
    check(&view, cx, ShowSetup, Tab::Setup);
    check(&view, cx, ShowTerminal, Tab::Terminal);
    check(&view, cx, ShowOverview, Tab::Overview);
}

#[gpui::test]
fn toggle_theme_flips_translucency_and_saves_it(cx: &mut TestAppContext) {
    let _guard = serial();
    // Given
    let (view, cx) = open(cx, "");

    // When
    cx.dispatch_action(ToggleTheme);
    cx.run_until_parked();

    // Then
    assert!(theme::is_translucent());
    assert_eq!(
        view.read_with(cx, |w, _| w.config.theme.clone()).as_deref(),
        Some("translucent")
    );
    let saved = std::fs::read_to_string(config_file()).unwrap();
    assert!(saved.contains("theme = \"translucent\""), "saved: {saved}");
    theme::set_translucent(false);
}

#[gpui::test]
fn palette_opens_filters_typed_text_and_closes(cx: &mut TestAppContext) {
    let _guard = serial();
    // Given
    let (view, cx) = open(cx, "");

    // When
    cx.dispatch_action(TogglePalette);
    cx.run_until_parked();
    cx.simulate_keystrokes("r e f r");
    let (open, query, first) = view.read_with(cx, |w, _| {
        (
            w.palette_open,
            w.palette_field.text.clone(),
            w.palette_matches().first().map(|e| e.label.clone()),
        )
    });
    cx.simulate_keystrokes("escape");

    // Then
    assert!(open);
    assert_eq!(query, "refr");
    assert_eq!(first.as_deref(), Some("Refresh"));
    assert!(!view.read_with(cx, |w, _| w.palette_open));
}

#[gpui::test]
fn shell_opens_a_terminal_tab_that_closes(cx: &mut TestAppContext) {
    let _guard = serial();
    // Given: a registered repo
    let repo = tempfile::tempdir().unwrap();
    let path = repo.path().to_path_buf();
    let (view, cx) = open(
        cx,
        &format!("[[repo]]\npath = {:?}\n", path.display().to_string()),
    );

    // When
    view.update(cx, |w, cx| w.run(Command::ShellAt(path.clone()), cx));
    cx.run_until_parked();
    let (tab, count, id) = view.read_with(cx, |w, _| (w.tab, w.terminals.len(), w.terminals[0].id));
    view.update(cx, |w, cx| w.run(Command::CloseTerminal(id), cx));

    // Then
    assert_eq!(tab, Tab::Terminal);
    assert_eq!(count, 1);
    assert!(view.read_with(cx, |w, _| w.terminals.is_empty()));
}

#[gpui::test]
fn unknown_ghostty_theme_shows_an_error(cx: &mut TestAppContext) {
    let _guard = serial();
    // Given
    let (view, cx) = open(cx, "");

    // When
    view.update(cx, |w, cx| {
        w.run(Command::SetColors(Some("No Such Theme".into())), cx)
    });

    // Then
    let toast = view.read_with(cx, |w, _| {
        w.toast.as_ref().map(|t| (t.error, t.message.to_string()))
    });
    let (error, message) = toast.expect("a toast");
    assert!(error);
    assert!(message.contains("No Such Theme"));
}
