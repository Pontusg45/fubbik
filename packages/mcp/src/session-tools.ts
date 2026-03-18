import type { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { z } from "zod";
import { apiFetch, truncate } from "./api-client.js";

export function registerSessionTools(server: McpServer): void {
    // 1. begin_implementation
    server.tool(
        "begin_implementation",
        "Start an implementation session. Returns context bundle with conventions, requirements, and architecture decisions.",
        {
            title: z.string().describe("Brief description of what you're implementing"),
            codebaseId: z.string().optional().describe("Codebase ID to scope context")
        },
        async ({ title, codebaseId }) => {
            const body: Record<string, unknown> = { title };
            if (codebaseId) body.codebaseId = codebaseId;

            const data = await apiFetch("/sessions", {
                method: "POST",
                body: JSON.stringify(body)
            }) as {
                session: { id: string; title: string };
                context: {
                    conventions: Array<{ id: string; title: string; content: string | null }>;
                    requirements: Array<{ id: string; title: string; steps: unknown; status: string }>;
                    architectureDecisions: Array<{ id: string; title: string; content: string | null; rationale: string | null }>;
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
}
