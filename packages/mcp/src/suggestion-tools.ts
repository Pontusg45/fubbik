import type { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { z } from "zod";
import { apiFetch } from "./api-client.js";

export function registerSuggestionTools(server: McpServer): void {
    // 1. suggest_requirements
    server.tool(
        "suggest_requirements",
        "Get context from the knowledge base to suggest new requirements. Returns existing requirements, coverage gaps, health issues, and relevant chunks for a focus area. Use this context to generate requirement suggestions for the developer.",
        {
            focus: z.string().optional().describe("Focus area (e.g., 'auth', 'error handling', 'testing'). Omit for broad overview."),
            codebaseId: z.string().optional().describe("Codebase ID to scope suggestions")
        },
        async ({ focus, codebaseId }) => {
            const params = new URLSearchParams();
            if (focus) params.set("focus", focus);
            if (codebaseId) params.set("codebaseId", codebaseId);

            const data = await apiFetch(`/requirements/suggest-context?${params}`) as {
                useCases: Array<{
                    id: string; name: string; parentId: string | null;
                    requirementCount: number;
                    requirements: Array<{ id: string; title: string; status: string }>;
                }>;
                coverageGaps: Array<{ id: string; title: string; type: string }>;
                healthIssues: { orphanCount: number; staleCount: number; thinCount: number };
                relevantChunks: Array<{ id: string; title: string; content: string }>;
            };

            const sections: string[] = [];
            sections.push("# Knowledge Base Context for Requirement Suggestions\n");
            if (focus) sections.push(`**Focus area:** ${focus}\n`);

            sections.push("## Existing Requirements\n");
            for (const uc of data.useCases) {
                const parent = uc.parentId ? " (sub use case)" : "";
                sections.push(`### ${uc.name}${parent} (${uc.requirementCount} requirements)`);
                for (const r of uc.requirements) {
                    sections.push(`- [${r.status}] ${r.title}`);
                }
                sections.push("");
            }

            if (data.coverageGaps.length > 0) {
                sections.push("## Uncovered Chunks (no requirements linked)\n");
                for (const gap of data.coverageGaps) {
                    sections.push(`- ${gap.title} (${gap.id})`);
                }
                sections.push("");
            }

            sections.push("## Knowledge Health\n");
            sections.push(`- Orphan chunks (no connections): ${data.healthIssues.orphanCount}`);
            sections.push(`- Stale chunks (>30 days old, neighbors updated): ${data.healthIssues.staleCount}`);
            sections.push(`- Thin chunks (<100 chars): ${data.healthIssues.thinCount}`);
            sections.push("");

            if (data.relevantChunks.length > 0) {
                sections.push("## Relevant Chunks\n");
                for (const c of data.relevantChunks) {
                    sections.push(`### ${c.title} (${c.id})`);
                    sections.push(c.content);
                    sections.push("");
                }
            }

            sections.push("---\n");
            sections.push("Based on this context, suggest new requirements organized into use cases.");
            sections.push("For each requirement, provide: title, Given/When/Then steps, priority, and which use case it belongs to.");
            sections.push("When ready, call `create_requirements_batch` to create the approved requirements.");

            return { content: [{ type: "text" as const, text: sections.join("\n") }] };
        }
    );

    // 2. create_requirements_batch
    server.tool(
        "create_requirements_batch",
        "Batch create multiple requirements with automatic use case resolution. Use cases are created automatically if they don't exist.",
        {
            requirements: z.array(z.object({
                title: z.string().describe("Requirement title"),
                description: z.string().optional().describe("Requirement description"),
                steps: z.array(z.object({
                    keyword: z.enum(["given", "when", "then", "and", "but"]),
                    text: z.string()
                })).min(1).describe("Given/When/Then steps"),
                priority: z.enum(["must", "should", "could", "wont"]).optional().describe("MoSCoW priority"),
                useCaseId: z.string().optional().describe("Existing use case ID"),
                useCaseName: z.string().optional().describe("Use case name (created if doesn't exist)"),
                parentUseCaseName: z.string().optional().describe("Parent use case name (created if doesn't exist)")
            })).min(1).max(50).describe("Requirements to create"),
            codebaseId: z.string().optional().describe("Codebase ID")
        },
        async ({ requirements, codebaseId }) => {
            const body: Record<string, unknown> = { requirements };
            if (codebaseId) body.codebaseId = codebaseId;

            const data = await apiFetch("/requirements/batch", {
                method: "POST",
                body: JSON.stringify(body)
            }) as {
                created: number;
                requirements: Array<{ id: string; title: string; useCaseId: string | null }>;
                useCasesCreated: Array<{ id: string; name: string; parentId: string | null }>;
            };

            const lines: string[] = [];
            lines.push(`# Created ${data.created} Requirements\n`);
            if (data.useCasesCreated.length > 0) {
                lines.push(`**Use cases auto-created:** ${data.useCasesCreated.map(uc => uc.name).join(", ")}\n`);
            }
            lines.push("## Requirements Created\n");
            for (const r of data.requirements) {
                lines.push(`- ${r.title} (${r.id})`);
            }

            return { content: [{ type: "text" as const, text: lines.join("\n") }] };
        }
    );
}
