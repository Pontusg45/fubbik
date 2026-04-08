import { Effect } from "effect";
import { Elysia, t } from "elysia";

import { listSavedQueries, createSavedQuery, deleteSavedQuery } from "@fubbik/db/repository";
import { requireSession } from "../require-session";
import { executeSearch, autocomplete } from "./service";
import { parseQueryString } from "./parser";

const ClauseSchema = t.Object({
    field: t.String(),
    operator: t.String(),
    value: t.String(),
    params: t.Optional(t.Record(t.String(), t.String())),
    negate: t.Optional(t.Boolean())
});

export const searchRoutes = new Elysia()
    .post(
        "/search/query",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session =>
                        executeSearch(session.user.id, {
                            clauses: ctx.body.clauses,
                            join: ctx.body.join ?? "and",
                            sort: ctx.body.sort,
                            limit: ctx.body.limit,
                            offset: ctx.body.offset,
                            codebaseId: ctx.body.codebaseId
                        })
                    )
                )
            ),
        {
            body: t.Object({
                clauses: t.Array(ClauseSchema),
                join: t.Optional(t.Union([t.Literal("and"), t.Literal("or")])),
                sort: t.Optional(
                    t.Union([
                        t.Literal("relevance"),
                        t.Literal("newest"),
                        t.Literal("oldest"),
                        t.Literal("updated")
                    ])
                ),
                limit: t.Optional(t.Number()),
                offset: t.Optional(t.Number()),
                codebaseId: t.Optional(t.String())
            })
        }
    )
    .get(
        "/search/parse",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.map(() => ({ clauses: parseQueryString(ctx.query.q) }))
                )
            ),
        {
            query: t.Object({
                q: t.String()
            })
        }
    )
    .get(
        "/search/autocomplete",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session =>
                        autocomplete(session.user.id, ctx.query.field, ctx.query.prefix)
                    )
                )
            ),
        {
            query: t.Object({
                field: t.String(),
                prefix: t.String()
            })
        }
    )
    .get(
        "/search/saved",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session =>
                        listSavedQueries(session.user.id, ctx.query.codebaseId)
                    )
                )
            ),
        {
            query: t.Object({
                codebaseId: t.Optional(t.String())
            })
        }
    )
    .post(
        "/search/saved",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session =>
                        createSavedQuery({
                            id: crypto.randomUUID(),
                            name: ctx.body.name,
                            query: ctx.body.query,
                            userId: session.user.id,
                            codebaseId: ctx.body.codebaseId
                        })
                    )
                )
            ),
        {
            body: t.Object({
                name: t.String({ maxLength: 200 }),
                query: t.Any(),
                codebaseId: t.Optional(t.String())
            })
        }
    )
    .delete(
        "/search/saved/:id",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session =>
                        deleteSavedQuery(ctx.params.id, session.user.id)
                    ),
                    Effect.map(() => ({ message: "Deleted" }))
                )
            )
    );
