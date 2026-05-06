import { Command } from "commander";

import { formatId, formatSuccess, formatTag } from "../lib/colors";
import { output, outputError } from "../lib/output";
import { getServerUrl } from "../lib/store";

export const updatesCommand = new Command("updates")
    .description("List chunk updates by tag")
    .option("--tag <tag>", "filter updates by tag")
    .option("--tags", "list all update tags with counts")
    .option("--codebase-id <id>", "filter to a specific codebase")
    .action(async (opts: { tag?: string; tags?: boolean; codebaseId?: string }, cmd: Command) => {
        const serverUrl = getServerUrl();
        if (!serverUrl) {
            outputError("Update tags require server mode. Set FUBBIK_SERVER_URL.");
            return;
        }

        if (opts.tags) {
            const params = new URLSearchParams();
            if (opts.codebaseId) params.set("codebaseId", opts.codebaseId);

            try {
                const res = await fetch(`${serverUrl}/api/chunks/updates/tags?${params}`);
                if (!res.ok) {
                    outputError(`Server error (${res.status}): ${await res.text()}`);
                    return;
                }
                const { tags } = (await res.json()) as { tags: { tag: string; count: number }[] };
                if (tags.length === 0) {
                    output(cmd, tags, "No update tags found.");
                    return;
                }
                const lines = tags.map(t => `  ${formatTag(t.tag)} — ${t.count} update${t.count !== 1 ? "s" : ""}`);
                output(cmd, tags, ["Update tags:", ...lines].join("\n"));
            } catch (err) {
                outputError(`Failed to connect: ${err instanceof Error ? err.message : err}`);
            }
            return;
        }

        if (!opts.tag) {
            outputError("Provide --tag <name> to list updates, or --tags to list all tags.");
            return;
        }

        const params = new URLSearchParams({ tag: opts.tag });
        if (opts.codebaseId) params.set("codebaseId", opts.codebaseId);

        try {
            const res = await fetch(`${serverUrl}/api/chunks/updates?${params}`);
            if (!res.ok) {
                outputError(`Server error (${res.status}): ${await res.text()}`);
                return;
            }
            const { updates } = (await res.json()) as {
                updates: Array<{
                    versionId: string;
                    chunkId: string;
                    chunkTitle: string;
                    version: number;
                    createdAt: string;
                    before: { title: string | null; content: string | null };
                    after: { title: string; content: string };
                }>;
            };

            if (updates.length === 0) {
                output(cmd, updates, `No updates found for tag "${opts.tag}".`);
                return;
            }

            const lines: string[] = [
                formatSuccess(`${updates.length} update${updates.length !== 1 ? "s" : ""} tagged "${opts.tag}":`),
                ""
            ];

            for (const u of updates) {
                const isCreation = u.version === 0;
                const action = isCreation ? "created" : "updated";
                const date = new Date(u.createdAt).toLocaleDateString();
                lines.push(`  ${formatId(u.chunkId)} ${u.chunkTitle} — ${action} ${date}`);

                if (!isCreation && u.before.title && u.before.title !== u.after.title) {
                    lines.push(`    title: "${u.before.title}" → "${u.after.title}"`);
                }
            }

            output(cmd, updates, lines.join("\n"));
        } catch (err) {
            outputError(`Failed to connect: ${err instanceof Error ? err.message : err}`);
        }
    });
