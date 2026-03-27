import { readFileSync } from "node:fs";
import { basename } from "node:path";

import { Command } from "commander";

import { formatBold, formatDim, formatType } from "../lib/colors";
import { output, outputError } from "../lib/output";
import { getServerUrl } from "../lib/store";

interface ExistingChunk {
    id: string;
    title: string;
    type: string;
    content: string;
}

interface Suggestion {
    type: string;
    title: string;
    reason: string;
}

export const suggestCommand = new Command("suggest")
    .description("Suggest chunks to create for a file")
    .argument("<path>", "source file path")
    .option("--codebase <name>", "scope to codebase")
    .action(async (filePath: string, opts: { codebase?: string }, cmd: Command) => {
        let content: string;
        try {
            content = readFileSync(filePath, "utf-8");
        } catch {
            outputError(`Cannot read file: ${filePath}`);
            process.exit(1);
        }

        const lines = content.split("\n");

        // Analyze complexity
        const stats = {
            lines: lines.length,
            functions: (content.match(/(?:function |const \w+ = (?:async )?\(|(?:export )?(?:async )?function)/g) || []).length,
            imports: (content.match(/^import /gm) || []).length,
            exports: (content.match(/^export /gm) || []).length,
            classes: (content.match(/^(?:export )?class /gm) || []).length,
        };

        // Check existing coverage
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

        let existingChunks: ExistingChunk[] = [];
        try {
            const params = new URLSearchParams({ path: filePath });
            if (opts.codebase) {
                params.set("codebaseId", opts.codebase);
            }
            const res = await fetch(`${serverUrl}/api/context/for-file?${params.toString()}`);
            if (res.ok) {
                existingChunks = (await res.json()) as ExistingChunk[];
            }
        } catch {
            // Continue without existing chunks if server is unreachable
        }

        // Suggest based on gaps
        const suggestions: Suggestion[] = [];
        const fileName = basename(filePath);

        if (existingChunks.length === 0) {
            suggestions.push({
                type: "reference",
                title: `${fileName} Documentation`,
                reason: "No chunks reference this file",
            });
        }

        if (stats.functions > 5 && !existingChunks.some(c => c.type === "reference")) {
            suggestions.push({
                type: "reference",
                title: `${fileName} API Reference`,
                reason: `${stats.functions} functions found but no reference chunk`,
            });
        }

        if (stats.exports > 3) {
            suggestions.push({
                type: "document",
                title: `${fileName} Architecture`,
                reason: `${stats.exports} exports suggest this is a key module`,
            });
        }

        if (stats.lines > 200) {
            suggestions.push({
                type: "note",
                title: `${fileName} Conventions`,
                reason: `Large file (${stats.lines} lines) may have implicit conventions`,
            });
        }

        if (stats.classes > 0 && !existingChunks.some(c => c.type === "schema")) {
            suggestions.push({
                type: "schema",
                title: `${fileName} Class Schema`,
                reason: `${stats.classes} class(es) found but no schema chunk`,
            });
        }

        // Output
        if (suggestions.length === 0) {
            const humanText = formatDim("No suggestions — file appears well-covered");
            output(cmd, { suggestions: [], stats }, humanText);
        } else {
            const humanLines: string[] = [
                formatBold(`${suggestions.length} suggestion(s) for ${filePath}:`),
                formatDim(`  Analysis: ${stats.lines} lines, ${stats.functions} functions, ${stats.imports} imports, ${stats.exports} exports`),
                "",
            ];
            for (const s of suggestions) {
                humanLines.push(`  ${formatType(s.type)} ${formatBold(s.title)}`);
                humanLines.push(`    ${formatDim(s.reason)}`);
            }
            output(cmd, { suggestions, stats }, humanLines.join("\n"));
        }
    });
