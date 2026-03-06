import { Command } from "commander";

import { searchChunks } from "../lib/store";

export const searchCommand = new Command("search")
    .description("Search chunks by title, content, or tags")
    .argument("<query>", "search query")
    .option("--json", "output as JSON")
    .action((query: string, opts: { json?: boolean }) => {
        const results = searchChunks(query);

        if (opts.json) {
            console.log(JSON.stringify(results, null, 2));
            return;
        }

        if (results.length === 0) {
            console.log(`No chunks matching "${query}".`);
            return;
        }

        console.log(`${results.length} result(s) for "${query}":\n`);
        for (const chunk of results) {
            const tags = chunk.tags.length > 0 ? ` [${chunk.tags.join(", ")}]` : "";
            console.log(`  ${chunk.id}  ${chunk.title}  (${chunk.type})${tags}`);
        }
    });
