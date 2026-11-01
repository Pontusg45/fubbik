//! fubbik desktop: a GPUI app for keeping an eye on registered repos —
//! their processes, databases, git state and fubbik knowledge.

mod activity;
mod agent_task;
mod backend;
mod claude_md;
mod config;
mod deps;
mod detect;
mod fubbik;
mod gaps;
mod ghostty;
mod git;
mod github;
mod global_hotkey;
mod launch;
mod mcp;
mod model;
mod poll;
mod probe;
mod scripts;
mod services;
mod sessions;
mod setup;
mod supervisor;
mod system;
mod term;
mod theme;
mod tmux;
mod tmux_events;
mod tmux_runs;
mod tools;
mod ui;
mod worktree;

use gpui::{
    App, Application, Bounds, Menu, MenuItem, TitlebarOptions, WindowBounds, WindowOptions, point,
    prelude::*, px, size,
};

use supervisor::Supervisor;
use ui::{
    CaptureNote, ChooseTheme, NextRepo, PrevRepo, Quit, QuitKeepServer, Refresh, RegisterRepo,
    ShowAgents, ShowChanges, ShowDatabases, ShowDeps, ShowHome, ShowInsights, ShowKnowledge,
    ShowLogs, ShowOverview, ShowPlans, ShowProcesses, ShowSessions, ShowSetup, ShowTerminal,
    ShowToday, ShowTools, ShowWorktrees, TogglePalette, ToggleTheme, Workspace,
};

fn main() {
    launch::adopt_login_path();
    launch::adopt_utf8_locale();
    let config = config::AppConfig::load_from(&config::path()).ok();
    let summon = config
        .as_ref()
        .map_or(Some(global_hotkey::DEFAULT.to_string()), |c| {
            c.global_hotkey().map(str::to_string)
        });
    if let Some(config) = &config {
        theme::set_translucent(config.translucent());
        if let Err(error) = theme::apply(config.ghostty_theme.as_deref()) {
            eprintln!("fubbik: {error}; using the built-in colors");
        }
    }
    // With tmux, processes and scripts can also run in sessions on the app's
    // tmux server (when you choose that), attachable from any terminal.
    let supervisor = if tmux::available() {
        Supervisor::with_runs(tmux_runs::Runs {
            server: tmux::Server::fubbik(),
            logs: backend::data_dir().join("runs"),
        })
    } else {
        Supervisor::new()
    };
    let embedded = config.as_ref().is_none_or(|c| c.embedded_backend());

    let args: Vec<String> = std::env::args().collect();
    // `fubbik-desktop --serve`: only the built-in server, no window, until
    // SIGTERM or Ctrl-C. "Quit, Keep Server Running" starts one of these.
    // `fubbik-desktop --mcp`: the app's MCP server over stdio (forwarded to
    // the running app's HTTP endpoint).
    if args.get(1).map(String::as_str) == Some("--mcp") {
        mcp::run_stdio_bridge();
        return;
    }
    if args.get(1).map(String::as_str) == Some("--serve") {
        serve_headless();
        return;
    }

    // `fubbik-desktop --probe <repo>`: print one poll of a repo without the UI.
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

    let backend = std::sync::Arc::new(std::sync::Mutex::new(
        embedded.then(|| backend::Backend::start(backend::data_dir())),
    ));
    let stop_backend = move || {
        if let Some(mut backend) = backend.lock().unwrap().take() {
            backend.stop();
        }
    };
    let stop_on_close = stop_backend.clone();

    system::start();
    mcp::set_supervisor(supervisor.clone());
    mcp::start_http();

    Application::new().run(move |cx: &mut App| {
        ui::keymap::bind_all(cx, &ui::keymap::path());
        cx.set_menus(vec![
            Menu {
                name: "fubbik".into(),
                items: vec![
                    MenuItem::action("Quit, Keep Server Running", QuitKeepServer),
                    MenuItem::action("Quit fubbik", Quit),
                ],
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
                    MenuItem::action("Today", ShowToday),
                    MenuItem::action("All Agents", ShowAgents),
                    MenuItem::action("Insights", ShowInsights),
                    MenuItem::action("Changes", ShowChanges),
                    MenuItem::action("Tools", ShowTools),
                    MenuItem::action("Dependencies", ShowDeps),
                    MenuItem::action("Home", ShowHome),
                    MenuItem::action("Capture Note…", CaptureNote),
                ],
            },
        ]);

        let keep_quit = supervisor.clone();
        let stop_for_keep = stop_backend.clone();
        cx.on_action(move |_: &QuitKeepServer, cx| {
            keep_quit.stop_all();
            // Hand over: stop ours (freeing :3000 and the data folder), then a
            // background copy of this app serves until stopped from Setup.
            let ours = matches!(backend::status(), backend::Status::Running { .. });
            stop_for_keep();
            if ours && let Err(error) = backend::spawn_background() {
                eprintln!("fubbik: could not keep the server running: {error}");
            }
            cx.quit();
        });

        let on_quit = supervisor.clone();
        cx.on_action(move |_: &Quit, cx| {
            on_quit.stop_all();
            stop_backend();
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
                stop_on_close();
                cx.quit();
            }
        })
        .detach();
        if let Some(keys) = summon {
            match global_hotkey::register(&keys) {
                // Brings the app forward from anywhere, or hides it when it
                // is already in front.
                Ok(presses) => cx
                    .spawn(async move |cx| {
                        while presses.recv().await.is_ok() {
                            let _ = cx.update(|cx| {
                                if cx.active_window().is_some() {
                                    cx.hide();
                                } else {
                                    cx.activate(true);
                                }
                            });
                        }
                    })
                    .detach(),
                Err(error) => eprintln!("fubbik: summon key: {error}"),
            }
        }
        cx.activate(true);
    });
}

/// `--serve`: the built-in server alone, stopped by SIGTERM or Ctrl-C.
fn serve_headless() {
    let mut server = backend::Backend::start(backend::data_dir());
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    runtime.block_on(async {
        let mut terminate =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                .expect("SIGTERM handler");
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {},
            _ = terminate.recv() => {},
        }
    });
    server.stop();
}
