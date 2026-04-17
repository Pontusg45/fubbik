import { eq } from "drizzle-orm";

import { requirement, requirementChunk, type RequirementStep } from "../../schema/requirement";
import { uuid } from "../factories";
import type { SeedContext } from "../context";

export async function seed(ctx: SeedContext): Promise<void> {
    const codebaseId = ctx.ids.codebases["fubbik"];
    if (!codebaseId) throw new Error("requirements needs fubbik codebase");

    const rows: Array<{
        name: string;
        title: string;
        description: string;
        steps: RequirementStep[];
        priority: string;
        useCase?: string;
        chunkNames?: string[];
    }> = [
        {
            name: "onboarding-graph",
            title: "New dev can render the graph in under a minute",
            description: "A new engineer can clone, seed, and render a meaningful graph without consulting a teammate.",
            steps: [
                { keyword: "given", text: "a fresh clone with Postgres running" },
                { keyword: "when",  text: "the dev runs pnpm seed && pnpm dev" },
                { keyword: "then",  text: "visiting /graph shows at least 5 chunks and 3 connections" }
            ],
            priority: "must",
            useCase: "onboarding",
            chunkNames: ["arch", "seed-system"]
        },
        {
            name: "catalog-driven-types",
            title: "Custom chunk types land without code changes",
            description: "A user can add a per-codebase chunk type via /settings/vocabulary and the type appears in filter/graph/legend without a deploy.",
            steps: [
                { keyword: "given", text: "the user is on /settings/vocabulary" },
                { keyword: "when",  text: "they create a chunk type \"runbook\"" },
                { keyword: "then",  text: "the graph legend and filter dialog include \"runbook\" within 24h of cache staleness" }
            ],
            priority: "should",
            useCase: "decisions",
            chunkNames: ["catalog-pattern"]
        }
    ];

    for (const r of rows) {
        const id = uuid();
        await ctx.db.insert(requirement).values({
            id,
            title: r.title,
            description: r.description,
            steps: r.steps,
            priority: r.priority,
            status: "untested",
            userId: ctx.userId,
            codebaseId,
            useCaseId: r.useCase ? (ctx.ids.useCases[r.useCase] ?? null) : null
        });
        ctx.ids.requirements[r.name] = id;

        if (r.chunkNames) {
            for (const cname of r.chunkNames) {
                const chunkId = ctx.ids.chunks[cname];
                if (!chunkId) throw new Error(`requirement "${r.name}" refs unknown chunk "${cname}"`);
                await ctx.db.insert(requirementChunk).values({ requirementId: id, chunkId });
            }
        }
    }
    ctx.counters["requirements"] = rows.length;
}

export async function reset(ctx: SeedContext): Promise<void> {
    await ctx.db.delete(requirement).where(eq(requirement.userId, ctx.userId));
}
