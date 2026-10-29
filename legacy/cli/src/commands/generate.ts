import { Command } from "commander";

import { output, outputQuiet } from "../lib/output";
import { getServerUrl } from "../lib/store";

async function resolveSpaceIdLocal(serverUrl: string, nameOrId: string): Promise<string> {
    // Try to use as-is first (it might be an ID)
    const listRes = await fetch(`${serverUrl}/api/spaces`);
    if (!listRes.ok) {
        console.error(`Failed to list spaces: ${listRes.status}`);
        process.exit(1);
    }

    const spaces = (await listRes.json()) as { id: string; name: string }[];
    const match = spaces.find(c => c.id === nameOrId || c.name === nameOrId);
    if (!match) {
        console.error(`Space "${nameOrId}" not found.`);
        process.exit(1);
    }

    return match.id;
}

const generateClaudeMd = new Command("claude.md")
    .description("Generate a CLAUDE.md file from knowledge base")
    .requiredOption("-s, --space <name>", "space name or ID")
    .action(async (opts: { space: string }, cmd: Command) => {
        const serverUrl = getServerUrl();
        if (!serverUrl) {
            console.error('No server URL configured. Run "fubbik init" first.');
            process.exit(1);
        }

        const spaceId = await resolveSpaceIdLocal(serverUrl, opts.space);
        const res = await fetch(`${serverUrl}/api/spaces/${spaceId}/generate-instructions?format=claude`);
        if (!res.ok) {
            const text = await res.text();
            console.error(`Failed to generate CLAUDE.md: ${res.status} ${text}`);
            process.exit(1);
        }

        const data = (await res.json()) as { content: string };
        outputQuiet(cmd, data.content);
        output(cmd, data, data.content);
    });

const generateAgentsMd = new Command("agents.md")
    .description("Generate an AGENTS.md file from knowledge base")
    .requiredOption("-s, --space <name>", "space name or ID")
    .action(async (opts: { space: string }, cmd: Command) => {
        const serverUrl = getServerUrl();
        if (!serverUrl) {
            console.error('No server URL configured. Run "fubbik init" first.');
            process.exit(1);
        }

        const spaceId = await resolveSpaceIdLocal(serverUrl, opts.space);
        const res = await fetch(`${serverUrl}/api/spaces/${spaceId}/generate-instructions?format=agents`);
        if (!res.ok) {
            const text = await res.text();
            console.error(`Failed to generate AGENTS.md: ${res.status} ${text}`);
            process.exit(1);
        }

        const data = (await res.json()) as { content: string };
        outputQuiet(cmd, data.content);
        output(cmd, data, data.content);
    });

const generateCursorRules = new Command("cursorrules")
    .description("Generate a .cursorrules file from knowledge base")
    .requiredOption("-s, --space <name>", "space name or ID")
    .action(async (opts: { space: string }, cmd: Command) => {
        const serverUrl = getServerUrl();
        if (!serverUrl) {
            console.error('No server URL configured. Run "fubbik init" first.');
            process.exit(1);
        }

        const spaceId = await resolveSpaceIdLocal(serverUrl, opts.space);
        const res = await fetch(`${serverUrl}/api/spaces/${spaceId}/generate-instructions?format=cursor`);
        if (!res.ok) {
            const text = await res.text();
            console.error(`Failed to generate .cursorrules: ${res.status} ${text}`);
            process.exit(1);
        }

        const data = (await res.json()) as { content: string };
        outputQuiet(cmd, data.content);
        output(cmd, data, data.content);
    });

export const generateCommand = new Command("generate")
    .description("Generate instruction files from knowledge base")
    .addCommand(generateClaudeMd)
    .addCommand(generateAgentsMd)
    .addCommand(generateCursorRules);
