import { eq } from "drizzle-orm";

import {
    behaviorMatrix,
    behaviorDimension,
    behaviorRule,
    behaviorCell,
    behaviorCellRequirement
} from "../../schema/behavior-matrix";
import { uuid } from "../factories";
import type { SeedContext } from "../context";

export async function seed(ctx: SeedContext): Promise<void> {
    const codebaseId = ctx.ids.codebases["fubbik"];
    if (!codebaseId) throw new Error("matrices needs fubbik codebase");

    // --- Matrix 1: Domain Invariants (rules × entities) ---

    const invariantId = uuid();
    await ctx.db.insert(behaviorMatrix).values({
        id: invariantId,
        name: "Domain Invariants",
        layer: "invariant",
        description: "Rules that must always hold across domain entities",
        codebaseId,
        userId: ctx.userId
    });
    ctx.ids.matrices["invariants"] = invariantId;

    const invariantDims = [
        { name: "Chunk", key: "chunk" },
        { name: "Plan", key: "plan" },
        { name: "Requirement", key: "requirement" },
        { name: "Feature", key: "feature" },
        { name: "Connection", key: "connection" }
    ];
    for (let i = 0; i < invariantDims.length; i++) {
        const dim = invariantDims[i]!;
        const id = uuid();
        await ctx.db.insert(behaviorDimension).values({
            id,
            matrixId: invariantId,
            name: dim.name,
            order: i
        });
        ctx.ids.matrixDimensions[dim.key] = id;
    }

    const invariantRules = [
        { title: "Title is required", category: "Validation", key: "title-required" },
        { title: "Cascade deletes to children", category: "Lifecycle", key: "cascade-delete" },
        { title: "Must belong to a user", category: "Auth", key: "user-ownership" },
        { title: "Changes create version history", category: "Lifecycle", key: "version-history" }
    ];
    for (let i = 0; i < invariantRules.length; i++) {
        const rule = invariantRules[i]!;
        const id = uuid();
        await ctx.db.insert(behaviorRule).values({
            id,
            matrixId: invariantId,
            title: rule.title,
            category: rule.category,
            order: i
        });
        ctx.ids.matrixRules[rule.key] = id;
    }

    // --- Matrix 2: API Contracts (capabilities × actors) ---

    const contractId = uuid();
    await ctx.db.insert(behaviorMatrix).values({
        id: contractId,
        name: "API Contracts",
        layer: "contract",
        description: "Capabilities available to each actor type",
        codebaseId,
        userId: ctx.userId
    });
    ctx.ids.matrices["contracts"] = contractId;

    const contractDims = [
        { name: "User", key: "user" },
        { name: "AI Agent", key: "ai-agent" },
        { name: "System", key: "system" },
        { name: "API Consumer", key: "api-consumer" }
    ];
    for (let i = 0; i < contractDims.length; i++) {
        const dim = contractDims[i]!;
        const id = uuid();
        await ctx.db.insert(behaviorDimension).values({
            id,
            matrixId: contractId,
            name: dim.name,
            order: i
        });
        ctx.ids.matrixDimensions[dim.key] = id;
    }

    const contractRules = [
        { title: "CRUD operations", category: "Data", key: "crud" },
        { title: "Search by text and tags", category: "Discovery", key: "search" },
        { title: "Semantic search", category: "Discovery", key: "semantic-search" },
        { title: "Bulk operations", category: "Data", key: "bulk-ops" },
        { title: "Export to external formats", category: "Integration", key: "export" }
    ];
    for (let i = 0; i < contractRules.length; i++) {
        const rule = contractRules[i]!;
        const id = uuid();
        await ctx.db.insert(behaviorRule).values({
            id,
            matrixId: contractId,
            title: rule.title,
            category: rule.category,
            order: i
        });
        ctx.ids.matrixRules[rule.key] = id;
    }

    // --- Cells: mark which intersections are relevant ---

    const cellDefs: Array<{ key: string; rule: string; dim: string }> = [
        // "Title is required" → Chunk, Plan, Requirement
        { key: "title-chunk", rule: "title-required", dim: "chunk" },
        { key: "title-plan", rule: "title-required", dim: "plan" },
        { key: "title-req", rule: "title-required", dim: "requirement" },
        // "Cascade deletes" → Plan, Feature
        { key: "cascade-plan", rule: "cascade-delete", dim: "plan" },
        { key: "cascade-feature", rule: "cascade-delete", dim: "feature" },
        // "User ownership" → all entities
        { key: "owner-chunk", rule: "user-ownership", dim: "chunk" },
        { key: "owner-plan", rule: "user-ownership", dim: "plan" },
        { key: "owner-req", rule: "user-ownership", dim: "requirement" },
        { key: "owner-feature", rule: "user-ownership", dim: "feature" },
        // "Version history" → Chunk only
        { key: "version-chunk", rule: "version-history", dim: "chunk" },
        // "CRUD" → User, AI Agent, API Consumer
        { key: "crud-user", rule: "crud", dim: "user" },
        { key: "crud-ai", rule: "crud", dim: "ai-agent" },
        { key: "crud-api", rule: "crud", dim: "api-consumer" },
        // "Search" → User, AI Agent, API Consumer
        { key: "search-user", rule: "search", dim: "user" },
        { key: "search-ai", rule: "search", dim: "ai-agent" },
        { key: "search-api", rule: "search", dim: "api-consumer" },
        // "Semantic search" → User, AI Agent
        { key: "semantic-user", rule: "semantic-search", dim: "user" },
        { key: "semantic-ai", rule: "semantic-search", dim: "ai-agent" },
        // "Bulk ops" → User, API Consumer
        { key: "bulk-user", rule: "bulk-ops", dim: "user" },
        { key: "bulk-api", rule: "bulk-ops", dim: "api-consumer" },
        // "Export" → User, AI Agent (gap: System is intentionally unspecified)
        { key: "export-user", rule: "export", dim: "user" },
        { key: "export-ai", rule: "export", dim: "ai-agent" }
    ];

    for (const c of cellDefs) {
        const ruleId = ctx.ids.matrixRules[c.rule];
        const dimId = ctx.ids.matrixDimensions[c.dim];
        if (!ruleId || !dimId) throw new Error(`missing rule "${c.rule}" or dim "${c.dim}"`);
        const id = uuid();
        await ctx.db.insert(behaviorCell).values({ id, ruleId, dimensionId: dimId });
        ctx.ids.matrixCells[c.key] = id;
    }

    // --- Link some cells to requirements for specified/unspecified variety ---

    const reqLinks: Array<{ cell: string; req: string }> = [
        { cell: "crud-user", req: "onboarding-graph" },
        { cell: "crud-ai", req: "onboarding-graph" },
        { cell: "search-user", req: "catalog-driven-types" },
        { cell: "search-ai", req: "catalog-driven-types" }
    ];

    for (const link of reqLinks) {
        const cellId = ctx.ids.matrixCells[link.cell];
        const requirementId = ctx.ids.requirements[link.req];
        if (!cellId || !requirementId) continue;
        await ctx.db.insert(behaviorCellRequirement).values({ cellId, requirementId });
    }

    ctx.counters["behavior_matrices"] = 2;
    ctx.counters["behavior_dimensions"] = invariantDims.length + contractDims.length;
    ctx.counters["behavior_rules"] = invariantRules.length + contractRules.length;
    ctx.counters["behavior_cells"] = cellDefs.length;
}

export async function reset(ctx: SeedContext): Promise<void> {
    await ctx.db.delete(behaviorCellRequirement);
    await ctx.db.delete(behaviorCell);
    await ctx.db.delete(behaviorRule);
    await ctx.db.delete(behaviorDimension);
    await ctx.db.delete(behaviorMatrix).where(eq(behaviorMatrix.userId, ctx.userId));
}
