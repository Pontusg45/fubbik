import type { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { z } from "zod";
import { apiFetch } from "./api-client.js";
import type { McpPlugin } from "./plugin.js";

export function registerContextTools(server: McpServer): void {
    server.tool(
        "sync_claude_md",
        "Generate .claude/CLAUDE.md content from chunks tagged with a specific tag (default: claude-context). Returns the markdown content that can be written to CLAUDE.md.",
        {
            tag: z.string().optional().describe("Tag to filter by (default: claude-context)"),
            codebaseId: z.string().optional().describe("Codebase ID to scope chunks")
        },
        async ({ tag, codebaseId }) => {
            const filterTag = tag ?? "claude-context";
            const params = new URLSearchParams();
            params.set("tags", filterTag);
            if (codebaseId) params.set("codebaseId", codebaseId);
            params.set("limit", "100");

            const data = (await apiFetch(`/chunks?${params}`)) as {
                chunks: Array<{
                    id: string;
                    title: string;
                    type: string | null;
                    content: string | null;
                }>;
            };

            if (data.chunks.length === 0) {
                return {
                    content: [
                        {
                            type: "text" as const,
                            text: `No chunks tagged "${filterTag}" found. Tag some chunks with "${filterTag}" to generate CLAUDE.md content.`
                        }
                    ]
                };
            }

            const sections: string[] = [];
            sections.push("# Project Context\n");
            sections.push(
                `> Auto-generated from ${data.chunks.length} chunks tagged \`${filterTag}\`.\n`
            );

            for (const chunk of data.chunks) {
                sections.push(`## ${chunk.title}\n`);
                if (chunk.content) {
                    sections.push(chunk.content);
                }
                sections.push("");
            }

            const content = sections.join("\n");

            return {
                content: [
                    {
                        type: "text" as const,
                        text: `Generated CLAUDE.md content (${data.chunks.length} chunks):\n\n${content}`
                    }
                ]
            };
        }
    );

    server.tool(
        "get_context",
        "Retrieve structured knowledge context. Provide planId to get context for a plan, concept for topic-based context, or filePath for file-specific context. Multiple params combine results.",
        {
            planId: z.string().optional().describe("Plan ID to get context for"),
            concept: z.string().optional().describe("Concept or topic to get context about"),
            filePath: z.string().optional().describe("File path to get context for"),
            maxTokens: z.number().optional().describe("Max tokens (default 8000)"),
            codebaseId: z.string().optional().describe("Codebase ID to scope context"),
        },
        async ({ planId, concept, filePath, maxTokens, codebaseId }) => {
            const tokens = maxTokens ?? 8000;
            const parts: string[] = [];

            if (planId && filePath) {
                // Make two calls and combine
                const planParams = new URLSearchParams({ planId, maxTokens: String(tokens), format: "structured-md" });
                if (codebaseId) planParams.set("codebaseId", codebaseId);
                const planData = (await apiFetch(`/context/for-plan?${planParams}`)) as { content?: string } | string;
                const planText = typeof planData === "string" ? planData : (planData as { content?: string }).content ?? "";

                const fileParams = new URLSearchParams({ paths: filePath, maxTokens: String(tokens), format: "structured-md" });
                if (codebaseId) fileParams.set("codebaseId", codebaseId);
                const fileData = (await apiFetch(`/context/for-files?${fileParams}`)) as { content?: string } | string;
                const fileText = typeof fileData === "string" ? fileData : (fileData as { content?: string }).content ?? "";

                parts.push(planText, fileText);
            } else if (planId) {
                const params = new URLSearchParams({ planId, maxTokens: String(tokens), format: "structured-md" });
                if (codebaseId) params.set("codebaseId", codebaseId);
                const data = (await apiFetch(`/context/for-plan?${params}`)) as { content?: string } | string;
                parts.push(typeof data === "string" ? data : (data as { content?: string }).content ?? "");
            } else if (concept) {
                const params = new URLSearchParams({ q: concept, maxTokens: String(tokens), format: "structured-md" });
                if (codebaseId) params.set("codebaseId", codebaseId);
                const data = (await apiFetch(`/context/about?${params}`)) as { content?: string } | string;
                parts.push(typeof data === "string" ? data : (data as { content?: string }).content ?? "");
            } else if (filePath) {
                const params = new URLSearchParams({ paths: filePath, maxTokens: String(tokens), format: "structured-md" });
                if (codebaseId) params.set("codebaseId", codebaseId);
                const data = (await apiFetch(`/context/for-files?${params}`)) as { content?: string } | string;
                parts.push(typeof data === "string" ? data : (data as { content?: string }).content ?? "");
            } else {
                return {
                    content: [{ type: "text" as const, text: "Provide at least one of: planId, concept, or filePath." }]
                };
            }

            return {
                content: [{ type: "text" as const, text: parts.filter(Boolean).join("\n\n---\n\n") }]
            };
        }
    );

    server.tool(
        "get_context_for_task",
        "Get tightly-scoped context for a specific task within a plan. Fetches plan detail, identifies relevant chunks for the task, and returns structured context.",
        {
            planId: z.string().describe("Plan ID"),
            taskId: z.string().describe("Task (step) ID within the plan"),
            maxTokens: z.number().optional().describe("Max tokens (default 4000)"),
        },
        async ({ planId, taskId, maxTokens }) => {
            const tokens = maxTokens ?? 4000;

            // Fetch plan detail
            const planDetail = (await apiFetch(`/plans/${planId}`)) as {
                plan?: unknown;
                tasks?: Array<{ id: string; chunks?: Array<{ chunkId: string }> }>;
                analyze?: { chunk?: Array<{ chunkId: string }> };
            };

            // Find the specific task
            const task = planDetail.tasks?.find((t) => t.id === taskId);

            // Collect chunk IDs from task + plan analyze
            const chunkIds = new Set<string>();
            if (task?.chunks) {
                for (const c of task.chunks) chunkIds.add(c.chunkId);
            }
            if (planDetail.analyze?.chunk) {
                for (const c of planDetail.analyze.chunk) chunkIds.add(c.chunkId);
            }

            // Use for-plan endpoint as the context source (v1 approximation)
            const params = new URLSearchParams({ planId, maxTokens: String(tokens), format: "structured-md" });
            const data = (await apiFetch(`/context/for-plan?${params}`)) as { content?: string } | string;
            const contextText = typeof data === "string" ? data : (data as { content?: string }).content ?? "";

            const header = task
                ? `# Context for Task: ${taskId}\n\nRelevant chunk IDs: ${[...chunkIds].join(", ") || "none"}\n\n`
                : `# Context for Plan: ${planId}\n\nTask ${taskId} not found — returning full plan context.\n\n`;

            return {
                content: [{ type: "text" as const, text: header + contextText }]
            };
        }
    );
}

export const contextPlugin: McpPlugin = {
    name: "context",
    description: "Context export and CLAUDE.md sync tools",
    register: registerContextTools,
};
