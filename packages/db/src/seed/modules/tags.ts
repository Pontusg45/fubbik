/**
 * Tags + tag types.
 *
 * Tag types group related tags under a color (e.g., all "feature:*" tags share
 * the "feature" color). Tag names get registered in ctx.ids.tags so other
 * modules can refer to them by name.
 */

import { eq } from "drizzle-orm";

import { tag, tagType } from "../../schema/tag";
import { makeTag, makeTagType } from "../factories";
import type { SeedContext } from "../context";

const TAG_TYPES = [
    { slug: "feature", color: "#3b82f6" },
    { slug: "techstack", color: "#10b981" },
    { slug: "infrastructure", color: "#f59e0b" },
    { slug: "pattern", color: "#8b5cf6" },
    { slug: "documentation", color: "#ef4444" },
    { slug: "system", color: "#64748b" },
    { slug: "ai", color: "#ec4899" },
    { slug: "integration", color: "#14b8a6" }
] as const;

const TAGS: Array<{ name: string; type: string }> = [
    // feature
    { name: "authentication", type: "feature" },
    { name: "search", type: "feature" },
    { name: "enrichment", type: "feature" },
    { name: "chunks", type: "feature" },
    { name: "connections", type: "feature" },
    { name: "graph", type: "feature" },
    { name: "plans", type: "feature" },
    { name: "requirements", type: "feature" },
    { name: "use-cases", type: "feature" },
    { name: "vocabulary", type: "feature" },
    { name: "templates", type: "feature" },
    { name: "workspaces", type: "feature" },
    { name: "codebases", type: "feature" },
    { name: "health", type: "feature" },
    { name: "staleness", type: "feature" },
    { name: "activity", type: "feature" },
    { name: "timeline", type: "feature" },
    { name: "density", type: "feature" },
    { name: "mermaid-export", type: "feature" },
    { name: "neighbors", type: "feature" },

    // techstack
    { name: "typescript", type: "techstack" },
    { name: "elysia", type: "techstack" },
    { name: "tanstack", type: "techstack" },
    { name: "react-flow", type: "techstack" },
    { name: "drizzle", type: "techstack" },
    { name: "effect", type: "techstack" },
    { name: "base-ui", type: "techstack" },
    { name: "tailwind", type: "techstack" },
    { name: "eden-treaty", type: "techstack" },
    { name: "better-auth", type: "techstack" },
    { name: "bun", type: "techstack" },
    { name: "pgvector", type: "techstack" },

    // infrastructure
    { name: "postgres", type: "infrastructure" },
    { name: "docker", type: "infrastructure" },
    { name: "turborepo", type: "infrastructure" },
    { name: "ci", type: "infrastructure" },
    { name: "caddy", type: "infrastructure" },

    // pattern
    { name: "repository-pattern", type: "pattern" },
    { name: "service-layer", type: "pattern" },
    { name: "error-handling", type: "pattern" },
    { name: "catalog-driven", type: "pattern" },
    { name: "render-prop", type: "pattern" },
    { name: "fixture-dsl", type: "pattern" },
    { name: "typed-errors", type: "pattern" },
    { name: "first-wins", type: "pattern" },

    // documentation
    { name: "architecture", type: "documentation" },
    { name: "onboarding", type: "documentation" },
    { name: "reference", type: "documentation" },
    { name: "convention", type: "documentation" },

    // system (meta)
    { name: "seed-system", type: "system" },
    { name: "self-documenting", type: "system" },
    { name: "performance", type: "system" },
    { name: "migration", type: "system" },

    // ai
    { name: "ollama", type: "ai" },
    { name: "embeddings", type: "ai" },
    { name: "semantic-search", type: "ai" },
    { name: "duplicate-detection", type: "ai" },
    { name: "context-export", type: "ai" },
    { name: "enrichment-ai", type: "ai" },

    // integration
    { name: "mcp", type: "integration" },
    { name: "vscode", type: "integration" },
    { name: "cli", type: "integration" },
    { name: "claude-md", type: "integration" },
    { name: "git-hooks", type: "integration" }
];

export async function seed(ctx: SeedContext): Promise<void> {
    for (const tt of TAG_TYPES) {
        const row = makeTagType({
            id: `seed-tt-${tt.slug}`,
            name: tt.slug,
            color: tt.color,
            userId: ctx.userId
        });
        await ctx.db.insert(tagType).values(row);
        ctx.ids.tagTypes[tt.slug] = row.id!;
    }
    ctx.counters["tag_types"] = TAG_TYPES.length;

    for (const t of TAGS) {
        const tagTypeId = ctx.ids.tagTypes[t.type];
        if (!tagTypeId) throw new Error(`Unknown tag type "${t.type}" for tag "${t.name}"`);
        const row = makeTag({
            id: `seed-tag-${t.name}`,
            name: t.name,
            tagTypeId,
            userId: ctx.userId
        });
        await ctx.db.insert(tag).values(row);
        ctx.ids.tags[t.name] = row.id!;
    }
    ctx.counters["tags"] = TAGS.length;
}

export async function reset(ctx: SeedContext): Promise<void> {
    await ctx.db.delete(tag).where(eq(tag.userId, ctx.userId));
    await ctx.db.delete(tagType).where(eq(tagType.userId, ctx.userId));
}
