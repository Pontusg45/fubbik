import { Command } from "commander";

import { getChunk } from "../lib/store";

export const getCommand = new Command("get")
  .description("Get a chunk by ID")
  .argument("<id>", "chunk ID")
  .option("--json", "output as JSON")
  .action((id: string, opts: { json?: boolean }) => {
    const chunk = getChunk(id);
    if (!chunk) {
      console.error(`✗ Chunk "${id}" not found.`);
      process.exit(1);
    }

    if (opts.json) {
      console.log(JSON.stringify(chunk, null, 2));
      return;
    }

    console.log(`${chunk.title}`);
    console.log(`  ID: ${chunk.id}`);
    console.log(`  Type: ${chunk.type}`);
    if (chunk.tags.length > 0) {
      console.log(`  Tags: ${chunk.tags.join(", ")}`);
    }
    console.log(`  Created: ${chunk.createdAt}`);
    console.log(`  Updated: ${chunk.updatedAt}`);
    if (chunk.content) {
      console.log();
      console.log(chunk.content);
    }
  });
