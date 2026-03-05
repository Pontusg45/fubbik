import { Command } from "commander";

import { deleteChunk, getChunk } from "../store";

export const removeCommand = new Command("remove")
  .description("Remove a chunk by ID")
  .argument("<id>", "chunk ID")
  .action((id: string) => {
    const chunk = getChunk(id);
    if (!chunk) {
      console.error(`✗ Chunk "${id}" not found.`);
      process.exit(1);
    }

    deleteChunk(id);
    console.log(`✓ Removed chunk ${id} (${chunk.title})`);
  });
