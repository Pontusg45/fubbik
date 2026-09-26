use super::*;

pub(super) fn chunk(
    title: String,
    content: String,
    chunk_type: &str,
    tags: Vec<String>,
    tier: u8,
    category: &str,
    source: String,
) -> DiscoveredChunk {
    DiscoveredChunk {
        title,
        content,
        chunk_type: chunk_type.into(),
        tags,
        tier,
        category: category.into(),
        source,
        applies_to: Vec::new(),
    }
}

pub(super) fn collect_files<F: Fn(&Path) -> bool>(
    root: &Path,
    directory: &Path,
    depth: usize,
    max_depth: usize,
    files: &mut Vec<PathBuf>,
    predicate: F,
) -> Result<()> {
    collect_files_ref(root, directory, depth, max_depth, files, &predicate)
}

pub(super) fn collect_files_ref(
    root: &Path,
    directory: &Path,
    depth: usize,
    max_depth: usize,
    files: &mut Vec<PathBuf>,
    predicate: &dyn Fn(&Path) -> bool,
) -> Result<()> {
    if depth >= max_depth {
        return Ok(());
    }
    for entry in std::fs::read_dir(directory)? {
        let entry = entry?;
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if !name.starts_with('.') && !IGNORED.contains(&name.as_ref()) {
                collect_files_ref(root, &path, depth + 1, max_depth, files, predicate)?;
            }
        } else if predicate(&path) {
            files.push(path);
        }
    }
    let _ = root;
    Ok(())
}

pub(super) fn find_named_directories(root: &Path, names: &[&str]) -> Result<Vec<PathBuf>> {
    fn visit(
        root: &Path,
        directory: &Path,
        names: &[&str],
        depth: usize,
        found: &mut Vec<PathBuf>,
    ) -> Result<()> {
        if depth >= 4 {
            return Ok(());
        }
        for entry in std::fs::read_dir(directory)? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().into_owned();
            if IGNORED.contains(&name.as_str()) || name.starts_with('.') {
                continue;
            }
            let path = entry.path();
            if names.contains(&name.as_str()) {
                found.push(path.clone());
            }
            visit(root, &path, names, depth + 1, found)?;
        }
        let _ = root;
        Ok(())
    }
    let mut found = Vec::new();
    visit(root, root, names, 0, &mut found)?;
    Ok(found)
}

pub(super) fn find_files_containing(root: &Path, markers: &[&str]) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    collect_files(root, root, 0, 4, &mut files, |path| {
        path.file_name()
            .and_then(|value| value.to_str())
            .is_some_and(|name| markers.iter().any(|marker| name.contains(marker)))
    })?;
    Ok(files)
}

pub(super) fn dependencies(package: &Value) -> HashMap<String, String> {
    ["dependencies", "devDependencies", "peerDependencies"]
        .into_iter()
        .filter_map(|field| package[field].as_object())
        .flat_map(|values| values.iter())
        .filter_map(|(name, version)| Some((name.clone(), version.as_str()?.into())))
        .collect()
}

pub(super) fn workspace_patterns(package: &Value) -> Option<Vec<String>> {
    let value = &package["workspaces"];
    let values = value.as_array().or_else(|| value["packages"].as_array())?;
    Some(
        values
            .iter()
            .filter_map(|value| value.as_str().map(str::to_owned))
            .collect(),
    )
}

pub(super) fn read_json(path: &Path) -> Option<Value> {
    serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()
}

pub(super) fn markdown_title(content: &str) -> Option<String> {
    content
        .lines()
        .find_map(|line| line.strip_prefix("# ").map(|title| title.trim().to_owned()))
}

pub(super) fn markdown_links(content: &str) -> Vec<&str> {
    let mut links = Vec::new();
    let mut rest = content;
    while let Some(start) = rest.find("](") {
        rest = &rest[start + 2..];
        let Some(end) = rest.find(')') else { break };
        let link = &rest[..end];
        if !link.starts_with("http://") && !link.starts_with("https://") {
            links.push(link);
        }
        rest = &rest[end + 1..];
    }
    links
}

pub(super) fn relative_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}
