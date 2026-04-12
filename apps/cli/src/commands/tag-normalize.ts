import { Command } from "commander";

import { formatBold, formatDim, formatError, formatSuccess } from "../lib/colors";
import { requireServer } from "../lib/api";
import { isJson, outputError } from "../lib/output";

interface TagIssue {
    action: "merge" | "remove";
    tag: string;
    mergeTo?: string;
    reason: string;
    affectedChunks: number;
}

// Known merge candidates: variants that should collapse to a canonical form
const MERGE_MAP: Record<string, string> = {
    documentation: "docs",
    document: "docs",
    documents: "docs",
    conventions: "convention",
    architectures: "architecture",
    configurations: "configuration",
    configs: "configuration",
    config: "configuration",
    tests: "testing",
    test: "testing",
    requirements: "requirement",
    implementations: "implementation",
    guides: "guide",
    templates: "template",
    features: "feature",
    migrations: "migration",
    runbooks: "runbook",
};

const FILENAME_PATTERN = /(\.\w{2,4}$|\/|^\d{4}-\d{2})/;

export const tagNormalizeCommand = new Command("normalize")
    .description("Identify tag issues and optionally fix them")
    .option("--dry-run", "show what would be changed (default)", true)
    .option("--confirm", "apply changes")
    .option("--codebase <name>", "scope to codebase")
    .action(async (opts, cmd) => {
        const serverUrl = requireServer();

        try {
            // Fetch all chunks to analyze tags
            const params = new URLSearchParams({ limit: "500" });
            if (opts.codebase) params.set("codebaseId", opts.codebase);
            const res = await fetch(`${serverUrl}/api/chunks?${params}`);
            if (!res.ok) {
                outputError(`API error: ${res.statusText}`);
                return;
            }
            const { chunks } = (await res.json()) as { chunks: any[] };

            // Build tag usage map: tag -> chunk IDs
            const tagUsage = new Map<string, string[]>();
            for (const chunk of chunks) {
                const tags: string[] = Array.isArray(chunk.tags)
                    ? chunk.tags.map((t: any) => (typeof t === "string" ? t : t.name ?? ""))
                    : [];
                for (const tag of tags) {
                    if (!tagUsage.has(tag)) tagUsage.set(tag, []);
                    tagUsage.get(tag)!.push(chunk.id);
                }
            }

            const issues: TagIssue[] = [];
            const totalChunks = chunks.length;
            const broadThreshold = Math.floor(totalChunks * 0.7);

            // 1. Merge candidates
            for (const [variant, canonical] of Object.entries(MERGE_MAP)) {
                if (tagUsage.has(variant) && variant !== canonical) {
                    issues.push({
                        action: "merge",
                        tag: variant,
                        mergeTo: canonical,
                        reason: "Variant of canonical tag",
                        affectedChunks: tagUsage.get(variant)!.length,
                    });
                }
            }

            // 2. Overly broad tags (> 70% of chunks)
            for (const [tag, chunkIds] of tagUsage) {
                if (chunkIds.length > broadThreshold) {
                    // Don't double-report if already flagged as merge candidate
                    if (!issues.some((i) => i.tag === tag)) {
                        issues.push({
                            action: "remove",
                            tag,
                            reason: "Too broad",
                            affectedChunks: chunkIds.length,
                        });
                    }
                }
            }

            // 3. Filename tags
            for (const [tag, chunkIds] of tagUsage) {
                if (FILENAME_PATTERN.test(tag)) {
                    if (!issues.some((i) => i.tag === tag)) {
                        issues.push({
                            action: "remove",
                            tag,
                            reason: "Filename tag",
                            affectedChunks: chunkIds.length,
                        });
                    }
                }
            }

            if (isJson(cmd)) {
                console.log(JSON.stringify({ issues, totalChunks }, null, 2));
                if (!opts.confirm) return;
            }

            if (issues.length === 0) {
                console.error(formatSuccess("No tag issues found"));
                return;
            }

            if (!opts.confirm) {
                console.error(formatBold("Tag normalization:"));
                for (const issue of issues) {
                    if (issue.action === "merge") {
                        console.error(
                            `  Merge: ${issue.tag} -> ${issue.mergeTo} (${issue.affectedChunks} chunks affected)`
                        );
                    } else {
                        console.error(
                            `  Remove: ${issue.tag} (${issue.affectedChunks} chunks, ${issue.reason.toLowerCase()})`
                        );
                    }
                }
                console.error(
                    formatDim("\nRun 'fubbik tag-normalize --confirm' to apply.")
                );
                return;
            }

            // Apply changes
            console.error(formatBold("Applying tag normalization...\n"));
            let applied = 0;
            let failed = 0;

            for (const issue of issues) {
                const chunkIds = tagUsage.get(issue.tag) ?? [];

                for (const chunkId of chunkIds) {
                    const chunk = chunks.find((c: any) => c.id === chunkId);
                    if (!chunk) continue;

                    const currentTags: string[] = Array.isArray(chunk.tags)
                        ? chunk.tags.map((t: any) => (typeof t === "string" ? t : t.name ?? ""))
                        : [];

                    let newTags: string[];
                    if (issue.action === "merge" && issue.mergeTo) {
                        // Replace variant with canonical, avoiding duplicates
                        newTags = currentTags
                            .map((t: string) => (t === issue.tag ? issue.mergeTo! : t))
                            .filter((t: string, i: number, arr: string[]) => arr.indexOf(t) === i);
                    } else {
                        // Remove the tag
                        newTags = currentTags.filter((t: string) => t !== issue.tag);
                    }

                    try {
                        const updateRes = await fetch(
                            `${serverUrl}/api/chunks/${chunkId}`,
                            {
                                method: "PATCH",
                                headers: { "Content-Type": "application/json" },
                                body: JSON.stringify({ tags: newTags }),
                            }
                        );
                        if (updateRes.ok) {
                            applied++;
                        } else {
                            failed++;
                        }
                    } catch {
                        failed++;
                    }
                }

                const verb = issue.action === "merge" ? "Merged" : "Removed";
                const detail =
                    issue.action === "merge"
                        ? `${issue.tag} -> ${issue.mergeTo}`
                        : issue.tag;
                console.error(
                    formatSuccess(`${verb}: ${detail} (${chunkIds.length} chunks)`)
                );
            }

            console.error(
                `\n${formatSuccess(`${applied} tag updates applied`)}${failed > 0 ? ` ${formatError(`${failed} failed`)}` : ""}`
            );
        } catch (e: any) {
            outputError(e.message);
        }
    });
