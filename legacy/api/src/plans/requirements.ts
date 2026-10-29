import * as planRepo from "@fubbik/db/repository/plan";
import { Effect } from "effect";
import { Elysia, t } from "elysia";

import { requireSession } from "../require-session";
import { getPlan } from "./service";

export const planRequirementRoutes = new Elysia({ prefix: "/plans/:id/requirements" })
    .post(
        "/",
        async ctx => {
            return await Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session => getPlan(ctx.params.id, session.user.id)),
                    Effect.flatMap(() => planRepo.addPlanRequirement(ctx.params.id, ctx.body.requirementId))
                )
            );
        },
        { body: t.Object({ requirementId: t.String() }) }
    )
    .delete("/:requirementId", async ctx => {
        await Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(session => getPlan(ctx.params.id, session.user.id)),
                Effect.flatMap(() => planRepo.removePlanRequirement(ctx.params.id, ctx.params.requirementId))
            )
        );
        return { ok: true };
    })
    .post(
        "/reorder",
        async ctx => {
            await Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session => getPlan(ctx.params.id, session.user.id)),
                    Effect.flatMap(() => planRepo.reorderPlanRequirements(ctx.params.id, ctx.body.requirementIds))
                )
            );
            return { ok: true };
        },
        { body: t.Object({ requirementIds: t.Array(t.String()) }) }
    );
