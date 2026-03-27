import { Command } from "commander";

import { formatBold, formatDim, formatType } from "../lib/colors";
import { isJson, outputError } from "../lib/output";
import { getServerUrl } from "../lib/store";

function requireServer(): string {
    const serverUrl = getServerUrl();
    if (!serverUrl) {
        console.error('No server URL configured. Run "fubbik init" first.');
        process.exit(1);
    }
    return serverUrl;
}

export const kbDiffCommand = new Command("kb-diff")
    .description("Show knowledge base changes since a date")
    .option("--since <date>", "date (ISO or relative like '7d', '2w')", "7d")
    .option("--codebase <name>", "scope to codebase")
    .action(async (opts: { since: string; codebase?: string }, cmd: Command) => {
        const serverUrl = requireServer();

        // Parse date
        let sinceDate: Date;
        const relative = opts.since.match(/^(\d+)([dhwm])$/);
        if (relative) {
            const amount = Number(relative[1]);
            const unit = relative[2];
            const ms = { d: 86400000, h: 3600000, w: 604800000, m: 2592000000 }[unit!]!;
            sinceDate = new Date(Date.now() - amount * ms);
        } else {
            sinceDate = new Date(opts.since);
        }

        try {
            // Fetch chunks updated since the date
            const days = Math.ceil((Date.now() - sinceDate.getTime()) / 86400000);
            const params = new URLSearchParams({
                after: String(days),
                sort: "updated",
                limit: "100",
            });
            if (opts.codebase) params.set("codebaseId", opts.codebase);

            const res = await fetch(`${serverUrl}/api/chunks?${params}`);
            if (!res.ok) {
                outputError("Failed to fetch chunks");
                return;
            }
            const { chunks, total } = (await res.json()) as { chunks: any[]; total: number };

            if (isJson(cmd)) {
                console.log(
                    JSON.stringify({ since: sinceDate.toISOString(), chunks, total }, null, 2)
                );
                return;
            }

            if (chunks.length === 0) {
                console.error(formatDim(`No changes since ${sinceDate.toLocaleDateString()}`));
                return;
            }

            console.error(
                formatBold(
                    `${total} chunk(s) changed since ${sinceDate.toLocaleDateString()}:\n`
                )
            );

            for (const c of chunks) {
                const updated = new Date(c.updatedAt);
                const created = new Date(c.createdAt);
                const isNew = Math.abs(updated.getTime() - created.getTime()) < 60000;
                const label = isNew ? "NEW" : "UPD";
                console.error(
                    `  ${formatType(label)} ${formatBold(c.title)} ${formatDim(`(${c.type}, ${updated.toLocaleDateString()})`)}`
                );
            }
        } catch (e: any) {
            outputError(e.message);
        }
    });
