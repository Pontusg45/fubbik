import { Command } from "commander";

import { updateChunk } from "../lib/store";
import { output, outputError, outputQuiet } from "../lib/output";

export const updateCommand = new Command("update")
    .description("Update a chunk by ID")
    .argument("<id>", "chunk ID")
    .option("-t, --title <title>", "new title")
    .option("-c, --content <content>", "new content")
    .option("--type <type>", "new type")
    .option("--tags <tags>", "new comma-separated tags")
    .option("--content-file <path>", "read content from file (use - for stdin)")
    .action(async (id: string, opts: { title?: string; content?: string; type?: string; tags?: string; contentFile?: string }, cmd: Command) => {
        const updates: Record<string, unknown> = {};
        if (opts.title !== undefined) updates.title = opts.title;
        if (opts.type !== undefined) updates.type = opts.type;
        if (opts.tags !== undefined) updates.tags = opts.tags.split(",").map(t => t.trim());

        if (opts.contentFile) {
            if (opts.contentFile === "-") {
                updates.content = await Bun.stdin.text();
            } else {
                const { readFileSync } = await import("node:fs");
                updates.content = readFileSync(opts.contentFile, "utf-8");
            }
        } else if (opts.content !== undefined) {
            updates.content = opts.content;
        }

        if (Object.keys(updates).length === 0) {
            outputError("✗ No updates provided. Use --title, --content, --type, --tags, or --content-file.");
            process.exit(1);
        }

        const chunk = updateChunk(id, updates);
        if (!chunk) {
            outputError(`✗ Chunk "${id}" not found.`);
            process.exit(1);
        }

        outputQuiet(cmd, chunk.id);
        output(cmd, chunk, [
            `✓ Updated chunk ${chunk.id}`,
            `  Title: ${chunk.title}`,
            `  Type: ${chunk.type}`,
            ...(chunk.tags.length > 0 ? [`  Tags: ${chunk.tags.join(", ")}`] : [])
        ].join("\n"));
    });
