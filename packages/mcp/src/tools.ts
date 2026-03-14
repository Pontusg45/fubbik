import type { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { z } from "zod";

function getServerUrl(): string {
    return process.env["FUBBIK_SERVER_URL"] ?? "http://localhost:3000";
}

async function apiFetch(path: string, options?: RequestInit): Promise<unknown> {
    const url = `${getServerUrl()}/api${path}`;
    const res = await fetch(url, {
        ...options,
        headers: {
            "Content-Type": "application/json",
            ...options?.headers
        }
    });
    if (!res.ok) {
        const text = await res.text();
        throw new Error(`API ${res.status}: ${text}`);
    }
    return res.json();
}

function truncate(text: string | null | undefined, maxLength: number): string {
    if (!text) return "";
    return text.length > maxLength ? text.slice(0, maxLength) + "..." : text;
}

export function registerTools(server: McpServer): void {
    // 1. search_chunks
    server.tool(
        "search_chunks",
        "Search the fubbik knowledge base for chunks by query, codebase, or tags",
        {
            query: z.string().optional().describe("Search query"),
            codebaseId: z.string().optional().describe("Codebase ID to scope search"),
            tags: z.string().optional().describe("Comma-separated tag names to filter by"),
            limit: z.number().optional().describe("Max results (default 10)")
        },
        async ({ query, codebaseId, tags, limit }) => {
            const params = new URLSearchParams();
            if (query) params.set("search", query);
            if (codebaseId) params.set("codebaseId", codebaseId);
            if (tags) params.set("tags", tags);
            params.set("limit", String(limit ?? 10));

            const data = (await apiFetch(`/chunks?${params}`)) as {
                chunks: Array<{
                    id: string;
                    title: string;
                    type: string | null;
                    content: string | null;
                }>;
            };

            const results = data.chunks.map((c) => ({
                id: c.id,
                title: c.title,
                type: c.type,
                content: truncate(c.content, 500)
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

    // 2. get_chunk
    server.tool(
        "get_chunk",
        "Get full details for a specific chunk by ID",
        {
            id: z.string().describe("Chunk ID")
        },
        async ({ id }) => {
            const data = (await apiFetch(`/chunks/${id}`)) as Record<string, unknown>;

            const result = {
                id: data.id,
                title: data.title,
                content: data.content,
                type: data.type,
                tags: data.tags,
                appliesTo: data.appliesTo,
                fileReferences: data.fileReferences,
                rationale: data.rationale,
                alternatives: data.alternatives,
                consequences: data.consequences,
                connections: data.connections
            };

            return {
                content: [
                    {
                        type: "text" as const,
                        text: JSON.stringify(result, null, 2)
                    }
                ]
            };
        }
    );

    // 3. create_chunk
    server.tool(
        "create_chunk",
        "Create a new knowledge chunk in fubbik",
        {
            title: z.string().describe("Chunk title"),
            content: z.string().describe("Chunk content"),
            type: z.string().optional().describe("Chunk type (e.g. note, decision, pattern)"),
            tags: z.array(z.string()).optional().describe("Tags to apply"),
            codebaseId: z.string().optional().describe("Codebase ID to associate with")
        },
        async ({ title, content, type, tags, codebaseId }) => {
            const body: Record<string, unknown> = { title, content };
            if (type) body.type = type;
            if (tags) body.tags = tags;
            if (codebaseId) body.codebaseIds = [codebaseId];

            const data = (await apiFetch("/chunks", {
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

    // 4. get_conventions
    server.tool(
        "get_conventions",
        "Get convention-type chunks for a codebase (chunks with rationale or convention-related tags)",
        {
            codebaseId: z.string().optional().describe("Codebase ID to scope search")
        },
        async ({ codebaseId }) => {
            const params = new URLSearchParams();
            if (codebaseId) params.set("codebaseId", codebaseId);
            params.set("limit", "100");

            const data = (await apiFetch(`/chunks?${params}`)) as {
                chunks: Array<{
                    id: string;
                    title: string;
                    type: string | null;
                    content: string | null;
                    rationale: string | null;
                    tags: Array<{ name: string }> | null;
                }>;
            };

            const conventionTags = new Set([
                "convention",
                "conventions",
                "pattern",
                "patterns",
                "standard",
                "standards",
                "guideline",
                "guidelines",
                "rule",
                "rules",
                "best-practice",
                "best-practices"
            ]);

            const conventions = data.chunks.filter((c) => {
                if (c.rationale) return true;
                if (c.tags?.some((t) => conventionTags.has(t.name.toLowerCase())))
                    return true;
                return false;
            });

            const results = conventions.map((c) => ({
                id: c.id,
                title: c.title,
                type: c.type,
                content: truncate(c.content, 500),
                rationale: c.rationale,
                tags: c.tags?.map((t) => t.name)
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

    // 5. get_requirements
    server.tool(
        "get_requirements",
        "Get requirements for a codebase",
        {
            codebaseId: z.string().optional().describe("Codebase ID to scope search"),
            status: z
                .string()
                .optional()
                .describe("Filter by status: passing, failing, or untested")
        },
        async ({ codebaseId, status }) => {
            const params = new URLSearchParams();
            if (codebaseId) params.set("codebaseId", codebaseId);
            if (status) params.set("status", status);

            const data = (await apiFetch(`/requirements?${params}`)) as {
                requirements: Array<{
                    id: string;
                    title: string;
                    steps: unknown;
                    status: string;
                }>;
            };

            const results = data.requirements.map((r) => ({
                id: r.id,
                title: r.title,
                steps: r.steps,
                status: r.status
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

    // 6. search_vocabulary
    server.tool(
        "search_vocabulary",
        "Search vocabulary entries for a codebase",
        {
            codebaseId: z.string().describe("Codebase ID (required)"),
            category: z
                .string()
                .optional()
                .describe(
                    "Filter by category: actor, action, target, outcome, state, modifier"
                )
        },
        async ({ codebaseId, category }) => {
            const params = new URLSearchParams();
            params.set("codebaseId", codebaseId);

            const data = (await apiFetch(`/vocabulary?${params}`)) as {
                entries: Array<{
                    id: string;
                    word: string;
                    category: string;
                    expects: string[] | null;
                }>;
            };

            let entries = data.entries;
            if (category) {
                entries = entries.filter(
                    (e) => e.category.toLowerCase() === category.toLowerCase()
                );
            }

            return {
                content: [
                    {
                        type: "text" as const,
                        text: JSON.stringify(entries, null, 2)
                    }
                ]
            };
        }
    );
}
