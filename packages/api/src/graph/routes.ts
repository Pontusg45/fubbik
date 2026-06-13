import { Effect } from "effect";
import { Elysia, t } from "elysia";

import { requireSession } from "../require-session";
import { computeCommunityRedundancy } from "./community-analysis";
import * as graphService from "./service";
import { getGraphEventsBetween, reconstructGraphAt } from "./timeline-service";

export const graphRoutes = new Elysia()
    .get(
        "/graph",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session => graphService.getUserGraph(session.user.id, ctx.query.codebaseId, ctx.query.workspaceId))
                )
            ),
        { query: t.Object({ codebaseId: t.Optional(t.String()), workspaceId: t.Optional(t.String()) }) }
    )
    .get(
        "/graph/communities",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session => graphService.getUserGraph(session.user.id, ctx.query.codebaseId, ctx.query.workspaceId)),
                    Effect.map(result => result.communities)
                )
            ),
        { query: t.Object({ codebaseId: t.Optional(t.String()), workspaceId: t.Optional(t.String()) }) }
    )
    .get(
        "/graph/bridges",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session => graphService.getUserGraph(session.user.id, ctx.query.codebaseId, ctx.query.workspaceId)),
                    Effect.map(result => result.bridges)
                )
            ),
        { query: t.Object({ codebaseId: t.Optional(t.String()), workspaceId: t.Optional(t.String()) }) }
    )
    .get(
        "/graph/redundancy",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session => graphService.getUserGraph(session.user.id, ctx.query.codebaseId, ctx.query.workspaceId)),
                    Effect.flatMap(result => computeCommunityRedundancy(result.communities))
                )
            ),
        { query: t.Object({ codebaseId: t.Optional(t.String()), workspaceId: t.Optional(t.String()) }) }
    )
    .get(
        "/graph/at",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(() => reconstructGraphAt(new Date(ctx.query.t)))
                )
            ),
        { query: t.Object({ t: t.String() }) }
    )
    .get(
        "/graph/events",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(() => getGraphEventsBetween(new Date(ctx.query.from), new Date(ctx.query.to)))
                )
            ),
        { query: t.Object({ from: t.String(), to: t.String() }) }
    );
