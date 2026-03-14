import { Command } from "commander";

import { output, outputQuiet } from "../lib/output";
import { getServerUrl } from "../lib/store";

export const contextCommand = new Command("context")
    .description("Export token-aware context for AI consumption")
    .option("--max-tokens <tokens>", "token budget", "4000")
    .option("--codebase <id>", "scope to a specific codebase ID")
    .option("--format <format>", "output format: markdown or json", "markdown")
    .action(async (opts: { maxTokens: string; codebase?: string; format: string }, cmd: Command) => {
        const serverUrl = getServerUrl();
        if (!serverUrl) {
            console.error('No server URL configured. Run "fubbik init" first.');
            process.exit(1);
        }

        const params = new URLSearchParams();
        params.set("maxTokens", opts.maxTokens);
        params.set("format", opts.format);
        if (opts.codebase) {
            params.set("codebaseId", opts.codebase);
        }

        const res = await fetch(`${serverUrl}/api/chunks/export/context?${params.toString()}`);
        if (!res.ok) {
            const text = await res.text();
            console.error(`Failed to export context: ${res.status} ${text}`);
            process.exit(1);
        }

        const data = (await res.json()) as Record<string, unknown>;

        if (opts.format === "json") {
            outputQuiet(cmd, JSON.stringify(data));
            output(cmd, data, JSON.stringify(data, null, 2));
        } else {
            const content = (data as { content?: string }).content ?? "";
            outputQuiet(cmd, content);
            output(cmd, data, content);
        }
    });
