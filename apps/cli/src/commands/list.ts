import { Command } from "commander";

import { output, outputQuiet } from "../lib/output";
import { listChunks } from "../lib/store";

export const listCommand = new Command("list")
    .description("List all chunks")
    .option("--type <type>", "filter by type")
    .option("--tag <tag>", "filter by tag")
    .option("--limit <n>", "max number of results")
    .option("--offset <n>", "skip first n results")
    .option("--sort <field>", "sort by field (title, createdAt, updatedAt)", "updatedAt")
    .option("--sort-dir <dir>", "sort direction (asc, desc)", "desc")
    .option("--fields <fields>", "comma-separated fields to include (e.g. id,title)")
    .action(
        (
            opts: {
                type?: string;
                tag?: string;
                limit?: string;
                offset?: string;
                sort?: string;
                sortDir?: string;
                fields?: string;
            },
            cmd: Command
        ) => {
            let chunks = listChunks({ type: opts.type, tag: opts.tag });

            // Sort
            const sortField = opts.sort ?? "updatedAt";
            const sortDir = opts.sortDir === "asc" ? 1 : -1;
            chunks.sort((a, b) => {
                const aVal = String((a as unknown as Record<string, unknown>)[sortField] ?? "");
                const bVal = String((b as unknown as Record<string, unknown>)[sortField] ?? "");
                return aVal.localeCompare(bVal) * sortDir;
            });

            // Paginate
            const offset = Number(opts.offset) || 0;
            const limit = opts.limit ? Number(opts.limit) : undefined;
            if (offset > 0 || limit !== undefined) {
                chunks = chunks.slice(offset, limit !== undefined ? offset + limit : undefined);
            }

            // Field filter
            let data: unknown = chunks;
            if (opts.fields) {
                const fields = opts.fields.split(",").map(f => f.trim());
                data = chunks.map(c => {
                    const obj: Record<string, unknown> = {};
                    for (const f of fields) {
                        if (f in c) obj[f] = (c as unknown as Record<string, unknown>)[f];
                    }
                    return obj;
                });
            }

            outputQuiet(cmd, chunks.map(c => c.id).join("\n"));
            if (chunks.length === 0) {
                output(cmd, data, "No chunks found.");
            } else {
                const lines = [`${chunks.length} chunk(s):\n`];
                for (const chunk of chunks) {
                    const tags = chunk.tags.length > 0 ? ` [${chunk.tags.join(", ")}]` : "";
                    lines.push(`  ${chunk.id}  ${chunk.title}  (${chunk.type})${tags}`);
                }
                output(cmd, data, lines.join("\n"));
            }
        }
    );
