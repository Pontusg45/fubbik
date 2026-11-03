//! The Setup tab: `fubbik doctor` and more — the server, the Rust binary,
//! the MCP server in Claude Code's config, and each worktree's pre-commit
//! hook and generated CLAUDE.md.

use gpui::{AnyElement, Context, Hsla, IntoElement, SharedString, div, prelude::*, px};

use super::widgets::{button, muted, pill, row, section, section_or};
use super::{Workspace, display_path};
use crate::claude_md::MdState;
use crate::model::Status;
use crate::setup::{self, HookStatus, McpStatus};
use crate::theme;

fn tone(ok: bool) -> Hsla {
    if ok {
        theme::status(Status::Running)
    } else {
        theme::status(Status::Degraded)
    }
}

fn check(title: &str, ok: bool, detail: String) -> gpui::Div {
    row()
        .child(
            div()
                .w(px(70.))
                .child(pill(if ok { "ok" } else { "fix" }, tone(ok))),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .flex_1()
                .overflow_hidden()
                .child(div().text_sm().child(title.to_string()))
                .child(muted(detail)),
        )
}

impl Workspace {
    pub(super) fn render_setup(&self, repo: usize, cx: &mut Context<Self>) -> impl IntoElement {
        let view = &self.repos[repo];

        let server_ok = matches!(
            self.global.as_ref().map(|g| &g.api),
            Some(Ok((health, _))) if health.status == "ok" && health.db == "connected"
        );
        let setup_state = view.setup.as_ref();
        let mcp = setup_state.map(|s| s.mcp.clone());
        let mut mcp_row = check(
            "fubbik MCP server for Claude Code",
            matches!(mcp, Some(McpStatus::Project | McpStatus::User)),
            match mcp {
                Some(McpStatus::Project) => "in this repo's .mcp.json".into(),
                Some(McpStatus::User) => "in ~/.claude.json".into(),
                Some(McpStatus::Missing) => {
                    "missing: sessions here cannot use join_board, get_context, …".into()
                }
                None => "checking…".into(),
            },
        );
        if mcp == Some(McpStatus::Missing) {
            mcp_row = mcp_row.child(
                button("install-mcp", "Add to .mcp.json")
                    .on_click(cx.listener(move |this, _, _, cx| this.install_mcp(repo, cx))),
            );
        }

        // ── per worktree ──
        let mut worktree_rows: Vec<AnyElement> = Vec::new();
        if let Some(state) = setup_state {
            for (i, (root, hook)) in state.hooks.iter().enumerate() {
                let id = |n: &str| SharedString::from(format!("setup-{n}-{i}"));
                let md = state.claude_md.as_ref().and_then(|sync| {
                    sync.worktrees
                        .iter()
                        .find(|(r, _)| r == root)
                        .map(|(_, s)| *s)
                });
                let mut r = row()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .overflow_hidden()
                            .child(div().text_sm().child(display_path(root)))
                            .child(muted(match hook {
                                HookStatus::Installed => "pre-commit hook: fubbik check-files",
                                HookStatus::Other => {
                                    "pre-commit hook: another hook is installed (left alone)"
                                }
                                HookStatus::Missing => "pre-commit hook: none",
                            })),
                    )
                    .child(pill(
                        match hook {
                            HookStatus::Installed => "hook",
                            HookStatus::Other => "other hook",
                            HookStatus::Missing => "no hook",
                        },
                        tone(*hook == HookStatus::Installed),
                    ));
                if *hook == HookStatus::Missing {
                    let root = root.clone();
                    r = r.child(button(id("hook"), "Install hook").on_click(
                        cx.listener(move |this, _, _, cx| this.install_hook(root.clone(), cx)),
                    ));
                }
                if let Some(md) = md {
                    r = r.child(pill(
                        format!("CLAUDE.md {}", md.label()),
                        tone(md == MdState::InSync),
                    ));
                    if md != MdState::InSync {
                        let root = root.clone();
                        r = r.child(button(id("sync"), "Sync CLAUDE.md").on_click(cx.listener(
                            move |this, _, _, cx| this.sync_claude_md(repo, root.clone(), cx),
                        )));
                    }
                }
                worktree_rows.push(r.into_any_element());
            }
        }
        let claude_md_note = match setup_state.and_then(|s| s.claude_md.as_ref()) {
            None => "CLAUDE.md sync is off: add a `claudeMd` section to fubbik.config.json (output, tag, maxTokens) to turn it on.".to_string(),
            Some(sync) => match &sync.generated {
                Ok(_) => format!("CLAUDE.md: {} from chunks tagged `{}`", sync.config.output.display(), sync.config.tag),
                Err(error) => format!("CLAUDE.md could not be generated: {error}"),
            },
        };

        let env_rows = self.env_drift_rows(repo);
        div()
            .flex()
            .flex_col()
            .gap_6()
            // What is broken first.
            .child(section(
                "This repo",
                vec![mcp_row.into_any_element()],
            ))
            .when(!server_ok, |d| {
                d.child(
                    row()
                        .child(div().flex_1().text_sm().text_color(theme::warning()).child(
                            "The fubbik server is not answering: knowledge, plans and boards here are unavailable",
                        ))
                        .child(button("setup-server", "Open Settings").on_click(cx.listener(
                            |this, _, _, cx| this.run(super::palette::Command::ShowTab(super::Tab::Settings), cx),
                        ))),
                )
            })
            .child(section_or(
                "Worktrees: hooks and CLAUDE.md",
                worktree_rows,
                if setup_state.is_none() {
                    "Checking…"
                } else {
                    "No worktrees"
                },
            ))
            .child(muted(claude_md_note))
            .when(self.config.env_check, |d| {
                d.child(section_or(".env files", env_rows, "No .env.example in this repo"))
            })
            .child(
                row()
                    .child(div().flex_1().child(muted(
                        "Preferences, the server, Claude Code hooks, the projects tracker, the .env check and keys are app-wide: they are in Settings.",
                    )))
                    .child(button("setup-settings", format!("Settings {}", super::Tab::Settings.shortcut())).on_click(
                        cx.listener(|this, _, _, cx| this.run(super::palette::Command::ShowTab(super::Tab::Settings), cx)),
                    )),
            )
    }

    /// Home → Settings: what applies to every repo.
    pub(super) fn render_settings(
        &self,
        window: &gpui::Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap_6()
            .pb_6()
            .child(section("fubbik", self.server_rows(cx)))
            .child(self.render_preferences(window, cx))
            .child(self.render_app_switches(cx))
            .child(self.render_claude_hooks(cx))
            .child(self.render_env_check(cx))
            .child(self.render_tracker_setup(cx))
            .child(self.render_keyboard_setup(cx))
    }

    /// The server and the binary: checked once for the whole app.
    fn server_rows(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let global = self.global.as_ref();
        let (server_ok, server_detail) = match global.map(|g| &g.api) {
            None => (false, "checking…".to_string()),
            Some(Ok((health, features))) => (
                health.status == "ok" && health.db == "connected",
                format!(
                    "{} — {} · db {} · groups: {}",
                    self.config.fubbik_url(),
                    health.status,
                    health.db,
                    features.join(", ")
                ),
            ),
            Some(Err(error)) => (false, format!("{} — {error}", self.config.fubbik_url())),
        };
        let bin = global.and_then(|g| g.fubbik_bin.clone());
        let checkout = self
            .repos
            .iter()
            .map(|r| r.entry.path.clone())
            .find(|p| setup::is_fubbik_checkout(p));
        let mut bin_row = check(
            "Rust fubbik binary",
            bin.is_some(),
            match &bin {
                Some(path) => display_path(path),
                None => "not found. The `fubbik` on PATH may be the legacy TypeScript CLI, which has no `mcp` command. Set fubbik_bin in the desktop config, or build it.".into(),
            },
        );
        if bin.is_none()
            && let Some(checkout) = checkout
        {
            bin_row = bin_row.child(button("build-fubbik", "Build fubbik").on_click(
                cx.listener(move |this, _, _, cx| this.build_fubbik(checkout.clone(), cx)),
            ));
        }
        let mut server_row = check("fubbik server", server_ok, server_detail);
        if server_ok {
            server_row = server_row.child(button("open-web", "Open web app").on_click(
                cx.listener(|this, _, _, cx| {
                    this.run(super::palette::Command::OpenWeb("/dashboard".into()), cx)
                }),
            ));
        } else {
            // Something to do about it, not just a link.
            server_row = server_row
                .child(button("server-retry", "Retry").on_click(
                    cx.listener(|this, _, _, cx| this.run(super::palette::Command::Refresh, cx)),
                ))
                .child(button("server-logs", "Show logs").on_click(cx.listener(
                    |this, _, _, cx| {
                        this.log_key = Some("builtin::server".into());
                        this.log_paused = None;
                        this.run(super::palette::Command::ShowTab(super::Tab::Logs), cx)
                    },
                )))
                .when(!self.config.embedded_backend(), |r| {
                    r.child(button("server-on", "Turn on the built-in server").on_click(
                        cx.listener(|this, _, _, cx| {
                            this.run(super::palette::Command::SetEmbeddedBackend(true), cx)
                        }),
                    ))
                });
        }

        vec![server_row.into_any_element(), bin_row.into_any_element()]
    }

    /// Keyboard: where keymap.toml is, what is wrong in it, and clashes.
    fn render_keyboard_setup(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let path = super::keymap::path();
        let problems = super::keymap::problems();
        let mut rows: Vec<AnyElement> = vec![
            row()
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .flex_1()
                        .child(div().text_sm().child("Key bindings"))
                        .child(muted(format!(
                            "{} — changes apply on the next launch; ⌘/ lists them all",
                            display_path(&path)
                        ))),
                )
                .child(
                    button("keymap-open", "Open keymap.toml")
                        .on_click(cx.listener(|this, _, _, cx| this.open_keymap_file(cx))),
                )
                .into_any_element(),
        ];
        rows.extend(problems.iter().map(|problem| {
            row()
                .child(div().w(px(70.)).child(pill("fix", tone(false))))
                .child(div().text_sm().child(problem.clone()))
                .into_any_element()
        }));
        section("Keyboard", rows)
    }
}

impl Workspace {
    fn tracker_dir(&self) -> std::path::PathBuf {
        let paths: Vec<std::path::PathBuf> =
            self.repos.iter().map(|r| r.entry.path.clone()).collect();
        crate::setup::tracker_dir(&paths)
    }

    /// Writes the tracker files and says what it did.
    pub(super) fn setup_tracker(&mut self) {
        let dir = self.tracker_dir();
        let bin = self.global.as_ref().and_then(|g| g.fubbik_bin.clone());
        let url = self.config.fubbik_url();
        let mcp = format!("http://{}/mcp", crate::mcp::addr());
        match crate::setup::install_tracker(&dir, bin.as_deref(), &url, &mcp) {
            Ok(done) => self.show_info(format!(
                "Tracker workspace ready — start `claude` in {}. {}",
                display_path(&dir),
                done.join("; ")
            )),
            Err(error) => self.show_error(format!("Could not set up the tracker: {error}")),
        }
    }

    /// Setup → a folder where an agent keeps track of every project.
    fn render_tracker_setup(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let dir = self.tracker_dir();
        let (has_mcp, has_claude, _) = self.setup_file_checks();
        let state = match (has_mcp, has_claude) {
            (true, true) => "set up: .mcp.json has fubbik-desktop, CLAUDE.md exists",
            (true, false) => ".mcp.json is set up; no CLAUDE.md yet",
            (false, true) => "has a CLAUDE.md; .mcp.json lacks fubbik-desktop",
            (false, false) => "not set up",
        };
        section(
            "Projects tracker",
            vec![
                row()
                    .child(
                        div()
                            .flex_1()
                            .flex()
                            .flex_col()
                            .gap_0p5()
                            .child(div().text_sm().child(format!(
                                "An agent in {} that keeps track of every project",
                                display_path(&dir)
                            )))
                            .child(muted(format!(
                                "Writes .mcp.json (this app's MCP server and fubbik's) and a CLAUDE.md with the tracker's job (never overwrites one). Then run `claude` there and ask \"what changed?\" — {state}."
                            ))),
                    )
                    .child(
                        button("tracker-setup", if has_mcp { "Update" } else { "Set up" })
                            .on_click(cx.listener(|this, _, _, cx| this.run(super::palette::Command::SetupTracker, cx))),
                    )
                    .into_any_element(),
            ],
        )
    }
}

impl Workspace {
    /// Setup → Claude Code hooks: live agent events instead of polling.
    fn render_claude_hooks(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let path = crate::claude_hooks::settings_path();
        let (_, _, on) = self.setup_file_checks();
        let install = super::palette::Command::AskFirst(Box::new(super::palette::Confirm {
            title: "Add the app's hooks to Claude Code?".into(),
            detail: format!(
                "Adds SessionStart, UserPromptSubmit, Stop, Notification and SessionEnd hooks to {} next to yours (backed up to settings.json.fubbik-backup). Each pipes the event to this app with a 1-second curl that never fails, so Claude is never slowed down.",
                display_path(&path)
            ),
            action: "Add hooks".into(),
            danger: false,
            command: super::palette::Command::ClaudeHooks(true),
        }));
        section(
            "Claude Code hooks",
            vec![
                row()
                    .child(
                        div()
                            .flex_1()
                            .flex()
                            .flex_col()
                            .gap_0p5()
                            .child(div().text_sm().child(if on {
                                "Installed: sessions report start, working, waiting and end as they happen"
                            } else {
                                "Not installed: agent state comes from polling session files"
                            }))
                            .child(muted(format!(
                                "Hooks in {}; they report to http://{}/hook",
                                display_path(&path),
                                crate::mcp::addr()
                            ))),
                    )
                    .child(if on {
                        button("hooks-off", "Remove hooks").on_click(cx.listener(|this, _, _, cx| {
                            this.run(super::palette::Command::ClaudeHooks(false), cx)
                        }))
                    } else {
                        button("hooks-on", "Add hooks…").on_click(cx.listener(move |this, _, _, cx| this.run(install.clone(), cx)))
                    })
                    .into_any_element(),
            ],
        )
    }
}

impl Workspace {
    /// A repo's `.env` drift rows (key names only), re-read at most every
    /// 30 seconds. Empty when the check is off.
    fn env_drift_rows(&self, repo: usize) -> Vec<AnyElement> {
        if !self.config.env_check {
            return Vec::new();
        }
        let path = self.repos[repo].entry.path.clone();
        let drift = {
            let mut cache = self.env_drift.borrow_mut();
            let fresh = cache
                .get(&path)
                .is_some_and(|(at, _)| at.elapsed() < std::time::Duration::from_secs(30));
            if !fresh {
                cache.insert(
                    path.clone(),
                    (std::time::Instant::now(), crate::env_drift::scan(&path)),
                );
            }
            cache[&path].1.clone()
        };
        drift
            .into_iter()
            .map(|d| {
                let folder = if d.dir.is_empty() {
                    ".".to_string()
                } else {
                    d.dir.clone()
                };
                let (color, text) = if !d.has_env {
                    (
                        theme::warning(),
                        format!("no .env yet ({} is there)", d.example),
                    )
                } else if d.is_clean() {
                    (theme::text_muted(), format!("in step with {}", d.example))
                } else {
                    let mut parts = Vec::new();
                    if !d.missing.is_empty() {
                        parts.push(format!("missing {}", d.missing.join(", ")));
                    }
                    if !d.extra.is_empty() {
                        parts.push(format!("not in {}: {}", d.example, d.extra.join(", ")));
                    }
                    (
                        if d.missing.is_empty() {
                            theme::text_muted()
                        } else {
                            theme::warning()
                        },
                        parts.join(" · "),
                    )
                };
                row()
                    .child(div().w(px(160.)).flex_none().text_sm().child(folder))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_sm()
                            .text_color(color)
                            .child(text),
                    )
                    .into_any_element()
            })
            .collect()
    }

    /// Settings → .env files: the opt-in switch, then every repo's drift.
    fn render_env_check(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let on = self.config.env_check;
        let mut rows = vec![
            row()
                .child(
                    div()
                        .flex_1()
                        .flex()
                        .flex_col()
                        .gap_0p5()
                        .child(div().text_sm().child("Compare .env with .env.example"))
                        .child(muted(
                            "In every repo. Key names only: values are never read into the app, shown or sent. Off by default.",
                        )),
                )
                .child(super::widgets::switch("env-check", on).on_click(cx.listener(
                    move |this, _, _, cx| this.run(super::palette::Command::SetEnvCheck(!on), cx),
                )))
                .into_any_element(),
        ];
        if on {
            for (i, view) in self.repos.iter().enumerate() {
                let found = self.env_drift_rows(i);
                if found.is_empty() {
                    continue;
                }
                rows.push(
                    div()
                        .px_4()
                        .pt_2()
                        .text_xs()
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .text_color(theme::text_muted())
                        .child(view.entry.name())
                        .into_any_element(),
                );
                rows.extend(found);
            }
        }
        section(".env files", rows)
    }

    /// Settings → app switches that have no other home: the menu bar item.
    fn render_app_switches(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let menu_bar = self.config.menu_bar != Some(false);
        section(
            "App",
            vec![
                row()
                    .child(
                        div()
                            .flex_1()
                            .flex()
                            .flex_col()
                            .gap_0p5()
                            .child(div().text_sm().child("Menu bar item"))
                            .child(muted(
                                "fk N in the menu bar: what needs you, Today, agents, Capture note",
                            )),
                    )
                    .child(
                        super::widgets::switch("menu-bar", menu_bar).on_click(cx.listener(
                            move |this, _, _, cx| {
                                this.run(super::palette::Command::SetMenuBar(!menu_bar), cx)
                            },
                        )),
                    )
                    .into_any_element(),
            ],
        )
    }
}

impl Workspace {
    /// Setup's file reads, kept a few seconds instead of read every frame.
    fn setup_file_checks(&self) -> (bool, bool, bool) {
        self.setup_files.get(|| {
            let dir = self.tracker_dir();
            (
                std::fs::read_to_string(dir.join(".mcp.json"))
                    .is_ok_and(|t| t.contains("fubbik-desktop")),
                dir.join("CLAUDE.md").exists(),
                crate::claude_hooks::installed_file(&crate::claude_hooks::settings_path()),
            )
        })
    }
}
