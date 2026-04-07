import { listChunksByTag, listRequirements, listSessions, getSessionDetail, listPlans, getStepsForPlan, getChunksForRequirement } from "@fubbik/db/repository";
import { Effect } from "effect";

interface GenerateClaudeMdParams {
    userId: string;
    codebaseId?: string;
    tag?: string;
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
        const plans = yield* listPlans(params.userId, params.codebaseId, "active");

        if (plans.length > 0) {
            parts.push("## Active Plans\n");

            for (const plan of plans) {
                const steps = yield* getStepsForPlan(plan.id);
                type Step = (typeof steps)[number];
                const done = steps.filter((s: Step) => s.status === "done" || s.status === "skipped").length;
                const total = steps.length;
                const pct = total > 0 ? Math.round((done / total) * 100) : 0;

                parts.push(`### ${plan.title} (${done}/${total} steps — ${pct}%)`);

                const pending = steps.filter(
                    (s: Step) => s.status === "pending" || s.status === "in_progress"
                );
                if (pending.length > 0) {
                    const pendingText = pending.map((s: Step) => `- [ ] ${s.description}`).join("\n");
                    parts.push(pendingText);
                }
            }
        }

        // ── Recent sessions section ──
        const { sessions } = yield* listSessions({
            userId: params.userId,
            status: "completed",
            limit: 5,
            offset: 0
        });

        if (sessions.length > 0) {
            parts.push("## Recent Implementation Sessions\n");

            for (const session of sessions) {
                const detail = yield* getSessionDetail(session.id);
                const reqCount = detail.requirementRefs?.length ?? 0;
                const unresolvedCount = (detail.assumptions ?? []).filter(
                    (a: { resolved: boolean }) => !a.resolved
                ).length;

                let line = `### ${session.title} — ${session.status}`;
                if (reqCount > 0) line += ` (${reqCount} requirements addressed)`;
                parts.push(line);

                if (unresolvedCount > 0) {
                    parts.push(`**${unresolvedCount} unresolved assumption(s)**`);
                }
            }
        }

        return { content: parts.join("\n\n"), chunks: chunks.length };
    });
}
