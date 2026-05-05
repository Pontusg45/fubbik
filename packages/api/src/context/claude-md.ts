import { listChunksByTag, listRequirements, listPlans, getChunksForRequirement } from "@fubbik/db/repository";
import { listTasks } from "@fubbik/db/repository/plan";
import { Effect } from "effect";

import { estimateTokens } from "./utils";

interface GenerateClaudeMdParams {
    userId: string;
    codebaseId?: string;
    tag?: string;
    maxTokens?: number;
}

interface ChunkRow {
    id: string;
    title: string;
    content: string;
    type: string;
    rationale: string | null;
    summary: string | null;
}

const TYPE_SECTIONS: Record<string, string> = {
    note: "Conventions",
    document: "Architecture",
    reference: "References"
};

function sectionLabel(type: string): string {
    return TYPE_SECTIONS[type] ?? "Other";
}

function formatChunkEntry(c: ChunkRow): string {
    const parts = [`### ${c.title}`];
    if (c.content) parts.push(c.content);
    if (c.rationale) parts.push(`**Rationale:** ${c.rationale}`);
    return parts.join("\n\n");
}

export function generateClaudeMd(params: GenerateClaudeMdParams) {
    const tagName = params.tag ?? "claude-context";

    return Effect.gen(function* () {
        // Fetch chunks (existing)
        const chunks = yield* listChunksByTag({
            userId: params.userId,
            tagName,
            codebaseId: params.codebaseId
        });

        const parts: string[] = ["# Project Context\n"];

        // ── Chunks section (existing logic) ──
        if (chunks.length === 0) {
            parts.push(`No chunks found with tag "${tagName}".\n`);
        } else {
            const sections = new Map<string, ChunkRow[]>();
            for (const c of chunks) {
                const label = sectionLabel(c.type);
                const group = sections.get(label) ?? [];
                group.push(c);
                sections.set(label, group);
            }

            const sectionOrder = ["Conventions", "Architecture", "References", "Other"];
            for (const sectionName of sectionOrder) {
                const group = sections.get(sectionName);
                if (!group || group.length === 0) continue;
                parts.push(`## ${sectionName}\n`);
                for (const c of group) {
                    parts.push(formatChunkEntry(c));
                }
            }
        }

        // ── Requirements section ──
        const { requirements } = yield* listRequirements({
            userId: params.userId,
            codebaseId: params.codebaseId,
            limit: 50,
            offset: 0
        });

        if (requirements.length > 0) {
            parts.push("## Requirements\n");

            const statusOrder: Record<string, number> = { failing: 0, untested: 1, passing: 2 };
            const sorted = [...requirements].sort(
                (a, b) => (statusOrder[a.status ?? ""] ?? 3) - (statusOrder[b.status ?? ""] ?? 3)
            );

            for (const req of sorted) {
                const marker =
                    req.status === "failing" || req.status === "untested"
                        ? " <!-- ACTION NEEDED -->"
                        : "";
                const priority = req.priority ? ` [${req.priority}]` : "";
                parts.push(`### ${req.title}${priority} — ${req.status}${marker}`);

                if (req.steps && Array.isArray(req.steps)) {
                    const stepsText = (req.steps as Array<{ keyword: string; text: string }>)
                        .map(s => `- **${s.keyword}** ${s.text}`)
                        .join("\n");
                    parts.push(stepsText);
                }

                const linkedChunks = yield* getChunksForRequirement(req.id);
                if (linkedChunks.length > 0) {
                    const chunkList = linkedChunks.map(c => c.title).join(", ");
                    parts.push(`**Linked chunks:** ${chunkList}`);
                }
            }
        }

        // ── Active plans section ──
        const plans = yield* listPlans({
            userId: params.userId,
            codebaseId: params.codebaseId,
            status: "in_progress",
        });

        if (plans.length > 0) {
            parts.push("## Active Plans\n");

            for (const plan of plans) {
                const tasks = yield* listTasks(plan.id);
                const done = tasks.filter(t => t.status === "done").length;
                const total = tasks.length;
                const pct = total > 0 ? Math.round((done / total) * 100) : 0;

                parts.push(`### ${plan.title} (${done}/${total} tasks — ${pct}%)`);

                const pending = tasks.filter(
                    t => t.status === "pending" || t.status === "in_progress"
                );
                if (pending.length > 0) {
                    const pendingText = pending.map(t => `- [ ] ${t.title}`).join("\n");
                    parts.push(pendingText);
                }
            }
        }

        const maxTokens = params.maxTokens ?? 32000;
        let content = parts.join("\n\n");
        const totalTokens = estimateTokens(content);

        if (totalTokens > maxTokens) {
            // Rebuild with budget — include chunks greedily until budget exhausted
            const budgetedParts: string[] = ["# Project Context\n"];
            let usedTokens = estimateTokens(budgetedParts[0]!);
            let omittedChunks = 0;

            if (chunks.length > 0) {
                const sections = new Map<string, ChunkRow[]>();
                for (const c of chunks) {
                    const label = sectionLabel(c.type);
                    const group = sections.get(label) ?? [];
                    group.push(c);
                    sections.set(label, group);
                }

                const sectionOrder = ["Conventions", "Architecture", "References", "Other"];
                for (const sectionName of sectionOrder) {
                    const group = sections.get(sectionName);
                    if (!group || group.length === 0) continue;
                    const sectionHeader = `## ${sectionName}\n`;
                    const sectionTokens = estimateTokens(sectionHeader);
                    if (usedTokens + sectionTokens > maxTokens) {
                        omittedChunks += group.length;
                        continue;
                    }
                    budgetedParts.push(sectionHeader);
                    usedTokens += sectionTokens;

                    for (const c of group) {
                        const entry = formatChunkEntry(c);
                        const entryTokens = estimateTokens(entry);
                        if (usedTokens + entryTokens > maxTokens) {
                            omittedChunks++;
                            continue;
                        }
                        budgetedParts.push(entry);
                        usedTokens += entryTokens;
                    }
                }
            }

            if (omittedChunks > 0) {
                budgetedParts.push(
                    `<!-- Truncated: ${omittedChunks} chunks omitted due to token budget. Increase maxTokens or narrow the tag filter. -->`
                );
            }

            content = budgetedParts.join("\n\n");
        }

        return { content, chunks: chunks.length };
    });
}
