//! Folds `(path, chunk)` pairs into a directory tree with per-node coverage
//! counts — the shape the density view renders.

use fubbik_db::repo::insights::DensityPath;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DensityChunk {
    pub id: String,
    pub title: String,
    #[serde(rename = "type")]
    pub chunk_type: String,
    /// `applies_to | file_ref`.
    pub source: String,
}

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DensityNode {
    pub name: String,
    pub path: String,
    /// Distinct chunks at this node **or anywhere below it**.
    pub chunk_count: usize,
    /// Chunks attached to this exact path only.
    pub direct_chunk_count: usize,
    /// `#[schema(no_recursion)]` is mandatory, not cosmetic: `DensityNode`
    /// contains itself, and utoipa inlines nested schemas by default, so
    /// without this the generator recurses until it overflows the stack.
    /// That is exactly what happened — `cargo run -- openapi` aborted with
    /// `has overflowed its stack` and produced a zero-byte spec. The
    /// attribute makes this field a `$ref` back to `DensityNode` instead.
    #[schema(no_recursion)]
    pub children: Vec<DensityNode>,
    pub chunks: Vec<DensityChunk>,
}

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DensityTotals {
    pub chunks_covered: usize,
    pub paths_tracked: usize,
}

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DensityResponse {
    pub tree: DensityNode,
    pub totals: DensityTotals,
}

/// Builds the tree.
///
/// Two details are load-bearing and both come from Node:
///
/// - **`chunk_count` is a distinct count over the subtree**, not a sum of
///   children. A chunk attached to `src/a` and `src/b` counts once at `src`.
///   Summing would double it, which is why the recursion returns the id set
///   rather than a number.
/// - **A `(chunk, source)` pair is recorded once per path.** The same chunk
///   can appear at one path twice — once via an applies-to glob and once via
///   an explicit file ref — and that is intentional: the view distinguishes
///   the two. Deduping on `chunk_id` alone would hide it.
pub fn build(paths: &[DensityPath]) -> DensityResponse {
    let covered: HashSet<&str> = paths.iter().map(|p| p.chunk_id.as_str()).collect();
    let totals = DensityTotals {
        chunks_covered: covered.len(),
        paths_tracked: paths.len(),
    };

    // Flat map first, parent links second — building the tree by value would
    // need the parent to exist before its children, which a path list does
    // not guarantee.
    let mut nodes: HashMap<String, DensityNode> = HashMap::new();
    let mut children_of: HashMap<String, Vec<String>> = HashMap::new();
    nodes.insert(
        String::new(),
        DensityNode {
            name: "(root)".into(),
            path: String::new(),
            chunk_count: 0,
            direct_chunk_count: 0,
            children: Vec::new(),
            chunks: Vec::new(),
        },
    );

    for p in paths {
        let mut current = String::new();
        for seg in p.path.split('/').filter(|s| !s.is_empty()) {
            let parent = current.clone();
            current = if parent.is_empty() {
                seg.to_string()
            } else {
                format!("{parent}/{seg}")
            };
            if !nodes.contains_key(&current) {
                nodes.insert(
                    current.clone(),
                    DensityNode {
                        name: seg.to_string(),
                        path: current.clone(),
                        chunk_count: 0,
                        direct_chunk_count: 0,
                        children: Vec::new(),
                        chunks: Vec::new(),
                    },
                );
                children_of.entry(parent).or_default().push(current.clone());
            }
        }
        if let Some(leaf) = nodes.get_mut(&p.path) {
            let dup = leaf
                .chunks
                .iter()
                .any(|c| c.id == p.chunk_id && c.source == p.source);
            if !dup {
                leaf.chunks.push(DensityChunk {
                    id: p.chunk_id.clone(),
                    title: p.chunk_title.clone(),
                    chunk_type: p.chunk_type.clone(),
                    source: p.source.clone(),
                });
            }
        }
    }

    let tree = assemble(&String::new(), &mut nodes, &children_of).0;
    DensityResponse { tree, totals }
}

/// Depth-first assembly, returning each node alongside the distinct chunk ids
/// in its subtree so the parent can union them without re-walking.
fn assemble(
    path: &String,
    nodes: &mut HashMap<String, DensityNode>,
    children_of: &HashMap<String, Vec<String>>,
) -> (DensityNode, HashSet<String>) {
    let mut node = nodes
        .remove(path)
        .expect("every path was inserted before assembly");
    let mut seen: HashSet<String> = node.chunks.iter().map(|c| c.id.clone()).collect();

    let mut built = Vec::new();
    for child in children_of.get(path).into_iter().flatten() {
        let (child_node, child_seen) = assemble(child, nodes, children_of);
        seen.extend(child_seen);
        built.push(child_node);
    }

    // Busiest first, then alphabetical — Node's
    // `b.chunkCount - a.chunkCount || a.name.localeCompare(b.name)`. Plain
    // `str` ordering stands in for `localeCompare`: these are file path
    // segments, so they are effectively ASCII.
    built.sort_by(|a, b| {
        b.chunk_count
            .cmp(&a.chunk_count)
            .then_with(|| a.name.cmp(&b.name))
    });

    node.direct_chunk_count = node.chunks.len();
    node.chunk_count = seen.len();
    node.children = built;
    (node, seen)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(path: &str, chunk: &str, source: &str) -> DensityPath {
        DensityPath {
            path: path.into(),
            chunk_id: chunk.into(),
            chunk_title: format!("Chunk {chunk}"),
            chunk_type: "note".into(),
            source: source.into(),
        }
    }

    /// A chunk attached under two sibling directories counts **once** at
    /// their shared parent. Summing children would report 2.
    #[test]
    fn subtree_counts_are_distinct_not_summed() {
        let tree = build(&[p("src/a", "c1", "file_ref"), p("src/b", "c1", "file_ref")]).tree;
        let src = &tree.children[0];
        assert_eq!(src.name, "src");
        assert_eq!(
            src.chunk_count, 1,
            "the same chunk under two children counts once"
        );
        assert_eq!(
            src.direct_chunk_count, 0,
            "nothing is attached to src itself"
        );
        assert_eq!(src.children.len(), 2);
        assert_eq!(tree.chunk_count, 1);
    }

    /// The same chunk at the same path via both sources is kept twice — the
    /// view distinguishes a glob match from an explicit reference.
    #[test]
    fn one_chunk_two_sources_at_one_path_is_kept_twice() {
        let tree = build(&[
            p("src/lib.rs", "c1", "applies_to"),
            p("src/lib.rs", "c1", "file_ref"),
            // ...but an exact repeat of one pair is deduped.
            p("src/lib.rs", "c1", "file_ref"),
        ])
        .tree;
        let leaf = &tree.children[0].children[0];
        assert_eq!(leaf.chunks.len(), 2, "two sources, two entries");
        assert_eq!(
            leaf.chunk_count, 1,
            "but it is still one distinct chunk for counting"
        );
        assert_eq!(leaf.direct_chunk_count, 2);
    }

    #[test]
    fn children_are_sorted_by_count_then_name() {
        let tree = build(&[
            p("zzz", "c1", "file_ref"),
            p("aaa", "c2", "file_ref"),
            p("mmm", "c3", "file_ref"),
            p("mmm", "c4", "file_ref"),
        ])
        .tree;
        let names: Vec<&str> = tree.children.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(
            names,
            ["mmm", "aaa", "zzz"],
            "busiest first, then alphabetical among equals"
        );
    }

    #[test]
    fn totals_count_distinct_chunks_and_every_pairing() {
        let out = build(&[
            p("a", "c1", "file_ref"),
            p("b", "c1", "applies_to"),
            p("c", "c2", "file_ref"),
        ]);
        assert_eq!(out.totals.chunks_covered, 2, "distinct chunks");
        assert_eq!(
            out.totals.paths_tracked, 3,
            "every pairing, not distinct paths"
        );
    }

    #[test]
    fn an_empty_input_yields_a_bare_root() {
        let out = build(&[]);
        assert_eq!(out.tree.name, "(root)");
        assert!(out.tree.children.is_empty());
        assert_eq!(out.totals.chunks_covered, 0);
    }
}
