import { Command } from "commander";

import { getGitRemoteUrl } from "../lib/detect-codebase";
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

const addCodebase = new Command("add")
    .description("Register a codebase with the server")
    .argument("<name>", "codebase name")
    .action(async (name: string, _opts: Record<string, unknown>, cmd: Command) => {
        const serverUrl = requireServer();
        const remoteUrl = getGitRemoteUrl();
        const localPath = process.cwd();

        const body: Record<string, unknown> = { name };
        if (remoteUrl) body.remoteUrl = remoteUrl;
        body.localPaths = [localPath];

        const res = await fetch(`${serverUrl}/api/codebases`, {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify(body)
        });

        if (!res.ok) {
            const text = await res.text();
            console.error(`Failed to create codebase: ${res.status} ${text}`);
            process.exit(1);
        }

        const data = (await res.json()) as { id: string; name: string };
        outputQuiet(cmd, data.id);
        output(cmd, data, `Created codebase "${data.name}" (${data.id})`);
    });

const listCodebases = new Command("list")
    .description("List all codebases")
    .action(async (_opts: Record<string, unknown>, cmd: Command) => {
        const serverUrl = requireServer();

        const res = await fetch(`${serverUrl}/api/codebases`);
        if (!res.ok) {
            console.error(`Failed to list codebases: ${res.status}`);
            process.exit(1);
        }

        const data = (await res.json()) as { id: string; name: string; remoteUrl?: string }[];
        outputQuiet(cmd, data.map(c => c.id).join("\n"));

        if (data.length === 0) {
            output(cmd, data, "No codebases found.");
        } else {
            const lines = [`${data.length} codebase(s):\n`];
            for (const cb of data) {
                const remote = cb.remoteUrl ? ` (${cb.remoteUrl})` : "";
                lines.push(`  ${cb.id}  ${cb.name}${remote}`);
            }
            output(cmd, data, lines.join("\n"));
        }
    });

const removeCodebase = new Command("remove")
    .description("Remove a codebase")
    .argument("<name>", "codebase name")
    .option("-f, --force", "skip confirmation prompt")
    .action(async (name: string, opts: { force?: boolean }, cmd: Command) => {
        const serverUrl = requireServer();

        // Look up codebase by name
        const listRes = await fetch(`${serverUrl}/api/codebases`);
        if (!listRes.ok) {
            console.error(`Failed to list codebases: ${listRes.status}`);
            process.exit(1);
        }

        const codebases = (await listRes.json()) as { id: string; name: string }[];
        const match = codebases.find(c => c.name === name);
        if (!match) {
            console.error(`Codebase "${name}" not found.`);
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
                    `This will unlink all chunks from codebase "${name}". Continue? [y/N] `,
                    resolve
                );
            });
            rl.close();
            if (answer.toLowerCase() !== "y") {
                console.error("Aborted.");
                process.exit(1);
            }
        }

        const delRes = await fetch(`${serverUrl}/api/codebases/${match.id}`, {
            method: "DELETE"
        });
        if (!delRes.ok) {
            console.error(`Failed to delete codebase: ${delRes.status}`);
            process.exit(1);
        }

        output(cmd, { id: match.id, name }, `Removed codebase "${name}" (${match.id})`);
    });

const currentCodebase = new Command("current")
    .description("Detect the codebase for the current directory")
    .action(async (_opts: Record<string, unknown>, cmd: Command) => {
        const { detectCodebase } = await import("../lib/detect-codebase");
        const result = await detectCodebase();

        if (!result) {
            console.error("No codebase detected for this directory.");
            process.exit(1);
        }

        outputQuiet(cmd, result.id);
        output(cmd, result, `Current codebase: "${result.name}" (${result.id})`);
    });

export const codebaseCommand = new Command("codebase")
    .description("Manage codebases")
    .addCommand(addCodebase)
    .addCommand(listCodebases)
    .addCommand(removeCodebase)
    .addCommand(currentCodebase);
