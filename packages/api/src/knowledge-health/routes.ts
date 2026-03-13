import { Effect } from "effect";
import { Elysia, t } from "elysia";

import { requireSession } from "../require-session";
import * as knowledgeHealthService from "./service";

export const knowledgeHealthRoutes = new Elysia().get(
    "/health/knowledge",
    ctx =>
        Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(session =>
                    knowledgeHealthService.getKnowledgeHealth(session.user.id, ctx.query.codebaseId)
                )
            )
        ),
    {
        query: t.Object({
            codebaseId: t.Optional(t.String())
        })
    }
);
