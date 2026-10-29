import { Effect } from "effect";
import { Elysia, t } from "elysia";

import { requireSession } from "../require-session";
import * as generateInstructionsService from "./service";

export const generateInstructionsRoutes = new Elysia().get(
    "/codebases/:id/generate-instructions",
    ctx =>
        Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(session =>
                    generateInstructionsService.generateInstructions(session.user.id, ctx.params.id, {
                        format: ctx.query.format as "claude" | "agents" | "cursor" | undefined
                    })
                )
            )
        ),
    {
        query: t.Object({
            format: t.Optional(t.Union([t.Literal("claude"), t.Literal("agents"), t.Literal("cursor")]))
        })
    }
);
