import { Command } from "commander";

import { formatSuccess } from "../lib/colors";
import { output, outputError, outputQuiet } from "../lib/output";
import { confirm } from "../lib/prompt";
import { getServerUrl } from "../lib/store";

export const unlinkCommand = new Command("unlink")
    .description("Remove a connection by ID from the server")
    .argument("<connection-id>", "connection ID")
    .option("-u, --url <url>", "server URL")
    .option("--token <token>", "auth token")
    .option("-y, --yes", "skip confirmation prompt")
    .action(async (connectionId: string, opts: { url?: string; token?: string; yes?: boolean }, cmd: Command) => {
        if (!opts.yes) {
            const ok = await confirm(`Delete connection "${connectionId}"?`);
            if (!ok) {
                console.error("Aborted.");
                return;
            }
        }

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
                outputError(`Failed to delete connection: ${res.status}`);
                process.exit(1);
            }

            outputQuiet(cmd, connectionId);
            output(cmd, { id: connectionId }, formatSuccess(`Removed connection ${connectionId}`));
        } catch (err) {
            outputError((err as Error).message);
            process.exit(1);
        }
    });
