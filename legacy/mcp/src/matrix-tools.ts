import type { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { z } from "zod";

import { apiFetch } from "./api-client.js";
import type { McpPlugin } from "./plugin.js";

export function registerMatrixTools(server: McpServer): void {
    server.tool(
        "list_matrices",
        "List behavioral specification matrices",
        {
            spaceId: z.string().optional(),
            layer: z.enum(["invariant", "contract"]).optional()
        },
        async ({ spaceId, layer }) => {
            const query = new URLSearchParams();
            if (spaceId) query.set("spaceId", spaceId);
            if (layer) query.set("layer", layer);
            const qs = query.toString();
            const result = await apiFetch(`/matrices${qs ? `?${qs}` : ""}`);
            return {
                content: [{ type: "text" as const, text: JSON.stringify(result, null, 2) }]
            };
        }
    );

    server.tool(
        "get_matrix_view",
        "Get the full behavioral matrix grid with computed cell statuses (specified/unspecified/violated)",
        { matrixId: z.string().describe("Matrix ID") },
        async ({ matrixId }) => {
            const result = await apiFetch(`/matrices/${matrixId}/view`);
            return {
                content: [{ type: "text" as const, text: JSON.stringify(result, null, 2) }]
            };
        }
    );

    server.tool(
        "create_matrix",
        "Create a new behavioral specification matrix",
        {
            name: z.string().describe("Matrix name"),
            layer: z.enum(["invariant", "contract"]).describe("invariant (rules x entities) or contract (capabilities x actors)"),
            description: z.string().optional(),
            spaceId: z.string().optional()
        },
        async ({ name, layer, description, spaceId }) => {
            const result = await apiFetch("/matrices", {
                method: "POST",
                body: JSON.stringify({ name, layer, description, spaceId })
            });
            return {
                content: [{ type: "text" as const, text: JSON.stringify(result, null, 2) }]
            };
        }
    );

    server.tool(
        "add_dimension",
        "Add a column (dimension) to a matrix",
        {
            matrixId: z.string().describe("Matrix ID"),
            name: z.string().describe("Dimension name, e.g. 'Chunk' or 'AI Agent'")
        },
        async ({ matrixId, name }) => {
            const result = await apiFetch(`/matrices/${matrixId}/dimensions`, {
                method: "POST",
                body: JSON.stringify({ name })
            });
            return {
                content: [{ type: "text" as const, text: JSON.stringify(result, null, 2) }]
            };
        }
    );

    server.tool(
        "add_rule",
        "Add a row (rule) to a matrix",
        {
            matrixId: z.string().describe("Matrix ID"),
            title: z.string().describe("Rule title, e.g. 'Cascade deletes to children'"),
            description: z.string().optional(),
            category: z.string().optional().describe("Category for grouping rules"),
            rationale: z.string().optional().describe("Why this rule exists"),
            alternatives: z.string().optional().describe("Alternatives that were considered"),
            consequences: z.string().optional().describe("Consequences of this rule"),
            counterexample: z.string().optional().describe("A counterexample illustrating a violation")
        },
        async ({ matrixId, title, description, category, rationale, alternatives, consequences, counterexample }) => {
            const result = await apiFetch(`/matrices/${matrixId}/rules`, {
                method: "POST",
                body: JSON.stringify({ title, description, category, rationale, alternatives, consequences, counterexample })
            });
            return {
                content: [{ type: "text" as const, text: JSON.stringify(result, null, 2) }]
            };
        }
    );

    server.tool(
        "toggle_cell",
        "Toggle a cell (intersection of rule and dimension). Creates cell if absent, deletes if present and has no linked requirements.",
        {
            matrixId: z.string().describe("Matrix ID"),
            ruleId: z.string().describe("Rule ID"),
            dimensionId: z.string().describe("Dimension ID")
        },
        async ({ matrixId, ruleId, dimensionId }) => {
            const result = await apiFetch(`/matrices/${matrixId}/cells`, {
                method: "PUT",
                body: JSON.stringify({ ruleId, dimensionId })
            });
            return {
                content: [{ type: "text" as const, text: JSON.stringify(result, null, 2) }]
            };
        }
    );

    server.tool(
        "link_cell_requirement",
        "Link a BDD requirement to a matrix cell",
        {
            matrixId: z.string().describe("Matrix ID"),
            cellId: z.string().describe("Cell ID"),
            requirementId: z.string().describe("Requirement ID to link")
        },
        async ({ matrixId, cellId, requirementId }) => {
            const result = await apiFetch(`/matrices/${matrixId}/cells/${cellId}/requirements`, {
                method: "POST",
                body: JSON.stringify({ requirementId })
            });
            return {
                content: [{ type: "text" as const, text: JSON.stringify(result, null, 2) }]
            };
        }
    );

    server.tool(
        "link_cell_code",
        "Link a code reference (file, symbol, or test) to a matrix cell so the behavior is traceable to its implementation",
        {
            matrixId: z.string().describe("Matrix ID"),
            cellId: z.string().describe("Cell ID"),
            kind: z.enum(["file", "symbol", "test"]).describe("Kind of code reference"),
            ref: z.string().describe("The reference: a file path, symbol name, or test id")
        },
        async ({ matrixId, cellId, kind, ref }) => {
            const result = await apiFetch(`/matrices/${matrixId}/cells/${cellId}/code`, {
                method: "POST",
                body: JSON.stringify({ kind, ref })
            });
            return {
                content: [{ type: "text" as const, text: JSON.stringify(result, null, 2) }]
            };
        }
    );

    server.tool(
        "record_test_result",
        "Record a test result (pass/fail) for a matrix cell",
        {
            matrixId: z.string().describe("Matrix ID"),
            cellId: z.string().describe("Cell ID"),
            testRef: z.string().describe("Test reference"),
            status: z.enum(["pass", "fail"]).describe("Test outcome"),
            detail: z.string().optional().describe("Optional detail, e.g. failure message")
        },
        async ({ matrixId, cellId, testRef, status, detail }) => {
            const result = await apiFetch(`/matrices/${matrixId}/cells/${cellId}/test-results`, {
                method: "POST",
                body: JSON.stringify({ testRef, status, detail })
            });
            return {
                content: [{ type: "text" as const, text: JSON.stringify(result, null, 2) }]
            };
        }
    );

    server.tool(
        "get_rule_history",
        "Get the version history of a rule (newest first), showing how its title, rationale, and other fields changed over time",
        {
            matrixId: z.string().describe("Matrix ID"),
            ruleId: z.string().describe("Rule ID")
        },
        async ({ matrixId, ruleId }) => {
            const result = await apiFetch(`/matrices/${matrixId}/rules/${ruleId}/history`);
            return {
                content: [{ type: "text" as const, text: JSON.stringify(result, null, 2) }]
            };
        }
    );

    server.tool(
        "get_behaviors_for_file",
        "Reverse lookup: find the behavioral rules that govern a given file, including their rationale and counterexamples",
        { path: z.string().describe("File path to look up") },
        async ({ path }) => {
            const result = await apiFetch(`/matrices/behaviors-for-file?path=${encodeURIComponent(path)}`);
            return {
                content: [{ type: "text" as const, text: JSON.stringify(result, null, 2) }]
            };
        }
    );

    server.tool(
        "get_matrix_gaps",
        "Get only unspecified and violated cells — shows what is missing or broken",
        { matrixId: z.string().describe("Matrix ID") },
        async ({ matrixId }) => {
            const view = (await apiFetch(`/matrices/${matrixId}/view`)) as {
                matrix: { name: string };
                dimensions: Array<{ id: string; name: string }>;
                rules: Array<{ id: string; title: string }>;
                cells: Record<string, { status: string; requirementCount: number } | null>;
                summary: {
                    specified: number;
                    unspecified: number;
                    violated: number;
                    total: number;
                };
            };

            const dimMap = new Map(view.dimensions.map(d => [d.id, d.name]));
            const ruleMap = new Map(view.rules.map(r => [r.id, r.title]));

            const gaps: Array<{ rule: string; dimension: string; status: string }> = [];
            for (const [key, cell] of Object.entries(view.cells)) {
                if (cell && (cell.status === "unspecified" || cell.status === "violated")) {
                    const parts = key.split(":");
                    const ruleId = parts[0] ?? "";
                    const dimId = parts[1] ?? "";
                    gaps.push({
                        rule: ruleMap.get(ruleId) ?? ruleId,
                        dimension: dimMap.get(dimId) ?? dimId,
                        status: cell.status
                    });
                }
            }

            return {
                content: [
                    {
                        type: "text" as const,
                        text: JSON.stringify({ matrix: view.matrix.name, summary: view.summary, gaps }, null, 2)
                    }
                ]
            };
        }
    );
}

export const matrixPlugin: McpPlugin = {
    name: "matrix",
    description: "Behavioral specification matrix tools",
    register: registerMatrixTools
};
