//! Home → Tools: the machine's developer tools and services (see
//! `src/tools.rs`), with install and upgrade through Homebrew in a terminal
//! tab, and a few one-click utilities.

use std::time::SystemTime;

use gpui::{AnyElement, FontWeight, SharedString};

use super::widgets::{button, loading, muted, pill, row, section};
use super::*;
use crate::theme::Hue;
use crate::tools;

/// "Upgraded today/yesterday" is flagged; older upgrades are just noted.
const RECENT_UPGRADE_DAYS: u64 = 2;

impl Workspace {
    pub(super) fn load_tools(&mut self, cx: &mut Context<Self>) {
        if matches!(self.tools, Some(Loadable::Loading)) || self.tools_refreshing {
            return;
        }
        if matches!(self.tools, Some(Loadable::Done(Ok(_)))) {
            self.tools_refreshing = true;
        } else {
            self.tools = Some(Loadable::Loading);
        }
        let work = cx
            .background_executor()
            .spawn(async move { tools::report() });
        cx.spawn(async move |this, cx| {
            let report = work.await;
            this.update(cx, |this, cx| {
                this.tools = Some(Loadable::Done(Ok(report)));
                this.tools_refreshing = false;
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// A command in a terminal tab, in the selected repo (terminal tabs
    /// belong to a repo); Homebrew does not care where it runs.
    fn tools_run(&self, label: String, command: String) -> Option<Command> {
        let dir = self.repos.get(self.selected)?.entry.path.clone();
        Some(Command::RunIn {
            dir,
            label,
            command,
        })
    }

    pub(super) fn render_tools(&mut self, cx: &mut Context<Self>) -> AnyElement {
        if self.tools.is_none() {
            self.load_tools(cx);
        }
        let report = match &self.tools {
            Some(Loadable::Done(Ok(report))) => report.clone(),
            Some(Loadable::Done(Err(error))) => return muted(error.clone()).into_any_element(),
            _ => return loading("Checking tools and services…").into_any_element(),
        };
        let now = SystemTime::now();
        let outdated: Vec<&tools::ToolStatus> = report
            .tools
            .iter()
            .filter(|t| t.outdated.is_some())
            .collect();

        // Toolbar: refresh, brew update, upgrade everything outdated.
        let mut toolbar = div().flex().items_center().gap_2().child(
            button(
                "tools-refresh",
                if self.tools_refreshing {
                    "Checking…"
                } else {
                    "Refresh"
                },
            )
            .on_click(cx.listener(|this, _, _, cx| {
                this.load_tools(cx);
                cx.notify();
            })),
        );
        if let Some(command) =
            self.tools_run("brew update".into(), "brew update && brew outdated".into())
        {
            toolbar =
                toolbar
                    .child(button("tools-brew-update", "brew update").on_click(
                        cx.listener(move |this, _, _, cx| this.run(command.clone(), cx)),
                    ));
        }
        if !outdated.is_empty() {
            let names: Vec<String> = outdated
                .iter()
                .filter_map(|t| t.brew)
                .map(|b| b.trim_start_matches("cask:").to_string())
                .collect();
            if let Some(command) = self.tools_run(
                "brew upgrade".into(),
                format!("brew upgrade {}", names.join(" ")),
            ) {
                let confirm = Command::AskFirst(Box::new(super::palette::Confirm {
                    title: format!("Upgrade {} tool(s)?", names.len()),
                    detail: format!(
                        "Runs brew upgrade {} in a terminal tab. Upgrades can change behavior (tmux 3.8 did); the app may need a restart after.",
                        names.join(" ")
                    ),
                    action: "Upgrade".into(),
                    danger: false,
                    command,
                }));
                toolbar = toolbar.child(
                    button(
                        "tools-upgrade-all",
                        format!("Upgrade all ({})", names.len()),
                    )
                    .on_click(cx.listener(move |this, _, _, cx| this.run(confirm.clone(), cx))),
                );
            }
        }
        toolbar = toolbar.child(muted(match &report.brew_error {
            Some(error) => format!("Homebrew: {error}"),
            None if outdated.is_empty() => "Everything from Homebrew is up to date".into(),
            None => format!("{} can be upgraded", outdated.len()),
        }));

        // Services the app leans on.
        let services: Vec<AnyElement> = report
            .services
            .iter()
            .enumerate()
            .map(|(i, service)| {
                let fix = match (service.ok, service.name) {
                    (false, "GitHub (gh auth)") => self
                        .tools_run("gh auth login".into(), "gh auth login".into())
                        .map(|c| ("Sign in", c)),
                    (false, "Docker / OrbStack") => self
                        .tools_run("OrbStack".into(), "open -a OrbStack".into())
                        .map(|c| ("Start OrbStack", c)),
                    (false, "Ollama") if !service.detail.contains("not installed") => self
                        .tools_run("ollama serve".into(), "ollama serve".into())
                        .map(|c| ("Start Ollama", c)),
                    _ => None,
                };
                row()
                    .child(pill(
                        if service.ok { "ok" } else { "off" },
                        if service.ok {
                            theme::status(Status::Running)
                        } else {
                            theme::status(Status::Degraded)
                        },
                    ))
                    .child(div().w(px(200.)).text_sm().child(service.name))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .child(muted(service.detail.clone())),
                    )
                    .children(fix.map(|(label, command)| {
                        button(("service-fix", i), label).on_click(
                            cx.listener(move |this, _, _, cx| this.run(command.clone(), cx)),
                        )
                    }))
                    .into_any_element()
            })
            .collect();

        // The toolchain.
        let tool_rows: Vec<AnyElement> = report
            .tools
            .iter()
            .enumerate()
            .map(|(i, tool)| {
                let mut badges = div().flex().flex_wrap().gap_1();
                if tool.path.is_none() {
                    badges = badges.child(pill("missing", theme::danger()));
                }
                if let Some(newer) = &tool.outdated {
                    badges = badges.child(pill(format!("→ {newer}"), theme::hue(Hue::Yellow)));
                }
                if let Some(at) = tool.brewed_at {
                    let days = now
                        .duration_since(at)
                        .map(|d| d.as_secs() / 86_400)
                        .unwrap_or(0);
                    if days < RECENT_UPGRADE_DAYS {
                        badges = badges.child(pill(
                            format!("upgraded {}", tools::age(at, now)),
                            theme::hue(Hue::Cyan),
                        ));
                    }
                }
                let action = match (tool.brew, &tool.path, &tool.outdated) {
                    (Some(brew), None, _) => self
                        .tools_run(
                            format!("install {}", tool.name),
                            tools::brew_command(brew, true),
                        )
                        .map(|c| ("Install", c)),
                    (Some(brew), Some(_), Some(_)) => self
                        .tools_run(
                            format!("upgrade {}", tool.name),
                            tools::brew_command(brew, false),
                        )
                        .map(|c| ("Upgrade", c)),
                    _ => None,
                };
                let path = tool.path.as_ref().map(|p| display_path(p));
                row()
                    .child(
                        div()
                            .w(px(150.))
                            .flex_none()
                            .flex()
                            .items_baseline()
                            .gap_2()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(tool.name),
                            )
                            .child(muted(tool.version.clone().unwrap_or_else(|| "—".into()))),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .gap_0p5()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(muted(tool.why))
                                    .child(badges),
                            )
                            .children(path.clone().map(|p| {
                                div()
                                    .text_xs()
                                    .text_color(theme::text_muted())
                                    .truncate()
                                    .child(p)
                            })),
                    )
                    .children(action.map(|(label, command)| {
                        button(("tool-action", i), label).on_click(
                            cx.listener(move |this, _, _, cx| this.run(command.clone(), cx)),
                        )
                    }))
                    .children(tool.path.clone().map(|p| {
                        let text = p.to_string_lossy().into_owned();
                        button(("tool-copy", i), "Copy path").on_click(
                            cx.listener(move |this, _, _, cx| this.copy(text.clone(), cx)),
                        )
                    }))
                    .into_any_element()
            })
            .collect();

        // One-click utilities.
        let utility = |id: &'static str,
                       label: &'static str,
                       make: fn() -> String,
                       cx: &mut Context<Self>| {
            button(id, label).on_click(cx.listener(move |this, _, _, cx| this.copy(make(), cx)))
        };
        let utilities = div()
            .flex()
            .flex_wrap()
            .gap_2()
            .child(utility("util-uuid", "Copy a new UUID", new_uuid, cx))
            .child(utility("util-unix", "Copy Unix time", unix_now, cx))
            .child(utility("util-iso", "Copy UTC time (ISO 8601)", iso_now, cx))
            .child(
                button("util-config", "Open the app's config folder").on_click(cx.listener(
                    |_, _, _, cx| {
                        let dir = crate::tmux::config_path()
                            .parent()
                            .map(|d| d.to_string_lossy().into_owned())
                            .unwrap_or_default();
                        cx.open_url(&format!("file://{dir}"));
                    },
                )),
            );

        div()
            .flex()
            .flex_col()
            .gap_4()
            .child(toolbar)
            .child(section("Services", services))
            .child(section(
                format!("Toolchain ({})", report.tools.len()),
                tool_rows,
            ))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme::text_muted())
                            .child(SharedString::from("UTILITIES")),
                    )
                    .child(utilities),
            )
            .into_any_element()
    }
}

/// A random (v4) UUID, from the kernel's randomness.
fn new_uuid() -> String {
    let mut bytes = [0u8; 16];
    if let Ok(mut f) = std::fs::File::open("/dev/urandom") {
        let _ = std::io::Read::read_exact(&mut f, &mut bytes);
    }
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    format!(
        "{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    )
}

fn unix_now() -> String {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs().to_string())
        .unwrap_or_default()
}

/// `2026-11-01T17:06:00Z`, from the civil-from-days algorithm (no time crate).
fn iso_now() -> String {
    let secs = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    iso_from_unix(secs)
}

pub(super) fn iso_from_unix(secs: i64) -> String {
    let (days, rem) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utilities_make_well_formed_values() {
        // Given / When
        let uuid = new_uuid();
        let epoch = iso_from_unix(0);
        let leap_day = iso_from_unix(951_782_400 + 3_661); // 2000-02-29 01:01:01

        // Then
        assert_eq!(uuid.len(), 36);
        assert_eq!(&uuid[14..15], "4", "{uuid}");
        assert_ne!(uuid, new_uuid());
        assert_eq!(epoch, "1970-01-01T00:00:00Z");
        assert_eq!(leap_day, "2000-02-29T01:01:01Z");
    }
}
