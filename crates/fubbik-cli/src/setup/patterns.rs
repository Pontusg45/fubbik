use super::*;

pub(super) fn scan_patterns(
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
