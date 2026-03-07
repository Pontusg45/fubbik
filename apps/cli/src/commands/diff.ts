import { Command } from "commander";

import { output, outputError } from "../lib/output";
import { getServerUrl, readStore } from "../lib/store";

export const diffCommand = new Command("diff")
    .description("Show differences between local and server chunks")
    .option("-u, --url <url>", "server URL")
    .option("--token <token>", "auth token")
    .action(async (opts: { url?: string; token?: string }, cmd: Command) => {
        const store = readStore();
        const serverUrl = opts.url ?? getServerUrl();
        if (!serverUrl) {
            outputError("No server URL configured. Use --url or run sync once with --url.");
            process.exit(1);
        }

        const headers: Record<string, string> = {};
        if (opts.token) headers["Authorization"] = `Bearer ${opts.token}`;

        try {
            const res = await fetch(`${serverUrl}/api/chunks?limit=1000`, { headers });
            if (!res.ok) {
                outputError(`✗ Server returned ${res.status}`);
                process.exit(1);
            }

            const serverData = (await res.json()) as { chunks: Array<{ id: string; title: string; updatedAt: string }> };
            const serverChunks = serverData.chunks;

            const localByTitle = new Map(store.chunks.map(c => [c.title, c]));
            const serverByTitle = new Map(serverChunks.map(c => [c.title, c]));

            const localOnly = store.chunks.filter(c => !serverByTitle.has(c.title));
            const serverOnly = serverChunks.filter(c => !localByTitle.has(c.title));
            const modified = store.chunks.filter(c => {
                const sc = serverByTitle.get(c.title);
                return sc && sc.updatedAt !== c.updatedAt;
            });

            const data = {
                localOnly: localOnly.map(c => ({ id: c.id, title: c.title })),
                serverOnly: serverOnly.map(c => ({ id: c.id, title: c.title })),
                modified: modified.map(c => ({ id: c.id, title: c.title })),
                lastSync: store.lastSync ?? null
            };

            const lines = [`Last sync: ${store.lastSync ?? "never"}`, `Local: ${store.chunks.length}, Server: ${serverChunks.length}`, ""];

            if (localOnly.length > 0) {
                lines.push(`Local only (${localOnly.length}):`);
                for (const c of localOnly) lines.push(`  + ${c.id}  ${c.title}`);
                lines.push("");
            }
            if (serverOnly.length > 0) {
                lines.push(`Server only (${serverOnly.length}):`);
                for (const c of serverOnly) lines.push(`  - ${c.id}  ${c.title}`);
                lines.push("");
            }
            if (modified.length > 0) {
                lines.push(`Modified (${modified.length}):`);
                for (const c of modified) lines.push(`  ~ ${c.id}  ${c.title}`);
                lines.push("");
            }
            if (localOnly.length === 0 && serverOnly.length === 0 && modified.length === 0) {
                lines.push("No differences found.");
            }

            output(cmd, data, lines.join("\n"));
        } catch (err) {
            outputError(`✗ ${(err as Error).message}`);
            process.exit(1);
        }
    });
