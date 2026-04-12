import { Effect } from "effect";
import { Elysia, t } from "elysia";

import { requireSession } from "../require-session";
import {
    createSnapshot,
    deleteSnapshot,
    getSnapshot,
    listSnapshots,
} from "./snapshot-service";

export const snapshotRoutes = new Elysia()
    // POST /context/snapshot — create a frozen context snapshot
    .post(
        "/context/snapshot",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session =>
                        createSnapshot(session.user.id, {
                            planId: ctx.body.planId,
                            taskId: ctx.body.taskId,
                            filePaths: ctx.body.filePaths,
                            concept: ctx.body.concept,
                            maxTokens: ctx.body.maxTokens,
                            codebaseId: ctx.body.codebaseId,
                        }),
                    ),
                ),
            ),
        {
            body: t.Object({
                planId: t.Optional(t.String()),
                taskId: t.Optional(t.String()),
                filePaths: t.Optional(t.Array(t.String())),
                concept: t.Optional(t.String()),
                maxTokens: t.Optional(t.Number()),
                codebaseId: t.Optional(t.String()),
            }),
        },
    )
    // GET /context/snapshot/:id — retrieve a frozen snapshot
    .get(
        "/context/snapshot/:id",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(() => getSnapshot(ctx.params.id)),
                ),
            ),
        {
            params: t.Object({ id: t.String() }),
        },
    )
    // GET /context/snapshots — list user's snapshots
    .get(
        "/context/snapshots",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session => listSnapshots(session.user.id)),
                ),
            ),
    )
    // DELETE /context/snapshot/:id — delete a snapshot
    .delete(
        "/context/snapshot/:id",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(() => deleteSnapshot(ctx.params.id)),
                ),
            ),
        {
            params: t.Object({ id: t.String() }),
        },
    );
