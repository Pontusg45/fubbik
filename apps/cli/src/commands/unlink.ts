import { Command } from "commander";

import { getServerUrl } from "../lib/store";
import { output, outputError, outputQuiet } from "../lib/output";

export const unlinkCommand = new Command("unlink")
    .description("Remove a connection by ID from the server")
    .argument("<connection-id>", "connection ID")
    .option("-u, --url <url>", "server URL")
    .option("--token <token>", "auth token")
    .action(async (connectionId: string, opts: { url?: string; token?: string }, cmd: Command) => {
        const serverUrl = opts.url ?? getServerUrl();
        if (!serverUrl) {
            outputError("No server URL configured. Use --url or run sync once with --url.");
            process.exit(1);
        }

        const headers: Record<string, string> = {};
        if (opts.token) headers["Authorization"] = `Bearer ${opts.token}`;

        try {
            const res = await fetch(`${serverUrl}/api/connections/${connectionId}`, {
                method: "DELETE",
                headers
            });

            if (!res.ok) {
                outputError(`✗ Failed to delete connection: ${res.status}`);
                process.exit(1);
            }

            outputQuiet(cmd, connectionId);
            output(cmd, { id: connectionId }, `✓ Removed connection ${connectionId}`);
        } catch (err) {
            outputError(`✗ ${(err as Error).message}`);
            process.exit(1);
        }
    });
