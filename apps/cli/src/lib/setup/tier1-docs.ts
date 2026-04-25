import { existsSync, readdirSync, readFileSync, statSync } from "node:fs";
import { basename, join, relative } from "node:path";

import { DEFAULT_THRESHOLDS } from "@fubbik/api/chunk-size";

import type { DiscoveredChunk } from "./types";

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
    ".fubbik",
]);

const DOC_FILES = ["README.md", "CLAUDE.md", "CONTRIBUTING.md", "Agents.md", "CHANGELOG.md"];

export function scanDocs(dir: string): DiscoveredChunk[] {
    const chunks: DiscoveredChunk[] = [];

    // 1. Root documentation files
    for (const docFile of DOC_FILES) {
        const path = join(dir, docFile);
        if (existsSync(path)) {
            const content = readFileSync(path, "utf-8");
            if (content.trim()) {
                addChunkWithAutoSplit(chunks, {
                    title: docFileName(docFile),
                    content,
                    type: "guide",
                    tags: ["documentation", "project"],
                    tier: 1,
                    category: "documents",
                    source: docFile,
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
            const title = extractMarkdownTitle(content) ?? basename(mdPath, ".md");
            addChunkWithAutoSplit(chunks, {
                title,
                content,
                type: "guide",
                tags: ["documentation", "docs", ...pathTags(rel)],
                tier: 1,
                category: "documents",
                source: rel,
            });
        }
    }

    // 3. Markdown files throughout the project (max depth 5, skip ignored dirs)
    for (const mdPath of findFiles(dir, ".md")) {
        const rel = relative(dir, mdPath);
        // Skip root docs already handled
        if (DOC_FILES.includes(basename(mdPath)) && rel === basename(mdPath)) continue;
        // Skip docs/ — already handled above
        if (rel.startsWith("docs/")) continue;

        const content = readFileSync(mdPath, "utf-8");
        if (!content.trim()) continue;
        const title = extractMarkdownTitle(content) ?? basename(mdPath, ".md");
        addChunkWithAutoSplit(chunks, {
            title,
            content,
            type: "guide",
            tags: ["documentation", ...pathTags(rel)],
            tier: 1,
            category: "documents",
            source: rel,
        });
    }

    return chunks;
}

// --- Auto-split ---

function exceedsWarning(content: string): boolean {
    return (
        content.split("\n").length > DEFAULT_THRESHOLDS.warningLines ||
        content.length > DEFAULT_THRESHOLDS.warningChars
    );
}

function splitByHeadings(content: string): { title: string; content: string }[] | null {
    const lines = content.split("\n");
    const sections: { title: string; content: string }[] = [];
    let currentTitle = "";
    let currentLines: string[] = [];

    for (const line of lines) {
        const match = line.match(/^(#{1,3})\s+(.+)$/);
        if (match) {
            const prev = currentLines.join("\n").trim();
            if (prev) sections.push({ title: currentTitle, content: prev });
            currentTitle = match[2]!;
            currentLines = [];
        } else {
            currentLines.push(line);
        }
    }
    const last = currentLines.join("\n").trim();
    if (last) sections.push({ title: currentTitle, content: last });

    return sections.length >= 2 ? sections : null;
}

function addChunkWithAutoSplit(chunks: DiscoveredChunk[], chunk: DiscoveredChunk): void {
    if (!exceedsWarning(chunk.content)) {
        chunks.push(chunk);
        return;
    }

    const sections = splitByHeadings(chunk.content);
    if (!sections) {
        chunks.push(chunk);
        return;
    }

    // Create index chunk with listing of sections
    const indexContent = sections.map(s => `- ${s.title || "(intro)"}`).join("\n");
    chunks.push({
        ...chunk,
        content: `Sections:\n\n${indexContent}`,
    });

    // Create sub-chunks
    for (const section of sections) {
        const sectionTitle = section.title || `${chunk.title} (intro)`;
        chunks.push({
            title: sectionTitle,
            content: section.content,
            type: chunk.type,
            tags: chunk.tags,
            tier: chunk.tier,
            category: chunk.category,
            source: chunk.source,
        });
    }
}

// --- Helpers ---

function docFileName(file: string): string {
    const map: Record<string, string> = {
        "README.md": "Project README",
        "CLAUDE.md": "AI Assistant Instructions (CLAUDE.md)",
        "CONTRIBUTING.md": "Contributing Guide",
        "Agents.md": "AI Agents Documentation",
        "CHANGELOG.md": "Changelog",
    };
    return map[file] ?? file;
}

function extractMarkdownTitle(content: string): string | null {
    const match = content.match(/^#\s+(.+)$/m);
    return match?.[1]?.trim() ?? null;
}

function pathTags(relPath: string): string[] {
    const parts = relPath.split("/").filter(Boolean);
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
