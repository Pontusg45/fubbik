import { eq } from "drizzle-orm";

import { plan, planAnalyzeItem, planRequirement, planTask } from "../../schema/plan";
import { uuid } from "../factories";
import type { SeedContext } from "../context";

export async function seed(ctx: SeedContext): Promise<void> {
    const codebaseId = ctx.ids.codebases["fubbik"];
    if (!codebaseId) throw new Error("plans needs fubbik codebase");

    // Demo plan — demonstrates linking a plan to requirements, tasks, and analyze items.
    const planId = uuid();
    await ctx.db.insert(plan).values({
        id: planId,
        title: "Round 2: per-codebase vocabulary CRUD UI",
        description: "Ship /settings/vocabulary editors for chunk types + connection relations.",
        status: "in_progress",
        userId: ctx.userId,
        codebaseId,
        metadata: { tokenEstimate: 15000, effortHours: 6 }
    });
    ctx.ids.plans["vocab-crud"] = planId;

    // Link requirement if present.
    const reqId = ctx.ids.requirements["catalog-driven-types"];
    if (reqId) {
        await ctx.db.insert(planRequirement).values({ planId, requirementId: reqId });
    }

    const tasks: Array<{ name: string; title: string; order: number; status?: string }> = [
        { name: "schema",       title: "Add chunk_type + connection_relation tables + seed builtins", order: 0, status: "done" },
        { name: "read-api",     title: "Read-through API + react-query hooks", order: 1, status: "done" },
        { name: "ui-catalog",   title: "Use catalog in graph-node icon + chunk-type-icon", order: 2, status: "done" },
        { name: "fk-cutover",   title: "FK cutover on chunk.type and chunk_connection.relation", order: 3, status: "done" },
        { name: "crud-ui",      title: "CRUD UI at /settings/vocabulary", order: 4, status: "done" },
        { name: "inverse-ui",   title: "Inverse relation labels on chunk detail", order: 5, status: "in_progress" }
    ];
    for (const t of tasks) {
        const id = uuid();
        await ctx.db.insert(planTask).values({
            id,
            planId,
            title: t.title,
            status: t.status ?? "pending",
            order: t.order,
            metadata: {}
        });
        ctx.ids.planTasks[t.name] = id;
    }
    ctx.counters["plan_tasks"] = tasks.length;

    // A couple of analyze items (the kinds the plan detail page groups by).
    const analyzeRows = [
        {
            id: uuid(),
            planId,
            kind: "risk",
            order: 0,
            chunkId: null,
            filePath: null,
            text: "FK cutover could fail if any existing chunk.type doesn't map to a catalog row.",
            metadata: { severity: "medium" }
        },
        {
            id: uuid(),
            planId,
            kind: "assumption",
            order: 1,
            chunkId: null,
            filePath: null,
            text: "Users will edit vocabulary rarely — a per-codebase JSON dump is plenty for now.",
            metadata: { verified: false }
        },
        {
            id: uuid(),
            planId,
            kind: "chunk",
            order: 2,
            chunkId: ctx.ids.chunks["catalog-pattern"] ?? null,
            filePath: null,
            text: "Existing catalog-pattern convention chunk",
            metadata: {}
        }
    ];
    await ctx.db.insert(planAnalyzeItem).values(analyzeRows);
    ctx.counters["plans"] = 1;
}

export async function reset(ctx: SeedContext): Promise<void> {
    await ctx.db.delete(plan).where(eq(plan.userId, ctx.userId));
}
