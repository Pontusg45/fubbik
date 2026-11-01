//! The Setup tab: `fubbik doctor` and more — the server, the Rust binary,
//! the MCP server in Claude Code's config, and each worktree's pre-commit
//! hook and generated CLAUDE.md.

use gpui::{AnyElement, Context, Hsla, IntoElement, SharedString, div, prelude::*, px};

use super::widgets::{button, muted, pill, row, section, section_or};
use super::{Workspace, display_path};
use crate::claude_md::MdState;
use crate::model::Status;
use crate::setup::{self, HookStatus, McpStatus};
use crate::theme;

fn tone(ok: bool) -> Hsla {
    if ok {
        theme::status(Status::Running)
    } else {
        theme::status(Status::Degraded)
    }
}

fn check(title: &str, ok: bool, detail: String) -> gpui::Div {
    row()
        .child(
            div()
                .w(px(70.))
                .child(pill(if ok { "ok" } else { "fix" }, tone(ok))),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .flex_1()
                .overflow_hidden()
                .child(div().text_sm().child(title.to_string()))
                .child(muted(detail)),
        )
}

impl Workspace {
    pub(super) fn render_setup(
        &self,
        repo: usize,
        window: &gpui::Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let view = &self.repos[repo];
        let global = self.global.as_ref();

        // ── doctor ──
        let (server_ok, server_detail) = match global.map(|g| &g.api) {
            None => (false, "checking…".to_string()),
            Some(Ok((health, features))) => (
                health.status == "ok" && health.db == "connected",
                format!(
                    "{} — {} · db {} · groups: {}",
                    self.config.fubbik_url(),
                    health.status,
                    health.db,
                    features.join(", ")
                ),
            ),
            Some(Err(error)) => (false, format!("{} — {error}", self.config.fubbik_url())),
        };
        let bin = global.and_then(|g| g.fubbik_bin.clone());
        let checkout = self
            .repos
            .iter()
            .map(|r| r.entry.path.clone())
            .find(|p| setup::is_fubbik_checkout(p));
        let mut bin_row = check(
            "Rust fubbik binary",
            bin.is_some(),
            match &bin {
                Some(path) => display_path(path),
                None => "not found. The `fubbik` on PATH may be the legacy TypeScript CLI, which has no `mcp` command. Set fubbik_bin in the desktop config, or build it.".into(),
            },
        );
        if bin.is_none()
            && let Some(checkout) = checkout
        {
            bin_row = bin_row.child(button("build-fubbik", "Build fubbik").on_click(
                cx.listener(move |this, _, _, cx| this.build_fubbik(checkout.clone(), cx)),
            ));
        }
        let mut server_row = check("fubbik server", server_ok, server_detail);
        if server_ok {
            server_row = server_row.child(button("open-web", "Open web app").on_click(
                cx.listener(|this, _, _, cx| {
                    this.run(super::palette::Command::OpenWeb("/dashboard".into()), cx)
                }),
            ));
        } else {
            // Something to do about it, not just a link.
            server_row = server_row
                .child(button("server-retry", "Retry").on_click(
                    cx.listener(|this, _, _, cx| this.run(super::palette::Command::Refresh, cx)),
                ))
                .child(button("server-logs", "Show logs").on_click(cx.listener(
                    |this, _, _, cx| {
                        this.log_key = Some("builtin::server".into());
                        this.log_paused = None;
                        this.run(super::palette::Command::ShowTab(super::Tab::Logs), cx)
                    },
                )))
                .when(!self.config.embedded_backend(), |r| {
                    r.child(button("server-on", "Turn on the built-in server").on_click(
                        cx.listener(|this, _, _, cx| {
                            this.run(super::palette::Command::SetEmbeddedBackend(true), cx)
                        }),
                    ))
                });
        }

        let setup_state = view.setup.as_ref();
        let mcp = setup_state.map(|s| s.mcp.clone());
        let mut mcp_row = check(
            "fubbik MCP server for Claude Code",
            matches!(mcp, Some(McpStatus::Project | McpStatus::User)),
            match mcp {
                Some(McpStatus::Project) => "in this repo's .mcp.json".into(),
                Some(McpStatus::User) => "in ~/.claude.json".into(),
                Some(McpStatus::Missing) => {
                    "missing: sessions here cannot use join_board, get_context, …".into()
                }
                None => "checking…".into(),
            },
        );
        if mcp == Some(McpStatus::Missing) {
            mcp_row = mcp_row.child(
                button("install-mcp", "Add to .mcp.json")
                    .on_click(cx.listener(move |this, _, _, cx| this.install_mcp(repo, cx))),
            );
        }

        // ── per worktree ──
        let mut worktree_rows: Vec<AnyElement> = Vec::new();
        if let Some(state) = setup_state {
            for (i, (root, hook)) in state.hooks.iter().enumerate() {
                let id = |n: &str| SharedString::from(format!("setup-{n}-{i}"));
                let md = state.claude_md.as_ref().and_then(|sync| {
                    sync.worktrees
                        .iter()
                        .find(|(r, _)| r == root)
                        .map(|(_, s)| *s)
                });
                let mut r = row()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .overflow_hidden()
                            .child(div().text_sm().child(display_path(root)))
                            .child(muted(match hook {
                                HookStatus::Installed => "pre-commit hook: fubbik check-files",
                                HookStatus::Other => {
                                    "pre-commit hook: another hook is installed (left alone)"
                                }
                                HookStatus::Missing => "pre-commit hook: none",
                            })),
                    )
                    .child(pill(
                        match hook {
                            HookStatus::Installed => "hook",
                            HookStatus::Other => "other hook",
                            HookStatus::Missing => "no hook",
                        },
                        tone(*hook == HookStatus::Installed),
                    ));
                if *hook == HookStatus::Missing {
                    let root = root.clone();
                    r = r.child(button(id("hook"), "Install hook").on_click(
                        cx.listener(move |this, _, _, cx| this.install_hook(root.clone(), cx)),
                    ));
                }
                if let Some(md) = md {
                    r = r.child(pill(
                        format!("CLAUDE.md {}", md.label()),
                        tone(md == MdState::InSync),
                    ));
                    if md != MdState::InSync {
                        let root = root.clone();
                        r = r.child(button(id("sync"), "Sync CLAUDE.md").on_click(cx.listener(
                            move |this, _, _, cx| this.sync_claude_md(repo, root.clone(), cx),
                        )));
                    }
                }
                worktree_rows.push(r.into_any_element());
            }
        }
        let claude_md_note = match setup_state.and_then(|s| s.claude_md.as_ref()) {
            None => "CLAUDE.md sync is off: add a `claudeMd` section to fubbik.config.json (output, tag, maxTokens) to turn it on.".to_string(),
            Some(sync) => match &sync.generated {
                Ok(_) => format!("CLAUDE.md: {} from chunks tagged `{}`", sync.config.output.display(), sync.config.tag),
                Err(error) => format!("CLAUDE.md could not be generated: {error}"),
            },
        };

        div()
            .flex()
            .flex_col()
            .gap_6()
            // What is broken first; preferences after.
            .child(section(
                "Doctor",
                vec![
                    server_row.into_any_element(),
                    bin_row.into_any_element(),
                    mcp_row.into_any_element(),
                ],
            ))
            .child(section_or(
                "Worktrees: hooks and CLAUDE.md",
                worktree_rows,
                if setup_state.is_none() {
                    "Checking…"
                } else {
                    "No worktrees"
                },
            ))
            .child(muted(claude_md_note))
            .child(self.render_preferences(window, cx))
            .child(self.render_keyboard_setup(cx))
    }

    /// Keyboard: where keymap.toml is, what is wrong in it, and clashes.
    fn render_keyboard_setup(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let path = super::keymap::path();
        let problems = super::keymap::problems();
        let mut rows: Vec<AnyElement> = vec![
            row()
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .flex_1()
                        .child(div().text_sm().child("Key bindings"))
                        .child(muted(format!(
                            "{} — changes apply on the next launch; ⌘/ lists them all",
                            display_path(&path)
                        ))),
                )
                .child(
                    button("keymap-open", "Open keymap.toml")
                        .on_click(cx.listener(|this, _, _, cx| this.open_keymap_file(cx))),
                )
                .into_any_element(),
        ];
        rows.extend(problems.iter().map(|problem| {
            row()
                .child(div().w(px(70.)).child(pill("fix", tone(false))))
                .child(div().text_sm().child(problem.clone()))
                .into_any_element()
        }));
        section("Keyboard", rows)
    }
}
