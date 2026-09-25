use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};
use serde::Serialize;
use serde_json::Value;

use crate::client::Client;
use crate::output::{self, OutputMode};

const IGNORED: [&str; 10] = [
    "node_modules",
    ".git",
    ".turbo",
    "dist",
    "build",
    ".next",
    ".output",
    ".cache",
    "coverage",
    ".fubbik",
];

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DiscoveredChunk {
    title: String,
    content: String,
    #[serde(rename = "type")]
    chunk_type: String,
    tags: Vec<String>,
    tier: u8,
    category: String,
    source: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    applies_to: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DiscoveredConnection {
    source_title: String,
    target_title: String,
    relation: String,
}

#[derive(Clone, Debug, Serialize)]
struct Tip {
    title: String,
    detail: String,
}

struct Discovery {
    chunks: Vec<DiscoveredChunk>,
    connections: Vec<DiscoveredConnection>,
    tags: Vec<String>,
    tips: Vec<Tip>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ImportError {
    item: String,
    error: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ImportResult {
    chunks_created: i64,
    connections_created: usize,
    errors: Vec<ImportError>,
}

pub async fn run(
    client: &Client,
    dry_run: bool,
    yes: bool,
    force: bool,
    mode: OutputMode,
) -> Result<()> {
    client.health().await.with_context(|| {
        format!(
            "cannot connect to server at {}; is it running?",
            client.base_url()
        )
    })?;

    let directory = std::env::current_dir()?.canonicalize()?;
    let local_path = directory.to_string_lossy().into_owned();
    let remote = git_remote();
    let mut space = client
        .detect_space(Some(&local_path), remote.as_deref())
        .await?;
    if space.is_none() {
        let default_name = project_name(&directory, remote.as_deref());
        let name = if remote.is_none() && !yes && mode == OutputMode::Human {
            prompt_name(&default_name)?
        } else {
            default_name
        };
        space = Some(
            client
                .create_space(&name, Some(&local_path), remote.as_deref())
                .await?,
        );
    }
    let space = space.context("space detection or creation returned no space")?;

    if mode == OutputMode::Human {
        println!("\nDetected space: {}\n", space.name);
    }
    if !force {
        let page = client.list_chunk_page(Some(&space.id), 0).await?;
        if page.total > 0 && !yes {
            if mode != OutputMode::Human {
                bail!(
                    "space already has {} chunks; pass --yes or --force to continue",
                    page.total
                );
            }
            println!("This space already has {} chunks.", page.total);
            if !confirm("Continue and add more?")? {
                println!("Aborted. Use --force to re-import.");
                return Ok(());
            }
        }
    }

    if mode == OutputMode::Human {
        println!("Scanning project...");
    }
    let Discovery {
        chunks,
        connections,
        tags,
        tips,
    } = discover(&directory)?;
    if chunks.is_empty() {
        if mode == OutputMode::Json {
            return output::json(&serde_json::json!({
                "chunks": 0,
                "message": "No knowledge sources found"
            }));
        }
        println!("No knowledge sources found in this project.");
        return Ok(());
    }

    if dry_run {
        return render_preview(&chunks, &connections, &tags, mode);
    }
    if !yes
        && mode == OutputMode::Human
        && !confirm(&format!("Import {} chunks to fubbik?", chunks.len()))?
    {
        println!("Aborted.");
        return Ok(());
    }

    let result = import(client, &space.id, &directory, &chunks, &connections).await;
    match mode {
        OutputMode::Json => output::json(&serde_json::json!({
            "chunksCreated": result.chunks_created,
            "connectionsCreated": result.connections_created,
            "errors": result.errors,
            "tips": tips,
        })),
        OutputMode::Quiet => {
            println!("{}", result.chunks_created);
            Ok(())
        }
        OutputMode::Human => {
            println!("{} chunks created", result.chunks_created);
            if result.connections_created > 0 {
                println!("{} connections established", result.connections_created);
            }
            for error in result.errors.iter().take(5) {
                eprintln!("{}: {}", error.item, error.error);
            }
            if !tips.is_empty() {
                println!("\nYou might also want to add:");
                for tip in tips {
                    println!("  • {} — {}", tip.title, tip.detail);
                }
            }
            Ok(())
        }
    }
}

fn discover(directory: &Path) -> Result<Discovery> {
    let mut chunks = scan_docs(directory)?;
    let package = read_json(&directory.join("package.json"));
    chunks.extend(scan_metadata(directory, package.as_ref())?);
    let (patterns, tips) = scan_patterns(directory, package.as_ref())?;
    chunks.extend(patterns);
    let connections = infer_connections(&chunks);
    let tags = chunks
        .iter()
        .flat_map(|chunk| chunk.tags.iter().cloned())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    Ok(Discovery {
        chunks,
        connections,
        tags,
        tips,
    })
}

fn scan_docs(directory: &Path) -> Result<Vec<DiscoveredChunk>> {
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

fn scan_metadata(directory: &Path, package: Option<&Value>) -> Result<Vec<DiscoveredChunk>> {
    let mut chunks = Vec::new();
    if let Some(package) = package {
        let dependencies = dependencies(package);
        let frameworks = [
            "next",
            "react",
            "vue",
            "svelte",
            "@angular/core",
            "elysia",
            "express",
            "fastify",
            "hono",
            "nuxt",
            "astro",
            "remix",
            "solid-js",
        ];
        let libraries = [
            "drizzle-orm",
            "prisma",
            "mongoose",
            "better-auth",
            "next-auth",
            "tailwindcss",
            "effect",
            "zod",
            "trpc",
            "@trpc/server",
            "@trpc/client",
        ];
        let tools = [
            "vitest",
            "jest",
            "typescript",
            "eslint",
            "prettier",
            "biome",
            "vite",
            "webpack",
            "esbuild",
        ];
        let groups = [
            ("Frameworks", frameworks.as_slice()),
            ("Key Libraries", libraries.as_slice()),
            ("Dev Tools", tools.as_slice()),
        ];
        let mut sections = Vec::new();
        let mut tags = vec!["tech-stack".into()];
        for (label, candidates) in groups {
            let found = candidates
                .iter()
                .filter(|dependency| dependencies.contains_key(**dependency))
                .copied()
                .collect::<Vec<_>>();
            if !found.is_empty() {
                sections.push(format!(
                    "## {label}\n{}",
                    found
                        .iter()
                        .map(|item| format!("- {item}"))
                        .collect::<Vec<_>>()
                        .join("\n")
                ));
                tags.extend(found.into_iter().map(str::to_owned));
            }
        }
        if !sections.is_empty() {
            chunks.push(chunk(
                "Tech Stack".into(),
                sections.join("\n\n"),
                "reference",
                tags,
                2,
                "tech-stack",
                "package.json".into(),
            ));
        }
        if let Some(workspaces) = workspace_patterns(package) {
            chunks.push(chunk(
                "Project Structure (Monorepo)".into(),
                format!(
                    "This project uses a monorepo structure with the following workspaces:\n\n{}",
                    workspaces
                        .iter()
                        .map(|item| format!("- `{item}`"))
                        .collect::<Vec<_>>()
                        .join("\n")
                ),
                "reference",
                vec!["monorepo".into(), "structure".into()],
                2,
                "structure",
                "package.json".into(),
            ));
        }
    }

    for filename in ["tsconfig.json", "jsconfig.json"] {
        if let Some(config) = read_json(&directory.join(filename)) {
            let options = &config["compilerOptions"];
            let mut lines = Vec::new();
            if options["strict"].as_bool() == Some(true) {
                lines.push("- **Strict mode**: enabled".into());
            }
            for (key, label) in [("target", "Target"), ("module", "Module")] {
                if let Some(value) = options[key].as_str() {
                    lines.push(format!("- **{label}**: {value}"));
                }
            }
            if !lines.is_empty() {
                chunks.push(chunk(
                    "TypeScript Configuration".into(),
                    lines.join("\n"),
                    "reference",
                    vec!["typescript".into(), "config".into(), "tooling".into()],
                    2,
                    "config",
                    filename.into(),
                ));
                break;
            }
        }
    }

    for filename in [".env.example", ".env.local.example"] {
        let path = directory.join(filename);
        if let Ok(content) = std::fs::read_to_string(&path) {
            let variables = content
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty() && !line.starts_with('#'))
                .filter_map(|line| line.split_once('=').map(|(name, _)| name.trim()))
                .collect::<Vec<_>>();
            if !variables.is_empty() {
                chunks.push(chunk(
                    "Environment Variables".into(),
                    format!(
                        "Required environment variables for this project:\n\n{}",
                        variables
                            .iter()
                            .map(|name| format!("- `{name}`"))
                            .collect::<Vec<_>>()
                            .join("\n")
                    ),
                    "schema",
                    vec!["env".into(), "config".into(), "infrastructure".into()],
                    2,
                    "config",
                    filename.into(),
                ));
                break;
            }
        }
    }
    let docker = ["docker-compose.yml", "docker-compose.yaml", "Dockerfile"]
        .into_iter()
        .filter(|name| directory.join(name).exists())
        .collect::<Vec<_>>();
    if !docker.is_empty() {
        chunks.push(chunk(
            "Infrastructure (Docker)".into(),
            format!(
                "This project uses Docker.\n\nFiles found:\n{}",
                docker
                    .iter()
                    .map(|name| format!("- {name}"))
                    .collect::<Vec<_>>()
                    .join("\n")
            ),
            "reference",
            vec!["docker".into(), "infrastructure".into(), "config".into()],
            2,
            "config",
            docker[0].into(),
        ));
    }
    for (file, name) in [("turbo.json", "Turborepo"), ("nx.json", "Nx")] {
        if directory.join(file).exists() {
            chunks.push(chunk(
                "Build Pipeline".into(),
                format!("This project uses {name} as its build pipeline orchestrator."),
                "reference",
                vec!["build".into(), "tooling".into(), "config".into()],
                2,
                "config",
                file.into(),
            ));
            break;
        }
    }
    let mut pipelines = Vec::new();
    let workflows = directory.join(".github/workflows");
    if workflows.is_dir() {
        for entry in std::fs::read_dir(workflows)? {
            let path = entry?.path();
            if matches!(
                path.extension().and_then(|value| value.to_str()),
                Some("yml" | "yaml")
            ) {
                pipelines.push(relative_path(directory, &path));
            }
        }
    }
    if directory.join(".gitlab-ci.yml").exists() {
        pipelines.push(".gitlab-ci.yml".into());
    }
    if !pipelines.is_empty() {
        chunks.push(chunk(
            "CI/CD Configuration".into(),
            format!(
                "This project has CI/CD pipelines configured.\n\nFiles found:\n{}",
                pipelines
                    .iter()
                    .map(|path| format!("- {path}"))
                    .collect::<Vec<_>>()
                    .join("\n")
            ),
            "reference",
            vec![
                "ci".into(),
                "cd".into(),
                "config".into(),
                "automation".into(),
            ],
            2,
            "config",
            pipelines[0].clone(),
        ));
    }
    Ok(chunks)
}

fn scan_patterns(
    directory: &Path,
    package: Option<&Value>,
) -> Result<(Vec<DiscoveredChunk>, Vec<Tip>)> {
    let dependencies = package.map(dependencies).unwrap_or_default();
    let detectors = [
        (
            "Routing Conventions",
            &[
                "elysia", "express", "fastify", "hono", "next", "nuxt", "astro", "remix", "koa",
            ][..],
            &["routes", "pages", "api"][..],
            "routing",
            "convention",
        ),
        (
            "Testing Conventions",
            &["vitest", "jest", "mocha"][..],
            &["__tests__", "tests", "test"][..],
            "testing",
            "convention",
        ),
        (
            "Database Conventions",
            &["drizzle-orm", "prisma", "mongoose"][..],
            &["schema", "migrations", "database", "db"][..],
            "database",
            "convention",
        ),
        (
            "Component Structure Conventions",
            &[
                "react",
                "vue",
                "svelte",
                "@angular/core",
                "solid-js",
                "preact",
                "qwik",
            ][..],
            &["components", "features", "ui"][..],
            "components",
            "reference",
        ),
        (
            "Authentication Conventions",
            &[
                "better-auth",
                "next-auth",
                "passport",
                "@auth/core",
                "lucia",
                "clerk",
                "supabase",
            ][..],
            &["auth"][..],
            "auth",
            "convention",
        ),
    ];
    let mut chunks = Vec::new();
    let mut tips = Vec::new();
    for (title, candidates, directory_names, tag, chunk_type) in detectors {
        let dependency = candidates
            .iter()
            .find(|name| dependencies.contains_key(**name));
        let mut paths = find_named_directories(directory, directory_names)?;
        let file_markers: &[&str] = match tag {
            "testing" => &[".test.", ".spec."],
            "database" => &["schema.", "migration."],
            "auth" => &["auth.ts", "auth.js"],
            _ => &[],
        };
        if !file_markers.is_empty() {
            paths.extend(find_files_containing(directory, file_markers)?);
        }
        paths.sort();
        paths.dedup();
        match (dependency, paths.is_empty()) {
            (Some(dependency), false) => {
                let relative = paths
                    .iter()
                    .map(|path| relative_path(directory, path))
                    .collect::<Vec<_>>();
                let version = dependencies
                    .get(*dependency)
                    .map(String::as_str)
                    .unwrap_or("unknown");
                let mut discovered = chunk(
                    title.into(),
                    format!(
                        "This project uses **{}** ({version}).\n\n## Detected locations\n{}",
                        dependency,
                        relative
                            .iter()
                            .map(|path| format!("- `{path}`"))
                            .collect::<Vec<_>>()
                            .join("\n")
                    ),
                    chunk_type,
                    vec![tag.into(), "conventions".into(), (*dependency).into()],
                    3,
                    if tag == "components" {
                        "structure"
                    } else {
                        "conventions"
                    },
                    format!("detected:{tag}:{dependency}"),
                );
                discovered.applies_to = relative.iter().map(|path| format!("{path}/**")).collect();
                chunks.push(discovered);
            }
            (Some(dependency), true) => tips.push(Tip {
                title: format!("Consider documenting your {tag} conventions"),
                detail: format!(
                    "Found `{dependency}` in dependencies but no matching project area."
                ),
            }),
            (None, false) => tips.push(Tip {
                title: format!("Consider documenting the detected {tag} area"),
                detail: format!("Found matching files but no recognized {tag} dependency."),
            }),
            (None, true) => {}
        }
    }
    Ok((chunks, tips))
}

fn infer_connections(chunks: &[DiscoveredChunk]) -> Vec<DiscoveredConnection> {
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

async fn import(
    client: &Client,
    space_id: &str,
    directory: &Path,
    chunks: &[DiscoveredChunk],
    connections: &[DiscoveredConnection],
) -> ImportResult {
    let mut result = ImportResult {
        chunks_created: 0,
        connections_created: 0,
        errors: Vec::new(),
    };
    let mut title_to_id = HashMap::new();
    let mut files = BTreeMap::new();
    for chunk in chunks.iter().filter(|chunk| chunk.tier == 1) {
        if let Ok(content) = std::fs::read_to_string(directory.join(&chunk.source)) {
            files.entry(chunk.source.clone()).or_insert(content);
        }
    }
    if !files.is_empty() {
        let files = files.into_iter().collect::<Vec<_>>();
        match client.import_chunk_documents(&files, space_id).await {
            Ok(value) => {
                result.chunks_created += value["created"].as_i64().unwrap_or(0);
                for error in value["errors"].as_array().into_iter().flatten() {
                    result.errors.push(ImportError {
                        item: error["path"].as_str().unwrap_or("import-docs").into(),
                        error: error["error"].as_str().unwrap_or("unknown error").into(),
                    });
                }
            }
            Err(error) => result.errors.push(ImportError {
                item: "import-docs".into(),
                error: error.to_string(),
            }),
        }
    }
    for chunk in chunks.iter().filter(|chunk| chunk.tier != 1) {
        match client
            .create_discovered_chunk(
                &chunk.title,
                &chunk.content,
                &chunk.chunk_type,
                &chunk.tags,
                space_id,
            )
            .await
        {
            Ok(created) => {
                result.chunks_created += 1;
                title_to_id.insert(chunk.title.clone(), created.id.clone());
                if !chunk.applies_to.is_empty() {
                    let _ = client.set_applies_to(&created.id, &chunk.applies_to).await;
                }
            }
            Err(error) => result.errors.push(ImportError {
                item: chunk.title.clone(),
                error: error.to_string(),
            }),
        }
    }
    let mut offset = 0;
    while let Ok(page) = client.list_chunk_page(Some(space_id), offset).await {
        let count = page.chunks.len();
        for chunk in page.chunks {
            title_to_id.entry(chunk.title).or_insert(chunk.id);
        }
        offset += count as u32;
        if count == 0 || i64::from(offset) >= page.total {
            break;
        }
    }
    for connection in connections {
        let Some(source) = title_to_id.get(&connection.source_title) else {
            continue;
        };
        let Some(target) = title_to_id.get(&connection.target_title) else {
            continue;
        };
        if client
            .create_discovered_connection(source, target, &connection.relation)
            .await
            .is_ok()
        {
            result.connections_created += 1;
        }
    }
    result
}

fn render_preview(
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

fn chunk(
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

fn collect_files<F: Fn(&Path) -> bool>(
    root: &Path,
    directory: &Path,
    depth: usize,
    max_depth: usize,
    files: &mut Vec<PathBuf>,
    predicate: F,
) -> Result<()> {
    collect_files_ref(root, directory, depth, max_depth, files, &predicate)
}

fn collect_files_ref(
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

fn find_named_directories(root: &Path, names: &[&str]) -> Result<Vec<PathBuf>> {
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

fn find_files_containing(root: &Path, markers: &[&str]) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    collect_files(root, root, 0, 4, &mut files, |path| {
        path.file_name()
            .and_then(|value| value.to_str())
            .is_some_and(|name| markers.iter().any(|marker| name.contains(marker)))
    })?;
    Ok(files)
}

fn dependencies(package: &Value) -> HashMap<String, String> {
    ["dependencies", "devDependencies", "peerDependencies"]
        .into_iter()
        .filter_map(|field| package[field].as_object())
        .flat_map(|values| values.iter())
        .filter_map(|(name, version)| Some((name.clone(), version.as_str()?.into())))
        .collect()
}

fn workspace_patterns(package: &Value) -> Option<Vec<String>> {
    let value = &package["workspaces"];
    let values = value.as_array().or_else(|| value["packages"].as_array())?;
    Some(
        values
            .iter()
            .filter_map(|value| value.as_str().map(str::to_owned))
            .collect(),
    )
}

fn read_json(path: &Path) -> Option<Value> {
    serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()
}

fn markdown_title(content: &str) -> Option<String> {
    content
        .lines()
        .find_map(|line| line.strip_prefix("# ").map(|title| title.trim().to_owned()))
}

fn markdown_links(content: &str) -> Vec<&str> {
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

fn relative_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn git_remote() -> Option<String> {
    let output = Command::new("git")
        .args(["config", "--get", "remote.origin.url"])
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn project_name(directory: &Path, remote: Option<&str>) -> String {
    remote
        .and_then(|remote| remote.trim_end_matches(".git").rsplit(['/', ':']).next())
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
        .or_else(|| directory.file_name()?.to_str().map(str::to_owned))
        .unwrap_or_else(|| "my-project".into())
}

fn prompt_name(default: &str) -> Result<String> {
    print!("No git remote detected. Space name [{default}]: ");
    io::stdout().flush()?;
    let mut value = String::new();
    io::stdin().read_line(&mut value)?;
    let value = value.trim();
    Ok(if value.is_empty() {
        default.into()
    } else {
        value.into()
    })
}

fn confirm(question: &str) -> Result<bool> {
    print!("{question} [y/N] ");
    io::stdout().flush()?;
    let mut answer = String::new();
    io::stdin().read_line(&mut answer)?;
    Ok(matches!(
        answer.trim().to_ascii_lowercase().as_str(),
        "y" | "yes"
    ))
}

#[cfg(test)]
mod tests {
    use super::{discover, infer_connections, project_name};

    #[test]
    fn discovery_combines_docs_metadata_and_code_patterns() {
        // Given a documented TypeScript project with React components and Vitest tests
        let directory = tempfile::tempdir().unwrap();
        std::fs::write(directory.path().join("README.md"), "# Example\nUses React.").unwrap();
        std::fs::write(
            directory.path().join("package.json"),
            r#"{"dependencies":{"react":"19"},"devDependencies":{"vitest":"3"}}"#,
        )
        .unwrap();
        std::fs::create_dir_all(directory.path().join("src/components")).unwrap();
        std::fs::create_dir_all(directory.path().join("tests")).unwrap();
        std::fs::create_dir_all(directory.path().join(".github/workflows")).unwrap();
        std::fs::write(
            directory.path().join(".github/workflows/check.yml"),
            "name: check",
        )
        .unwrap();

        // When the project is discovered
        let discovery = discover(directory.path()).unwrap();

        // Then all three tiers and their useful metadata are present
        assert!(discovery.chunks.iter().any(|chunk| chunk.tier == 1));
        assert!(
            discovery
                .chunks
                .iter()
                .any(|chunk| chunk.tier == 2 && chunk.title == "Tech Stack")
        );
        assert!(
            discovery
                .chunks
                .iter()
                .any(|chunk| chunk.title == "CI/CD Configuration")
        );
        assert!(
            discovery
                .chunks
                .iter()
                .any(|chunk| chunk.tier == 3 && chunk.title == "Testing Conventions")
        );
        assert!(discovery.tags.contains(&"react".to_owned()));
        assert!(discovery.tips.is_empty());
    }

    #[test]
    fn connection_inference_links_markdown_references_and_architecture() {
        // Given discovered docs that link to one another and a monorepo tech stack
        let directory = tempfile::tempdir().unwrap();
        std::fs::write(
            directory.path().join("README.md"),
            "# Home\n[Guide](docs/guide.md)",
        )
        .unwrap();
        std::fs::create_dir(directory.path().join("docs")).unwrap();
        std::fs::write(directory.path().join("docs/guide.md"), "# Guide").unwrap();
        std::fs::write(
            directory.path().join("package.json"),
            r#"{"workspaces":["apps/*"],"dependencies":{"react":"19"}}"#,
        )
        .unwrap();
        let discovery = discover(directory.path()).unwrap();

        // When connections are inferred
        let connections = infer_connections(&discovery.chunks);

        // Then document references and structural relationships are retained
        assert!(
            connections
                .iter()
                .any(|connection| connection.relation == "references")
        );
        assert!(
            connections
                .iter()
                .any(|connection| connection.relation == "part_of")
        );
    }

    #[test]
    fn project_names_support_ssh_and_https_git_remotes() {
        // Given common Git remote URL formats
        let directory = std::path::Path::new("/tmp/fallback");

        // When project names are derived
        let ssh = project_name(directory, Some("git@github.com:team/fubbik.git"));
        let https = project_name(directory, Some("https://github.com/team/fubbik.git"));

        // Then both formats produce the repository name
        assert_eq!(ssh, "fubbik");
        assert_eq!(https, "fubbik");
    }
}
