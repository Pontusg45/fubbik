//! Preferences: what `desktop.toml` holds, editable in the app. Shown at the
//! top of Setup and on the first-launch screen.

use super::*;
use gpui::AnyElement;

use super::widgets::{button, label, muted, pill, row, section, segment};

impl Workspace {
    pub(super) fn render_preferences(&self, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let theme_label = match self.config.ghostty_theme.as_deref() {
            None | Some("") | Some("builtin") => "Built-in".to_string(),
            Some("follow") => "Following the Ghostty config".to_string(),
            Some(name) => format!("Ghostty: {name}"),
        };
        let translucent = theme::is_translucent();
        let persistent = self.config.tmux_persistence();
        let external = self.config.external_terminal();
        let tmux_note = if tmux::available() {
            "Terminal tabs are tmux sessions that survive quitting the app (new tabs)"
        } else {
            "tmux is not installed: terminal tabs end with the app"
        };

        // A switch: the label is the state, a click flips it.
        let toggle = |id: &'static str, on: bool, command: Command, cx: &mut Context<Self>| {
            super::widgets::switch(id, on)
                .on_click(cx.listener(move |this, _, _, cx| this.run(command.clone(), cx)))
        };
        let choice_row = |title: &str, detail: String, control: AnyElement| {
            row()
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .flex_1()
                        .child(div().text_sm().child(title.to_string()))
                        .child(muted(detail)),
                )
                .child(control)
                .into_any_element()
        };
        let field_row = |title: &str,
                         field: Field,
                         placeholder: &str,
                         window: &Window,
                         cx: &mut Context<Self>| {
            row()
                .child(div().w(px(150.)).child(label(title.to_uppercase())))
                .child(
                    div()
                        .flex_1()
                        .child(self.field(field, placeholder, window, cx)),
                )
                .child(
                    button(SharedString::from(format!("pref-save-{title}")), "Save").on_click(
                        cx.listener(move |this, _, _, cx| {
                            this.submit_preference(field, cx);
                            cx.notify();
                        }),
                    ),
                )
                .into_any_element()
        };

        let rows = vec![
            choice_row(
                "Colors",
                theme_label,
                button("pref-theme", "Choose…")
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.open_theme_picker(window);
                        cx.notify();
                    }))
                    .into_any_element(),
            ),
            choice_row(
                "Translucent window",
                "The desktop shows through a blur".into(),
                toggle("pref-translucent", translucent, Command::ToggleTheme, cx)
                    .into_any_element(),
            ),
            choice_row(
                "Agents at once",
                match self.config.max_agents {
                    Some(n) => format!(
                        "At most {n} running; more launches wait in a queue ({} waiting)",
                        self.launch_queue.len()
                    ),
                    None => "No limit".into(),
                },
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(button("pref-agents-less", "−").on_click(cx.listener(
                        |this, _, _, cx| {
                            let next = this
                                .config
                                .max_agents
                                .map(|n| n.saturating_sub(1).max(1))
                                .or(Some(4));
                            this.run(Command::SetMaxAgents(next), cx)
                        },
                    )))
                    .child(
                        div().w(px(64.)).text_center().text_sm().child(
                            self.config
                                .max_agents
                                .map_or("no limit".to_string(), |n| n.to_string()),
                        ),
                    )
                    .child(button("pref-agents-more", "+").on_click(cx.listener(
                        |this, _, _, cx| {
                            let next = this.config.max_agents.map(|n| n + 1).or(Some(4));
                            this.run(Command::SetMaxAgents(next), cx)
                        },
                    )))
                    .child(
                        segment(
                            "pref-agents-none",
                            "No limit",
                            self.config.max_agents.is_none(),
                        )
                        .on_click(
                            cx.listener(|this, _, _, cx| this.run(Command::SetMaxAgents(None), cx)),
                        ),
                    )
                    .into_any_element(),
            ),
            choice_row(
                "Keep terminals running (tmux)",
                tmux_note.into(),
                toggle(
                    "pref-tmux",
                    persistent,
                    Command::SetPersistence(!persistent),
                    cx,
                )
                .into_any_element(),
            ),
            choice_row(
                "AI sessions open in",
                if external {
                    "Terminal.app".into()
                } else {
                    "the Terminal tab".to_string()
                },
                div()
                    .flex()
                    .gap_1()
                    .child(
                        segment("pref-external-tab", "Terminal tab", !external).on_click(
                            cx.listener(|this, _, _, cx| {
                                this.run(Command::SetExternalTerminal(false), cx)
                            }),
                        ),
                    )
                    .child(
                        segment("pref-external-app", "Terminal.app", external).on_click(
                            cx.listener(|this, _, _, cx| {
                                this.run(Command::SetExternalTerminal(true), cx)
                            }),
                        ),
                    )
                    .into_any_element(),
            ),
            choice_row(
                "Start processes and scripts",
                match self.config.run_in.as_deref().and_then(RunTarget::from_key) {
                    Some(target) => format!("{} (saved)", target.title()),
                    None => "Ask each time".to_string(),
                },
                div()
                    .flex()
                    .gap_1()
                    .child(
                        segment("pref-run-ask", "Ask", self.config.run_in.is_none()).on_click(
                            cx.listener(|this, _, _, cx| this.run(Command::SetRunIn(None), cx)),
                        ),
                    )
                    .children(RunTarget::ALL.into_iter().map(|target| {
                        let label = match target {
                            RunTarget::Background => "Background",
                            RunTarget::Tmux => "tmux",
                            RunTarget::Tab => "Tab",
                            RunTarget::TerminalApp => "Terminal.app",
                        };
                        let active = self.config.run_in.as_deref() == Some(target.key());
                        segment(
                            SharedString::from(format!("pref-run-{}", target.key())),
                            label,
                            active,
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.run(Command::SetRunIn(Some(target)), cx)
                        }))
                    }))
                    .into_any_element(),
            ),
            choice_row(
                "Built-in server (API + PostgreSQL)",
                {
                    // The switch shows what you chose; until a restart the
                    // server keeps its current state.
                    let running = !matches!(crate::backend::status(), crate::backend::Status::Off);
                    let chosen = self.config.embedded_backend();
                    if chosen != running {
                        format!(
                            "{} — {} after restart",
                            crate::backend::status().label(),
                            if chosen { "on" } else { "off" }
                        )
                    } else {
                        crate::backend::status().label()
                    }
                },
                {
                    let running = !matches!(crate::backend::status(), crate::backend::Status::Off);
                    let chosen = self.config.embedded_backend();
                    div()
                        .flex()
                        .gap_1()
                        .child(toggle(
                            "pref-backend",
                            chosen,
                            Command::SetEmbeddedBackend(!chosen),
                            cx,
                        ))
                        .when(chosen != running, |d| {
                            d.child(
                                button("pref-restart", "Restart now")
                                    .on_click(cx.listener(|_, _, _, cx| cx.restart())),
                            )
                        })
                        .into_any_element()
                },
            ),
            choice_row(
                "Import existing data",
                "Replace the built-in database with a copy of one of these (each is only read)"
                    .into(),
                div()
                    .flex()
                    .gap_2()
                    .children(
                        self.import_sources()
                            .into_iter()
                            .enumerate()
                            .map(|(i, source)| {
                                // Host and database in full; never a password.
                                let label = source
                                    .split_once("://")
                                    .map(|(_, rest)| {
                                        rest.rsplit_once('@').map_or(rest, |(_, host)| host)
                                    })
                                    .unwrap_or(&source)
                                    .to_string();
                                button(("pref-import", i), label).on_click(cx.listener(
                                    move |this, _, _, cx| {
                                        this.run(Command::ImportDatabase(source.clone()), cx)
                                    },
                                ))
                            }),
                    )
                    .into_any_element(),
            ),
            choice_row(
                "Backups",
                format!(
                    "One a day, the last 7 kept, in {}",
                    display_path(&crate::backend::data_dir().join("backups"))
                ),
                div()
                    .flex()
                    .gap_2()
                    .child(
                        button("pref-backup", "Back up now").on_click(
                            cx.listener(|this, _, _, cx| this.run(Command::BackupNow, cx)),
                        ),
                    )
                    .child(
                        button("pref-backups-show", "Show in Finder").on_click(
                            cx.listener(|this, _, _, cx| this.run(Command::ShowBackups, cx)),
                        ),
                    )
                    .into_any_element(),
            ),
            choice_row(
                "When the app quits",
                if matches!(crate::backend::status(), crate::backend::Status::External)
                    && crate::backend::discovery::owner_pid().is_some()
                {
                    "A background server (from \"Quit, Keep Server Running\") is serving".into()
                } else {
                    "Quit stops the server; \"Quit, Keep Server Running\" leaves it serving MCP and the CLI".into()
                },
                div()
                    .flex()
                    .gap_2()
                    .child(
                        button("pref-keep", "Quit, keep server running").on_click(cx.listener(
                            |_, _, window, cx| {
                                window.dispatch_action(Box::new(super::QuitKeepServer), cx)
                            },
                        )),
                    )
                    // Only when a background server (not this app) holds the port.
                    .when(
                        crate::backend::discovery::owner_pid()
                            .is_some_and(|pid| pid != std::process::id()),
                        |d| {
                            d.child(button("pref-stop-bg", "Stop background server").on_click(
                                cx.listener(|this, _, _, cx| {
                                    this.run(Command::StopBackgroundServer, cx)
                                }),
                            ))
                        },
                    )
                    .into_any_element(),
            ),
            field_row(
                "fubbik server",
                Field::FubbikUrl,
                "http://localhost:3000 — enter saves",
                window,
                cx,
            ),
            field_row(
                "fubbik web app",
                Field::WebUrl,
                "http://localhost:3001 — enter saves",
                window,
                cx,
            ),
            field_row(
                "fubbik binary",
                Field::FubbikBin,
                "found automatically — or a path, enter saves",
                window,
                cx,
            ),
        ];
        section(
            format!("Preferences — {}", display_path(&self.config_path)),
            rows,
        )
        .into_any_element()
    }

    /// Databases to import from: the registered repos' PostgreSQL databases
    /// (services files) and the default, without duplicates.
    fn import_sources(&self) -> Vec<String> {
        let mut sources = vec![crate::backend::DEFAULT_IMPORT_SOURCE.to_string()];
        for db in self
            .repos
            .iter()
            .filter_map(|r| r.snapshot.as_ref())
            .flat_map(|s| s.databases.iter())
        {
            let url = db.spec.url.clone();
            if url.starts_with("postgres") && !sources.contains(&url) {
                sources.push(url);
            }
        }
        sources.truncate(4);
        sources
    }

    /// The palette, filtered to themes.
    pub(super) fn open_theme_picker(&mut self, window: &mut Window) {
        if !self.palette_open {
            self.toggle_palette(window);
        }
        self.palette_field.text = "theme ".into();
        self.palette_selected = 0;
    }

    /// Text preference fields, from the config.
    pub(super) fn sync_preference_fields(&mut self) {
        self.fubbik_url_field.text = self.config.fubbik_url.clone().unwrap_or_default();
        self.web_url_field.text = self.config.web_url.clone().unwrap_or_default();
        self.fubbik_bin_field.text = self
            .config
            .fubbik_bin
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_default();
    }

    /// Enter in a preference field: save it and apply it.
    pub(super) fn submit_preference(&mut self, field: Field, cx: &mut Context<Self>) {
        let value = |text: &str| Some(text.trim().to_string()).filter(|t| !t.is_empty());
        match field {
            Field::FubbikUrl | Field::WebUrl
                if value(&self.field_mut(field).text)
                    .is_some_and(|v| !v.starts_with("http://") && !v.starts_with("https://")) =>
            {
                self.show_error("A URL starts with http:// or https://");
                return;
            }
            Field::FubbikUrl => {
                self.config.fubbik_url = value(&self.fubbik_url_field.text);
                self.client = FubbikClient::new(self.config.fubbik_url());
                self.api_backoff = Duration::ZERO;
                self.knowledge_paused_until = None;
            }
            Field::WebUrl => self.config.web_url = value(&self.web_url_field.text),
            Field::FubbikBin => {
                self.config.fubbik_bin = value(&self.fubbik_bin_field.text).map(PathBuf::from)
            }
            _ => return,
        }
        self.save_config();
        self.show_info("Saved");
        self.poll_now(true, cx);
    }

    /// First launch: what to set up, with the next step for each.
    pub(super) fn render_checklist(&self, cx: &mut Context<Self>) -> AnyElement {
        let api = self.global.as_ref().map(|g| g.api.is_ok());
        let bin = self.global.as_ref().map(|g| g.fubbik_bin.is_some());
        let step = |done: Option<bool>, title: &str, detail: &str| {
            let (text, color) = match done {
                Some(true) => ("done", theme::status(crate::model::Status::Running)),
                Some(false) => ("to do", theme::status(crate::model::Status::Degraded)),
                None => ("checking", theme::text_muted()),
            };
            row()
                .child(div().w(px(80.)).child(pill(text, color)))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .flex_1()
                        .child(div().text_sm().child(title.to_string()))
                        .child(muted(detail.to_string())),
                )
        };
        section(
            "Getting started",
            vec![
                step(Some(false), "Register a repo", "⌘O, or the button below")
                    .child(button("check-register", "Register repo…").on_click(cx.listener(|this, _, _, cx| this.run(Command::RegisterRepo, cx))))
                    .into_any_element(),
                                step(api, "The fubbik server", "The app's built-in server starts on its own (Setup → Built-in server); knowledge, plans and boards come from it").into_any_element(),
                step(bin, "Build the Rust fubbik binary", "Register the fubbik repo, then Setup → Build fubbik (needed for the MCP server)").into_any_element(),
            ],
        )
        .into_any_element()
    }
}
