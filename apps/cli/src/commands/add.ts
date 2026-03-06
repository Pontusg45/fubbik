import { Command } from "commander";

import { addChunk } from "../lib/store";

export const addCommand = new Command("add")
  .description("Add a new chunk to the knowledge base")
  .requiredOption("-t, --title <title>", "chunk title")
  .option("-c, --content <content>", "chunk content", "")
  .option("--type <type>", "chunk type", "note")
  .option("--tags <tags>", "comma-separated tags", "")
  .action((opts: { title: string; content: string; type: string; tags: string }) => {
    const tags = opts.tags ? opts.tags.split(",").map((t) => t.trim()) : [];
    const chunk = addChunk({
      title: opts.title,
      content: opts.content,
      type: opts.type,
      tags,
    });
    console.log(`✓ Created chunk ${chunk.id}`);
    console.log(`  Title: ${chunk.title}`);
    console.log(`  Type: ${chunk.type}`);
    if (tags.length > 0) {
      console.log(`  Tags: ${tags.join(", ")}`);
    }
  });
