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

#[gpui::test]
fn knowledge_backs_off_while_the_api_is_down(cx: &mut TestAppContext) {
    let _guard = serial();
    // Given
    let (view, cx) = open(cx, "");
    let down = |w: &mut Workspace| {
        w.global = Some(GlobalStatus {
            api: Err("down".into()),
            fubbik_bin: None,
        });
        w.note_api_status();
        (w.api_backoff, w.knowledge_paused_until.is_some())
    };

    // (The window's own first poll may already have found the API down.)
    view.update(cx, |w, _| {
        w.api_backoff = Duration::ZERO;
        w.knowledge_paused_until = None;
    });

    // When
    let first = view.update(cx, |w, _| down(w));
    let second = view.update(cx, |w, _| down(w));
    let recovered = view.update(cx, |w, _| {
        w.global = Some(GlobalStatus {
            api: Ok((crate::fubbik::ApiHealth::default(), Vec::new())),
            fubbik_bin: None,
        });
        w.note_api_status();
        (w.api_backoff, w.knowledge_paused_until.is_some())
    });

    // Then
    assert_eq!(first, (Duration::from_secs(60), true));
    assert_eq!(second, (Duration::from_secs(120), true));
    assert_eq!(recovered, (Duration::ZERO, false));
}

#[gpui::test]
fn a_new_services_file_is_picked_up_without_waiting_for_the_tick(cx: &mut TestAppContext) {
    let _guard = serial();
    // Given: a registered repo without a services file, after the first poll
    let repo = tempfile::tempdir().unwrap();
    let path = repo.path().canonicalize().unwrap();
    let (view, cx) = open(
        cx,
        &format!("[[repo]]\npath = {:?}\n", path.display().to_string()),
    );
    let before = view.read_with(cx, |w, _| {
        w.repos[0].snapshot.as_ref().map(|s| s.has_services_file)
    });

    // When: the file appears; only the watcher (not the 1 s tick) can notice
    std::fs::write(
        path.join(crate::services::FILE_NAME),
        "[[process]]\nname = \"web\"\ncommand = \"true\"\n",
    )
    .unwrap();
    let mut processes = Vec::new();
    let mut advanced = Duration::ZERO;
    for _ in 0..40 {
        std::thread::sleep(Duration::from_millis(100)); // real time for the FS event
        if advanced < WATCH_DEBOUNCE + Duration::from_millis(100) {
            // Past the debounce, never as far as the first 1 s tick.
            cx.executor().advance_clock(Duration::from_millis(100));
            advanced += Duration::from_millis(100);
        }
        cx.run_until_parked();
        processes = view.read_with(cx, |w, _| {
            w.repos[0]
                .snapshot
                .as_ref()
                .map(|s| s.processes.iter().map(|p| p.spec.name.clone()).collect())
                .unwrap_or_default()
        });
        if !processes.is_empty() {
            break;
        }
    }

    // Then
    assert_eq!(before, Some(false));
    assert_eq!(processes, ["web"]);
    assert!(
        advanced < TICK,
        "found by the watcher, not the tick ({advanced:?})"
    );
}

#[gpui::test]
fn typing_reaches_the_shell_through_the_input_handler(cx: &mut TestAppContext) {
    let _guard = serial();
    // Given: a shell tab, focused
    let repo = tempfile::tempdir().unwrap();
    let path = repo.path().to_path_buf();
    let (view, cx) = open(
        cx,
        &format!("[[repo]]\npath = {:?}\n", path.display().to_string()),
    );
    view.update(cx, |w, cx| w.run(Command::ShellAt(path.clone()), cx));
    cx.run_until_parked();
    let terminal = view.read_with(cx, |w, _| w.terminals[0].view.clone());

    // When: printable keys (input handler) and enter (key encoder)
    cx.simulate_keystrokes("e c h o space t y p e d - $ ( ( 4 0 + 2 ) ) enter");
    let mut screen = String::new();
    for _ in 0..100 {
        std::thread::sleep(Duration::from_millis(50));
        cx.run_until_parked();
        screen = terminal.update(cx, |t, _| t.screen_text());
        if screen.contains("typed-42") {
            break;
        }
    }

    // Then: each key arrived once, in order, and the shell ran it
    assert!(screen.contains("echo typed-$((40+2))"), "screen:\n{screen}");
    assert!(
        screen.lines().any(|l| l.trim() == "typed-42"),
        "screen:\n{screen}"
    );
}

#[gpui::test]
fn cmd_f_searches_the_scrollback(cx: &mut TestAppContext) {
    let _guard = serial();
    // Given: a shell that printed far more than one screen
    let repo = tempfile::tempdir().unwrap();
    let path = repo.path().to_path_buf();
    let (view, cx) = open(
        cx,
        &format!("[[repo]]\npath = {:?}\n", path.display().to_string()),
    );
    view.update(cx, |w, cx| w.run(Command::ShellAt(path.clone()), cx));
    cx.run_until_parked();
    let terminal = view.read_with(cx, |w, _| w.terminals[0].view.clone());
    cx.simulate_keystrokes("s e q space 3 0 0 enter");
    for _ in 0..100 {
        std::thread::sleep(Duration::from_millis(50));
        cx.run_until_parked();
        if terminal
            .update(cx, |t, _| t.screen_text())
            .lines()
            .any(|l| l.trim() == "300")
        {
            break;
        }
    }

    // When
    cx.simulate_keystrokes("cmd-f 1 5 7");
    let status = terminal.read_with(cx, |t, _| t.search_status());
    cx.simulate_keystrokes("escape");

    // Then: "157" appears once (seq prints 1..300)
    assert_eq!(status, Some(("157".to_string(), 1)));
    assert_eq!(terminal.read_with(cx, |t, _| t.search_status()), None);
}

#[gpui::test]
fn all_agents_tab_and_clickable_alerts(cx: &mut TestAppContext) {
    let _guard = serial();
    // Given: two repos, the first selected
    let (a, b) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let config = format!(
        "[[repo]]\npath = {:?}\n\n[[repo]]\npath = {:?}\n",
        a.path().display().to_string(),
        b.path().display().to_string()
    );
    let (view, cx) = open(cx, &config);

    // When: ⇧⌘A's action, then an alert about the second repo is acted on
    cx.dispatch_action(ShowAgents);
    cx.run_until_parked();
    let on_agents = view.read_with(cx, |w, _| w.tab);
    view.update(cx, |w, cx| {
        w.show_alert(Alert {
            message: "web stopped".into(),
            target: Command::Focus {
                repo: 1,
                tab: Tab::Processes,
            },
        });
        let action = w.toast.as_ref().and_then(|t| t.action.clone()).unwrap();
        w.run(action, cx);
    });

    // Then
    assert_eq!(on_agents, Tab::Agents);
    assert_eq!(
        view.read_with(cx, |w, _| (w.selected, w.tab)),
        (1, Tab::Processes)
    );
}

#[gpui::test]
fn preferences_save_to_the_config(cx: &mut TestAppContext) {
    let _guard = serial();
    // Given
    let (view, cx) = open(cx, "");

    // When
    view.update(cx, |w, cx| {
        w.fubbik_url_field.text = "http://127.0.0.1:4000".into();
        w.submit_preference(Field::FubbikUrl, cx);
        w.run(Command::SetPersistence(true), cx);
        w.run(Command::SetExternalTerminal(true), cx);
    });
    let saved = std::fs::read_to_string(config_file()).unwrap();
    view.update(cx, |w, cx| w.run(Command::SetPersistence(false), cx));
    let off = std::fs::read_to_string(config_file()).unwrap();

    // Then
    assert!(
        saved.contains("fubbik_url = \"http://127.0.0.1:4000\""),
        "{saved}"
    );
    assert!(
        !saved.contains("terminal_persistence"),
        "on is the default, so it is not written: {saved}"
    );
    assert!(saved.contains("terminal = \"external\""), "{saved}");
    assert!(off.contains("terminal_persistence = \"none\""), "{off}");
}

#[gpui::test]
fn j_and_k_move_through_ai_sessions(cx: &mut TestAppContext) {
    let _guard = serial();
    // Given: a repo whose snapshot has three sessions, on AI Sessions
    let repo = tempfile::tempdir().unwrap();
    let (view, cx) = open(
        cx,
        &format!("[[repo]]\npath = {:?}\n", repo.path().display().to_string()),
    );
    view.update(cx, |w, cx| {
        let session = |id: &str| crate::sessions::AiSession {
            agent: crate::sessions::Agent::ClaudeCode,
            id: id.into(),
            title: id.into(),
            cwd: repo.path().to_path_buf(),
            branch: None,
            state: crate::sessions::SessionState::Ended,
            pid: None,
            last_activity: std::time::SystemTime::now(),
            pane: None,
            preview: None,
        };
        let mut snapshot = w.repos[0].snapshot.clone().unwrap_or_default();
        snapshot.sessions = vec![session("a"), session("b"), session("c")];
        w.repos[0].snapshot = Some(snapshot);
        w.run(Command::ShowTab(Tab::Sessions), cx);
    });
    cx.run_until_parked();

    // When
    cx.simulate_keystrokes("j j j j k");

    // Then: three rows, so j stops at the last; k goes back one
    assert_eq!(view.read_with(cx, |w, _| w.list_cursor), Some(1));
}

#[gpui::test]
fn first_launch_shows_the_checklist(cx: &mut TestAppContext) {
    let _guard = serial();
    // Given / When
    let (view, cx) = open(cx, "");

    // Then: it renders with no repos, and preferences are populated from the config
    assert!(view.read_with(cx, |w, _| w.repos.is_empty()));
    assert_eq!(
        view.read_with(cx, |w, _| w.fubbik_url_field.text.clone()),
        ""
    );
}

#[gpui::test]
fn split_opens_a_second_shell_and_unsplit_keeps_it(cx: &mut TestAppContext) {
    let _guard = serial();
    // Given: one shell tab
    let repo = tempfile::tempdir().unwrap();
    let path = repo.path().to_path_buf();
    let (view, cx) = open(
        cx,
        &format!("[[repo]]\npath = {:?}\n", path.display().to_string()),
    );
    view.update(cx, |w, cx| w.run(Command::ShellAt(path.clone()), cx));
    cx.run_until_parked();

    // When
    view.update(cx, |w, cx| w.run(Command::ToggleSplit, cx));
    cx.run_until_parked();
    let split = view.read_with(cx, |w, _| {
        (w.terminals.len(), w.split_with.is_some(), w.active_terminal)
    });
    view.update(cx, |w, cx| w.run(Command::ToggleSplit, cx));

    // Then: two shells while split; unsplit hides, not closes; the active one stays
    assert_eq!((split.0, split.1), (2, true));
    assert_eq!(
        view.read_with(cx, |w, _| (
            w.terminals.len(),
            w.split_with,
            w.active_terminal
        )),
        (2, None, split.2)
    );
}
