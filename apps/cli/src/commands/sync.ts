import { Command } from "commander";

import { output, outputError } from "../lib/output";
import { readStore, getServerUrl, setServerUrl, updateLastSync, addChunk } from "../lib/store";

export const syncCommand = new Command("sync")
    .description("Sync local chunks with a fubbik server")
    .option("-u, --url <url>", "Server URL (e.g., http://localhost:3000)")
    .option("--push", "Only push local chunks to server")
    .option("--pull", "Only pull server chunks to local")
    .option("--token <token>", "Auth token for the server")
    .action(async (options: { url?: string; push?: boolean; pull?: boolean; token?: string }, cmd: Command) => {
        try {
            const store = readStore();

            // Resolve server URL
            const serverUrl = options.url ?? getServerUrl();
            if (!serverUrl) {
                outputError("No server URL configured. Use --url or run sync once with --url to save it.");
                process.exit(1);
            }

            // Save URL for future use
            if (options.url) {
                setServerUrl(options.url);
            }

            const headers: Record<string, string> = {
                "Content-Type": "application/json"
            };
            if (options.token) {
                headers["Authorization"] = `Bearer ${options.token}`;
            }

            // Fetch server chunks
            console.log(`Connecting to ${serverUrl}...`);
            const response = await fetch(`${serverUrl}/api/chunks?limit=1000`, { headers });
            if (!response.ok) {
                outputError(`Server returned ${response.status}: ${await response.text()}`);
                process.exit(1);
            }
            const serverData = (await response.json()) as {
                chunks: Array<{
                    id: string;
                    title: string;
                    content: string;
                    type: string;
                    tags: string[];
                    createdAt: string;
                    updatedAt: string;
                }>;
            };
            const serverChunks = serverData.chunks;

            console.log(`Local: ${store.chunks.length} chunks, Server: ${serverChunks.length} chunks`);

            // Build maps by title for comparison
            const localByTitle = new Map(store.chunks.map(c => [c.title, c]));
            const serverByTitle = new Map(serverChunks.map(c => [c.title, c]));

            let pushed = 0;
            let pulled = 0;

            // Push: local chunks not on server
            if (!options.pull) {
                const localOnly = store.chunks.filter(c => !serverByTitle.has(c.title));
                if (localOnly.length > 0) {
                    console.log(`Pushing ${localOnly.length} local-only chunks to server...`);
                    const importResponse = await fetch(`${serverUrl}/api/chunks/import`, {
                        method: "POST",
                        headers,
                        body: JSON.stringify({
                            chunks: localOnly.map(c => ({
                                title: c.title,
                                content: c.content,
                                type: c.type,
                                tags: c.tags
                            }))
                        })
                    });
                    if (!importResponse.ok) {
                        outputError(`Failed to push chunks: ${importResponse.status}`);
                    } else {
                        pushed = localOnly.length;
                        console.log(`Pushed ${pushed} chunks`);
                    }
                }
            }

            // Pull: server chunks not in local
            if (!options.push) {
                const serverOnly = serverChunks.filter(c => !localByTitle.has(c.title));
                if (serverOnly.length > 0) {
                    console.log(`Pulling ${serverOnly.length} server-only chunks to local...`);
                    for (const sc of serverOnly) {
                        addChunk({
                            title: sc.title,
                            content: sc.content,
                            type: sc.type,
                            tags: sc.tags
                        });
                        pulled++;
                    }
                    console.log(`Pulled ${pulled} chunks`);
                }
            }

            updateLastSync();
            const summary = { pushed, pulled, localCount: store.chunks.length, serverCount: serverChunks.length };
            output(cmd, summary, `Sync complete. Pushed: ${pushed}, Pulled: ${pulled}`);
        } catch (err) {
            outputError("Sync failed: " + (err as Error).message);
            process.exit(1);
        }
    });
