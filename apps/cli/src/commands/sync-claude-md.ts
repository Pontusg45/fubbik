import { mkdirSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";

import { Command } from "commander";

import { formatSuccess } from "../lib/colors";
import { resolveCodebaseId } from "../lib/detect-codebase";
import { output, outputError, outputQuiet } from "../lib/output";
import { getServerUrl } from "../lib/store";

export const syncClaudeMdCommand = new Command("sync-claude-md")
    .description("Generate a CLAUDE.md file from chunks tagged with a specific tag")
    .option("--tag <tag>", "tag to filter chunks by", "claude-context")
    .option("--output <path>", "output file path", ".claude/CLAUDE.md")
    .option("--codebase <name>", "scope to a specific codebase")
    .option("--global", "only include global (unscoped) chunks")
    .option("--watch", "watch for changes and regenerate periodically")
    .option("--interval <seconds>", "polling interval in seconds for watch mode", "30")
    .action(
        async (
            opts: {
                tag: string;
                output: string;
                codebase?: string;
                global?: boolean;
                watch?: boolean;
                interval: string;
            },
            cmd: Command
        ) => {
            let serverUrl: string | undefined;
            try {
                serverUrl = getServerUrl();
            } catch {
                outputError('No server URL configured. Run "fubbik init" first.');
                process.exit(1);
            }

            if (!serverUrl) {
                outputError('No server URL configured. Run "fubbik init" first.');
                process.exit(1);
            }

            const codebaseId = await resolveCodebaseId(serverUrl, {
                global: opts.global,
                codebase: opts.codebase
            });

            const generate = async () => {
                const params = new URLSearchParams();
                params.set("tag", opts.tag);
                if (codebaseId) {
                    params.set("codebaseId", codebaseId);
                }

                const res = await fetch(`${serverUrl}/api/chunks/export/claude-md?${params.toString()}`);
                if (!res.ok) {
                    const text = await res.text();
                    outputError(`Failed to generate CLAUDE.md: ${res.status} ${text}`);
                    return false;
                }

                const data = (await res.json()) as { content: string; chunks: number };

                const outputPath = resolve(opts.output);
                mkdirSync(dirname(outputPath), { recursive: true });
                writeFileSync(outputPath, data.content, "utf-8");

                outputQuiet(cmd, outputPath);
                output(
                    cmd,
                    { path: outputPath, chunks: data.chunks },
                    formatSuccess(`Wrote ${data.chunks} chunk(s) to ${outputPath}`)
                );
                return true;
            };

            const ok = await generate();
            if (!ok && !opts.watch) {
                process.exit(1);
            }

            if (opts.watch) {
                const intervalMs = Number(opts.interval) * 1000;
                output(cmd, {}, `Watching for changes every ${opts.interval}s... (Ctrl+C to stop)`);
                setInterval(async () => {
                    await generate();
                }, intervalMs);
            }
        }
    );
