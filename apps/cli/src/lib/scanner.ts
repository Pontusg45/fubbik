import { existsSync, readdirSync, readFileSync, statSync } from "node:fs";
import { basename, join, relative } from "node:path";

export interface ScannedChunk {
    title: string;
    content: string;
    type: string;
    tags: string[];
    /** Directory relative to project root, used to group related chunks */
    folder: string;
    /** Whether this chunk is the index/README for its folder */
    isIndex?: boolean;
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
                    tags: ["documentation", "project"],
                    folder: ".",
                    isIndex: docFile === "README.md"
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
            const folder = relative(dir, join(mdPath, ".."));
            const title = extractMarkdownTitle(content) ?? rel;
            chunks.push({
                title,
                content,
                type: "guide",
                tags: ["documentation", ...pathTags(rel)],
                folder,
                isIndex: isIndexFile(basename(mdPath))
            });
        }
    }

    // 3. Markdown files throughout the project
    for (const mdPath of findFiles(dir, ".md")) {
        const rel = relative(dir, mdPath);
        // Skip root docs already handled and node_modules
        if (DOC_FILES.includes(basename(mdPath)) && rel === basename(mdPath)) continue;
        if (rel.startsWith("docs/")) continue; // already handled above

        const content = readFileSync(mdPath, "utf-8");
        if (!content.trim()) continue;
        const title = extractMarkdownTitle(content) ?? rel;
        const folder = relative(dir, join(mdPath, ".."));
        chunks.push({
            title,
            content,
            type: "guide",
            tags: ["documentation", ...pathTags(rel)],
            folder,
            isIndex: isIndexFile(basename(mdPath))
        });
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

function isIndexFile(fileName: string): boolean {
    const lower = fileName.toLowerCase();
    return lower === "readme.md" || lower === "index.md";
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

