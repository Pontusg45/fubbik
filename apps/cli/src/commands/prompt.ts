import { readFileSync } from "node:fs";

import { Command } from "commander";

import { formatBold, formatDim, formatSuccess, formatType } from "../lib/colors";
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

export const promptCommand = new Command("prompt")
    .description("Manage reusable prompt templates")

    .addCommand(
        new Command("list")
            .description("List all prompt templates")
            .action(async (_opts, cmd) => {
                const serverUrl = requireServer();

                try {
                    const res = await fetch(`${serverUrl}/api/chunks?tags=prompt&limit=50`);
                    if (!res.ok) {
                        outputError("Failed to fetch prompts");
                        return;
                    }
                    const { chunks } = (await res.json()) as { chunks: any[] };

                    if (isJson(cmd)) {
                        console.log(JSON.stringify(chunks, null, 2));
                        return;
                    }

                    if (chunks.length === 0) {
                        console.error(formatDim("No prompt templates found. Create chunks with the 'prompt' tag."));
                        return;
                    }

                    console.error(formatBold(`${chunks.length} prompt template(s):\n`));
                    for (const c of chunks) {
                        console.error(
                            `  ${formatType(c.type)} ${formatBold(c.title)} ${formatDim(`(${c.id.slice(0, 8)})`)}`
                        );
                        if (c.summary) console.error(`    ${formatDim(c.summary)}`);
                    }
                } catch (e: any) {
                    outputError(e.message);
                }
            })
    )

    .addCommand(
        new Command("get")
            .description("Get a prompt template by title or ID")
            .argument("<name>", "prompt title (partial match) or ID")
            .action(async (name, _opts, cmd) => {
                const serverUrl = requireServer();

                try {
                    // Try by ID first, then search by title
                    let chunk: any = null;

                    // Try direct ID
                    const idRes = await fetch(`${serverUrl}/api/chunks/${name}`).catch(() => null);
                    if (idRes?.ok) {
                        const data = (await idRes.json()) as any;
                        chunk = data.chunk ?? data;
                    }

                    // Try search by title among prompt-tagged chunks
                    if (!chunk) {
                        const searchRes = await fetch(
                            `${serverUrl}/api/chunks?tags=prompt&search=${encodeURIComponent(name)}&limit=1`
                        );
                        if (searchRes.ok) {
                            const { chunks } = (await searchRes.json()) as { chunks: any[] };
                            if (chunks.length > 0) chunk = chunks[0];
                        }
                    }

                    if (!chunk) {
                        outputError(`Prompt "${name}" not found`);
                        return;
                    }

                    if (isJson(cmd)) {
                        console.log(JSON.stringify(chunk, null, 2));
                        return;
                    }

                    // Output the prompt content (ready to pipe)
                    console.log(chunk.content);
                } catch (e: any) {
                    outputError(e.message);
                }
            })
    )

    .addCommand(
        new Command("add")
            .description("Create a new prompt template")
            .requiredOption("-t, --title <title>", "prompt title")
            .option("-c, --content <content>", "prompt content")
            .option("--content-file <path>", "read content from file")
            .action(async (opts, cmd) => {
                const serverUrl = requireServer();

                try {
                    let content = opts.content ?? "";
                    if (opts.contentFile) {
                        content = readFileSync(
                            opts.contentFile === "-" ? "/dev/stdin" : opts.contentFile,
                            "utf-8"
                        );
                    }

                    const res = await fetch(`${serverUrl}/api/chunks`, {
                        method: "POST",
                        headers: { "Content-Type": "application/json" },
                        body: JSON.stringify({
                            title: opts.title,
                            content,
                            type: "note",
                            tags: ["prompt"]
                        })
                    });

                    if (!res.ok) {
                        outputError("Failed to create prompt");
                        return;
                    }
                    const data = await res.json();

                    if (isJson(cmd)) {
                        console.log(JSON.stringify(data, null, 2));
                        return;
                    }
                    console.error(formatSuccess(`Created prompt: ${opts.title}`));
                } catch (e: any) {
                    outputError(e.message);
                }
            })
    );
