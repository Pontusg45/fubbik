import { Command } from "commander";

import { getChunk } from "../lib/store";
import { output, outputError, outputQuiet } from "../lib/output";

export const getCommand = new Command("get")
    .description("Get a chunk by ID")
    .argument("<id>", "chunk ID")
    .action((id: string, _opts: unknown, cmd: Command) => {
        const chunk = getChunk(id);
        if (!chunk) {
            outputError(`✗ Chunk "${id}" not found.`);
            process.exit(1);
        }

        outputQuiet(cmd, chunk.id);
        const lines = [
            chunk.title,
            `  ID: ${chunk.id}`,
            `  Type: ${chunk.type}`,
            ...(chunk.tags.length > 0 ? [`  Tags: ${chunk.tags.join(", ")}`] : []),
            `  Created: ${chunk.createdAt}`,
            `  Updated: ${chunk.updatedAt}`,
            ...(chunk.content ? ["", chunk.content] : [])
        ];
        output(cmd, chunk, lines.join("\n"));
    });
