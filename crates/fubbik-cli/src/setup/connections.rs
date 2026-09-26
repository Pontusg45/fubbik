use super::*;

pub(super) fn infer_connections(chunks: &[DiscoveredChunk]) -> Vec<DiscoveredConnection> {
    let mut connections = Vec::new();
    let mut seen = HashSet::new();
    let by_source = chunks
        .iter()
        .map(|chunk| (chunk.source.as_str(), chunk))
        .collect::<HashMap<_, _>>();
    let mut add = |source: &str, target: &str, relation: &str| {
        let key = format!("{source}\0{target}\0{relation}");
        if source != target && seen.insert(key) {
            connections.push(DiscoveredConnection {
                source_title: source.into(),
                target_title: target.into(),
                relation: relation.into(),
            });
        }
    };
    for source in chunks.iter().filter(|chunk| chunk.tier == 1) {
        for link in markdown_links(&source.content) {
            if let Some(target) = by_source.get(link.trim_start_matches("./"))
                && target.tier == 1
            {
                add(&source.title, &target.title, "references");
            }
        }
    }
    if let Some(structure) = chunks
        .iter()
        .find(|chunk| chunk.title == "Project Structure (Monorepo)")
    {
        for tech in chunks.iter().filter(|chunk| chunk.category == "tech-stack") {
            add(&tech.title, &structure.title, "part_of");
        }
    }
    for routing in chunks
        .iter()
        .filter(|chunk| chunk.tags.iter().any(|tag| tag == "routing"))
    {
        for target in chunks.iter().filter(|chunk| {
            chunk
                .tags
                .iter()
                .any(|tag| tag == "database" || tag == "auth")
        }) {
            add(&routing.title, &target.title, "depends_on");
        }
    }
    connections
}
