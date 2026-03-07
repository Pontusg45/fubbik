import { Command } from "commander";

import { getChunk } from "../lib/store";
import { outputError } from "../lib/output";

export const catCommand = new Command("cat")
    .description("Output raw content of a chunk (no metadata)")
    .argument("<id>", "chunk ID")
    .action((id: string) => {
        const chunk = getChunk(id);
        if (!chunk) {
            outputError(`✗ Chunk "${id}" not found.`);
            process.exit(1);
        }
        process.stdout.write(chunk.content);
    });
