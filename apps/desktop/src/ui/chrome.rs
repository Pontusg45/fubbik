//! The window: the root render with its key actions, the titlebar strip,
//! and toasts.

use super::widgets::button;
use super::*;
use gpui::AnyElement;

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

    /// The toasts, newest at the bottom: × closes one; one with an action
    /// shows where it goes and runs it on click.
    pub(super) fn render_toast(&self, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        if self.toasts.is_empty() {
            return None;
        }
        let rows = self.toasts.iter().enumerate().map(|(i, toast)| {
            let action = toast.action.clone();
            let has_action = action.is_some();
            div()
                .id(("toast", i))
                .flex()
                .items_start()
                .gap_3()
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
                .when(has_action, |d| {
                    d.cursor_pointer()
                        .hover(|d| d.border_color(theme::accent()))
                })
                .on_click(cx.listener(move |this, _, _, cx| {
                    if let Some(command) = action.clone() {
                        this.run(Command::DismissToast(i), cx);
                        this.run(command, cx);
                    }
                }))
                .child(
                    div()
                        .flex_1()
                        .text_color(if toast.error {
                            theme::danger()
                        } else {
                            theme::text()
                        })
                        .child(toast.message.clone()),
                )
                .when(has_action, |d| {
                    d.child(div().text_xs().text_color(theme::accent()).child("Open →"))
                })
                .child(
                    div()
                        .id(("toast-close", i))
                        .text_xs()
                        .text_color(theme::text_muted())
                        .cursor_pointer()
                        .hover(|d| d.text_color(theme::text()))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            cx.stop_propagation();
                            this.run(Command::DismissToast(i), cx);
                            cx.notify();
                        }))
                        .child("✕"),
                )
        });
        Some(
            div()
                .absolute()
                .bottom_4()
                .right_4()
                .flex()
                .flex_col()
                .items_end()
                .gap_2()
                .children(rows),
        )
    }

    /// "Are you sure?": ⏎ confirms, esc cancels.
    pub(super) fn render_confirm(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let confirm = self.confirm.clone()?;
        Some(
            div()
                .absolute()
                .inset_0()
                .flex()
                .justify_center()
                .pt(px(140.))
                .bg(theme::overlay())
                .child(
                    div()
                        .id("confirm")
                        .w(px(460.))
                        .flex()
                        .flex_col()
                        .gap_3()
                        .p_4()
                        .rounded_lg()
                        .border_1()
                        .border_color(if confirm.danger {
                            theme::danger()
                        } else {
                            theme::border()
                        })
                        .bg(theme::elevated())
                        .shadow(theme::frame_shadow())
                        .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                            this.confirm = None;
                            cx.notify();
                        }))
                        .child(
                            div()
                                .text_sm()
                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                .child(confirm.title.clone()),
                        )
                        .when(!confirm.detail.is_empty(), |d| {
                            d.child(
                                div()
                                    .text_xs()
                                    .text_color(theme::text_muted())
                                    .child(confirm.detail.clone()),
                            )
                        })
                        .child(
                            div()
                                .flex()
                                .justify_end()
                                .gap_2()
                                .child(button("confirm-cancel", "Cancel (esc)").on_click(
                                    cx.listener(|this, _, _, cx| {
                                        this.confirm = None;
                                        cx.notify();
                                    }),
                                ))
                                .child(
                                    button("confirm-ok", format!("{} (⏎)", confirm.action))
                                        .when(confirm.danger, |b| {
                                            b.text_color(theme::danger())
                                                .border_color(theme::danger())
                                        })
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.confirm_now(cx);
                                            cx.notify();
                                        })),
                                ),
                        ),
                )
                .into_any_element(),
        )
    }

    /// Runs the open confirmation's command.
    pub(super) fn confirm_now(&mut self, cx: &mut Context<Self>) {
        if let Some(confirm) = self.confirm.take() {
            self.confirmed = true;
            self.run(confirm.command, cx);
            self.confirmed = false;
        }
    }

    /// Keys while the confirmation is open; true when used.
    pub(super) fn on_confirm_key(&mut self, key: &str, cx: &mut Context<Self>) -> bool {
        if self.confirm.is_none() {
            return false;
        }
        match key {
            "enter" => self.confirm_now(cx),
            "escape" => self.confirm = None,
            _ => {}
        }
        true
    }
}

impl Workspace {
    /// ⇧⌘N: a note into the selected repo's fubbik space, from anywhere.
    fn render_capture(&self, window: &Window, cx: &mut Context<Self>) -> impl IntoElement {
        let repo = self
            .repos
            .get(self.selected)
            .map(|r| r.entry.name())
            .unwrap_or_default();
        div()
            .absolute()
            .inset_0()
            .flex()
            .justify_center()
            .pt(px(110.))
            .bg(theme::overlay())
            .child(
                div()
                    .id("capture")
                    .w(px(520.))
                    .flex()
                    .flex_col()
                    .gap_2()
                    .p_3()
                    .rounded_lg()
                    .border_1()
                    .border_color(theme::border())
                    .bg(theme::elevated())
                    .shadow(theme::frame_shadow())
                    .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                        this.capture_open = false;
                        cx.notify();
                    }))
                    .child(
                        div()
                            .text_sm()
                            .child(format!("Note to {repo}'s fubbik space")),
                    )
                    .child(self.field(
                        Field::NoteTitle,
                        "Title — enter goes to the text",
                        window,
                        cx,
                    ))
                    .child(self.field(
                        Field::NoteBody,
                        "Text — enter saves, esc cancels",
                        window,
                        cx,
                    )),
            )
    }
}

impl Workspace {
    /// ⌘/: every shortcut, from the live keymap, plus the list keys. The
    /// Terminal group comes first while a terminal has focus.
    fn render_cheat_sheet(&self, window: &Window, cx: &mut Context<Self>) -> impl IntoElement {
        let in_terminal = self
            .active_terminal
            .and_then(|a| self.terminals.iter().find(|t| t.id == a))
            .is_some_and(|t| t.view.read(cx).focus.is_focused(window));
        let bindings = keymap::current();
        let mut groups: Vec<&str> = Vec::new();
        for binding in &bindings {
            if !groups.contains(&binding.group) {
                groups.push(binding.group);
            }
        }
        if in_terminal && let Some(i) = groups.iter().position(|g| *g == "Terminal") {
            let terminal = groups.remove(i);
            groups.insert(0, terminal);
        }
        let entry = |keys: String, what: String| {
            div()
                .flex()
                .gap_3()
                .py_0p5()
                .child(
                    div()
                        .w(px(120.))
                        .flex_none()
                        .text_color(theme::accent())
                        .child(keys),
                )
                .child(div().text_color(theme::text()).child(what))
        };
        let group_block = |title: String, rows: Vec<gpui::Div>| {
            div()
                .flex()
                .flex_col()
                .gap_0p5()
                .child(
                    div()
                        .pb_1()
                        .text_xs()
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .text_color(theme::text_muted())
                        .child(title.to_uppercase()),
                )
                .children(rows)
        };
        let mut blocks: Vec<gpui::Div> = groups
            .iter()
            .map(|group| {
                let rows = bindings
                    .iter()
                    .filter(|b| b.group == *group && !b.keys.is_empty())
                    // ⌃1–⌃9 and ⌘1–⌘9 in a terminal: one line each, not nine.
                    .filter(|b| {
                        !matches!(b.action.as_bytes().last(), Some(b'2'..=b'9'))
                            || !(b.action.starts_with("select_repo_")
                                || b.action.starts_with("terminal_tab_"))
                    })
                    .map(|b| entry(keymap::pretty(&b.keys), b.description.to_string()))
                    .collect();
                group_block(group.to_string(), rows)
            })
            .collect();
        let mut list_groups: Vec<&str> = Vec::new();
        for (group, _, _) in keymap::LIST_KEYS {
            if !list_groups.contains(group) {
                list_groups.push(group);
            }
        }
        blocks.push(group_block(
            "In lists (when no text field or terminal has focus)".into(),
            keymap::LIST_KEYS
                .iter()
                .map(|(group, keys, what)| entry(keys.to_string(), format!("{group}: {what}")))
                .collect(),
        ));
        let mut anywhere = vec![
            entry("⌘K @".into(), "Palette: repos and their tabs".into()),
            entry("⌘K #".into(), "Palette: open tasks on every plan".into()),
            entry(
                "⌘K !".into(),
                "Palette: running processes and live agents".into(),
            ),
            entry("⌘K >".into(), "Palette: commands only".into()),
            entry("⌘K ?".into(), "Palette: search chunks".into()),
        ];
        if let Some(keys) = self.config.global_hotkey() {
            anywhere.insert(
                0,
                entry(
                    keymap::pretty(keys),
                    "Show or hide fubbik from any app (global_hotkey in desktop.toml)".into(),
                ),
            );
        }
        blocks.insert(0, group_block("Anywhere".into(), anywhere));
        div()
            .absolute()
            .inset_0()
            .flex()
            .justify_center()
            .pt(px(60.))
            .bg(theme::overlay())
            .child(
                div()
                    .id("cheat-sheet")
                    .w(px(900.))
                    .max_h(px(640.))
                    .overflow_y_scroll()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .p_4()
                    .rounded_lg()
                    .border_1()
                    .border_color(theme::border())
                    .bg(theme::elevated())
                    .shadow(theme::frame_shadow())
                    .text_sm()
                    .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                        this.cheat_sheet_open = false;
                        cx.notify();
                    }))
                    .child(
                        div()
                            .flex()
                            .justify_between()
                            .child(
                                div()
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .child("Keyboard shortcuts"),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme::text_muted())
                                    .child("esc closes · change keys in Setup → Keyboard"),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .gap_6()
                            .children(blocks.into_iter().map(|b| b.w(px(410.)))),
                    ),
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
            self.render_empty(window, cx).into_any_element()
        } else if self.home {
            self.render_home(cx)
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
            .on_action(cx.listener(|this, _: &ShowCheatSheet, _, cx| {
                this.cheat_sheet_open = !this.cheat_sheet_open;
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &FocusSidebar, window, cx| {
                this.sidebar_nav = true;
                this.focus.focus(window);
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &NextNeedsYou, _, cx| this.next_needs_you(cx)))
            .on_action(
                cx.listener(|this, _: &ReplyLatest, window, cx| this.reply_latest(window, cx)),
            )
            .on_action(cx.listener(|this, _: &StartNextTask, _, cx| this.start_next_task(cx)))
            .on_action(cx.listener(|this, _: &NewShell, _, cx| this.new_shell(cx)))
            .on_action(cx.listener(|this, _: &CloseTerminalTab, _, cx| this.close_terminal_tab(cx)))
            .on_action(cx.listener(|this, _: &PrevTerminalTab, _, cx| {
                this.switch_terminal_tab(None, -1, cx)
            }))
            .on_action(
                cx.listener(|this, _: &NextTerminalTab, _, cx| {
                    this.switch_terminal_tab(None, 1, cx)
                }),
            )
            .on_action(
                cx.listener(|this, _: &SplitTerminal, _, cx| this.run(Command::ToggleSplit, cx)),
            )
            .on_action(cx.listener(|this, _: &ClearTerminal, _, cx| this.clear_terminal(cx)))
            .on_action(cx.listener(|this, _: &SelectRepo1, _, cx| this.select_repo_n(0, cx)))
            .on_action(
                cx.listener(|this, _: &TerminalTab1, _, cx| {
                    this.switch_terminal_tab(Some(0), 0, cx)
                }),
            )
            .on_action(cx.listener(|this, _: &SelectRepo2, _, cx| this.select_repo_n(1, cx)))
            .on_action(
                cx.listener(|this, _: &TerminalTab2, _, cx| {
                    this.switch_terminal_tab(Some(1), 0, cx)
                }),
            )
            .on_action(cx.listener(|this, _: &SelectRepo3, _, cx| this.select_repo_n(2, cx)))
            .on_action(
                cx.listener(|this, _: &TerminalTab3, _, cx| {
                    this.switch_terminal_tab(Some(2), 0, cx)
                }),
            )
            .on_action(cx.listener(|this, _: &SelectRepo4, _, cx| this.select_repo_n(3, cx)))
            .on_action(
                cx.listener(|this, _: &TerminalTab4, _, cx| {
                    this.switch_terminal_tab(Some(3), 0, cx)
                }),
            )
            .on_action(cx.listener(|this, _: &SelectRepo5, _, cx| this.select_repo_n(4, cx)))
            .on_action(
                cx.listener(|this, _: &TerminalTab5, _, cx| {
                    this.switch_terminal_tab(Some(4), 0, cx)
                }),
            )
            .on_action(cx.listener(|this, _: &SelectRepo6, _, cx| this.select_repo_n(5, cx)))
            .on_action(
                cx.listener(|this, _: &TerminalTab6, _, cx| {
                    this.switch_terminal_tab(Some(5), 0, cx)
                }),
            )
            .on_action(cx.listener(|this, _: &SelectRepo7, _, cx| this.select_repo_n(6, cx)))
            .on_action(
                cx.listener(|this, _: &TerminalTab7, _, cx| {
                    this.switch_terminal_tab(Some(6), 0, cx)
                }),
            )
            .on_action(cx.listener(|this, _: &SelectRepo8, _, cx| this.select_repo_n(7, cx)))
            .on_action(
                cx.listener(|this, _: &TerminalTab8, _, cx| {
                    this.switch_terminal_tab(Some(7), 0, cx)
                }),
            )
            .on_action(cx.listener(|this, _: &SelectRepo9, _, cx| this.select_repo_n(8, cx)))
            .on_action(
                cx.listener(|this, _: &TerminalTab9, _, cx| {
                    this.switch_terminal_tab(Some(8), 0, cx)
                }),
            )
            .on_action(
                cx.listener(|this, _: &ShowToday, _, cx| {
                    this.run(Command::ShowTab(Tab::Today), cx)
                }),
            )
            .on_action(cx.listener(|this, _: &ShowInsights, _, cx| {
                this.run(Command::ShowTab(Tab::Insights), cx)
            }))
            .on_action(cx.listener(|this, _: &ShowHome, _, cx| {
                this.go_home();
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &CaptureNote, window, cx| {
                this.capture_open = true;
                this.note_title.focus.focus(window);
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &ShowAgents, _, cx| {
                this.run(Command::ShowTab(Tab::Agents), cx)
            }))
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
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                if this.on_confirm_key(&event.keystroke.key, cx) {
                    cx.stop_propagation();
                    cx.notify();
                    return;
                }
                if this.on_run_prompt_key(&event.keystroke, cx) {
                    cx.stop_propagation();
                    cx.notify();
                    return;
                }
                if event.keystroke.key == "escape"
                    && (this.panel.is_some() || this.cheat_sheet_open)
                {
                    this.panel = None;
                    this.cheat_sheet_open = false;
                    cx.notify();
                    return;
                }
                // Lists answer plain keys only when nothing that types has focus.
                if this.focus.is_focused(window) && this.on_list_key(&event.keystroke, window, cx) {
                    cx.stop_propagation();
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
            .when(self.capture_open, |root| {
                root.child(self.render_capture(window, cx))
            })
            .children(self.render_run_prompt(cx))
            .children(self.render_confirm(cx))
            .when(self.cheat_sheet_open, |root| {
                root.child(self.render_cheat_sheet(window, cx))
            })
            .children(self.render_toast(cx))
    }
}
