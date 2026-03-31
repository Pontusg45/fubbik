import { readdirSync, readFileSync, statSync } from "node:fs";
import { basename, join, relative, resolve } from "node:path";

import { Command } from "commander";

import { formatSuccess } from "../lib/colors";
import { resolveCodebaseId } from "../lib/detect-codebase";
import { output, outputError, outputQuiet } from "../lib/output";
import { addChunk, getServerUrl } from "../lib/store";

function collectMarkdownFiles(dir: string, recursive: boolean): string[] {
    const files: string[] = [];
    const entries = readdirSync(dir, { withFileTypes: true });
    for (const entry of entries) {
        const fullPath = join(dir, entry.name);
        if (entry.isDirectory() && recursive) {
            files.push(...collectMarkdownFiles(fullPath, true));
        } else if (entry.isFile() && entry.name.endsWith(".md")) {
            files.push(fullPath);
        }
    }
    return files;
}

function titleFromFilename(filename: string): string {
    return basename(filename, ".md").replace(/[-_]/g, " ");
}

function tagsFromPath(filePath: string, baseDir: string): string[] {
    const rel = relative(baseDir, filePath);
    const parts = rel.split("/");
    parts.pop();
    return parts.filter(Boolean);
}

function parseFrontmatter(content: string): {
    meta: Record<string, any>;
    body: string;
} {
    const match = content.match(/^---\n([\s\S]*?)\n---\n?([\s\S]*)$/);
    if (!match) return { meta: {}, body: content };

    const meta: Record<string, any> = {};
    let currentKey: string | null = null;
    let currentArray: string[] | null = null;

    for (const line of (match[1] ?? "").split("\n")) {
        const arrMatch = line.match(/^\s+-\s+(.+)$/);
        if (arrMatch && currentKey) {
            if (!currentArray) currentArray = [];
            currentArray.push(arrMatch[1]!.trim());
            continue;
        }
        if (currentKey && currentArray) {
            meta[currentKey] = currentArray;
            currentArray = null;
        }
        const kvMatch = line.match(/^(\w+):\s*(.*)$/);
        if (kvMatch) {
            currentKey = kvMatch[1]!;
            const val = kvMatch[2]!.trim();
            if (val) {
                meta[currentKey] = val;
                currentKey = null;
            }
        }
    }
    if (currentKey && currentArray) meta[currentKey] = currentArray;

    // Strip leading "# Title" line from body
    let body = match[2] ?? "";
    const titleLine = body.match(/^# .+\n\n?/);
    if (titleLine) body = body.slice(titleLine[0].length);

    return { meta, body: body.trim() };
}

export const importCommand = new Command("import")
    .description("Import chunks from a JSON file, markdown file, or directory")
    .argument("<path>", "path to JSON file, .md file, or directory of .md files")
    .option(
        "--server",
        "send to server (required for frontmatter parsing and codebase scoping)"
    )
    .option("--codebase <name>", "codebase name (implies --server)")
    .option("--type <type>", "default chunk type", "document")
    .option("--no-recursive", "do not recurse into subdirectories")
    .action(
        async (
            inputPath: string,
            opts: {
                server?: boolean;
                codebase?: string;
                type: string;
                recursive: boolean;
            },
            cmd: Command
        ) => {
            const useServer = opts.server || !!opts.codebase;

            let stat: ReturnType<typeof statSync>;
            try {
                stat = statSync(inputPath);
            } catch {
                outputError(`Path not found: ${inputPath}`);
                process.exit(1);
            }

            // JSON file
            if (stat.isFile() && inputPath.endsWith(".json")) {
                const raw = readFileSync(inputPath, "utf-8");
                const parsed = JSON.parse(raw);
                const chunks = Array.isArray(parsed) ? parsed : parsed.chunks;
                if (!Array.isArray(chunks)) {
                    outputError("Invalid JSON format");
                    process.exit(1);
                }

                if (useServer) {
                    const serverUrl = getServerUrl();
                    if (!serverUrl) {
                        outputError(
                            "No server URL configured. Run 'fubbik init' first."
                        );
                        process.exit(1);
                    }
                    const res = await fetch(`${serverUrl}/api/chunks/import`, {
                        method: "POST",
                        headers: { "Content-Type": "application/json" },
                        body: JSON.stringify({ chunks })
                    });
                    if (!res.ok) {
                        const text = await res.text();
                        outputError(`Server error (${res.status}): ${text}`);
                        process.exit(1);
                    }
                    const data = (await res.json()) as { imported: number };
                    output(
                        cmd,
                        data,
                        formatSuccess(
                            `Imported ${data.imported} chunks to server`
                        )
                    );
                } else {
                    const added = chunks.map((c: any) =>
                        addChunk({
                            title: c.title,
                            content: c.content ?? "",
                            type: c.type ?? opts.type,
                            tags: c.tags ?? []
                        })
                    );
                    outputQuiet(cmd, added.map(a => a.id).join("\n"));
                    output(
                        cmd,
                        { added: added.length },
                        formatSuccess(`Imported ${added.length} chunks locally`)
                    );
                }
                return;
            }

            // Markdown files (single file or directory)
            let mdFiles: string[];
            let baseDir: string;

            if (stat.isFile() && inputPath.endsWith(".md")) {
                mdFiles = [resolve(inputPath)];
                baseDir = resolve(inputPath, "..");
            } else if (stat.isDirectory()) {
                baseDir = resolve(inputPath);
                mdFiles = collectMarkdownFiles(baseDir, opts.recursive);
            } else {
                outputError(
                    "Path must be a .json file, .md file, or directory."
                );
                process.exit(1);
            }

            if (mdFiles.length === 0) {
                outputError("No .md files found.");
                process.exit(1);
            }
            if (mdFiles.length > 500) {
                outputError(
                    `Found ${mdFiles.length} files, max is 500. Import in smaller batches.`
                );
                process.exit(1);
            }

            if (useServer) {
                // Server mode: send raw files for server-side frontmatter parsing
                const serverUrl = getServerUrl();
                if (!serverUrl) {
                    outputError(
                        "No server URL configured. Run 'fubbik init' first."
                    );
                    process.exit(1);
                }
                const codebaseId = await resolveCodebaseId(serverUrl, {
                    codebase: opts.codebase
                });
                if (!codebaseId) {
                    outputError("Could not resolve codebase.");
                    process.exit(1);
                }

                const files = mdFiles.map(f => ({
                    path: relative(baseDir, f),
                    content: readFileSync(f, "utf-8")
                }));

                try {
                    const res = await fetch(
                        `${serverUrl}/api/chunks/import-docs`,
                        {
                            method: "POST",
                            headers: { "Content-Type": "application/json" },
                            body: JSON.stringify({ files, codebaseId })
                        }
                    );

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
                        formatSuccess(
                            `Created: ${data.created} | Skipped: ${data.skipped} | Errors: ${data.errors.length}`
                        )
                    );
                } catch (err) {
                    outputError(`Failed to connect to server: ${err}`);
                    process.exit(1);
                }
            } else {
                // Local mode: basic frontmatter extraction
                const added: { id: string; title: string }[] = [];
                for (const filePath of mdFiles) {
                    const raw = readFileSync(filePath, "utf-8");
                    const { meta, body } = parseFrontmatter(raw);
                    const title =
                        (meta.title as string)?.replace(/^"|"$/g, "") ??
                        titleFromFilename(filePath);
                    const fmTags = Array.isArray(meta.tags) ? meta.tags : [];
                    const folderTags = tagsFromPath(filePath, baseDir);
                    const tags = [...new Set([...fmTags, ...folderTags])];
                    const type = (meta.type as string) ?? opts.type;
                    const chunk = addChunk({
                        title,
                        content: body,
                        type,
                        tags
                    });
                    added.push({ id: chunk.id, title: chunk.title });
                }

                outputQuiet(cmd, added.map(a => a.id).join("\n"));
                output(
                    cmd,
                    { added: added.length, files: mdFiles.length },
                    formatSuccess(
                        `Imported ${added.length} chunks from ${mdFiles.length} files`
                    )
                );
            }
        }
    );
