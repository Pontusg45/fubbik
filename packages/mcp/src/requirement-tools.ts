import type { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { z } from "zod";
import { apiFetch } from "./api-client.js";

export function registerRequirementTools(server: McpServer): void {
    // 1. list_requirements
    server.tool(
        "list_requirements",
        "List requirements with optional status and priority filters",
        {
            status: z
                .enum(["passing", "failing", "untested"])
                .optional()
                .describe("Filter by requirement status"),
            priority: z
                .enum(["must", "should", "could", "wont"])
                .optional()
                .describe("Filter by priority"),
            codebaseId: z.string().optional().describe("Filter by codebase ID"),
            search: z.string().optional().describe("Search in title and description")
        },
        async ({ status, priority, codebaseId, search }) => {
            const params = new URLSearchParams();
            if (status) params.set("status", status);
            if (priority) params.set("priority", priority);
            if (codebaseId) params.set("codebaseId", codebaseId);
            if (search) params.set("search", search);

            const data = (await apiFetch(`/requirements?${params}`)) as {
                requirements: Array<{
                    id: string;
                    title: string;
                    status: string;
                    priority: string | null;
                    description: string | null;
                }>;
                total: number;
            };

            const results = data.requirements.map(r => ({
                id: r.id,
                title: r.title,
                status: r.status,
                priority: r.priority,
                description: r.description
            }));

            return {
                content: [
                    {
                        type: "text" as const,
                        text: JSON.stringify({ requirements: results, total: data.total }, null, 2)
                    }
                ]
            };
        }
    );

    // 2. create_requirement
    server.tool(
        "create_requirement",
        "Create a new requirement with title, description, priority, and BDD steps",
        {
            title: z.string().describe("Requirement title"),
            description: z.string().optional().describe("Requirement description"),
            priority: z
                .enum(["must", "should", "could", "wont"])
                .optional()
                .describe("Requirement priority"),
            steps: z
                .array(
                    z.object({
                        keyword: z.enum(["given", "when", "then", "and", "but"]),
                        text: z.string()
                    })
                )
                .optional()
                .describe("BDD-style steps (Given/When/Then)"),
            codebaseId: z.string().optional().describe("Codebase ID to associate with")
        },
        async ({ title, description, priority, steps, codebaseId }) => {
            const body: Record<string, unknown> = {
                title,
                steps: steps ?? []
            };
            if (description) body.description = description;
            if (priority) body.priority = priority;
            if (codebaseId) body.codebaseId = codebaseId;

            const data = (await apiFetch("/requirements", {
                method: "POST",
                body: JSON.stringify(body)
            })) as { id: string; title: string; status: string };

            return {
                content: [
                    {
                        type: "text" as const,
                        text: `Created requirement "${data.title}" (${data.status}). ID: ${data.id}`
                    }
                ]
            };
        }
    );

    // 3. update_requirement_status
    server.tool(
        "update_requirement_status",
        "Update the status of a requirement (passing, failing, untested)",
        {
            requirementId: z.string().describe("Requirement ID"),
            status: z
                .enum(["passing", "failing", "untested"])
                .describe("New requirement status")
        },
        async ({ requirementId, status }) => {
            const data = (await apiFetch(`/requirements/${requirementId}`, {
                method: "PATCH",
                body: JSON.stringify({ status })
            })) as { id: string; title: string; status: string };

            return {
                content: [
                    {
                        type: "text" as const,
                        text: `Requirement "${data.title}" status updated to ${data.status}. ID: ${data.id}`
                    }
                ]
            };
        }
    );
}
