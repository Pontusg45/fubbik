import { Command } from "commander";

import { output, outputError } from "../lib/output";
import { readStore } from "../lib/store";

export const enrichCommand = new Command("enrich")
    .description("Trigger AI enrichment (summary, aliases, embedding) for chunks")
    .argument("[id]", "chunk ID to enrich (omit for all)")
    .option("--all", "enrich all chunks")
    .action(async (id, opts, cmd) => {
        const store = readStore();
        if (!store.serverUrl) {
            outputError("No server URL configured. Run 'fubbik init' first.");
            process.exit(1);
        }

        if (id) {
            const res = await fetch(`${store.serverUrl}/api/chunks/${id}/enrich`, { method: "POST" });
            if (!res.ok) {
                outputError(`Failed to enrich chunk ${id}: ${res.status}`);
                process.exit(1);
            }
            output(cmd, await res.json(), `Enriched chunk ${id}`);
        } else if (opts.all) {
            const res = await fetch(`${store.serverUrl}/api/chunks/enrich-all`, { method: "POST" });
            if (!res.ok) {
                outputError(`Failed to enrich chunks: ${res.status}`);
                process.exit(1);
            }
            const data = (await res.json()) as { enriched: number };
            output(cmd, data, `Enriched ${data.enriched} chunks`);
        } else {
            outputError("Provide a chunk ID or use --all");
            process.exit(1);
        }
    });
