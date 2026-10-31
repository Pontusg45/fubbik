//! fubbik desktop: a GPUI app for keeping an eye on registered repos —
//! their processes, databases, git state and fubbik knowledge.

mod agent_task;
mod claude_md;
mod config;
mod detect;
mod fubbik;
mod gaps;
mod ghostty;
mod git;
mod launch;
mod model;
mod poll;
mod probe;
mod services;
mod sessions;
mod setup;
mod supervisor;
mod term;
mod theme;
mod tmux;
mod ui;
mod worktree;

use gpui::{
    App, Application, Bounds, KeyBinding, Menu, MenuItem, TitlebarOptions, WindowBounds,
    WindowOptions, point, prelude::*, px, size,
};

use supervisor::Supervisor;
use ui::{
    ChooseTheme, NextRepo, PrevRepo, Quit, Refresh, RegisterRepo, ShowAgents, ShowDatabases,
    ShowKnowledge, ShowLogs, ShowOverview, ShowPlans, ShowProcesses, ShowSessions, ShowSetup,
    ShowTerminal, ShowWorktrees, TogglePalette, ToggleTheme, Workspace,
};

fn main() {
    launch::adopt_login_path();
    if let Ok(config) = config::AppConfig::load_from(&config::path()) {
        theme::set_translucent(config.translucent());
        if let Err(error) = theme::apply(config.ghostty_theme.as_deref()) {
            eprintln!("fubbik: {error}; using the built-in colors");
        }
    }
    let supervisor = Supervisor::new();

    // `fubbik-desktop --probe <repo>`: print one poll of a repo without the UI.
    let args: Vec<String> = std::env::args().collect();
    if let [_, flag, repo] = args.as_slice()
        && flag == "--probe"
    {
        let entry = config::RepoEntry {
            path: repo.into(),
            group: None,
        };
        let mut snapshot = poll::snapshot(&entry, &supervisor, None);
        let index = std::sync::Mutex::new(sessions::SessionIndex::new());
        poll::agents(&entry, &mut snapshot, None, &index, true);
        println!("{snapshot:#?}");
        let root = std::path::Path::new(repo);
        println!(
            "setup: mcp={:?} hook={:?} claude_md={:?} gaps_files={} fubbik_bin={:?}",
            setup::mcp_status(root, &setup::claude_json()),
            setup::hook_status(root),
            claude_md::read_config(root),
            gaps::source_files(root).len(),
            setup::resolve_fubbik_bin(None, &[root.to_path_buf()]),
        );
        return;
    }

    Application::new().run(move |cx: &mut App| {
        cx.bind_keys([
            KeyBinding::new("cmd-q", Quit, None),
            KeyBinding::new("cmd-k", TogglePalette, None),
            KeyBinding::new("cmd-]", NextRepo, None),
            KeyBinding::new("cmd-[", PrevRepo, None),
            KeyBinding::new("cmd-1", ShowOverview, None),
            KeyBinding::new("cmd-2", ShowProcesses, None),
            KeyBinding::new("cmd-3", ShowDatabases, None),
            KeyBinding::new("cmd-4", ShowLogs, None),
            KeyBinding::new("cmd-5", ShowWorktrees, None),
            KeyBinding::new("cmd-6", ShowSessions, None),
            KeyBinding::new("cmd-7", ShowPlans, None),
            KeyBinding::new("cmd-8", ShowKnowledge, None),
            KeyBinding::new("cmd-9", ShowSetup, None),
            KeyBinding::new("cmd-0", ShowTerminal, None),
            KeyBinding::new("cmd-shift-a", ShowAgents, None),
            KeyBinding::new("cmd-o", RegisterRepo, None),
            KeyBinding::new("cmd-shift-t", ToggleTheme, None),
            KeyBinding::new("cmd-r", Refresh, None),
        ]);
        cx.set_menus(vec![
            Menu {
                name: "fubbik".into(),
                items: vec![MenuItem::action("Quit fubbik", Quit)],
            },
            Menu {
                name: "Repo".into(),
                items: vec![
                    MenuItem::action("Register Repo…", RegisterRepo),
                    MenuItem::action("Refresh", Refresh),
                    MenuItem::separator(),
                    MenuItem::action("Next Repo", NextRepo),
                    MenuItem::action("Previous Repo", PrevRepo),
                ],
            },
            Menu {
                name: "View".into(),
                items: vec![
                    MenuItem::action("Command Palette", TogglePalette),
                    MenuItem::action("Choose Theme…", ChooseTheme),
                    MenuItem::action("Toggle Translucent Theme", ToggleTheme),
                    MenuItem::separator(),
                    MenuItem::action("Overview", ShowOverview),
                    MenuItem::action("Processes", ShowProcesses),
                    MenuItem::action("Databases", ShowDatabases),
                    MenuItem::action("Logs", ShowLogs),
                    MenuItem::action("Worktrees", ShowWorktrees),
                    MenuItem::action("AI Sessions", ShowSessions),
                    MenuItem::action("Plans", ShowPlans),
                    MenuItem::action("Knowledge", ShowKnowledge),
                    MenuItem::action("Setup", ShowSetup),
                    MenuItem::action("Terminal", ShowTerminal),
                    MenuItem::action("All Agents", ShowAgents),
                ],
            },
        ]);

        let on_quit = supervisor.clone();
        cx.on_action(move |_: &Quit, cx| {
            on_quit.stop_all();
            cx.quit();
        });

        let bounds = Bounds::centered(None, size(px(1240.), px(800.)), cx);
        let for_window = supervisor.clone();
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some("fubbik".into()),
                    appears_transparent: true,
                    traffic_light_position: Some(point(px(12.), px(8.))),
                }),
                window_background: theme::window_background(),
                ..Default::default()
            },
            move |window, cx| cx.new(|cx| Workspace::new(for_window, window, cx)),
        )
        .expect("failed to open the main window");

        let on_close = supervisor.clone();
        cx.on_window_closed(move |cx| {
            if cx.windows().is_empty() {
                on_close.stop_all();
                cx.quit();
            }
        })
        .detach();
        cx.activate(true);
    });
}
