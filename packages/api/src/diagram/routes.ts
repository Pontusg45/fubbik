import { Effect } from "effect";
import { Elysia, t } from "elysia";

import { requireSession } from "../require-session";
import * as diagramService from "./service";

export const diagramRoutes = new Elysia()
    .get("/codebases/:id/diagram", ctx =>
        Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(session => diagramService.generateDiagram(session.user.id, ctx.params.id))
            )
        )
    )
    .get(
        "/graph/export/mermaid",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session => {
                        const max = ctx.query.maxNodes ? Number(ctx.query.maxNodes) : undefined;
                        const direction = ctx.query.direction === "TB" ? "TB" : "LR";
                        return diagramService.generateDiagram(session.user.id, {
                            codebaseId: ctx.query.codebaseId || undefined,
                            workspaceId: ctx.query.workspaceId || undefined,
                            maxNodes: max && max > 0 ? Math.min(max, 500) : undefined,
                            direction
                        });
                    })
                )
            ),
        {
            query: t.Object({
                codebaseId: t.Optional(t.String()),
                workspaceId: t.Optional(t.String()),
                maxNodes: t.Optional(t.String()),
                direction: t.Optional(t.Union([t.Literal("LR"), t.Literal("TB")]))
            })
        }
    );
