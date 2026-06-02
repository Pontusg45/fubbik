import { Command } from "commander";

import { getGitRemoteUrl } from "../lib/detect-space";
import { output, outputQuiet } from "../lib/output";
import { getServerUrl } from "../lib/store";

function requireServer(): string {
    const serverUrl = getServerUrl();
    if (!serverUrl) {
        console.error('No server URL configured. Run "fubbik init" first.');
        process.exit(1);
    }
    return serverUrl;
}

const addSpace = new Command("add")
    .description("Register a space with the server")
    .argument("<name>", "space name")
    .action(async (name: string, _opts: Record<string, unknown>, cmd: Command) => {
        const serverUrl = requireServer();
        const remoteUrl = getGitRemoteUrl();
        const localPath = process.cwd();

        const body: Record<string, unknown> = { name, kind: "code" };
        if (remoteUrl) body.remoteUrl = remoteUrl;
        body.localPaths = [localPath];

        const res = await fetch(`${serverUrl}/api/spaces`, {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify(body)
        });

        if (!res.ok) {
            const text = await res.text();
            console.error(`Failed to create space: ${res.status} ${text}`);
            process.exit(1);
        }

        const data = (await res.json()) as { id: string; name: string };
        outputQuiet(cmd, data.id);
        output(cmd, data, `Created space "${data.name}" (${data.id})`);
    });

const listSpaces = new Command("list")
    .description("List all spaces")
    .action(async (_opts: Record<string, unknown>, cmd: Command) => {
        const serverUrl = requireServer();

        const res = await fetch(`${serverUrl}/api/spaces`);
        if (!res.ok) {
            console.error(`Failed to list spaces: ${res.status}`);
            process.exit(1);
        }

        const data = (await res.json()) as { id: string; name: string; remoteUrl?: string }[];
        outputQuiet(cmd, data.map(c => c.id).join("\n"));

        if (data.length === 0) {
            output(cmd, data, "No spaces found.");
        } else {
            const lines = [`${data.length} space(s):\n`];
            for (const sp of data) {
                const remote = sp.remoteUrl ? ` (${sp.remoteUrl})` : "";
                lines.push(`  ${sp.id}  ${sp.name}${remote}`);
            }
            output(cmd, data, lines.join("\n"));
        }
    });

const removeSpace = new Command("remove")
    .description("Remove a space")
    .argument("<name>", "space name")
    .option("-f, --force", "skip confirmation prompt")
    .action(async (name: string, opts: { force?: boolean }, cmd: Command) => {
        const serverUrl = requireServer();

        // Look up space by name
        const listRes = await fetch(`${serverUrl}/api/spaces`);
        if (!listRes.ok) {
            console.error(`Failed to list spaces: ${listRes.status}`);
            process.exit(1);
        }

        const spaces = (await listRes.json()) as { id: string; name: string }[];
        const match = spaces.find(c => c.name === name);
        if (!match) {
            console.error(`Space "${name}" not found.`);
            process.exit(1);
        }

        if (!opts.force) {
            const readline = await import("node:readline");
            const rl = readline.createInterface({
                input: process.stdin,
                output: process.stdout
            });
            const answer = await new Promise<string>(resolve => {
                rl.question(
                    `This will unlink all chunks from space "${name}". Continue? [y/N] `,
                    resolve
                );
            });
            rl.close();
            if (answer.toLowerCase() !== "y") {
                console.error("Aborted.");
                process.exit(1);
            }
        }

        const delRes = await fetch(`${serverUrl}/api/spaces/${match.id}`, {
            method: "DELETE"
        });
        if (!delRes.ok) {
            console.error(`Failed to delete space: ${delRes.status}`);
            process.exit(1);
        }

        output(cmd, { id: match.id, name }, `Removed space "${name}" (${match.id})`);
    });

const currentSpace = new Command("current")
    .description("Detect the space for the current directory")
    .action(async (_opts: Record<string, unknown>, cmd: Command) => {
        const { detectSpace } = await import("../lib/detect-space");
        const result = await detectSpace();

        if (!result) {
            console.error("No space detected for this directory.");
            process.exit(1);
        }

        outputQuiet(cmd, result.id);
        output(cmd, result, `Current space: "${result.name}" (${result.id})`);
    });

export const spaceCommand = new Command("space")
    .description("Manage spaces")
    .addCommand(addSpace)
    .addCommand(listSpaces)
    .addCommand(removeSpace)
    .addCommand(currentSpace);
