import { Command } from "commander";

import { output } from "../lib/output";
import { readStore } from "../lib/store";

export const statsCommand = new Command("stats").description("Show knowledge base statistics").action((_opts: unknown, cmd: Command) => {
    const store = readStore();
    const chunks = store.chunks;

    const typeCounts = new Map<string, number>();
    const tagCounts = new Map<string, number>();

    for (const chunk of chunks) {
        typeCounts.set(chunk.type, (typeCounts.get(chunk.type) ?? 0) + 1);
        for (const tag of chunk.tags) {
            tagCounts.set(tag, (tagCounts.get(tag) ?? 0) + 1);
        }
    }

    const data = {
        name: store.name,
        totalChunks: chunks.length,
        types: Object.fromEntries(typeCounts),
        uniqueTags: tagCounts.size,
        topTags: [...tagCounts.entries()]
            .sort((a, b) => b[1] - a[1])
            .slice(0, 5)
            .map(([tag, count]) => ({ tag, count })),
        lastSync: store.lastSync ?? null,
        serverUrl: store.serverUrl ?? null
    };

    const lines = [
        `Knowledge base: ${store.name}`,
        `Total chunks: ${chunks.length}`,
        "",
        "Types:",
        ...[...typeCounts.entries()].map(([type, count]) => `  ${type}: ${count}`),
        "",
        `Unique tags: ${tagCounts.size}`,
        ...(data.topTags.length > 0 ? ["Top tags:", ...data.topTags.map(t => `  ${t.tag} (${t.count})`)] : []),
        "",
        `Last sync: ${store.lastSync ?? "never"}`,
        `Server: ${store.serverUrl ?? "not configured"}`
    ];

    output(cmd, data, lines.join("\n"));
});
