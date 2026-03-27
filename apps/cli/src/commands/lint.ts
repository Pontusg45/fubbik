import { Command } from "commander";

import { formatBold, formatDim, formatError, formatSuccess } from "../lib/colors";
import { isJson, outputError } from "../lib/output";
import { getServerUrl } from "../lib/store";

interface LintIssue {
    chunkId: string;
    chunkTitle: string;
    severity: "error" | "warning";
    rule: string;
    message: string;
}

function requireServer(): string {
    const serverUrl = getServerUrl();
    if (!serverUrl) {
        console.error('No server URL configured. Run "fubbik init" first.');
        process.exit(1);
    }
    return serverUrl;
}

export const lintCommand = new Command("lint")
    .description("Check all chunks for quality issues")
    .option("--codebase <name>", "scope to codebase")
    .option("--fix", "suggest fixes (not auto-fix)")
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

            // Exit with non-zero for CI
            if (errors.length > 0) process.exit(1);
        } catch (e: any) {
            outputError(e.message);
        }
    });
