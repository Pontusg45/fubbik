//! `.env` drift: keys a repo's `.env.example` lists that its `.env` lacks,
//! and keys in `.env` the example does not mention. Key NAMES only: each
//! line is cut at `=` as it is read and the value is never kept, shown or
//! sent anywhere. Off unless turned on in Setup.

use std::io::BufRead;
use std::path::{Path, PathBuf};

/// The example files the app understands, in order of preference.
const EXAMPLES: [&str; 3] = [".env.example", ".env.sample", ".env.template"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Drift {
    /// The folder (relative to the repo; empty for the root).
    pub dir: String,
    pub example: String,
    /// False: there is an example but no `.env` at all.
    pub has_env: bool,
    /// In the example, not in `.env`.
    pub missing: Vec<String>,
    /// In `.env`, not in the example.
    pub extra: Vec<String>,
}

impl Drift {
    pub fn is_clean(&self) -> bool {
        self.has_env && self.missing.is_empty() && self.extra.is_empty()
    }
}

/// The key on a line (`export FOO=bar` → `FOO`), or None for comments,
/// blank lines and anything that is not `NAME=...`.
pub fn key(line: &str) -> Option<&str> {
    let line = line.trim_start();
    if line.starts_with('#') {
        return None;
    }
    let line = line.strip_prefix("export ").unwrap_or(line);
    let (name, _) = line.split_once('=')?;
    let name = name.trim();
    (!name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.' || c == '-'))
    .then_some(name)
}

/// The key names in a file, sorted; values are dropped line by line.
pub fn keys(path: &Path) -> Option<Vec<String>> {
    let file = std::fs::File::open(path).ok()?;
    let mut out: Vec<String> = std::io::BufReader::new(file)
        .lines()
        .map_while(Result::ok)
        // A byte-order mark (editors on Windows) is not part of the first key.
        .filter_map(|line| key(line.trim_start_matches('\u{feff}')).map(str::to_string))
        .collect();
    out.sort();
    out.dedup();
    Some(out)
}

pub fn compare(example: &[String], env: &[String]) -> (Vec<String>, Vec<String>) {
    (
        example
            .iter()
            .filter(|k| !env.contains(k))
            .cloned()
            .collect(),
        env.iter()
            .filter(|k| !example.contains(k))
            .cloned()
            .collect(),
    )
}

/// Folders with an example file: the root and up to two levels down
/// (workspace apps and packages), skipping dependencies and build output.
fn folders(repo: &Path) -> Vec<PathBuf> {
    fn walk(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
        if EXAMPLES.iter().any(|e| dir.join(e).is_file()) {
            out.push(dir.to_path_buf());
        }
        if depth == 2 {
            return;
        }
        for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let path = entry.path();
            if path.is_dir()
                && !name.starts_with('.')
                && !matches!(
                    name.as_str(),
                    "node_modules" | "target" | "dist" | "build" | "legacy"
                )
            {
                walk(&path, depth + 1, out);
            }
        }
    }
    let mut out = Vec::new();
    walk(repo, 0, &mut out);
    out.sort();
    out
}

/// Every example in the repo against its `.env`.
pub fn scan(repo: &Path) -> Vec<Drift> {
    folders(repo)
        .into_iter()
        .filter_map(|dir| {
            let example = EXAMPLES.iter().find(|e| dir.join(e).is_file())?;
            let wanted = keys(&dir.join(example))?;
            let env = keys(&dir.join(".env"));
            let (missing, extra) = compare(&wanted, env.as_deref().unwrap_or_default());
            Some(Drift {
                dir: dir
                    .strip_prefix(repo)
                    .unwrap_or(&dir)
                    .to_string_lossy()
                    .into_owned(),
                example: example.to_string(),
                has_env: env.is_some(),
                missing: if env.is_some() { missing } else { Vec::new() },
                extra,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_key_names_only() {
        // Given / When / Then
        assert_eq!(
            key("DATABASE_URL=postgres://u:secret@h/db"),
            Some("DATABASE_URL")
        );
        assert_eq!(key("  export PORT = 3000"), Some("PORT"));
        assert_eq!(key("# OLD_KEY=1"), None);
        assert_eq!(key(""), None);
        assert_eq!(key("not a pair"), None);
        assert_eq!(key("A B=1"), None);
    }

    #[test]
    fn finds_missing_and_extra_keys_without_keeping_values() {
        // Given: a root pair and a package with only an example
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path();
        std::fs::write(
            repo.join(".env.example"),
            "# db\nDATABASE_URL=\nPORT=3000\nNEW_FLAG=\n",
        )
        .unwrap();
        std::fs::write(
            repo.join(".env"),
            "DATABASE_URL=postgres://u:hunter2@h/db\nPORT=4000\nLOCAL_ONLY=x\n",
        )
        .unwrap();
        std::fs::create_dir_all(repo.join("apps/web")).unwrap();
        std::fs::write(repo.join("apps/web/.env.example"), "VITE_URL=\n").unwrap();
        std::fs::create_dir_all(repo.join("node_modules/x")).unwrap();
        std::fs::write(repo.join("node_modules/x/.env.example"), "NO=\n").unwrap();

        // When
        let drift = scan(repo);

        // Then
        assert_eq!(drift.len(), 2, "{drift:?}");
        assert_eq!(drift[0].dir, "");
        assert_eq!(drift[0].missing, ["NEW_FLAG"]);
        assert_eq!(drift[0].extra, ["LOCAL_ONLY"]);
        assert!(
            !format!("{drift:?}").contains("hunter2"),
            "no value is kept"
        );
        assert_eq!(drift[1].dir, "apps/web");
        assert!(!drift[1].has_env && !drift[1].is_clean());
    }

    #[test]
    fn odd_lines_are_names_or_nothing() {
        // Given / When / Then
        assert_eq!(key("KEY=\"a=b\""), Some("KEY"));
        assert_eq!(key("KEY="), Some("KEY"));
        assert_eq!(key("=value"), None);
        assert!(
            key("\u{feff}BOM=1").is_none(),
            "a BOM is not part of a name"
        );
        assert_eq!(key("  # KEY=1"), None);
        assert_eq!(key("KEY: 1"), None);
        assert_eq!(key("lower_case=1"), Some("lower_case"));
    }

    #[test]
    fn crlf_bom_and_duplicates_do_not_count_as_drift() {
        // Given: the same keys, one file with CRLF, a BOM and a duplicate
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(".env.example"), "\u{feff}A=\r\nB=\r\n").unwrap();
        std::fs::write(dir.path().join(".env"), "A=1\nB=2\nB=3\n").unwrap();

        // When
        let drift = scan(dir.path());

        // Then
        assert!(drift[0].is_clean(), "{drift:?}");
    }
}
