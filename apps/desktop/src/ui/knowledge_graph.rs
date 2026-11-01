//! The Knowledge tab's graph: the repo's space as chunks (nodes, colored by
//! type, sized by how connected they are) and connections (edges), laid out
//! by `graph_layout.rs`. Hover a node to light up its neighbours; click to
//! select it and see its connections below, with "Open" for the chunk.
//!
//! "3D" lays the same graph out in three dimensions and projects it with
//! perspective: drag to turn it, scroll to zoom; it turns slowly on its own
//! (Auto-rotate) while nothing is hovered. Nearer chunks are bigger, brighter
//! and drawn on top.

use std::cell::Cell;
use std::collections::HashSet;
use std::rc::Rc;
use std::time::Duration;

use gpui::{
    AnyElement, FontWeight, MouseButton, MouseDownEvent, MouseMoveEvent, PathBuilder,
    ScrollWheelEvent, SharedString, Task, canvas, point, relative,
};

use super::widgets::{button, loading, muted, segment, switch};
use super::*;
use crate::graph_layout::{Camera, Graph, project};
use crate::theme::Hue;

const GRAPH_HEIGHT: f32 = 520.;
/// Above this many nodes, only well-connected ones get a label by default.
const LABEL_ALL_UNDER: usize = 40;

/// One frame of auto-rotation.
const SPIN_EVERY: Duration = Duration::from_millis(33);
const SPIN_STEP: f32 = 0.006;

/// The graph's view state (one repo at a time).
pub struct GraphState {
    /// The space it was loaded for, and the graph.
    pub loaded: Option<(String, Loadable<Graph>)>,
    pub hover: Option<usize>,
    pub selected: Option<usize>,
    pub hidden: HashSet<String>,
    pub labels: bool,
    pub three_d: bool,
    /// Color by what needs attention instead of by type.
    pub by_attention: bool,
    /// A connection being made: from, to, and the relation picked.
    pub link: Option<(usize, usize)>,
    pub link_relation: &'static str,
    pub yaw: f32,
    pub pitch: f32,
    pub zoom: f32,
    pub auto_rotate: bool,
    /// Where a drag that turns the 3D graph was last.
    pub drag: Option<gpui::Point<gpui::Pixels>>,
    /// The area's size at the last paint (width, height), for the aspect.
    pub area: Rc<Cell<(f32, f32)>>,
    pub spin: Option<Task<()>>,
}

impl Default for GraphState {
    fn default() -> Self {
        Self {
            loaded: None,
            hover: None,
            selected: None,
            hidden: HashSet::new(),
            labels: false,
            three_d: false,
            by_attention: false,
            link: None,
            link_relation: "related_to",
            yaw: 0.6,
            pitch: -0.35,
            zoom: 1.0,
            auto_rotate: true,
            drag: None,
            area: Rc::new(Cell::new((1000., GRAPH_HEIGHT))),
            spin: None,
        }
    }
}

/// Where a node is drawn: as fractions of the area, its diameter, how
/// opaque, and its depth (for drawing order; 0 in 2D).
#[derive(Clone, Copy)]
struct Place {
    fx: f32,
    fy: f32,
    size: f32,
    alpha: f32,
    depth: f32,
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

/// The relations a connection can have.
const RELATIONS: [&str; 8] = [
    "related_to",
    "part_of",
    "depends_on",
    "extends",
    "references",
    "supports",
    "contradicts",
    "alternative_to",
];

/// Attention coloring: flagged red, stale amber, thin orange, orphan grey,
/// healthy green.
fn attention_color(node: &crate::graph_layout::Node) -> gpui::Hsla {
    if node.attention.iter().any(|a| a == "flagged") {
        theme::danger()
    } else if node.attention.iter().any(|a| a == "stale") {
        theme::hue(Hue::Yellow)
    } else if node.attention.iter().any(|a| a == "thin") {
        theme::hue(Hue::Orange)
    } else if node.degree == 0 {
        theme::text_muted()
    } else {
        theme::hue(Hue::Green)
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
            let previous = std::mem::take(&mut self.graph);
            self.graph = GraphState {
                labels: previous.labels,
                three_d: previous.three_d,
                auto_rotate: previous.auto_rotate,
                ..Default::default()
            };
            if self.graph.three_d {
                self.start_spin(cx);
            }
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

    /// Turns the 3D graph a little every frame while it is on screen,
    /// auto-rotate is on, and nothing is hovered or dragged.
    fn start_spin(&mut self, cx: &mut Context<Self>) {
        self.graph.spin = Some(cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(SPIN_EVERY).await;
                let keep = this
                    .update(cx, |this, cx| {
                        let g = &mut this.graph;
                        let showing = !this.home && this.tab == Tab::Knowledge;
                        if showing && g.auto_rotate && g.drag.is_none() && g.hover.is_none() {
                            g.yaw += SPIN_STEP;
                            cx.notify();
                        }
                        g.three_d
                    })
                    .unwrap_or(false);
                if !keep {
                    break;
                }
            }
        }));
    }

    fn set_three_d(&mut self, on: bool, cx: &mut Context<Self>) {
        self.graph.three_d = on;
        if on {
            self.start_spin(cx);
        } else {
            self.graph.spin = None;
        }
        cx.notify();
    }

    /// Each node's place on screen, by the current mode and camera.
    fn graph_places(&self, graph: &Graph) -> Vec<Place> {
        if !self.graph.three_d {
            return graph
                .nodes
                .iter()
                .map(|n| Place {
                    fx: n.x,
                    fy: n.y,
                    size: node_size(n.degree),
                    alpha: 1.0,
                    depth: 0.0,
                })
                .collect();
        }
        let (w, h) = self.graph.area.get();
        let camera = Camera {
            yaw: self.graph.yaw,
            pitch: self.graph.pitch,
            zoom: self.graph.zoom,
            aspect: (w / h.max(1.0)).max(0.1),
        };
        graph
            .nodes
            .iter()
            .map(|n| {
                let p = project(n.p3, camera);
                Place {
                    fx: p.fx,
                    fy: p.fy,
                    size: node_size(n.degree) * p.scale * self.graph.zoom.sqrt(),
                    // Far chunks fade into the background.
                    alpha: (0.3 + 0.7 * (p.depth + 1.0) / 2.0).clamp(0.2, 1.0),
                    depth: p.depth,
                }
            })
            .collect()
    }

    /// Nodes whose title contains the search text (none without one).
    fn graph_matches(&self, graph: &Graph) -> Vec<usize> {
        let query = self.graph_search.text.trim().to_lowercase();
        if query.is_empty() {
            return Vec::new();
        }
        (0..graph.nodes.len())
            .filter(|&i| graph.nodes[i].title.to_lowercase().contains(&query))
            .collect()
    }

    /// ⏎ in the search: select the first match and, in 3D, turn the camera
    /// so it faces you.
    pub(super) fn graph_fly_to_match(&mut self, cx: &mut Context<Self>) {
        let Some((_, Loadable::Done(Ok(graph)))) = &self.graph.loaded else {
            return;
        };
        let Some(&first) = self.graph_matches(graph).first() else {
            self.show_info("No chunk in the graph matches");
            return;
        };
        let [x, y, z] = graph.nodes[first].p3;
        self.graph.selected = Some(first);
        if self.graph.three_d {
            let r = (x * x + z * z).sqrt();
            self.graph.yaw = (-x).atan2(z);
            self.graph.pitch = y.atan2(r).clamp(-1.45, 1.45);
            self.graph.auto_rotate = false;
        }
        cx.notify();
    }

    /// Creates the pending connection through the API, then adds it here.
    fn create_link(&mut self, cx: &mut Context<Self>) {
        let (Some((from, to)), Some((_, Loadable::Done(Ok(graph))))) =
            (self.graph.link, &self.graph.loaded)
        else {
            return;
        };
        let (source, target) = (graph.nodes[from].id.clone(), graph.nodes[to].id.clone());
        let relation = self.graph.link_relation;
        let client = self.client.clone();
        let work = cx
            .background_executor()
            .spawn(async move { client.connect(&source, &target, relation) });
        cx.spawn(async move |this, cx| {
            let result = work.await;
            this.update(cx, |this, cx| {
                match result {
                    Ok(()) => {
                        if let Some((_, Loadable::Done(Ok(graph)))) = &mut this.graph.loaded {
                            graph.connect(from, to, relation);
                            let (a, b) = (
                                graph.nodes[from].title.clone(),
                                graph.nodes[to].title.clone(),
                            );
                            this.show_info(format!(
                                "Connected {a} → {b} ({})",
                                relation.replace('_', " ")
                            ));
                        }
                        this.graph.link = None;
                    }
                    Err(error) => this.show_error(format!("Could not connect: {error}")),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub(super) fn render_knowledge_graph(
        &self,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
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
        // A search lights up its matches; else hover or selection lights a
        // chunk and its neighbours.
        let matches = self.graph_matches(&graph);
        let focus = self.graph.hover.or(self.graph.selected);
        let lit: HashSet<usize> = if !matches.is_empty() && self.graph.hover.is_none() {
            matches.iter().copied().chain(self.graph.selected).collect()
        } else {
            focus
                .map(|f| {
                    let mut set: HashSet<usize> = graph.neighbours(f).into_iter().collect();
                    set.insert(f);
                    set
                })
                .unwrap_or_default()
        };
        let searching = !matches.is_empty();

        // Type chips: count, color, click to hide or show.
        let mut kinds: Vec<(String, usize)> = Vec::new();
        for node in &graph.nodes {
            match kinds.iter_mut().find(|(k, _)| *k == node.kind) {
                Some((_, n)) => *n += 1,
                None => kinds.push((node.kind.clone(), 1)),
            }
        }
        kinds.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
        let chips = kinds
            .into_iter()
            .map(|(kind, count)| {
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
            })
            .collect::<Vec<_>>();
        let toolbar =
            div()
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
                .child(div().w(px(220.)).child(self.field(
                    Field::GraphSearch,
                    "Find a chunk — ⏎ goes to it",
                    window,
                    cx,
                )))
                .child(muted("Color"))
                .child(
                    segment("graph-by-type", "Type", !self.graph.by_attention).on_click(
                        cx.listener(|this, _, _, cx| {
                            this.graph.by_attention = false;
                            cx.notify();
                        }),
                    ),
                )
                .child(
                    segment("graph-by-attention", "Attention", self.graph.by_attention).on_click(
                        cx.listener(|this, _, _, cx| {
                            this.graph.by_attention = true;
                            cx.notify();
                        }),
                    ),
                )
                .child(
                    segment("graph-2d", "2D", !self.graph.three_d)
                        .on_click(cx.listener(|this, _, _, cx| this.set_three_d(false, cx))),
                )
                .child(
                    segment("graph-3d", "3D", self.graph.three_d)
                        .on_click(cx.listener(|this, _, _, cx| this.set_three_d(true, cx))),
                )
                .when(self.graph.three_d, |d| {
                    d.child(
                        div()
                            .flex()
                            .items_center()
                            .gap_1()
                            .child(muted("Auto-rotate"))
                            .child(switch("graph-spin", self.graph.auto_rotate).on_click(
                                cx.listener(|this, _, _, cx| {
                                    this.graph.auto_rotate = !this.graph.auto_rotate;
                                    cx.notify();
                                }),
                            )),
                    )
                    .child(button("graph-reset", "Reset view").on_click(cx.listener(
                        |this, _, _, cx| {
                            let fresh = GraphState::default();
                            (this.graph.yaw, this.graph.pitch, this.graph.zoom) =
                                (fresh.yaw, fresh.pitch, fresh.zoom);
                            cx.notify();
                        },
                    )))
                })
                .when(!self.graph.by_attention, |d| d.children(chips))
                .when(self.graph.by_attention, |d| {
                    let count = |f: &dyn Fn(&crate::graph_layout::Node) -> bool| {
                        graph.nodes.iter().filter(|n| f(n)).count()
                    };
                    let has =
                        |n: &crate::graph_layout::Node, k: &str| n.attention.iter().any(|a| a == k);
                    let legend = [
                        ("flagged", theme::danger(), count(&|n| has(n, "flagged"))),
                        (
                            "stale",
                            theme::hue(Hue::Yellow),
                            count(&|n| !has(n, "flagged") && has(n, "stale")),
                        ),
                        (
                            "thin",
                            theme::hue(Hue::Orange),
                            count(&|n| !has(n, "flagged") && !has(n, "stale") && has(n, "thin")),
                        ),
                        (
                            "orphan",
                            theme::text_muted(),
                            count(&|n| n.attention.is_empty() && n.degree == 0),
                        ),
                        (
                            "healthy",
                            theme::hue(Hue::Green),
                            count(&|n| n.attention.is_empty() && n.degree > 0),
                        ),
                    ];
                    d.children(legend.into_iter().map(|(label, color, n)| {
                        div()
                            .flex()
                            .items_center()
                            .gap_1()
                            .child(div().size(px(9.)).rounded_full().bg(color))
                            .child(muted(format!("{label} {n}")))
                    }))
                })
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
                .child(button("graph-refresh", "Refresh").on_click(cx.listener(
                    |this, _, _, cx| {
                        this.ensure_graph(this.selected, true, cx);
                        cx.notify();
                    },
                )))
                .child(button("graph-web", "Open in web").on_click(
                    cx.listener(|this, _, _, cx| this.run(Command::OpenWeb("/graph".into()), cx)),
                ));

        // Edges, drawn on a canvas behind the nodes; in 3D, far ones fainter.
        let places = self.graph_places(&graph);
        let edges: Vec<(f32, f32, f32, f32, bool, f32)> = graph
            .edges
            .iter()
            .filter(|e| visible[e.source] && visible[e.target])
            .map(|e| {
                let (a, b) = (places[e.source], places[e.target]);
                let on = focus.is_some_and(|f| e.source == f || e.target == f);
                (a.fx, a.fy, b.fx, b.fy, on, (a.alpha + b.alpha) / 2.0)
            })
            .collect();
        let area_size = self.graph.area.clone();
        let (dim, bright) = (theme::text_muted(), theme::accent());
        let dimmed = focus.is_some();
        let lines = canvas(
            // Remember the size: the 3D projection keeps its aspect from it.
            move |bounds, _, _| {
                area_size.set((bounds.size.width / px(1.), bounds.size.height / px(1.)));
            },
            move |bounds, (), window, _| {
                let at = |x: f32, y: f32| {
                    point(
                        bounds.origin.x + bounds.size.width * x,
                        bounds.origin.y + bounds.size.height * y,
                    )
                };
                // Faint ones first, the lit ones on top.
                for pass in [false, true] {
                    for (x1, y1, x2, y2, on, alpha) in &edges {
                        if *on != pass {
                            continue;
                        }
                        // Bolder lines; in 3D, far ones a little thinner.
                        let width = if *on { 2.6 } else { 1.2 + 0.8 * alpha };
                        let mut path = PathBuilder::stroke(px(width));
                        path.move_to(at(*x1, *y1));
                        path.line_to(at(*x2, *y2));
                        if let Ok(path) = path.build() {
                            let color = if *on {
                                bright
                            } else if dimmed {
                                dim.opacity(0.15)
                            } else {
                                dim.opacity(0.55 * alpha)
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
        let three_d = self.graph.three_d;
        // Far to near, so nearer chunks are drawn over farther ones.
        let mut order: Vec<usize> = (0..graph.nodes.len()).filter(|&i| visible[i]).collect();
        order.sort_by(|a, b| places[*a].depth.total_cmp(&places[*b].depth));
        let nodes = order
            .into_iter()
            .map(|i| (i, &graph.nodes[i]))
            .map(|(i, node)| {
                let place = places[i];
                let size = place.size;
                let color = if self.graph.by_attention {
                    attention_color(node)
                } else {
                    kind_color(&node.kind)
                };
                let faded = (dimmed || searching) && !lit.contains(&i);
                // In 3D, only the near half gets labels (the rest would pile up).
                let near = !three_d || place.depth > 0.0;
                let show_label = lit.contains(&i)
                    || (!dimmed && !searching && near && (label_all || node.degree >= 4));
                let selected = self.graph.selected == Some(i);
                div()
                    .id(("graph-node", i))
                    .absolute()
                    .left(relative(place.fx))
                    .top(relative(place.fy))
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
                    .on_click(cx.listener(move |this, event: &gpui::ClickEvent, _, cx| {
                        // Shift-click with a chunk selected: connect them.
                        if event.modifiers().shift
                            && let Some(from) = this.graph.selected
                            && from != i
                        {
                            this.graph.link = Some((from, i));
                            cx.notify();
                            return;
                        }
                        this.graph.link = None;
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
                            .bg(color.opacity(if faded { 0.2 } else { 0.9 * place.alpha }))
                            // A contour: a lighter tint of its own color,
                            // thicker and in the text color when selected.
                            .when(selected, |d| d.border_2().border_color(theme::text()))
                            .when(!selected, |d| {
                                let tint = gpui::Hsla {
                                    l: (color.l + 0.22).min(0.92),
                                    ..color
                                };
                                d.border_1().border_color(tint.opacity(if faded {
                                    0.3
                                } else {
                                    place.alpha
                                }))
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
            .when(three_d, |d| {
                d.cursor_grab()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, e: &MouseDownEvent, _, _| {
                            this.graph.drag = Some(e.position)
                        }),
                    )
                    .on_mouse_move(cx.listener(|this, e: &MouseMoveEvent, _, cx| {
                        let Some(last) = this.graph.drag else { return };
                        if e.pressed_button != Some(MouseButton::Left) {
                            this.graph.drag = None;
                            return;
                        }
                        let (dx, dy) = (
                            (e.position.x - last.x) / px(1.),
                            (e.position.y - last.y) / px(1.),
                        );
                        this.graph.yaw += dx * 0.008;
                        this.graph.pitch = (this.graph.pitch + dy * 0.008).clamp(-1.45, 1.45);
                        this.graph.drag = Some(e.position);
                        cx.notify();
                    }))
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            this.graph.drag = None;
                            cx.notify();
                        }),
                    )
                    .on_scroll_wheel(cx.listener(|this, e: &ScrollWheelEvent, window, cx| {
                        let dy = e.delta.pixel_delta(window.line_height()).y / px(1.);
                        this.graph.zoom = (this.graph.zoom * (1.0 - dy * 0.002)).clamp(0.4, 3.0);
                        // The page under it does not scroll while zooming.
                        cx.stop_propagation();
                        cx.notify();
                    }))
            })
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
            .child(muted(if three_d {
                "Drag to turn · scroll to zoom · click a chunk to select it · shift-click another to connect them"
            } else {
                "Click a chunk to select it · shift-click another to connect them"
            }))
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
        // A connection being made from this chunk.
        let linking = self
            .graph
            .link
            .filter(|(from, _)| *from == s)
            .map(|(_, to)| {
                let target = graph.nodes[to].title.clone();
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .p_2()
                    .rounded_md()
                    .border_1()
                    .border_color(theme::accent())
                    .child(
                        div()
                            .text_sm()
                            .child(format!("Connect {} → {target}", node.title)),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .gap_1()
                            .children(RELATIONS.iter().map(|relation| {
                                let relation: &'static str = relation;
                                segment(
                                    SharedString::from(format!("rel-{relation}")),
                                    relation.replace('_', " "),
                                    self.graph.link_relation == relation,
                                )
                                .on_click(cx.listener(
                                    move |this, _, _, cx| {
                                        this.graph.link_relation = relation;
                                        cx.notify();
                                    },
                                ))
                            })),
                    )
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .child(
                                button("link-create", "Connect")
                                    .on_click(cx.listener(|this, _, _, cx| this.create_link(cx))),
                            )
                            .child(button("link-cancel", "Cancel").on_click(cx.listener(
                                |this, _, _, cx| {
                                    this.graph.link = None;
                                    cx.notify();
                                },
                            ))),
                    )
            });
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
            .children(linking)
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
