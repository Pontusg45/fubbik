import type { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { z } from "zod";
import { apiFetch } from "./api-client.js";
import type { McpPlugin } from "./plugin.js";

export function registerTaskTools(server: McpServer): void {
    server.tool(
        "add_task",
        "Add a quick task for tracking",
        {
            title: z.string(),
            description: z.string().optional()
        },
        async ({ title, description }) => {
            const task = (await apiFetch("/tasks", {
                method: "POST",
                body: JSON.stringify({ title, description })
            })) as { id: string };

            return {
                content: [
                    {
                        type: "text" as const,
                        text: `Task created: "${title}" (ID: ${task.id})`
                    }
                ]
            };
        }
    );

    server.tool("list_tasks", "List open tasks", {}, async () => {
        const tasks = (await apiFetch("/tasks")) as any[];
        if (!Array.isArray(tasks) || tasks.length === 0) {
            return {
                content: [{ type: "text" as const, text: "No open tasks" }]
            };
        }
        const list = tasks
            .map((t: any) => `- [${t.status}] ${t.title} (${t.id})`)
            .join("\n");
        return {
            content: [
                {
                    type: "text" as const,
                    text: `${tasks.length} open task(s):\n${list}`
                }
            ]
        };
    });

    server.tool(
        "complete_task",
        "Complete a task",
        {
            taskId: z.string(),
            note: z.string().optional()
        },
        async ({ taskId, note }) => {
            await apiFetch(`/tasks/${taskId}/complete`, {
                method: "POST",
                body: JSON.stringify({ note })
            });
            return {
                content: [{ type: "text" as const, text: "Task completed" }]
            };
        }
    );
}

export const taskPlugin: McpPlugin = {
    name: "tasks",
    description: "Quick task management tools for AI agents",
    register: registerTaskTools
};
