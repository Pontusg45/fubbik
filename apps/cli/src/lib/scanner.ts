import { existsSync, readdirSync, readFileSync, statSync } from "node:fs";
import { basename, extname, join, relative } from "node:path";

export interface ScannedChunk {
    title: string;
    content: string;
    type: string;
    tags: string[];
}

interface ScanOptions {
    dir: string;
    verbose?: boolean;
}

const IGNORE_DIRS = new Set([
    "node_modules",
    ".git",
    ".turbo",
    "dist",
    "build",
    ".next",
    ".output",
    ".cache",
    "coverage",
    ".fubbik"
]);

const DOC_FILES = ["README.md", "CLAUDE.md", "CONTRIBUTING.md", "Agents.md", "CHANGELOG.md"];

const SOURCE_EXTENSIONS = new Set([".ts", ".tsx", ".js", ".jsx"]);

export function scanProject(opts: ScanOptions): ScannedChunk[] {
    const chunks: ScannedChunk[] = [];
    const { dir } = opts;

    // 1. Root documentation files
    for (const docFile of DOC_FILES) {
        const path = join(dir, docFile);
        if (existsSync(path)) {
            const content = readFileSync(path, "utf-8");
            if (content.trim()) {
                chunks.push({
                    title: docFileName(docFile),
                    content,
                    type: "guide",
                    tags: ["documentation", "project"]
                });
            }
        }
    }

    // 2. docs/ directory — each markdown file becomes a chunk
    const docsDir = join(dir, "docs");
    if (existsSync(docsDir) && statSync(docsDir).isDirectory()) {
        for (const mdPath of findFiles(docsDir, ".md")) {
            const content = readFileSync(mdPath, "utf-8");
            const rel = relative(dir, mdPath);
            const title = extractMarkdownTitle(content) ?? rel;
            chunks.push({
                title,
                content,
                type: "guide",
                tags: ["documentation", ...pathTags(rel)]
            });
        }
    }

    // 3. Project structure overview
    const tree = buildDirectoryTree(dir, 3);
    chunks.push({
        title: "Project Structure",
        content: `Directory layout of the project:\n\n\`\`\`\n${tree}\n\`\`\``,
        type: "reference",
        tags: ["architecture", "structure"]
    });

    // 4. Package.json files — tech stack and dependencies
    const rootPkg = readPkg(join(dir, "package.json"));
    if (rootPkg) {
        chunks.push({
            title: "Root Package Configuration",
            content: packageSummary(rootPkg, "root"),
            type: "reference",
            tags: ["configuration", "dependencies"]
        });
    }

    // Sub-packages
    for (const pkgPath of findFiles(dir, "package.json", 3)) {
        const rel = relative(dir, pkgPath);
        if (rel === "package.json") continue; // already handled
        const pkg = readPkg(pkgPath);
        if (pkg) {
            const name = pkg.name ?? rel.replace("/package.json", "");
            chunks.push({
                title: `Package: ${name}`,
                content: packageSummary(pkg, rel.replace("/package.json", "")),
                type: "reference",
                tags: ["configuration", "package", ...pathTags(rel)]
            });
        }
    }

    // 5. Config files
    for (const cfgFile of ["tsconfig.json", "turbo.json", "docker-compose.yml", "Dockerfile"]) {
        const path = join(dir, cfgFile);
        if (existsSync(path)) {
            const content = readFileSync(path, "utf-8");
            chunks.push({
                title: `Config: ${cfgFile}`,
                content: `\`\`\`\n${content}\n\`\`\``,
                type: "reference",
                tags: ["configuration", cfgFile.split(".")[0]!]
            });
        }
    }

    // 6. Source code — scan for key patterns (exports, route definitions, schemas)
    const sourceChunks = scanSourceCode(dir);
    chunks.push(...sourceChunks);

    return chunks;
}

function scanSourceCode(dir: string): ScannedChunk[] {
    const chunks: ScannedChunk[] = [];

    // Find index/entry files that define public APIs
    const entryPatterns = ["index.ts", "index.tsx", "routes.ts", "schema.ts", "service.ts"];
    const sourceFiles = findSourceFiles(dir, 5);

    // Group by directory to create per-module summaries
    const modules = new Map<string, { path: string; content: string; name: string }[]>();

    for (const filePath of sourceFiles) {
        const rel = relative(dir, filePath);
        const fileName = basename(filePath);

        if (!entryPatterns.some(p => fileName === p || fileName.endsWith(`.${p}`))) continue;

        const content = readFileSync(filePath, "utf-8");
        if (content.length > 20000) continue; // skip huge files

        const dirKey = relative(dir, join(filePath, ".."));
        if (!modules.has(dirKey)) modules.set(dirKey, []);
        modules.get(dirKey)!.push({ path: rel, content, name: fileName });
    }

    for (const [moduleDir, files] of modules) {
        // Extract exports, types, function signatures
        const summaryParts: string[] = [`Module: \`${moduleDir}\`\n`];

        for (const file of files) {
            const exports = extractExports(file.content);
            const routes = extractRoutes(file.content);

            if (exports.length > 0 || routes.length > 0) {
                summaryParts.push(`### ${file.path}\n`);
                if (exports.length > 0) {
                    summaryParts.push("**Exports:**");
                    for (const exp of exports) {
                        summaryParts.push(`- \`${exp}\``);
                    }
                    summaryParts.push("");
                }
                if (routes.length > 0) {
                    summaryParts.push("**Routes:**");
                    for (const route of routes) {
                        summaryParts.push(`- \`${route}\``);
                    }
                    summaryParts.push("");
                }
            }
        }

        if (summaryParts.length > 1) {
            chunks.push({
                title: `Module: ${moduleDir}`,
                content: summaryParts.join("\n"),
                type: "reference",
                tags: ["code", "module", ...pathTags(moduleDir)]
            });
        }
    }

    // Scan for Drizzle schema files specifically
    for (const filePath of sourceFiles) {
        const rel = relative(dir, filePath);
        if (!rel.includes("schema") || basename(filePath) === "index.ts") continue;

        const content = readFileSync(filePath, "utf-8");
        const tables = extractDrizzleTables(content);
        if (tables.length > 0) {
            chunks.push({
                title: `Database Schema: ${basename(filePath, extname(filePath))}`,
                content: `Schema defined in \`${rel}\`:\n\n\`\`\`typescript\n${content}\n\`\`\``,
                type: "reference",
                tags: ["database", "schema", "drizzle"]
            });
        }
    }

    return chunks;
}

// --- Helpers ---

function docFileName(file: string): string {
    const map: Record<string, string> = {
        "README.md": "Project README",
        "CLAUDE.md": "AI Assistant Instructions (CLAUDE.md)",
        "CONTRIBUTING.md": "Contributing Guide",
        "Agents.md": "AI Agents Documentation",
        "CHANGELOG.md": "Changelog"
    };
    return map[file] ?? file;
}

function extractMarkdownTitle(content: string): string | null {
    const match = content.match(/^#\s+(.+)$/m);
    return match?.[1]?.trim() ?? null;
}

function pathTags(relPath: string): string[] {
    const parts = relPath.split("/").filter(Boolean);
    // Take meaningful path segments as tags
    return parts
        .filter(p => !["src", "lib", "index.ts", "package.json"].includes(p))
        .slice(0, 3);
}

function buildDirectoryTree(dir: string, maxDepth: number, prefix = "", depth = 0): string {
    if (depth >= maxDepth) return "";
    const entries = readdirSync(dir, { withFileTypes: true })
        .filter(e => !e.name.startsWith(".") && !IGNORE_DIRS.has(e.name))
        .sort((a, b) => {
            if (a.isDirectory() && !b.isDirectory()) return -1;
            if (!a.isDirectory() && b.isDirectory()) return 1;
            return a.name.localeCompare(b.name);
        });

    const lines: string[] = [];
    for (let i = 0; i < entries.length; i++) {
        const entry = entries[i]!;
        const isLast = i === entries.length - 1;
        const connector = isLast ? "└── " : "├── ";
        const childPrefix = isLast ? "    " : "│   ";

        if (entry.isDirectory()) {
            lines.push(`${prefix}${connector}${entry.name}/`);
            const subtree = buildDirectoryTree(join(dir, entry.name), maxDepth, prefix + childPrefix, depth + 1);
            if (subtree) lines.push(subtree);
        } else {
            lines.push(`${prefix}${connector}${entry.name}`);
        }
    }
    return lines.join("\n");
}

function findFiles(dir: string, ext: string, maxDepth = 5, depth = 0): string[] {
    if (depth >= maxDepth) return [];
    const results: string[] = [];
    try {
        const entries = readdirSync(dir, { withFileTypes: true });
        for (const entry of entries) {
            if (entry.name.startsWith(".") || IGNORE_DIRS.has(entry.name)) continue;
            const full = join(dir, entry.name);
            if (entry.isDirectory()) {
                results.push(...findFiles(full, ext, maxDepth, depth + 1));
            } else if (entry.name.endsWith(ext)) {
                results.push(full);
            }
        }
    } catch {
        // permission errors etc
    }
    return results;
}

function findSourceFiles(dir: string, maxDepth: number, depth = 0): string[] {
    if (depth >= maxDepth) return [];
    const results: string[] = [];
    try {
        const entries = readdirSync(dir, { withFileTypes: true });
        for (const entry of entries) {
            if (entry.name.startsWith(".") || IGNORE_DIRS.has(entry.name)) continue;
            const full = join(dir, entry.name);
            if (entry.isDirectory()) {
                results.push(...findSourceFiles(full, maxDepth, depth + 1));
            } else if (SOURCE_EXTENSIONS.has(extname(entry.name))) {
                results.push(full);
            }
        }
    } catch {
        // permission errors etc
    }
    return results;
}

interface PkgJson {
    name?: string;
    description?: string;
    scripts?: Record<string, string>;
    dependencies?: Record<string, string>;
    devDependencies?: Record<string, string>;
    bin?: Record<string, string> | string;
}

function readPkg(path: string): PkgJson | null {
    try {
        return JSON.parse(readFileSync(path, "utf-8")) as PkgJson;
    } catch {
        return null;
    }
}

function packageSummary(pkg: PkgJson, location: string): string {
    const lines: string[] = [];
    if (pkg.name) lines.push(`**Name:** ${pkg.name}`);
    if (pkg.description) lines.push(`**Description:** ${pkg.description}`);
    lines.push(`**Location:** \`${location}\``);

    if (pkg.scripts && Object.keys(pkg.scripts).length > 0) {
        lines.push("\n**Scripts:**");
        for (const [name, cmd] of Object.entries(pkg.scripts)) {
            lines.push(`- \`${name}\`: \`${cmd}\``);
        }
    }

    if (pkg.dependencies && Object.keys(pkg.dependencies).length > 0) {
        lines.push("\n**Dependencies:**");
        for (const [name, version] of Object.entries(pkg.dependencies)) {
            lines.push(`- ${name}: ${version}`);
        }
    }

    if (pkg.devDependencies && Object.keys(pkg.devDependencies).length > 0) {
        lines.push("\n**Dev Dependencies:**");
        for (const [name, version] of Object.entries(pkg.devDependencies)) {
            lines.push(`- ${name}: ${version}`);
        }
    }

    return lines.join("\n");
}

function extractExports(content: string): string[] {
    const exports: string[] = [];
    const re = /^export\s+(?:async\s+)?(?:function|const|class|type|interface|enum)\s+(\w+)/gm;
    let match;
    while ((match = re.exec(content)) !== null) {
        exports.push(match[1]!);
    }
    // Also catch re-exports
    const reExport = /^export\s+\*\s+from\s+"([^"]+)"/gm;
    while ((match = reExport.exec(content)) !== null) {
        exports.push(`* from "${match[1]}"`);
    }
    return exports;
}

function extractRoutes(content: string): string[] {
    const routes: string[] = [];
    const re = /\.(get|post|put|patch|delete)\(\s*["'`]([^"'`]+)["'`]/g;
    let match;
    while ((match = re.exec(content)) !== null) {
        routes.push(`${match[1]!.toUpperCase()} ${match[2]}`);
    }
    return routes;
}

function extractDrizzleTables(content: string): string[] {
    const tables: string[] = [];
    const re = /pgTable\(\s*["'`](\w+)["'`]/g;
    let match;
    while ((match = re.exec(content)) !== null) {
        tables.push(match[1]!);
    }
    return tables;
}
