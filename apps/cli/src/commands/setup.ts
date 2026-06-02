import { Command } from "commander";

import { formatBold, formatDim, formatSuccess } from "../lib/colors";
import { loadConfig } from "../lib/config";
import { getGitRemoteUrl } from "../lib/detect-space";
import { isJson, output, outputError } from "../lib/output";
import { confirm, promptInput } from "../lib/prompt";
import { discover, formatPreview, formatPreviewJson, importToServer } from "../lib/setup";
import { getServerUrl } from "../lib/store";

export const setupCommand = new Command("setup")
    .description("Scan this project and populate your fubbik knowledge base")
    .option("--server <url>", "server URL (overrides config)")
    .option("--dry-run", "show preview without importing")
    .option("--yes", "skip confirmation prompt")
    .option("--force", "re-import even if chunks exist for this codebase")
    .action(async (opts: { server?: string; dryRun?: boolean; yes?: boolean; force?: boolean }, cmd: Command) => {
        const jsonMode = isJson(cmd);

        // --- Phase 0: Preflight ---
        const serverUrl = opts.server ?? loadConfig().serverUrl ?? getServerUrl();
        if (!serverUrl) {
            outputError("No server URL configured. Set it in fubbik.config.json or pass --server <url>.");
            outputError("Is the fubbik server running? Start it with: pnpm dev");
            process.exit(1);
        }

        // Test connectivity
        try {
            const res = await fetch(`${serverUrl}/api/health`);
            if (!res.ok) {
                outputError(`Server at ${serverUrl} returned ${res.status}. Is it running?`);
                process.exit(1);
            }
        } catch {
            outputError(`Cannot connect to server at ${serverUrl}. Is it running?`);
            process.exit(1);
        }

        // Detect or create space
        const remoteUrl = getGitRemoteUrl();
        const localPath = process.cwd();
        let spaceId: string | null = null;
        let spaceName: string = "";

        const detectParams = new URLSearchParams();
        if (remoteUrl) detectParams.set("remoteUrl", remoteUrl);
        else detectParams.set("localPath", localPath);

        try {
            const res = await fetch(`${serverUrl}/api/spaces/detect?${detectParams}`);
            if (res.ok) {
                const data = (await res.json()) as { id?: string; name?: string };
                if (data?.id) {
                    spaceId = data.id;
                    spaceName = data.name!;
                }
            }
        } catch {
            // Will create below
        }

        if (!spaceId) {
            if (!remoteUrl) {
                spaceName = await promptInput("No git remote detected. Space name", localPath.split("/").pop() ?? "my-project");
            } else {
                const match = remoteUrl.match(/\/([^/]+?)(?:\.git)?$/);
                spaceName = match?.[1] ?? "my-project";
            }

            try {
                const res = await fetch(`${serverUrl}/api/spaces`, {
                    method: "POST",
                    headers: { "Content-Type": "application/json" },
                    body: JSON.stringify({
                        name: spaceName,
                        remoteUrl: remoteUrl ?? undefined,
                        localPaths: [localPath],
                    }),
                });
                if (res.ok) {
                    const data = (await res.json()) as { id: string };
                    spaceId = data.id;
                } else {
                    outputError(`Failed to create space: ${res.status} ${await res.text()}`);
                    process.exit(1);
                }
            } catch (err) {
                outputError(`Failed to create space: ${err}`);
                process.exit(1);
            }
        }

        if (!jsonMode) {
            console.log(`\nDetected space: ${formatBold(spaceName)}${remoteUrl ? ` (${formatDim(remoteUrl)})` : ""}\n`);
        }

        // Check if space already has chunks
        if (!opts.force) {
            try {
                const res = await fetch(`${serverUrl}/api/chunks?spaceId=${spaceId}&limit=1`);
                if (res.ok) {
                    const data = (await res.json()) as { total: number };
                    if (data.total > 0) {
                        if (!jsonMode) {
                            console.log(`This space already has ${data.total} chunks.`);
                        }
                        if (!opts.yes) {
                            const proceed = await confirm("Continue and add more?");
                            if (!proceed) {
                                console.log("Aborted. Use --force to re-import.");
                                process.exit(0);
                            }
                        }
                    }
                }
            } catch {
                // Non-fatal
            }
        }

        // --- Phase 1: Discover ---
        if (!jsonMode) console.log("Scanning project...");

        const result = discover(localPath, {
            name: spaceName,
            remoteUrl,
            localPath,
        });

        if (result.chunks.length === 0) {
            if (jsonMode) {
                output(cmd, { chunks: 0, message: "No knowledge sources found" }, "");
            } else {
                console.log("\nNo knowledge sources found in this project.");
            }
            process.exit(0);
        }

        if (!jsonMode) {
            const tierCounts = [1, 2, 3].map(t => result.chunks.filter(c => c.tier === t).length);
            console.log(formatSuccess(`${tierCounts[0]} markdown docs found`));
            console.log(formatSuccess(`${tierCounts[1]} config files analyzed`));
            console.log(formatSuccess(`${tierCounts[2]} code patterns detected`));
        }

        // --- Phase 2: Preview ---
        if (jsonMode) {
            const previewData = formatPreviewJson(result.chunks, result.connections, result.tags);
            if (opts.dryRun) {
                output(cmd, previewData, "");
                process.exit(0);
            }
        } else {
            console.log(formatPreview(result.chunks, result.connections, result.tags));
            if (opts.dryRun) {
                console.log("Dry run complete. No changes made.");
                process.exit(0);
            }
        }

        // --- Phase 3: Confirm ---
        if (!opts.yes && !jsonMode) {
            const proceed = await confirm(`Import ${result.chunks.length} chunks to fubbik?`);
            if (!proceed) {
                console.log("Aborted.");
                process.exit(0);
            }
        }

        // --- Phase 4: Import ---
        if (!jsonMode) console.log("");

        const importResult = await importToServer(
            serverUrl,
            spaceId!,
            result.chunks,
            result.connections,
            localPath,
            jsonMode ? undefined : (msg) => console.log(`  ${formatDim(msg)}`),
        );

        if (!jsonMode) {
            console.log("");
            console.log(formatSuccess(`${importResult.chunksCreated} chunks created`));
            if (importResult.connectionsCreated > 0) {
                console.log(formatSuccess(`${importResult.connectionsCreated} connections established`));
            }
            if (importResult.errors.length > 0) {
                console.log(`\n  ${importResult.errors.length} errors:`);
                for (const err of importResult.errors.slice(0, 5)) {
                    console.log(`    ${err.item}: ${err.error}`);
                }
                if (importResult.errors.length > 5) {
                    console.log(`    ... and ${importResult.errors.length - 5} more`);
                }
            }
        }

        // --- Phase 5: Tips ---
        if (result.tips.length > 0 && !jsonMode) {
            console.log(`\n${formatBold("You might also want to add:")}`);
            for (const tip of result.tips) {
                console.log(`  ${formatDim("•")} ${tip.title} — ${formatDim(tip.detail)}`);
            }
        }

        const webUrl = serverUrl.replace(":3000", ":3001");
        if (!jsonMode) {
            console.log(`\n  View your knowledge graph: ${formatDim(webUrl + "/graph")}\n`);
        }

        if (jsonMode) {
            output(cmd, {
                chunksCreated: importResult.chunksCreated,
                connectionsCreated: importResult.connectionsCreated,
                errors: importResult.errors,
                tips: result.tips,
            }, "");
        }
    });
