import { readdirSync, statSync, writeFileSync } from "node:fs";
import { join, relative } from "node:path";

import { Command } from "commander";

import { output, outputError, outputQuiet } from "../lib/output";
import { getServerUrl } from "../lib/store";

interface ContextChunk {
    id: string;
    title: string;
    type: string;
    content: string;
    summary: string | null;
    tags?: Array<{ name: string }>;
    matchReason: "file-ref" | "applies-to";
}

function collectFiles(dir: string, maxFiles: number): string[] {
    const files: string[] = [];

    function walk(currentDir: string) {
        if (files.length >= maxFiles) return;

        let entries: string[];
        try {
            entries = readdirSync(currentDir);
        } catch {
            return;
        }

        for (const entry of entries) {
            if (files.length >= maxFiles) break;
            if (entry.startsWith(".") || entry === "node_modules") continue;

            const fullPath = join(currentDir, entry);
            try {
                const stat = statSync(fullPath);
                if (stat.isDirectory()) {
                    walk(fullPath);
                } else if (stat.isFile()) {
                    files.push(fullPath);
                }
            } catch {
                // skip inaccessible files
            }
        }
    }

    walk(dir);
    return files;
}

function hasTag(chunk: ContextChunk, tagName: string): boolean {
    return chunk.tags?.some(t => t.name.toLowerCase() === tagName.toLowerCase()) ?? false;
}

function formatDirectoryMarkdown(chunks: ContextChunk[], directory: string): string {
    if (chunks.length === 0) {
        return `# Knowledge Context for ${directory}\n\nNo relevant chunks found.`;
    }

    const conventions: ContextChunk[] = [];
    const architecture: ContextChunk[] = [];
    const references: ContextChunk[] = [];

    for (const chunk of chunks) {
        if (chunk.type === "note" || hasTag(chunk, "convention")) {
            conventions.push(chunk);
        } else if (chunk.type === "document" || hasTag(chunk, "architecture")) {
            architecture.push(chunk);
        } else {
            references.push(chunk);
        }
    }

    const lines: string[] = [`# Knowledge Context for ${directory}`, ""];

    if (conventions.length > 0) {
        lines.push("## Conventions", "");
        for (const chunk of conventions) {
            lines.push(`### ${chunk.title}`, "");
            if (chunk.summary) {
                lines.push(`> ${chunk.summary}`, "");
            }
            lines.push(chunk.content, "");
            lines.push("---", "");
        }
    }

    if (architecture.length > 0) {
        lines.push("## Architecture", "");
        for (const chunk of architecture) {
            lines.push(`### ${chunk.title}`, "");
            if (chunk.summary) {
                lines.push(`> ${chunk.summary}`, "");
            }
            lines.push(chunk.content, "");
            lines.push("---", "");
        }
    }

    if (references.length > 0) {
        lines.push("## References", "");
        for (const chunk of references) {
            lines.push(`### ${chunk.title}`, "");
            if (chunk.summary) {
                lines.push(`> ${chunk.summary}`, "");
            }
            lines.push(chunk.content, "");
            lines.push("---", "");
        }
    }

    return lines.join("\n");
}

export const contextDirCommand = new Command("dir")
    .description("Generate CLAUDE.md-style context for all files in a directory")
    .argument("<directory>", "directory to generate context for")
    .option("--codebase <name>", "scope to a specific codebase")
    .option("--output <file>", "write output to a file instead of stdout")
    .action(async (directory: string, opts: { codebase?: string; output?: string }, cmd: Command) => {
        let serverUrl: string | undefined;
        try {
            serverUrl = getServerUrl();
        } catch {
            outputError('No server URL configured. Run "fubbik init" first.');
            process.exit(1);
        }

        if (!serverUrl) {
            outputError('No server URL configured. Run "fubbik init" first.');
            process.exit(1);
        }

        const resolvedDir = join(process.cwd(), directory);
        const files = collectFiles(resolvedDir, 200);

        if (files.length === 0) {
            outputError(`No files found in ${directory}`);
            process.exit(1);
        }

        const relativePaths = files.map(f => relative(process.cwd(), f));
        const allChunks: ContextChunk[] = [];

        // Batch fetch — single HTTP call for all files
        const params = new URLSearchParams();
        params.set("paths", relativePaths.join(","));
        params.set("format", "structured-json");
        if (opts.codebase) params.set("codebaseId", opts.codebase);

        try {
            const res = await fetch(`${serverUrl}/api/context/for-files?${params.toString()}`);
            if (res.ok) {
                const data = (await res.json()) as {
                    sections?: Array<{
                        title: string;
                        chunks: Array<{
                            id: string;
                            title: string;
                            type: string;
                            content: string;
                            summary?: string | null;
                            rationale?: string | null;
                        }>;
                    }>;
                };
                if (data.sections) {
                    const seenIds = new Set<string>();
                    for (const section of data.sections) {
                        for (const chunk of section.chunks) {
                            if (!seenIds.has(chunk.id)) {
                                seenIds.add(chunk.id);
                                allChunks.push({
                                    id: chunk.id,
                                    title: chunk.title,
                                    type: chunk.type,
                                    content: chunk.content,
                                    summary: chunk.summary ?? null,
                                    matchReason: "file-ref",
                                });
                            }
                        }
                    }
                }
            }
        } catch {
            outputError("Failed to fetch context for directory");
            process.exit(1);
        }

        const markdown = formatDirectoryMarkdown(allChunks, directory);

        if (opts.output) {
            try {
                writeFileSync(opts.output, markdown, "utf-8");
                output(cmd, allChunks, `Wrote context for ${allChunks.length} chunks to ${opts.output}`);
                outputQuiet(cmd, opts.output);
            } catch (err) {
                outputError(`Failed to write to ${opts.output}`);
                if (err instanceof Error) outputError(`  ${err.message}`);
                process.exit(1);
            }
        } else {
            outputQuiet(cmd, markdown);
            output(cmd, allChunks, markdown);
        }
    });
