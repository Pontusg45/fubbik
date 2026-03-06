import { Command } from "commander";

import { listChunks } from "../lib/store";

export const listCommand = new Command("list")
    .description("List all chunks")
    .option("--type <type>", "filter by type")
    .option("--tag <tag>", "filter by tag")
    .option("--json", "output as JSON")
    .action((opts: { type?: string; tag?: string; json?: boolean }) => {
        const chunks = listChunks({ type: opts.type, tag: opts.tag });

        if (opts.json) {
            console.log(JSON.stringify(chunks, null, 2));
            return;
        }

        if (chunks.length === 0) {
            console.log("No chunks found.");
            return;
        }

        console.log(`${chunks.length} chunk(s):\n`);
        for (const chunk of chunks) {
            const tags = chunk.tags.length > 0 ? ` [${chunk.tags.join(", ")}]` : "";
            console.log(`  ${chunk.id}  ${chunk.title}  (${chunk.type})${tags}`);
        }
    });
