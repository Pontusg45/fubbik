import { existsSync, readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";

import type { DiscoveredChunk, Tip } from "./types";

export interface MetadataResult {
    chunks: DiscoveredChunk[];
    tips: Tip[];
}

// --- Helpers ---

function readJson<T>(path: string): T | null {
    try {
        return JSON.parse(readFileSync(path, "utf-8")) as T;
    } catch {
        return null;
    }
}

function readText(path: string): string | null {
    try {
        return readFileSync(path, "utf-8");
    } catch {
        return null;
    }
}

// --- Detection patterns ---

const FRAMEWORKS = [
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

const KEY_LIBRARIES = [
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

const DEV_TOOLS = [
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

// Map of dep name → tag
const DEP_TAG_MAP: Record<string, string> = {
    // Frameworks
    next: "framework",
    react: "framework",
    vue: "framework",
    svelte: "framework",
    "@angular/core": "framework",
    elysia: "framework",
    express: "framework",
    fastify: "framework",
    hono: "framework",
    nuxt: "framework",
    astro: "framework",
    remix: "framework",
    "solid-js": "framework",
    // Database
    "drizzle-orm": "database",
    prisma: "database",
    mongoose: "database",
    // Auth
    "better-auth": "auth",
    "next-auth": "auth",
    // UI
    tailwindcss: "styling",
    // Utilities
    effect: "tooling",
    zod: "tooling",
    trpc: "tooling",
    "@trpc/server": "tooling",
    "@trpc/client": "tooling",
    // Dev tools
    vitest: "testing",
    jest: "testing",
    typescript: "tooling",
    eslint: "tooling",
    prettier: "tooling",
    biome: "tooling",
    vite: "tooling",
    webpack: "tooling",
    esbuild: "tooling",
};

interface PackageJson {
    name?: string;
    workspaces?: string[] | { packages: string[] };
    dependencies?: Record<string, string>;
    devDependencies?: Record<string, string>;
    peerDependencies?: Record<string, string>;
}

// --- Scanners ---

function scanPackageJson(
    dir: string,
    pkg: PackageJson,
): { techStack: DiscoveredChunk | null; structure: DiscoveredChunk | null } {
    const allDeps: Record<string, string> = {
        ...pkg.dependencies,
        ...pkg.devDependencies,
        ...pkg.peerDependencies,
    };

    // Tech stack chunk
    const foundFrameworks: string[] = [];
    const foundLibraries: string[] = [];
    const foundDevTools: string[] = [];
    const tags = new Set<string>(["tech-stack"]);

    for (const dep of FRAMEWORKS) {
        if (dep in allDeps) {
            foundFrameworks.push(dep);
            const tag = DEP_TAG_MAP[dep];
            if (tag) tags.add(tag);
        }
    }
    for (const dep of KEY_LIBRARIES) {
        if (dep in allDeps) {
            foundLibraries.push(dep);
            const tag = DEP_TAG_MAP[dep];
            if (tag) tags.add(tag);
        }
    }
    for (const dep of DEV_TOOLS) {
        if (dep in allDeps) {
            foundDevTools.push(dep);
            const tag = DEP_TAG_MAP[dep];
            if (tag) tags.add(tag);
        }
    }

    let techStack: DiscoveredChunk | null = null;
    if (foundFrameworks.length > 0 || foundLibraries.length > 0 || foundDevTools.length > 0) {
        const lines: string[] = [];
        if (foundFrameworks.length > 0) {
            lines.push(`## Frameworks\n${foundFrameworks.map(f => `- ${f}`).join("\n")}`);
        }
        if (foundLibraries.length > 0) {
            lines.push(`## Key Libraries\n${foundLibraries.map(l => `- ${l}`).join("\n")}`);
        }
        if (foundDevTools.length > 0) {
            lines.push(`## Dev Tools\n${foundDevTools.map(t => `- ${t}`).join("\n")}`);
        }

        techStack = {
            title: "Tech Stack",
            content: lines.join("\n\n"),
            type: "reference",
            tags: Array.from(tags),
            tier: 2,
            category: "tech-stack",
            source: "package.json",
        };
    }

    // Monorepo structure chunk
    let structure: DiscoveredChunk | null = null;
    const workspaces = pkg.workspaces;
    if (workspaces) {
        const patterns = Array.isArray(workspaces) ? workspaces : workspaces.packages ?? [];

        // Look for workspace package names
        const workspaceDirs: string[] = [];
        for (const pattern of patterns) {
            // Resolve simple glob patterns like "apps/*" → check apps/
            const base = pattern.replace(/\/\*$/, "");
            const fullBase = join(dir, base);
            if (existsSync(fullBase)) {
                try {
                    const entries = readdirSync(fullBase, { withFileTypes: true });
                    for (const entry of entries) {
                        if (entry.isDirectory()) {
                            workspaceDirs.push(`${base}/${entry.name}`);
                        }
                    }
                } catch {
                    // ignore
                }
            } else {
                workspaceDirs.push(pattern);
            }
        }

        const lines = [
            "This project uses a monorepo structure with the following workspaces:",
            "",
            ...workspaceDirs.map(d => `- \`${d}\``),
        ];

        structure = {
            title: "Project Structure (Monorepo)",
            content: lines.join("\n"),
            type: "reference",
            tags: ["monorepo", "structure"],
            tier: 2,
            category: "structure",
            source: "package.json",
        };
    }

    return { techStack, structure };
}

interface TsConfig {
    compilerOptions?: {
        strict?: boolean;
        target?: string;
        module?: string;
        paths?: Record<string, string[]>;
        [key: string]: unknown;
    };
}

function scanTsConfig(path: string, filename: string): DiscoveredChunk | null {
    const tsconfig = readJson<TsConfig>(path);
    if (!tsconfig) return null;

    const opts = tsconfig.compilerOptions ?? {};
    const lines: string[] = [];

    if (opts.strict) lines.push("- **Strict mode**: enabled");
    if (opts.target) lines.push(`- **Target**: ${opts.target}`);
    if (opts.module) lines.push(`- **Module**: ${opts.module}`);

    const paths = opts.paths;
    if (paths && Object.keys(paths).length > 0) {
        lines.push("\n## Path Aliases");
        for (const [alias, targets] of Object.entries(paths)) {
            lines.push(`- \`${alias}\` → \`${targets[0]}\``);
        }
    }

    if (lines.length === 0) return null;

    return {
        title: "TypeScript Configuration",
        content: lines.join("\n"),
        type: "reference",
        tags: ["typescript", "config", "tooling"],
        tier: 2,
        category: "config",
        source: filename,
    };
}

function scanEnvExample(path: string, filename: string): DiscoveredChunk | null {
    const content = readText(path);
    if (!content) return null;

    const vars: string[] = [];
    for (const line of content.split("\n")) {
        const trimmed = line.trim();
        if (!trimmed || trimmed.startsWith("#")) continue;
        const eqIdx = trimmed.indexOf("=");
        if (eqIdx > 0) {
            vars.push(trimmed.slice(0, eqIdx).trim());
        }
    }

    if (vars.length === 0) return null;

    const lines = [
        "Required environment variables for this project:",
        "",
        ...vars.map(v => `- \`${v}\``),
    ];

    return {
        title: "Environment Variables",
        content: lines.join("\n"),
        type: "schema",
        tags: ["env", "config", "infrastructure"],
        tier: 2,
        category: "config",
        source: filename,
    };
}

function scanDocker(dir: string): DiscoveredChunk | null {
    const composeFiles = ["docker-compose.yml", "docker-compose.yaml"];
    const dockerfileFiles = ["Dockerfile"];
    const found: string[] = [];

    for (const f of [...composeFiles, ...dockerfileFiles]) {
        if (existsSync(join(dir, f))) found.push(f);
    }

    if (found.length === 0) return null;

    return {
        title: "Infrastructure (Docker)",
        content: `This project uses Docker for containerization.\n\nFiles found:\n${found.map(f => `- ${f}`).join("\n")}`,
        type: "reference",
        tags: ["docker", "infrastructure", "config"],
        tier: 2,
        category: "config",
        source: found[0]!,
    };
}

function scanBuildPipeline(dir: string): DiscoveredChunk | null {
    const tools: Array<{ file: string; name: string }> = [
        { file: "turbo.json", name: "Turborepo" },
        { file: "nx.json", name: "Nx" },
    ];

    const found = tools.filter(t => existsSync(join(dir, t.file)));
    if (found.length === 0) return null;

    const toolNames = found.map(t => t.name).join(", ");
    return {
        title: "Build Pipeline",
        content: `This project uses ${toolNames} as the build pipeline orchestrator.\n\nFiles found:\n${found.map(t => `- ${t.file}`).join("\n")}`,
        type: "reference",
        tags: ["build", "tooling", "config"],
        tier: 2,
        category: "config",
        source: found[0]!.file,
    };
}

function scanCiCd(dir: string): DiscoveredChunk | null {
    const found: string[] = [];

    // GitHub Actions
    const githubWorkflowsDir = join(dir, ".github", "workflows");
    if (existsSync(githubWorkflowsDir)) {
        try {
            const entries = readdirSync(githubWorkflowsDir, { withFileTypes: true });
            for (const entry of entries) {
                if (!entry.isDirectory() && (entry.name.endsWith(".yml") || entry.name.endsWith(".yaml"))) {
                    found.push(`.github/workflows/${entry.name}`);
                }
            }
        } catch {
            // ignore
        }
    }

    // GitLab CI
    if (existsSync(join(dir, ".gitlab-ci.yml"))) {
        found.push(".gitlab-ci.yml");
    }

    if (found.length === 0) return null;

    return {
        title: "CI/CD Configuration",
        content: `This project has CI/CD pipelines configured.\n\nFiles found:\n${found.map(f => `- ${f}`).join("\n")}`,
        type: "reference",
        tags: ["ci", "cd", "config", "automation"],
        tier: 2,
        category: "config",
        source: found[0]!,
    };
}

// --- Main export ---

export function scanMetadata(dir: string): MetadataResult {
    const chunks: DiscoveredChunk[] = [];
    const tips: Tip[] = [];

    // package.json
    const pkgPath = join(dir, "package.json");
    if (existsSync(pkgPath)) {
        const pkg = readJson<PackageJson>(pkgPath);
        if (pkg) {
            const { techStack, structure } = scanPackageJson(dir, pkg);
            if (techStack) chunks.push(techStack);
            if (structure) chunks.push(structure);
        }
    }

    // tsconfig.json / jsconfig.json
    for (const filename of ["tsconfig.json", "jsconfig.json"]) {
        const tsconfigPath = join(dir, filename);
        if (existsSync(tsconfigPath)) {
            const chunk = scanTsConfig(tsconfigPath, filename);
            if (chunk) {
                chunks.push(chunk);
                break; // only one tsconfig chunk
            }
        }
    }

    // .env.example / .env.local.example — NEVER read .env
    for (const filename of [".env.example", ".env.local.example"]) {
        const envPath = join(dir, filename);
        if (existsSync(envPath)) {
            const chunk = scanEnvExample(envPath, filename);
            if (chunk) {
                chunks.push(chunk);
                break; // one env chunk
            }
        }
    }

    // Docker
    const dockerChunk = scanDocker(dir);
    if (dockerChunk) chunks.push(dockerChunk);

    // Build pipeline
    const buildChunk = scanBuildPipeline(dir);
    if (buildChunk) chunks.push(buildChunk);

    // CI/CD
    const ciChunk = scanCiCd(dir);
    if (ciChunk) chunks.push(ciChunk);

    return { chunks, tips };
}
