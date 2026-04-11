import type { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { z } from "zod";
import { apiFetch } from "./api-client.js";
import type { McpPlugin } from "./plugin.js";

export function registerPlanTools(server: McpServer): void {
    server.tool(
        "create_plan",
        "Create a new plan with title, optional description, codebase, and requirements",
        {
            title: z.string().describe("Plan title"),
            description: z.string().optional().describe("Plan description"),
            codebaseId: z.string().optional().describe("Codebase ID to associate with"),
            requirementIds: z.array(z.string()).optional().describe("Requirement IDs to link"),
        },
        async ({ title, description, codebaseId, requirementIds }) => {
            const body: Record<string, unknown> = { title };
            if (description) body.description = description;
            if (codebaseId) body.codebaseId = codebaseId;
            if (requirementIds) body.requirementIds = requirementIds;

            const plan = (await apiFetch("/plans", {
                method: "POST",
                body: JSON.stringify(body),
            })) as Record<string, unknown>;

            return { content: [{ type: "text" as const, text: JSON.stringify(plan, null, 2) }] };
        },
    );

    server.tool(
        "list_plans",
        "List plans with optional filters",
        {
            codebaseId: z.string().optional().describe("Filter by codebase ID"),
            status: z.string().optional().describe("Filter by status (draft, analyzing, ready, in_progress, completed, archived)"),
            requirementId: z.string().optional().describe("Filter by linked requirement ID"),
        },
        async ({ codebaseId, status, requirementId }) => {
            const params = new URLSearchParams();
            if (codebaseId) params.set("codebaseId", codebaseId);
            if (status) params.set("status", status);
            if (requirementId) params.set("requirementId", requirementId);

            const plans = (await apiFetch(`/plans?${params}`)) as unknown;
            return { content: [{ type: "text" as const, text: JSON.stringify(plans, null, 2) }] };
        },
    );

    server.tool(
        "get_plan",
        "Get plan details including tasks, analyze items, and requirements",
        { planId: z.string().describe("Plan ID") },
        async ({ planId }) => {
            const detail = (await apiFetch(`/plans/${planId}`)) as unknown;
            return { content: [{ type: "text" as const, text: JSON.stringify(detail, null, 2) }] };
        },
    );

    server.tool(
        "update_plan",
        "Update plan title, description, or status",
        {
            planId: z.string().describe("Plan ID"),
            title: z.string().optional().describe("New title"),
            description: z.string().optional().describe("New description"),
            status: z
                .enum(["draft", "analyzing", "ready", "in_progress", "completed", "archived"])
                .optional()
                .describe("New status"),
        },
        async ({ planId, ...patch }) => {
            const plan = (await apiFetch(`/plans/${planId}`, {
                method: "PATCH",
                body: JSON.stringify(patch),
            })) as unknown;
            return { content: [{ type: "text" as const, text: JSON.stringify(plan, null, 2) }] };
        },
    );

    server.tool(
        "link_requirement",
        "Link a requirement to a plan",
        { planId: z.string().describe("Plan ID"), requirementId: z.string().describe("Requirement ID") },
        async ({ planId, requirementId }) => {
            await apiFetch(`/plans/${planId}/requirements`, {
                method: "POST",
                body: JSON.stringify({ requirementId }),
            });
            return { content: [{ type: "text" as const, text: "Requirement linked" }] };
        },
    );

    server.tool(
        "unlink_requirement",
        "Unlink a requirement from a plan",
        { planId: z.string().describe("Plan ID"), requirementId: z.string().describe("Requirement ID") },
        async ({ planId, requirementId }) => {
            await apiFetch(`/plans/${planId}/requirements/${requirementId}`, { method: "DELETE" });
            return { content: [{ type: "text" as const, text: "Requirement unlinked" }] };
        },
    );

    server.tool(
        "add_analyze_item",
        "Add a chunk, file, risk, assumption, or question to the plan's analyze phase",
        {
            planId: z.string().describe("Plan ID"),
            kind: z
                .enum(["chunk", "file", "risk", "assumption", "question"])
                .describe("Kind of analyze item"),
            chunkId: z.string().optional().describe("Chunk ID (for kind=chunk)"),
            filePath: z.string().optional().describe("File path (for kind=file)"),
            text: z.string().optional().describe("Text content (for risk/assumption/question)"),
            metadata: z.record(z.unknown()).optional().describe("Additional metadata"),
        },
        async ({ planId, kind, chunkId, filePath, text, metadata }) => {
            const body: Record<string, unknown> = { kind };
            if (chunkId) body.chunkId = chunkId;
            if (filePath) body.filePath = filePath;
            if (text) body.text = text;
            if (metadata) body.metadata = metadata;

            const item = (await apiFetch(`/plans/${planId}/analyze`, {
                method: "POST",
                body: JSON.stringify(body),
            })) as unknown;
            return { content: [{ type: "text" as const, text: JSON.stringify(item, null, 2) }] };
        },
    );

    server.tool(
        "update_analyze_item",
        "Update an analyze item's text or metadata",
        {
            planId: z.string().describe("Plan ID"),
            itemId: z.string().describe("Analyze item ID"),
            text: z.string().optional().describe("Updated text"),
            metadata: z.record(z.unknown()).optional().describe("Updated metadata"),
        },
        async ({ planId, itemId, text, metadata }) => {
            const body: Record<string, unknown> = {};
            if (text !== undefined) body.text = text;
            if (metadata !== undefined) body.metadata = metadata;

            const item = (await apiFetch(`/plans/${planId}/analyze/${itemId}`, {
                method: "PATCH",
                body: JSON.stringify(body),
            })) as unknown;
            return { content: [{ type: "text" as const, text: JSON.stringify(item, null, 2) }] };
        },
    );

    server.tool(
        "delete_analyze_item",
        "Delete an analyze item from a plan",
        { planId: z.string().describe("Plan ID"), itemId: z.string().describe("Analyze item ID") },
        async ({ planId, itemId }) => {
            await apiFetch(`/plans/${planId}/analyze/${itemId}`, { method: "DELETE" });
            return { content: [{ type: "text" as const, text: "Analyze item deleted" }] };
        },
    );

    server.tool(
        "add_task",
        "Add a task to a plan",
        {
            planId: z.string().describe("Plan ID"),
            title: z.string().describe("Task title"),
            description: z.string().optional().describe("Task description"),
            acceptanceCriteria: z.array(z.string()).optional().describe("Acceptance criteria"),
            chunks: z
                .array(
                    z.object({
                        chunkId: z.string(),
                        relation: z.enum(["context", "created", "modified"]),
                    }),
                )
                .optional()
                .describe("Chunk links with relation type"),
            dependsOnTaskIds: z.array(z.string()).optional().describe("Task IDs this task depends on"),
        },
        async ({ planId, title, description, acceptanceCriteria, chunks, dependsOnTaskIds }) => {
            const body: Record<string, unknown> = { title };
            if (description) body.description = description;
            if (acceptanceCriteria) body.acceptanceCriteria = acceptanceCriteria;
            if (chunks) body.chunks = chunks;
            if (dependsOnTaskIds) body.dependsOnTaskIds = dependsOnTaskIds;

            const task = (await apiFetch(`/plans/${planId}/tasks`, {
                method: "POST",
                body: JSON.stringify(body),
            })) as unknown;
            return { content: [{ type: "text" as const, text: JSON.stringify(task, null, 2) }] };
        },
    );

    server.tool(
        "update_task",
        "Update a task's title, description, acceptance criteria, or status",
        {
            planId: z.string().describe("Plan ID"),
            taskId: z.string().describe("Task ID"),
            title: z.string().optional().describe("New title"),
            description: z.string().optional().describe("New description"),
            acceptanceCriteria: z.array(z.string()).optional().describe("Updated acceptance criteria"),
            status: z
                .enum(["pending", "in_progress", "done", "skipped", "blocked"])
                .optional()
                .describe("New status"),
        },
        async ({ planId, taskId, title, description, acceptanceCriteria, status }) => {
            const body: Record<string, unknown> = {};
            if (title !== undefined) body.title = title;
            if (description !== undefined) body.description = description;
            if (acceptanceCriteria !== undefined) body.acceptanceCriteria = acceptanceCriteria;
            if (status !== undefined) body.status = status;

            const task = (await apiFetch(`/plans/${planId}/tasks/${taskId}`, {
                method: "PATCH",
                body: JSON.stringify(body),
            })) as unknown;
            return { content: [{ type: "text" as const, text: JSON.stringify(task, null, 2) }] };
        },
    );

    server.tool(
        "delete_task",
        "Delete a task from a plan",
        { planId: z.string().describe("Plan ID"), taskId: z.string().describe("Task ID") },
        async ({ planId, taskId }) => {
            await apiFetch(`/plans/${planId}/tasks/${taskId}`, { method: "DELETE" });
            return { content: [{ type: "text" as const, text: "Task deleted" }] };
        },
    );

    server.tool(
        "link_task_chunk",
        "Link a chunk to a task with a relation type",
        {
            planId: z.string().describe("Plan ID"),
            taskId: z.string().describe("Task ID"),
            chunkId: z.string().describe("Chunk ID"),
            relation: z.enum(["context", "created", "modified"]).describe("Relation type"),
        },
        async ({ planId, taskId, chunkId, relation }) => {
            const link = (await apiFetch(`/plans/${planId}/tasks/${taskId}/chunks`, {
                method: "POST",
                body: JSON.stringify({ chunkId, relation }),
            })) as unknown;
            return { content: [{ type: "text" as const, text: JSON.stringify(link, null, 2) }] };
        },
    );
}

export const planPlugin: McpPlugin = {
    name: "plans",
    description: "Implementation plan management tools",
    register: registerPlanTools,
};
