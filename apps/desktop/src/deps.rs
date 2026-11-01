//! Dependency alignment across projects: what each registered repo declares
//! (package.json, Cargo.toml, toolchain pins), grouped by name, and where the
//! repos disagree. Declared requirements, not lockfiles: `^18.2.0` and
//! `18.3.1` are both "18.x", so they align; `18` and `19` do not.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// How deep manifests are looked for (apps/web/package.json is depth 2).
const MAX_DEPTH: usize = 4;

/// Folders never searched.
const SKIP_DIRS: &[&str] = &[
    "node_modules",
    "target",
    ".git",
    "dist",
    "build",
    ".next",
    ".turbo",
    "vendor",
    "legacy",
    ".worktrees",
    ".claude",
    "coverage",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Eco {
    Toolchain,
    Npm,
    Cargo,
}

impl Eco {
    pub fn label(self) -> &'static str {
        match self {
            Eco::Toolchain => "toolchain",
            Eco::Npm => "npm",
            Eco::Cargo => "cargo",
        }
    }
}

/// One declaration of a dependency in one manifest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decl {
    pub eco: Eco,
    pub name: String,
    /// As written (`^18.2.0`, `1`, `20.x`).
    pub spec: String,
    /// The manifest, relative to the repo.
    pub file: PathBuf,
    pub dev: bool,
}

/// How the repos' versions of one dependency relate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Align {
    /// Different breaking versions (majors; minors below 1.0).
    Breaking,
    /// Same breaking version, different minors.
    Drift,
    Aligned,
    /// Versions that could not be read (`*`, a git URL) next to ones that could.
    Unknown,
}

impl Align {
    pub fn label(self) -> &'static str {
        match self {
            Align::Breaking => "different majors",
            Align::Drift => "minor drift",
            Align::Aligned => "aligned",
            Align::Unknown => "unreadable",
        }
    }
}

/// One dependency across every repo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    pub eco: Eco,
    pub name: String,
    /// Per repo (same order as given), its declarations; empty: not used.
    pub per_repo: Vec<Vec<Decl>>,
    pub align: Align,
    /// The newest version any repo declares: cells are colored by how far
    /// behind it they are.
    pub newest: Option<(u64, u64)>,
}

impl Row {
    pub fn repos_using(&self) -> usize {
        self.per_repo.iter().filter(|d| !d.is_empty()).count()
    }

    /// How far one repo's declarations are behind the newest: its oldest
    /// readable version against [`Row::newest`].
    pub fn lag(&self, repo: usize) -> Align {
        let oldest = self.per_repo[repo]
            .iter()
            .filter_map(|d| version_of(&d.spec))
            .min();
        match (oldest, self.newest) {
            (Some(v), Some(n)) if breaking(v) != breaking(n) => Align::Breaking,
            (Some(v), Some(n)) if v != n => Align::Drift,
            (None, Some(_)) if !self.per_repo[repo].is_empty() => Align::Unknown,
            _ => Align::Aligned,
        }
    }

    /// The distinct requirements one repo declares (`^18.2.0 · ^18`).
    pub fn specs(&self, repo: usize) -> String {
        let mut specs: Vec<&str> = self.per_repo[repo]
            .iter()
            .map(|d| d.spec.as_str())
            .collect();
        specs.sort();
        specs.dedup();
        specs.join(" · ")
    }
}

/// `(major, minor)` of the first version in a requirement: `^18.2.0` →
/// (18, 2), `>=1.70` → (1, 70), `20.x` → (20, 0), `1` → (1, 0). None for
/// what is not a version (`*`, `latest`, URLs, `workspace:*`).
pub fn version_of(spec: &str) -> Option<(u64, u64)> {
    let spec = spec.trim();
    if spec.contains("://") || spec.contains(':') || spec.contains('/') {
        return None;
    }
    let start = spec.find(|c: char| c.is_ascii_digit())?;
    // Only operators before the number (`^`, `~`, `>=`, `=`, `v`).
    if !spec[..start].chars().all(|c| "^~>=<v ".contains(c)) {
        return None;
    }
    let mut parts = spec[start..]
        .split(|c: char| !c.is_ascii_digit() && c != '.' && c != 'x' && c != '*')
        .next()?
        .split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next().and_then(|m| m.parse().ok()).unwrap_or(0);
    Some((major, minor))
}

/// The part of a version that breaks compatibility: the major, or for 0.x
/// the minor too (`0.8`).
pub fn breaking(version: (u64, u64)) -> String {
    match version {
        (0, minor) => format!("0.{minor}"),
        (major, _) => major.to_string(),
    }
}

fn walk(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if kind.is_dir() {
            if depth < MAX_DEPTH && !SKIP_DIRS.contains(&name.as_str()) && !name.starts_with('.') {
                walk(&path, depth + 1, out);
            }
        } else if matches!(
            name.as_str(),
            "package.json"
                | "Cargo.toml"
                | "rust-toolchain.toml"
                | "rust-toolchain"
                | ".nvmrc"
                | ".node-version"
                | ".tool-versions"
                | "mise.toml"
                | ".mise.toml"
                | "pnpm-workspace.yaml"
        ) {
            out.push(path);
        }
    }
}

/// pnpm catalogs from pnpm-workspace.yaml: `catalog:` (the default, key "")
/// and `catalogs: { name: {...} }`. A line-based read of the simple form.
pub fn parse_catalogs(yaml: &str) -> BTreeMap<String, BTreeMap<String, String>> {
    let mut out: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
    let mut current: Option<String> = None;
    let mut in_named = false;
    let unquote = |s: &str| s.trim().trim_matches(|c| c == '"' || c == '\'').to_string();
    for line in yaml.lines() {
        if line.trim().is_empty() || line.trim_start().starts_with('#') {
            continue;
        }
        let indent = line.len() - line.trim_start().len();
        let text = line.trim();
        if indent == 0 {
            in_named = text == "catalogs:";
            current = (text == "catalog:").then(String::new);
            continue;
        }
        let Some((key, value)) = text.split_once(':') else {
            continue;
        };
        let (key, value) = (unquote(key), unquote(value));
        if in_named && value.is_empty() {
            current = Some(key);
        } else if let Some(catalog) = &current
            && !value.is_empty()
        {
            out.entry(catalog.clone()).or_default().insert(key, value);
        }
    }
    out
}

fn push(out: &mut Vec<Decl>, eco: Eco, name: &str, spec: &str, file: &Path, dev: bool) {
    let spec = spec.trim();
    if spec.is_empty() {
        return;
    }
    out.push(Decl {
        eco,
        name: name.to_string(),
        spec: spec.to_string(),
        file: file.to_path_buf(),
        dev,
    });
}

fn scan_package_json(
    text: &str,
    file: &Path,
    catalogs: &BTreeMap<String, BTreeMap<String, String>>,
    out: &mut Vec<Decl>,
) {
    let Ok(json) = serde_json::from_str::<serde_json::Value>(text) else {
        return;
    };
    for (section, dev) in [
        ("dependencies", false),
        ("devDependencies", true),
        ("peerDependencies", false),
        ("optionalDependencies", false),
    ] {
        let Some(deps) = json[section].as_object() else {
            continue;
        };
        for (name, spec) in deps {
            let Some(spec) = spec.as_str() else { continue };
            let spec = match spec.strip_prefix("catalog:") {
                Some(catalog) => match catalogs.get(catalog.trim()).and_then(|c| c.get(name)) {
                    Some(resolved) => resolved.as_str(),
                    None => continue,
                },
                None => spec,
            };
            // Internal and non-registry: not something to align.
            if [
                "workspace:",
                "link:",
                "file:",
                "npm:",
                "git",
                "http",
                "github:",
                "portal:",
            ]
            .iter()
            .any(|p| spec.starts_with(p))
            {
                continue;
            }
            push(out, Eco::Npm, name, spec, file, dev);
        }
    }
    if let Some(node) = json["engines"]["node"].as_str() {
        push(out, Eco::Toolchain, "node", node, file, false);
    }
    // `pnpm@10.10.0+sha512…`
    if let Some(pm) = json["packageManager"].as_str()
        && let Some((tool, version)) = pm.split_once('@')
    {
        let version = version.split('+').next().unwrap_or(version);
        push(out, Eco::Toolchain, tool, version, file, false);
    }
}

fn scan_cargo_toml(text: &str, file: &Path, out: &mut Vec<Decl>) {
    let Ok(value) = text.parse::<toml::Table>() else {
        return;
    };
    let mut tables: Vec<(&toml::Table, bool)> = Vec::new();
    for (key, dev) in [
        ("dependencies", false),
        ("dev-dependencies", true),
        ("build-dependencies", true),
    ] {
        if let Some(t) = value.get(key).and_then(|v| v.as_table()) {
            tables.push((t, dev));
        }
    }
    if let Some(t) = value
        .get("workspace")
        .and_then(|w| w.get("dependencies"))
        .and_then(|v| v.as_table())
    {
        tables.push((t, false));
    }
    for (table, dev) in tables {
        for (name, spec) in table {
            let version = match spec {
                toml::Value::String(s) => Some(s.as_str()),
                // `{ workspace = true }` is counted where the workspace
                // declares it; path- and git-only deps are internal.
                toml::Value::Table(t) => t.get("version").and_then(|v| v.as_str()),
                _ => None,
            };
            // A renamed dependency counts under its crate's name.
            let name = spec.get("package").and_then(|p| p.as_str()).unwrap_or(name);
            if let Some(version) = version {
                push(out, Eco::Cargo, name, version, file, dev);
            }
        }
    }
    let package = value
        .get("package")
        .or_else(|| value.get("workspace").and_then(|w| w.get("package")));
    if let Some(package) = package {
        if let Some(msrv) = package.get("rust-version").and_then(|v| v.as_str()) {
            push(out, Eco::Toolchain, "rust (MSRV)", msrv, file, false);
        }
        if let Some(edition) = package.get("edition").and_then(|v| v.as_str()) {
            push(out, Eco::Toolchain, "rust edition", edition, file, false);
        }
    }
}

/// Every declaration in the repo at `root`.
pub fn scan(root: &Path) -> Vec<Decl> {
    let mut files = Vec::new();
    walk(root, 0, &mut files);
    files.sort();
    let catalogs = files
        .iter()
        .find(|f| f.file_name().is_some_and(|n| n == "pnpm-workspace.yaml"))
        .and_then(|f| std::fs::read_to_string(f).ok())
        .map(|t| parse_catalogs(&t))
        .unwrap_or_default();
    let mut out = Vec::new();
    for path in files {
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let rel = path.strip_prefix(root).unwrap_or(&path).to_path_buf();
        match path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default()
        {
            "package.json" => scan_package_json(&text, &rel, &catalogs, &mut out),
            "Cargo.toml" => scan_cargo_toml(&text, &rel, &mut out),
            "rust-toolchain.toml" => {
                if let Some(channel) = text.parse::<toml::Table>().ok().and_then(|t| {
                    t.get("toolchain")?
                        .get("channel")?
                        .as_str()
                        .map(str::to_string)
                }) {
                    push(&mut out, Eco::Toolchain, "rust", &channel, &rel, false);
                }
            }
            "rust-toolchain" => push(&mut out, Eco::Toolchain, "rust", text.trim(), &rel, false),
            ".nvmrc" | ".node-version" => {
                push(&mut out, Eco::Toolchain, "node", text.trim(), &rel, false)
            }
            ".tool-versions" => {
                for line in text.lines() {
                    let mut words = line.split_whitespace();
                    if let (Some(tool), Some(version)) = (words.next(), words.next())
                        && !tool.starts_with('#')
                    {
                        let tool = if tool == "nodejs" { "node" } else { tool };
                        push(&mut out, Eco::Toolchain, tool, version, &rel, false);
                    }
                }
            }
            "mise.toml" | ".mise.toml" => {
                if let Some(tools) = text
                    .parse::<toml::Table>()
                    .ok()
                    .and_then(|t| t.get("tools")?.as_table().cloned())
                {
                    for (tool, version) in tools {
                        if let Some(version) = version.as_str() {
                            push(&mut out, Eco::Toolchain, &tool, version, &rel, false);
                        }
                    }
                }
            }
            _ => {}
        }
    }
    out
}

/// Groups declarations by dependency across repos and says how they align.
/// Only dependencies used in at least `min_repos` repos are kept.
pub fn compare(repos: &[Vec<Decl>], min_repos: usize) -> Vec<Row> {
    let mut names: BTreeSet<(Eco, String)> = BTreeSet::new();
    for decls in repos {
        names.extend(decls.iter().map(|d| (d.eco, d.name.clone())));
    }
    let mut rows: Vec<Row> = names
        .into_iter()
        .map(|(eco, name)| {
            let per_repo: Vec<Vec<Decl>> = repos
                .iter()
                .map(|decls| {
                    decls
                        .iter()
                        .filter(|d| d.eco == eco && d.name == name)
                        .cloned()
                        .collect()
                })
                .collect();
            let all: Vec<&Decl> = per_repo.iter().flatten().collect();
            let versions: Vec<(u64, u64)> =
                all.iter().filter_map(|d| version_of(&d.spec)).collect();
            let unreadable = versions.len() < all.len();
            let keys: BTreeSet<String> = versions.iter().map(|v| breaking(*v)).collect();
            let minors: BTreeSet<(u64, u64)> = versions.iter().copied().collect();
            // The edition is a year: any difference is "different".
            let align = if keys.len() > 1 {
                Align::Breaking
            } else if minors.len() > 1 {
                Align::Drift
            } else if unreadable && !versions.is_empty() {
                Align::Unknown
            } else {
                Align::Aligned
            };
            Row {
                eco,
                name,
                per_repo,
                align,
                newest: versions.iter().max().copied(),
            }
        })
        .filter(|r| r.repos_using() >= min_repos)
        .collect();
    rows.sort_by(|a, b| {
        (a.align, a.eco, std::cmp::Reverse(a.repos_using()), &a.name).cmp(&(
            b.align,
            b.eco,
            std::cmp::Reverse(b.repos_using()),
            &b.name,
        ))
    });
    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decl(eco: Eco, name: &str, spec: &str) -> Decl {
        Decl {
            eco,
            name: name.into(),
            spec: spec.into(),
            file: "package.json".into(),
            dev: false,
        }
    }

    #[test]
    fn reads_versions_from_requirements() {
        // Given
        let specs = [
            "^18.2.0",
            "~1.4",
            ">=1.70",
            "20.x",
            "1",
            "v22.11.0",
            "*",
            "latest",
            "workspace:*",
            "github:o/r",
            "0.8.3",
            ">=2 <3",
        ];

        // When
        let versions: Vec<Option<(u64, u64)>> = specs.iter().map(|s| version_of(s)).collect();

        // Then
        assert_eq!(
            versions,
            [
                Some((18, 2)),
                Some((1, 4)),
                Some((1, 70)),
                Some((20, 0)),
                Some((1, 0)),
                Some((22, 11)),
                None,
                None,
                None,
                None,
                Some((0, 8)),
                Some((2, 0)),
            ]
        );
        assert_eq!(breaking((0, 8)), "0.8");
        assert_eq!(breaking((18, 2)), "18");
    }

    #[test]
    fn flags_majors_then_drift_and_keeps_shared_only() {
        // Given: react differs by major, axum below 1.0 by minor (breaking),
        // tokio by minor only, serde aligned, lodash in one repo only
        let a = vec![
            decl(Eco::Npm, "react", "^18.2.0"),
            decl(Eco::Cargo, "axum", "0.7"),
            decl(Eco::Cargo, "tokio", "1.38"),
            decl(Eco::Cargo, "serde", "1"),
            decl(Eco::Npm, "lodash", "^4"),
        ];
        let b = vec![
            decl(Eco::Npm, "react", "^19.0.0"),
            decl(Eco::Cargo, "axum", "0.8"),
            decl(Eco::Cargo, "tokio", "1.40"),
            decl(Eco::Cargo, "serde", "1.0.200"),
        ];

        // When
        let rows = compare(&[a, b], 2);

        // Then
        let summary: Vec<(&str, Align)> = rows.iter().map(|r| (r.name.as_str(), r.align)).collect();
        assert_eq!(
            summary,
            [
                ("react", Align::Breaking),
                ("axum", Align::Breaking),
                ("tokio", Align::Drift),
                ("serde", Align::Aligned),
            ]
        );
        assert_eq!(rows[0].per_repo[1][0].spec, "^19.0.0");
        assert_eq!(rows[0].newest, Some((19, 0)));
        assert_eq!(
            (rows[0].lag(0), rows[0].lag(1)),
            (Align::Breaking, Align::Aligned)
        );
        assert_eq!(
            (rows[2].lag(0), rows[2].lag(1)),
            (Align::Drift, Align::Aligned)
        );
    }

    #[test]
    fn scans_package_json_cargo_and_toolchain_pins() {
        // Given: a small monorepo
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::write(
            root.join("package.json"),
            r#"{"packageManager": "pnpm@10.10.0+sha512.abc", "engines": {"node": ">=22"},
                "devDependencies": {"typescript": "catalog:", "internal": "workspace:*"}}"#,
        )
        .unwrap();
        std::fs::write(
            root.join("pnpm-workspace.yaml"),
            "packages:\n  - apps/*\ncatalog:\n  typescript: ^5.9.0\n  \"@types/node\": ^24\n",
        )
        .unwrap();
        std::fs::create_dir_all(root.join("apps/web")).unwrap();
        std::fs::write(
            root.join("apps/web/package.json"),
            r#"{"dependencies": {"react": "^19.1.0", "@types/node": "catalog:"}}"#,
        )
        .unwrap();
        std::fs::create_dir_all(root.join("node_modules/x")).unwrap();
        std::fs::write(
            root.join("node_modules/x/package.json"),
            r#"{"dependencies": {"ignored": "1"}}"#,
        )
        .unwrap();
        std::fs::write(
            root.join("Cargo.toml"),
            "[workspace.package]\nedition = \"2024\"\nrust-version = \"1.85\"\n[workspace.dependencies]\ntokio = { version = \"1.40\", features = [\"rt\"] }\nlocal = { path = \"crates/local\" }\n",
        )
        .unwrap();
        std::fs::write(
            root.join("rust-toolchain.toml"),
            "[toolchain]\nchannel = \"1.90\"\n",
        )
        .unwrap();
        std::fs::write(root.join(".nvmrc"), "22.11.0\n").unwrap();

        // When
        let decls = scan(root);

        // Then
        let mut found: Vec<(Eco, &str, &str)> = decls
            .iter()
            .map(|d| (d.eco, d.name.as_str(), d.spec.as_str()))
            .collect();
        found.sort();
        assert_eq!(
            found,
            [
                (Eco::Toolchain, "node", "22.11.0"),
                (Eco::Toolchain, "node", ">=22"),
                (Eco::Toolchain, "pnpm", "10.10.0"),
                (Eco::Toolchain, "rust", "1.90"),
                (Eco::Toolchain, "rust (MSRV)", "1.85"),
                (Eco::Toolchain, "rust edition", "2024"),
                (Eco::Npm, "@types/node", "^24"),
                (Eco::Npm, "react", "^19.1.0"),
                (Eco::Npm, "typescript", "^5.9.0"),
                (Eco::Cargo, "tokio", "1.40"),
            ]
        );
    }
}

#[cfg(test)]
mod live {
    /// This machine's repos (`cargo test print_alignment -- --ignored --nocapture`).
    #[test]
    #[ignore]
    fn print_alignment() {
        // Given
        let home = std::env::var("HOME").unwrap();
        let repos: Vec<String> = ["fubbik", "vpg-bot-v2", "vpg-platform", "multibots"]
            .iter()
            .map(|r| format!("{home}/projects/{r}"))
            .collect();

        // When
        let started = std::time::Instant::now();
        let scans: Vec<Vec<super::Decl>> = repos.iter().map(|r| super::scan(r.as_ref())).collect();
        let rows = super::compare(&scans, 2);

        // Then
        for (repo, decls) in repos.iter().zip(&scans) {
            println!("{repo}: {} declarations", decls.len());
        }
        for row in rows
            .iter()
            .filter(|r| r.align != super::Align::Aligned)
            .take(40)
        {
            let cells: Vec<String> = row
                .per_repo
                .iter()
                .map(|d| {
                    d.iter()
                        .map(|d| d.spec.clone())
                        .collect::<Vec<_>>()
                        .join(",")
                })
                .collect();
            println!(
                "{:?} {:9} {:28} {:?}",
                row.align,
                row.eco.label(),
                row.name,
                cells
            );
        }
        let aligned = rows
            .iter()
            .filter(|r| r.align == super::Align::Aligned)
            .count();
        println!(
            "{} shared, {aligned} aligned, in {:?}",
            rows.len(),
            started.elapsed()
        );
    }
}
