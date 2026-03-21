import { Effect } from "effect";
import { Elysia, t } from "elysia";

import { requireSession } from "../require-session";
import * as graphService from "./service";

export const graphRoutes = new Elysia().get(
    "/graph",
    ctx =>
        Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(session =>
                    graphService.getUserGraph(session.user.id, ctx.query.codebaseId, ctx.query.workspaceId)
                )
            )
        ),
    { query: t.Object({ codebaseId: t.Optional(t.String()), workspaceId: t.Optional(t.String()) }) }
);
