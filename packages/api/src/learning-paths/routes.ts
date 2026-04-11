import { Effect } from "effect";
import { Elysia, t } from "elysia";

import {
    listLearningPaths,
    getLearningPath,
    createLearningPath,
    updateLearningPath,
    deleteLearningPath,
} from "@fubbik/db/repository";
import { requireSession } from "../require-session";

export const learningPathRoutes = new Elysia()
    .get("/learning-paths", ctx =>
        Effect.runPromise(
            requireSession(ctx).pipe(Effect.flatMap(session => listLearningPaths(session.user.id))),
        ),
    )
    .get("/learning-paths/:id", ctx =>
        Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(session => getLearningPath(ctx.params.id, session.user.id)),
            ),
        ),
    )
    .post(
        "/learning-paths",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session =>
                        createLearningPath({
                            id: crypto.randomUUID(),
                            title: ctx.body.title,
                            description: ctx.body.description,
                            chunkIds: ctx.body.chunkIds,
                            userId: session.user.id,
                        }),
                    ),
                ),
            ),
        {
            body: t.Object({
                title: t.String(),
                description: t.Optional(t.String()),
                chunkIds: t.Array(t.String()),
            }),
        },
    )
    .patch(
        "/learning-paths/:id",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session =>
                        updateLearningPath(ctx.params.id, session.user.id, ctx.body),
                    ),
                ),
            ),
        {
            body: t.Object({
                title: t.Optional(t.String()),
                description: t.Optional(t.String()),
                chunkIds: t.Optional(t.Array(t.String())),
            }),
        },
    )
    .delete("/learning-paths/:id", ctx =>
        Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(session => deleteLearningPath(ctx.params.id, session.user.id)),
            ),
        ),
    );
