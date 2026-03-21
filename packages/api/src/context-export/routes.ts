import { Effect } from "effect";
import { Elysia, t } from "elysia";

import { requireSession } from "../require-session";
import { generateClaudeMd } from "./claude-md";
import * as contextExportService from "./service";

export const contextExportRoutes = new Elysia()
    .get(
        "/chunks/export/context",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session =>
                        contextExportService.exportContext(session.user.id, {
                            codebaseId: ctx.query.codebaseId,
                            maxTokens: ctx.query.maxTokens ? Number(ctx.query.maxTokens) : undefined,
                            format: ctx.query.format as "markdown" | "json" | undefined,
                            forPath: ctx.query.forPath
                        })
                    )
                )
            ),
        {
            query: t.Object({
                codebaseId: t.Optional(t.String()),
                maxTokens: t.Optional(t.String()),
                format: t.Optional(t.Union([t.Literal("markdown"), t.Literal("json")])),
                forPath: t.Optional(t.String())
            })
        }
    )
    .get(
        "/chunks/export/claude-md",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session =>
                        generateClaudeMd({
                            userId: session.user.id,
                            codebaseId: ctx.query.codebaseId,
                            tag: ctx.query.tag
                        })
                    )
                )
            ),
        {
            query: t.Object({
                codebaseId: t.Optional(t.String()),
                tag: t.Optional(t.String())
            })
        }
    );
