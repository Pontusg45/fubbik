//! Home → Tools → Disk: what each repo's builds and packages take, and
//! Docker's images, volumes and cache; every cleanup asks first.

use std::collections::HashSet;
use std::path::PathBuf;

use gpui::{AnyElement, FontWeight, SharedString};

use super::widgets::{button, disabled_button, loading, muted, row, section_or};
use super::*;
use crate::disk::{self, Item, Kind};

/// Today suggests a cleanup past this much build output and packages.
pub const NUDGE_BYTES: u64 = 30 * 1024 * 1024 * 1024;

#[derive(Debug, Clone)]
pub struct DiskReport {
    pub repos: Vec<(String, PathBuf, Vec<Item>)>,
    pub docker: Result<Vec<crate::containers::DiskUse>, String>,
}

#[derive(Default)]
pub struct DiskState {
    pub report: Option<Loadable<DiskReport>>,
    pub cleaning: HashSet<(PathBuf, String)>,
}

fn kind_key(kind: Kind) -> &'static str {
    match kind {
        Kind::Target => "target",
        Kind::NodeModules => "node_modules",
        Kind::Turbo => "turbo",
    }
}

impl Workspace {
    pub(super) fn scan_disk(&mut self, cx: &mut Context<Self>) {
        if matches!(self.disk.report, Some(Loadable::Loading)) {
            return;
        }
        self.disk.report = Some(Loadable::Loading);
        let repos: Vec<(String, PathBuf)> = self
            .repos
            .iter()
            .map(|r| (r.entry.name(), r.entry.path.clone()))
            .collect();
        let work = cx.background_executor().spawn(async move {
            DiskReport {
                repos: repos
                    .into_iter()
                    .map(|(name, path)| {
                        let items = disk::scan(&path);
                        (name, path, items)
                    })
                    .collect(),
                docker: crate::containers::disk(),
            }
        });
        cx.spawn(async move |this, cx| {
            let report = work.await;
            this.update(cx, |this, cx| {
                this.disk.report = Some(Loadable::Done(Ok(report)));
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// Measures in the background two minutes after launch, then daily, so
    /// Today can say when builds take too much room.
    pub(super) fn watch_disk(&mut self, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            let mut wait = std::time::Duration::from_secs(120);
            loop {
                cx.background_executor().timer(wait).await;
                if this.update(cx, |this, cx| this.scan_disk(cx)).is_err() {
                    break;
                }
                wait = std::time::Duration::from_secs(24 * 3600);
            }
        })
        .detach();
    }

    /// Cleans one item (after the confirmation), then rescans.
    pub(super) fn clean_disk(&mut self, repo: PathBuf, kind: &'static str, cx: &mut Context<Self>) {
        let Some(Loadable::Done(Ok(report))) = &self.disk.report else {
            return;
        };
        let Some(item) = report
            .repos
            .iter()
            .find(|(_, p, _)| *p == repo)
            .and_then(|(_, _, items)| items.iter().find(|i| kind_key(i.kind) == kind))
            .cloned()
        else {
            return;
        };
        self.disk.cleaning.insert((repo.clone(), kind.to_string()));
        let work = {
            let repo = repo.clone();
            cx.background_executor()
                .spawn(async move { disk::clean(&repo, &item).map(|_| item) })
        };
        cx.spawn(async move |this, cx| {
            let result = work.await;
            this.update(cx, |this, cx| {
                this.disk.cleaning.remove(&(repo.clone(), kind.to_string()));
                match result {
                    Ok(item) => this.show_info(format!(
                        "Freed {} — {}",
                        crate::pg::human_size(item.bytes),
                        disk::restore_hint(&repo, item.kind)
                    )),
                    Err(error) => this.show_error(format!("Clean failed: {error}")),
                }
                this.scan_disk(cx);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub(super) fn render_disk(&mut self, cx: &mut Context<Self>) -> AnyElement {
        if self.disk.report.is_none() {
            self.scan_disk(cx);
        }
        let report = match &self.disk.report {
            Some(Loadable::Done(Ok(report))) => report.clone(),
            _ => return loading("Measuring builds and packages…").into_any_element(),
        };
        let mut rows: Vec<AnyElement> = Vec::new();
        let mut total = 0u64;
        for (r, (name, path, items)) in report.repos.iter().enumerate() {
            for item in items {
                total += item.bytes;
                let key = kind_key(item.kind);
                let id = SharedString::from(format!("disk-{r}-{key}"));
                let busy = self
                    .disk
                    .cleaning
                    .contains(&(path.clone(), key.to_string()));
                let where_ = if item.paths.len() == 1 {
                    display_path(&item.paths[0])
                } else {
                    format!("{} folders", item.paths.len())
                };
                let confirm = Command::AskFirst(Box::new(super::palette::Confirm {
                    title: format!(
                        "Remove {name}'s {} ({})?",
                        item.kind.label(),
                        crate::pg::human_size(item.bytes)
                    ),
                    detail: format!(
                        "{}{}: {}.",
                        if item.kind == Kind::Target {
                            "cargo clean in the repo"
                        } else {
                            "Deletes "
                        },
                        if item.kind == Kind::Target {
                            String::new()
                        } else {
                            where_.clone()
                        },
                        disk::restore_hint(path, item.kind)
                    ),
                    action: "Remove".into(),
                    danger: false,
                    command: Command::CleanDisk {
                        repo: path.clone(),
                        kind: key,
                    },
                }));
                rows.push(
                    row()
                        .child(
                            div()
                                .w(px(160.))
                                .flex_none()
                                .text_sm()
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(name.clone()),
                        )
                        .child(
                            div()
                                .w(px(200.))
                                .flex_none()
                                .text_sm()
                                .child(item.kind.label()),
                        )
                        .child(
                            div()
                                .w(px(100.))
                                .flex_none()
                                .text_sm()
                                .child(crate::pg::human_size(item.bytes)),
                        )
                        .child(div().flex_1().min_w_0().truncate().child(muted(where_)))
                        .child(if busy {
                            disabled_button(id, "Removing…")
                        } else {
                            button(id, "Remove…").on_click(
                                cx.listener(move |this, _, _, cx| this.run(confirm.clone(), cx)),
                            )
                        })
                        .into_any_element(),
                );
            }
        }
        let docker_rows: Vec<AnyElement> = match &report.docker {
            Err(error) => vec![
                muted(format!(
                    "Docker is not answering: {}",
                    error.lines().next().unwrap_or(error)
                ))
                .into_any_element(),
            ],
            Ok(lines) => lines
                .iter()
                .map(|d| {
                    row()
                        .child(
                            div()
                                .w(px(160.))
                                .flex_none()
                                .text_sm()
                                .child(d.kind.clone()),
                        )
                        .child(
                            div()
                                .w(px(100.))
                                .flex_none()
                                .text_sm()
                                .child(d.size.clone()),
                        )
                        .child(muted(format!(
                            "{} · reclaimable {}",
                            d.count, d.reclaimable
                        )))
                        .into_any_element()
                })
                .collect(),
        };
        let prune = self.repos.first().map(|r| {
            Command::AskFirst(Box::new(super::palette::Confirm {
                title: "Prune Docker?".into(),
                detail: "docker system prune: removes stopped containers, unused networks, dangling images and the build cache. Running containers, volumes and tagged images stay. Runs in a terminal tab.".into(),
                action: "Prune".into(),
                danger: false,
                command: Command::RunIn {
                    dir: r.entry.path.clone(),
                    label: "docker prune".into(),
                    command: "docker system prune -f".into(),
                },
            }))
        });
        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme::text_muted())
                            .child(format!(
                                "DISK · {} IN BUILDS AND PACKAGES",
                                crate::pg::human_size(total)
                            )),
                    )
                    .child(div().flex_1())
                    .child(button("disk-rescan", "Rescan").on_click(cx.listener(
                        |this, _, _, cx| {
                            this.scan_disk(cx);
                            cx.notify();
                        },
                    )))
                    .children(prune.map(|confirm| {
                        button("disk-prune", "Prune Docker…").on_click(
                            cx.listener(move |this, _, _, cx| this.run(confirm.clone(), cx)),
                        )
                    })),
            )
            .child(section_or("Projects", rows, "Nothing to clean"))
            .child(section_or("Docker", docker_rows, "No Docker data"))
            .into_any_element()
    }
}
