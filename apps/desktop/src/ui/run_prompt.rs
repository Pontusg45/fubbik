//! "Where should this run?": shown when a process or script starts and no
//! choice is saved (`run_in` in desktop.toml). 1–4 pick, A toggles "always
//! use this", esc cancels.

use gpui::{AnyElement, FontWeight, Keystroke, SharedString};

use super::widgets::muted;
use super::*;

impl Workspace {
    /// Keys while the prompt is open; true when used.
    pub(super) fn on_run_prompt_key(
        &mut self,
        keystroke: &Keystroke,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(what) = self.run_prompt.clone() else {
            return false;
        };
        match keystroke.key.as_str() {
            "escape" => self.run_prompt = None,
            "a" => self.run_prompt_remember = !self.run_prompt_remember,
            key => {
                let Some(target) = key
                    .parse::<usize>()
                    .ok()
                    .and_then(|n| n.checked_sub(1))
                    .and_then(|i| RunTarget::ALL.get(i).copied())
                else {
                    return false;
                };
                if target == RunTarget::Tmux && !tmux::available() {
                    return true;
                }
                let remember = self.run_prompt_remember;
                self.run(
                    Command::RunAt {
                        what,
                        target,
                        remember,
                    },
                    cx,
                );
            }
        }
        true
    }

    pub(super) fn render_run_prompt(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let what = self.run_prompt.clone()?;
        let title = self.run_what_label(&what);
        let remember = self.run_prompt_remember;
        let options = RunTarget::ALL.into_iter().enumerate().map(|(i, target)| {
            let disabled = target == RunTarget::Tmux && !tmux::available();
            let what = what.clone();
            div()
                .id(SharedString::from(format!("run-at-{}", target.key())))
                .flex()
                .items_start()
                .gap_3()
                .px_3()
                .py_2()
                .rounded_md()
                .border_1()
                .border_color(theme::border())
                .when(!disabled, |d| {
                    d.cursor_pointer()
                        .hover(|d| d.bg(theme::surface_hover()))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            let remember = this.run_prompt_remember;
                            this.run(
                                Command::RunAt {
                                    what: what.clone(),
                                    target,
                                    remember,
                                },
                                cx,
                            );
                            cx.notify();
                        }))
                })
                .when(disabled, |d| d.opacity(0.5))
                .child(
                    div()
                        .w(px(18.))
                        .flex_none()
                        .text_sm()
                        .text_color(theme::accent())
                        .child(format!("{}", i + 1)),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_0p5()
                        .child(
                            div()
                                .text_sm()
                                .font_weight(FontWeight::MEDIUM)
                                .child(target.title()),
                        )
                        .child(muted(if disabled {
                            "tmux is not installed (brew install tmux)"
                        } else {
                            target.detail()
                        })),
                )
        });
        Some(
            div()
                .absolute()
                .inset_0()
                .flex()
                .justify_center()
                .pt(px(110.))
                .bg(theme::overlay())
                .child(
                    div()
                        .id("run-prompt")
                        .w(px(540.))
                        .flex()
                        .flex_col()
                        .gap_2()
                        .p_4()
                        .rounded_lg()
                        .border_1()
                        .border_color(theme::border())
                        .bg(theme::elevated())
                        .shadow(theme::frame_shadow())
                        .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                            this.run_prompt = None;
                            cx.notify();
                        }))
                        .child(
                            div()
                                .text_sm()
                                .font_weight(FontWeight::SEMIBOLD)
                                .child("Where should this run?"),
                        )
                        .child(muted(title))
                        .children(options)
                        .child(
                            div()
                                .id("run-prompt-remember")
                                .flex()
                                .items_center()
                                .gap_2()
                                .pt_1()
                                .text_xs()
                                .cursor_pointer()
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.run_prompt_remember = !this.run_prompt_remember;
                                    cx.notify();
                                }))
                                .child(
                                    div()
                                        .size(px(12.))
                                        .rounded_sm()
                                        .border_1()
                                        .border_color(theme::border())
                                        .when(remember, |d| d.bg(theme::accent())),
                                )
                                .child("Always use this (A) — change it in Setup")
                                .child(div().flex_1())
                                .child(muted("1–4 choose · esc cancel")),
                        ),
                )
                .into_any_element(),
        )
    }
}
