import { Command } from "commander";

import { output, outputError } from "../lib/output";

const DEFAULT_URL = "http://localhost:3000";

export const healthCommand = new Command("health")
    .description("Check API server connection")
    .option("-u, --url <url>", "server URL", DEFAULT_URL)
    .action(async (opts: { url: string }, cmd: Command) => {
        try {
            const res = await fetch(opts.url);
            const body = await res.text();

            if (res.ok) {
                output(
                    cmd,
                    { status: res.status, url: opts.url, response: body },
                    [`✓ Connected to ${opts.url}`, `  Status: ${res.status}`, `  Response: ${body}`].join("\n")
                );
            } else {
                outputError(`✗ Server returned ${res.status}`);
                process.exit(1);
            }
        } catch (err) {
            outputError(`✗ Could not connect to ${opts.url}`);
            if (err instanceof Error) outputError(`  ${err.message}`);
            process.exit(1);
        }
    });
