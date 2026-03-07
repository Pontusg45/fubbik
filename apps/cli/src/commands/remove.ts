import { Command } from "commander";

import { deleteChunk, getChunk } from "../lib/store";
import { output, outputError, outputQuiet } from "../lib/output";

export const removeCommand = new Command("remove")
    .description("Remove a chunk by ID")
    .argument("<id>", "chunk ID")
    .action((id: string, _opts: unknown, cmd: Command) => {
        const chunk = getChunk(id);
        if (!chunk) {
            outputError(`✗ Chunk "${id}" not found.`);
            process.exit(1);
        }

        deleteChunk(id);
        outputQuiet(cmd, id);
        output(cmd, { id, title: chunk.title }, `✓ Removed chunk ${id} (${chunk.title})`);
    });
