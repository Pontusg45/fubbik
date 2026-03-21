import type { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { z } from "zod";
import { apiFetch } from "./api-client.js";

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
}
