import { Effect } from "effect";
import { Elysia, t } from "elysia";

import { requireSession } from "../require-session";
import * as groupService from "./group-service";

export const chunkGroupRoutes = new Elysia()
    .get(
        "/chunks/grouped",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session =>
                        ctx.query.subGroupBy
                            ? groupService.listCompoundGroupedCounts(session.user.id, {
                                  ...ctx.query,
                                  subGroupBy: ctx.query.subGroupBy,
                              })
                            : groupService.listGroupedCounts(session.user.id, ctx.query)
                    )
                )
            ),
        {
            query: t.Object({
                groupBy: t.String(),
                tagTypeId: t.Optional(t.String()),
                subGroupBy: t.Optional(t.String()),
                subTagTypeId: t.Optional(t.String()),
                codebaseId: t.Optional(t.String()),
                workspaceId: t.Optional(t.String()),
                global: t.Optional(t.String()),
                type: t.Optional(t.String()),
                search: t.Optional(t.String()),
                tags: t.Optional(t.String()),
                tagMode: t.Optional(t.Union([t.Literal("any"), t.Literal("all")])),
                origin: t.Optional(t.Union([t.Literal("human"), t.Literal("ai")])),
                reviewStatus: t.Optional(
                    t.Union([t.Literal("draft"), t.Literal("reviewed"), t.Literal("approved")])
                ),
            }),
        }
    )
    .get(
        "/chunks/grouped/:groupName/chunks",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session =>
                        groupService.listGroupChunks(
                            session.user.id,
                            ctx.params.groupName,
                            ctx.query
                        )
                    )
                )
            ),
        {
            query: t.Object({
                groupBy: t.String(),
                tagTypeId: t.Optional(t.String()),
                codebaseId: t.Optional(t.String()),
                workspaceId: t.Optional(t.String()),
                global: t.Optional(t.String()),
                type: t.Optional(t.String()),
                search: t.Optional(t.String()),
                tags: t.Optional(t.String()),
                tagMode: t.Optional(t.Union([t.Literal("any"), t.Literal("all")])),
                origin: t.Optional(t.Union([t.Literal("human"), t.Literal("ai")])),
                reviewStatus: t.Optional(
                    t.Union([t.Literal("draft"), t.Literal("reviewed"), t.Literal("approved")])
                ),
                sort: t.Optional(
                    t.Union([
                        t.Literal("newest"),
                        t.Literal("oldest"),
                        t.Literal("alpha"),
                        t.Literal("updated"),
                    ])
                ),
                limit: t.Optional(t.String()),
                offset: t.Optional(t.String()),
            }),
        }
    );
