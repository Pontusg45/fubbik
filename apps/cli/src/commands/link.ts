import { Command } from "commander";

import { formatRelation, formatSuccess } from "../lib/colors";
import { output, outputError, outputQuiet } from "../lib/output";
import { getServerUrl } from "../lib/store";

export const linkCommand = new Command("link")
    .description("Create a connection between two chunks on the server")
    .argument("<source-id>", "source chunk ID")
    .argument("<target-id>", "target chunk ID")
    .option("-r, --relation <type>", "relation type", "related")
    .option("-u, --url <url>", "server URL")
    .option("--token <token>", "auth token")
    .action(async (sourceId: string, targetId: string, opts: { relation: string; url?: string; token?: string }, cmd: Command) => {
        const serverUrl = opts.url ?? getServerUrl();
        if (!serverUrl) {
            outputError("No server URL configured. Use --url or run sync once with --url.");
            process.exit(1);
        }

        const headers: Record<string, string> = { "Content-Type": "application/json" };
        if (opts.token) headers["Authorization"] = `Bearer ${opts.token}`;

        try {
            const res = await fetch(`${serverUrl}/api/connections`, {
                method: "POST",
                headers,
                body: JSON.stringify({ sourceId, targetId, relation: opts.relation })
            });

            if (!res.ok) {
                outputError(`Failed to create connection: ${res.status} ${await res.text()}`);
                process.exit(1);
            }

            const data = await res.json();
            outputQuiet(cmd, (data as { id?: string }).id ?? "");
            output(cmd, data, formatSuccess(`Linked ${sourceId} \u2192 ${targetId} (${formatRelation(opts.relation)})`));
        } catch (err) {
            outputError((err as Error).message);
            process.exit(1);
        }
    });
