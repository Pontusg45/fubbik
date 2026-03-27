import { watch } from "node:fs";

import { Command } from "commander";

import { formatBold, formatDim, formatType } from "../lib/colors";
import { getServerUrl } from "../lib/store";

function requireServer(): string {
    const serverUrl = getServerUrl();
    if (!serverUrl) {
        console.error('No server URL configured. Run "fubbik init" first.');
        process.exit(1);
    }
    return serverUrl;
}

export const watchCommand = new Command("watch")
    .description("Watch file changes and show relevant chunks")
    .option("--dir <dir>", "directory to watch", ".")
    .action(async (opts: { dir: string }) => {
        const serverUrl = requireServer();

        console.error(formatBold(`Watching ${opts.dir} for changes...\n`));

        // Debounce per file
        const pending = new Map<string, NodeJS.Timeout>();

        watch(opts.dir, { recursive: true }, (_eventType, filename) => {
            if (!filename || filename.startsWith(".") || filename.includes("node_modules")) return;

            const key = filename;
            if (pending.has(key)) clearTimeout(pending.get(key));
            pending.set(
                key,
                setTimeout(async () => {
                    pending.delete(key);
                    try {
                        const res = await fetch(
                            `${serverUrl}/api/context/for-file?path=${encodeURIComponent(filename)}`
                        );
                        if (!res.ok) return;
                        const chunks = await res.json();
                        if (!Array.isArray(chunks) || chunks.length === 0) return;

                        console.error(
                            `\n${formatDim(`[${new Date().toLocaleTimeString()}]`)} ${formatBold(filename)} — ${chunks.length} relevant chunk(s):`
                        );
                        for (const c of chunks.slice(0, 5) as Array<{
                            type: string;
                            title: string;
                            matchReason: string;
                        }>) {
                            console.error(
                                `  ${formatType(c.type)} ${c.title} ${formatDim(`(${c.matchReason})`)}`
                            );
                        }
                    } catch {
                        /* ignore fetch errors */
                    }
                }, 500)
            );
        });

        // Keep process alive
        await new Promise(() => {});
    });
