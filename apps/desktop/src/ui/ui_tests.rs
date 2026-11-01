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
            tokens: None,
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

fn git(repo: &std::path::Path, args: &[&str]) {
    assert!(
        std::process::Command::new("git")
            .arg("-C")
            .arg(repo)
            .args(args)
            .status()
            .unwrap()
            .success(),
        "git {args:?}"
    );
}

fn commit(repo: &std::path::Path, msg: &str) {
    git(
        repo,
        &[
            "-c",
            "user.email=t@t",
            "-c",
            "user.name=t",
            "commit",
            "-q",
            "--allow-empty",
            "-m",
            msg,
        ],
    );
}

/// A plan with one task per status (b waits on a), for board and Today tests.
fn plan_with_tasks() -> crate::fubbik::PlanDetail {
    let task = |id: &str, status: &str, deps: &[&str]| crate::fubbik::Task {
        id: id.into(),
        title: id.to_uppercase(),
        status: status.into(),
        depends_on: deps.iter().map(|d| d.to_string()).collect(),
        acceptance: vec![("works".into(), false)],
        ..Default::default()
    };
    crate::fubbik::PlanDetail {
        id: "p1".into(),
        title: "Desktop".into(),
        status: "in_progress".into(),
        tasks: vec![
            task("a", "in_progress", &[]),
            task("b", "pending", &["a"]),
            task("c", "pending", &[]),
            task("d", "done", &[]),
        ],
    }
}

fn with_knowledge(w: &mut Workspace, plan: crate::fubbik::PlanDetail) {
    w.repos[0].knowledge = Some(Ok(crate::fubbik::Knowledge {
        space: Some(crate::fubbik::Space {
            id: "s1".into(),
            name: "app".into(),
        }),
        plan_details: vec![plan],
        ..Default::default()
    }));
}

#[gpui::test]
fn merged_task_branch_closes_the_task_and_removes_its_worktree(cx: &mut TestAppContext) {
    let _guard = serial();
    // Given: a task branch in its own worktree, with work merged into main
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().join("app");
    std::fs::create_dir(&repo).unwrap();
    git(&repo, &["init", "-q", "-b", "main"]);
    commit(&repo, "base");
    let base = crate::worktree::head(&repo).unwrap();
    let wt = crate::worktree::create(&repo, "task/a").unwrap();
    commit(&wt, "the work");
    git(
        &repo,
        &[
            "-c",
            "user.email=t@t",
            "-c",
            "user.name=t",
            "merge",
            "-q",
            "--no-ff",
            "-m",
            "merge",
            "task/a",
        ],
    );
    let (server, log) =
        crate::fubbik::test_server::fake_server(vec![("/api/plans/p1/tasks/a", 200, "{}".into())]);
    let (view, cx) = open(
        cx,
        &format!(
            "fubbik_url = {server:?}\n[[repo]]\npath = {:?}\n",
            repo.display().to_string()
        ),
    );
    view.update(cx, |w, _| {
        let mut plan = plan_with_tasks();
        plan.tasks[0].link = crate::fubbik::TaskLink {
            branch: Some("task/a".into()),
            base_commit: Some(base.clone()),
            worktree: Some(wt.display().to_string()),
        };
        with_knowledge(w, plan);
        let mut snapshot = w.repos[0].snapshot.clone().unwrap_or_default();
        snapshot.worktrees = Some(crate::worktree::read(&repo));
        w.repos[0].snapshot = Some(snapshot);
    });

    // When
    view.update(cx, |w, cx| w.close_merged_tasks(cx));
    cx.run_until_parked();

    // Then: marked done through the API, worktree gone, a note offered
    let patched = log
        .lock()
        .unwrap()
        .iter()
        .any(|r| r == "PATCH /api/plans/p1/tasks/a");
    assert!(patched, "requests: {:?}", log.lock().unwrap());
    assert!(!wt.exists(), "the clean, merged worktree is removed");
    let action = view.read_with(cx, |w, _| w.toast.as_ref().and_then(|t| t.action.clone()));
    assert!(
        matches!(action, Some(Command::SaveTaskNote { .. })),
        "{action:?}"
    );
}

#[gpui::test]
fn agents_beyond_the_limit_wait_in_the_queue(cx: &mut TestAppContext) {
    let _guard = serial();
    // Given: room for one agent
    let repo = tempfile::tempdir().unwrap();
    let path = repo.path().to_path_buf();
    let (view, cx) = open(
        cx,
        &format!(
            "max_agents = 1\n[[repo]]\npath = {:?}\n",
            path.display().to_string()
        ),
    );

    // When: two launches ("true" stands in for the agent command)
    view.update(cx, |w, cx| {
        w.launch_agent(path.clone(), "first".into(), "true".into(), false, cx);
        w.launch_agent(path.clone(), "second".into(), "true".into(), false, cx);
    });
    let (tabs, queued) = view.read_with(cx, |w, _| (w.terminals.len(), w.launch_queue.len()));
    // The first agent's slot frees (it ended), so the queue drains.
    view.update(cx, |w, cx| {
        w.recent_launches.clear();
        w.drain_launch_queue(cx);
    });

    // Then
    assert_eq!((tabs, queued), (1, 1));
    assert_eq!(
        view.read_with(cx, |w, _| (w.terminals.len(), w.launch_queue.len())),
        (2, 0)
    );
}

#[gpui::test]
fn board_moves_and_checklist_ticks_reach_the_api(cx: &mut TestAppContext) {
    let _guard = serial();
    // Given
    let repo = tempfile::tempdir().unwrap();
    let (server, log) =
        crate::fubbik::test_server::fake_server(vec![("/api/plans/p1/tasks/c", 200, "{}".into())]);
    let (view, cx) = open(
        cx,
        &format!(
            "fubbik_url = {server:?}\n[[repo]]\npath = {:?}\n",
            repo.path().display().to_string()
        ),
    );
    view.update(cx, |w, cx| {
        with_knowledge(w, plan_with_tasks());
        w.run(Command::ShowTab(Tab::Plans), cx);
    });
    cx.run_until_parked();

    // When
    view.update(cx, |w, cx| {
        w.run(
            Command::SetTaskStatus {
                plan: "p1".into(),
                task: "c".into(),
                status: "in_progress",
            },
            cx,
        );
        w.run(
            Command::ToggleCriterion {
                repo: 0,
                plan: "p1".into(),
                task: "c".into(),
                index: 0,
            },
            cx,
        );
    });
    let ticked = view.read_with(cx, |w, _| {
        w.find_task(0, "p1", "c").map(|(_, t)| t.acceptance[0].1)
    });
    cx.run_until_parked();

    // Then: shown at once, and both writes sent
    assert_eq!(ticked, Some(true));
    assert_eq!(
        log.lock()
            .unwrap()
            .iter()
            .filter(|r| *r == "PATCH /api/plans/p1/tasks/c")
            .count(),
        2
    );
}

#[gpui::test]
fn today_insights_and_capture_render(cx: &mut TestAppContext) {
    let _guard = serial();
    // Given: a repo with a plan
    let repo = tempfile::tempdir().unwrap();
    git(repo.path(), &["init", "-q", "-b", "main"]);
    commit(repo.path(), "base");
    let (view, cx) = open(
        cx,
        &format!("[[repo]]\npath = {:?}\n", repo.path().display().to_string()),
    );
    view.update(cx, |w, _| with_knowledge(w, plan_with_tasks()));

    // When: each view renders (a panic would fail the test)
    let initial = view.read_with(cx, |w, _| w.tab);
    cx.dispatch_action(ShowInsights);
    cx.run_until_parked();
    let insights = view.read_with(cx, |w, _| {
        matches!(w.repos[0].insights, Some(Loadable::Done(Ok(_))))
    });
    cx.dispatch_action(ShowToday);
    cx.run_until_parked();
    cx.dispatch_action(CaptureNote);
    cx.run_until_parked();
    let capturing = view.read_with(cx, |w, _| w.capture_open);
    cx.simulate_keystrokes("escape");

    // Then
    assert_eq!(
        initial,
        Tab::Today,
        "Today is the home screen once repos exist"
    );
    assert!(insights, "insights were computed in the background");
    assert!(capturing);
    assert!(!view.read_with(cx, |w, _| w.capture_open));
}

#[gpui::test]
fn task_timeline_lists_claims_and_board_notes_in_order(cx: &mut TestAppContext) {
    let _guard = serial();
    // Given: a board with a claim and a progress note for task "a"
    let repo = tempfile::tempdir().unwrap();
    let board = serde_json::json!({
        "plan": {"id": "p1", "title": "Desktop"},
        "tasks": [], "runs": [{"id": "r1", "handle": "claude-1", "status": "active", "metadata": {}}],
        "claims": [{"taskId": "a", "agentRunId": "r1", "claimedAt": "2026-10-31T10:00:00Z", "expired": false}],
        "entries": [{"taskId": "a", "kind": "progress", "body": "half way\nmore", "createdAt": "2026-10-31T11:30:00Z"}]
    });
    let (server, _) = crate::fubbik::test_server::fake_server(vec![(
        "/api/plans/p1/board",
        200,
        board.to_string(),
    )]);
    let (view, cx) = open(
        cx,
        &format!(
            "fubbik_url = {server:?}\n[[repo]]\npath = {:?}\n",
            repo.path().display().to_string()
        ),
    );
    view.update(cx, |w, _| {
        let mut plan = plan_with_tasks();
        plan.tasks[0].created_at = Some("2026-10-30T09:00:00Z".into());
        with_knowledge(w, plan);
    });

    // When
    view.update(cx, |w, cx| {
        w.run(
            Command::TaskTimeline {
                repo: 0,
                plan: "p1".into(),
                task: "a".into(),
            },
            cx,
        )
    });
    cx.run_until_parked();

    // Then
    let text = view.read_with(cx, |w, _| match w.panel.as_ref().map(|p| &p.body) {
        Some(Loadable::Done(Ok(text))) => text.clone(),
        other => format!("{other:?}"),
    });
    let created = text.find("created").expect(&text);
    let claimed = text.find("claimed by claude-1").expect(&text);
    let note = text.find("progress: half way").expect(&text);
    assert!(created < claimed && claimed < note, "{text}");
    assert!(text.contains("2026-10-31 10:00"), "{text}");
}

#[gpui::test]
fn keys_from_the_keymap_drive_the_app(cx: &mut TestAppContext) {
    let _guard = serial();
    // Given: two repos, the real keymap, and one repo with stale knowledge
    cx.update(|cx| keymap::bind_all(cx, std::path::Path::new("/nonexistent/keymap.toml")));
    let (a, b) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let config = format!(
        "[[repo]]\npath = {:?}\n\n[[repo]]\npath = {:?}\n",
        a.path().display().to_string(),
        b.path().display().to_string()
    );
    let (view, cx) = open(cx, &config);
    view.update(cx, |w, _| {
        w.repos[1].knowledge = Some(Ok(crate::fubbik::Knowledge {
            space: Some(crate::fubbik::Space {
                id: "s".into(),
                name: "b".into(),
            }),
            stale: 2,
            ..Default::default()
        }));
    });
    let state = |view: &Entity<Workspace>, cx: &mut VisualTestContext| {
        view.read_with(cx, |w, _| (w.tab, w.selected))
    };

    // When / Then
    cx.simulate_keystrokes("cmd-,");
    assert_eq!(state(&view, cx).0, Tab::Setup);
    cx.simulate_keystrokes("cmd-1");
    assert_eq!(state(&view, cx).0, Tab::Today);
    cx.simulate_keystrokes("ctrl-2");
    assert_eq!(state(&view, cx).1, 1);
    cx.simulate_keystrokes("cmd-/");
    assert!(view.read_with(cx, |w, _| w.cheat_sheet_open));
    cx.simulate_keystrokes("escape");
    assert!(!view.read_with(cx, |w, _| w.cheat_sheet_open));
    view.update(cx, |w, _| w.select(0));
    cx.simulate_keystrokes("cmd-shift-j");
    assert_eq!(
        state(&view, cx),
        (Tab::Knowledge, 1),
        "jumped to the repo with stale knowledge"
    );
}

#[gpui::test]
fn list_keys_drive_the_board_today_sidebar_and_inbox(cx: &mut TestAppContext) {
    let _guard = serial();
    // Given: two repos; the first has a plan, a stale flag and a fake fubbik
    cx.update(|cx| keymap::bind_all(cx, std::path::Path::new("/nonexistent/keymap.toml")));
    let (a, b) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let (server, log) = crate::fubbik::test_server::fake_server(vec![
        ("/api/plans/p1/tasks/a", 200, "{}".into()),
        ("/api/plans/p1/tasks/c", 200, "{}".into()),
        ("/api/chunks/f1/dismiss-staleness", 200, "{}".into()),
    ]);
    let config = format!(
        "fubbik_url = {server:?}\n[[repo]]\npath = {:?}\n\n[[repo]]\npath = {:?}\n",
        a.path().display().to_string(),
        b.path().display().to_string()
    );
    let (view, cx) = open(cx, &config);
    view.update(cx, |w, _| with_knowledge(w, plan_with_tasks()));

    // When: board — first card of Pending is "b"; l → In progress ("a"); → moves "a" to done
    cx.simulate_keystrokes("cmd-7");
    cx.simulate_keystrokes("j l right");
    let cursor = view.read_with(cx, |w, _| w.board_cursor);
    // Knowledge inbox (after the board's refresh settles): the stale flag is row 0; d dismisses it
    cx.run_until_parked();
    view.update(cx, |w, _| {
        with_knowledge(w, plan_with_tasks());
        if let Some(Ok(k)) = w.repos[0].knowledge.as_mut() {
            k.stale_flags = vec![crate::fubbik::StaleFlag {
                id: "f1".into(),
                chunk_id: "c1".into(),
                chunk_title: "Old".into(),
                reason: "age".into(),
                detail: None,
            }];
        }
    });
    cx.simulate_keystrokes("cmd-8 j d");
    // Sidebar: ⇧⌘E, j selects repo 2, esc leaves
    cx.simulate_keystrokes("cmd-shift-e j escape");
    let (selected, nav) = view.read_with(cx, |w, _| (w.selected, w.sidebar_nav));
    cx.run_until_parked();

    // Then
    assert_eq!(cursor, Some((1, 0)), "on the In progress column");
    let log = log.lock().unwrap();
    assert!(
        log.iter().any(|r| r == "PATCH /api/plans/p1/tasks/a"),
        "{log:?}"
    );
    assert!(
        log.iter()
            .any(|r| r == "POST /api/chunks/f1/dismiss-staleness"),
        "{log:?}"
    );
    assert_eq!((selected, nav), (1, false));
}

#[gpui::test]
fn enter_on_today_runs_the_highlighted_line(cx: &mut TestAppContext) {
    let _guard = serial();
    // Given: Today lists the repo's stale knowledge under "Needs you"
    cx.update(|cx| keymap::bind_all(cx, std::path::Path::new("/nonexistent/keymap.toml")));
    let repo = tempfile::tempdir().unwrap();
    let (view, cx) = open(
        cx,
        &format!("[[repo]]\npath = {:?}\n", repo.path().display().to_string()),
    );
    view.update(cx, |w, _| {
        w.repos[0].knowledge = Some(Ok(crate::fubbik::Knowledge {
            space: Some(crate::fubbik::Space {
                id: "s".into(),
                name: "a".into(),
            }),
            stale: 1,
            ..Default::default()
        }));
    });

    // When
    cx.simulate_keystrokes("cmd-1 j enter");

    // Then
    assert_eq!(view.read_with(cx, |w, _| w.tab), Tab::Knowledge);
}

#[gpui::test]
fn palette_prefixes_pick_what_is_listed_and_recent_comes_first(cx: &mut TestAppContext) {
    let _guard = serial();
    // Given: one repo with a plan
    let repo = tempfile::tempdir().unwrap();
    let (view, cx) = open(
        cx,
        &format!("[[repo]]\npath = {:?}\n", repo.path().display().to_string()),
    );
    view.update(cx, |w, _| with_knowledge(w, plan_with_tasks()));
    let listed = |cx: &mut gpui::VisualTestContext, text: &str| {
        view.update(cx, |w, _| {
            w.palette_field.text = text.into();
            w.palette_matches()
                .into_iter()
                .map(|e| e.label)
                .collect::<Vec<_>>()
        })
    };

    // When
    let tasks = listed(cx, "#");
    let repos = listed(cx, "@");
    let commands = listed(cx, ">tod");
    let running = listed(cx, "!");
    view.update(cx, |w, cx| w.run_from_palette(Command::ToggleTheme, cx));
    let first = listed(cx, "").into_iter().next();
    view.update(cx, |w, cx| w.run_from_palette(Command::ToggleTheme, cx));

    // Then: open tasks only (not "d", which is done), repo lines, commands
    assert_eq!(tasks.len(), 3, "{tasks:?}");
    assert!(tasks.iter().all(|t| t.starts_with('#')), "{tasks:?}");
    assert!(
        repos.iter().all(|r| !r.starts_with("Start")) && !repos.is_empty(),
        "{repos:?}"
    );
    assert_eq!(commands.first().map(String::as_str), Some("Today"));
    assert!(running.is_empty(), "{running:?}");
    assert!(
        first.is_some_and(|f| f.starts_with("Theme: switch")),
        "recent first"
    );
}
