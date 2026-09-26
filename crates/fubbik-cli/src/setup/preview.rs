use super::*;

pub(super) fn render_preview(
    chunks: &[DiscoveredChunk],
    connections: &[DiscoveredConnection],
    tags: &[String],
    mode: OutputMode,
) -> Result<()> {
    let mut groups: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for chunk in chunks {
        groups
            .entry(&chunk.category)
            .or_default()
            .push(&chunk.title);
    }
    if mode == OutputMode::Json {
        let groups = groups
            .into_iter()
            .map(|(category, titles)| {
                (
                    category,
                    serde_json::json!({"count": titles.len(), "titles": titles}),
                )
            })
            .collect::<BTreeMap<_, _>>();
        return output::json(&serde_json::json!({
            "totalChunks": chunks.len(),
            "groups": groups,
            "connections": connections.len(),
            "tags": tags,
        }));
    }
    if mode == OutputMode::Quiet {
        println!("{}", chunks.len());
        return Ok(());
    }
    println!("\nReady to import {} chunks\n", chunks.len());
    for category in [
        "documents",
        "tech-stack",
        "structure",
        "conventions",
        "config",
    ] {
        if let Some(titles) = groups.get(category) {
            println!(
                "  {category:<12} {:>5}   {}",
                titles.len(),
                titles
                    .iter()
                    .take(3)
                    .copied()
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }
    }
    println!(
        "\n  + {} connections, {} tags\n",
        connections.len(),
        tags.len()
    );
    println!("Dry run complete. No chunks imported.");
    Ok(())
}
