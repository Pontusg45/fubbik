//! The window: the root render with its key actions, the titlebar strip,
//! and toasts.

use super::*;

impl Workspace {
    /// The strip under the transparent system titlebar: traffic lights on
    /// the left, the title in the middle. Nothing in it is clickable, so the
    /// system can drag the window from it.
    pub(super) fn render_titlebar(&self) -> impl IntoElement {
        let title = match self.repos.get(self.selected) {
            Some(repo) => format!("fubbik — {}", repo.entry.name()),
            None => "fubbik".to_string(),
        };
        div()
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .h(px(TITLEBAR_HEIGHT))
            .border_b_1()
            .border_color(theme::border())
            .bg(theme::sidebar())
            .text_xs()
            .text_color(theme::text_muted())
            .child(title)
    }

    pub(super) fn render_toast(&self) -> Option<impl IntoElement> {
        let toast = self.toast.as_ref()?;
        Some(
            div()
                .absolute()
                .bottom_4()
                .right_4()
                .max_w(px(460.))
                .px_4()
                .py_2()
                .rounded_lg()
                .border_1()
                .border_color(if toast.error {
                    theme::danger()
                } else {
                    theme::border()
                })
                .bg(theme::elevated())
                .shadow(theme::frame_shadow())
                .text_sm()
                .text_color(if toast.error {
                    theme::danger()
                } else {
                    theme::text()
                })
                .child(toast.message.clone()),
        )
    }
}

impl Render for Workspace {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let translucent = theme::is_translucent();
        if self.applied_translucent != Some(translucent) {
            window.set_background_appearance(theme::window_background());
            self.applied_translucent = Some(translucent);
        }
        let main = if self.repos.is_empty() {
            self.render_empty(cx).into_any_element()
        } else {
            self.render_repo(window, cx).into_any_element()
        };
        div()
            .id("workspace")
            .relative()
            .flex()
            .flex_col()
            .size_full()
            .bg(theme::backdrop())
            .text_color(theme::text())
            .track_focus(&self.focus)
            .on_action(cx.listener(|this, _: &TogglePalette, window, cx| {
                this.toggle_palette(window);
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &NextRepo, _, cx| {
                if !this.repos.is_empty() {
                    this.select((this.selected + 1) % this.repos.len());
                }
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &PrevRepo, _, cx| {
                if !this.repos.is_empty() {
                    this.select((this.selected + this.repos.len() - 1) % this.repos.len());
                }
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &ShowOverview, _, cx| {
                this.run(Command::ShowTab(Tab::Overview), cx)
            }))
            .on_action(cx.listener(|this, _: &ShowProcesses, _, cx| {
                this.run(Command::ShowTab(Tab::Processes), cx)
            }))
            .on_action(cx.listener(|this, _: &ShowDatabases, _, cx| {
                this.run(Command::ShowTab(Tab::Databases), cx)
            }))
            .on_action(
                cx.listener(|this, _: &ShowLogs, _, cx| this.run(Command::ShowTab(Tab::Logs), cx)),
            )
            .on_action(cx.listener(|this, _: &ShowWorktrees, _, cx| {
                this.run(Command::ShowTab(Tab::Worktrees), cx)
            }))
            .on_action(
                cx.listener(|this, _: &ShowPlans, _, cx| {
                    this.run(Command::ShowTab(Tab::Plans), cx)
                }),
            )
            .on_action(
                cx.listener(|this, _: &ShowSetup, _, cx| {
                    this.run(Command::ShowTab(Tab::Setup), cx)
                }),
            )
            .on_action(cx.listener(|this, _: &ShowTerminal, _, cx| {
                this.run(Command::ShowTab(Tab::Terminal), cx)
            }))
            .on_action(
                cx.listener(|this, _: &ToggleTheme, _, cx| this.run(Command::ToggleTheme, cx)),
            )
            .on_action(cx.listener(|this, _: &ChooseTheme, window, cx| {
                if !this.palette_open {
                    this.toggle_palette(window);
                }
                this.palette_field.text = "theme ".into();
                this.palette_selected = 0;
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &ShowSessions, _, cx| {
                this.run(Command::ShowTab(Tab::Sessions), cx)
            }))
            .on_action(cx.listener(|this, _: &ShowKnowledge, _, cx| {
                this.run(Command::ShowTab(Tab::Knowledge), cx)
            }))
            .on_action(
                cx.listener(|this, _: &RegisterRepo, _, cx| this.run(Command::RegisterRepo, cx)),
            )
            .on_action(cx.listener(|this, _: &Refresh, _, cx| this.run(Command::Refresh, cx)))
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                if event.keystroke.key == "escape" && this.panel.is_some() {
                    this.panel = None;
                    cx.notify();
                }
            }))
            .on_drop(cx.listener(|this, paths: &ExternalPaths, _, cx| {
                this.show_file_context(paths.paths().to_vec(), cx);
            }))
            .child(self.render_titlebar())
            .child(
                div()
                    .flex()
                    .flex_1()
                    .overflow_hidden()
                    .child(self.render_sidebar(cx))
                    .child(main),
            )
            .when(self.palette_open, |root| {
                root.child(self.render_palette(window, cx))
            })
            .children(self.render_panel(cx))
            .children(self.render_toast())
    }
}
