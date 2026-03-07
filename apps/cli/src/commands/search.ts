import { Command } from "commander";

import { output, outputError, outputQuiet } from "../lib/output";
import { readStore, searchChunks } from "../lib/store";

export const searchCommand = new Command("search")
    .description("Search chunks by title, content, or tags")
    .argument("<query>", "search query")
    .option("--limit <n>", "max number of results")
    .option("--offset <n>", "skip first n results")
    .option("--fields <fields>", "comma-separated fields to include")
    .option("--semantic", "use semantic (AI embedding) search via server")
    .action(async (query: string, opts: { limit?: string; offset?: string; fields?: string; semantic?: boolean }, cmd: Command) => {
        if (opts.semantic) {
            const store = readStore();
            if (!store.serverUrl) {
                outputError("No server URL configured. Run 'fubbik init' first.");
                process.exit(1);
            }
            const params = new URLSearchParams({ q: query });
            if (opts.limit) params.set("limit", opts.limit);
            const res = await fetch(`${store.serverUrl}/api/chunks/search/semantic?${params}`);
            if (!res.ok) {
                outputError(`Semantic search failed: ${res.status}`);
                process.exit(1);
            }
            const results = await res.json() as { id: string; title: string; type: string; similarity: number }[];
            outputQuiet(cmd, results.map(c => c.id).join("\n"));
            if (results.length === 0) {
                output(cmd, results, `No semantic matches for "${query}".`);
            } else {
                const lines = [`${results.length} semantic result(s) for "${query}":\n`];
                for (const r of results) {
                    lines.push(`  ${r.id}  ${r.title}  (${r.type})  score: ${r.similarity.toFixed(3)}`);
                }
                output(cmd, results, lines.join("\n"));
            }
            return;
        }

        let results = searchChunks(query);

        const offset = Number(opts.offset) || 0;
        const limit = opts.limit ? Number(opts.limit) : undefined;
        if (offset > 0 || limit !== undefined) {
            results = results.slice(offset, limit !== undefined ? offset + limit : undefined);
        }

        let data: unknown = results;
        if (opts.fields) {
            const fields = opts.fields.split(",").map(f => f.trim());
            data = results.map(c => {
                const obj: Record<string, unknown> = {};
                for (const f of fields) {
                    if (f in c) obj[f] = (c as unknown as Record<string, unknown>)[f];
                }
                return obj;
            });
        }

        outputQuiet(cmd, results.map(c => c.id).join("\n"));
        if (results.length === 0) {
            output(cmd, data, `No chunks matching "${query}".`);
        } else {
            const lines = [`${results.length} result(s) for "${query}":\n`];
            for (const chunk of results) {
                const tags = chunk.tags.length > 0 ? ` [${chunk.tags.join(", ")}]` : "";
                lines.push(`  ${chunk.id}  ${chunk.title}  (${chunk.type})${tags}`);
            }
            output(cmd, data, lines.join("\n"));
        }
    });
