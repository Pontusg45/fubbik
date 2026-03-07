import { Command } from "commander";

import { addChunk } from "../lib/store";
import { output, outputQuiet } from "../lib/output";

export const addCommand = new Command("add")
    .description("Add a new chunk to the knowledge base")
    .requiredOption("-t, --title <title>", "chunk title")
    .option("-c, --content <content>", "chunk content", "")
    .option("--type <type>", "chunk type", "note")
    .option("--tags <tags>", "comma-separated tags", "")
    .option("--content-file <path>", "read content from file (use - for stdin)")
    .action(async (opts: { title: string; content: string; type: string; tags: string; contentFile?: string }, cmd: Command) => {
        let content = opts.content;
        if (opts.contentFile) {
            if (opts.contentFile === "-") {
                content = await Bun.stdin.text();
            } else {
                const { readFileSync } = await import("node:fs");
                content = readFileSync(opts.contentFile, "utf-8");
            }
        }

        const tags = opts.tags ? opts.tags.split(",").map(t => t.trim()) : [];
        const chunk = addChunk({ title: opts.title, content, type: opts.type, tags });

        outputQuiet(cmd, chunk.id);
        output(cmd, chunk, [
            `✓ Created chunk ${chunk.id}`,
            `  Title: ${chunk.title}`,
            `  Type: ${chunk.type}`,
            ...(tags.length > 0 ? [`  Tags: ${tags.join(", ")}`] : [])
        ].join("\n"));
    });
