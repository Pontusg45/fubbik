import { Effect } from "effect";
import { Elysia, t } from "elysia";

import { requireSession } from "../require-session";
import * as contextExportService from "./service";

export const contextExportRoutes = new Elysia().get(
    "/chunks/export/context",
    ctx =>
        Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(session =>
                    contextExportService.exportContext(session.user.id, {
                        codebaseId: ctx.query.codebaseId,
                        maxTokens: ctx.query.maxTokens ? Number(ctx.query.maxTokens) : undefined,
                        format: ctx.query.format as "markdown" | "json" | undefined
                    })
                )
            )
        ),
    {
        query: t.Object({
            codebaseId: t.Optional(t.String()),
            maxTokens: t.Optional(t.String()),
            format: t.Optional(t.Union([t.Literal("markdown"), t.Literal("json")]))
        })
    }
);
