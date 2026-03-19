import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";

import { Command } from "commander";

import { formatSuccess } from "../lib/colors";
import { output, outputError } from "../lib/output";
import { readStore } from "../lib/store";

export const exportCommand = new Command("export")
    .description("Export the knowledge base")
    .option("--format <format>", "output format: json or md", "json")
    .option("--out <dir>", "output directory for md format", "export")
    .action((opts: { format: string; out: string }, cmd: Command) => {
        const store = readStore();

        if (opts.format === "json") {
            output(cmd, store.chunks, JSON.stringify(store.chunks, null, 2));
        } else if (opts.format === "md") {
            mkdirSync(opts.out, { recursive: true });
            for (const chunk of store.chunks) {
                const frontmatter = [
                    "---",
                    `id: ${chunk.id}`,
                    `title: "${chunk.title.replace(/"/g, '\\"')}"`,
                    `type: ${chunk.type}`,
                    `tags: [${chunk.tags.map(t => `"${t}"`).join(", ")}]`,
                    `createdAt: ${chunk.createdAt}`,
                    `updatedAt: ${chunk.updatedAt}`,
                    "---",
                    "",
                    `# ${chunk.title}`,
                    "",
                    chunk.content
                ].join("\n");

                const filename = `${chunk.id}.md`;
                writeFileSync(join(opts.out, filename), frontmatter);
            }
            output(cmd, { count: store.chunks.length, dir: opts.out }, formatSuccess(`Exported ${store.chunks.length} chunk(s) to ${opts.out}/`));
        } else {
            outputError(`Unknown format "${opts.format}". Use json or md.`);
            process.exit(1);
        }
    });
