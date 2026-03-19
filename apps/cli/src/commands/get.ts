import { Command } from "commander";

import { formatDim, formatTag, formatTitle, formatType } from "../lib/colors";
import { output, outputError, outputQuiet } from "../lib/output";
import { getChunk } from "../lib/store";

export const getCommand = new Command("get")
    .description("Get a chunk by ID")
    .argument("<id>", "chunk ID")
    .action((id: string, _opts: unknown, cmd: Command) => {
        const chunk = getChunk(id);
        if (!chunk) {
            outputError(`Chunk "${id}" not found.`);
            process.exit(1);
        }

        outputQuiet(cmd, chunk.id);
        const lines = [
            formatTitle(chunk.title),
            `  ID: ${formatDim(chunk.id)}`,
            `  Type: ${formatType(chunk.type)}`,
            ...(chunk.tags.length > 0 ? [`  Tags: ${chunk.tags.map(formatTag).join(", ")}`] : []),
            `  Created: ${formatDim(chunk.createdAt)}`,
            `  Updated: ${formatDim(chunk.updatedAt)}`,
            ...(chunk.content ? ["", chunk.content] : [])
        ];
        output(cmd, chunk, lines.join("\n"));
    });
