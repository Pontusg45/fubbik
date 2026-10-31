//! Source files no chunk covers. Mirrors `fubbik gaps`: walk the repo, skip
//! dot-directories and build output, keep source extensions minus tests, and
//! ask `/api/context/for-file` for each; a file with any chunk is covered.

use std::collections::BTreeMap;
use std::path::Path;

use crate::fubbik::FubbikClient;

const SKIP_DIRS: [&str; 9] = [
    "node_modules",
    ".git",
    "dist",
    "build",
    ".next",
    ".turbo",
    ".claude",
    ".fubbik",
    "target",
];
const EXTENSIONS: [&str; 11] = [
    "ts", "tsx", "js", "jsx", "py", "go", "rs", "java", "rb", "vue", "svelte",
];
/// One request per file: cap the scan.
pub const MAX_FILES: usize = 400;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GapReport {
    pub total: usize,
    pub covered: usize,
    /// True when the repo had more than `MAX_FILES` source files.
    pub truncated: bool,
    /// Directory → uncovered files in it, most first.
    pub by_directory: Vec<(String, Vec<String>)>,
}

impl GapReport {
    pub fn coverage(&self) -> usize {
        (self.covered * 100).checked_div(self.total).unwrap_or(100)
    }
}

fn is_source(name: &str) -> bool {
    let Some((_, ext)) = name.rsplit_once('.') else {
        return false;
    };
    EXTENSIONS.contains(&ext)
        && !name.ends_with(".test.ts")
        && !name.ends_with(".test.tsx")
        && !name.ends_with(".spec.ts")
}

/// Repo-relative source files, sorted.
pub fn source_files(root: &Path) -> Vec<String> {
    fn walk(root: &Path, dir: &Path, out: &mut Vec<String>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        let mut entries: Vec<_> = entries.flatten().collect();
        entries.sort_by_key(|e| e.file_name());
        for entry in entries {
            let name = entry.file_name().to_string_lossy().into_owned();
            let path = entry.path();
            if path.is_dir() {
                if !name.starts_with('.') && !SKIP_DIRS.contains(&name.as_str()) {
                    walk(root, &path, out);
                }
            } else if is_source(&name)
                && let Ok(relative) = path.strip_prefix(root)
            {
                out.push(relative.to_string_lossy().into_owned());
            }
        }
    }
    let mut files = Vec::new();
    walk(root, root, &mut files);
    files
}

pub fn group_uncovered(uncovered: &[String]) -> Vec<(String, Vec<String>)> {
    let mut groups: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for file in uncovered {
        let dir = Path::new(file)
            .parent()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default();
        let dir = if dir.is_empty() { ".".to_string() } else { dir };
        groups.entry(dir).or_default().push(file.clone());
    }
    let mut groups: Vec<_> = groups.into_iter().collect();
    groups.sort_by_key(|(_, files)| std::cmp::Reverse(files.len()));
    groups
}

pub fn scan(
    client: &FubbikClient,
    root: &Path,
    space_id: Option<&str>,
) -> Result<GapReport, String> {
    let files = source_files(root);
    let truncated = files.len() > MAX_FILES;
    let mut covered = 0;
    let mut uncovered = Vec::new();
    for file in files.iter().take(MAX_FILES) {
        if client.chunks_for_file(file, space_id)?.is_empty() {
            uncovered.push(file.clone());
        } else {
            covered += 1;
        }
    }
    Ok(GapReport {
        total: files.len().min(MAX_FILES),
        covered,
        truncated,
        by_directory: group_uncovered(&uncovered),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn walks_sources_and_skips_build_output_and_tests() {
        // Given
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        for (path, text) in [
            ("src/lib.rs", ""),
            ("src/app.ts", ""),
            ("src/app.test.ts", ""),
            ("README.md", ""),
            ("node_modules/pkg/index.js", ""),
            ("dist/app.js", ""),
            (".hidden/x.ts", ""),
        ] {
            std::fs::create_dir_all(root.join(path).parent().unwrap()).unwrap();
            std::fs::write(root.join(path), text).unwrap();
        }

        // When
        let files = source_files(root);

        // Then
        assert_eq!(files, ["src/app.ts", "src/lib.rs"]);
    }

    #[test]
    fn groups_uncovered_by_directory_most_first() {
        // Given
        let uncovered = ["a/x.rs", "b/y.rs", "b/z.rs", "top.rs"].map(String::from);

        // When
        let groups = group_uncovered(&uncovered);

        // Then
        assert_eq!(
            groups[0],
            (
                "b".to_string(),
                vec!["b/y.rs".to_string(), "b/z.rs".to_string()]
            )
        );
        assert!(groups.iter().any(|(dir, _)| dir == "."));
    }

    #[test]
    fn coverage_of_nothing_is_full() {
        // Given
        let report = GapReport::default();

        // When
        let coverage = report.coverage();

        // Then
        assert_eq!(coverage, 100);
    }
}
