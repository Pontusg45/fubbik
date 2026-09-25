use std::path::Path;

use anyhow::{Context, Result};
use serde::Serialize;

use crate::client::Client;
use crate::output::{self, OutputMode};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Convention {
    title: &'static str,
    content: &'static str,
    tags: &'static [&'static str],
    applies_to: &'static [&'static str],
}

pub async fn run(client: &Client, file: &Path, dry_run: bool, mode: OutputMode) -> Result<()> {
    let markdown = std::fs::read_to_string(file)
        .with_context(|| format!("could not read {}", file.display()))?;
    let conventions = extract(&markdown);
    if dry_run {
        return render_dry_run(&conventions, mode);
    }

    let mut created = Vec::new();
    let mut errors = Vec::new();
    for convention in &conventions {
        let tags = convention
            .tags
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>();
        match client
            .create_chunk(
                convention.title,
                convention.content,
                "convention",
                &tags,
                &[],
            )
            .await
        {
            Ok(chunk) => {
                let patterns = convention
                    .applies_to
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>();
                if let Err(error) = client.set_applies_to(&chunk.id, &patterns).await {
                    errors.push(format!("{}: {error}", convention.title));
                }
                created.push(serde_json::json!({"id": chunk.id, "title": chunk.title}));
            }
            Err(error) => errors.push(format!("{}: {error}", convention.title)),
        }
    }
    let result = serde_json::json!({
        "created": created.len(),
        "total": conventions.len(),
        "chunks": created,
        "errors": errors,
    });
    if mode == OutputMode::Json {
        return output::json(&result);
    }
    if mode == OutputMode::Quiet {
        for chunk in result["chunks"].as_array().into_iter().flatten() {
            println!("{}", chunk["id"].as_str().unwrap_or(""));
        }
        return Ok(());
    }
    for chunk in result["chunks"].as_array().into_iter().flatten() {
        println!("Created {}", chunk["title"].as_str().unwrap_or("Untitled"));
    }
    for error in &errors {
        eprintln!("Failed: {error}");
    }
    println!(
        "\nCreated {} of {} convention chunks.",
        created.len(),
        conventions.len()
    );
    Ok(())
}

fn render_dry_run(conventions: &[Convention], mode: OutputMode) -> Result<()> {
    if mode == OutputMode::Json {
        return output::json(&conventions);
    }
    for convention in conventions {
        if mode == OutputMode::Quiet {
            println!("{}", convention.title);
            continue;
        }
        println!("  [convention] {}", convention.title);
        println!("    Tags: {}", convention.tags.join(", "));
        if !convention.applies_to.is_empty() {
            println!("    Applies to: {}", convention.applies_to.join(", "));
        }
        println!();
    }
    if mode == OutputMode::Human {
        println!(
            "Dry run: {} conventions would be created.",
            conventions.len()
        );
    }
    Ok(())
}

fn extract(markdown: &str) -> Vec<Convention> {
    let candidates = [
        Convention {
            title: "Use render prop pattern, not asChild",
            content: "shadcn-ui is built on @base-ui/react which uses the `render` prop pattern, NOT Radix `asChild`. All dialog triggers, menu items, and interactive components must use `render={<Component />}` instead of `asChild`.",
            tags: &["convention", "ui", "base-ui"],
            applies_to: &["apps/web/src/**/*.tsx"],
        },
        Convention {
            title: "DropdownMenu separators use plain HTML",
            content: "DropdownMenuSeparator and DropdownMenuLabel use plain HTML elements (NOT base-ui primitives) to avoid Menu.Group context requirement.",
            tags: &["convention", "ui"],
            applies_to: &["apps/web/src/**/*.tsx"],
        },
        Convention {
            title: "Backend: Repository -> Service -> Route pattern",
            content: "Repositories (packages/db/src/repository/) return Effect<T, DatabaseError>. Services (packages/api/src/*/service.ts) compose repository Effects, add business logic, introduce NotFoundError/AuthError/ValidationError. Routes (packages/api/src/*/routes.ts) call Effect.runPromise(requireSession(ctx).pipe(...)). Errors propagate to global .onError handler.",
            tags: &["convention", "architecture", "backend"],
            applies_to: &[
                "packages/api/src/**/*.ts",
                "packages/db/src/repository/**/*.ts",
            ],
        },
        Convention {
            title: "Effect for typed error handling",
            content: "Use the Effect library for typed errors in the service layer. Repositories return Effect<T, DatabaseError>. Services introduce NotFoundError, AuthError, ValidationError. The global error handler extracts Effect errors from FiberFailure and maps _tag to HTTP status codes (ValidationError->400, AuthError->401, NotFoundError->404, DatabaseError->500).",
            tags: &["convention", "error-handling", "effect"],
            applies_to: &["packages/api/src/**/*.ts"],
        },
        Convention {
            title: "Elysia t schema for validation, not arktype",
            content: "Use Elysia's built-in `t` schema for request validation. Arktype was removed from the project. All body/query validation should use t.Object, t.String, t.Optional, etc.",
            tags: &["convention", "validation", "backend"],
            applies_to: &["packages/api/src/**/routes.ts"],
        },
        Convention {
            title: "Frontend feature-based structure",
            content: "Route files in apps/web/src/routes/. Feature components in apps/web/src/features/ (e.g., features/auth/, features/graph/). Shared UI in apps/web/src/components/ui/. Shared page components: PageContainer, PageHeader, PageLoading, PageEmpty in components/ui/page.tsx.",
            tags: &["convention", "frontend", "architecture"],
            applies_to: &["apps/web/src/**/*.tsx"],
        },
        Convention {
            title: "Ollama for AI, not Vercel AI SDK",
            content: "AI features use Ollama directly for embeddings (nomic-embed-text) and generation (llama3.2). The Vercel AI SDK was removed. OLLAMA_URL env var defaults to http://localhost:11434.",
            tags: &["convention", "ai", "ollama"],
            applies_to: &[
                "packages/api/src/ollama/**/*.ts",
                "packages/api/src/enrich/**/*.ts",
            ],
        },
        Convention {
            title: "VS Code extension is standalone",
            content: "The VS Code extension at apps/vscode/ does NOT import from other fubbik packages. It communicates with the fubbik API via HTTP (fetch-based) and is bundled to CJS via esbuild.",
            tags: &["convention", "vscode", "architecture"],
            applies_to: &["apps/vscode/**/*.ts"],
        },
        Convention {
            title: "Package manager is pnpm, runtime is bun",
            content: "Use pnpm for package management (pnpm install, pnpm add). Use bun for runtime execution (bun run, bun test). TypeScript checking uses tsgo (pnpm run check-types).",
            tags: &["convention", "tooling"],
            applies_to: &["package.json", "pnpm-workspace.yaml"],
        },
        Convention {
            title: "Database uses drizzle ORM with postgres",
            content: "Database schema is owned by forward-only Rust SQLx migrations. PostgreSQL uses pgvector, pg_trgm, and Apache AGE. Use pnpm db:studio only for read-only inspection.",
            tags: &["convention", "database", "drizzle"],
            applies_to: &["packages/db/src/**/*.ts"],
        },
    ];
    let needles: &[&[&str]] = &[
        &["render", "asChild"],
        &["DropdownMenuSeparator", "plain HTML"],
        &["Repository", "Service", "Route"],
        &["Effect", "FiberFailure"],
        &["Elysia", "arktype was removed"],
        &["features/"],
        &["Ollama", "vercel-ai SDK was removed"],
        &["does NOT import from other fubbik packages"],
        &["pnpm", "bun"],
        &["drizzle", "pgvector"],
    ];
    candidates
        .into_iter()
        .zip(needles)
        .filter(|(_, required)| required.iter().all(|needle| markdown.contains(needle)))
        .map(|(convention, _)| convention)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extraction_requires_each_conventions_full_signature() {
        // Given markdown containing one complete signature and one partial signature
        let markdown = "Use pnpm with the bun runtime. We also use Ollama.";
        // When conventions are extracted
        let conventions = extract(markdown);
        // Then only the convention whose complete signature is present is returned
        assert_eq!(conventions.len(), 1);
        assert_eq!(
            conventions[0].title,
            "Package manager is pnpm, runtime is bun"
        );
    }
}
