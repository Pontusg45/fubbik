import { Command } from "commander";

import { scanProject } from "../lib/scanner";
import { output, outputError, outputQuiet } from "../lib/output";
import { addChunk, createStore, getServerUrl, readStore, storeDir, storeExists } from "../lib/store";

export const initCommand = new Command("init")
    .description("Initialize a new knowledge base")
    .argument("[name]", "name for the knowledge base", "my-knowledge-base")
    .option("-f, --force", "overwrite existing knowledge base")
    .option("--scan", "scan the current project and auto-generate chunks")
    .option("--push", "push scanned chunks to the server (requires --scan and a configured server URL)")
    .option("--dry-run", "show what --scan would generate without writing anything")
    .action(async (name: string, opts: { force?: boolean; scan?: boolean; push?: boolean; dryRun?: boolean }, cmd: Command) => {
        // Initialize store if needed
        if (!opts.dryRun) {
            if (storeExists() && !opts.force && !opts.scan) {
                outputError("Knowledge base already exists. Use --force to overwrite, or --scan to add project chunks.");
                process.exit(1);
            }

            if (!storeExists() || opts.force) {
                createStore(name);
                console.log(`Initialized knowledge base: ${name}`);
                console.log(`  Location: ${storeDir()}`);
            }
        }

        if (!opts.scan && !opts.dryRun) {
            const store = readStore();
            outputQuiet(cmd, store.name);
            output(cmd, { name: store.name, location: storeDir() }, "");
            return;
        }

        // Scan the project
        console.log("\nScanning project...");
        const chunks = scanProject({ dir: process.cwd() });
        console.log(`Found ${chunks.length} chunks:\n`);

        for (const c of chunks) {
            const contentPreview = c.content.length > 80 ? c.content.slice(0, 80) + "..." : c.content;
            console.log(`  [${c.type}] ${c.title}`);
            console.log(`    Tags: ${c.tags.join(", ")}`);
            console.log(`    ${contentPreview.split("\n")[0]}`);
            console.log();
        }

        if (opts.dryRun) {
            console.log(`Dry run complete. ${chunks.length} chunks would be created.`);
            output(cmd, chunks, "");
            return;
        }

        // Add chunks locally
        const added: string[] = [];
        for (const c of chunks) {
            const chunk = addChunk(c);
            added.push(chunk.id);
        }
        console.log(`Added ${added.length} chunks to local store.`);

        // Optionally push to server
        if (opts.push) {
            const serverUrl = getServerUrl();
            if (!serverUrl) {
                outputError("No server URL configured. Run 'fubbik sync --url <url>' first.");
            } else {
                console.log(`\nPushing ${chunks.length} chunks to ${serverUrl}...`);
                const res = await fetch(`${serverUrl}/api/chunks/import`, {
                    method: "POST",
                    headers: { "Content-Type": "application/json" },
                    body: JSON.stringify({ chunks: chunks.map(c => ({ title: c.title, content: c.content, type: c.type, tags: c.tags })) })
                });
                if (!res.ok) {
                    outputError(`Failed to push: ${res.status}`);
                } else {
                    const data = (await res.json()) as { imported: number };
                    console.log(`Pushed ${data.imported} chunks to server.`);
                }
            }
        }

        output(cmd, { chunksCreated: added.length, chunkIds: added }, `\nScan complete. ${added.length} chunks created.`);
    });
