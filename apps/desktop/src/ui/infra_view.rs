//! The Databases tab's lower half: each declared PostgreSQL database in
//! detail (size, connections, version, sqlx migration drift; psql, snapshot,
//! restore), and the repo's Docker containers (state, ports; logs, start,
//! stop, restart). Read while the tab is open.

use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use gpui::{AnyElement, FontWeight, SharedString};

use super::widgets::{button, disabled_button, loading, muted, pill, row, section_or};
use super::*;
use crate::containers::{self, Container};
use crate::pg::{self, DbInfo};

const CONTAINERS_EVERY: Duration = Duration::from_secs(15);
const DATABASES_EVERY: Duration = Duration::from_secs(60);

#[derive(Default)]
pub struct InfraState {
    pub containers: Option<(Instant, Loadable<Vec<Container>>)>,
    pub databases: HashMap<String, (Instant, Loadable<DbInfo>)>,
    pub snapshotting: HashSet<String>,
    pub container_busy: HashSet<String>,
}

impl Workspace {
    fn refresh_containers(&mut self, force: bool, cx: &mut Context<Self>) {
        let fresh =
            matches!(&self.infra.containers, Some((at, _)) if at.elapsed() < CONTAINERS_EVERY);
        let loading = matches!(&self.infra.containers, Some((_, Loadable::Loading)));
        if (fresh && !force) || loading {
            return;
        }
        if self.infra.containers.is_none() {
            self.infra.containers = Some((Instant::now(), Loadable::Loading));
        }
        let work = cx
            .background_executor()
            .spawn(async move { containers::list() });
        cx.spawn(async move |this, cx| {
            let result = work.await;
            this.update(cx, |this, cx| {
                this.infra.containers = Some((Instant::now(), Loadable::Done(result)));
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn refresh_database(&mut self, url: String, force: bool, cx: &mut Context<Self>) {
        match self.infra.databases.get(&url) {
            Some((_, Loadable::Loading)) => return,
            Some((at, _)) if at.elapsed() < DATABASES_EVERY && !force => return,
            _ => {}
        }
        if !self.infra.databases.contains_key(&url) {
            self.infra
                .databases
                .insert(url.clone(), (Instant::now(), Loadable::Loading));
        }
        let work = {
            let url = url.clone();
            cx.background_executor()
                .spawn(async move { pg::inspect(&url) })
        };
        cx.spawn(async move |this, cx| {
            let result = work.await;
            this.update(cx, |this, cx| {
                this.infra
                    .databases
                    .insert(url, (Instant::now(), Loadable::Done(result)));
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub(super) fn snapshot_database(&mut self, url: String, cx: &mut Context<Self>) {
        if !self.infra.snapshotting.insert(url.clone()) {
            return;
        }
        let stamp = {
            let (date, _) = crate::digest::today();
            let secs = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_secs() % 86_400);
            format!("{date}-{secs:05}")
        };
        let work = {
            let url = url.clone();
            cx.background_executor()
                .spawn(async move { pg::snapshot(&url, &stamp) })
        };
        cx.spawn(async move |this, cx| {
            let result = work.await;
            this.update(cx, |this, cx| {
                this.infra.snapshotting.remove(&url);
                match result {
                    Ok(file) => this.show_info(format!("Snapshot saved: {}", display_path(&file))),
                    Err(error) => this.show_error(format!("Snapshot failed: {error}")),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub(super) fn container_action(
        &mut self,
        name: String,
        action: &'static str,
        cx: &mut Context<Self>,
    ) {
        self.infra.container_busy.insert(name.clone());
        let work = {
            let name = name.clone();
            cx.background_executor()
                .spawn(async move { containers::act(action, &name) })
        };
        cx.spawn(async move |this, cx| {
            let result = work.await;
            this.update(cx, |this, cx| {
                this.infra.container_busy.remove(&name);
                if let Err(error) = result {
                    this.show_error(format!("docker {action} {name}: {error}"));
                }
                this.refresh_containers(true, cx);
                this.poll_now(false, cx);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub(super) fn render_infra(
        &mut self,
        index: usize,
        snapshot: &RepoSnapshot,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        self.refresh_containers(false, cx);
        let repo = self.repos[index].entry.path.clone();
        let repo_name = self.repos[index].entry.name();
        let urls: Vec<(String, String)> = snapshot
            .databases
            .iter()
            .filter(|d| d.spec.url.starts_with("postgres"))
            .map(|d| (d.spec.name.clone(), d.spec.url.clone()))
            .collect();
        for (_, url) in &urls {
            self.refresh_database(url.clone(), false, cx);
        }
        let sets = pg::migration_sets(&repo);

        // Databases in detail.
        let db_rows: Vec<AnyElement> = urls
            .iter()
            .enumerate()
            .map(|(i, (name, url))| {
                let id = |what: &str| SharedString::from(format!("pg-{what}-{index}-{i}"));
                let detail: AnyElement = match self.infra.databases.get(url) {
                    None | Some((_, Loadable::Loading)) => loading("Reading…").into_any_element(),
                    Some((_, Loadable::Done(Err(error)))) => {
                        muted(format!("not reachable: {}", error.lines().next().unwrap_or(error))).into_any_element()
                    }
                    Some((_, Loadable::Done(Ok(info)))) => {
                        let mut parts = div().flex().flex_wrap().items_center().gap_2().child(muted(format!(
                            "PostgreSQL {} · {} · {} connection(s)",
                            info.version,
                            pg::human_size(info.size_bytes),
                            info.connections
                        )));
                        if let Some(applied) = &info.applied {
                            let files = pg::best_set(applied, &sets);
                            let drift = pg::drift(applied, &files);
                            parts = parts.child(if files.is_empty() {
                                pill(format!("{} migrations applied", applied.len()), theme::text_muted())
                            } else if drift.pending.is_empty() && drift.unknown.is_empty() {
                                pill(format!("migrations current ({})", applied.len()), theme::status(Status::Running))
                            } else if !drift.pending.is_empty() {
                                pill(format!("{} pending: {}", drift.pending.len(), drift.pending.join(", ")), theme::status(Status::Degraded))
                            } else {
                                pill(
                                    format!("applied but not in this checkout: {:?}", drift.unknown),
                                    theme::status(Status::Degraded),
                                )
                            });
                        }
                        parts.into_any_element()
                    }
                };
                let snapshots = pg::snapshots(url);
                let mut actions = div().flex().gap_1();
                if let Some(cmd) = pg::psql_command(url) {
                    let repo = repo.clone();
                    actions = actions.child(button(id("psql"), "psql").on_click(cx.listener(move |this, _, _, cx| {
                        this.run(Command::RunIn { dir: repo.clone(), label: "psql".into(), command: cmd.clone() }, cx)
                    })));
                }
                actions = actions.child(if self.infra.snapshotting.contains(url) {
                    disabled_button(id("snap"), "Snapshotting…")
                } else {
                    let url = url.clone();
                    button(id("snap"), "Snapshot").on_click(cx.listener(move |this, _, _, cx| {
                        this.snapshot_database(url.clone(), cx);
                        cx.notify();
                    }))
                });
                if let (Some(latest), Some(cmd)) = (snapshots.first(), snapshots.first().and_then(|f| pg::restore_command(url, f))) {
                    let repo = repo.clone();
                    let file_name = latest.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                    let confirm = Command::AskFirst(Box::new(super::palette::Confirm {
                        title: format!("Restore {} from {file_name}?", pg::db_name(url)),
                        detail: "pg_restore --clean drops and recreates what the snapshot contains: changes since it are lost. Runs in a terminal tab.".into(),
                        action: "Restore".into(),
                        danger: true,
                        command: Command::RunIn { dir: repo, label: format!("restore {}", pg::db_name(url)), command: cmd },
                    }));
                    actions = actions.child(
                        button(id("restore"), format!("Restore latest ({})", snapshots.len()))
                            .on_click(cx.listener(move |this, _, _, cx| this.run(confirm.clone(), cx))),
                    );
                }
                let url_for_refresh = url.clone();
                actions = actions.child(button(id("refresh"), "Refresh").on_click(cx.listener(move |this, _, _, cx| {
                    this.refresh_database(url_for_refresh.clone(), true, cx);
                    cx.notify();
                })));
                row()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).child(format!("{name} · {}", pg::db_name(url))))
                            .child(detail),
                    )
                    .child(actions)
                    .into_any_element()
            })
            .collect();

        // Containers.
        let container_rows: Vec<AnyElement> = match &self.infra.containers {
            None | Some((_, Loadable::Loading)) => vec![loading("Asking Docker…").into_any_element()],
            Some((_, Loadable::Done(Err(error)))) => vec![muted(format!(
                "Docker is not answering ({}). Start OrbStack to see containers.",
                error.lines().next().unwrap_or(error)
            ))
            .into_any_element()],
            Some((_, Loadable::Done(Ok(all)))) => all
                .iter()
                .filter(|c| c.belongs_to(&repo, &repo_name))
                .enumerate()
                .map(|(i, c)| {
                    let id = |what: &str| SharedString::from(format!("ctr-{what}-{index}-{i}"));
                    let busy = self.infra.container_busy.contains(&c.name);
                    let (name_logs, name_a, name_b) = (c.name.clone(), c.name.clone(), c.name.clone());
                    let repo = repo.clone();
                    let mut actions = div().flex().gap_1().child(button(id("logs"), "Logs").on_click(cx.listener(
                        move |this, _, _, cx| {
                            this.run(
                                Command::RunIn {
                                    dir: repo.clone(),
                                    label: format!("logs {name_logs}"),
                                    command: containers::logs_command(&name_logs),
                                },
                                cx,
                            )
                        },
                    )));
                    if busy {
                        actions = actions.child(disabled_button(id("busy"), "…"));
                    } else if c.running() {
                        let stop = Command::AskFirst(Box::new(super::palette::Confirm {
                            title: format!("Stop {}?", c.name),
                            detail: "docker stop: anything using it (a dev server, tests) loses it until it is started again.".into(),
                            action: "Stop".into(),
                            danger: false,
                            command: Command::ContainerAction { name: c.name.clone(), action: "stop" },
                        }));
                        actions = actions
                            .child(button(id("restart"), "Restart").on_click(cx.listener(move |this, _, _, cx| {
                                this.run(Command::ContainerAction { name: name_a.clone(), action: "restart" }, cx)
                            })))
                            .child(button(id("stop"), "Stop").on_click(cx.listener(move |this, _, _, cx| this.run(stop.clone(), cx))));
                    } else {
                        actions = actions.child(button(id("start"), "Start").on_click(cx.listener(move |this, _, _, cx| {
                            this.run(Command::ContainerAction { name: name_b.clone(), action: "start" }, cx)
                        })));
                    }
                    row()
                        .child(pill(
                            c.state.clone(),
                            if c.running() { theme::status(Status::Running) } else { theme::text_muted() },
                        ))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .flex()
                                .flex_col()
                                .child(div().text_sm().child(match c.compose_project() {
                                    Some(project) => format!("{} · compose {project}", c.service()),
                                    None => c.service().to_string(),
                                }))
                                .child(muted(format!("{} · {} · {}", c.image, c.status, if c.ports.is_empty() { "no ports" } else { &c.ports }))),
                        )
                        .child(actions)
                        .into_any_element()
                })
                .collect(),
        };

        div()
            .flex()
            .flex_col()
            .gap_6()
            .when(!db_rows.is_empty(), |d| {
                d.child(section_or("Database details", db_rows, ""))
            })
            .child(section_or(
                "Containers (Docker / OrbStack)",
                container_rows,
                "No containers for this repo",
            ))
            .into_any_element()
    }
}
