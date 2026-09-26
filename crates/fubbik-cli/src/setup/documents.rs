use super::*;

pub(super) fn scan_docs(directory: &Path) -> Result<Vec<DiscoveredChunk>> {
    let mut files = Vec::new();
    collect_files(directory, directory, 0, 5, &mut files, |path| {
        path.extension().and_then(|value| value.to_str()) == Some("md")
    })?;
    files.sort();
    let names = HashMap::from([
        ("README.md", "Project README"),
        ("CLAUDE.md", "AI Assistant Instructions (CLAUDE.md)"),
        ("CONTRIBUTING.md", "Contributing Guide"),
        ("Agents.md", "AI Agents Documentation"),
        ("CHANGELOG.md", "Changelog"),
    ]);
    let mut chunks = Vec::new();
    for path in files {
        let content = std::fs::read_to_string(&path)?;
        if content.trim().is_empty() {
            continue;
        }
        let relative = relative_path(directory, &path);
        let filename = path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("document");
        let title = names
            .get(filename)
            .filter(|_| path.parent() == Some(directory))
            .map(|value| (*value).to_owned())
            .or_else(|| markdown_title(&content))
            .unwrap_or_else(|| filename.trim_end_matches(".md").to_owned());
        let mut tags = vec!["documentation".into()];
        if relative.starts_with("docs/") {
            tags.push("docs".into());
        }
        for component in Path::new(&relative)
            .parent()
            .into_iter()
            .flat_map(Path::components)
            .take(3)
        {
            let value = component.as_os_str().to_string_lossy().into_owned();
            if !tags.contains(&value) {
                tags.push(value);
            }
        }
        chunks.push(chunk(
            title,
            content,
            "guide",
            tags,
            1,
            "documents",
            relative,
        ));
    }
    Ok(chunks)
}
