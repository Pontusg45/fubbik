use super::*;

pub(super) fn scan_metadata(
    directory: &Path,
    package: Option<&Value>,
) -> Result<Vec<DiscoveredChunk>> {
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
