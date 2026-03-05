import { Command } from "commander";

import { createStore, storeDir, storeExists } from "../store";

export const initCommand = new Command("init")
  .description("Initialize a new knowledge base")
  .argument("[name]", "name for the knowledge base", "my-knowledge-base")
  .option("-f, --force", "overwrite existing knowledge base")
  .action((name: string, opts: { force?: boolean }) => {
    if (storeExists() && !opts.force) {
      console.error("✗ Knowledge base already exists in this directory.");
      console.error("  Use --force to overwrite.");
      process.exit(1);
    }

    const store = createStore(name);
    console.log(`✓ Initialized knowledge base: ${store.name}`);
    console.log(`  Location: ${storeDir()}`);
  });
