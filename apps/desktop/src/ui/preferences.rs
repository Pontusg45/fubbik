//! Preferences: what `desktop.toml` holds, editable in the app. Shown at the
//! top of Setup and on the first-launch screen.

use super::*;
use gpui::AnyElement;

use super::widgets::{button, label, muted, pill, row, section};

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

        let toggle = |id: &'static str, on: bool, command: Command, cx: &mut Context<Self>| {
            button(
                id,
                if on {
                    "On — turn off"
                } else {
                    "Off — turn on"
                },
            )
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
                button(
                    "pref-external",
                    if external {
                        "Use the Terminal tab"
                    } else {
                        "Use Terminal.app"
                    },
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.run(Command::SetExternalTerminal(!external), cx)
                }))
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
                step(api, "Run the fubbik server", "`just dev` in the fubbik repo — knowledge, plans and agents boards come from it").into_any_element(),
                step(bin, "Build the Rust fubbik binary", "Register the fubbik repo, then Setup → Build fubbik (needed for the MCP server)").into_any_element(),
            ],
        )
        .into_any_element()
    }
}
