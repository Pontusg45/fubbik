import type { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { z } from "zod";
import { apiFetch, truncate } from "./api-client.js";
import type { McpPlugin } from "./plugin.js";

export function registerSessionTools(server: McpServer): void {
    // 1. begin_implementation
    server.tool(
        "begin_implementation",
        "Start an implementation session. Returns context bundle with conventions, requirements, and architecture decisions.",
        {
            title: z.string().describe("Brief description of what you're implementing"),
            codebaseId: z.string().optional().describe("Codebase ID to scope context"),
            planId: z.string().optional().describe("Plan ID to link this session to a plan (auto-activates draft plans, includes plan steps in context)")
        },
        async ({ title, codebaseId, planId }) => {
            const body: Record<string, unknown> = { title };
            if (codebaseId) body.codebaseId = codebaseId;
            if (planId) body.planId = planId;

            const data = await apiFetch("/sessions", {
                method: "POST",
                body: JSON.stringify(body)
            }) as {
                session: { id: string; title: string };
                context: {
                    conventions: Array<{ id: string; title: string; content: string | null }>;
                    requirements: Array<{ id: string; title: string; steps: unknown; status: string }>;
                    architectureDecisions: Array<{ id: string; title: string; content: string | null; rationale: string | null }>;
                    planSteps?: Array<{ id: string; description: string; status: string; order: number; chunkId: string | null }>;
                };
            };

            const sections: string[] = [];
            sections.push(`# Implementation Session Started\n`);
            sections.push(`**Session ID:** ${data.session.id}`);
            sections.push(`**Task:** ${data.session.title}\n`);

            if (data.context.conventions.length > 0) {
                sections.push(`## Conventions (${data.context.conventions.length})\n`);
                for (const c of data.context.conventions) {
                    sections.push(`### ${c.title} (${c.id})`);
                    sections.push(truncate(c.content, 500));
                    sections.push("");
                }
            }

            if (data.context.requirements.length > 0) {
                sections.push(`## Requirements (${data.context.requirements.length})\n`);
                for (const r of data.context.requirements) {
                    sections.push(`- **${r.title}** [${r.status}] (${r.id})`);
                }
                sections.push("");
            }

            if (data.context.architectureDecisions.length > 0) {
                sections.push(`## Architecture Decisions (${data.context.architectureDecisions.length})\n`);
                for (const d of data.context.architectureDecisions) {
                    sections.push(`### ${d.title} (${d.id})`);
                    sections.push(truncate(d.content, 300));
                    if (d.rationale) sections.push(`**Rationale:** ${truncate(d.rationale, 200)}`);
                    sections.push("");
                }
            }

            if (data.context.planSteps && data.context.planSteps.length > 0) {
                sections.push(`## Plan Steps (${data.context.planSteps.length})\n`);
                for (const s of data.context.planSteps) {
                    const statusIcon = s.status === "done" ? "[x]" : s.status === "skipped" ? "[-]" : "[ ]";
                    sections.push(`${statusIcon} ${s.order + 1}. ${s.description} (${s.id})${s.chunkId ? ` → chunk:${s.chunkId}` : ""}`);
                }
                sections.push("");
            }

            return {
                content: [{ type: "text" as const, text: sections.join("\n") }]
            };
        }
    );

    // 2. record_chunk_reference
    server.tool(
        "record_chunk_reference",
        "Record that you referenced a specific knowledge chunk during implementation",
        {
            sessionId: z.string().describe("Session ID from begin_implementation"),
            chunkId: z.string().describe("ID of the chunk you referenced"),
            reason: z.string().describe("Why you referenced this chunk (e.g. 'followed error handling convention')")
        },
        async ({ sessionId, chunkId, reason }) => {
            await apiFetch(`/sessions/${sessionId}/chunk-refs`, {
                method: "POST",
                body: JSON.stringify({ chunkId, reason })
            });
            return {
                content: [{ type: "text" as const, text: `Recorded reference to chunk ${chunkId}` }]
            };
        }
    );

    // 3. record_assumption
    server.tool(
        "record_assumption",
        "Record an assumption you made because no knowledge chunk covered this area",
        {
            sessionId: z.string().describe("Session ID from begin_implementation"),
            description: z.string().describe("What you assumed and why (e.g. 'No convention for CSV encoding — assumed UTF-8 with comma delimiter')")
        },
        async ({ sessionId, description }) => {
            await apiFetch(`/sessions/${sessionId}/assumptions`, {
                method: "POST",
                body: JSON.stringify({ description })
            });
            return {
                content: [{ type: "text" as const, text: `Assumption recorded: ${description}` }]
            };
        }
    );

    // 4. record_requirement_addressed
    server.tool(
        "record_requirement_addressed",
        "Record that you implemented (fully or partially) a requirement",
        {
            sessionId: z.string().describe("Session ID from begin_implementation"),
            requirementId: z.string().describe("ID of the requirement you addressed"),
            stepsAddressed: z.array(z.number()).optional().describe("Indices of specific steps addressed (omit for full requirement)")
        },
        async ({ sessionId, requirementId, stepsAddressed }) => {
            const body: Record<string, unknown> = { requirementId };
            if (stepsAddressed) body.stepsAddressed = stepsAddressed;
            await apiFetch(`/sessions/${sessionId}/requirement-refs`, {
                method: "POST",
                body: JSON.stringify(body)
            });
            return {
                content: [{ type: "text" as const, text: `Recorded: addressed requirement ${requirementId}` }]
            };
        }
    );

    // 5. complete_implementation
    server.tool(
        "complete_implementation",
        "Mark an implementation session as complete. Generates a review brief for the developer.",
        {
            sessionId: z.string().describe("Session ID from begin_implementation"),
            prUrl: z.string().optional().describe("URL of the pull request or branch")
        },
        async ({ sessionId, prUrl }) => {
            const body: Record<string, unknown> = {};
            if (prUrl) body.prUrl = prUrl;

            const data = await apiFetch(`/sessions/${sessionId}/complete`, {
                method: "PATCH",
                body: JSON.stringify(body)
            }) as { reviewBrief: string };

            return {
                content: [{ type: "text" as const, text: `# Implementation Complete\n\n${data.reviewBrief ?? "Review brief generated. Developer can view it at /reviews/" + sessionId}` }]
            };
        }
    );

    // 6. resolve_assumption_as_chunk
    server.tool(
        "resolve_assumption_as_chunk",
        "Resolve a session assumption by creating a knowledge chunk from it. This turns knowledge gaps into documented chunks automatically — the assumption is marked resolved and a new chunk is created with the assumption context as content.",
        {
            sessionId: z.string().describe("Session ID"),
            assumptionId: z.string().describe("Assumption ID to resolve"),
            chunkTitle: z.string().describe("Title for the new chunk"),
            chunkContent: z.string().describe("Content for the new chunk (expand on the assumption with the correct answer)"),
            chunkType: z.string().optional().describe("Chunk type (default: note)"),
            codebaseId: z.string().optional().describe("Codebase ID to associate the chunk with"),
            resolution: z.string().optional().describe("Resolution note (defaults to 'Resolved by creating chunk')")
        },
        async ({ sessionId, assumptionId, chunkTitle, chunkContent, chunkType, codebaseId, resolution }) => {
            // 1. Create the chunk
            const chunkBody: Record<string, unknown> = {
                title: chunkTitle,
                content: chunkContent
            };
            if (chunkType) chunkBody.type = chunkType;
            if (codebaseId) chunkBody.codebaseIds = [codebaseId];

            const chunk = (await apiFetch("/chunks", {
                method: "POST",
                body: JSON.stringify(chunkBody)
            })) as { id: string; title: string };

            // 2. Resolve the assumption
            await apiFetch(`/sessions/${sessionId}/assumptions/${assumptionId}`, {
                method: "PATCH",
                body: JSON.stringify({
                    resolved: true,
                    resolution: resolution ?? `Resolved by creating chunk: ${chunk.title} (${chunk.id})`
                })
            });

            return {
                content: [{
                    type: "text" as const,
                    text: `Assumption resolved and chunk created:\n- Chunk: ${chunk.title} (${chunk.id})\n- Assumption marked as resolved`
                }]
            };
        }
    );

    // 7. mark_plan_step
    server.tool(
        "mark_plan_step",
        "Mark a plan step as done",
        {
            sessionId: z.string().describe("Session ID from begin_implementation"),
            stepId: z.string().describe("Plan step ID to mark as done"),
            note: z.string().optional().describe("Optional note about the step completion")
        },
        async ({ sessionId, stepId, note }) => {
            const session = (await apiFetch(`/sessions/${sessionId}`)) as { session: { planId: string | null } };
            if (!session.session.planId) {
                return {
                    content: [{ type: "text" as const, text: "Session not linked to a plan" }]
                };
            }
            const body: Record<string, unknown> = { status: "done" };
            if (note) body.note = note;
            await apiFetch(`/plans/${session.session.planId}/steps/${stepId}`, {
                method: "PATCH",
                body: JSON.stringify(body)
            });
            return {
                content: [{ type: "text" as const, text: `Step ${stepId} marked as done` }]
            };
        }
    );

    // 8. get_plan_progress
    server.tool(
        "get_plan_progress",
        "Get progress of the plan linked to a session",
        {
            sessionId: z.string().describe("Session ID from begin_implementation")
        },
        async ({ sessionId }) => {
            const session = (await apiFetch(`/sessions/${sessionId}`)) as { session: { planId: string | null } };
            if (!session.session.planId) {
                return {
                    content: [{ type: "text" as const, text: "Session not linked to a plan" }]
                };
            }
            const plan = (await apiFetch(`/plans/${session.session.planId}`)) as {
                title: string;
                status: string;
                steps: Array<{ id: string; description: string; status: string; order: number }>;
            };

            const total = plan.steps.length;
            const done = plan.steps.filter(s => s.status === "done").length;
            const skipped = plan.steps.filter(s => s.status === "skipped").length;
            const pct = total > 0 ? Math.round(((done + skipped) / total) * 100) : 0;

            const sections: string[] = [];
            sections.push(`# Plan Progress: ${plan.title} [${plan.status}]\n`);
            sections.push(`**Progress:** ${done}/${total} done (${pct}%)${skipped > 0 ? `, ${skipped} skipped` : ""}\n`);

            for (const s of plan.steps.sort((a, b) => a.order - b.order)) {
                const icon = s.status === "done" ? "[x]" : s.status === "skipped" ? "[-]" : s.status === "in_progress" ? "[~]" : "[ ]";
                sections.push(`${icon} ${s.order + 1}. ${s.description} (${s.id})`);
            }

            return {
                content: [{ type: "text" as const, text: sections.join("\n") }]
            };
        }
    );
}

export const sessionPlugin: McpPlugin = {
    name: "sessions",
    description: "Implementation session tracking tools",
    register: registerSessionTools,
};
