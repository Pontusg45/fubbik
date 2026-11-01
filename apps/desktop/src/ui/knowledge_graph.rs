//! The Knowledge tab's graph: the repo's space as chunks (nodes, colored by
//! type, sized by how connected they are) and connections (edges), laid out
//! by `graph_layout.rs`. Hover a node to light up its neighbours; click to
//! select it and see its connections below, with "Open" for the chunk.

use std::collections::HashSet;

use gpui::{AnyElement, FontWeight, PathBuilder, SharedString, canvas, point, relative};

use super::widgets::{button, loading, muted, segment, switch};
use super::*;
use crate::graph_layout::Graph;
use crate::theme::Hue;

const GRAPH_HEIGHT: f32 = 520.;
/// Above this many nodes, only well-connected ones get a label by default.
const LABEL_ALL_UNDER: usize = 40;

/// The graph's view state (one repo at a time).
#[derive(Default)]
pub struct GraphState {
    /// The space it was loaded for, and the graph.
    pub loaded: Option<(String, Loadable<Graph>)>,
    pub hover: Option<usize>,
    pub selected: Option<usize>,
    pub hidden: HashSet<String>,
    pub labels: bool,
}

fn kind_color(kind: &str) -> gpui::Hsla {
    match kind {
        "note" => theme::hue(Hue::Blue),
        "document" => theme::hue(Hue::Green),
        "reference" => theme::hue(Hue::Cyan),
        "schema" => theme::hue(Hue::Violet),
        "checklist" => theme::hue(Hue::Orange),
        "convention" => theme::hue(Hue::Yellow),
        "guide" => theme::hue(Hue::Pink),
        _ => theme::hue(Hue::Lime),
    }
}

/// A node's diameter: bigger when more connected.
fn node_size(degree: usize) -> f32 {
    8. + degree.min(12) as f32 * 1.4
}

impl Workspace {
    /// Loads the selected repo's graph when its space has none loaded yet.
    pub(super) fn ensure_graph(&mut self, repo: usize, force: bool, cx: &mut Context<Self>) {
        let Some(space) = self
            .repos
            .get(repo)
            .and_then(|r| r.knowledge.as_ref())
            .and_then(|k| k.as_ref().ok())
            .and_then(|k| k.space.as_ref())
            .map(|s| s.id.clone())
        else {
            return;
        };
        let current = matches!(&self.graph.loaded, Some((id, _)) if *id == space);
        if current && !force {
            return;
        }
        if !current {
            self.graph = GraphState {
                labels: self.graph.labels,
                ..Default::default()
            };
        }
        self.graph.loaded = Some((space.clone(), Loadable::Loading));
        let client = self.client.clone();
        let work = {
            let space = space.clone();
            cx.background_executor()
                .spawn(async move { client.graph(&space) })
        };
        cx.spawn(async move |this, cx| {
            let result = work.await;
            this.update(cx, |this, cx| {
                if matches!(&this.graph.loaded, Some((id, _)) if *id == space) {
                    this.graph.loaded = Some((space, Loadable::Done(result)));
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    pub(super) fn render_knowledge_graph(&self, cx: &mut Context<Self>) -> AnyElement {
        let graph = match &self.graph.loaded {
            Some((_, Loadable::Done(Ok(graph)))) => graph.clone(),
            Some((_, Loadable::Done(Err(error)))) => {
                return muted(format!("The graph could not be read: {error}")).into_any_element();
            }
            _ => return loading("Laying out the knowledge graph…").into_any_element(),
        };
        if graph.nodes.is_empty() {
            return muted("This space has no chunks yet").into_any_element();
        }

        // Which nodes show (types can be hidden), and who is lit up.
        let visible: Vec<bool> = graph
            .nodes
            .iter()
            .map(|n| !self.graph.hidden.contains(&n.kind))
            .collect();
        let focus = self.graph.hover.or(self.graph.selected);
        let lit: HashSet<usize> = focus
            .map(|f| {
                let mut set: HashSet<usize> = graph.neighbours(f).into_iter().collect();
                set.insert(f);
                set
            })
            .unwrap_or_default();

        // Type chips: count, color, click to hide or show.
        let mut kinds: Vec<(String, usize)> = Vec::new();
        for node in &graph.nodes {
            match kinds.iter_mut().find(|(k, _)| *k == node.kind) {
                Some((_, n)) => *n += 1,
                None => kinds.push((node.kind.clone(), 1)),
            }
        }
        kinds.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
        let chips = kinds.into_iter().map(|(kind, count)| {
            let shown = !self.graph.hidden.contains(&kind);
            let color = kind_color(&kind);
            let id = SharedString::from(format!("graph-kind-{kind}"));
            let label = format!("{kind} {count}");
            segment(id, label, shown)
                .text_color(if shown { color } else { theme::text_muted() })
                .on_click(cx.listener(move |this, _, _, cx| {
                    if !this.graph.hidden.remove(&kind) {
                        this.graph.hidden.insert(kind.clone());
                    }
                    cx.notify();
                }))
        });
        let toolbar = div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap_2()
            .child(
                div()
                    .text_xs()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(theme::text_muted())
                    .child(format!(
                        "KNOWLEDGE GRAPH · {} CHUNKS · {} CONNECTIONS",
                        graph.nodes.len(),
                        graph.edges.len()
                    )),
            )
            .child(div().flex_1())
            .children(chips)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(muted("Labels"))
                    .child(
                        switch("graph-labels", self.graph.labels).on_click(cx.listener(
                            |this, _, _, cx| {
                                this.graph.labels = !this.graph.labels;
                                cx.notify();
                            },
                        )),
                    ),
            )
            .child(
                button("graph-refresh", "Refresh").on_click(cx.listener(|this, _, _, cx| {
                    this.ensure_graph(this.selected, true, cx);
                    cx.notify();
                })),
            )
            .child(button("graph-web", "Open in web").on_click(
                cx.listener(|this, _, _, cx| this.run(Command::OpenWeb("/graph".into()), cx)),
            ));

        // Edges, drawn on a canvas behind the nodes.
        let edges: Vec<(f32, f32, f32, f32, bool)> = graph
            .edges
            .iter()
            .filter(|e| visible[e.source] && visible[e.target])
            .map(|e| {
                let (a, b) = (&graph.nodes[e.source], &graph.nodes[e.target]);
                let on = focus.is_some_and(|f| e.source == f || e.target == f);
                (a.x, a.y, b.x, b.y, on)
            })
            .collect();
        let (dim, bright) = (
            theme::text_muted().opacity(0.25),
            theme::accent().opacity(0.9),
        );
        let dimmed = focus.is_some();
        let lines = canvas(
            move |_, _, _| (),
            move |bounds, (), window, _| {
                let at = |x: f32, y: f32| {
                    point(
                        bounds.origin.x + bounds.size.width * x,
                        bounds.origin.y + bounds.size.height * y,
                    )
                };
                // Faint ones first, the lit ones on top.
                for pass in [false, true] {
                    for (x1, y1, x2, y2, on) in &edges {
                        if *on != pass {
                            continue;
                        }
                        let mut path = PathBuilder::stroke(px(if *on { 1.6 } else { 1. }));
                        path.move_to(at(*x1, *y1));
                        path.line_to(at(*x2, *y2));
                        if let Ok(path) = path.build() {
                            let color = if *on {
                                bright
                            } else if dimmed {
                                dim.opacity(0.12)
                            } else {
                                dim
                            };
                            window.paint_path(path, color);
                        }
                    }
                }
            },
        )
        .absolute()
        .inset_0();

        let label_all = self.graph.labels || graph.nodes.len() < LABEL_ALL_UNDER;
        let nodes = graph
            .nodes
            .iter()
            .enumerate()
            .filter(|(i, _)| visible[*i])
            .map(|(i, node)| {
                let size = node_size(node.degree);
                let color = kind_color(&node.kind);
                let faded = dimmed && !lit.contains(&i);
                let show_label = lit.contains(&i) || (!dimmed && (label_all || node.degree >= 4));
                let selected = self.graph.selected == Some(i);
                div()
                    .id(("graph-node", i))
                    .absolute()
                    .left(relative(node.x))
                    .top(relative(node.y))
                    .ml(px(-size / 2.))
                    .mt(px(-size / 2.))
                    .flex()
                    .items_center()
                    .gap_1()
                    .cursor_pointer()
                    .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                        let next = if *hovered { Some(i) } else { None };
                        if this.graph.hover != next && (*hovered || this.graph.hover == Some(i)) {
                            this.graph.hover = next;
                            cx.notify();
                        }
                    }))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.graph.selected = if this.graph.selected == Some(i) {
                            None
                        } else {
                            Some(i)
                        };
                        cx.notify();
                    }))
                    .child(
                        div()
                            .size(px(size))
                            .flex_none()
                            .rounded_full()
                            .bg(color.opacity(if faded { 0.2 } else { 0.9 }))
                            .border_2()
                            .border_color(if selected {
                                theme::text()
                            } else {
                                color.opacity(0.0)
                            }),
                    )
                    .when(show_label, |d| {
                        d.child(
                            div()
                                .max_w(px(180.))
                                .truncate()
                                .px_1()
                                .rounded_sm()
                                .bg(theme::surface().opacity(0.85))
                                .text_xs()
                                .text_color(if lit.contains(&i) {
                                    theme::text()
                                } else {
                                    theme::text_muted()
                                })
                                .child(node.title.clone()),
                        )
                    })
            });

        let area = div()
            .id("knowledge-graph")
            .relative()
            .w_full()
            .h(px(GRAPH_HEIGHT))
            .rounded_lg()
            .border_1()
            .border_color(theme::border())
            .bg(theme::surface())
            .overflow_hidden()
            .child(lines)
            .children(nodes);

        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(toolbar)
            .child(area)
            .children(
                self.graph
                    .selected
                    .and_then(|s| graph.nodes.get(s).map(|_| s))
                    .map(|s| self.render_graph_selection(&graph, s, cx)),
            )
            .into_any_element()
    }

    /// Under the graph: the selected chunk and what it connects to.
    fn render_graph_selection(
        &self,
        graph: &Graph,
        s: usize,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let node = &graph.nodes[s];
        let id = node.id.clone();
        let mut links: Vec<(usize, String, bool)> = graph
            .edges
            .iter()
            .filter_map(|e| {
                if e.source == s {
                    Some((e.target, e.relation.replace('_', " "), true))
                } else if e.target == s {
                    Some((e.source, e.relation.replace('_', " "), false))
                } else {
                    None
                }
            })
            .collect();
        links.sort_by_key(|(n, _, _)| graph.nodes[*n].title.clone());
        div()
            .flex()
            .flex_col()
            .gap_1()
            .p_3()
            .rounded_lg()
            .border_1()
            .border_color(theme::border())
            .bg(theme::elevated())
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .size(px(10.))
                            .rounded_full()
                            .bg(kind_color(&node.kind)),
                    )
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(node.title.clone()),
                    )
                    .child(muted(node.kind.clone()))
                    .child(div().flex_1())
                    .child(button("graph-open", "Open").on_click(cx.listener(
                        move |this, _, _, cx| {
                            this.run(Command::OpenWeb(format!("/chunks/{id}")), cx)
                        },
                    ))),
            )
            .children(if links.is_empty() {
                vec![muted("No connections: an orphan chunk").into_any_element()]
            } else {
                links
                    .into_iter()
                    .enumerate()
                    .map(|(k, (other, relation, outgoing))| {
                        let title = graph.nodes[other].title.clone();
                        let arrow = if outgoing {
                            format!("→ {relation}")
                        } else {
                            format!("← {relation}")
                        };
                        div()
                            .id(("graph-link", k))
                            .flex()
                            .gap_2()
                            .text_sm()
                            .cursor_pointer()
                            .hover(|d| d.text_color(theme::accent()))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.graph.selected = Some(other);
                                cx.notify();
                            }))
                            .child(
                                div()
                                    .w(px(150.))
                                    .text_color(theme::text_muted())
                                    .child(arrow),
                            )
                            .child(title)
                            .into_any_element()
                    })
                    .collect()
            })
            .into_any_element()
    }
}
