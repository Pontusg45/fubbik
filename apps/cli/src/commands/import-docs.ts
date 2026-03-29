import { readdirSync, readFileSync, statSync } from "node:fs";
import { join, relative } from "node:path";

import { Command } from "commander";

import { resolveCodebaseId } from "../lib/detect-codebase";
import { output, outputError, outputQuiet } from "../lib/output";
import { getServerUrl } from "../lib/store";

function collectMarkdownFiles(dir: string): string[] {
    const files: string[] = [];
    const entries = readdirSync(dir, { withFileTypes: true });
    for (const entry of entries) {
        const fullPath = join(dir, entry.name);
        if (entry.isDirectory()) {
            files.push(...collectMarkdownFiles(fullPath));
        } else if (entry.isFile() && entry.name.endsWith(".md")) {
            files.push(fullPath);
        }
    }
    return files;
}

export const importDocsCommand = new Command("import-docs")
    .description("Import a folder of markdown documents as chunks")
    .argument("<path>", "path to directory containing .md files")
    .requiredOption("--codebase <name>", "codebase name (required)")
    .action(async (dirPath: string, opts: { codebase: string }, cmd: Command) => {
        const stat = statSync(dirPath);
        if (!stat.isDirectory()) {
            outputError(`${dirPath} is not a directory`);
            process.exit(1);
        }

        const serverUrl = getServerUrl();
        if (!serverUrl) {
            outputError("No server URL configured. Run 'fubbik init' first.");
            process.exit(1);
        }

        const codebaseId = await resolveCodebaseId(serverUrl, { codebase: opts.codebase });
        if (!codebaseId) {
            outputError(`Could not resolve codebase "${opts.codebase}".`);
            process.exit(1);
        }

        const mdFiles = collectMarkdownFiles(dirPath);
        if (mdFiles.length === 0) {
            outputError("No .md files found in the directory.");
            process.exit(1);
        }

        if (mdFiles.length > 500) {
            outputError(`Found ${mdFiles.length} files, max is 500. Import in smaller batches.`);
            process.exit(1);
        }

        const files = mdFiles.map(fullPath => ({
            path: relative(dirPath, fullPath),
            content: readFileSync(fullPath, "utf-8")
        }));

        try {
            const res = await fetch(`${serverUrl}/api/chunks/import-docs`, {
                method: "POST",
                headers: { "Content-Type": "application/json" },
                body: JSON.stringify({ files, codebaseId })
            });

            if (!res.ok) {
                const text = await res.text();
                outputError(`Server error (${res.status}): ${text}`);
                process.exit(1);
            }

            const data = (await res.json()) as {
                created: number;
                skipped: number;
                errors: { path: string; error: string }[];
            };

            if (data.errors.length > 0) {
                for (const err of data.errors) {
                    console.error(`  Error: ${err.path} — ${err.error}`);
                }
            }

            outputQuiet(cmd, String(data.created));
            output(
                cmd,
                data,
                `Created: ${data.created} | Skipped: ${data.skipped} | Errors: ${data.errors.length}`
            );
        } catch (err) {
            outputError(`Failed to connect to server: ${err}`);
            process.exit(1);
        }
    });
