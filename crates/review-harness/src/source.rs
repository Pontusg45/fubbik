//! File collection with one shared ignore list, so every check sees the same corpus.

use std::fs;
use std::io;
use std::path::Path;

/// Directory names never walked.
const IGNORED_DIRS: &[&str] = &[".git", "node_modules", "target", "dist", "build", ".turbo"];

/// File-name fragments marking generated output.
const IGNORED_FILE_MARKERS: &[&str] = &[".gen.", ".d.ts"];

/// A file's repo-relative path and contents.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextFile {
    pub path: String,
    pub text: String,
}

/// Collect every non-ignored file under `roots` (relative to `repo_root`) whose name ends in one of `extensions`.
/// Missing roots are skipped; a check whose roots are all missing sees an empty corpus.
pub fn collect(repo_root: &Path, roots: &[&str], extensions: &[&str]) -> io::Result<Vec<TextFile>> {
    let mut files = Vec::new();
    for root in roots {
        let dir = repo_root.join(root);
        if dir.is_dir() {
            walk(repo_root, &dir, extensions, &mut files)?;
        }
    }
    files.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(files)
}

fn walk(
    repo_root: &Path,
    dir: &Path,
    extensions: &[&str],
    out: &mut Vec<TextFile>,
) -> io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if entry.file_type()?.is_dir() {
            if !IGNORED_DIRS.contains(&name.as_str()) {
                walk(repo_root, &path, extensions, out)?;
            }
            continue;
        }
        if IGNORED_FILE_MARKERS.iter().any(|m| name.contains(m))
            || !extensions.iter().any(|e| name.ends_with(e))
        {
            continue;
        }
        let rel = path.strip_prefix(repo_root).unwrap_or(&path);
        out.push(TextFile {
            path: rel.to_string_lossy().replace('\\', "/"),
            text: fs::read_to_string(&path)?,
        });
    }
    Ok(())
}
