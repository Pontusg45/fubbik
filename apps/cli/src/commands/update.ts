import { Command } from "commander";

import { updateChunk } from "../store";

export const updateCommand = new Command("update")
  .description("Update a chunk by ID")
  .argument("<id>", "chunk ID")
  .option("-t, --title <title>", "new title")
  .option("-c, --content <content>", "new content")
  .option("--type <type>", "new type")
  .option("--tags <tags>", "new comma-separated tags")
  .action(
    (id: string, opts: { title?: string; content?: string; type?: string; tags?: string }) => {
      const updates: Record<string, unknown> = {};
      if (opts.title !== undefined) updates.title = opts.title;
      if (opts.content !== undefined) updates.content = opts.content;
      if (opts.type !== undefined) updates.type = opts.type;
      if (opts.tags !== undefined) updates.tags = opts.tags.split(",").map((t) => t.trim());

      if (Object.keys(updates).length === 0) {
        console.error("✗ No updates provided. Use --title, --content, --type, or --tags.");
        process.exit(1);
      }

      const chunk = updateChunk(id, updates);
      if (!chunk) {
        console.error(`✗ Chunk "${id}" not found.`);
        process.exit(1);
      }

      console.log(`✓ Updated chunk ${chunk.id}`);
      console.log(`  Title: ${chunk.title}`);
      console.log(`  Type: ${chunk.type}`);
      if (chunk.tags.length > 0) {
        console.log(`  Tags: ${chunk.tags.join(", ")}`);
      }
    },
  );
