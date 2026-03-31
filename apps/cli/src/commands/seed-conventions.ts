import { readFileSync } from "node:fs";

import { Command } from "commander";

import { formatSuccess } from "../lib/colors";
import { output, outputError } from "../lib/output";
import { getServerUrl } from "../lib/store";

interface Convention {
    title: string;
    content: string;
    tags: string[];
    appliesTo?: string[];
}

function extractConventions(markdown: string): Convention[] {
    const conventions: Convention[] = [];

    // UI conventions
    if (markdown.includes("render") && markdown.includes("asChild")) {
        conventions.push({
            title: "Use render prop pattern, not asChild",
            content:
                "shadcn-ui is built on @base-ui/react which uses the `render` prop pattern, NOT Radix `asChild`. All dialog triggers, menu items, and interactive components must use `render={<Component />}` instead of `asChild`.",
            tags: ["convention", "ui", "base-ui"],
            appliesTo: ["apps/web/src/**/*.tsx"],
        });
    }

    if (markdown.includes("DropdownMenuSeparator") && markdown.includes("plain HTML")) {
        conventions.push({
            title: "DropdownMenu separators use plain HTML",
            content:
                "DropdownMenuSeparator and DropdownMenuLabel use plain HTML elements (NOT base-ui primitives) to avoid Menu.Group context requirement.",
            tags: ["convention", "ui"],
            appliesTo: ["apps/web/src/**/*.tsx"],
        });
    }

    // Backend architecture
    if (markdown.includes("Repository") && markdown.includes("Service") && markdown.includes("Route")) {
        conventions.push({
            title: "Backend: Repository -> Service -> Route pattern",
            content:
                "Repositories (packages/db/src/repository/) return Effect<T, DatabaseError>. Services (packages/api/src/*/service.ts) compose repository Effects, add business logic, introduce NotFoundError/AuthError/ValidationError. Routes (packages/api/src/*/routes.ts) call Effect.runPromise(requireSession(ctx).pipe(...)). Errors propagate to global .onError handler.",
            tags: ["convention", "architecture", "backend"],
            appliesTo: ["packages/api/src/**/*.ts", "packages/db/src/repository/**/*.ts"],
        });
    }

    // Error handling
    if (markdown.includes("Effect") && markdown.includes("FiberFailure")) {
        conventions.push({
            title: "Effect for typed error handling",
            content:
                "Use the Effect library for typed errors in the service layer. Repositories return Effect<T, DatabaseError>. Services introduce NotFoundError, AuthError, ValidationError. The global error handler extracts Effect errors from FiberFailure and maps _tag to HTTP status codes (ValidationError->400, AuthError->401, NotFoundError->404, DatabaseError->500).",
            tags: ["convention", "error-handling", "effect"],
            appliesTo: ["packages/api/src/**/*.ts"],
        });
    }

    // Validation
    if (markdown.includes("Elysia") && markdown.includes("arktype was removed")) {
        conventions.push({
            title: "Elysia t schema for validation, not arktype",
            content:
                "Use Elysia's built-in `t` schema for request validation. Arktype was removed from the project. All body/query validation should use t.Object, t.String, t.Optional, etc.",
            tags: ["convention", "validation", "backend"],
            appliesTo: ["packages/api/src/**/routes.ts"],
        });
    }

    // Frontend structure
    if (markdown.includes("Feature-based Structure") || markdown.includes("features/")) {
        conventions.push({
            title: "Frontend feature-based structure",
            content:
                "Route files in apps/web/src/routes/. Feature components in apps/web/src/features/ (e.g., features/auth/, features/graph/). Shared UI in apps/web/src/components/ui/. Shared page components: PageContainer, PageHeader, PageLoading, PageEmpty in components/ui/page.tsx.",
            tags: ["convention", "frontend", "architecture"],
            appliesTo: ["apps/web/src/**/*.tsx"],
        });
    }

    // AI / Ollama
    if (markdown.includes("Ollama") && markdown.includes("vercel-ai SDK was removed")) {
        conventions.push({
            title: "Ollama for AI, not Vercel AI SDK",
            content:
                "AI features use Ollama directly for embeddings (nomic-embed-text) and generation (llama3.2). The Vercel AI SDK was removed. OLLAMA_URL env var defaults to http://localhost:11434.",
            tags: ["convention", "ai", "ollama"],
            appliesTo: ["packages/api/src/ollama/**/*.ts", "packages/api/src/enrich/**/*.ts"],
        });
    }

    // VS Code extension
    if (markdown.includes("does NOT import from other fubbik packages")) {
        conventions.push({
            title: "VS Code extension is standalone",
            content:
                "The VS Code extension at apps/vscode/ does NOT import from other fubbik packages. It communicates with the fubbik API via HTTP (fetch-based) and is bundled to CJS via esbuild.",
            tags: ["convention", "vscode", "architecture"],
            appliesTo: ["apps/vscode/**/*.ts"],
        });
    }

    // Tooling
    if (markdown.includes("pnpm") && markdown.includes("bun")) {
        conventions.push({
            title: "Package manager is pnpm, runtime is bun",
            content:
                "Use pnpm for package management (pnpm install, pnpm add). Use bun for runtime execution (bun run, bun test). TypeScript checking uses tsgo (pnpm run check-types).",
            tags: ["convention", "tooling"],
            appliesTo: ["package.json", "pnpm-workspace.yaml"],
        });
    }

    // Database
    if (markdown.includes("drizzle") && markdown.includes("pgvector")) {
        conventions.push({
            title: "Database uses drizzle ORM with postgres",
            content:
                "Database schema defined with drizzle ORM. Uses pgvector extension for embeddings and pg_trgm for fuzzy text search. Schema push via pnpm db:push. Studio via pnpm db:studio.",
            tags: ["convention", "database", "drizzle"],
            appliesTo: ["packages/db/src/**/*.ts"],
        });
    }

    return conventions;
}

export const seedConventionsCommand = new Command("seed-conventions")
    .description("Extract conventions from CLAUDE.md and create chunks")
    .option("--file <path>", "path to CLAUDE.md", "CLAUDE.md")
    .option("--dry-run", "show what would be created without creating")
    .action(async (opts: { file: string; dryRun?: boolean }, cmd: Command) => {
        let markdown: string;
        try {
            markdown = readFileSync(opts.file, "utf-8");
        } catch {
            outputError(`Could not read ${opts.file}`);
            process.exit(1);
        }

        const conventions = extractConventions(markdown);

        if (opts.dryRun) {
            for (const c of conventions) {
                console.log(`  [convention] ${c.title}`);
                console.log(`    Tags: ${c.tags.join(", ")}`);
                if (c.appliesTo) console.log(`    Applies to: ${c.appliesTo.join(", ")}`);
                console.log();
            }
            console.log(`Dry run: ${conventions.length} conventions would be created.`);
            output(cmd, conventions, "");
            return;
        }

        const serverUrl = getServerUrl();
        if (!serverUrl) {
            outputError("Server URL required. Run 'fubbik init --server <url>' first.");
            process.exit(1);
        }

        let created = 0;
        for (const c of conventions) {
            try {
                const res = await fetch(`${serverUrl}/api/chunks`, {
                    method: "POST",
                    headers: { "Content-Type": "application/json" },
                    body: JSON.stringify({
                        title: c.title,
                        content: c.content,
                        type: "convention",
                        tags: c.tags,
                    }),
                });
                if (res.ok) {
                    const chunk = (await res.json()) as { id: string };
                    // Set applies-to patterns if any
                    if (c.appliesTo && c.appliesTo.length > 0) {
                        await fetch(`${serverUrl}/api/chunks/${chunk.id}/applies-to`, {
                            method: "PUT",
                            headers: { "Content-Type": "application/json" },
                            body: JSON.stringify({ patterns: c.appliesTo.map((p) => ({ pattern: p })) }),
                        });
                    }
                    created++;
                    console.log(formatSuccess(`${c.title}`));
                } else {
                    outputError(`Failed to create: ${c.title} (${res.status})`);
                }
            } catch (err) {
                outputError(`Error creating ${c.title}: ${err}`);
            }
        }

        output(cmd, { created, total: conventions.length }, `\nCreated ${created} of ${conventions.length} convention chunks.`);
    });
