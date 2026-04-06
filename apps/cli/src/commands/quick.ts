import { readFileSync } from "node:fs";

import { Command } from "commander";

import { formatId, formatSuccess, formatTag, formatType } from "../lib/colors";
import { resolveCodebaseId } from "../lib/detect-codebase";
import { output, outputError, outputQuiet } from "../lib/output";
import { addChunk, getServerUrl } from "../lib/store";

export const quickCommand = new Command("quick")
    .description("Quickly create a chunk from a one-liner or piped content")
    .argument("[title...]", "chunk title (joined with spaces)")
    .option("-t, --title <title>", "chunk title (alternative to positional argument)")
    .option("--type <type>", "chunk type", "note")
    .option("--tags <tags>", "comma-separated tags", "")
    .option("--global", "skip codebase scoping")
    .option("--codebase <name>", "scope to a specific codebase by name")
    .action(async (titleWords: string[], opts: {
        title?: string;
        type: string;
        tags: string;
        global?: boolean;
        codebase?: string;
    }, cmd: Command) => {
        const title = opts.title || titleWords.join(" ");
        if (!title) {
            outputError("Title is required. Pass as argument or use --title.");
            return;
        }

        // Read content from stdin if piped
        let content = "";
        if (!process.stdin.isTTY) {
            try {
                content = readFileSync("/dev/stdin", "utf-8");
            } catch {
                // stdin not available, proceed with empty content
            }
        }

        const tags = opts.tags ? opts.tags.split(",").map(t => t.trim()).filter(Boolean) : [];

        const serverUrl = getServerUrl();
        if (serverUrl) {
            // Server mode: POST to API
            const codebaseId = await resolveCodebaseId(serverUrl, {
                global: opts.global,
                codebase: opts.codebase,
            });

            const body: Record<string, unknown> = {
                title,
                content,
                type: opts.type,
                tags,
            };
            if (codebaseId) {
                body.codebaseIds = [codebaseId];
            }

            try {
                const res = await fetch(`${serverUrl}/api/chunks`, {
                    method: "POST",
                    headers: { "Content-Type": "application/json" },
                    body: JSON.stringify(body),
                });

                if (!res.ok) {
                    const text = await res.text();
                    outputError(`Server error (${res.status}): ${text}`);
                    return;
                }

                const chunk = (await res.json()) as { id: string; title: string; type: string };
                outputQuiet(cmd, chunk.id);
                output(
                    cmd,
                    chunk,
                    [
                        formatSuccess(`Created ${formatId(chunk.id)}`),
                        `  Title: ${title}`,
                        `  Type: ${formatType(opts.type)}`,
                        ...(tags.length > 0 ? [`  Tags: ${tags.map(formatTag).join(", ")}`] : []),
                        `  URL: ${serverUrl.replace(/:\d+$/, ":3001")}/chunks/${chunk.id}`,
                    ].join("\n")
                );
            } catch (err) {
                outputError(`Failed to connect to server: ${err instanceof Error ? err.message : err}`);
            }
        } else {
            // Local mode: use store
            const chunk = addChunk({ title, content, type: opts.type, tags });
            outputQuiet(cmd, chunk.id);
            output(
                cmd,
                chunk,
                [
                    formatSuccess(`Created chunk ${formatId(chunk.id)}`),
                    `  Title: ${chunk.title}`,
                    `  Type: ${formatType(chunk.type)}`,
                    ...(tags.length > 0 ? [`  Tags: ${tags.map(formatTag).join(", ")}`] : []),
                ].join("\n")
            );
        }
    });
