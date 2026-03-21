import type { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { z } from "zod";
import { apiFetch } from "./api-client.js";

export function registerPlanTools(server: McpServer): void {
    // 1. create_plan
    server.tool(
        "create_plan",
        "Create a new plan with title, description, and optional steps",
        {
            title: z.string().describe("Plan title"),
            description: z.string().optional().describe("Plan description"),
            steps: z
                .array(z.string())
                .optional()
                .describe("List of step descriptions to add")
        },
        async ({ title, description, steps }) => {
            const body: Record<string, unknown> = { title };
            if (description) body.description = description;
            if (steps) {
                body.steps = steps.map((s, i) => ({
                    description: s,
                    order: i
                }));
            }

            const data = (await apiFetch("/plans", {
                method: "POST",
                body: JSON.stringify(body)
            })) as { id: string; title: string };

            return {
                content: [
                    {
                        type: "text" as const,
                        text: JSON.stringify(
                            { id: data.id, title: data.title },
                            null,
                            2
                        )
                    }
                ]
            };
        }
    );

    // 2. list_plans
    server.tool(
        "list_plans",
        "List plans with optional status filter",
        {
            status: z
                .enum(["draft", "active", "completed", "archived"])
                .optional()
                .describe("Filter by plan status")
        },
        async ({ status }) => {
            const params = new URLSearchParams();
            if (status) params.set("status", status);

            const data = (await apiFetch(`/plans?${params}`)) as {
                plans: Array<{
                    id: string;
                    title: string;
                    status: string;
                    description: string | null;
                }>;
            };

            const results = data.plans.map((p) => ({
                id: p.id,
                title: p.title,
                status: p.status,
                description: p.description
            }));

            return {
                content: [
                    {
                        type: "text" as const,
                        text: JSON.stringify(results, null, 2)
                    }
                ]
            };
        }
    );

    // 3. get_plan
    server.tool(
        "get_plan",
        "Get plan details with steps and progress",
        {
            planId: z.string().describe("Plan ID")
        },
        async ({ planId }) => {
            const data = (await apiFetch(`/plans/${planId}`)) as Record<
                string,
                unknown
            >;

            return {
                content: [
                    {
                        type: "text" as const,
                        text: JSON.stringify(data, null, 2)
                    }
                ]
            };
        }
    );

    // 4. update_plan_step
    server.tool(
        "update_plan_step",
        "Update a step's status or add a note",
        {
            planId: z.string().describe("Plan ID"),
            stepId: z.string().describe("Step ID"),
            status: z.string().optional().describe("New step status"),
            note: z.string().optional().describe("Note to add to the step")
        },
        async ({ planId, stepId, status, note }) => {
            const body: Record<string, unknown> = {};
            if (status) body.status = status;
            if (note) body.note = note;

            const data = (await apiFetch(`/plans/${planId}/steps/${stepId}`, {
                method: "PATCH",
                body: JSON.stringify(body)
            })) as { id: string; description: string; status: string };

            return {
                content: [
                    {
                        type: "text" as const,
                        text: `Step updated: ${data.description} (${data.status})`
                    }
                ]
            };
        }
    );

    // 5. add_plan_step
    server.tool(
        "add_plan_step",
        "Add a new step to a plan",
        {
            planId: z.string().describe("Plan ID"),
            description: z.string().describe("Step description")
        },
        async ({ planId, description }) => {
            const data = (await apiFetch(`/plans/${planId}/steps`, {
                method: "POST",
                body: JSON.stringify({ description })
            })) as { id: string; description: string };

            return {
                content: [
                    {
                        type: "text" as const,
                        text: JSON.stringify(
                            { id: data.id, description: data.description },
                            null,
                            2
                        )
                    }
                ]
            };
        }
    );

    // 6. complete_plan
    server.tool(
        "complete_plan",
        "Mark a plan as completed",
        {
            planId: z.string().describe("Plan ID")
        },
        async ({ planId }) => {
            const data = (await apiFetch(`/plans/${planId}`, {
                method: "PATCH",
                body: JSON.stringify({ status: "completed" })
            })) as { id: string; title: string; status: string };

            return {
                content: [
                    {
                        type: "text" as const,
                        text: `Plan completed: ${data.title} (${data.id})`
                    }
                ]
            };
        }
    );
}
