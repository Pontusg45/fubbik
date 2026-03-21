import { existsSync, readFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";

import { Command } from "commander";

import { output, outputError, outputQuiet } from "../lib/output";
import { getServerUrl } from "../lib/store";

interface ContextChunk {
    id: string;
    title: string;
    type: string;
    content: string;
    summary: string | null;
    matchReason: "file-ref" | "applies-to" | "dependency";
}

const DEP_FILES = ["package.json", "go.mod"] as const;

/**
 * Walk up from a file path to find the nearest dependency manifest.
 * Returns { filename, content } or null.
 */
function findNearestDepFile(filePath: string): { filename: string; content: string } | null {
    let dir = dirname(resolve(filePath));
    const root = "/";
    while (true) {
        for (const name of DEP_FILES) {
            const candidate = join(dir, name);
            if (existsSync(candidate)) {
                return { filename: name, content: readFileSync(candidate, "utf-8") };
            }
        }
        if (dir === root) break;
        const parent = dirname(dir);
        if (parent === dir) break;
        dir = parent;
    }
    return null;
}

function parseDepsFromFile(filename: string, content: string): string[] {
    if (filename === "package.json") {
        try {
            const pkg = JSON.parse(content);
            return Object.keys(pkg.dependencies ?? {});
        } catch {
            return [];
        }
    }
    if (filename === "go.mod") {
        const deps: string[] = [];
        const requireBlock = content.match(/require\s*\(([\s\S]*?)\)/);
        if (requireBlock) {
            for (const line of requireBlock[1]!.split("\n")) {
                const match = line.trim().match(/^(\S+)\s+/);
                if (match) deps.push(match[1]!);
            }
        }
        return deps;
    }
    return [];
}

function formatMarkdown(chunks: ContextChunk[], filePath: string): string {
    if (chunks.length === 0) {
        return `# Context for ${filePath}\n\nNo relevant chunks found.`;
    }

    const lines: string[] = [`# Context for ${filePath}`, ""];

    for (const chunk of chunks) {
        lines.push(`## ${chunk.title}`);
        lines.push("");
        lines.push(`**Type:** ${chunk.type} | **Match:** ${chunk.matchReason}`);
        lines.push("");
        if (chunk.summary) {
            lines.push(`> ${chunk.summary}`);
            lines.push("");
        }
        lines.push(chunk.content);
        lines.push("");
        lines.push("---");
        lines.push("");
    }

    return lines.join("\n");
}

export const contextForCommand = new Command("context-for")
    .description("Generate focused context for a specific file")
    .argument("<path>", "file path to get context for")
    .option("--codebase <name>", "scope to a specific codebase")
    .option("--format <format>", "output format: markdown or json", "markdown")
    .option("--include-deps", "include chunks from dependency codebases")
    .action(async (filePath: string, opts: { codebase?: string; format: string; includeDeps?: boolean }, cmd: Command) => {
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

        const params = new URLSearchParams();
        params.set("path", filePath);
        if (opts.codebase) {
            params.set("codebaseId", opts.codebase);
        }
        if (opts.includeDeps) {
            const depFile = findNearestDepFile(filePath);
            if (depFile) {
                const deps = parseDepsFromFile(depFile.filename, depFile.content);
                if (deps.length > 0) {
                    params.set("deps", deps.join(","));
                }
            }
        }

        try {
            const res = await fetch(`${serverUrl}/api/context/for-file?${params.toString()}`);
            if (!res.ok) {
                const text = await res.text();
                outputError(`Failed to fetch context: ${res.status} ${text}`);
                process.exit(1);
            }

            const chunks = (await res.json()) as ContextChunk[];

            if (opts.format === "json") {
                outputQuiet(cmd, JSON.stringify(chunks));
                output(cmd, chunks, JSON.stringify(chunks, null, 2));
            } else {
                const markdown = formatMarkdown(chunks, filePath);
                outputQuiet(cmd, markdown);
                output(cmd, chunks, markdown);
            }
        } catch (err) {
            outputError(`Could not connect to ${serverUrl}`);
            if (err instanceof Error) outputError(`  ${err.message}`);
            process.exit(1);
        }
    });
