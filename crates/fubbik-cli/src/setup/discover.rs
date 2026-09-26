use super::*;

pub fn discover(directory: &Path) -> Result<Discovery> {
    let mut chunks = scan_docs(directory)?;
    let package = read_json(&directory.join("package.json"));
    chunks.extend(scan_metadata(directory, package.as_ref())?);
    let (patterns, tips) = scan_patterns(directory, package.as_ref())?;
    chunks.extend(patterns);
    chunks.sort_by(|left, right| {
        (left.tier, &left.source, &left.title).cmp(&(right.tier, &right.source, &right.title))
    });
    let mut connections = infer_connections(&chunks);
    connections.sort_by(|left, right| {
        (&left.source_title, &left.target_title, &left.relation).cmp(&(
            &right.source_title,
            &right.target_title,
            &right.relation,
        ))
    });
    let mut tips = tips;
    tips.sort_by(|left, right| (&left.title, &left.detail).cmp(&(&right.title, &right.detail)));
    let tags = chunks
        .iter()
        .flat_map(|chunk| chunk.tags.iter().cloned())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    Ok(Discovery {
        root: directory.to_path_buf(),
        chunks,
        connections,
        tags,
        tips,
    })
}
