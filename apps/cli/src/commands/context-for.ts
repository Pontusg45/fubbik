import { Command } from "commander";

import { output, outputError, outputQuiet } from "../lib/output";
import { getServerUrl } from "../lib/store";

interface ContextChunk {
    id: string;
    title: string;
    type: string;
    content: string;
    summary: string | null;
    matchReason: "file-ref" | "applies-to";
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
    .action(async (filePath: string, opts: { codebase?: string; format: string }, cmd: Command) => {
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
