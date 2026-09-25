use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Result, bail};

use crate::client::Client;
use crate::output::{self, OutputMode};

const SOURCE_EXTENSIONS: &[&str] = &[
    "ts", "tsx", "js", "jsx", "py", "go", "rs", "java", "rb", "vue", "svelte",
];
const SKIP_DIRECTORIES: &[&str] = &[
    "node_modules",
    ".git",
    "dist",
    "build",
    ".next",
    ".turbo",
    ".claude",
    ".fubbik",
];

pub async fn run(
    client: &Client,
    directory: &Path,
    space: Option<&str>,
    limit: usize,
    mode: OutputMode,
) -> Result<()> {
    if limit == 0 {
        bail!("limit must be greater than zero");
    }
    let root = absolute(directory)?;
    let files = collect_source_files(&root)?;
    if files.is_empty() {
        bail!("no source files found in {}", directory.display());
    }
    let space = client.resolve_space(space).await?;
    let mut covered = Vec::new();
    let mut uncovered = Vec::new();
    for file in &files {
        let relative = file.strip_prefix(&root).unwrap_or(file);
        let path = relative.to_string_lossy().into_owned();
        let has_context = client
            .context_for_file(&path, space.as_deref(), 8000, "json-legacy")
            .await
            .ok()
            .and_then(|value| value["chunks"].as_array().map(|chunks| !chunks.is_empty()))
            .unwrap_or(false);
        if has_context {
            covered.push(path);
        } else {
            uncovered.push(path);
        }
    }

    let grouped = group_by_directory(&uncovered);
    let coverage = covered.len() * 100 / files.len();
    let gaps = grouped
        .iter()
        .map(|(directory, files)| {
            serde_json::json!({
                "directory": directory,
                "count": files.len(),
                "files": files,
            })
        })
        .collect::<Vec<_>>();
    let result = serde_json::json!({
        "directory": directory,
        "totalFiles": files.len(),
        "coveredFiles": covered.len(),
        "uncoveredFiles": uncovered.len(),
        "coverage": coverage,
        "gaps": gaps,
    });
    if mode == OutputMode::Json {
        return output::json(&result);
    }
    println!(
        "Knowledge gaps in {}:\n  {} source files scanned\n  {} covered ({}%)\n  {} with no knowledge\n",
        directory.display(),
        files.len(),
        covered.len(),
        coverage,
        uncovered.len()
    );
    let mut shown = 0;
    for (directory, files) in grouped {
        if shown >= limit {
            break;
        }
        println!("  {directory}/ ({} uncovered)", files.len());
        for file in files.iter().take(5) {
            if shown >= limit {
                break;
            }
            println!("    - {file}");
            shown += 1;
        }
        if files.len() > 5 {
            println!("    ... and {} more", files.len() - 5);
        }
    }
    Ok(())
}

fn absolute(path: &Path) -> Result<std::path::PathBuf> {
    let path = if path.is_absolute() {
        path.to_owned()
    } else {
        std::env::current_dir()?.join(path)
    };
    if !path.is_dir() {
        bail!("not a directory: {}", path.display());
    }
    Ok(path)
}

fn collect_source_files(root: &Path) -> Result<Vec<std::path::PathBuf>> {
    let mut files = Vec::new();
    collect_into(root, &mut files)?;
    files.sort();
    Ok(files)
}

fn collect_into(directory: &Path, files: &mut Vec<std::path::PathBuf>) -> Result<()> {
    let mut entries = std::fs::read_dir(directory)?.collect::<std::io::Result<Vec<_>>>()?;
    entries.sort_by_key(std::fs::DirEntry::file_name);
    for entry in entries {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            if !name.starts_with('.') && !SKIP_DIRECTORIES.contains(&name.as_ref()) {
                collect_into(&entry.path(), files)?;
            }
            continue;
        }
        let path = entry.path();
        let extension = path
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or("");
        let is_test =
            name.ends_with(".test.ts") || name.ends_with(".test.tsx") || name.ends_with(".spec.ts");
        if file_type.is_file() && SOURCE_EXTENSIONS.contains(&extension) && !is_test {
            files.push(path);
        }
    }
    Ok(())
}

fn group_by_directory(files: &[String]) -> Vec<(String, Vec<String>)> {
    let mut grouped = BTreeMap::<String, Vec<String>>::new();
    for file in files {
        let directory = Path::new(file)
            .parent()
            .and_then(Path::to_str)
            .filter(|value| !value.is_empty())
            .unwrap_or(".");
        grouped
            .entry(directory.into())
            .or_default()
            .push(file.clone());
    }
    let mut grouped = grouped.into_iter().collect::<Vec<_>>();
    grouped.sort_by(|left, right| right.1.len().cmp(&left.1.len()).then(left.0.cmp(&right.0)));
    grouped
}

#[cfg(test)]
mod tests {
    use super::{collect_source_files, group_by_directory};

    #[test]
    fn source_collection_skips_dependencies_build_output_and_tests() {
        // Given source, test, dependency, and build-output files
        let root = tempfile::tempdir().unwrap();
        for directory in ["src", "node_modules", "dist"] {
            std::fs::create_dir(root.path().join(directory)).unwrap();
        }
        std::fs::write(root.path().join("src/lib.rs"), "lib").unwrap();
        std::fs::write(root.path().join("src/app.ts"), "app").unwrap();
        std::fs::write(root.path().join("src/app.test.ts"), "test").unwrap();
        std::fs::write(root.path().join("node_modules/pkg.js"), "dep").unwrap();
        std::fs::write(root.path().join("dist/app.js"), "built").unwrap();

        // When source files are collected
        let files = collect_source_files(root.path()).unwrap();

        // Then only production source files are returned
        assert_eq!(files.len(), 2);
        assert!(files.iter().any(|path| path.ends_with("src/lib.rs")));
        assert!(files.iter().any(|path| path.ends_with("src/app.ts")));
    }

    #[test]
    fn gap_groups_are_sorted_by_uncovered_count() {
        // Given uncovered files across two directories
        let files = vec![
            "small/a.rs".into(),
            "large/a.rs".into(),
            "large/b.rs".into(),
        ];

        // When files are grouped for reporting
        let groups = group_by_directory(&files);

        // Then the directory with most gaps is first
        assert_eq!(groups[0].0, "large");
        assert_eq!(groups[0].1.len(), 2);
    }
}
