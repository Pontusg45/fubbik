import { Command } from "commander";

import { formatBold, formatDim, formatError, formatSuccess } from "../lib/colors";
import { requireServer } from "../lib/api";
import { isJson, outputError } from "../lib/output";

interface LintIssue {
    chunkId: string;
    chunkTitle: string;
    severity: "error" | "warning";
    rule: string;
    message: string;
}

const GENERIC_TAGS = new Set([
    "docs",
    "documentation",
    "document",
    "plans",
    "plan",
    "notes",
    "note",
    "general",
    "misc",
    "other",
]);

function computeQualityScore(chunk: any): number {
    let score = 0;

    // Content > 100 chars: +20
    if (chunk.content && chunk.content.length > 100) score += 20;

    // Has rationale: +20
    if (chunk.rationale) score += 20;

    // Has connections (> 0): +15
    const connections = chunk.connections ?? chunk.sourceConnections ?? [];
    const targetConnections = chunk.targetConnections ?? [];
    if (connections.length > 0 || targetConnections.length > 0) score += 15;

    // Has appliesTo patterns: +15
    const appliesTo = chunk.appliesTo ?? [];
    if (appliesTo.length > 0) score += 15;

    // Has meaningful tags: +15
    const tags: string[] = Array.isArray(chunk.tags)
        ? chunk.tags.map((t: any) => (typeof t === "string" ? t : t.name ?? ""))
        : [];
    const meaningfulTags = tags.filter((t: string) => !GENERIC_TAGS.has(t.toLowerCase()));
    if (meaningfulTags.length > 0) score += 15;

    // Has type other than "note" or "guide": +5
    if (chunk.type && chunk.type !== "note" && chunk.type !== "guide") score += 5;

    // Has AI summary: +10
    if (chunk.summary) score += 10;

    return score;
}

export const lintCommand = new Command("lint")
    .description("Check all chunks for quality issues")
    .option("--codebase <name>", "scope to codebase")
    .option("--fix", "auto-fix safe issues (e.g. enrich unenriched chunks)")
    .option("--score", "compute and display quality scores per chunk")
    .action(async (opts, cmd) => {
        const serverUrl = requireServer();

        try {
            // Fetch all chunks
            const params = new URLSearchParams({ limit: "500" });
            if (opts.codebase) params.set("codebaseId", opts.codebase);
            const res = await fetch(`${serverUrl}/api/chunks?${params}`);
            if (!res.ok) {
                outputError(`API error: ${res.statusText}`);
                return;
            }
            const { chunks } = (await res.json()) as { chunks: any[] };

            const issues: LintIssue[] = [];

            for (const chunk of chunks) {
                const id = chunk.id;
                const title = chunk.title;

                // Rule: thin content (<100 chars)
                if (chunk.content.length < 100) {
                    issues.push({
                        chunkId: id,
                        chunkTitle: title,
                        severity: "warning",
                        rule: "thin-content",
                        message: `Content is only ${chunk.content.length} chars (min recommended: 100)`
                    });
                }

                // Rule: missing rationale for documents
                if (chunk.type === "document" && !chunk.rationale) {
                    issues.push({
                        chunkId: id,
                        chunkTitle: title,
                        severity: "warning",
                        rule: "missing-rationale",
                        message: "Document chunks should have a rationale explaining 'why'"
                    });
                }

                // Rule: no enrichment (no summary)
                if (!chunk.summary) {
                    issues.push({
                        chunkId: id,
                        chunkTitle: title,
                        severity: "warning",
                        rule: "not-enriched",
                        message: "Chunk has no AI-generated summary (run 'fubbik enrich')"
                    });
                }

                // Rule: title too short
                if (chunk.title.length < 5) {
                    issues.push({
                        chunkId: id,
                        chunkTitle: title,
                        severity: "error",
                        rule: "short-title",
                        message: "Title should be at least 5 characters"
                    });
                }

                // Rule: title too long
                if (chunk.title.length > 150) {
                    issues.push({
                        chunkId: id,
                        chunkTitle: title,
                        severity: "warning",
                        rule: "long-title",
                        message: `Title is ${chunk.title.length} chars (max recommended: 150)`
                    });
                }

                // Rule: draft AI chunks not reviewed
                if (chunk.origin === "ai" && chunk.reviewStatus === "draft") {
                    issues.push({
                        chunkId: id,
                        chunkTitle: title,
                        severity: "warning",
                        rule: "unreviewed-ai",
                        message: "AI-generated chunk still in draft status"
                    });
                }
            }

            // Fetch knowledge health to check for orphans and stale chunks
            const healthRes = await fetch(
                `${serverUrl}/api/health/knowledge${opts.codebase ? `?codebaseId=${opts.codebase}` : ""}`
            );
            if (healthRes.ok) {
                const health = (await healthRes.json()) as any;
                for (const orphan of health.orphans?.chunks ?? []) {
                    issues.push({
                        chunkId: orphan.id,
                        chunkTitle: orphan.title,
                        severity: "warning",
                        rule: "orphan",
                        message: "Chunk has no connections to other chunks"
                    });
                }
                for (const stale of health.stale?.chunks ?? []) {
                    issues.push({
                        chunkId: stale.id,
                        chunkTitle: stale.title,
                        severity: "warning",
                        rule: "stale",
                        message: "Chunk not updated in 30+ days but neighbors have been"
                    });
                }
            }

            // Score mode
            if (opts.score) {
                // Fetch detailed chunk data for connections/appliesTo
                // Use concurrency control to avoid rate limits
                const scored: { title: string; type: string; score: number; id: string }[] = [];
                const CONCURRENCY = 5;

                for (let i = 0; i < chunks.length; i += CONCURRENCY) {
                    const batch = chunks.slice(i, i + CONCURRENCY);
                    const results = await Promise.allSettled(
                        batch.map(async (chunk: any) => {
                            let detail = chunk;
                            try {
                                const detailRes = await fetch(`${serverUrl}/api/chunks/${chunk.id}`);
                                if (detailRes.ok) {
                                    detail = await detailRes.json();
                                }
                            } catch {
                                // Use basic chunk data if detail fetch fails
                            }
                            return {
                                id: chunk.id,
                                title: chunk.title,
                                type: chunk.type,
                                score: computeQualityScore(detail),
                            };
                        })
                    );
                    for (const result of results) {
                        if (result.status === "fulfilled") {
                            scored.push(result.value);
                        }
                    }
                }

                scored.sort((a, b) => a.score - b.score);

                if (isJson(cmd)) {
                    const avg = scored.length > 0
                        ? Math.round(scored.reduce((s, c) => s + c.score, 0) / scored.length)
                        : 0;
                    console.log(
                        JSON.stringify(
                            {
                                scores: scored,
                                summary: {
                                    average: avg,
                                    below50: scored.filter((s) => s.score < 50).length,
                                    above80: scored.filter((s) => s.score >= 80).length,
                                    total: scored.length,
                                },
                            },
                            null,
                            2
                        )
                    );
                    return;
                }

                console.error(formatBold("Quality scores (lowest first):"));
                for (const item of scored) {
                    const scoreStr = String(item.score).padStart(3);
                    console.error(
                        `  ${scoreStr}  [${item.type}] ${item.title}`
                    );
                }

                const avg = scored.length > 0
                    ? Math.round(scored.reduce((s, c) => s + c.score, 0) / scored.length)
                    : 0;
                const below50 = scored.filter((s) => s.score < 50).length;
                const above80 = scored.filter((s) => s.score >= 80).length;

                console.error(
                    `\nAverage: ${avg} | Below 50: ${below50} chunks | Above 80: ${above80} chunks`
                );
                return;
            }

            // Output
            if (isJson(cmd)) {
                console.log(
                    JSON.stringify(
                        {
                            issues,
                            summary: {
                                total: issues.length,
                                errors: issues.filter(i => i.severity === "error").length,
                                warnings: issues.filter(i => i.severity === "warning").length,
                                chunks: chunks.length
                            }
                        },
                        null,
                        2
                    )
                );
                return;
            }

            const errors = issues.filter(i => i.severity === "error");
            const warnings = issues.filter(i => i.severity === "warning");

            if (issues.length === 0) {
                console.error(formatSuccess(`All ${chunks.length} chunks passed lint checks`));
                return;
            }

            console.error(formatBold(`Linted ${chunks.length} chunks:\n`));

            // Group by chunk
            const grouped = new Map<string, LintIssue[]>();
            for (const issue of issues) {
                const key = issue.chunkId;
                if (!grouped.has(key)) grouped.set(key, []);
                grouped.get(key)!.push(issue);
            }

            for (const [, chunkIssues] of grouped) {
                const first = chunkIssues[0]!;
                console.error(`  ${formatBold(first.chunkTitle)} ${formatDim(`(${first.chunkId.slice(0, 8)})`)}`);
                for (const issue of chunkIssues) {
                    const icon =
                        issue.severity === "error"
                            ? formatError(issue.rule)
                            : formatDim(`warn:${issue.rule}`);
                    console.error(`    ${icon} ${issue.message}`);
                }
            }

            console.error(`\n${formatError(`${errors.length} error(s)`)} ${formatDim(`${warnings.length} warning(s)`)}`);

            // Auto-fix safe issues when --fix is set
            if (opts.fix) {
                console.error(formatBold("\nAttempting fixes...\n"));
                let fixed = 0;

                for (const issue of issues) {
                    if (issue.rule === "not-enriched") {
                        try {
                            await fetch(`${serverUrl}/api/chunks/${issue.chunkId}/enrich`, {
                                method: "POST",
                            });
                            console.error(formatSuccess(`  Enriched: ${issue.chunkTitle}`));
                            fixed++;
                        } catch {
                            console.error(
                                formatDim(`  Failed to enrich: ${issue.chunkTitle}`)
                            );
                        }
                    }
                }

                console.error(`\n${formatSuccess(`${fixed} issue(s) auto-fixed`)}`);
            }

            // Exit with non-zero for CI
            if (errors.length > 0) process.exit(1);
        } catch (e: any) {
            outputError(e.message);
        }
    });
