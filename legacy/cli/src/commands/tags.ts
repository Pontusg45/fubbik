import { Command } from "commander";

import { output, outputQuiet } from "../lib/output";
import { readStore } from "../lib/store";

export const tagsCommand = new Command("list").description("List all unique tags with counts").action((_opts: unknown, cmd: Command) => {
    const store = readStore();
    const counts = new Map<string, number>();

    for (const chunk of store.chunks) {
        for (const tag of chunk.tags) {
            counts.set(tag, (counts.get(tag) ?? 0) + 1);
        }
    }

    const sorted = [...counts.entries()].sort((a, b) => b[1] - a[1]);
    const data = sorted.map(([tag, count]) => ({ tag, count }));

    outputQuiet(cmd, sorted.map(([tag]) => tag).join("\n"));

    if (sorted.length === 0) {
        output(cmd, data, "No tags found.");
    } else {
        const lines = [`${sorted.length} tag(s):\n`];
        for (const [tag, count] of sorted) {
            lines.push(`  ${tag} (${count})`);
        }
        output(cmd, data, lines.join("\n"));
    }
});
