//! Home → Tools → Local domains: a `.test` name per process with a port,
//! served by Caddy, and ports two processes both claim.

use gpui::{AnyElement, FontWeight, SharedString};

use super::widgets::{button, muted, row, section_or};
use super::*;
use crate::proxy::{self, Site};

/// What the Caddyfile and /etc/hosts say, read when the section first shows
/// and after each change (not every frame).
#[derive(Debug, Clone, Default)]
pub struct DomainsState {
    pub read: bool,
    pub in_caddyfile: Vec<String>,
    pub hosts: String,
    pub caddy_running: bool,
}

impl Workspace {
    /// Every process with a port, across repos.
    pub(super) fn sites(&self) -> Vec<Site> {
        self.repos
            .iter()
            .filter_map(|r| r.snapshot.as_ref().map(|s| (r.entry.name(), s)))
            .flat_map(|(repo, s)| {
                s.processes.iter().filter_map(move |p| {
                    p.spec.port.map(|port| Site {
                        repo: repo.clone(),
                        process: p.spec.name.clone(),
                        port,
                    })
                })
            })
            .collect()
    }

    /// The sites chosen for a domain.
    pub(super) fn proxied_sites(&self) -> Vec<Site> {
        self.sites()
            .into_iter()
            .filter(|s| self.config.domains.contains(&s.key()))
            .collect()
    }

    pub(super) fn read_domains(&mut self) {
        let caddyfile = std::fs::read_to_string(proxy::caddyfile_path()).unwrap_or_default();
        self.domains = DomainsState {
            read: true,
            in_caddyfile: proxy::managed_domains(&caddyfile),
            hosts: proxy::hosts_text(),
            caddy_running: proxy::caddy_running(),
        };
    }

    pub(super) fn toggle_domain(&mut self, key: String) {
        if let Some(i) = self.config.domains.iter().position(|k| *k == key) {
            self.config.domains.remove(i);
        } else {
            self.config.domains.push(key);
            self.config.domains.sort();
        }
        self.save_config();
    }

    pub(super) fn write_caddyfile(&mut self) {
        match proxy::write_caddyfile(&self.proxied_sites()) {
            Ok(path) => self.show_info(format!(
                "Wrote {}{}",
                display_path(&path),
                if proxy::caddy_running() {
                    " — Caddy reloads it"
                } else {
                    " — start Caddy to serve it"
                }
            )),
            Err(error) => self.show_error(format!("Caddyfile: {error}")),
        }
        self.read_domains();
    }

    pub(super) fn render_domains(&mut self, cx: &mut Context<Self>) -> AnyElement {
        if !self.domains.read {
            self.read_domains();
        }
        let sites = self.sites();
        let conflicts = proxy::conflicts(&sites);
        let chosen: Vec<Site> = self.proxied_sites();
        let chosen_domains: Vec<String> = chosen.iter().map(Site::domain).collect();
        let missing_hosts = proxy::missing_hosts(&self.domains.hosts, &chosen_domains);
        let mut in_file = self.domains.in_caddyfile.clone();
        in_file.sort();
        let mut wanted = chosen_domains.clone();
        wanted.sort();
        let caddyfile_stale = in_file != wanted;
        let first_dir = self.repos.first().map(|r| r.entry.path.clone());

        let rows: Vec<AnyElement> = sites
            .iter()
            .enumerate()
            .map(|(i, site)| {
                let key = site.key();
                let on = self.config.domains.contains(&key);
                let domain = site.domain();
                let clash = conflicts.get(&site.port).map(|keys| {
                    keys.iter()
                        .filter(|k| **k != key)
                        .cloned()
                        .collect::<Vec<_>>()
                        .join(", ")
                });
                let state = if !on {
                    String::new()
                } else if !self.domains.in_caddyfile.contains(&domain) {
                    "not in the Caddyfile yet".into()
                } else if missing_hosts.contains(&domain) {
                    "no /etc/hosts entry".into()
                } else if !self.domains.caddy_running {
                    "Caddy is not running".into()
                } else {
                    "served".into()
                };
                let url = format!("https://{domain}");
                let served = state == "served";
                row()
                    .child(
                        div()
                            .w(px(150.))
                            .flex_none()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(site.repo.clone()),
                    )
                    .child(
                        div()
                            .w(px(110.))
                            .flex_none()
                            .text_sm()
                            .child(site.process.clone()),
                    )
                    .child(
                        div()
                            .w(px(70.))
                            .flex_none()
                            .text_sm()
                            .text_color(if clash.is_some() {
                                theme::danger()
                            } else {
                                theme::text()
                            })
                            .child(format!(":{}", site.port)),
                    )
                    .child(
                        div().flex_1().min_w_0().truncate().child(match &clash {
                            Some(others) => div()
                                .text_sm()
                                .text_color(theme::danger())
                                .child(format!("Port also declared by {others}")),
                            None if on => div().text_sm().child(format!("{domain} · {state}")),
                            None => muted(domain.clone()),
                        }),
                    )
                    .children(served.then(|| {
                        button(SharedString::from(format!("dom-open-{i}")), "Open")
                            .on_click(move |_, _, cx| cx.open_url(&url))
                    }))
                    .child(
                        button(
                            SharedString::from(format!("dom-{i}")),
                            if on { "Remove domain" } else { "Add domain" },
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.run(Command::ToggleDomain(key.clone()), cx)
                        })),
                    )
                    .into_any_element()
            })
            .collect();

        let write = Command::AskFirst(Box::new(super::palette::Confirm {
            title: "Update the Caddyfile?".into(),
            detail: format!(
                "Writes {} site{} into the app's marked block in {} (backed up to Caddyfile.fubbik-backup first); your own sites stay as they are. Each uses tls internal and reverse_proxy localhost:<port>.",
                chosen.len(),
                if chosen.len() == 1 { "" } else { "s" },
                display_path(&proxy::caddyfile_path())
            ),
            action: "Write".into(),
            danger: false,
            command: Command::WriteCaddyfile,
        }));
        let hosts = proxy::hosts_command(&missing_hosts)
            .zip(first_dir.clone())
            .map(|(command, dir)| {
                Command::AskFirst(Box::new(super::palette::Confirm {
                    title: "Add to /etc/hosts?".into(),
                    detail: format!(
                        "Runs sudo in a terminal tab (it asks for your password there) to add: {}.",
                        missing_hosts.join(", ")
                    ),
                    action: "Open terminal".into(),
                    danger: false,
                    command: Command::RunIn {
                        dir,
                        label: "hosts".into(),
                        command,
                    },
                }))
            });
        let start_caddy = (!self.domains.caddy_running)
            .then(proxy::caddy_command)
            .flatten()
            .zip(first_dir)
            .map(|(command, dir)| Command::RunIn {
                dir,
                label: "caddy".into(),
                command,
            });
        let conflict_note = (!conflicts.is_empty()).then(|| {
            div().text_sm().text_color(theme::danger()).child(format!(
                "{} port{} declared twice: those processes cannot run at once",
                conflicts.len(),
                if conflicts.len() == 1 { "" } else { "s" }
            ))
        });

        div()
            .flex()
            .flex_col()
            .gap_2()
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
                                "LOCAL DOMAINS · CADDY {}",
                                if self.domains.caddy_running {
                                    "RUNNING"
                                } else if proxy::caddy_bin().is_some() {
                                    "NOT RUNNING"
                                } else {
                                    "NOT INSTALLED"
                                }
                            )),
                    )
                    .child(div().flex_1())
                    .child(button("dom-refresh", "Refresh").on_click(cx.listener(
                        |this, _, _, cx| {
                            this.read_domains();
                            cx.notify();
                        },
                    )))
                    .children(caddyfile_stale.then(|| {
                        button("dom-write", "Update Caddyfile…").on_click(
                            cx.listener(move |this, _, _, cx| this.run(write.clone(), cx)),
                        )
                    }))
                    .children(hosts.map(|confirm| {
                        button("dom-hosts", "Add hosts entries…").on_click(
                            cx.listener(move |this, _, _, cx| this.run(confirm.clone(), cx)),
                        )
                    }))
                    .children(start_caddy.map(|command| {
                        button("dom-caddy", "Start Caddy").on_click(
                            cx.listener(move |this, _, _, cx| this.run(command.clone(), cx)),
                        )
                    })),
            )
            .children(conflict_note)
            .child(section_or(
                "Processes with a port",
                rows,
                "No process declares a port in a services file",
            ))
            .into_any_element()
    }
}
