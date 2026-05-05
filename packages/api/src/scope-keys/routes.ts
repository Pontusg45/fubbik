import { createScopeKey, deleteScopeKey, listScopeKeys } from "@fubbik/db/repository";
import { Effect } from "effect";
import { Elysia, t } from "elysia";

import { NotFoundError } from "../errors";
import { requireSession } from "../require-session";

export const scopeKeyRoutes = new Elysia()
    .get("/scope-keys", ctx =>
        Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(session => listScopeKeys(session.user.id))
            )
        )
    )
    .post(
        "/scope-keys",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session =>
                        createScopeKey({
                            id: crypto.randomUUID(),
                            userId: session.user.id,
                            key: ctx.body.key,
                            description: ctx.body.description,
                            valueType: ctx.body.valueType,
                            allowedValues: ctx.body.allowedValues,
                        })
                    )
                )
            ),
        {
            body: t.Object({
                key: t.String(),
                description: t.Optional(t.String()),
                valueType: t.Optional(
                    t.Union([
                        t.Literal("string"),
                        t.Literal("number"),
                        t.Literal("boolean"),
                        t.Literal("enum"),
                    ])
                ),
                allowedValues: t.Optional(t.Array(t.String())),
            }),
        }
    )
    .delete(
        "/scope-keys/:id",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session =>
                        deleteScopeKey(ctx.params.id, session.user.id).pipe(
                            Effect.flatMap(deleted =>
                                deleted
                                    ? Effect.succeed({ deleted: true })
                                    : Effect.fail(new NotFoundError({ resource: "ScopeKey" }))
                            )
                        )
                    )
                )
            ),
        {
            params: t.Object({ id: t.String() }),
        }
    );
