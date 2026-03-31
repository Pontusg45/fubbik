import { Command } from "commander";

import { formatBold, formatDim, formatError, formatSuccess } from "../lib/colors";
import { isJson, outputError } from "../lib/output";
import { getServerUrl } from "../lib/store";

interface CleanupCandidate {
    id: string;
    title: string;
    type: string;
    reason: string;
    category: "plan-artifact" | "near-empty" | "duplicate-title";
}

function requireServer(): string {
    const serverUrl = getServerUrl();
    if (!serverUrl) {
        console.error('No server URL configured. Run "fubbik init" first.');
        process.exit(1);
    }
    return serverUrl;
}

export const cleanupCommand = new Command("cleanup")
    .description("Identify and optionally remove low-value chunks")
    .option("--dry-run", "show what would be removed (default)", true)
    .option("--confirm", "actually remove flagged chunks")
    .option("--type <type>", "filter to specific chunk type")
    .option("--codebase <name>", "scope to codebase")
    .action(async (opts, cmd) => {
        const serverUrl = requireServer();

        try {
            const params = new URLSearchParams({ limit: "500" });
            if (opts.codebase) params.set("codebaseId", opts.codebase);
            const res = await fetch(`${serverUrl}/api/chunks?${params}`);
            if (!res.ok) {
                outputError(`API error: ${res.statusText}`);
                return;
            }
            const { chunks } = (await res.json()) as { chunks: any[] };

            const candidates: CleanupCandidate[] = [];

            // Filter by type if specified
            const filtered = opts.type
                ? chunks.filter((c: any) => c.type === opts.type)
                : chunks;

            // 1. Plan task artifacts: guide chunks with tags that look like plan filenames
            const planFilePattern = /\d{4}-.*\.md/;
            for (const chunk of filtered) {
                const tags: string[] = Array.isArray(chunk.tags)
                    ? chunk.tags.map((t: any) => (typeof t === "string" ? t : t.name ?? ""))
                    : [];

                if (
                    chunk.type === "guide" &&
                    tags.some((tag: string) => planFilePattern.test(tag) && tag.includes(".md"))
                ) {
                    candidates.push({
                        id: chunk.id,
                        title: chunk.title,
                        type: chunk.type,
                        reason: "Plan task artifact (type: guide, tagged with plan filename)",
                        category: "plan-artifact",
                    });
                    continue; // Don't double-count
                }

                // 2. Near-empty chunks
                if (chunk.content.length < 50) {
                    candidates.push({
                        id: chunk.id,
                        title: chunk.title,
                        type: chunk.type,
                        reason: `Near-empty (${chunk.content.length} chars)`,
                        category: "near-empty",
                    });
                }
            }

            // 3. Duplicate titles
            const titleMap = new Map<string, any[]>();
            for (const chunk of filtered) {
                const key = chunk.title.toLowerCase().trim();
                if (!titleMap.has(key)) titleMap.set(key, []);
                titleMap.get(key)!.push(chunk);
            }
            for (const [, dupes] of titleMap) {
                if (dupes.length > 1) {
                    // Keep the first (oldest), flag the rest
                    const sorted = dupes.sort(
                        (a: any, b: any) =>
                            new Date(a.createdAt).getTime() - new Date(b.createdAt).getTime()
                    );
                    for (let i = 1; i < sorted.length; i++) {
                        const chunk = sorted[i];
                        // Avoid double-counting if already flagged
                        if (!candidates.some((c) => c.id === chunk.id)) {
                            candidates.push({
                                id: chunk.id,
                                title: chunk.title,
                                type: chunk.type,
                                reason: `Duplicate title (${dupes.length} copies)`,
                                category: "duplicate-title",
                            });
                        }
                    }
                }
            }

            // Count by category
            const planArtifacts = candidates.filter((c) => c.category === "plan-artifact");
            const nearEmpty = candidates.filter((c) => c.category === "near-empty");
            const duplicates = candidates.filter((c) => c.category === "duplicate-title");

            if (isJson(cmd)) {
                console.log(
                    JSON.stringify(
                        {
                            candidates,
                            summary: {
                                planArtifacts: planArtifacts.length,
                                nearEmpty: nearEmpty.length,
                                duplicateTitles: duplicates.length,
                                total: candidates.length,
                            },
                        },
                        null,
                        2
                    )
                );
                if (!opts.confirm) return;
            }

            if (candidates.length === 0) {
                console.error(formatSuccess("No low-value chunks found"));
                return;
            }

            if (!opts.confirm) {
                // Dry-run output
                console.error(formatBold("Cleanup analysis:"));
                if (planArtifacts.length > 0) {
                    console.error(
                        `  ${planArtifacts.length} plan task artifacts (type: guide, tagged with plan filenames)`
                    );
                }
                if (nearEmpty.length > 0) {
                    console.error(
                        `  ${nearEmpty.length} near-empty chunks (< 50 chars)`
                    );
                }
                if (duplicates.length > 0) {
                    console.error(
                        `  ${duplicates.length} duplicate titles`
                    );
                }
                console.error(
                    `\nTotal: ${candidates.length} chunks flagged`
                );
                console.error(
                    formatDim("Run 'fubbik cleanup --confirm' to remove.")
                );
                return;
            }

            // Actually remove
            console.error(formatBold("Removing flagged chunks...\n"));
            let removed = 0;
            let failed = 0;

            for (const candidate of candidates) {
                try {
                    const delRes = await fetch(
                        `${serverUrl}/api/chunks/${candidate.id}`,
                        { method: "DELETE" }
                    );
                    if (delRes.ok) {
                        console.error(
                            formatSuccess(`Removed: ${candidate.title} ${formatDim(`(${candidate.reason})`)}`)
                        );
                        removed++;
                    } else {
                        console.error(
                            formatError(`Failed: ${candidate.title} (${delRes.statusText})`)
                        );
                        failed++;
                    }
                } catch {
                    console.error(
                        formatError(`Failed: ${candidate.title} (network error)`)
                    );
                    failed++;
                }
            }

            console.error(
                `\n${formatSuccess(`${removed} removed`)}${failed > 0 ? ` ${formatError(`${failed} failed`)}` : ""}`
            );
        } catch (e: any) {
            outputError(e.message);
        }
    });
