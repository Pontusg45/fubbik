import { Elysia, t } from "elysia";
import { Effect } from "effect";

import * as planRepo from "@fubbik/db/repository/plan";
import type { PlanAnalyzeItem, PlanAnalyzeKind } from "@fubbik/db/schema/plan";

import { requireSession } from "../require-session";
import { ValidationError } from "../errors";
import { VALID_ANALYZE_KINDS, getPlan } from "./service";

function groupByKind(items: PlanAnalyzeItem[]) {
    const grouped: Record<PlanAnalyzeKind, PlanAnalyzeItem[]> = {
        chunk: [],
        file: [],
        risk: [],
        assumption: [],
        question: [],
    };
    for (const item of items) {
        if (VALID_ANALYZE_KINDS.includes(item.kind as PlanAnalyzeKind)) {
            grouped[item.kind as PlanAnalyzeKind].push(item);
        }
    }
    return grouped;
}

function validateKind(kind: string): Effect.Effect<PlanAnalyzeKind, ValidationError> {
    if (!VALID_ANALYZE_KINDS.includes(kind as PlanAnalyzeKind)) {
        return Effect.fail(new ValidationError({ message: `Invalid analyze kind: ${kind}` }));
    }
    return Effect.succeed(kind as PlanAnalyzeKind);
}

export const planAnalyzeRoutes = new Elysia({ prefix: "/api/plans/:id/analyze" })
    .get("/", async ctx => {
        return await Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(() => getPlan(ctx.params.id)),
                Effect.flatMap(() => planRepo.listAnalyzeItems(ctx.params.id)),
                Effect.map(groupByKind),
            ),
        );
    })
    .post(
        "/",
        async ctx => {
            return await Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(() => getPlan(ctx.params.id)),
                    Effect.flatMap(() => validateKind(ctx.body.kind)),
                    Effect.flatMap(kind =>
                        planRepo.createAnalyzeItem({
                            planId: ctx.params.id,
                            kind,
                            chunkId: ctx.body.chunkId ?? null,
                            filePath: ctx.body.filePath ?? null,
                            text: ctx.body.text ?? null,
                            metadata: ctx.body.metadata ?? {},
                        }),
                    ),
                ),
            );
        },
        {
            body: t.Object({
                kind: t.String(),
                chunkId: t.Optional(t.String()),
                filePath: t.Optional(t.String()),
                text: t.Optional(t.String()),
                metadata: t.Optional(t.Any()),
            }),
        },
    )
    .patch(
        "/:itemId",
        async ctx => {
            return await Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(() => getPlan(ctx.params.id)),
                    Effect.flatMap(() => planRepo.updateAnalyzeItem(ctx.params.itemId, ctx.body)),
                ),
            );
        },
        {
            body: t.Object({
                text: t.Optional(t.String()),
                metadata: t.Optional(t.Any()),
                chunkId: t.Optional(t.String()),
                filePath: t.Optional(t.String()),
            }),
        },
    )
    .delete("/:itemId", async ctx => {
        await Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(() => getPlan(ctx.params.id)),
                Effect.flatMap(() => planRepo.deleteAnalyzeItem(ctx.params.itemId)),
            ),
        );
        return { ok: true };
    })
    .post(
        "/reorder",
        async ctx => {
            await Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(() => getPlan(ctx.params.id)),
                    Effect.flatMap(() => validateKind(ctx.body.kind)),
                    Effect.flatMap(kind =>
                        planRepo.reorderAnalyzeItems(ctx.params.id, kind, ctx.body.itemIds),
                    ),
                ),
            );
            return { ok: true };
        },
        { body: t.Object({ kind: t.String(), itemIds: t.Array(t.String()) }) },
    );
