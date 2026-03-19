import { Command } from "commander";

import { formatId, formatTag, formatTitle, formatType } from "../lib/colors";
import { resolveCodebaseId } from "../lib/detect-codebase";
import { output, outputError, outputQuiet } from "../lib/output";
import { listChunks, readStore } from "../lib/store";

export const listCommand = new Command("list")
    .description("List all chunks")
    .option("--type <type>", "filter by type")
    .option("--tag <tag>", "filter by tag")
    .option("--limit <n>", "max number of results")
    .option("--offset <n>", "skip first n results")
    .option("--sort <field>", "sort by field (title, createdAt, updatedAt)", "updatedAt")
    .option("--sort-dir <dir>", "sort direction (asc, desc)", "desc")
    .option("--fields <fields>", "comma-separated fields to include (e.g. id,title)")
    .option("--scope <pairs>", "filter by scope (key:value,key:value) — requires server")
    .option("--exclude <terms>", "exclude chunks about these terms (comma-separated) — requires server")
    .option("--global", "skip codebase scoping (show all chunks)")
    .option("--codebase <name>", "scope to a specific codebase by name")
    .action(
        async (
            opts: {
                type?: string;
                tag?: string;
                limit?: string;
                offset?: string;
                sort?: string;
                sortDir?: string;
                fields?: string;
                scope?: string;
                exclude?: string;
                global?: boolean;
                codebase?: string;
            },
            cmd: Command
        ) => {
            if (opts.scope || opts.exclude) {
                const store = readStore();
                if (!store.serverUrl) {
                    outputError("No server URL configured. Run 'fubbik init' first.");
                    process.exit(1);
                }
                const params = new URLSearchParams();
                if (opts.type) params.set("type", opts.type);
                if (opts.scope) params.set("scope", opts.scope);
                if (opts.exclude) params.set("exclude", opts.exclude);
                if (opts.limit) params.set("limit", opts.limit);
                if (opts.offset) params.set("offset", opts.offset);

                const codebaseId = await resolveCodebaseId(store.serverUrl, {
                    global: opts.global,
                    codebase: opts.codebase
                });
                if (codebaseId) params.set("codebaseId", codebaseId);

                const res = await fetch(`${store.serverUrl}/api/chunks?${params}`);
                if (!res.ok) {
                    outputError(`Server list failed: ${res.status}`);
                    process.exit(1);
                }
                const data = (await res.json()) as { chunks: { id: string; title: string; type: string; tags: string[] }[] };
                const chunks = data.chunks;
                outputQuiet(cmd, chunks.map(c => c.id).join("\n"));
                if (chunks.length === 0) {
                    output(cmd, chunks, "No chunks found.");
                } else {
                    const lines = [`${chunks.length} chunk(s):\n`];
                    for (const chunk of chunks) {
                        const tags = chunk.tags.length > 0 ? ` [${chunk.tags.map(formatTag).join(", ")}]` : "";
                        lines.push(`  ${formatId(chunk.id)}  ${formatTitle(chunk.title)}  ${formatType(chunk.type)}${tags}`);
                    }
                    output(cmd, chunks, lines.join("\n"));
                }
                return;
            }

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
                    const tags = chunk.tags.length > 0 ? ` [${chunk.tags.map(formatTag).join(", ")}]` : "";
                    lines.push(`  ${formatId(chunk.id)}  ${formatTitle(chunk.title)}  ${formatType(chunk.type)}${tags}`);
                }
                output(cmd, data, lines.join("\n"));
            }
        }
    );
