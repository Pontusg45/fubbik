import { Command } from "commander";

import { output, outputError, outputQuiet } from "../lib/output";
import { createStore, storeDir, storeExists } from "../lib/store";

export const initCommand = new Command("init")
    .description("Initialize a new knowledge base")
    .argument("[name]", "name for the knowledge base", "my-knowledge-base")
    .option("-f, --force", "overwrite existing knowledge base")
    .action((name: string, opts: { force?: boolean }, cmd: Command) => {
        if (storeExists() && !opts.force) {
            outputError("✗ Knowledge base already exists in this directory.");
            outputError("  Use --force to overwrite.");
            process.exit(1);
        }

        const store = createStore(name);
        outputQuiet(cmd, store.name);
        output(
            cmd,
            { name: store.name, location: storeDir() },
            [`✓ Initialized knowledge base: ${store.name}`, `  Location: ${storeDir()}`].join("\n")
        );
    });
