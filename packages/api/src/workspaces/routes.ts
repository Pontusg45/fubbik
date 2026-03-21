import { Effect } from "effect";
import { Elysia, t } from "elysia";

import { requireSession } from "../require-session";
import * as workspaceService from "./service";

export const workspaceRoutes = new Elysia()
    .get("/workspaces", ctx =>
        Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(session => workspaceService.listWorkspaces(session.user.id))
            )
        )
    )
    .post(
        "/workspaces",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session =>
                        workspaceService.createWorkspace(session.user.id, ctx.body)
                    ),
                    Effect.tap(() =>
                        Effect.sync(() => {
                            ctx.set.status = 201;
                        })
                    )
                )
            ),
        {
            body: t.Object({
                name: t.String({ maxLength: 200 }),
                description: t.Optional(t.String({ maxLength: 2000 }))
            })
        }
    )
    .get("/workspaces/:id", ctx =>
        Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(session =>
                    workspaceService.getWorkspaceDetail(ctx.params.id, session.user.id)
                )
            )
        )
    )
    .patch(
        "/workspaces/:id",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session =>
                        workspaceService.updateWorkspace(ctx.params.id, session.user.id, ctx.body)
                    )
                )
            ),
        {
            body: t.Object({
                name: t.Optional(t.String({ maxLength: 200 })),
                description: t.Optional(t.Union([t.String({ maxLength: 2000 }), t.Null()]))
            })
        }
    )
    .delete("/workspaces/:id", ctx =>
        Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(session =>
                    workspaceService.deleteWorkspace(ctx.params.id, session.user.id)
                ),
                Effect.map(() => ({ message: "Deleted" }))
            )
        )
    )
    // ── Workspace Codebases ─────────────────────────────────────────
    .post(
        "/workspaces/:id/codebases",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session =>
                        workspaceService.addCodebaseToWorkspace(
                            ctx.params.id,
                            session.user.id,
                            ctx.body.codebaseId
                        )
                    ),
                    Effect.tap(() =>
                        Effect.sync(() => {
                            ctx.set.status = 201;
                        })
                    )
                )
            ),
        {
            body: t.Object({
                codebaseId: t.String()
            })
        }
    )
    .delete("/workspaces/:id/codebases/:codebaseId", ctx =>
        Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(session =>
                    workspaceService.removeCodebaseFromWorkspace(
                        ctx.params.id,
                        session.user.id,
                        ctx.params.codebaseId
                    )
                ),
                Effect.map(() => ({ message: "Deleted" }))
            )
        )
    );
