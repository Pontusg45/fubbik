import { readdirSync, readFileSync, statSync } from "node:fs";
import { join, resolve } from "node:path";

import { Command } from "commander";

import { formatBold, formatDim, formatSuccess } from "../lib/colors";
import { resolveCodebaseId } from "../lib/detect-codebase";
import { isJson, output, outputError, outputQuiet } from "../lib/output";
import { getServerUrl } from "../lib/store";

// ── Helpers ─────────────────────────────────────────────────────────

function requireServer(): string {
    const serverUrl = getServerUrl();
    if (!serverUrl) {
        console.error('No server URL configured. Run "fubbik init" first.');
        process.exit(1);
    }
    return serverUrl;
}

async function fetchApi(path: string, opts?: RequestInit): Promise<Response> {
    const serverUrl = requireServer();
    return fetch(`${serverUrl}/api${path}`, {
        ...opts,
        headers: {
            "Content-Type": "application/json",
            ...opts?.headers
        }
    });
}

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

interface Document {
    id: string;
    title: string;
    sourcePath: string | null;
    chunkCount?: number;
    createdAt: string;
    updatedAt: string;
}

// ── Subcommands ─────────────────────────────────────────────────────

const importDoc = new Command("import")
    .description("Import a single markdown file as a document")
    .argument("<path>", "path to .md file")
    .option("--codebase <name>", "target codebase name")
    .action(async (filePath: string, opts: { codebase?: string }, cmd: Command) => {
        try {
            const serverUrl = requireServer();
            const codebaseId = await resolveCodebaseId(serverUrl, opts);

            const absPath = resolve(filePath);
            const content = readFileSync(absPath, "utf-8");

            const body: Record<string, unknown> = {
                sourcePath: absPath,
                content
            };
            if (codebaseId) body.codebaseId = codebaseId;

            const res = await fetchApi("/documents/import", {
                method: "POST",
                body: JSON.stringify(body)
            });

            if (!res.ok) {
                outputError(`Failed to import document: ${res.status} ${await res.text()}`);
                process.exit(1);
            }

            const doc = (await res.json()) as Document;
            outputQuiet(cmd, doc.id);
            output(cmd, doc, formatSuccess(`Imported "${doc.title}" (${doc.id})`));
        } catch (err) {
            outputError(String(err));
            process.exit(1);
        }
    });

const importDir = new Command("import-dir")
    .description("Import a directory of markdown files as documents")
    .argument("<dir>", "path to directory")
    .option("--codebase <name>", "target codebase name")
    .action(async (dirPath: string, opts: { codebase?: string }, cmd: Command) => {
        try {
            const serverUrl = requireServer();
            const codebaseId = await resolveCodebaseId(serverUrl, opts);

            const absDir = resolve(dirPath);
            const stat = statSync(absDir);
            if (!stat.isDirectory()) {
                outputError(`Not a directory: ${absDir}`);
                process.exit(1);
            }

            const mdFiles = collectMarkdownFiles(absDir);
            if (mdFiles.length === 0) {
                outputError("No .md files found in directory.");
                process.exit(1);
            }

            const files = mdFiles.map(f => ({
                sourcePath: f,
                content: readFileSync(f, "utf-8")
            }));

            const body: Record<string, unknown> = { files };
            if (codebaseId) body.codebaseId = codebaseId;

            const res = await fetchApi("/documents/import-dir", {
                method: "POST",
                body: JSON.stringify(body)
            });

            if (!res.ok) {
                outputError(`Failed to import directory: ${res.status} ${await res.text()}`);
                process.exit(1);
            }

            const docs = (await res.json()) as Document[];
            if (isJson(cmd)) {
                console.log(JSON.stringify(docs, null, 2));
                return;
            }

            outputQuiet(cmd, docs.map(d => d.id).join("\n"));
            output(
                cmd,
                docs,
                formatSuccess(`Imported ${docs.length} document(s) from ${absDir}`)
            );
        } catch (err) {
            outputError(String(err));
            process.exit(1);
        }
    });

const listDocs = new Command("list")
    .description("List imported documents")
    .option("--codebase <name>", "filter by codebase name")
    .action(async (opts: { codebase?: string }, cmd: Command) => {
        try {
            const serverUrl = requireServer();
            const codebaseId = await resolveCodebaseId(serverUrl, opts);

            const params = new URLSearchParams();
            if (codebaseId) params.set("codebaseId", codebaseId);
            const qs = params.toString();

            const res = await fetchApi(`/documents${qs ? `?${qs}` : ""}`);
            if (!res.ok) {
                outputError(`Failed to list documents: ${res.status}`);
                process.exit(1);
            }

            const docs = (await res.json()) as Document[];

            if (isJson(cmd)) {
                console.log(JSON.stringify(docs, null, 2));
                return;
            }

            outputQuiet(cmd, docs.map(d => d.id).join("\n"));

            if (docs.length === 0) {
                output(cmd, docs, "No documents found.");
                return;
            }

            const lines: string[] = [];
            for (const doc of docs) {
                const chunks = doc.chunkCount != null ? `${doc.chunkCount} chunks` : "";
                lines.push(
                    `  ${formatBold(doc.title)} ${formatDim(`(${doc.id})`)}`,
                    `    ${formatDim(doc.sourcePath ?? "no source")}  ${formatDim(chunks)}`,
                    ""
                );
            }
            output(cmd, docs, lines.join("\n"));
        } catch (err) {
            outputError(String(err));
            process.exit(1);
        }
    });

const showDoc = new Command("show")
    .description("Show document details")
    .argument("<id>", "document ID")
    .action(async (id: string, _opts: Record<string, unknown>, cmd: Command) => {
        try {
            const res = await fetchApi(`/documents/${id}`);
            if (!res.ok) {
                outputError(`Failed to get document: ${res.status}`);
                process.exit(1);
            }

            const doc = (await res.json()) as Document & { chunks?: { id: string; title: string }[] };

            if (isJson(cmd)) {
                console.log(JSON.stringify(doc, null, 2));
                return;
            }

            const lines = [
                formatBold(doc.title),
                formatDim(`ID: ${doc.id}`),
                formatDim(`Source: ${doc.sourcePath ?? "none"}`),
                formatDim(`Updated: ${doc.updatedAt}`),
                ""
            ];

            if (doc.chunks && doc.chunks.length > 0) {
                lines.push(`Chunks (${doc.chunks.length}):`);
                for (const chunk of doc.chunks) {
                    lines.push(`  - ${chunk.title} ${formatDim(`(${chunk.id})`)}`);
                }
            }

            output(cmd, doc, lines.join("\n"));
        } catch (err) {
            outputError(String(err));
            process.exit(1);
        }
    });

const syncDoc = new Command("sync")
    .description("Re-import a document from its source file on disk")
    .argument("<id>", "document ID")
    .option("--codebase <name>", "target codebase name")
    .action(async (id: string, opts: { codebase?: string }, cmd: Command) => {
        try {
            const serverUrl = requireServer();
            const codebaseId = await resolveCodebaseId(serverUrl, opts);

            // First get the document to find its source path
            const getRes = await fetchApi(`/documents/${id}`);
            if (!getRes.ok) {
                outputError(`Failed to get document: ${getRes.status}`);
                process.exit(1);
            }

            const doc = (await getRes.json()) as Document;
            if (!doc.sourcePath) {
                outputError("Document has no source path — cannot sync from disk.");
                process.exit(1);
            }

            const content = readFileSync(doc.sourcePath, "utf-8");

            const body: Record<string, unknown> = { content };
            if (codebaseId) body.codebaseId = codebaseId;

            const res = await fetchApi(`/documents/${id}/sync`, {
                method: "POST",
                body: JSON.stringify(body)
            });

            if (!res.ok) {
                outputError(`Failed to sync document: ${res.status} ${await res.text()}`);
                process.exit(1);
            }

            const synced = (await res.json()) as Document;
            output(cmd, synced, formatSuccess(`Synced "${synced.title}" from ${doc.sourcePath}`));
        } catch (err) {
            outputError(String(err));
            process.exit(1);
        }
    });

const renderDoc = new Command("render")
    .description("Output reconstructed markdown for a document")
    .argument("<id>", "document ID")
    .action(async (id: string, _opts: Record<string, unknown>, cmd: Command) => {
        try {
            const res = await fetchApi(`/documents/${id}/render`);
            if (!res.ok) {
                outputError(`Failed to render document: ${res.status}`);
                process.exit(1);
            }

            const data = (await res.json()) as { markdown: string };

            if (isJson(cmd)) {
                console.log(JSON.stringify(data, null, 2));
                return;
            }

            console.log(data.markdown);
        } catch (err) {
            outputError(String(err));
            process.exit(1);
        }
    });

// ── Export ───────────────────────────────────────────────────────────

export const docsCommand = new Command("docs")
    .description("Manage imported documents")
    .addCommand(importDoc)
    .addCommand(importDir)
    .addCommand(listDocs)
    .addCommand(showDoc)
    .addCommand(syncDoc)
    .addCommand(renderDoc);
