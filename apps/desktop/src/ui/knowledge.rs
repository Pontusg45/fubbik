//! The Knowledge tab: the space's health, an inbox of things that need a
//! decision (stale flags, proposals, enrichment, gaps), requirements and
//! behaviour matrices, feature overlays, and a quick note.

use gpui::{
    AnyElement, Context, FontWeight, IntoElement, SharedString, Window, div, prelude::*, px,
};

use super::palette::Command;
use super::widgets::{button, label, muted, pill, row, section, stat_card};
use super::{Field, Loadable, Workspace};
use crate::fubbik::Knowledge;
use crate::model::Status;
use crate::theme;

impl Workspace {
    /// The repo's knowledge, or a notice saying why there is none.
    pub(super) fn knowledge_or_notice(&self, repo: usize) -> Result<&Knowledge, AnyElement> {
        let notice = |title: String, detail: String| {
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(div().text_sm().child(title))
                .child(muted(detail))
                .into_any_element()
        };
        match &self.repos[repo].knowledge {
            None => Err(muted("Asking fubbik…").into_any_element()),
            Some(Err(error)) => Err(notice(
                format!("fubbik isn't reachable at {}", self.config.fubbik_url()),
                format!(
                    "Start it with `just dev`, or set fubbik_url in {}. ({error})",
                    self.config_path.display()
                ),
            )),
            Some(Ok(Knowledge { space: None, .. })) => Err(notice(
                "No fubbik space matches this repo".into(),
                "Create one from the repo folder with `fubbik space add`.".into(),
            )),
            Some(Ok(knowledge)) => Ok(knowledge),
        }
    }

    pub(super) fn render_knowledge(
        &self,
        repo: usize,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let k = match self.knowledge_or_notice(repo) {
            Ok(k) => k,
            Err(notice) => return notice,
        };
        let space = k.space.clone().expect("checked by knowledge_or_notice");
        let space_path = format!("/spaces/{}", space.id);

        let links =
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(muted(format!("Space: {}", space.name)))
                .child(button("k-space", "Open space").on_click(cx.listener(
                    move |this, _, _, cx| this.run(Command::OpenWeb(space_path.clone()), cx),
                )))
                .child(button("k-dashboard", "Dashboard").on_click(cx.listener(
                    |this, _, _, cx| this.run(Command::OpenWeb("/dashboard".into()), cx),
                )))
                .child(
                    button("k-health", "Health").on_click(cx.listener(|this, _, _, cx| {
                        this.run(Command::OpenWeb("/knowledge-health".into()), cx)
                    })),
                )
                .child(button("k-graph", "Graph").on_click(
                    cx.listener(|this, _, _, cx| this.run(Command::OpenWeb("/graph".into()), cx)),
                ));

        let cards = div()
            .flex()
            .gap_4()
            .child(stat_card("Chunks", k.chunks.to_string()))
            .child(stat_card("Stale", k.stale.to_string()))
            .child(stat_card("Orphans", k.orphans.to_string()))
            .child(stat_card("Thin", k.thin.to_string()))
            .child(stat_card("Active agents", k.active_agents().to_string()));

        div()
            .flex()
            .flex_col()
            .gap_6()
            .child(links)
            .child(cards)
            .child(muted("Drop a file on the window to see why it is the way it is · ⌘K then `?words` searches chunks"))
            .child(self.render_inbox(repo, k, cx))
            .child(self.render_enrichment(repo, k, cx))
            .child(self.render_gaps(repo, cx))
            .child(self.render_quality(k, cx))
            .child(self.render_features(repo, k, cx))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(label("QUICK NOTE"))
                    .child(self.field(Field::NoteTitle, "Title", window, cx))
                    .child(self.field(Field::NoteBody, "Content — enter saves", window, cx))
                    .child(div().flex().child(button("note-save", "Save note").on_click(cx.listener(|this, _, _, cx| {
                        this.submit_note(cx);
                        cx.notify();
                    })))),
            )
            .into_any_element()
    }

    fn render_inbox(
        &self,
        _repo: usize,
        k: &Knowledge,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let mut rows: Vec<AnyElement> = Vec::new();
        for (i, flag) in k.stale_flags.iter().enumerate() {
            let id = |n: &str| SharedString::from(format!("stale-{n}-{i}"));
            let (chunk, flag_id) = (flag.chunk_id.clone(), flag.id.clone());
            rows.push(
                row()
                    .when(
                        self.tab == super::Tab::Knowledge && self.list_cursor == Some(i),
                        |r| r.bg(theme::surface_hover()),
                    )
                    .child(pill("stale", theme::status(Status::Degraded)))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .overflow_hidden()
                            .child(div().text_sm().child(flag.chunk_title.clone()))
                            .child(muted(format!(
                                "{}{}",
                                flag.reason.replace('_', " "),
                                flag.detail
                                    .as_ref()
                                    .map(|d| format!(" — {d}"))
                                    .unwrap_or_default()
                            ))),
                    )
                    .child(button(id("open"), "Open").on_click(cx.listener(
                        move |this, _, _, cx| {
                            this.run(Command::OpenWeb(format!("/chunks/{chunk}")), cx)
                        },
                    )))
                    .child(button(id("dismiss"), "Dismiss").on_click(
                        cx.listener(move |this, _, _, cx| this.dismiss_stale(flag_id.clone(), cx)),
                    ))
                    .into_any_element(),
            );
        }
        for (i, proposal) in k.proposals.iter().enumerate() {
            let id = |n: &str| SharedString::from(format!("proposal-{n}-{i}"));
            let chunk = proposal.chunk_id.clone();
            let (approve, reject) = (proposal.id.clone(), proposal.id.clone());
            let mut detail = format!("changes {}", proposal.fields.join(", "));
            if let Some(reason) = &proposal.reason {
                detail.push_str(&format!(" — {reason}"));
            }
            let at = k.stale_flags.len() + i;
            rows.push(
                row()
                    .when(
                        self.tab == super::Tab::Knowledge && self.list_cursor == Some(at),
                        |r| r.bg(theme::surface_hover()),
                    )
                    .child(pill("proposal", theme::accent()))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .overflow_hidden()
                            .child(div().text_sm().child(proposal.chunk_title.clone()))
                            .child(muted(detail)),
                    )
                    .child(button(id("open"), "Open").on_click(cx.listener(
                        move |this, _, _, cx| {
                            this.run(Command::OpenWeb(format!("/chunks/{chunk}")), cx)
                        },
                    )))
                    .child(button(id("approve"), "Approve").on_click(
                        cx.listener(move |this, _, _, cx| this.review(approve.clone(), true, cx)),
                    ))
                    .child(
                        button(id("reject"), "Reject")
                            .text_color(theme::danger())
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.review(reject.clone(), false, cx)
                            })),
                    )
                    .into_any_element(),
            );
        }
        let title = format!(
            "Needs attention ({} stale · {} proposals pending, all spaces)",
            k.stale_flags.len(),
            k.proposals_pending
        );
        section(title, rows)
    }

    fn render_enrichment(
        &self,
        repo: usize,
        k: &Knowledge,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let ollama = self.repos[repo]
            .snapshot
            .as_ref()
            .and_then(|s| s.processes.iter().find(|p| p.spec.name == "ollama"))
            .map(|p| p.status);
        let ollama_text = match ollama {
            Some(status) if status.is_up() => "Ollama is up".to_string(),
            Some(_) => "Ollama is stopped — start it in Processes first".to_string(),
            None => {
                "Ollama is not in fubbik.services.toml; enrichment needs it running".to_string()
            }
        };
        section(
            "Enrichment",
            vec![
                row()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .child(div().text_sm().child(format!(
                                "{} chunks not fully enriched (summary, aliases or embedding missing) · {} embeddings older than their chunk",
                                k.unenriched, k.stale_embeddings
                            )))
                            .child(muted(ollama_text)),
                    )
                    .child(
                        button("enrich-all", "Enrich all")
                            .on_click(cx.listener(|this, _, _, cx| this.enrich_all(cx))),
                    )
                    .into_any_element(),
            ],
        )
    }

    fn render_gaps(&self, repo: usize, cx: &mut Context<Self>) -> impl IntoElement {
        let scan = button("gaps-scan", "Scan for gaps")
            .on_click(cx.listener(move |this, _, _, cx| this.scan_gaps(repo, cx)));
        let mut rows: Vec<AnyElement> = Vec::new();
        let summary = match &self.repos[repo].gaps {
            None => "Source files no chunk covers (like `fubbik gaps`). One request per file, up to 400.".to_string(),
            Some(Loadable::Loading) => "Scanning…".to_string(),
            Some(Loadable::Done(Err(error))) => format!("Scan failed: {error}"),
            Some(Loadable::Done(Ok(report))) => {
                for (dir, files) in report.by_directory.iter().take(10) {
                    let sample: Vec<&str> = files.iter().take(3).map(String::as_str).collect();
                    rows.push(
                        row()
                            .child(div().w(px(56.)).child(div().text_sm().font_weight(FontWeight::MEDIUM).child(files.len().to_string())))
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .flex_1()
                                    .overflow_hidden()
                                    .child(div().text_sm().child(dir.clone()))
                                    .child(muted(sample.join(", "))),
                            )
                            .into_any_element(),
                    );
                }
                format!(
                    "{}% covered — {} of {} files{}",
                    report.coverage(),
                    report.covered,
                    report.total,
                    if report.truncated { " (first 400 only)" } else { "" }
                )
            }
        };
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(label("GAPS"))
                    .child(scan)
                    .child(muted(summary)),
            )
            .when(!rows.is_empty(), |d| {
                d.child(section("Uncovered, by directory", rows))
            })
    }

    fn render_quality(&self, k: &Knowledge, cx: &mut Context<Self>) -> impl IntoElement {
        let req = &k.requirements;
        let requirement_rows: Vec<AnyElement> = k
            .open_requirements
            .iter()
            .enumerate()
            .map(|(i, r)| {
                let id = |n: &str| SharedString::from(format!("req-{n}-{i}"));
                let color = if r.status == "failing" {
                    theme::danger()
                } else {
                    theme::text_muted()
                };
                let (pass, fail) = (r.id.clone(), r.id.clone());
                row()
                    .child(div().w(px(80.)).child(pill(r.status.clone(), color)))
                    .child(
                        div()
                            .flex_1()
                            .overflow_hidden()
                            .text_sm()
                            .child(r.title.clone()),
                    )
                    .child(muted(r.priority.clone().unwrap_or_default()))
                    .child(button(id("pass"), "Mark passing").on_click(cx.listener(
                        move |this, _, _, cx| this.set_requirement(pass.clone(), "passing", cx),
                    )))
                    .child(button(id("fail"), "Mark failing").on_click(cx.listener(
                        move |this, _, _, cx| this.set_requirement(fail.clone(), "failing", cx),
                    )))
                    .into_any_element()
            })
            .collect();
        let matrix_rows: Vec<AnyElement> =
            k.matrices
                .iter()
                .enumerate()
                .map(|(i, m)| {
                    let matrix = m.id.clone();
                    row()
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .flex_1()
                                .child(div().text_sm().child(m.name.clone()))
                                .child(muted(format!(
                                    "{} · {} verified · {} specified · {} gaps",
                                    m.layer, m.verified, m.specified, m.unspecified
                                ))),
                        )
                        .when(m.violated > 0, |r| {
                            r.child(pill(format!("{} violated", m.violated), theme::danger()))
                        })
                        .child(
                            button(SharedString::from(format!("matrix-open-{i}")), "Open")
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.run(Command::OpenWeb(format!("/matrices/{matrix}")), cx)
                                })),
                        )
                        .into_any_element()
                })
                .collect();
        div()
            .flex()
            .flex_col()
            .gap_4()
            .child(section(
                format!(
                    "Requirements — {} passing · {} failing · {} untested",
                    req.passing, req.failing, req.untested
                ),
                requirement_rows,
            ))
            .child(section("Behaviour matrices", matrix_rows))
            .child(muted(
                "Test processes with a [process.report] table in fubbik.services.toml report their pass/fail here when they exit.",
            ))
    }

    fn render_features(
        &self,
        repo: usize,
        k: &Knowledge,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let rows: Vec<AnyElement> = k
            .features
            .iter()
            .filter(|f| matches!(f.status.as_str(), "active" | "inactive"))
            .enumerate()
            .map(|(i, f)| {
                let on = k.active_features.contains(&f.id);
                let feature = f.id.clone();
                row()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .child(div().text_sm().child(f.name.clone()))
                            .child(muted(format!("{} changed chunks", f.delta_count))),
                    )
                    .child(pill(
                        if on { "on" } else { "off" },
                        if on {
                            theme::accent()
                        } else {
                            theme::text_muted()
                        },
                    ))
                    .child(
                        button(
                            SharedString::from(format!("feature-{i}")),
                            if on { "Turn off" } else { "Turn on" },
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.toggle_feature(repo, feature.clone(), cx)
                        })),
                    )
                    .into_any_element()
            })
            .collect();
        section(
            "Feature overlays (your active set, used in all context)",
            rows,
        )
    }
}
