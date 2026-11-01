//! The Processes tab's scripts: one card, a header per tool, a compact row
//! per just `[group(...)]`, small chips, the scripts run last on top, and a
//! filter (⏎ runs the first match).

use gpui::{AnyElement, Div, SharedString, Stateful};

use super::widgets::{label, muted, pill, top_highlight};
use super::*;
use crate::model::{Status, process_key};
use crate::scripts::Script;

/// Chips per group row before "+N more" (a filter shows them all).
const FOLDED: usize = 9;
/// Scripts remembered per repo for the "Recent" row.
const RECENT: usize = 6;

impl Workspace {
    /// Called when a script runs, for the "Recent" row.
    pub(super) fn remember_script(&mut self, repo: usize, name: &str) {
        let Some(view) = self.repos.get(repo) else {
            return;
        };
        let recent = self
            .script_recent
            .entry(view.entry.path.clone())
            .or_default();
        recent.retain(|n| n != name);
        recent.insert(0, name.to_string());
        recent.truncate(RECENT);
    }

    /// The selected repo's scripts that match the filter, in list order.
    pub(super) fn filtered_scripts<'a>(&self, snapshot: &'a RepoSnapshot) -> Vec<&'a Script> {
        let query = self.script_filter.text.trim().to_lowercase();
        snapshot
            .scripts
            .iter()
            .filter(|s| {
                query.is_empty()
                    || s.name.to_lowercase().contains(&query)
                    || s.tool.to_lowercase().contains(&query)
                    || s.group
                        .as_deref()
                        .is_some_and(|g| g.to_lowercase().contains(&query))
            })
            .collect()
    }

    /// ⏎ in the filter: runs the first match.
    pub(super) fn run_first_script_match(&mut self, cx: &mut Context<Self>) {
        let repo = self.selected;
        let first = self
            .repos
            .get(repo)
            .and_then(|r| r.snapshot.as_ref())
            .and_then(|s| {
                self.filtered_scripts(s)
                    .first()
                    .map(|s| (s.name.clone(), s.command.clone()))
            });
        match first {
            Some((name, command)) => self.run(
                Command::RunScript {
                    repo,
                    name,
                    command,
                },
                cx,
            ),
            None => self.show_info("No script matches"),
        }
    }

    fn script_chip(
        &self,
        repo: usize,
        script: &Script,
        slot: &str,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let running = self.repos.get(repo).is_some_and(|r| {
            self.supervisor.is_running(&process_key(
                &r.entry.path,
                &format!("script:{}", script.name),
            ))
        });
        let (name, command) = (script.name.clone(), script.command.clone());
        div()
            .id(SharedString::from(format!(
                "script-{slot}-{}-{}",
                script.tool, script.name
            )))
            .flex()
            .flex_none()
            .items_center()
            .gap_1p5()
            .px_2()
            .py(px(3.))
            .rounded_md()
            .text_xs()
            .bg(theme::input())
            .text_color(theme::text())
            .cursor_pointer()
            .hover(|c| c.bg(theme::surface_hover()))
            .when(running, |c| {
                c.border_1()
                    .border_color(theme::status(Status::Running))
                    .child(
                        div()
                            .size(px(6.))
                            .rounded_full()
                            .bg(theme::status(Status::Running)),
                    )
            })
            .child(script.name.clone())
            .on_click(cx.listener(move |this, _, _, cx| {
                this.run(
                    Command::RunScript {
                        repo,
                        name: name.clone(),
                        command: command.clone(),
                    },
                    cx,
                )
            }))
    }

    /// A row: a muted label on the left, chips on the right.
    fn script_row(title: impl Into<SharedString>, chips: Vec<AnyElement>) -> AnyElement {
        div()
            .flex()
            .items_start()
            .gap_3()
            .px_4()
            .py_1p5()
            .child(
                div()
                    .w(px(132.))
                    .flex_none()
                    .pt(px(3.))
                    .text_xs()
                    .text_color(theme::text_muted())
                    .overflow_hidden()
                    .child(title.into()),
            )
            .child(div().flex().flex_1().flex_wrap().gap_1().children(chips))
            .into_any_element()
    }

    pub(super) fn render_scripts_panel(
        &self,
        index: usize,
        snapshot: &RepoSnapshot,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let filtering = !self.script_filter.text.trim().is_empty();
        let matches = self.filtered_scripts(snapshot);
        let mut rows: Vec<AnyElement> = Vec::new();

        let recent: Vec<&Script> = self
            .script_recent
            .get(&self.repos[index].entry.path)
            .into_iter()
            .flatten()
            .filter_map(|name| snapshot.scripts.iter().find(|s| &s.name == name))
            .collect();
        if !filtering && !recent.is_empty() {
            let chips = recent
                .into_iter()
                .map(|s| self.script_chip(index, s, "recent", cx).into_any_element())
                .collect();
            rows.push(Self::script_row("Recent", chips));
        }

        // Tools in first-seen order; within a tool, groups in first-seen order.
        let mut tools: Vec<&str> = Vec::new();
        for script in &matches {
            if !tools.contains(&script.tool.as_str()) {
                tools.push(&script.tool);
            }
        }
        for tool in tools {
            let of_tool: Vec<&Script> =
                matches.iter().copied().filter(|s| s.tool == tool).collect();
            rows.push(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .px_4()
                    .pt_3()
                    .pb_1()
                    .child(pill(tool.to_string(), theme::tool(tool)))
                    .child(muted(format!("{} script(s)", of_tool.len())))
                    .into_any_element(),
            );
            let mut groups: Vec<Option<&str>> = Vec::new();
            for script in &of_tool {
                if !groups.contains(&script.group.as_deref()) {
                    groups.push(script.group.as_deref());
                }
            }
            for group in groups {
                let scripts: Vec<&Script> = of_tool
                    .iter()
                    .copied()
                    .filter(|s| s.group.as_deref() == group)
                    .collect();
                let key = format!("{index}:{tool}:{}", group.unwrap_or(""));
                let expanded = filtering || self.scripts_expanded.contains(&key);
                let hidden = scripts.len().saturating_sub(FOLDED);
                let shown = if expanded {
                    scripts.len()
                } else {
                    scripts.len().min(FOLDED)
                };
                let mut chips: Vec<AnyElement> = scripts
                    .iter()
                    .take(shown)
                    .map(|s| self.script_chip(index, s, "list", cx).into_any_element())
                    .collect();
                if hidden > 0 && !filtering {
                    let key = key.clone();
                    chips.push(
                        div()
                            .id(SharedString::from(format!("scripts-more-{key}")))
                            .px_2()
                            .py(px(3.))
                            .text_xs()
                            .text_color(theme::accent())
                            .cursor_pointer()
                            .hover(|c| c.underline())
                            .child(if expanded {
                                "fewer".to_string()
                            } else {
                                format!("+{hidden} more")
                            })
                            .on_click(cx.listener(move |this, _, _, cx| {
                                if !this.scripts_expanded.remove(&key) {
                                    this.scripts_expanded.insert(key.clone());
                                }
                                cx.notify();
                            }))
                            .into_any_element(),
                    );
                }
                rows.push(Self::script_row(group.unwrap_or("").to_string(), chips));
            }
        }
        if matches.is_empty() {
            let message = if filtering {
                format!("No script matches “{}”", self.script_filter.text.trim())
            } else {
                "No justfile, package.json scripts or Makefile in this repo".to_string()
            };
            rows.push(div().px_4().py_3().child(muted(message)).into_any_element());
        }

        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(label("SCRIPTS"))
                    .child(muted("output in Logs"))
                    .child(div().flex_1())
                    .child(div().w(px(240.)).child(self.field(
                        Field::ScriptFilter,
                        "Filter scripts… ⏎ runs the first",
                        window,
                        cx,
                    ))),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .pb_2()
                    .rounded_lg()
                    .border_1()
                    .border_color(theme::border())
                    .bg(theme::surface())
                    .shadow(theme::card_shadow())
                    .relative()
                    .overflow_hidden()
                    .child(top_highlight())
                    .children(rows),
            )
            .into_any_element()
    }
}
