//! A knowledge graph for the Knowledge tab: chunks as nodes, connections as
//! edges, placed by a force-directed layout (Fruchterman–Reingold) into the
//! unit square. Deterministic: the same graph always lands the same way, so
//! the picture does not jump on every refresh.

#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    pub id: String,
    pub title: String,
    /// The chunk type (note, document, reference, schema, checklist).
    pub kind: String,
    /// Position in [0, 1] × [0, 1].
    pub x: f32,
    pub y: f32,
    /// Connections in or out.
    pub degree: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edge {
    /// Indexes into `nodes`.
    pub source: usize,
    pub target: usize,
    pub relation: String,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Graph {
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
}

impl Graph {
    /// Builds from the `GET /api/graph` JSON (`chunks`, `connections`);
    /// connections to chunks outside the response are dropped.
    pub fn from_api(json: &serde_json::Value) -> Graph {
        let mut nodes: Vec<Node> = json["chunks"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|c| {
                Some(Node {
                    id: c["id"].as_str()?.to_string(),
                    title: c["title"].as_str().unwrap_or("untitled").to_string(),
                    kind: c["type"].as_str().unwrap_or("note").to_string(),
                    x: 0.5,
                    y: 0.5,
                    degree: 0,
                })
            })
            .collect();
        let index: std::collections::HashMap<&str, usize> = nodes
            .iter()
            .enumerate()
            .map(|(i, n)| (n.id.as_str(), i))
            .collect();
        let mut edges: Vec<Edge> = json["connections"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|c| {
                let source = *index.get(c["sourceId"].as_str()?)?;
                let target = *index.get(c["targetId"].as_str()?)?;
                (source != target).then(|| Edge {
                    source,
                    target,
                    relation: c["relation"].as_str().unwrap_or("related_to").to_string(),
                })
            })
            .collect();
        edges.sort_by_key(|e| (e.source, e.target));
        edges.dedup_by_key(|e| (e.source, e.target));
        for edge in &edges {
            nodes[edge.source].degree += 1;
            nodes[edge.target].degree += 1;
        }
        let mut graph = Graph { nodes, edges };
        graph.layout();
        graph
    }

    /// Places the nodes. Connected chunks get a force-directed layout
    /// (Fruchterman–Reingold with a pull to the centre) over the area;
    /// orphans (no connections) line up in a strip along the bottom, so they
    /// neither crowd the edges nor squeeze the connected part into a corner.
    pub fn layout(&mut self) {
        let n = self.nodes.len();
        if n == 0 {
            return;
        }
        let connected: Vec<usize> = (0..n).filter(|&i| self.nodes[i].degree > 0).collect();
        let orphans: Vec<usize> = (0..n).filter(|&i| self.nodes[i].degree == 0).collect();
        if connected.is_empty() {
            // Nothing connects: a grid over the whole area.
            self.grid(&orphans, 0.06, 0.94);
            return;
        }
        let bottom = if orphans.is_empty() { 0.94 } else { 0.80 };
        self.force_layout(&connected, bottom);
        if !orphans.is_empty() {
            self.grid(&orphans, 0.86, 0.96);
        }
    }

    /// Rows of evenly spaced nodes between `top` and `bottom`.
    fn grid(&mut self, nodes: &[usize], top: f32, bottom: f32) {
        let per_row = 18usize.min(nodes.len()).max(1);
        let rows = nodes.len().div_ceil(per_row);
        for (k, &i) in nodes.iter().enumerate() {
            let (row, col) = (k / per_row, k % per_row);
            let in_row = per_row.min(nodes.len() - row * per_row);
            self.nodes[i].x = 0.06 + 0.88 * (col as f32 + 0.5) / in_row as f32;
            self.nodes[i].y = if rows == 1 {
                (top + bottom) / 2.0
            } else {
                top + (bottom - top) * row as f32 / (rows - 1) as f32
            };
        }
    }

    /// Fruchterman–Reingold over `subset` (by index), fitted into
    /// [0.06, 0.94] × [0.06, `bottom`].
    fn force_layout(&mut self, subset: &[usize], bottom: f32) {
        let n = subset.len();
        let local: std::collections::HashMap<usize, usize> =
            subset.iter().enumerate().map(|(k, &i)| (i, k)).collect();
        let edges: Vec<(usize, usize)> = self
            .edges
            .iter()
            .filter_map(|e| Some((*local.get(&e.source)?, *local.get(&e.target)?)))
            .collect();
        let (mut xs, mut ys) = (vec![0.5f32; n], vec![0.5f32; n]);
        if n > 1 {
            // Start on a circle, by index (deterministic), with a small
            // per-node offset so symmetric graphs still separate.
            for k in 0..n {
                let angle = k as f32 / n as f32 * std::f32::consts::TAU;
                let wobble = ((k * 7919) % 97) as f32 / 97.0 * 0.05;
                xs[k] = 0.5 + (0.35 + wobble) * angle.cos();
                ys[k] = 0.5 + (0.35 + wobble) * angle.sin();
            }
            let kk = (1.0 / n as f32).sqrt() * 0.8;
            let iterations = if n > 400 { 80 } else { 300 };
            let (mut dx, mut dy) = (vec![0f32; n], vec![0f32; n]);
            for step in 0..iterations {
                let temperature = 0.1 * (1.0 - step as f32 / iterations as f32) + 0.002;
                dx.iter_mut().for_each(|v| *v = 0.0);
                dy.iter_mut().for_each(|v| *v = 0.0);
                for a in 0..n {
                    for b in (a + 1)..n {
                        let (ddx, ddy) = (xs[a] - xs[b], ys[a] - ys[b]);
                        let dist = (ddx * ddx + ddy * ddy).sqrt().max(0.001);
                        let force = kk * kk / dist;
                        let (fx, fy) = (ddx / dist * force, ddy / dist * force);
                        dx[a] += fx;
                        dy[a] += fy;
                        dx[b] -= fx;
                        dy[b] -= fy;
                    }
                }
                for &(a, b) in &edges {
                    let (ddx, ddy) = (xs[a] - xs[b], ys[a] - ys[b]);
                    let dist = (ddx * ddx + ddy * ddy).sqrt().max(0.001);
                    let force = dist * dist / kk;
                    let (fx, fy) = (ddx / dist * force, ddy / dist * force);
                    dx[a] -= fx;
                    dy[a] -= fy;
                    dx[b] += fx;
                    dy[b] += fy;
                }
                for k in 0..n {
                    dx[k] -= (xs[k] - 0.5) * kk * 0.3;
                    dy[k] -= (ys[k] - 0.5) * kk * 0.3;
                    let len = (dx[k] * dx[k] + dy[k] * dy[k]).sqrt().max(0.0001);
                    let step = len.min(temperature);
                    xs[k] += dx[k] / len * step;
                    ys[k] += dy[k] / len * step;
                }
            }
        }
        // Fit into the box, filling both directions (the area is wider than
        // tall; stretching reads better than empty bands).
        let (top, left, right) = (0.06f32, 0.06f32, 0.94f32);
        let fit = |values: &[f32], lo: f32, hi: f32| -> Vec<f32> {
            let (min, max) = values
                .iter()
                .fold((f32::MAX, f32::MIN), |(a, b), v| (a.min(*v), b.max(*v)));
            let span = max - min;
            values
                .iter()
                .map(|v| {
                    if span < 1e-6 {
                        (lo + hi) / 2.0
                    } else {
                        lo + (v - min) / span * (hi - lo)
                    }
                })
                .collect()
        };
        let (xs, ys) = (fit(&xs, left, right), fit(&ys, top, bottom));
        for (k, &i) in subset.iter().enumerate() {
            self.nodes[i].x = xs[k];
            self.nodes[i].y = ys[k];
        }
    }

    /// Indexes of the nodes connected to `node`.
    pub fn neighbours(&self, node: usize) -> Vec<usize> {
        self.edges
            .iter()
            .filter_map(|e| {
                if e.source == node {
                    Some(e.target)
                } else if e.target == node {
                    Some(e.source)
                } else {
                    None
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample() -> serde_json::Value {
        // Two clusters (a-b-c and d-e) joined by one edge, plus a loner.
        json!({
            "chunks": [
                { "id": "a", "title": "A", "type": "document" },
                { "id": "b", "title": "B", "type": "note" },
                { "id": "c", "title": "C", "type": "note" },
                { "id": "d", "title": "D", "type": "schema" },
                { "id": "e", "title": "E", "type": "schema" },
                { "id": "f", "title": "F", "type": "note" },
            ],
            "connections": [
                { "id": "1", "sourceId": "a", "targetId": "b", "relation": "related_to" },
                { "id": "2", "sourceId": "b", "targetId": "c", "relation": "part_of" },
                { "id": "3", "sourceId": "a", "targetId": "c", "relation": "depends_on" },
                { "id": "4", "sourceId": "d", "targetId": "e", "relation": "extends" },
                { "id": "5", "sourceId": "c", "targetId": "d", "relation": "references" },
                { "id": "6", "sourceId": "c", "targetId": "zz", "relation": "references" },
                { "id": "7", "sourceId": "a", "targetId": "b", "relation": "supports" },
            ],
        })
    }

    #[test]
    fn builds_nodes_edges_and_degrees_from_the_api() {
        // Given / When
        let graph = Graph::from_api(&sample());

        // Then
        assert_eq!(graph.nodes.len(), 6);
        assert_eq!(
            graph.edges.len(),
            5,
            "the dangling edge and the duplicate a→b go"
        );
        let degree = |id: &str| graph.nodes.iter().find(|n| n.id == id).unwrap().degree;
        assert_eq!((degree("c"), degree("f")), (3, 0));
        assert_eq!(graph.neighbours(2).len(), 3);
    }

    #[test]
    fn layout_is_inside_the_square_deterministic_and_pulls_neighbours_together() {
        // Given / When
        let first = Graph::from_api(&sample());
        let second = Graph::from_api(&sample());

        // Then
        assert_eq!(first, second, "the same graph lands the same way");
        for node in &first.nodes {
            assert!(
                (0.0..=1.0).contains(&node.x) && (0.0..=1.0).contains(&node.y),
                "{node:?}"
            );
            assert!(node.x.is_finite() && node.y.is_finite());
        }
        let dist = |a: usize, b: usize| {
            let (p, q) = (&first.nodes[a], &first.nodes[b]);
            ((p.x - q.x).powi(2) + (p.y - q.y).powi(2)).sqrt()
        };
        // Connected a–b are closer than unconnected a–e.
        assert!(dist(0, 1) < dist(0, 4), "{} vs {}", dist(0, 1), dist(0, 4));
        // The orphan f sits in the bottom strip, below every connected node.
        let f = &first.nodes[5];
        assert!(f.y > 0.85, "orphan at {}", f.y);
        assert!(first.nodes[..5].iter().all(|n| n.y <= 0.81));
        // No two nodes on top of each other.
        for i in 0..first.nodes.len() {
            for j in (i + 1)..first.nodes.len() {
                assert!(dist(i, j) > 0.02, "{i} and {j} overlap");
            }
        }
    }

    #[test]
    fn empty_and_single_graphs_are_fine() {
        // Given
        let empty = Graph::from_api(&json!({ "chunks": [], "connections": [] }));
        let one =
            Graph::from_api(&json!({ "chunks": [{ "id": "a", "title": "A", "type": "note" }] }));

        // Then
        assert!(empty.nodes.is_empty());
        assert!((one.nodes[0].x - 0.5).abs() < 1e-6 && (one.nodes[0].y - 0.5).abs() < 1e-6);
    }
}
