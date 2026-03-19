import { Command } from "commander";

import { formatSuccess } from "../lib/colors";
import { output, outputError, outputQuiet } from "../lib/output";
import { confirm } from "../lib/prompt";
import { deleteChunk, getChunk } from "../lib/store";

export const removeCommand = new Command("remove")
    .description("Remove a chunk by ID")
    .argument("<id>", "chunk ID")
    .option("-y, --yes", "skip confirmation prompt")
    .action(async (id: string, opts: { yes?: boolean }, cmd: Command) => {
        const chunk = getChunk(id);
        if (!chunk) {
            outputError(`Chunk "${id}" not found.`);
            process.exit(1);
        }

        if (!opts.yes) {
            const ok = await confirm(`Delete "${chunk.title}" (${id})? This cannot be undone.`);
            if (!ok) {
                console.error("Aborted.");
                return;
            }
        }

        deleteChunk(id);
        outputQuiet(cmd, id);
        output(cmd, { id, title: chunk.title }, formatSuccess(`Removed chunk ${id} (${chunk.title})`));
    });
