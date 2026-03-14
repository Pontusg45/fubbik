import {
    getTagsForChunks,
    listChunks as listChunksRepo
} from "@fubbik/db/repository";
import { Effect } from "effect";

type InstructionFormat = "claude" | "agents" | "cursor";

interface GenerateInstructionsQuery {
    format?: InstructionFormat;
}

interface CategorizedChunks {
    overview: ChunkWithTags[];
    architecture: ChunkWithTags[];
    conventions: ChunkWithTags[];
    commands: ChunkWithTags[];
    other: ChunkWithTags[];
}

interface ChunkWithTags {
    id: string;
    title: string;
    content: string;
    type: string;
    rationale: string | null;
    alternatives: string[] | null;
    consequences: string | null;
    tags: string[];
}

export function generateInstructions(userId: string, codebaseId: string, query: GenerateInstructionsQuery) {
    const format = query.format ?? "claude";

    return listChunksRepo({
        userId,
        codebaseId,
        limit: 500,
        offset: 0
    }).pipe(
        Effect.flatMap(({ chunks }) => {
            const chunkIds = chunks.map(c => c.id);
            return Effect.all({
                chunks: Effect.succeed(chunks),
                tags: getTagsForChunks(chunkIds)
            });
        }),
        Effect.map(({ chunks, tags }) => {
            // Build tag map
            const tagMap = new Map<string, string[]>();
            for (const t of tags) {
                const existing = tagMap.get(t.chunkId) ?? [];
                existing.push(t.tagName);
                tagMap.set(t.chunkId, existing);
            }

            // Enrich chunks with tags
            const enriched: ChunkWithTags[] = chunks.map(c => ({
                id: c.id,
                title: c.title,
                content: c.content,
                type: c.type,
                rationale: c.rationale,
                alternatives: c.alternatives,
                consequences: c.consequences,
                tags: tagMap.get(c.id) ?? []
            }));

            // Categorize
            const categorized = categorizeChunks(enriched);

            // Format output
            switch (format) {
                case "claude":
                    return { format, content: formatClaude(categorized) };
                case "agents":
                    return { format, content: formatAgents(categorized) };
                case "cursor":
                    return { format, content: formatCursor(categorized) };
            }
        })
    );
}

function categorizeChunks(chunks: ChunkWithTags[]): CategorizedChunks {
    const result: CategorizedChunks = {
        overview: [],
        architecture: [],
        conventions: [],
        commands: [],
        other: []
    };

    for (const chunk of chunks) {
        const lowerTags = chunk.tags.map(t => t.toLowerCase());
        const lowerContent = chunk.content.toLowerCase();
        const lowerTitle = chunk.title.toLowerCase();

        if (lowerTags.includes("architecture") || lowerTags.includes("core")) {
            result.overview.push(chunk);
        }

        if (chunk.type === "document" && lowerTags.includes("architecture")) {
            result.architecture.push(chunk);
        } else if (chunk.type === "document" && (lowerTitle.includes("architecture") || lowerTitle.includes("pattern"))) {
            result.architecture.push(chunk);
        }

        if (chunk.rationale || lowerTags.includes("convention") || lowerTags.includes("conventions")) {
            result.conventions.push(chunk);
        }

        if (lowerContent.includes("pnpm ") || lowerContent.includes("npm ") || lowerContent.includes("bun ") || lowerTags.includes("commands") || lowerTags.includes("scripts")) {
            result.commands.push(chunk);
        }

        // If not categorized in any of the above
        const isInAny = result.overview.includes(chunk)
            || result.architecture.includes(chunk)
            || result.conventions.includes(chunk)
            || result.commands.includes(chunk);

        if (!isInAny) {
            result.other.push(chunk);
        }
    }

    return result;
}

function formatClaude(cat: CategorizedChunks): string {
    const sections: string[] = [];
    sections.push("# CLAUDE.md");
    sections.push("");
    sections.push("This file provides context about the project for AI assistants.");
    sections.push("");

    if (cat.overview.length > 0) {
        sections.push("## Project Overview");
        sections.push("");
        for (const c of cat.overview) {
            sections.push(c.content);
            sections.push("");
        }
    }

    if (cat.architecture.length > 0) {
        sections.push("## Architecture");
        sections.push("");
        for (const c of cat.architecture) {
            sections.push(`### ${c.title}`);
            sections.push("");
            sections.push(c.content);
            sections.push("");
        }
    }

    if (cat.conventions.length > 0) {
        sections.push("## Conventions");
        sections.push("");
        for (const c of cat.conventions) {
            sections.push(`### ${c.title}`);
            sections.push("");
            sections.push(c.content);
            if (c.rationale) {
                sections.push("");
                sections.push(`**Rationale:** ${c.rationale}`);
            }
            if (c.alternatives && c.alternatives.length > 0) {
                sections.push("");
                sections.push(`**Alternatives considered:** ${c.alternatives.join(", ")}`);
            }
            if (c.consequences) {
                sections.push("");
                sections.push(`**Consequences:** ${c.consequences}`);
            }
            sections.push("");
        }
    }

    if (cat.commands.length > 0) {
        sections.push("## Commands");
        sections.push("");
        for (const c of cat.commands) {
            sections.push(`### ${c.title}`);
            sections.push("");
            sections.push(c.content);
            sections.push("");
        }
    }

    if (cat.other.length > 0) {
        sections.push("## Additional Context");
        sections.push("");
        for (const c of cat.other) {
            sections.push(`### ${c.title}`);
            sections.push("");
            sections.push(c.content);
            sections.push("");
        }
    }

    return sections.join("\n");
}

function formatAgents(cat: CategorizedChunks): string {
    const sections: string[] = [];
    sections.push("# AGENTS.md");
    sections.push("");
    sections.push("Instructions for AI agents working on this project.");
    sections.push("");

    if (cat.conventions.length > 0) {
        sections.push("## Rules");
        sections.push("");
        for (const c of cat.conventions) {
            sections.push(`- **${c.title}**: ${c.content.split("\n")[0]}`);
            if (c.rationale) {
                sections.push(`  - Rationale: ${c.rationale}`);
            }
        }
        sections.push("");
    }

    if (cat.architecture.length > 0) {
        sections.push("## Architecture");
        sections.push("");
        for (const c of cat.architecture) {
            sections.push(`### ${c.title}`);
            sections.push("");
            sections.push(c.content);
            sections.push("");
        }
    }

    if (cat.commands.length > 0) {
        sections.push("## Available Commands");
        sections.push("");
        for (const c of cat.commands) {
            sections.push(`### ${c.title}`);
            sections.push("");
            sections.push(c.content);
            sections.push("");
        }
    }

    if (cat.other.length > 0) {
        sections.push("## Additional Context");
        sections.push("");
        for (const c of cat.other) {
            sections.push(`### ${c.title}`);
            sections.push("");
            sections.push(c.content);
            sections.push("");
        }
    }

    return sections.join("\n");
}

function formatCursor(cat: CategorizedChunks): string {
    const sections: string[] = [];
    sections.push("# .cursorrules");
    sections.push("");

    const allConventions = cat.conventions;
    if (allConventions.length > 0) {
        sections.push("## Coding Conventions");
        sections.push("");
        for (const c of allConventions) {
            sections.push(`- ${c.title}: ${c.content.split("\n")[0]}`);
        }
        sections.push("");
    }

    if (cat.architecture.length > 0) {
        sections.push("## Architecture");
        sections.push("");
        for (const c of cat.architecture) {
            sections.push(`### ${c.title}`);
            sections.push(c.content);
            sections.push("");
        }
    }

    if (cat.commands.length > 0) {
        sections.push("## Commands");
        sections.push("");
        for (const c of cat.commands) {
            sections.push(c.content);
            sections.push("");
        }
    }

    return sections.join("\n");
}
